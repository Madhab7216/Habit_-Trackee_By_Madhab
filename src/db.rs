use crate::domain::{Cadence, CivilDate, HabitSchedule};
use crate::schedule_history::{Recurrence, TimeZoneStatus};
use crate::timezone::IanaTimeZone;
use jiff::Timestamp;
use rusqlite::{Connection, Result, params};
use std::{env, fs, path::PathBuf};

const INITIAL_MIGRATION: &str = include_str!("../migrations/0001_initial.sql");
const PAUSE_MIGRATION: &str = include_str!("../migrations/0002_pause_periods.sql");
const SCHEMA_VERSION: i64 = 2;

#[derive(Debug, Clone)]
pub struct TodayHabit {
    pub id: i64,
    pub name: String,
    pub completed: bool,
    pub paused: bool,
    /// False means the habit is intentionally not due on this habit's local date.
    pub scheduled: bool,
    /// The civil date used for this habit, which may differ between time zones.
    pub local_date: Option<CivilDate>,
    /// The canonical time zone used to resolve local_date. Legacy records use the
    /// current system IANA zone until the user saves a verified revision.
    pub time_zone_id: Option<String>,
}

pub fn open() -> Result<Connection> {
    let data_dir = data_directory()
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    fs::create_dir_all(&data_dir)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;

    let path = data_dir.join("habit-tracker.sqlite3");
    let mut connection = Connection::open(path)?;
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")?;
    migrate(&mut connection)?;
    crate::schedule_history::initialize(&mut connection)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    Ok(connection)
}

fn resolve_data_directory(
    xdg_data_home: Option<PathBuf>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(path) = xdg_data_home.filter(|path| path.is_absolute()) {
        return Some(path.join("madhab-habit-tracker"));
    }

    home.filter(|path| path.is_absolute()).map(|path| {
        path.join(".local")
            .join("share")
            .join("madhab-habit-tracker")
    })
}

fn data_directory() -> std::io::Result<PathBuf> {
    let xdg_data_home = env::var_os("XDG_DATA_HOME").map(PathBuf::from);
    let home = env::var_os("HOME").map(PathBuf::from);

    resolve_data_directory(xdg_data_home, home).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Cannot resolve an absolute XDG_DATA_HOME or HOME; refusing to write app data to the current directory.",
        )
    })
}

fn migrate(connection: &mut Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );",
    )?;

    let mut current_version: i64 = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;

    if current_version > SCHEMA_VERSION {
        return Err(rusqlite::Error::InvalidQuery);
    }

    if current_version < 1 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(INITIAL_MIGRATION)?;
        transaction.execute("INSERT INTO schema_migrations(version) VALUES (1)", [])?;
        transaction.commit()?;
        current_version = 1;
    }

    if current_version < 2 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(PAUSE_MIGRATION)?;
        transaction.execute("INSERT INTO schema_migrations(version) VALUES (2)", [])?;
        transaction.commit()?;
    }

    Ok(())
}

fn validate_habit_name(name: &str) -> Result<&str> {
    let normalized = name.trim();
    if normalized.is_empty() || normalized.chars().count() > 120 {
        return Err(rusqlite::Error::InvalidParameterName(
            "habit name must contain between 1 and 120 characters".to_string(),
        ));
    }
    Ok(normalized)
}

pub fn create_habit(connection: &Connection, name: &str) -> Result<()> {
    create_habit_with_cadence(connection, name, Cadence::Daily)
}

/// Create a binary habit using a persisted and validated recurrence rule.
/// A weekly-frequency habit is checked once on each completed session day;
/// the weekly quota is stored independently in target_per_period.
pub fn create_habit_with_cadence(
    connection: &Connection,
    name: &str,
    cadence: Cadence,
) -> Result<()> {
    let normalized = validate_habit_name(name)?;
    let start_date = local_today(connection)?;
    HabitSchedule::new(cadence, start_date, None)
        .map_err(|_| invalid_schedule_field("new schedule"))?;

    let (schedule_type, weekday_mask, interval_days, target_per_period, tracking_type) =
        match cadence {
            Cadence::Daily => ("daily", None, None, None, "binary"),
            Cadence::Weekdays { weekday_mask } => (
                "weekdays",
                Some(i64::from(weekday_mask)),
                None,
                None,
                "binary",
            ),
            Cadence::EveryNDays { interval } => (
                "every_n_days",
                None,
                Some(i64::from(interval)),
                None,
                "binary",
            ),
            Cadence::TimesPerWeek { target } => (
                "times_per_week",
                None,
                None,
                Some(i64::from(target)),
                "frequency",
            ),
        };

    let transaction = connection.unchecked_transaction()?;
    transaction.execute(
        "INSERT INTO habits
            (name, tracking_type, schedule_type, weekday_mask, interval_days,
             target_per_period, period_unit, start_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'week', ?7)",
        params![
            normalized,
            tracking_type,
            schedule_type,
            weekday_mask,
            interval_days,
            target_per_period,
            start_date.to_string()
        ],
    )?;
    let habit_id = transaction.last_insert_rowid();
    crate::schedule_history::ensure_habit_baseline(&transaction, habit_id)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    transaction.commit()?;
    Ok(())
}

