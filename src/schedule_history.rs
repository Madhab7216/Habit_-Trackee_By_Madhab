//! Append-only, effective-dated habit schedule history.
//!
//! Legacy schedules are backfilled as an unverified baseline. New revisions
//! require a valid IANA timezone and are immutable after insertion. A revision
//! applies from its effective date (inclusive) until the next revision's date
//! (exclusive), subject to its own optional inclusive end date.

use crate::timezone::IanaTimeZone;
use jiff::civil::Date;
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params, types::Type};
use std::{error::Error, fmt, io};

const MIGRATION_VERSION: i64 = 1;
const MAX_REASON_CHARS: usize = 240;

const SCHEDULE_HISTORY_MIGRATION: &str = r#"
CREATE TABLE IF NOT EXISTS habit_schedule_revisions (
    id INTEGER PRIMARY KEY,
    habit_id INTEGER NOT NULL REFERENCES habits(id) ON DELETE RESTRICT,
    revision_no INTEGER NOT NULL CHECK (revision_no >= 1),
    effective_from TEXT NOT NULL
        CHECK (date(effective_from) IS NOT NULL AND date(effective_from) = effective_from),
    end_date TEXT
        CHECK (
            end_date IS NULL OR (
                date(end_date) IS NOT NULL
                AND date(end_date) = end_date
                AND end_date >= effective_from
            )
        ),
    schedule_type TEXT NOT NULL
        CHECK (schedule_type IN ('daily', 'weekdays', 'every_n_days', 'times_per_week')),
    weekday_mask INTEGER CHECK (weekday_mask IS NULL OR weekday_mask BETWEEN 1 AND 127),
    interval_days INTEGER CHECK (interval_days IS NULL OR interval_days BETWEEN 1 AND 65535),
    target_per_period INTEGER CHECK (target_per_period IS NULL OR target_per_period BETWEEN 1 AND 7),
    period_unit TEXT NOT NULL DEFAULT 'day'
        CHECK (period_unit IN ('day', 'week', 'month')),
    time_zone_id TEXT NOT NULL CHECK (length(trim(time_zone_id)) BETWEEN 1 AND 128),
    time_zone_status TEXT NOT NULL
        CHECK (time_zone_status IN ('validated_iana', 'legacy_unverified')),
    source TEXT NOT NULL CHECK (source IN ('legacy', 'user')),
    change_reason TEXT NOT NULL DEFAULT '' CHECK (length(change_reason) <= 240),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (habit_id, revision_no),
    UNIQUE (habit_id, effective_from),
    CHECK (schedule_type != 'weekdays' OR weekday_mask IS NOT NULL),
    CHECK (schedule_type != 'every_n_days' OR interval_days IS NOT NULL),
    CHECK (
        schedule_type != 'times_per_week'
        OR (target_per_period IS NOT NULL AND period_unit = 'week')
    )
);

CREATE INDEX IF NOT EXISTS idx_habit_schedule_revisions_effective
    ON habit_schedule_revisions(habit_id, effective_from DESC);

CREATE TRIGGER IF NOT EXISTS habit_schedule_revisions_no_update
BEFORE UPDATE ON habit_schedule_revisions
BEGIN
    SELECT RAISE(ABORT, 'schedule revisions are immutable; append a new revision');
END;

CREATE TRIGGER IF NOT EXISTS habit_schedule_revisions_no_delete
BEFORE DELETE ON habit_schedule_revisions
BEGIN
    SELECT RAISE(ABORT, 'schedule revisions are immutable and retained as history');
END;
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recurrence {
    Daily,
    /// Bit 0 = Monday through bit 6 = Sunday.
    Weekdays {
        weekday_mask: u8,
    },
    EveryNDays {
        interval_days: u16,
    },
    /// Sessions can occur on any day; the quota is per ISO week.
    TimesPerWeek {
        target_per_week: u8,
    },
}

impl Recurrence {
    fn validate(self) -> Result<(), ScheduleHistoryError> {
        match self {
            Self::Weekdays { weekday_mask }
                if weekday_mask == 0 || weekday_mask & !0b0111_1111 != 0 =>
            {
                Err(ScheduleHistoryError::InvalidRecurrence)
            }
            Self::EveryNDays { interval_days: 0 } => Err(ScheduleHistoryError::InvalidRecurrence),
            Self::TimesPerWeek { target_per_week } if !(1..=7).contains(&target_per_week) => {
                Err(ScheduleHistoryError::InvalidRecurrence)
            }
            _ => Ok(()),
        }
    }

