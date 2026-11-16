use crate::domain::{Cadence, CivilDate, HabitSchedule};
use rusqlite::{Connection, OptionalExtension, Result, params};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

const INITIAL_MIGRATION: &str = include_str!("../migrations/0001_initial.sql");
const PAUSE_MIGRATION: &str = include_str!("../migrations/0002_pause_periods.sql");
const SCHEDULE_REVISION_MIGRATION: &str = include_str!("../migrations/0003_schedule_revisions.sql");
const SCHEMA_VERSION: i64 = 3;

#[derive(Debug, Clone)]
pub struct TodayHabit {
    pub id: i64,
    pub name: String,
    pub completed: bool,
    pub paused: bool,
    /// False means the habit is intentionally not due on the requested local date.
    pub scheduled: bool,
}

pub fn open() -> Result<Connection> {
    let data_dir = data_directory()
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    fs::create_dir_all(&data_dir)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    secure_data_directory(&data_dir)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;

    let path = data_dir.join("habit-tracker.sqlite3");
    validate_database_path(&path)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    let mut connection = Connection::open(&path)?;
    secure_database_file(&path)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")?;
    migrate(&mut connection)?;
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

/// Restrict the app-owned data directory to the current account on Unix.
/// The directory permission also protects SQLite WAL/SHM sidecar files.
fn secure_data_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "refusing a symbolic-link app data directory",
        ));
    }
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "app data path is not a directory",
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }

    Ok(())
}

/// Refuse to let SQLite follow a symlink or open a non-regular path.
fn validate_database_path(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "refusing a symbolic-link database path",
        )),
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "database path is not a regular file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Restrict an existing SQLite database to owner read/write access on Unix.
fn secure_database_file(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "refusing a symbolic-link database path",
        ));
    }
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "database path is not a regular file",
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
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
        current_version = 2;
    }

    if current_version < 3 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(SCHEDULE_REVISION_MIGRATION)?;
        transaction.execute("INSERT INTO schema_migrations(version) VALUES (3)", [])?;
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

    connection.execute(
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
    Ok(())
}


/// Add or replace a future-effective schedule revision without rewriting past rules.
///
/// A revision takes effect on effective_from (inclusive). Existing history is closed
/// at that date (exclusive). Same-day and retrospective changes are deliberately rejected.
pub fn change_habit_schedule(
    connection: &Connection,
    habit_id: i64,
    cadence: Cadence,
    effective_from: CivilDate,
) -> Result<()> {
    let today = local_today(connection)?;
    if effective_from <= today {
        return Err(rusqlite::Error::InvalidParameterName(
            "schedule changes must take effect on a future local date".to_string(),
        ));
    }
    HabitSchedule::new(cadence, effective_from, None)
        .map_err(|_| invalid_schedule_field("new schedule"))?;

    let (schedule_type, weekday_mask, interval_days, target_per_period) =
        cadence_to_columns(cadence);
    let transaction = connection.unchecked_transaction()?;

    let active: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM habits WHERE id = ?1 AND archived_at IS NULL)",
        [habit_id],
        |row| row.get(0),
    )?;
    if !active {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }

    let current: Option<(i64, String, String)> = transaction
        .query_row(
            "SELECT id, effective_from_date, time_zone_id
             FROM habit_schedule_revisions
             WHERE habit_id = ?1 AND effective_until_date IS NULL
             ORDER BY effective_from_date DESC
             LIMIT 1",
            [habit_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;

    let (current_id, current_from_text, time_zone_id) =
        current.ok_or_else(|| invalid_schedule_field("missing open schedule revision"))?;
    let current_from = parse_civil_date(&current_from_text)
        .ok_or_else(|| invalid_schedule_field("effective_from_date"))?;

    if effective_from < current_from {
        return Err(rusqlite::Error::InvalidParameterName(
            "schedule revisions must be appended in effective-date order".to_string(),
        ));
    }

    if effective_from == current_from {
        // The open revision may be edited only while it is still in the future. The
        // database trigger independently enforces this condition.
        transaction.execute(
            "UPDATE habit_schedule_revisions
             SET schedule_type = ?1, weekday_mask = ?2, interval_days = ?3,
                 target_per_period = ?4, period_unit = 'week', schedule_end_date = NULL
             WHERE id = ?5 AND effective_until_date IS NULL",
            params![
                schedule_type,
                weekday_mask,
                interval_days,
                target_per_period,
                current_id
            ],
        )?;
    } else {
        transaction.execute(
            "UPDATE habit_schedule_revisions
             SET effective_until_date = ?1
             WHERE id = ?2 AND effective_until_date IS NULL",
            params![effective_from.to_string(), current_id],
        )?;
        transaction.execute(
            "INSERT INTO habit_schedule_revisions (
                habit_id, effective_from_date, schedule_type, weekday_mask,
                interval_days, target_per_period, period_unit, schedule_end_date,
                time_zone_id
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'week', NULL, ?7)",
            params![
                habit_id,
                effective_from.to_string(),
                schedule_type,
                weekday_mask,
                interval_days,
                target_per_period,
                time_zone_id
            ],
        )?;
    }

    transaction.commit()
}

