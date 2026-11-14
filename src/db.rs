use crate::domain::{Cadence, CivilDate, HabitSchedule};
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
    /// False means the habit is intentionally not due on the requested local date.
    pub scheduled: bool,
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
        "SELECT h.id, h.name, h.schedule_type, h.weekday_mask, h.interval_days,
                h.target_per_period, h.start_date, h.end_date,
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
    match schedule_type.as_str() {
        "daily" => Ok(Cadence::Daily),
        "weekdays" => {
            let raw: Option<i64> = row.get(3)?;
            let weekday_mask = raw
                .and_then(|value| u8::try_from(value).ok())
                .ok_or_else(|| invalid_schedule_field("weekday_mask"))?;
            Ok(Cadence::Weekdays { weekday_mask })
        }
        "every_n_days" => {
            let raw: Option<i64> = row.get(4)?;
            let interval = raw
                .and_then(|value| u16::try_from(value).ok())
                .ok_or_else(|| invalid_schedule_field("interval_days"))?;
            Ok(Cadence::EveryNDays { interval })
        }
        "times_per_week" => {
            let raw: Option<i64> = row.get(5)?;
            let target = raw
                .and_then(|value| u8::try_from(value).ok())
                .ok_or_else(|| invalid_schedule_field("target_per_period"))?;
            Ok(Cadence::TimesPerWeek { target })
        }
        _ => Err(invalid_schedule_field("schedule_type")),
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
    fn persisted_weekday_schedule_only_marks_selected_days() {
        let connection = memory_connection();
        let habit_id = new_habit(&connection, "Walk");
        connection
            .execute(
                "UPDATE habits SET schedule_type = 'weekdays', weekday_mask = 1,
                start_date = '2026-11-01' WHERE id = ?1",
                [habit_id],
            )
            .expect("set Monday-only schedule");

        assert!(habits_for_date(&connection, date(2026, 11, 16)).unwrap()[0].scheduled);
        assert!(!habits_for_date(&connection, date(2026, 11, 15)).unwrap()[0].scheduled);
    }

    #[test]
    fn every_n_days_uses_start_anchor_and_date_bounds() {
        let connection = memory_connection();
        let habit_id = new_habit(&connection, "Stretch");
        connection
            .execute(
                "UPDATE habits SET schedule_type = 'every_n_days', interval_days = 2,
                start_date = '2026-11-01', end_date = '2026-11-10' WHERE id = ?1",
                [habit_id],
            )
            .expect("set every-two-days schedule");

        assert!(habits_for_date(&connection, date(2026, 11, 3)).unwrap()[0].scheduled);
        assert!(!habits_for_date(&connection, date(2026, 11, 4)).unwrap()[0].scheduled);
        assert!(!habits_for_date(&connection, date(2026, 10, 31)).unwrap()[0].scheduled);
        assert!(!habits_for_date(&connection, date(2026, 11, 11)).unwrap()[0].scheduled);
    }

    #[test]
    fn weekly_frequency_uses_a_monday_anchored_evaluation_period() {
        let connection = memory_connection();
        let habit_id = new_habit(&connection, "Practice piano");
        connection
            .execute(
                "UPDATE habits SET schedule_type = 'times_per_week', target_per_period = 3,
                period_unit = 'week', start_date = '2026-11-01' WHERE id = ?1",
                [habit_id],
            )
            .expect("set times-per-week schedule");

        assert!(habits_for_date(&connection, date(2026, 11, 9)).unwrap()[0].scheduled);
        assert!(habits_for_date(&connection, date(2026, 11, 14)).unwrap()[0].scheduled);
    }

    #[test]
    fn unscheduled_checkins_are_rejected_and_due_checkins_are_saved() {
        let connection = memory_connection();
        let habit_id = new_habit(&connection, "Read");
        connection
            .execute(
                "UPDATE habits SET schedule_type = 'weekdays', weekday_mask = 1,
                start_date = '2026-11-01' WHERE id = ?1",
                [habit_id],
            )
            .expect("set Monday-only schedule");

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