    fn encode(
        self,
    ) -> (
        &'static str,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        &'static str,
    ) {
        match self {
            Self::Daily => ("daily", None, None, None, "day"),
            Self::Weekdays { weekday_mask } => {
                ("weekdays", Some(i64::from(weekday_mask)), None, None, "day")
            }
            Self::EveryNDays { interval_days } => (
                "every_n_days",
                None,
                Some(i64::from(interval_days)),
                None,
                "day",
            ),
            Self::TimesPerWeek { target_per_week } => (
                "times_per_week",
                None,
                None,
                Some(i64::from(target_per_week)),
                "week",
            ),
        }
    }

    fn decode(
        schedule_type: &str,
        weekday_mask: Option<i64>,
        interval_days: Option<i64>,
        target_per_period: Option<i64>,
        period_unit: &str,
    ) -> Result<Self, ScheduleHistoryError> {
        let recurrence = match schedule_type {
            "daily" => Self::Daily,
            "weekdays" => Self::Weekdays {
                weekday_mask: weekday_mask
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or(ScheduleHistoryError::CorruptRevision)?,
            },
            "every_n_days" => Self::EveryNDays {
                interval_days: interval_days
                    .and_then(|value| u16::try_from(value).ok())
                    .ok_or(ScheduleHistoryError::CorruptRevision)?,
            },
            "times_per_week" if period_unit == "week" => Self::TimesPerWeek {
                target_per_week: target_per_period
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or(ScheduleHistoryError::CorruptRevision)?,
            },
            _ => return Err(ScheduleHistoryError::CorruptRevision),
        };
        recurrence
            .validate()
            .map_err(|_| ScheduleHistoryError::CorruptRevision)?;
        Ok(recurrence)
    }
}

/// Input for a new schedule revision. A real IANA timezone is required.
#[derive(Debug, Clone)]
pub struct NewScheduleRevision {
    pub effective_from: Date,
    /// The schedule's own end date is inclusive.
    pub end_date: Option<Date>,
    pub recurrence: Recurrence,
    pub time_zone_id: String,
    pub change_reason: String,
}

impl NewScheduleRevision {
    fn validate(&self) -> Result<String, ScheduleHistoryError> {
        self.recurrence.validate()?;
        if self.end_date.is_some_and(|end| end < self.effective_from) {
            return Err(ScheduleHistoryError::EndBeforeStart);
        }
        if self.change_reason.chars().count() > MAX_REASON_CHARS
            || self.change_reason.chars().any(char::is_control)
        {
            return Err(ScheduleHistoryError::InvalidReason);
        }
        let zone = IanaTimeZone::parse(&self.time_zone_id)
            .map_err(|_| ScheduleHistoryError::InvalidTimeZone)?;
        Ok(zone.id().to_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PastEditConfirmation {
    NotConfirmed,
    Confirmed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeZoneStatus {
    ValidatedIana,
    LegacyUnverified,
}

impl TimeZoneStatus {
    fn parse(value: &str) -> Result<Self, ScheduleHistoryError> {
        match value {
            "validated_iana" => Ok(Self::ValidatedIana),
            "legacy_unverified" => Ok(Self::LegacyUnverified),
            _ => Err(ScheduleHistoryError::CorruptRevision),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleRevision {
    pub revision_no: i64,
    pub effective_from: Date,
    /// Exclusive boundary from the next revision, if one exists.
    pub effective_until_exclusive: Option<Date>,
    /// This revision's own optional inclusive end date.
    pub end_date: Option<Date>,
    pub recurrence: Recurrence,
    pub time_zone_id: String,
    pub time_zone_status: TimeZoneStatus,
    pub source: String,
    pub change_reason: String,
}

#[derive(Debug)]
pub enum ScheduleHistoryError {
    Database(rusqlite::Error),
    InvalidDate,
    InvalidRecurrence,
    InvalidTimeZone,
    InvalidReason,
    EndBeforeStart,
    DateBeforeHabitStart,
    PastEditRequiresConfirmation,
    DuplicateEffectiveDate,
    HabitNotFound,
    HabitArchived,
    CorruptRevision,
}

impl fmt::Display for ScheduleHistoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database error: {error}"),
            Self::InvalidDate => write!(formatter, "schedule date is invalid"),
            Self::InvalidRecurrence => write!(formatter, "schedule recurrence is invalid"),
            Self::InvalidTimeZone => {
                write!(formatter, "a valid IANA time-zone identifier is required")
            }
            Self::InvalidReason => {
                write!(formatter, "change reason must be at most 240 characters")
            }
            Self::EndBeforeStart => {
                write!(
                    formatter,
                    "schedule end date cannot precede its effective date"
                )
            }
            Self::DateBeforeHabitStart => {
                write!(formatter, "a revision cannot start before the habit itself")
            }
            Self::PastEditRequiresConfirmation => {
                write!(
                    formatter,
                    "a past-effective schedule edit needs explicit confirmation"
                )
            }
            Self::DuplicateEffectiveDate => {
                write!(
                    formatter,
                    "a schedule revision already exists for that effective date"
                )
            }
            Self::HabitNotFound => write!(formatter, "habit was not found"),
            Self::HabitArchived => {
                write!(
                    formatter,
                    "an archived habit cannot receive a new schedule revision"
                )
            }
            Self::CorruptRevision => write!(formatter, "stored schedule revision is invalid"),
        }
    }
}

impl Error for ScheduleHistoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for ScheduleHistoryError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}

/// Create the schedule-history ledger and append-only table, then backfill a
/// legacy baseline for every existing habit that has no revision.
///
/// Call after the core habit migrations have created the habits table. It is
/// idempotent and can also be called when a new habit is first read.
pub fn initialize(connection: &mut Connection) -> Result<(), ScheduleHistoryError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schedule_history_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );",
    )?;

