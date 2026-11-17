//! Validated, local-first measurement configuration and check-in persistence.
//!
//! The base schema already contains measurement columns. This module supplies the
//! missing typed API without changing existing rows or weakening the binary workflow.
//! GTK entry forms can call these functions in a separate UI increment.

use crate::{
    db::{self, TodayHabit},
    domain::{Cadence, CivilDate, Direction, achievement_ratio},
};
use rusqlite::{Connection, OptionalExtension, params};
use std::{error::Error, fmt};

const MAX_UNIT_CHARS: usize = 24;
const MAX_NOTE_CHARS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementKind {
    Numeric,
    Duration,
    Count,
    Limit,
}

impl MeasurementKind {
    fn as_db_value(self) -> &'static str {
        match self {
            Self::Numeric => "numeric",
            Self::Duration => "duration",
            Self::Count => "count",
            Self::Limit => "limit",
        }
    }

    fn from_db_value(value: &str) -> Option<Self> {
        match value {
            "numeric" => Some(Self::Numeric),
            "duration" => Some(Self::Duration),
            "count" => Some(Self::Count),
            "limit" => Some(Self::Limit),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementDirection {
    AtLeast,
    AtMost,
}

impl MeasurementDirection {
    fn as_db_value(self) -> &'static str {
        match self {
            Self::AtLeast => "at_least",
            Self::AtMost => "at_most",
        }
    }

    fn from_db_value(value: &str) -> Option<Self> {
        match value {
            "at_least" => Some(Self::AtLeast),
            "at_most" => Some(Self::AtMost),
            _ => None,
        }
    }

    fn as_domain_direction(self) -> Direction {
        match self {
            Self::AtLeast => Direction::AtLeast,
            Self::AtMost => Direction::AtMost,
        }
    }
}

/// A measurable habit's target and display unit.
///
/// Targets must be finite and greater than zero. A zero-limit "avoid entirely"
/// habit is intentionally rejected for now because the existing scoring v1
/// contract treats zero targets as invalid; it must be enabled together with
/// a versioned scoring-policy change rather than silently producing bad scores.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasurementSpec {
    pub kind: MeasurementKind,
    pub direction: MeasurementDirection,
    pub target: f64,
    pub unit: String,
}