/// Return the schedule that was in force on a civil date, including its anchor date.
pub fn habit_schedule_for_date(
    connection: &Connection,
    habit_id: i64,
    date: CivilDate,
) -> Result<Option<HabitSchedule>> {
    let revision: Option<(String, Option<i64>, Option<i64>, Option<i64>, String, Option<String>)> =
        connection
            .query_row(
                "SELECT schedule_type, weekday_mask, interval_days, target_per_period,
                        effective_from_date, schedule_end_date
                 FROM habit_schedule_revisions
                 WHERE habit_id = ?1
                   AND effective_from_date <= ?2
                   AND (effective_until_date IS NULL OR ?2 < effective_until_date)
                 ORDER BY effective_from_date DESC
                 LIMIT 1",
                params![habit_id, date.to_string()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .optional()?;

    let Some((kind, weekday_mask, interval_days, target, start_text, end_text)) = revision else {
        return Ok(None);
    };
    let cadence = cadence_from_values(&kind, weekday_mask, interval_days, target)?;
    let start = parse_civil_date(&start_text)
        .ok_or_else(|| invalid_schedule_field("effective_from_date"))?;
    let end = end_text
        .map(|value| {
            parse_civil_date(&value).ok_or_else(|| invalid_schedule_field("schedule_end_date"))
        })
        .transpose()?;

    HabitSchedule::new(cadence, start, end)
        .map(Some)
        .map_err(|_| invalid_schedule_field("schedule bounds"))
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
        })
    })?;
    rows.collect()
}

/// Read the machine-local date once and use it consistently for every row in Today.
pub fn today_habits(connection: &Connection) -> Result<Vec<TodayHabit>> {
    habits_for_date(connection, local_today(connection)?)
}

/// Deterministic schedule evaluation for a supplied civil date. This is reusable by
/// calendar/history views and avoids coupling schedule tests to the wall clock.
pub fn habits_for_date(connection: &Connection, date: CivilDate) -> Result<Vec<TodayHabit>> {
    let mut statement = connection.prepare(
        "SELECT h.id, h.name, r.schedule_type, r.weekday_mask, r.interval_days,
                r.target_per_period, r.effective_from_date, r.schedule_end_date,
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
         JOIN habit_schedule_revisions r
           ON r.habit_id = h.id
          AND r.effective_from_date <= ?1
          AND (r.effective_until_date IS NULL OR ?1 < r.effective_until_date)
         WHERE h.archived_at IS NULL
         ORDER BY h.sort_order, h.id",
    )?;

    let rows = statement.query_map([date.to_string()], |row| {
        let cadence = cadence_from_row(row)?;
        let start_date = civil_date_from_row(row, 6, "start_date")?;
        let end_date = row
            .get::<_, Option<String>>(7)?
            .map(|value| parse_civil_date(&value).ok_or_else(|| invalid_schedule_field("end_date")))
            .transpose()?;
        let schedule = HabitSchedule::new(cadence, start_date, end_date)
            .map_err(|_| invalid_schedule_field("schedule bounds"))?;

        Ok(TodayHabit {
            id: row.get(0)?,
            name: row.get(1)?,
            completed: row.get::<_, i64>(8)? != 0,
            paused: row.get::<_, i64>(9)? != 0,
            scheduled: schedule.period_for(date).is_some(),
        })
    })?;
    rows.collect()
}

fn local_today(connection: &Connection) -> Result<CivilDate> {
    let value: String =
        connection.query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))?;
    parse_civil_date(&value).ok_or_else(|| invalid_schedule_field("current local date"))
}