    let current_version: i64 = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schedule_history_migrations",
        [],
        |row| row.get(0),
    )?;
    if current_version > MIGRATION_VERSION {
        return Err(ScheduleHistoryError::CorruptRevision);
    }

    if current_version < MIGRATION_VERSION {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(SCHEDULE_HISTORY_MIGRATION)?;
        transaction.execute(
            "INSERT INTO schedule_history_migrations(version) VALUES (?1)",
            [MIGRATION_VERSION],
        )?;
        transaction.commit()?;
    }

    // Habits created after the first initialization also receive a baseline.
    // Preserve legacy timezone text verbatim; "local" cannot safely be
    // translated into an IANA identifier after the host setting may have changed.
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute(
        "INSERT INTO habit_schedule_revisions (
            habit_id, revision_no, effective_from, end_date, schedule_type,
            weekday_mask, interval_days, target_per_period, period_unit,
            time_zone_id, time_zone_status, source, change_reason
         )
         SELECT h.id, 1, h.start_date, h.end_date, h.schedule_type,
                h.weekday_mask, h.interval_days, h.target_per_period, h.period_unit,
                h.time_zone_id, 'legacy_unverified', 'legacy',
                'Imported from the pre-revision schedule fields'
         FROM habits h
         WHERE NOT EXISTS (
             SELECT 1 FROM habit_schedule_revisions r WHERE r.habit_id = h.id
         )",
        [],
    )?;
    transaction.commit()?;
    connection.execute_batch(
        "CREATE TRIGGER IF NOT EXISTS habits_schedule_fields_require_revision
         BEFORE UPDATE OF
             schedule_type, weekday_mask, interval_days, target_per_period,
             period_unit, start_date, end_date, time_zone_id
         ON habits
         WHEN EXISTS (
             SELECT 1 FROM habit_schedule_revisions r WHERE r.habit_id = OLD.id
         )
         AND (
             NEW.schedule_type IS NOT OLD.schedule_type
             OR NEW.weekday_mask IS NOT OLD.weekday_mask
             OR NEW.interval_days IS NOT OLD.interval_days
             OR NEW.target_per_period IS NOT OLD.target_per_period
             OR NEW.period_unit IS NOT OLD.period_unit
             OR NEW.start_date IS NOT OLD.start_date
             OR NEW.end_date IS NOT OLD.end_date
             OR NEW.time_zone_id IS NOT OLD.time_zone_id
         )
         BEGIN
             SELECT RAISE(ABORT, 'schedule fields are versioned; append a schedule revision');
         END;",
    )?;
    Ok(())
}

