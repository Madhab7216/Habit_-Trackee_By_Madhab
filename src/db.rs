use rusqlite::{Connection, Result, params};
use std::{env, fs, path::PathBuf};

const INITIAL_MIGRATION: &str = include_str!("../migrations/0001_initial.sql");
const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone)]
pub struct TodayHabit {
    pub id: i64,
    pub name: String,
    pub completed: bool,
}

pub fn open() -> Result<Connection> {
    let data_dir = data_directory();
    fs::create_dir_all(&data_dir)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;

    let path = data_dir.join("habit-tracker.sqlite3");
    let mut connection = Connection::open(path)?;
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")?;
    migrate(&mut connection)?;
    Ok(connection)
}

fn data_directory() -> PathBuf {
    if let Some(xdg_data_home) = env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(xdg_data_home).join("madhab-habit-tracker");
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("madhab-habit-tracker");
    }
    PathBuf::from(".").join(".madhab-habit-tracker")
}

fn migrate(connection: &mut Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );",
    )?;

    let current_version: i64 = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;

    if current_version > SCHEMA_VERSION {
        return Err(rusqlite::Error::InvalidQuery);
    }

    if current_version < SCHEMA_VERSION {
        let transaction = connection.transaction()?;
        transaction.execute_batch(INITIAL_MIGRATION)?;
        transaction.execute(
            "INSERT INTO schema_migrations(version) VALUES (?1)",
            [SCHEMA_VERSION],
        )?;
        transaction.commit()?;
    }
    Ok(())
}

pub fn create_habit(connection: &Connection, name: &str) -> Result<()> {
    let normalized = name.trim();
    if normalized.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "habit name must not be empty".to_string(),
        ));
    }
    connection.execute(
        "INSERT INTO habits(name, tracking_type, schedule_type) VALUES (?1, 'binary', 'daily')",
        params![normalized],
    )?;
    Ok(())
}

pub fn today_habits(connection: &Connection) -> Result<Vec<TodayHabit>> {
    let mut statement = connection.prepare(
        "SELECT h.id, h.name,
                EXISTS (
                    SELECT 1 FROM habit_entries e
                    WHERE e.habit_id = h.id
                      AND e.local_date = date('now', 'localtime')
                      AND e.status = 'completed'
                ) AS completed
         FROM habits h
         WHERE h.archived_at IS NULL
         ORDER BY h.sort_order, h.id",
    )?;

    let rows = statement.query_map([], |row| {
        Ok(TodayHabit {
            id: row.get(0)?,
            name: row.get(1)?,
            completed: row.get::<_, i64>(2)? != 0,
        })
    })?;
    rows.collect()
}

pub fn set_today_completed(connection: &Connection, habit_id: i64, completed: bool) -> Result<()> {
    if completed {
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