fn cadence_from_row(row: &rusqlite::Row<'_>) -> Result<Cadence> {
    let schedule_type: String = row.get(2)?;
    cadence_from_values(&schedule_type, row.get(3)?, row.get(4)?, row.get(5)?)
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

fn cadence_to_columns(cadence: Cadence) -> (&'static str, Option<i64>, Option<i64>, Option<i64>) {
    match cadence {
        Cadence::Daily => ("daily", None, None, None),
        Cadence::Weekdays { weekday_mask } => (
            "weekdays",
            Some(i64::from(weekday_mask)),
            None,
            None,
        ),
        Cadence::EveryNDays { interval } => (
            "every_n_days",
            None,
            Some(i64::from(interval)),
            None,
        ),
        Cadence::TimesPerWeek { target } => (
            "times_per_week",
            None,
            None,
            Some(i64::from(target)),
        ),
    }
}

fn civil_date_from_row(row: &rusqlite::Row<'_>, index: usize, field: &str) -> Result<CivilDate> {
    let value: String = row.get(index)?;
    parse_civil_date(&value).ok_or_else(|| invalid_schedule_field(field))
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

/// Pause is effective from the current local date. A resume date is exclusive:
/// resuming today makes the habit active again today, without deleting pause history.
pub fn set_paused(connection: &Connection, habit_id: i64, paused: bool) -> Result<()> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM habits WHERE id = ?1 AND archived_at IS NULL)",
        [habit_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }

    if paused {
        connection.execute(
            "INSERT INTO habit_pause_periods(habit_id, starts_on)
             SELECT ?1, date('now', 'localtime')
             WHERE NOT EXISTS (
                 SELECT 1 FROM habit_pause_periods
                 WHERE habit_id = ?1 AND ends_on IS NULL
             )",
            [habit_id],
        )?;
    } else {
        connection.execute(
            "UPDATE habit_pause_periods
             SET ends_on = date('now', 'localtime')
             WHERE habit_id = ?1 AND ends_on IS NULL",
            [habit_id],
        )?;
    }
    Ok(())
}