/// Ensure a newly inserted legacy-shaped habit has its baseline schedule revision.
///
/// This is intentionally safe for repeated calls. The application invokes it in the
/// same transaction as habit creation so backup/restore never sees an untracked habit.
pub fn ensure_habit_baseline(
    connection: &Connection,
    habit_id: i64,
) -> Result<(), ScheduleHistoryError> {
    connection.execute(
        "INSERT INTO habit_schedule_revisions (
            habit_id, revision_no, effective_from, end_date, schedule_type,
            weekday_mask, interval_days, target_per_period, period_unit,
            time_zone_id, time_zone_status, source, change_reason
         )
         SELECT h.id, 1, h.start_date, h.end_date, h.schedule_type,
                h.weekday_mask, h.interval_days, h.target_per_period, h.period_unit,
                h.time_zone_id, 'legacy_unverified', 'legacy',
                'Imported from the pre-revision schedule fields'
         FROM habits h
         WHERE h.id = ?1
           AND NOT EXISTS (
               SELECT 1 FROM habit_schedule_revisions r WHERE r.habit_id = h.id
           )",
        [habit_id],
    )?;
    Ok(())
}

/// Create a validated IANA baseline for a brand-new habit.
///
/// Existing rows discovered during a migration must continue to use
/// `initialize()` and remain `legacy_unverified`; this helper is only for
/// the explicit new-habit creation path where the current system zone is known.
pub fn ensure_habit_baseline_with_timezone(
    connection: &Connection,
    habit_id: i64,
    time_zone_id: &str,
) -> Result<(), ScheduleHistoryError> {
    let zone = IanaTimeZone::parse(time_zone_id)
        .map_err(|_| ScheduleHistoryError::InvalidTimeZone)?;
    connection.execute(
        "INSERT INTO habit_schedule_revisions (
            habit_id, revision_no, effective_from, end_date, schedule_type,
            weekday_mask, interval_days, target_per_period, period_unit,
            time_zone_id, time_zone_status, source, change_reason
         )
         SELECT h.id, 1, h.start_date, h.end_date, h.schedule_type,
                h.weekday_mask, h.interval_days, h.target_per_period, h.period_unit,
                ?2, 'validated_iana', 'user',
                'Default IANA zone captured when the habit was created'
         FROM habits h
         WHERE h.id = ?1
           AND NOT EXISTS (
               SELECT 1 FROM habit_schedule_revisions r WHERE r.habit_id = h.id
           )",
        params![habit_id, zone.id()],
    )?;
    Ok(())
}

/// Append an immutable schedule revision.
///
/// The effective date is inclusive. If it is before today, callers must pass
/// Confirmed only after the UI has shown the affected historical period.
/// Check-in rows are never rewritten, but historical analytics will use the
/// newly selected schedule revision.
pub fn append_revision(
    connection: &mut Connection,
    habit_id: i64,
    revision: &NewScheduleRevision,
    today: Date,
    past_confirmation: PastEditConfirmation,
) -> Result<i64, ScheduleHistoryError> {
    let canonical_zone = revision.validate()?;
    if revision.effective_from < today && past_confirmation != PastEditConfirmation::Confirmed {
        return Err(ScheduleHistoryError::PastEditRequiresConfirmation);
    }

    initialize(connection)?;
    let habit: Option<(String, Option<String>)> = connection
        .query_row(
            "SELECT start_date, archived_at FROM habits WHERE id = ?1",
            [habit_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((start_date, archived_at)) = habit else {
        return Err(ScheduleHistoryError::HabitNotFound);
    };
    if archived_at.is_some() {
        return Err(ScheduleHistoryError::HabitArchived);
    }
    if revision.effective_from < parse_date(&start_date)? {
        return Err(ScheduleHistoryError::DateBeforeHabitStart);
    }

    let date_text = revision.effective_from.to_string();
    let duplicate: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM habit_schedule_revisions
            WHERE habit_id = ?1 AND effective_from = ?2
         )",
        params![habit_id, date_text],
        |row| row.get(0),
    )?;
    if duplicate {
        return Err(ScheduleHistoryError::DuplicateEffectiveDate);
    }

    let encoded = revision.recurrence.encode();
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let next_number: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(revision_no), 0) + 1
         FROM habit_schedule_revisions WHERE habit_id = ?1",
        [habit_id],
        |row| row.get(0),
    )?;
    transaction.execute(
        "INSERT INTO habit_schedule_revisions (
            habit_id, revision_no, effective_from, end_date, schedule_type,
            weekday_mask, interval_days, target_per_period, period_unit,
            time_zone_id, time_zone_status, source, change_reason
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                   'validated_iana', 'user', ?11)",
        params![
            habit_id,
            next_number,
            date_text,
            revision.end_date.map(|date| date.to_string()),
            encoded.0,
            encoded.1,
            encoded.2,
            encoded.3,
            encoded.4,
            canonical_zone,
            revision.change_reason
        ],
    )?;
    transaction.commit()?;
    Ok(next_number)
}