/// Rename an active habit without changing its history or schedule.
pub fn rename_habit(connection: &Connection, habit_id: i64, name: &str) -> Result<()> {
    let normalized = validate_habit_name(name)?;
    let changed = connection.execute(
        "UPDATE habits SET name = ?1, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?2 AND archived_at IS NULL",
        params![normalized, habit_id],
    )?;
    if changed == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

/// Archive is reversible and preserves every check-in, schedule and pause record.
pub fn archive_habit(connection: &Connection, habit_id: i64) -> Result<()> {
    let changed = connection.execute(
        "UPDATE habits
         SET archived_at = COALESCE(archived_at, CURRENT_TIMESTAMP),
             updated_at = CURRENT_TIMESTAMP
         WHERE id = ?1",
        [habit_id],
    )?;
    if changed == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

pub fn restore_habit(connection: &Connection, habit_id: i64) -> Result<()> {
    let changed = connection.execute(
        "UPDATE habits SET archived_at = NULL, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?1",
        [habit_id],
    )?;
    if changed == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

pub fn archived_habits(connection: &Connection) -> Result<Vec<TodayHabit>> {
    let mut statement = connection.prepare(
        "SELECT id, name, 0, 0 FROM habits
         WHERE archived_at IS NOT NULL
         ORDER BY archived_at DESC, id DESC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(TodayHabit {
            id: row.get(0)?,
            name: row.get(1)?,
            completed: row.get::<_, i64>(2)? != 0,
            paused: row.get::<_, i64>(3)? != 0,
            scheduled: false,
            local_date: None,
            time_zone_id: None,
        })
    })?;
    rows.collect()
}

/// Resolve every habit's local date from the IANA zone on its currently effective
/// schedule revision. A legacy-unverified revision deliberately falls back to the
/// current system IANA zone; historical dates are never rewritten.
pub fn today_habits(connection: &Connection) -> Result<Vec<TodayHabit>> {
    today_habits_at(connection, Timestamp::now())
}

/// Deterministic Today view for a supplied instant. Habits in different zones may
/// legitimately display different civil dates at the same absolute instant.
pub fn today_habits_at(connection: &Connection, instant: Timestamp) -> Result<Vec<TodayHabit>> {
    let mut statement = connection.prepare(
        "SELECT id, name FROM habits
         WHERE archived_at IS NULL
         ORDER BY sort_order, id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    let active_habits = rows.collect::<rusqlite::Result<Vec<_>>>()?;

    active_habits
        .into_iter()
        .map(|(habit_id, name)| {
            let today = resolve_habit_today(connection, habit_id, instant)?;
            let date_text = today.local_date.to_string();
            let (completed, paused): (bool, bool) = connection.query_row(
                "SELECT
                    EXISTS (
                        SELECT 1 FROM habit_entries
                        WHERE habit_id = ?1 AND local_date = ?2 AND status = 'completed'
                    ),
                    EXISTS (
                        SELECT 1 FROM habit_pause_periods
                        WHERE habit_id = ?1
                          AND starts_on <= ?2
                          AND (ends_on IS NULL OR ?2 < ends_on)
                    )",
                params![habit_id, date_text],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;

            Ok(TodayHabit {
                id: habit_id,
                name,
                completed,
                paused,
                scheduled: today.scheduled,
                local_date: Some(today.local_date),
                time_zone_id: Some(today.time_zone_id),
            })
        })
        .collect()
}

/// Deterministic schedule evaluation for a supplied civil date. This is reusable by
/// calendar/history views and avoids coupling schedule tests to the wall clock.
pub fn habits_for_date(connection: &Connection, date: CivilDate) -> Result<Vec<TodayHabit>> {
    let mut statement = connection.prepare(
        "SELECT h.id, h.name, r.schedule_type, r.weekday_mask, r.interval_days,
                r.target_per_period, r.effective_from, r.end_date,
                EXISTS (
                    SELECT 1 FROM habit_entries e
                    WHERE e.habit_id = h.id
                      AND e.local_date = ?1
                      AND e.status = 'completed'
                ) AS completed,
                EXISTS (
                    SELECT 1 FROM habit_pause_periods p
                    WHERE p.habit_id = h.id
                      AND p.starts_on <= ?1
                      AND (p.ends_on IS NULL OR ?1 < p.ends_on)
                ) AS paused
         FROM habits h
         LEFT JOIN habit_schedule_revisions r
           ON r.id = (
               SELECT candidate.id
               FROM habit_schedule_revisions candidate
               WHERE candidate.habit_id = h.id
                 AND candidate.effective_from <= ?1
                 AND NOT EXISTS (
                     SELECT 1
                     FROM habit_schedule_revisions later
                     WHERE later.habit_id = candidate.habit_id
                       AND later.effective_from > candidate.effective_from
                       AND later.effective_from <= ?1
                 )
                 AND (candidate.end_date IS NULL OR candidate.end_date >= ?1)
               ORDER BY candidate.effective_from DESC, candidate.revision_no DESC
               LIMIT 1
           )
         WHERE h.archived_at IS NULL
         ORDER BY h.sort_order, h.id",
    )?;

    let rows = statement.query_map([date.to_string()], |row| {
        let schedule_type: Option<String> = row.get(2)?;
        let scheduled = if let Some(schedule_type) = schedule_type {
            let cadence =
                cadence_from_values(&schedule_type, row.get(3)?, row.get(4)?, row.get(5)?)?;
            let start_date_text: Option<String> = row.get(6)?;
            let start_date = start_date_text
                .as_deref()
                .and_then(parse_civil_date)
                .ok_or_else(|| invalid_schedule_field("effective_from"))?;
            let end_date = row
                .get::<_, Option<String>>(7)?
                .map(|value| {
                    parse_civil_date(&value).ok_or_else(|| invalid_schedule_field("end_date"))
                })
                .transpose()?;
            let schedule = HabitSchedule::new(cadence, start_date, end_date)
                .map_err(|_| invalid_schedule_field("schedule bounds"))?;
            schedule.period_for(date).is_some()
        } else {
            false
        };

        Ok(TodayHabit {
            id: row.get(0)?,
            name: row.get(1)?,
            completed: row.get::<_, i64>(8)? != 0,
            paused: row.get::<_, i64>(9)? != 0,
            scheduled,
            local_date: Some(date),
            time_zone_id: None,
        })
    })?;
    rows.collect()
}

#[derive(Debug, Clone)]
struct ResolvedHabitToday {
    local_date: CivilDate,
    scheduled: bool,
    time_zone_id: String,
}

fn resolve_habit_today(
    connection: &Connection,
    habit_id: i64,
    instant: Timestamp,
) -> Result<ResolvedHabitToday> {
    let revisions = crate::schedule_history::revisions_for_habit_readonly(connection, habit_id)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    if revisions.is_empty() {
        return Err(invalid_schedule_field("missing schedule history"));
    }

    let mut latest_future: Option<(String, CivilDate)> = None;
    for revision in revisions.iter().rev() {
        let zone = match revision.time_zone_status {
            TimeZoneStatus::ValidatedIana => IanaTimeZone::parse(&revision.time_zone_id)
                .map_err(|error| invalid_schedule_field(&format!("time_zone_id: {error}")))?,
            TimeZoneStatus::LegacyUnverified => IanaTimeZone::system()
                .map_err(|error| invalid_schedule_field(&format!("system time zone: {error}")))?,
        };
        let local_date = parse_civil_date(&zone.local_date_at(instant).to_string())
            .ok_or_else(|| invalid_schedule_field("time-zone local date"))?;
        let effective_from = parse_civil_date(&revision.effective_from.to_string())
            .ok_or_else(|| invalid_schedule_field("effective_from"))?;

        // A future revision is interpreted in its own IANA zone. Until that zone's
        // effective civil date arrives, the previous revision remains in force.
        if local_date < effective_from {
            if latest_future.is_none() {
                latest_future = Some((zone.id().to_owned(), local_date));
            }
            continue;
        }

        // Once a revision has become effective, it supersedes older revisions
        // permanently. If its own end date has passed, do not resurrect an older rule.
        let before_or_on_end = revision.end_date.is_none_or(|end| {
            local_date.to_string() <= end.to_string()
        });
        let scheduled = if before_or_on_end {
            let cadence = cadence_from_revision(&revision.recurrence);
            let end = revision
                .end_date
                .map(|value| {
                    parse_civil_date(&value.to_string())
                        .ok_or_else(|| invalid_schedule_field("end_date"))
                })
                .transpose()?;
            HabitSchedule::new(cadence, effective_from, end)
                .map_err(|_| invalid_schedule_field("schedule bounds"))?
                .period_for(local_date)
                .is_some()
        } else {
            false
        };

        return Ok(ResolvedHabitToday {
            local_date,
            scheduled,
            time_zone_id: zone.id().to_owned(),
        });
    }

    let (time_zone_id, local_date) = latest_future
        .ok_or_else(|| invalid_schedule_field("no effective schedule revision"))?;
    Ok(ResolvedHabitToday {
        local_date,
        scheduled: false,
        time_zone_id,
    })
}

fn cadence_from_revision(recurrence: &Recurrence) -> Cadence {
    match recurrence {
        Recurrence::Daily => Cadence::Daily,
        Recurrence::Weekdays { weekday_mask } => Cadence::Weekdays {
            weekday_mask: *weekday_mask,
        },
        Recurrence::EveryNDays { interval_days } => Cadence::EveryNDays {
            interval: *interval_days,
        },
        Recurrence::TimesPerWeek { target_per_week } => Cadence::TimesPerWeek {
            target: *target_per_week,
        },
    }
}

fn local_today(connection: &Connection) -> Result<CivilDate> {
    let value: String =
        connection.query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))?;
    parse_civil_date(&value).ok_or_else(|| invalid_schedule_field("current local date"))
}

fn cadence_from_values(
    schedule_type: &str,
    raw_weekday_mask: Option<i64>,
    raw_interval_days: Option<i64>,
    raw_target_per_period: Option<i64>,
) -> Result<Cadence> {
    match schedule_type {
        "daily" => Ok(Cadence::Daily),
        "weekdays" => {
            let weekday_mask = raw_weekday_mask
                .and_then(|value| u8::try_from(value).ok())
                .ok_or_else(|| invalid_schedule_field("weekday_mask"))?;
            Ok(Cadence::Weekdays { weekday_mask })
        }
        "every_n_days" => {
            let interval = raw_interval_days
                .and_then(|value| u16::try_from(value).ok())
                .ok_or_else(|| invalid_schedule_field("interval_days"))?;
            Ok(Cadence::EveryNDays { interval })
        }
        "times_per_week" => {
            let target = raw_target_per_period
                .and_then(|value| u8::try_from(value).ok())
                .ok_or_else(|| invalid_schedule_field("target_per_period"))?;
            Ok(Cadence::TimesPerWeek { target })
        }
        _ => Err(invalid_schedule_field("schedule_type")),
    }
}

fn parse_civil_date(value: &str) -> Option<CivilDate> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u8>().ok()?;
    let day = parts.next()?.parse::<u8>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    CivilDate::new(year, month, day)
}

fn invalid_schedule_field(field: &str) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(format!(
        "invalid persisted habit schedule field: {field}"
    ))
}