impl MeasurementSpec {
    pub fn validate(&self) -> Result<String, MeasurementError> {
        if !self.target.is_finite() || self.target <= 0.0 {
            return Err(MeasurementError::InvalidTarget);
        }

        if self.kind == MeasurementKind::Limit && self.direction != MeasurementDirection::AtMost {
            return Err(MeasurementError::LimitMustBeAtMost);
        }

        let unit = self.unit.trim();
        let count = unit.chars().count();
        if count == 0 || count > MAX_UNIT_CHARS || unit.chars().any(char::is_control) {
            return Err(MeasurementError::InvalidUnit);
        }

        Ok(unit.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeasurementConfig {
    pub kind: MeasurementKind,
    pub direction: MeasurementDirection,
    pub target: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementStatus {
    Completed,
    Partial,
}

impl MeasurementStatus {
    fn as_db_value(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Partial => "partial",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeasurementOutcome {
    pub status: MeasurementStatus,
    pub actual: f64,
    pub target: f64,
    /// Normalized to the inclusive range 0.0..=1.0 using scoring v1's direction.
    pub achievement_ratio: f64,
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeasurementRecord {
    pub status: MeasurementStatus,
    pub actual: f64,
    pub note: String,
}

#[derive(Debug)]
pub enum MeasurementError {
    Database(rusqlite::Error),
    UnsupportedCadence,
    InvalidTarget,
    InvalidActual,
    InvalidUnit,
    InvalidNote,
    LimitMustBeAtMost,
    NotFound,
    NotMeasurable,
    Paused,
    Unscheduled,
    InvalidStoredConfiguration,
}

impl fmt::Display for MeasurementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database error: {error}"),
            Self::UnsupportedCadence => {
                write!(
                    formatter,
                    "measurable habits cannot use a weekly-frequency cadence"
                )
            }
            Self::InvalidTarget => write!(
                formatter,
                "target must be a finite number greater than zero"
            ),
            Self::InvalidActual => write!(
                formatter,
                "measurement must be a finite number greater than or equal to zero"
            ),
            Self::InvalidUnit => write!(
                formatter,
                "unit must contain 1 to 24 non-control characters"
            ),
            Self::InvalidNote => write!(formatter, "note must not exceed 500 characters"),
            Self::LimitMustBeAtMost => {
                write!(formatter, "limit habits must use the at-most direction")
            }
            Self::NotFound => write!(formatter, "active habit was not found"),
            Self::NotMeasurable => write!(formatter, "habit does not accept numeric measurements"),
            Self::Paused => write!(formatter, "a paused habit cannot be checked in"),
            Self::Unscheduled => write!(formatter, "habit is not scheduled on this date"),
            Self::InvalidStoredConfiguration => {
                write!(formatter, "stored measurement configuration is invalid")
            }
        }
    }
}

impl Error for MeasurementError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for MeasurementError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}

/// Create a measured habit and its configuration atomically.
///
/// A failed name, schedule, or measurement validation leaves no partial habit row.
/// Weekly-frequency habits remain a separate tracking type because their target
/// is a count of sessions per ISO week rather than a per-check-in measured value.
pub fn create_measurable_habit(
    connection: &mut Connection,
    name: &str,
    cadence: Cadence,
    spec: MeasurementSpec,
) -> Result<i64, MeasurementError> {
    let unit = spec.validate()?;
    if matches!(cadence, Cadence::TimesPerWeek { .. }) {
        return Err(MeasurementError::UnsupportedCadence);
    }

    let transaction = connection.transaction()?;
    db::create_habit_with_cadence(&transaction, name, cadence)?;
    let habit_id = transaction.last_insert_rowid();

    let changed = transaction.execute(
        "UPDATE habits
         SET tracking_type = ?1, direction = ?2, target_value = ?3, unit = ?4,
             updated_at = CURRENT_TIMESTAMP
         WHERE id = ?5 AND tracking_type = 'binary'",
        params![
            spec.kind.as_db_value(),
            spec.direction.as_db_value(),
            spec.target,
            unit,
            habit_id
        ],
    )?;

    if changed != 1 {
        return Err(MeasurementError::InvalidStoredConfiguration);
    }

    transaction.commit()?;
    Ok(habit_id)
}

/// Read and validate a habit's saved measurement configuration.
pub fn measurement_config(
    connection: &Connection,
    habit_id: i64,
) -> Result<MeasurementConfig, MeasurementError> {
    let saved: Option<(String, String, Option<f64>, Option<String>, Option<String>)> = connection
        .query_row(
            "SELECT tracking_type, direction, target_value, unit, archived_at
             FROM habits WHERE id = ?1",
            [habit_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?;

    let Some((kind_value, direction_value, target, unit, archived_at)) = saved else {
        return Err(MeasurementError::NotFound);
    };
    if archived_at.is_some() {
        return Err(MeasurementError::NotFound);
    }

    let kind =
        MeasurementKind::from_db_value(&kind_value).ok_or(MeasurementError::NotMeasurable)?;
    let direction = MeasurementDirection::from_db_value(&direction_value)
        .ok_or(MeasurementError::InvalidStoredConfiguration)?;
    let target = target.ok_or(MeasurementError::InvalidStoredConfiguration)?;
    let unit = unit.ok_or(MeasurementError::InvalidStoredConfiguration)?;
    let spec = MeasurementSpec {
        kind,
        direction,
        target,
        unit,
    };
    let unit = spec
        .validate()
        .map_err(|_| MeasurementError::InvalidStoredConfiguration)?;

    Ok(MeasurementConfig {
        kind,
        direction,
        target,
        unit,
    })
}

/// Read the machine's current SQLite local civil date using the same source
/// as the existing Today and schedule workflow.
pub fn local_today_date(connection: &Connection) -> Result<CivilDate, MeasurementError> {
    let value: String =
        connection.query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))?;
    let mut parts = value.split('-');
    let year = parts
        .next()
        .and_then(|part| part.parse::<i32>().ok())
        .ok_or(MeasurementError::InvalidStoredConfiguration)?;
    let month = parts
        .next()
        .and_then(|part| part.parse::<u8>().ok())
        .ok_or(MeasurementError::InvalidStoredConfiguration)?;
    let day = parts
        .next()
        .and_then(|part| part.parse::<u8>().ok())
        .ok_or(MeasurementError::InvalidStoredConfiguration)?;
    if parts.next().is_some() {
        return Err(MeasurementError::InvalidStoredConfiguration);
    }
    CivilDate::new(year, month, day).ok_or(MeasurementError::InvalidStoredConfiguration)
}

/// Return the saved completed/partial value for a measurable habit and date.
pub fn measurement_for_date(
    connection: &Connection,
    habit_id: i64,
    date: CivilDate,
) -> Result<Option<MeasurementRecord>, MeasurementError> {
    let saved: Option<(String, Option<f64>, String)> = connection
        .query_row(
            "SELECT status, value, note FROM habit_entries
             WHERE habit_id = ?1 AND local_date = ?2",
            params![habit_id, date.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;

    let Some((status, actual, note)) = saved else {
        return Ok(None);
    };
    let status = match status.as_str() {
        "completed" => MeasurementStatus::Completed,
        "partial" => MeasurementStatus::Partial,
        "skipped" => return Ok(None),
        _ => return Err(MeasurementError::InvalidStoredConfiguration),
    };
    let actual = actual.ok_or(MeasurementError::InvalidStoredConfiguration)?;
    if !actual.is_finite() || actual < 0.0 {
        return Err(MeasurementError::InvalidStoredConfiguration);
    }

    Ok(Some(MeasurementRecord {
        status,
        actual,
        note,
    }))
}

/// Record a measured value for an eligible local calendar date.
///
/// The saved status is derived from the configured direction: meeting the target
/// is "completed"; falling short is "partial". The actual value is always retained.
/// Duplicate submissions for the same habit/date update that date's single entry.
pub fn record_measurement_for_date(
    connection: &Connection,
    habit_id: i64,
    date: CivilDate,
    actual: f64,
    note: &str,
) -> Result<MeasurementOutcome, MeasurementError> {
    if !actual.is_finite() || actual < 0.0 {
        return Err(MeasurementError::InvalidActual);
    }
    let note = note.trim();
    if note.chars().count() > MAX_NOTE_CHARS {
        return Err(MeasurementError::InvalidNote);
    }

    let config = measurement_config(connection, habit_id)?;
    let today_habit: Option<TodayHabit> = db::habits_for_date(connection, date)?
        .into_iter()
        .find(|habit| habit.id == habit_id);

    let Some(today_habit) = today_habit else {
        return Err(MeasurementError::NotFound);
    };
    if today_habit.paused {
        return Err(MeasurementError::Paused);
    }
    if !today_habit.scheduled {
        return Err(MeasurementError::Unscheduled);
    }

    let ratio = achievement_ratio(
        actual,
        config.target,
        config.direction.as_domain_direction(),
    )
    .map_err(|_| MeasurementError::InvalidStoredConfiguration)?;

    let status = if ratio >= 1.0 {
        MeasurementStatus::Completed
    } else {
        MeasurementStatus::Partial
    };

    connection.execute(
        "INSERT INTO habit_entries(habit_id, local_date, status, value, note)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(habit_id, local_date) DO UPDATE SET
            status = excluded.status,
            value = excluded.value,
            note = excluded.note,
            updated_at = CURRENT_TIMESTAMP",
        params![
            habit_id,
            date.to_string(),
            status.as_db_value(),
            actual,
            note
        ],
    )?;

    Ok(MeasurementOutcome {
        status,
        actual,
        target: config.target,
        achievement_ratio: ratio,
        unit: config.unit,
    })
}

/// Undo only a measured completion/partial entry. A deliberate skipped record is
/// not removed by this function.
pub fn undo_measurement_for_date(
    connection: &Connection,
    habit_id: i64,
    date: CivilDate,
) -> Result<bool, MeasurementError> {
    let changed = connection.execute(
        "DELETE FROM habit_entries
         WHERE habit_id = ?1 AND local_date = ?2
           AND status IN ('completed', 'partial')",
        params![habit_id, date.to_string()],
    )?;
    Ok(changed == 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn memory_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .expect("enable foreign keys");
        connection
            .execute_batch(include_str!("../migrations/0001_initial.sql"))
            .expect("apply base migration");
        connection
            .execute_batch(include_str!("../migrations/0002_pause_periods.sql"))
            .expect("apply pause migration");
        connection
    }

    fn today(connection: &Connection) -> CivilDate {
        let value: String = connection
            .query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))
            .expect("read local date");
        parse_date(&value)
    }

    fn parse_date(value: &str) -> CivilDate {
        let mut parts = value.split('-');
        CivilDate::new(
            parts.next().expect("year").parse().expect("valid year"),
            parts.next().expect("month").parse().expect("valid month"),
            parts.next().expect("day").parse().expect("valid day"),
        )
        .expect("valid civil date")
    }

    fn spec(
        kind: MeasurementKind,
        direction: MeasurementDirection,
        target: f64,
        unit: &str,
    ) -> MeasurementSpec {
        MeasurementSpec {
            kind,
            direction,
            target,
            unit: unit.to_string(),
        }
    }

    fn new_habit(connection: &mut Connection, measurement: MeasurementSpec) -> i64 {
        create_measurable_habit(
            connection,
            "Sample measured habit",
            Cadence::Daily,
            measurement,
        )
        .expect("create measurable habit")
    }

    #[test]
    fn creation_persists_measurement_type_target_direction_and_unit() {
        let mut connection = memory_connection();
        let id = new_habit(
            &mut connection,
            spec(
                MeasurementKind::Numeric,
                MeasurementDirection::AtLeast,
                1800.0,
                "ml",
            ),
        );

        let config = measurement_config(&connection, id).expect("read configuration");
        assert_eq!(config.kind, MeasurementKind::Numeric);
        assert_eq!(config.direction, MeasurementDirection::AtLeast);
        assert_eq!(config.target, 1800.0);
        assert_eq!(config.unit, "ml");
    }

    #[test]
    fn at_least_measurement_is_completed_only_when_target_is_met() {
        let mut connection = memory_connection();
        let id = new_habit(
            &mut connection,
            spec(
                MeasurementKind::Duration,
                MeasurementDirection::AtLeast,
                30.0,
                "minutes",
            ),
        );
        let date = today(&connection);

        let partial = record_measurement_for_date(&connection, id, date, 12.0, "Short session")
            .expect("save partial duration");
        assert_eq!(partial.status, MeasurementStatus::Partial);
        assert_eq!(partial.achievement_ratio, 0.4);

        let completed = record_measurement_for_date(&connection, id, date, 35.0, "Target met")
            .expect("update the same day's measurement");
        assert_eq!(completed.status, MeasurementStatus::Completed);
        assert_eq!(completed.achievement_ratio, 1.0);

        let saved: (String, f64, String) = connection
            .query_row(
                "SELECT status, value, note FROM habit_entries
                 WHERE habit_id = ?1 AND local_date = ?2",
                params![id, date.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read saved measurement");
        assert_eq!(
            saved,
            ("completed".to_string(), 35.0, "Target met".to_string())
        );
    }

    #[test]
    fn limit_measurement_uses_at_most_direction_and_never_rewards_higher_use() {
        let mut connection = memory_connection();
        let id = new_habit(
            &mut connection,
            spec(
                MeasurementKind::Limit,
                MeasurementDirection::AtMost,
                2.0,
                "cups",
            ),
        );
        let date = today(&connection);

        let over_limit = record_measurement_for_date(&connection, id, date, 3.0, "")
            .expect("save an over-limit value");
        assert_eq!(over_limit.status, MeasurementStatus::Partial);
        assert!((over_limit.achievement_ratio - (2.0 / 3.0)).abs() < 1e-12);

        let within_limit = record_measurement_for_date(&connection, id, date, 1.0, "")
            .expect("replace with a value within the limit");
        assert_eq!(within_limit.status, MeasurementStatus::Completed);
        assert_eq!(within_limit.achievement_ratio, 1.0);
    }

    #[test]
    fn count_measurements_are_stored_as_values_not_binary_checkboxes() {
        let mut connection = memory_connection();
        let id = new_habit(
            &mut connection,
            spec(
                MeasurementKind::Count,
                MeasurementDirection::AtLeast,
                10.0,
                "pages",
            ),
        );
        let outcome = record_measurement_for_date(&connection, id, today(&connection), 7.0, "")
            .expect("save count measurement");

        assert_eq!(outcome.status, MeasurementStatus::Partial);
        assert_eq!(outcome.actual, 7.0);
        assert_eq!(outcome.unit, "pages");
    }

    #[test]
    fn rejects_invalid_values_targets_units_and_limit_direction() {
        let mut connection = memory_connection();

        for target in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let invalid = spec(
                MeasurementKind::Numeric,
                MeasurementDirection::AtLeast,
                target,
                "pages",
            );
            assert!(matches!(
                create_measurable_habit(&mut connection, "Invalid target", Cadence::Daily, invalid),
                Err(MeasurementError::InvalidTarget)
            ));
        }

        let long_unit = "a".repeat(25);
        for unit in ["", "   ", long_unit.as_str(), "bad\nunit"] {
            let invalid = spec(
                MeasurementKind::Numeric,
                MeasurementDirection::AtLeast,
                1.0,
                unit,
            );
            assert!(matches!(
                create_measurable_habit(&mut connection, "Invalid unit", Cadence::Daily, invalid),
                Err(MeasurementError::InvalidUnit)
            ));
        }

        let invalid_limit = spec(
            MeasurementKind::Limit,
            MeasurementDirection::AtLeast,
            1.0,
            "cups",
        );
        assert!(matches!(
            create_measurable_habit(
                &mut connection,
                "Invalid limit",
                Cadence::Daily,
                invalid_limit
            ),
            Err(MeasurementError::LimitMustBeAtMost)
        ));
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM habits", [], |row| row.get(0))
            .expect("count habits");
        assert_eq!(count, 0, "invalid settings must not leave partial records");
    }

    #[test]
    fn measurable_habit_rejects_weekly_frequency_cadence() {
        let mut connection = memory_connection();
        let result = create_measurable_habit(
            &mut connection,
            "Mixed targets",
            Cadence::TimesPerWeek { target: 3 },
            spec(
                MeasurementKind::Count,
                MeasurementDirection::AtLeast,
                5.0,
                "pages",
            ),
        );
        assert!(matches!(result, Err(MeasurementError::UnsupportedCadence)));
    }

    #[test]
    fn paused_and_unscheduled_dates_reject_measurements() {
        let mut connection = memory_connection();
        let id = new_habit(
            &mut connection,
            spec(
                MeasurementKind::Numeric,
                MeasurementDirection::AtLeast,
                1.0,
                "km",
            ),
        );
        let date = today(&connection);

        db::set_paused(&connection, id, true).expect("pause habit");
        assert!(matches!(
            record_measurement_for_date(&connection, id, date, 1.0, ""),
            Err(MeasurementError::Paused)
        ));
        db::set_paused(&connection, id, false).expect("resume habit");

        let yesterday_text: String = connection
            .query_row("SELECT date('now', 'localtime', '-1 day')", [], |row| {
                row.get(0)
            })
            .expect("read yesterday");
        let yesterday = parse_date(&yesterday_text);
        assert!(matches!(
            record_measurement_for_date(&connection, id, yesterday, 1.0, ""),
            Err(MeasurementError::Unscheduled)
        ));
    }

    #[test]
    fn binary_habits_cannot_accept_measurement_values() {
        let connection = memory_connection();
        db::create_habit(&connection, "Binary habit").expect("create binary habit");
        let id: i64 = connection
            .query_row(
                "SELECT id FROM habits WHERE name = 'Binary habit'",
                [],
                |row| row.get(0),
            )
            .expect("read id");

        assert!(matches!(
            measurement_config(&connection, id),
            Err(MeasurementError::NotMeasurable)
        ));
    }

    #[test]
    fn invalid_actual_values_and_oversized_notes_are_rejected() {
        let mut connection = memory_connection();
        let id = new_habit(
            &mut connection,
            spec(
                MeasurementKind::Numeric,
                MeasurementDirection::AtLeast,
                1.0,
                "km",
            ),
        );
        let date = today(&connection);

        for actual in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                record_measurement_for_date(&connection, id, date, actual, ""),
                Err(MeasurementError::InvalidActual)
            ));
        }
        assert!(matches!(
            record_measurement_for_date(&connection, id, date, 1.0, &"x".repeat(501)),
            Err(MeasurementError::InvalidNote)
        ));
    }

    #[test]
    fn undo_removes_only_a_measured_entry_and_preserves_skip_records() {
        let mut connection = memory_connection();
        let id = new_habit(
            &mut connection,
            spec(
                MeasurementKind::Numeric,
                MeasurementDirection::AtLeast,
                1.0,
                "km",
            ),
        );
        let date = today(&connection);

        record_measurement_for_date(&connection, id, date, 0.5, "").expect("record partial value");
        assert!(undo_measurement_for_date(&connection, id, date).expect("undo partial"));

        connection
            .execute(
                "INSERT INTO habit_entries(habit_id, local_date, status, value)
                 VALUES (?1, ?2, 'skipped', NULL)",
                params![id, date.to_string()],
            )
            .expect("add intentional skip");
        assert!(!undo_measurement_for_date(&connection, id, date).expect("skip is not an undo"));
        let status: String = connection
            .query_row(
                "SELECT status FROM habit_entries WHERE habit_id = ?1 AND local_date = ?2",
                params![id, date.to_string()],
                |row| row.get(0),
            )
            .expect("read skip status");
        assert_eq!(status, "skipped");
    }

    #[test]
    fn failed_creation_rolls_back_the_inserted_habit() {
        let mut connection = memory_connection();
        let result = create_measurable_habit(
            &mut connection,
            "   ",
            Cadence::Daily,
            spec(
                MeasurementKind::Numeric,
                MeasurementDirection::AtLeast,
                5.0,
                "pages",
            ),
        );
        assert!(matches!(result, Err(MeasurementError::Database(_))));
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM habits", [], |row| row.get(0))
            .expect("count habits");
        assert_eq!(count, 0);
    }
}
