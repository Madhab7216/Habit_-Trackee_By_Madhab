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
    let normalized = validate_habit_name(name)?;
    connection.execute(
        "INSERT INTO habits(name, tracking_type, schedule_type) VALUES (?1, 'binary', 'daily')",
        params![normalized],
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
        })
    })?;
    rows.collect()
}

pub fn today_habits(connection: &Connection) -> Result<Vec<TodayHabit>> {
    let mut statement = connection.prepare(
        "SELECT h.id, h.name,
                EXISTS (
                    SELECT 1 FROM habit_entries e
                    WHERE e.habit_id = h.id
                      AND e.local_date = date('now', 'localtime')
                      AND e.status = 'completed'
                ) AS completed,
                EXISTS (
                    SELECT 1 FROM habit_pause_periods p
                    WHERE p.habit_id = h.id
                      AND p.starts_on <= date('now', 'localtime')
                      AND (p.ends_on IS NULL OR date('now', 'localtime') < p.ends_on)
                ) AS paused
         FROM habits h
         WHERE h.archived_at IS NULL
         ORDER BY h.sort_order, h.id",
    )?;

    let rows = statement.query_map([], |row| {
        Ok(TodayHabit {
            id: row.get(0)?,
            name: row.get(1)?,
            completed: row.get::<_, i64>(2)? != 0,
            paused: row.get::<_, i64>(3)? != 0,
        })
    })?;
    rows.collect()
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
    let active: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM habits WHERE id = ?1 AND archived_at IS NULL)",
        [habit_id],
        |row| row.get(0),
    )?;
    if !active {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }

    if completed {
        let paused: bool = connection.query_row(
            "SELECT EXISTS (
                SELECT 1 FROM habit_pause_periods
                WHERE habit_id = ?1
                  AND starts_on <= date('now', 'localtime')
                  AND (ends_on IS NULL OR date('now', 'localtime') < ends_on)
             )",
            [habit_id],
            |row| row.get(0),
        )?;
        if paused {
            return Err(rusqlite::Error::InvalidParameterName(
                "a paused habit cannot be checked in".to_string(),
            ));
        }
        connection.execute(
            "INSERT INTO habit_entries(habit_id, local_date, status, value)
             VALUES (?1, date('now', 'localtime'), 'completed', 1.0)
             ON CONFLICT(habit_id, local_date) DO UPDATE SET
                status = 'completed', value = 1.0, updated_at = CURRENT_TIMESTAMP",
            [habit_id],
        )?;
    } else {
        connection.execute(
            "DELETE FROM habit_entries
             WHERE habit_id = ?1
               AND local_date = date('now', 'localtime')
               AND status = 'completed'",
            [habit_id],
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
            .query_row("SELECT id FROM habits WHERE name = 'Read'", [], |row| row.get(0))
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
            .query_row("SELECT id FROM habits WHERE name = 'Walk'", [], |row| row.get(0))
            .expect("read habit id");
        set_today_completed(&connection, habit_id, true).expect("complete habit");

        archive_habit(&connection, habit_id).expect("archive habit");
        assert!(today_habits(&connection).expect("load active habits").is_empty());
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
            .query_row("SELECT id FROM habits WHERE name = 'Journal'", [], |row| row.get(0))
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

}