/// Pause/resume is effective on the habit's own current civil date.
pub fn set_paused(connection: &Connection, habit_id: i64, paused: bool) -> Result<()> {
    set_paused_at(connection, habit_id, paused, Timestamp::now())
}

/// Deterministic pause/resume API. The date is resolved in the currently effective
/// schedule's IANA zone, not the machine's local zone.
pub fn set_paused_at(
    connection: &Connection,
    habit_id: i64,
    paused: bool,
    instant: Timestamp,
) -> Result<()> {
    let today = resolve_habit_today(connection, habit_id, instant)?;
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM habits WHERE id = ?1 AND archived_at IS NULL)",
        [habit_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }

    let date_text = today.local_date.to_string();
    if paused {
        connection.execute(
            "INSERT INTO habit_pause_periods(habit_id, starts_on)
             SELECT ?1, ?2
             WHERE NOT EXISTS (
                 SELECT 1 FROM habit_pause_periods
                 WHERE habit_id = ?1 AND ends_on IS NULL
             )",
            params![habit_id, date_text],
        )?;
    } else {
        connection.execute(
            "UPDATE habit_pause_periods
             SET ends_on = ?2
             WHERE habit_id = ?1 AND ends_on IS NULL",
            params![habit_id, date_text],
        )?;
    }
    Ok(())
}