pub fn set_today_completed(connection: &Connection, habit_id: i64, completed: bool) -> Result<()> {
    set_completed_for_date(connection, habit_id, completed, local_today(connection)?)
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
    fn migrations_reach_schema_version_three() {
        let connection = memory_connection();
        let version: i64 = connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("read schema version");
        assert_eq!(version, 3);
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

    #[test]
    fn schedule_changes_are_future_dated_and_preserve_past_rules() {
        let connection = memory_connection();
        let habit_id = new_habit(&connection, "Read");

        let today = local_today(&connection).expect("read current local date");
        let future_text: String = connection
            .query_row("SELECT date('now', 'localtime', '+1 day')", [], |row| row.get(0))
            .expect("read next local date");
        let future = parse_civil_date(&future_text).expect("parse next local date");
        let next_weekday = 1_u8 << future.weekday_monday_zero();

        change_habit_schedule(
            &connection,
            habit_id,
            Cadence::Weekdays { weekday_mask: next_weekday },
            future,
        ).expect("schedule future weekday rule");

        let old_schedule = habit_schedule_for_date(&connection, habit_id, today)
            .expect("read old schedule")
            .expect("old schedule exists");
        assert_eq!(old_schedule.cadence(), Cadence::Daily);

        let future_schedule = habit_schedule_for_date(&connection, habit_id, future)
            .expect("read future schedule")
            .expect("future schedule exists");
        assert_eq!(future_schedule.cadence(), Cadence::Weekdays { weekday_mask: next_weekday });

        change_habit_schedule(
            &connection,
            habit_id,
            Cadence::EveryNDays { interval: 2 },
            future,
        ).expect("replace still-future revision");
        let replaced = habit_schedule_for_date(&connection, habit_id, future)
            .expect("read replaced schedule")
            .expect("replaced schedule exists");
        assert_eq!(replaced.cadence(), Cadence::EveryNDays { interval: 2 });

        assert!(change_habit_schedule(&connection, habit_id, Cadence::Daily, today).is_err());

        let revision_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM habit_schedule_revisions WHERE habit_id = ?1",
                [habit_id],
                |row| row.get(0),
            )
            .expect("count revisions");
        assert_eq!(revision_count, 2);
    }

    #[test]
    fn historical_schedule_revisions_cannot_be_changed_or_deleted() {
        let connection = memory_connection();
        let habit_id = new_habit(&connection, "Write");
        let result = connection.execute(
            "UPDATE habit_schedule_revisions SET schedule_type = 'weekdays', weekday_mask = 1
             WHERE habit_id = ?1",
            [habit_id],
        );
        assert!(result.is_err());
        let result = connection.execute(
            "DELETE FROM habit_schedule_revisions WHERE habit_id = ?1",
            [habit_id],
        );
        assert!(result.is_err());
    }

    #[test]
    fn persisted_weekday_schedule_only_marks_selected_days() {
        let connection = memory_connection();
        connection.execute(
            "INSERT INTO habits(name, schedule_type, weekday_mask, start_date)
             VALUES ('Walk', 'weekdays', 1, '2026-11-01')",
            [],
        ).expect("insert Monday-only schedule");

        assert!(habits_for_date(&connection, date(2026, 11, 16)).unwrap()[0].scheduled);
        assert!(!habits_for_date(&connection, date(2026, 11, 15)).unwrap()[0].scheduled);
    }

    #[test]
    fn every_n_days_uses_start_anchor_and_date_bounds() {
        let connection = memory_connection();
        connection.execute(
            "INSERT INTO habits(name, schedule_type, interval_days, start_date, end_date)
             VALUES ('Stretch', 'every_n_days', 2, '2026-11-01', '2026-11-10')",
            [],
        ).expect("insert every-two-days schedule");

        assert!(habits_for_date(&connection, date(2026, 11, 3)).unwrap()[0].scheduled);
        assert!(!habits_for_date(&connection, date(2026, 11, 4)).unwrap()[0].scheduled);
        assert!(!habits_for_date(&connection, date(2026, 10, 31)).unwrap()[0].scheduled);
        assert!(!habits_for_date(&connection, date(2026, 11, 11)).unwrap()[0].scheduled);
    }

    #[test]
    fn weekly_frequency_uses_a_monday_anchored_evaluation_period() {
        let connection = memory_connection();
        connection.execute(
            "INSERT INTO habits(name, schedule_type, target_per_period, period_unit, start_date)
             VALUES ('Practice piano', 'times_per_week', 3, 'week', '2026-11-01')",
            [],
        ).expect("insert times-per-week schedule");

        assert!(habits_for_date(&connection, date(2026, 11, 9)).unwrap()[0].scheduled);
        assert!(habits_for_date(&connection, date(2026, 11, 14)).unwrap()[0].scheduled);
    }

    #[test]
    fn unscheduled_checkins_are_rejected_and_due_checkins_are_saved() {
        let connection = memory_connection();
        connection.execute(
            "INSERT INTO habits(name, schedule_type, weekday_mask, start_date)
             VALUES ('Read', 'weekdays', 1, '2026-11-01')",
            [],
        ).expect("insert Monday-only schedule");
        let habit_id = connection.last_insert_rowid();

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

#[cfg(all(test, unix))]
mod private_storage_tests {
    use super::*;
    use std::{
        os::unix::fs::{PermissionsExt, symlink},
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_PATH: AtomicU64 = AtomicU64::new(0);

    fn temporary_path(label: &str) -> PathBuf {
        let sequence = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "madhab-private-storage-{label}-{}-{sequence}",
            std::process::id()
        ))
    }

    #[test]
    fn app_data_directory_is_restricted_to_owner() {
        let path = temporary_path("directory");
        fs::create_dir(&path).expect("create test directory");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
            .expect("make test directory permissive");

        secure_data_directory(&path).expect("secure app data directory");

        let mode = fs::metadata(&path)
            .expect("read directory metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);

        fs::remove_dir_all(path).expect("remove test directory");
    }

    #[test]
    fn database_file_is_restricted_to_owner() {
        let directory = temporary_path("database");
        fs::create_dir(&directory).expect("create test directory");
        let path = directory.join("habit-tracker.sqlite3");
        fs::write(&path, b"test database").expect("create test database");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
            .expect("make test database permissive");

        secure_database_file(&path).expect("secure database file");

        let mode = fs::metadata(&path)
            .expect("read database metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);

        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn database_path_rejects_symbolic_links() {
        let directory = temporary_path("symlink");
        fs::create_dir(&directory).expect("create test directory");
        let target = directory.join("target.sqlite3");
        let link = directory.join("linked.sqlite3");
        fs::write(&target, b"target").expect("create target file");
        symlink(&target, &link).expect("create database symlink");

        assert_eq!(
            validate_database_path(&link).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            secure_database_file(&link).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );

        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn app_data_directory_rejects_symbolic_links() {
        let directory = temporary_path("directory-symlink");
        fs::create_dir(&directory).expect("create target directory");
        let link = temporary_path("directory-link");
        symlink(&directory, &link).expect("create directory symlink");

        assert_eq!(
            secure_data_directory(&link).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );

        fs::remove_file(link).expect("remove directory symlink");
        fs::remove_dir_all(directory).expect("remove target directory");
    }

    #[test]
    fn database_path_rejects_directories() {
        let directory = temporary_path("not-a-file");
        fs::create_dir(&directory).expect("create directory");

        assert_eq!(
            validate_database_path(&directory).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );

        fs::remove_dir_all(directory).expect("remove test directory");
    }
}