/// Return the revision governing a date, including its next-revision boundary.
pub fn schedule_for_date(
    connection: &mut Connection,
    habit_id: i64,
    date: Date,
) -> Result<Option<ScheduleRevision>, ScheduleHistoryError> {
    initialize(connection)?;
    let result = connection
        .query_row(
            "SELECT r.revision_no, r.effective_from,
                    (SELECT MIN(next.effective_from)
                     FROM habit_schedule_revisions next
                     WHERE next.habit_id = r.habit_id
                       AND next.effective_from > r.effective_from) AS effective_until,
                    r.end_date, r.schedule_type, r.weekday_mask, r.interval_days,
                    r.target_per_period, r.period_unit, r.time_zone_id,
                    r.time_zone_status, r.source, r.change_reason
             FROM habit_schedule_revisions r
             WHERE r.habit_id = ?1
               AND r.effective_from <= ?2
               AND NOT EXISTS (
                   SELECT 1 FROM habit_schedule_revisions later
                   WHERE later.habit_id = r.habit_id
                     AND later.effective_from > r.effective_from
                     AND later.effective_from <= ?2
               )
               AND (r.end_date IS NULL OR r.end_date >= ?2)
             ORDER BY r.effective_from DESC
             LIMIT 1",
            params![habit_id, date.to_string()],
            row_to_revision,
        )
        .optional()?;
    Ok(result)
}

/// List revisions in effective-date order with exclusive next-revision bounds.
pub fn revisions_for_habit(
    connection: &mut Connection,
    habit_id: i64,
) -> Result<Vec<ScheduleRevision>, ScheduleHistoryError> {
    initialize(connection)?;
    revisions_for_habit_readonly(connection, habit_id)
}

/// Read schedule revisions without running migrations.
///
/// The application calls this after database initialization. It is intentionally
/// read-only so Today/check-in code can resolve each habit's local date without
/// needing a mutable SQLite borrow.
pub fn revisions_for_habit_readonly(
    connection: &Connection,
    habit_id: i64,
) -> Result<Vec<ScheduleRevision>, ScheduleHistoryError> {
    let mut statement = connection.prepare(
        "SELECT r.revision_no, r.effective_from,
                (SELECT MIN(next.effective_from)
                 FROM habit_schedule_revisions next
                 WHERE next.habit_id = r.habit_id
                   AND next.effective_from > r.effective_from) AS effective_until,
                r.end_date, r.schedule_type, r.weekday_mask, r.interval_days,
                r.target_per_period, r.period_unit, r.time_zone_id,
                r.time_zone_status, r.source, r.change_reason
         FROM habit_schedule_revisions r
         WHERE r.habit_id = ?1
         ORDER BY r.effective_from, r.revision_no",
    )?;
    let rows = statement.query_map([habit_id], row_to_revision)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn row_to_revision(row: &Row<'_>) -> rusqlite::Result<ScheduleRevision> {
    let schedule_type: String = row.get(4)?;
    let weekday_mask: Option<i64> = row.get(5)?;
    let interval_days: Option<i64> = row.get(6)?;
    let target_per_period: Option<i64> = row.get(7)?;
    let period_unit: String = row.get(8)?;
    let recurrence = Recurrence::decode(
        &schedule_type,
        weekday_mask,
        interval_days,
        target_per_period,
        &period_unit,
    )
    .map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            Type::Text,
            Box::new(io::Error::new(
                io::ErrorKind::InvalidData,
                error.to_string(),
            )),
        )
    })?;

    let time_zone_status: String = row.get(10)?;
    let time_zone_status = TimeZoneStatus::parse(&time_zone_status).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            10,
            Type::Text,
            Box::new(io::Error::new(
                io::ErrorKind::InvalidData,
                error.to_string(),
            )),
        )
    })?;

    Ok(ScheduleRevision {
        revision_no: row.get(0)?,
        effective_from: parse_date_sql(row.get(1)?, 1)?,
        effective_until_exclusive: row
            .get::<_, Option<String>>(2)?
            .map(|value| parse_date_sql(value, 2))
            .transpose()?,
        end_date: row
            .get::<_, Option<String>>(3)?
            .map(|value| parse_date_sql(value, 3))
            .transpose()?,
        recurrence,
        time_zone_id: row.get(9)?,
        time_zone_status,
        source: row.get(11)?,
        change_reason: row.get(12)?,
    })
}