pub fn set_today_completed(connection: &Connection, habit_id: i64, completed: bool) -> Result<()> {
    set_today_completed_at(connection, habit_id, completed, Timestamp::now())
}

/// Persist a Today check-in using the current civil date in this habit's active
/// IANA time zone. Schedule, pause and entry lookup all use exactly the same date.
pub fn set_today_completed_at(
    connection: &Connection,
    habit_id: i64,
    completed: bool,
    instant: Timestamp,
) -> Result<()> {
    let today = resolve_habit_today(connection, habit_id, instant)?;
    let active: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM habits WHERE id = ?1 AND archived_at IS NULL)",
        [habit_id],
        |row| row.get(0),
    )?;
    if !active {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }

    let date_text = today.local_date.to_string();
    if completed {
        let paused: bool = connection.query_row(
            "SELECT EXISTS (
                SELECT 1 FROM habit_pause_periods
                WHERE habit_id = ?1
                  AND starts_on <= ?2
                  AND (ends_on IS NULL OR ?2 < ends_on)
            )",
            params![habit_id, date_text],
            |row| row.get(0),
        )?;
        if paused {
            return Err(rusqlite::Error::InvalidParameterName(
                "a paused habit cannot be checked in".to_string(),
            ));
        }
        if !today.scheduled {
            return Err(rusqlite::Error::InvalidParameterName(
                "a habit cannot be checked in on an unscheduled local date".to_string(),
            ));
        }
        connection.execute(
            "INSERT INTO habit_entries(habit_id, local_date, status, value)
             VALUES (?1, ?2, 'completed', 1.0)
             ON CONFLICT(habit_id, local_date) DO UPDATE SET
                status = 'completed', value = 1.0, updated_at = CURRENT_TIMESTAMP",
            params![habit_id, date_text],
        )?;
    } else {
        connection.execute(
            "DELETE FROM habit_entries
             WHERE habit_id = ?1 AND local_date = ?2 AND status = 'completed'",
            params![habit_id, date_text],
        )?;
    }
    Ok(())
}