fn parse_date(value: &str) -> Result<Date, ScheduleHistoryError> {
    let date = value
        .parse::<Date>()
        .map_err(|_| ScheduleHistoryError::InvalidDate)?;
    if date.to_string() != value {
        return Err(ScheduleHistoryError::InvalidDate);
    }
    Ok(date)
}

fn parse_date_sql(value: String, column: usize) -> rusqlite::Result<Date> {
    value.parse::<Date>().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(column, Type::Text, Box::new(error))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    fn memory_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .expect("enable foreign keys");
        connection
            .execute_batch(include_str!("../migrations/0001_initial.sql"))
            .expect("core schema");
        connection
            .execute_batch(include_str!("../migrations/0002_pause_periods.sql"))
            .expect("pause schema");
        connection
            .execute_batch(include_str!("../migrations/0003_goal_milestones.sql"))
            .expect("goal milestone schema");
        connection
    }

    fn add_habit(connection: &Connection, zone: &str) -> i64 {
        connection
            .execute(
                "INSERT INTO habits(name, start_date, time_zone_id)
                 VALUES ('Schedule history test', '2026-11-01', ?1)",
                [zone],
            )
            .expect("insert habit");
        connection.last_insert_rowid()
    }

    fn revision(
        effective_from: Date,
        recurrence: Recurrence,
        zone: &str,
        end_date: Option<Date>,
    ) -> NewScheduleRevision {
        NewScheduleRevision {
            effective_from,
            end_date,
            recurrence,
            time_zone_id: zone.to_owned(),
            change_reason: "User changed the recurrence".to_owned(),
        }
    }

    #[test]
    fn backfills_legacy_schedules_and_initialization_is_idempotent() {
        let mut connection = memory_connection();
        let habit_id = add_habit(&connection, "local");

        initialize(&mut connection).expect("initialize");
        initialize(&mut connection).expect("initialize again");

        let revisions = revisions_for_habit(&mut connection, habit_id).expect("load revisions");
        assert_eq!(revisions.len(), 1);
        assert_eq!(revisions[0].revision_no, 1);
        assert_eq!(revisions[0].effective_from, date(2026, 11, 1));
        assert_eq!(revisions[0].time_zone_id, "local");
        assert_eq!(
            revisions[0].time_zone_status,
            TimeZoneStatus::LegacyUnverified
        );
        assert_eq!(revisions[0].source, "legacy");
    }

    #[test]
    fn legacy_schedule_columns_cannot_be_overwritten_after_history_backfill() {
        let mut connection = memory_connection();
        let habit_id = add_habit(&connection, "local");
        initialize(&mut connection).expect("initialize");
        let result = connection.execute(
            "UPDATE habits SET schedule_type = 'weekdays', weekday_mask = 1
             WHERE id = ?1",
            [habit_id],
        );
        assert!(result.is_err());
    }

    #[test]
    fn next_revision_only_applies_from_its_effective_date() {
        let mut connection = memory_connection();
        let habit_id = add_habit(&connection, "local");
        initialize(&mut connection).expect("initialize");

        let next = revision(
            date(2026, 11, 20),
            Recurrence::Weekdays {
                weekday_mask: 0b001_1111,
            },
            "Asia/Kolkata",
            None,
        );
        let number = append_revision(
            &mut connection,
            habit_id,
            &next,
            date(2026, 11, 17),
            PastEditConfirmation::NotConfirmed,
        )
        .expect("append future revision");
        assert_eq!(number, 2);

        let old = schedule_for_date(&mut connection, habit_id, date(2026, 11, 19))
            .expect("query old schedule")
            .expect("old schedule exists");
        assert_eq!(old.revision_no, 1);
        assert_eq!(old.effective_until_exclusive, Some(date(2026, 11, 20)));
        assert_eq!(old.time_zone_status, TimeZoneStatus::LegacyUnverified);

        let future = schedule_for_date(&mut connection, habit_id, date(2026, 11, 20))
            .expect("query future schedule")
            .expect("future schedule exists");
        assert_eq!(future.revision_no, 2);
        assert_eq!(future.time_zone_id, "Asia/Kolkata");
        assert_eq!(future.time_zone_status, TimeZoneStatus::ValidatedIana);
        assert_eq!(
            future.recurrence,
            Recurrence::Weekdays {
                weekday_mask: 0b001_1111
            }
        );
    }

    #[test]
    fn an_expired_later_revision_does_not_resurrect_an_older_schedule() {
        let mut connection = memory_connection();
        let habit_id = add_habit(&connection, "local");
        initialize(&mut connection).expect("initialize");

        let temporary = revision(
            date(2026, 11, 10),
            Recurrence::Weekdays {
                weekday_mask: 0b001_1111,
            },
            "Asia/Kolkata",
            Some(date(2026, 11, 12)),
        );
        append_revision(
            &mut connection,
            habit_id,
            &temporary,
            date(2026, 11, 5),
            PastEditConfirmation::NotConfirmed,
        )
        .expect("append a temporary revision");

        let baseline = schedule_for_date(&mut connection, habit_id, date(2026, 11, 9))
            .expect("query baseline")
            .expect("baseline remains active before next revision");
        assert_eq!(baseline.revision_no, 1);

        let active = schedule_for_date(&mut connection, habit_id, date(2026, 11, 12))
            .expect("query temporary revision")
            .expect("temporary revision applies through its inclusive end date");
        assert_eq!(active.revision_no, 2);

        assert!(
            schedule_for_date(&mut connection, habit_id, date(2026, 11, 13))
                .expect("query after temporary revision")
                .is_none(),
            "the old baseline must not become active again after revision 2 expires"
        );
    }

    #[test]
    fn retroactive_edit_requires_confirmation_and_keeps_saved_checkins() {
        let mut connection = memory_connection();
        let habit_id = add_habit(&connection, "local");
        initialize(&mut connection).expect("initialize");
        connection
            .execute(
                "INSERT INTO habit_entries(habit_id, local_date, status, value)
                 VALUES (?1, '2026-11-10', 'completed', 1.0)",
                [habit_id],
            )
            .expect("insert existing check-in");

        let past = revision(
            date(2026, 11, 5),
            Recurrence::EveryNDays { interval_days: 2 },
            "Asia/Kolkata",
            None,
        );
        assert!(matches!(
            append_revision(
                &mut connection,
                habit_id,
                &past,
                date(2026, 11, 17),
                PastEditConfirmation::NotConfirmed,
            ),
            Err(ScheduleHistoryError::PastEditRequiresConfirmation)
        ));

        append_revision(
            &mut connection,
            habit_id,
            &past,
            date(2026, 11, 17),
            PastEditConfirmation::Confirmed,
        )
        .expect("append confirmed historical revision");

        let saved: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM habit_entries
                 WHERE habit_id = ?1 AND local_date = '2026-11-10' AND status = 'completed'",
                [habit_id],
                |row| row.get(0),
            )
            .expect("count check-ins");
        assert_eq!(saved, 1);
        let history = revisions_for_habit(&mut connection, habit_id).expect("read history");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].effective_from, date(2026, 11, 1));
        assert_eq!(history[1].effective_from, date(2026, 11, 5));
    }

    #[test]
    fn rejects_invalid_zone_recurrence_end_date_and_reason() {
        let mut connection = memory_connection();
        let habit_id = add_habit(&connection, "local");
        initialize(&mut connection).expect("initialize");

        let invalid_zone = revision(
            date(2026, 11, 20),
            Recurrence::Daily,
            "Mars/OlympusMons",
            None,
        );
        assert!(matches!(
            append_revision(
                &mut connection,
                habit_id,
                &invalid_zone,
                date(2026, 11, 17),
                PastEditConfirmation::NotConfirmed,
            ),
            Err(ScheduleHistoryError::InvalidTimeZone)
        ));

        let invalid_mask = revision(
            date(2026, 11, 20),
            Recurrence::Weekdays { weekday_mask: 0 },
            "Asia/Kolkata",
            None,
        );
        assert!(matches!(
            append_revision(
                &mut connection,
                habit_id,
                &invalid_mask,
                date(2026, 11, 17),
                PastEditConfirmation::NotConfirmed,
            ),
            Err(ScheduleHistoryError::InvalidRecurrence)
        ));

        let invalid_end = revision(
            date(2026, 11, 20),
            Recurrence::Daily,
            "Asia/Kolkata",
            Some(date(2026, 11, 19)),
        );
        assert!(matches!(
            append_revision(
                &mut connection,
                habit_id,
                &invalid_end,
                date(2026, 11, 17),
                PastEditConfirmation::NotConfirmed,
            ),
            Err(ScheduleHistoryError::EndBeforeStart)
        ));

        let mut invalid_reason =
            revision(date(2026, 11, 20), Recurrence::Daily, "Asia/Kolkata", None);
        invalid_reason.change_reason = "x".repeat(MAX_REASON_CHARS + 1);
        assert!(matches!(
            append_revision(
                &mut connection,
                habit_id,
                &invalid_reason,
                date(2026, 11, 17),
                PastEditConfirmation::NotConfirmed,
            ),
            Err(ScheduleHistoryError::InvalidReason)
        ));
    }

    #[test]
    fn new_revision_must_not_precede_habit_start_or_duplicate_an_effective_date() {
        let mut connection = memory_connection();
        let habit_id = add_habit(&connection, "local");
        initialize(&mut connection).expect("initialize");

        let before_start = revision(date(2026, 10, 31), Recurrence::Daily, "Asia/Kolkata", None);
        assert!(matches!(
            append_revision(
                &mut connection,
                habit_id,
                &before_start,
                date(2026, 10, 1),
                PastEditConfirmation::NotConfirmed,
            ),
            Err(ScheduleHistoryError::DateBeforeHabitStart)
        ));

        let duplicate = revision(date(2026, 11, 1), Recurrence::Daily, "Asia/Kolkata", None);
        assert!(matches!(
            append_revision(
                &mut connection,
                habit_id,
                &duplicate,
                date(2026, 11, 1),
                PastEditConfirmation::Confirmed,
            ),
            Err(ScheduleHistoryError::DuplicateEffectiveDate)
        ));
    }

    #[test]
    fn schedule_rows_cannot_be_updated_or_deleted() {
        let mut connection = memory_connection();
        let habit_id = add_habit(&connection, "local");
        initialize(&mut connection).expect("initialize");

        assert!(
            connection
                .execute(
                    "UPDATE habit_schedule_revisions SET time_zone_id = 'Asia/Kolkata'
                 WHERE habit_id = ?1",
                    [habit_id],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "DELETE FROM habit_schedule_revisions WHERE habit_id = ?1",
                    [habit_id],
                )
                .is_err()
        );
    }

    #[test]
    fn revisions_reject_unknown_or_archived_habits() {
        let mut connection = memory_connection();
        let habit_id = add_habit(&connection, "local");
        initialize(&mut connection).expect("initialize");
        let next = revision(date(2026, 11, 20), Recurrence::Daily, "Asia/Kolkata", None);

        assert!(matches!(
            append_revision(
                &mut connection,
                999,
                &next,
                date(2026, 11, 17),
                PastEditConfirmation::NotConfirmed,
            ),
            Err(ScheduleHistoryError::HabitNotFound)
        ));

        connection
            .execute(
                "UPDATE habits SET archived_at = CURRENT_TIMESTAMP WHERE id = ?1",
                [habit_id],
            )
            .expect("archive habit");
        assert!(matches!(
            append_revision(
                &mut connection,
                habit_id,
                &next,
                date(2026, 11, 17),
                PastEditConfirmation::NotConfirmed,
            ),
            Err(ScheduleHistoryError::HabitArchived)
        ));
    }
}