/// Persist a check-in only when the active habit is due and not paused for that date.
/// Undo remains allowed for an active habit even if its schedule was later edited.
fn set_completed_for_date(
    connection: &Connection,
    habit_id: i64,
    completed: bool,
    date: CivilDate,
) -> Result<()> {
    let active: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM habits WHERE id = ?1 AND archived_at IS NULL)",
        [habit_id],
        |row| row.get(0),
    )?;
    if !active {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }

    let date_text = date.to_string();
    if completed {
        let habit = habits_for_date(connection, date)?
            .into_iter()
            .find(|habit| habit.id == habit_id)
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;

        if habit.paused {
            return Err(rusqlite::Error::InvalidParameterName(
                "a paused habit cannot be checked in".to_string(),
            ));
        }
        if !habit.scheduled {
            return Err(rusqlite::Error::InvalidParameterName(
                "a habit cannot be checked in on an unscheduled date".to_string(),
            ));
        }

        connection.execute(
            "INSERT INTO habit_entries(habit_id, local_date, status, value)
             VALUES (?1, ?2, 'completed', 1.0)
             ON CONFLICT(habit_id, local_date) DO UPDATE SET
                status = 'completed', value = 1.0, updated_at = CURRENT_TIMESTAMP",
            params![habit_id, date_text],
        )?;
    } else {
        connection.execute(
            "DELETE FROM habit_entries
             WHERE habit_id = ?1 AND local_date = ?2 AND status = 'completed'",
            params![habit_id, date_text],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_connection() -> Connection {
        let mut connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .expect("enable foreign keys");
        migrate(&mut connection).expect("apply migrations");
        crate::schedule_history::initialize(&mut connection).expect("initialize schedule history");
        connection
    }

    #[test]
    fn absolute_xdg_data_home_takes_precedence() {
        let resolved = resolve_data_directory(
            Some(PathBuf::from("/tmp/test-xdg")),
            Some(PathBuf::from("/home/madhab")),
        );
        assert_eq!(
            resolved,
            Some(PathBuf::from("/tmp/test-xdg/madhab-habit-tracker"))
        );
    }

    #[test]
    fn relative_xdg_data_home_falls_back_to_absolute_home() {
        let resolved = resolve_data_directory(
            Some(PathBuf::from("relative/data")),
            Some(PathBuf::from("/home/madhab")),
        );
        assert_eq!(
            resolved,
            Some(PathBuf::from(
                "/home/madhab/.local/share/madhab-habit-tracker"
            ))
        );
    }

    #[test]
    fn refuses_to_use_current_directory_when_home_is_unavailable() {
        assert_eq!(
            resolve_data_directory(Some(PathBuf::from("relative/data")), None),
            None
        );
        assert_eq!(resolve_data_directory(None, None), None);
    }

    #[test]
    fn create_habit_persists_selected_weekday_schedule() {
        let connection = memory_connection();
        create_habit_with_cadence(
            &connection,
            "Read",
            Cadence::Weekdays {
                weekday_mask: 0b001_1111,
            },
        )
        .expect("create weekday habit");

        let stored: (String, i64) = connection
            .query_row(
                "SELECT schedule_type, weekday_mask FROM habits WHERE name = 'Read'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read saved schedule");
        assert_eq!(stored, ("weekdays".to_string(), 0b001_1111));
    }

    #[test]
    fn create_habit_creates_a_baseline_schedule_revision_atomically() {
        let connection = memory_connection();
        create_habit_with_cadence(&connection, "Journal", Cadence::Daily).expect("create habit");
        let row: (String, String, String, i64) = connection
            .query_row(
                "SELECT r.schedule_type, r.time_zone_status, r.source, r.revision_no
                 FROM habit_schedule_revisions r
                 JOIN habits h ON h.id = r.habit_id
                 WHERE h.name = 'Journal'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read schedule history baseline");
        assert_eq!(
            row,
            (
                "daily".to_string(),
                "legacy_unverified".to_string(),
                "legacy".to_string(),
                1,
            )
        );
    }

    #[test]
    fn create_habit_persists_weekly_frequency_target() {
        let connection = memory_connection();
        create_habit_with_cadence(&connection, "Workout", Cadence::TimesPerWeek { target: 3 })
            .expect("create weekly frequency habit");

        let stored: (String, String, i64) = connection
            .query_row(
                "SELECT tracking_type, schedule_type, target_per_period
                 FROM habits WHERE name = 'Workout'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read frequency schedule");
        assert_eq!(
            stored,
            ("frequency".to_string(), "times_per_week".to_string(), 3)
        );
    }

    #[test]
    fn create_habit_rejects_invalid_recurrence() {
        let connection = memory_connection();
        assert!(
            create_habit_with_cadence(
                &connection,
                "Invalid weekdays",
                Cadence::Weekdays { weekday_mask: 0 }
            )
            .is_err()
        );
        assert!(
            create_habit_with_cadence(
                &connection,
                "Invalid weekly target",
                Cadence::TimesPerWeek { target: 8 }
            )
            .is_err()
        );
    }

    #[test]
    fn migrations_reach_schema_version_two() {
        let connection = memory_connection();
        let version: i64 = connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("read schema version");
        assert_eq!(version, 2);
    }

    #[test]
    fn pause_and_resume_are_persisted_without_deleting_history() {
        let connection = memory_connection();
        create_habit(&connection, "Read 20 pages").expect("create habit");
        let habit_id: i64 = connection
            .query_row(
                "SELECT id FROM habits WHERE name = 'Read 20 pages'",
                [],
                |row| row.get(0),
            )
            .expect("read habit id");

        set_paused(&connection, habit_id, true).expect("pause habit");
        let paused = today_habits(&connection).expect("load today's habits");
        assert!(paused[0].paused);
        assert!(set_today_completed(&connection, habit_id, true).is_err());

        set_paused(&connection, habit_id, false).expect("resume habit");
        let resumed = today_habits(&connection).expect("reload today's habits");
        assert!(!resumed[0].paused);

        let pause_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM habit_pause_periods WHERE habit_id = ?1",
                [habit_id],
                |row| row.get(0),
            )
            .expect("count pause history");
        assert_eq!(pause_count, 1);
    }

    #[test]
    fn pause_operation_is_idempotent() {
        let connection = memory_connection();
        create_habit(&connection, "Meditate").expect("create habit");
        let habit_id: i64 = connection
            .query_row("SELECT id FROM habits WHERE name = 'Meditate'", [], |row| {
                row.get(0)
            })
            .expect("read habit id");

        set_paused(&connection, habit_id, true).expect("pause once");
        set_paused(&connection, habit_id, true).expect("pause again");

        let pause_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM habit_pause_periods WHERE habit_id = ?1",
                [habit_id],
                |row| row.get(0),
            )
            .expect("count pause history");
        assert_eq!(pause_count, 1);
    }

    #[test]
    fn rejects_pause_for_unknown_habit() {
        let connection = memory_connection();
        assert!(set_paused(&connection, 999, true).is_err());
    }
    #[test]
    fn rename_preserves_existing_habit_history() {
        let connection = memory_connection();
        create_habit(&connection, "Read").expect("create habit");
        let habit_id: i64 = connection
            .query_row("SELECT id FROM habits WHERE name = 'Read'", [], |row| {
                row.get(0)
            })
            .expect("read habit id");
        set_today_completed(&connection, habit_id, true).expect("complete habit");

        rename_habit(&connection, habit_id, "  Read twenty pages  ").expect("rename habit");

        let habits = today_habits(&connection).expect("load habits");
        assert_eq!(habits[0].name, "Read twenty pages");
        assert!(habits[0].completed);
    }

    #[test]
    fn archive_and_restore_preserve_checkins() {
        let connection = memory_connection();
        create_habit(&connection, "Walk").expect("create habit");
        let habit_id: i64 = connection
            .query_row("SELECT id FROM habits WHERE name = 'Walk'", [], |row| {
                row.get(0)
            })
            .expect("read habit id");
        set_today_completed(&connection, habit_id, true).expect("complete habit");

        archive_habit(&connection, habit_id).expect("archive habit");
        assert!(
            today_habits(&connection)
                .expect("load active habits")
                .is_empty()
        );
        let archived = archived_habits(&connection).expect("load archived habits");
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].name, "Walk");

        assert!(set_today_completed(&connection, habit_id, false).is_err());
        restore_habit(&connection, habit_id).expect("restore habit");

        let restored = today_habits(&connection).expect("load restored habit");
        assert_eq!(restored.len(), 1);
        assert!(restored[0].completed);
    }

    #[test]
    fn rename_rejects_empty_and_overlong_names() {
        let connection = memory_connection();
        create_habit(&connection, "Journal").expect("create habit");
        let habit_id: i64 = connection
            .query_row("SELECT id FROM habits WHERE name = 'Journal'", [], |row| {
                row.get(0)
            })
            .expect("read habit id");

        assert!(rename_habit(&connection, habit_id, "   ").is_err());
        assert!(rename_habit(&connection, habit_id, &"x".repeat(121)).is_err());
    }

    #[test]
    fn archive_rejects_unknown_habits() {
        let connection = memory_connection();
        assert!(archive_habit(&connection, 999).is_err());
        assert!(restore_habit(&connection, 999).is_err());
    }
    use crate::schedule_history::{NewScheduleRevision, PastEditConfirmation, Recurrence};
    use jiff::civil::date as jiff_date;

    fn date(year: i32, month: u8, day: u8) -> CivilDate {
        CivilDate::new(year, month, day).expect("valid test date")
    }

    fn new_habit(connection: &Connection, name: &str) -> i64 {
        create_habit(connection, name).expect("create habit");
        connection
            .query_row("SELECT id FROM habits WHERE name = ?1", [name], |row| {
                row.get(0)
            })
            .expect("read habit id")
    }

    fn new_habit_with_schedule(
        connection: &Connection,
        name: &str,
        cadence: Cadence,
        start: CivilDate,
        end: Option<CivilDate>,
    ) -> i64 {
        let (
            tracking_type,
            schedule_type,
            weekday_mask,
            interval_days,
            target_per_period,
            period_unit,
        ) = match cadence {
            Cadence::Daily => ("binary", "daily", None, None, None, "day"),
            Cadence::Weekdays { weekday_mask } => (
                "binary",
                "weekdays",
                Some(i64::from(weekday_mask)),
                None,
                None,
                "day",
            ),
            Cadence::EveryNDays { interval } => (
                "binary",
                "every_n_days",
                None,
                Some(i64::from(interval)),
                None,
                "day",
            ),
            Cadence::TimesPerWeek { target } => (
                "frequency",
                "times_per_week",
                None,
                None,
                Some(i64::from(target)),
                "week",
            ),
        };
        connection
            .execute(
                "INSERT INTO habits (
                    name, tracking_type, schedule_type, weekday_mask, interval_days,
                    target_per_period, period_unit, start_date, end_date
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    name,
                    tracking_type,
                    schedule_type,
                    weekday_mask,
                    interval_days,
                    target_per_period,
                    period_unit,
                    start.to_string(),
                    end.map(|value| value.to_string())
                ],
            )
            .expect("insert habit with initial schedule");
        let habit_id = connection.last_insert_rowid();
        crate::schedule_history::ensure_habit_baseline(connection, habit_id)
            .expect("create initial schedule revision");
        habit_id
    }

    fn append_timezone_revision(
        connection: &mut Connection,
        habit_id: i64,
        effective_from: &str,
        time_zone_id: &str,
    ) {
        let effective_from = effective_from
            .parse::<jiff::civil::Date>()
            .expect("valid schedule effective date");
        let revision = NewScheduleRevision {
            effective_from,
            end_date: None,
            recurrence: Recurrence::Daily,
            time_zone_id: time_zone_id.to_owned(),
            change_reason: "Set the habit's IANA time zone".to_owned(),
        };
        crate::schedule_history::append_revision(
            connection,
            habit_id,
            &revision,
            effective_from,
            PastEditConfirmation::NotConfirmed,
        )
        .expect("append timezone revision");
    }

    #[test]
    fn today_uses_each_habits_own_iana_date_for_checkins_and_pause_history() {
        let mut connection = memory_connection();
        let kolkata_id = new_habit_with_schedule(
            &connection,
            "Kolkata habit",
            Cadence::Daily,
            date(2026, 11, 1),
            None,
        );
        let los_angeles_id = new_habit_with_schedule(
            &connection,
            "Los Angeles habit",
            Cadence::Daily,
            date(2026, 11, 1),
            None,
        );
        append_timezone_revision(
            &mut connection,
            kolkata_id,
            "2026-11-10",
            "Asia/Kolkata",
        );
        append_timezone_revision(
            &mut connection,
            los_angeles_id,
            "2026-11-10",
            "America/Los_Angeles",
        );

        let instant: Timestamp = "2026-11-17T18:30:00Z"
            .parse()
            .expect("valid deterministic instant");
        let habits = today_habits_at(&connection, instant).expect("load per-zone Today");
        let kolkata = habits.iter().find(|habit| habit.id == kolkata_id).unwrap();
        let los_angeles = habits
            .iter()
            .find(|habit| habit.id == los_angeles_id)
            .unwrap();

        assert_eq!(kolkata.local_date, Some(date(2026, 11, 18)));
        assert_eq!(kolkata.time_zone_id.as_deref(), Some("Asia/Kolkata"));
        assert_eq!(los_angeles.local_date, Some(date(2026, 11, 17)));
        assert_eq!(
            los_angeles.time_zone_id.as_deref(),
            Some("America/Los_Angeles")
        );

        set_today_completed_at(&connection, kolkata_id, true, instant)
            .expect("save Kolkata check-in");
        set_today_completed_at(&connection, los_angeles_id, true, instant)
            .expect("save Los Angeles check-in");
        let saved_dates: Vec<(i64, String)> = {
            let mut statement = connection
                .prepare("SELECT habit_id, local_date FROM habit_entries ORDER BY habit_id")
                .expect("prepare saved-date query");
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("query saved dates")
                .collect::<rusqlite::Result<Vec<_>>>()
                .expect("collect saved dates")
        };
        assert_eq!(
            saved_dates,
            vec![
                (kolkata_id, "2026-11-18".to_owned()),
                (los_angeles_id, "2026-11-17".to_owned())
            ]
        );

        set_paused_at(&connection, kolkata_id, true, instant).expect("pause in Kolkata date");
        let pause_start: String = connection
            .query_row(
                "SELECT starts_on FROM habit_pause_periods WHERE habit_id = ?1",
                [kolkata_id],
                |row| row.get(0),
            )
            .expect("read pause start");
        assert_eq!(pause_start, "2026-11-18");

        let resume_instant: Timestamp = "2026-11-18T18:30:00Z"
            .parse()
            .expect("valid resume instant");
        set_paused_at(&connection, kolkata_id, false, resume_instant)
            .expect("resume in Kolkata date");
        let pause_end: String = connection
            .query_row(
                "SELECT ends_on FROM habit_pause_periods WHERE habit_id = ?1",
                [kolkata_id],
                |row| row.get(0),
            )
            .expect("read pause end");
        assert_eq!(pause_end, "2026-11-19");
    }

    #[test]
    fn future_timezone_revision_activates_at_midnight_in_its_own_zone() {
        let mut connection = memory_connection();
        let habit_id = new_habit_with_schedule(
            &connection,
            "Zone transition",
            Cadence::Daily,
            date(2026, 11, 1),
            None,
        );
        append_timezone_revision(&mut connection, habit_id, "2026-11-02", "Asia/Kolkata");
        append_timezone_revision(
            &mut connection,
            habit_id,
            "2026-11-18",
            "America/Los_Angeles",
        );

        let before: Timestamp = "2026-11-18T07:59:59Z"
            .parse()
            .expect("valid instant before Los Angeles midnight");
        let after: Timestamp = "2026-11-18T08:00:00Z"
            .parse()
            .expect("valid instant at Los Angeles midnight");

        let before_today = today_habits_at(&connection, before).expect("resolve old revision");
        assert_eq!(before_today[0].time_zone_id.as_deref(), Some("Asia/Kolkata"));
        assert_eq!(before_today[0].local_date, Some(date(2026, 11, 18)));

        let after_today = today_habits_at(&connection, after).expect("resolve new revision");
        assert_eq!(
            after_today[0].time_zone_id.as_deref(),
            Some("America/Los_Angeles")
        );
        assert_eq!(after_today[0].local_date, Some(date(2026, 11, 18)));
    }

    #[test]
    fn legacy_unverified_schedule_uses_system_iana_zone_without_rewriting_history() {
        let connection = memory_connection();
        let habit_id = new_habit_with_schedule(
            &connection,
            "Legacy habit",
            Cadence::Daily,
            date(2026, 11, 1),
            None,
        );
        let instant: Timestamp = "2026-11-17T18:30:00Z"
            .parse()
            .expect("valid deterministic instant");
        let system_zone = IanaTimeZone::system().expect("CI system zone must be IANA");
        let expected_date = parse_civil_date(&system_zone.local_date_at(instant).to_string())
            .expect("system local date");

        let habits = today_habits_at(&connection, instant).expect("resolve legacy zone");
        assert_eq!(habits[0].id, habit_id);
        assert_eq!(habits[0].local_date, Some(expected_date));
        assert_eq!(habits[0].time_zone_id.as_deref(), Some(system_zone.id()));

        let status: String = connection
            .query_row(
                "SELECT time_zone_status FROM habit_schedule_revisions WHERE habit_id = ?1",
                [habit_id],
                |row| row.get(0),
            )
            .expect("read legacy timezone status");
        assert_eq!(status, "legacy_unverified");
    }

    #[test]
    fn persisted_weekday_schedule_only_marks_selected_days() {
        let connection = memory_connection();
        new_habit_with_schedule(
            &connection,
            "Walk",
            Cadence::Weekdays { weekday_mask: 1 },
            date(2026, 11, 1),
            None,
        );

        assert!(habits_for_date(&connection, date(2026, 11, 16)).unwrap()[0].scheduled);
        assert!(!habits_for_date(&connection, date(2026, 11, 15)).unwrap()[0].scheduled);
    }

    #[test]
    fn every_n_days_uses_revision_start_anchor_and_date_bounds() {
        let connection = memory_connection();
        new_habit_with_schedule(
            &connection,
            "Stretch",
            Cadence::EveryNDays { interval: 2 },
            date(2026, 11, 1),
            Some(date(2026, 11, 10)),
        );

        assert!(habits_for_date(&connection, date(2026, 11, 3)).unwrap()[0].scheduled);
        assert!(!habits_for_date(&connection, date(2026, 11, 4)).unwrap()[0].scheduled);
        assert!(!habits_for_date(&connection, date(2026, 10, 31)).unwrap()[0].scheduled);
        assert!(!habits_for_date(&connection, date(2026, 11, 11)).unwrap()[0].scheduled);
    }

    #[test]
    fn weekly_frequency_uses_a_monday_anchored_evaluation_period() {
        let connection = memory_connection();
        new_habit_with_schedule(
            &connection,
            "Practice piano",
            Cadence::TimesPerWeek { target: 3 },
            date(2026, 11, 1),
            None,
        );

        assert!(habits_for_date(&connection, date(2026, 11, 9)).unwrap()[0].scheduled);
        assert!(habits_for_date(&connection, date(2026, 11, 14)).unwrap()[0].scheduled);
    }

    #[test]
    fn checkins_use_the_effective_schedule_revision_and_never_revive_an_expired_rule() {
        let mut connection = memory_connection();
        let habit_id = new_habit(&connection, "Read");
        let revision = NewScheduleRevision {
            effective_from: jiff_date(2026, 11, 20),
            end_date: Some(jiff_date(2026, 11, 22)),
            recurrence: Recurrence::Weekdays {
                weekday_mask: 0b001_1111,
            },
            time_zone_id: "Asia/Kolkata".to_owned(),
            change_reason: "Switch to weekdays".to_owned(),
        };
        crate::schedule_history::append_revision(
            &mut connection,
            habit_id,
            &revision,
            jiff_date(2026, 11, 17),
            PastEditConfirmation::NotConfirmed,
        )
        .expect("append future weekday schedule");

        assert!(habits_for_date(&connection, date(2026, 11, 19)).unwrap()[0].scheduled);
        assert!(habits_for_date(&connection, date(2026, 11, 20)).unwrap()[0].scheduled);
        assert!(!habits_for_date(&connection, date(2026, 11, 21)).unwrap()[0].scheduled);
        assert!(
            !habits_for_date(&connection, date(2026, 11, 23)).unwrap()[0].scheduled,
            "the superseded daily baseline must not reappear after revision 2 expires"
        );

        assert!(
            set_completed_for_date(&connection, habit_id, true, date(2026, 11, 21)).is_err(),
            "check-ins on dates excluded by the active revision must be rejected"
        );
        set_completed_for_date(&connection, habit_id, true, date(2026, 11, 20))
            .expect("save check-in on a scheduled weekday");
    }

    #[test]
    fn unscheduled_checkins_are_rejected_and_due_checkins_are_saved() {
        let connection = memory_connection();
        let habit_id = new_habit_with_schedule(
            &connection,
            "Read",
            Cadence::Weekdays { weekday_mask: 1 },
            date(2026, 11, 1),
            None,
        );

        assert!(set_completed_for_date(&connection, habit_id, true, date(2026, 11, 15)).is_err());
        set_completed_for_date(&connection, habit_id, true, date(2026, 11, 16))
            .expect("save Monday check-in");

        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM habit_entries WHERE habit_id = ?1",
                [habit_id],
                |row| row.get(0),
            )
            .expect("count entries");
        assert_eq!(count, 1);
    }

    #[test]
    fn archived_habits_are_excluded_from_date_queries() {
        let connection = memory_connection();
        let habit_id = new_habit(&connection, "Archive test");
        archive_habit(&connection, habit_id).expect("archive habit");
        assert!(
            habits_for_date(&connection, date(2026, 11, 16))
                .unwrap()
                .is_empty()
        );
    }
}
