//! Privacy-conscious export, verified backup and safe restore operations.
//!
//! All output paths are explicit. Existing files/directories are never overwritten.
//! Backup files and CSV bundles are created with owner-only permissions on Unix.

use rusqlite::{Connection, OpenFlags, backup::Backup, types::ValueRef};
use std::{
    error::Error,
    fs::{self, DirBuilder, File, OpenOptions},
    io::{self, BufWriter, Write},
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub type PortabilityResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const CURRENT_SCHEMA_VERSION: i64 = 2;
const EXPORT_FORMAT_VERSION: u32 = 1;
const EXPORT_TABLES: &[&str] = &[
    "categories",
    "habits",
    "habit_entries",
    "habit_pause_periods",
    "habit_tags",
    "habit_tag_links",
    "goals",
    "goal_habits",
    "goal_milestones",
    "routines",
    "routine_steps",
    "routine_runs",
    "routine_run_steps",
    "settings",
];

fn ensure_parent_directory(destination: &Path) -> io::Result<()> {
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn create_private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).read(true).create_new(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    options.open(path)
}

fn create_private_directory(path: &Path) -> io::Result<()> {
    let mut builder = DirBuilder::new();

    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }

    builder.create(path)
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn schema_version(connection: &Connection) -> PortabilityResult<i64> {
    let has_migrations: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'table' AND name = 'schema_migrations'
        )",
        [],
        |row| row.get(0),
    )?;

    if !has_migrations {
        return Err(invalid_data("database is missing schema_migrations").into());
    }

    let version: i64 = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;

    if !(1..=CURRENT_SCHEMA_VERSION).contains(&version) {
        return Err(invalid_data(format!(
            "unsupported database schema version {version}; this build supports versions 1 through {CURRENT_SCHEMA_VERSION}"
        ))
        .into());
    }

    Ok(version)
}

/// Verify database integrity, foreign-key consistency and the expected app schema.
pub fn verify_database(connection: &Connection) -> PortabilityResult<i64> {
    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(invalid_data(format!("SQLite integrity_check failed: {integrity}")).into());
    }

    {
        let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
        let mut rows = statement.query([])?;
        if let Some(row) = rows.next()? {
            let table: String = row.get(0)?;
            let rowid: Option<i64> = row.get(1)?;
            return Err(invalid_data(format!(
                "foreign-key violation in table {table}, row {rowid:?}"
            ))
            .into());
        }
    }

    for required_table in ["habits", "habit_entries", "goals", "settings"] {
        let exists: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = ?1
            )",
            [required_table],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(invalid_data(format!(
                "database is missing required table {required_table}"
            ))
            .into());
        }
    }

    let has_routine_ledger: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'table' AND name = 'routine_schema_migrations'
        )",
        [],
        |row| row.get(0),
    )?;
    if has_routine_ledger {
        let routine_version: i64 = connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM routine_schema_migrations",
            [],
            |row| row.get(0),
        )?;
        if routine_version > 1 {
            return Err(invalid_data(format!(
                "unsupported routine schema version {routine_version}"
            ))
            .into());
        }
        if routine_version >= 1 {
            for required_table in [
                "routines",
                "routine_steps",
                "routine_runs",
                "routine_run_steps",
            ] {
                let exists: bool = connection.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM sqlite_master
                        WHERE type = 'table' AND name = ?1
                    )",
                    [required_table],
                    |row| row.get(0),
                )?;
                if !exists {
                    return Err(invalid_data(format!(
                        "routine schema is missing required table {required_table}"
                    ))
                    .into());
                }
            }
        }
    }

    let version = schema_version(connection)?;
    if version >= 2 {
        let has_pause_history: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'habit_pause_periods'
            )",
            [],
            |row| row.get(0),
        )?;
        if !has_pause_history {
            return Err(
                invalid_data("schema version 2 database is missing habit_pause_periods").into(),
            );
        }
    }

    Ok(version)
}

/// Verify a backup file using a read-only SQLite connection.
pub fn verify_backup_file(path: &Path) -> PortabilityResult<i64> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    verify_database(&connection)
}

/// Create a consistent SQLite online backup.
///
/// The destination must not already exist. The file is reserved with create_new before
/// SQLite opens it, so an existing backup or unrelated file is never overwritten.
pub fn create_verified_backup(source: &Connection, destination: &Path) -> PortabilityResult<()> {
    ensure_parent_directory(destination)?;
    let reservation = create_private_file(destination)?;
    drop(reservation);

    let result = (|| -> PortabilityResult<()> {
        let mut destination_connection = Connection::open(destination)?;
        {
            let backup = Backup::new(source, &mut destination_connection)?;
            backup.run_to_completion(
                128,
                Duration::from_millis(20),
                None::<fn(rusqlite::backup::Progress)>,
            )?;
        }
        verify_database(&destination_connection)?;
        drop(destination_connection);
        verify_backup_file(destination)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(destination);
    }

    result
}

/// Restore a verified backup into a new database file.
///
/// This never replaces the active database or an existing file. The caller can inspect
/// the restored file and explicitly choose when to switch the application to it.
pub fn restore_backup_to_new_file(backup_path: &Path, destination: &Path) -> PortabilityResult<()> {
    let source_version = verify_backup_file(backup_path)?;
    ensure_parent_directory(destination)?;
    let reservation = create_private_file(destination)?;
    drop(reservation);

    let result = (|| -> PortabilityResult<()> {
        let source = Connection::open_with_flags(backup_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mut restored = Connection::open(destination)?;
        {
            let backup = Backup::new(&source, &mut restored)?;
            backup.run_to_completion(
                128,
                Duration::from_millis(20),
                None::<fn(rusqlite::backup::Progress)>,
            )?;
        }

        let restored_version = verify_database(&restored)?;
        if restored_version != source_version {
            return Err(invalid_data(format!(
                "restored schema version {restored_version} differs from source version {source_version}"
            ))
            .into());
        }
        drop(restored);
        verify_backup_file(destination)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(destination);
    }

    result
}

/// Export all known user-owned tables into a new directory of RFC 4180 CSV files.
///
/// A read transaction gives every CSV file the same logical database snapshot.
/// Null values become empty fields; BLOB values are written as lowercase hex.
/// Export a spreadsheet-safe CSV bundle by neutralizing formula-like text cells.
///
/// Values changed by this export are marked in the manifest. Use
/// `export_csv_bundle_raw` when exact, machine-readable text preservation is required.
pub fn export_csv_bundle(source: &mut Connection, destination: &Path) -> PortabilityResult<()> {
    export_csv_bundle_with_mode(source, destination, true)
}

/// Export an exact, lossless CSV snapshot.
///
/// This mode intentionally does not neutralize spreadsheet formulas. Only use it for
/// trusted data-processing workflows; prefer the default spreadsheet-safe export when
/// a person will open the files in Excel, LibreOffice Calc or another spreadsheet.
pub fn export_csv_bundle_raw(
    source: &mut Connection,
    destination: &Path,
) -> PortabilityResult<()> {
    export_csv_bundle_with_mode(source, destination, false)
}

fn export_csv_bundle_with_mode(
    source: &mut Connection,
    destination: &Path,
    spreadsheet_safe: bool,
) -> PortabilityResult<()> {
    ensure_parent_directory(destination)?;
    create_private_directory(destination)?;

    let result = (|| -> PortabilityResult<()> {
        let transaction = source.transaction()?;
        let version = schema_version(&transaction)?;
        let mut exported_files = Vec::new();

        for table in EXPORT_TABLES {
            if table_exists(&transaction, table)? {
                let filename = format!("{table}.csv");
                export_table(
                    &transaction,
                    table,
                    &destination.join(&filename),
                    spreadsheet_safe,
                )?;
                exported_files.push(filename);
            }
        }

        write_manifest(destination, version, &exported_files, spreadsheet_safe)?;
        transaction.commit()?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }

    result
}

fn table_exists(connection: &Connection, table: &str) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'table' AND name = ?1
        )",
        [table],
        |row| row.get(0),
    )
}

fn export_table(
    connection: &Connection,
    table: &str,
    destination: &Path,
    spreadsheet_safe: bool,
) -> PortabilityResult<()> {
    // Table names come only from EXPORT_TABLES above, never from user input.
    let sql = format!("SELECT * FROM \"{table}\" ORDER BY 1");
    let mut statement = connection.prepare(&sql)?;
    let headers = statement
        .column_names()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();

    let file = create_private_file(destination)?;
    let mut writer = BufWriter::new(file);
    write_csv_record(&mut writer, &headers)?;

    let column_count = statement.column_count();
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let mut fields = Vec::with_capacity(column_count);
        let mut force_quotes = Vec::with_capacity(column_count);
        for index in 0..column_count {
            let value = row.get_ref(index)?;
            fields.push(match value {
                ValueRef::Null => {
                    force_quotes.push(false);
                    String::new()
                }
                ValueRef::Integer(number) => {
                    force_quotes.push(false);
                    number.to_string()
                }
                ValueRef::Real(number) => {
                    force_quotes.push(false);
                    number.to_string()
                }
                ValueRef::Text(bytes) => {
                    let text = String::from_utf8_lossy(bytes).into_owned();
                    if spreadsheet_safe {
                        let (safe_text, must_quote) = spreadsheet_safe_text(&text);
                        force_quotes.push(must_quote);
                        safe_text
                    } else {
                        force_quotes.push(false);
                        text
                    }
                }
                ValueRef::Blob(bytes) => {
                    force_quotes.push(false);
                    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
                }
            });
        }
        write_csv_record_with_quote_flags(&mut writer, &fields, &force_quotes)?;
    }

    writer.flush()?;
    writer.get_ref().sync_all()?;
    Ok(())
}

/// Return a spreadsheet-oriented representation of untrusted text.
///
/// OWASP documents tab-prefixing formula-leading cells as an Excel-resistant mitigation
/// (with a data-fidelity trade-off). Leading control characters use an apostrophe prefix.
/// This is defense in depth, not a universal guarantee across every spreadsheet program.
fn spreadsheet_safe_text(value: &str) -> (String, bool) {
    if matches!(value.chars().next(), Some('\t' | '\r' | '\n')) {
        return (format!("'{value}"), true);
    }

    let first_non_whitespace = value.trim_start().chars().next();
    if matches!(
        first_non_whitespace,
        Some('=' | '+' | '-' | '@' | '＝' | '＋' | '－' | '＠')
    ) {
        return (format!("\t{value}"), true);
    }

    (value.to_owned(), false)
}

fn write_csv_record(writer: &mut impl Write, fields: &[String]) -> io::Result<()> {
    write_csv_record_with_quote_flags(writer, fields, &[])
}

fn write_csv_record_with_quote_flags(
    writer: &mut impl Write,
    fields: &[String],
    force_quotes: &[bool],
) -> io::Result<()> {
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            writer.write_all(b",")?;
        }

        let requires_quotes = force_quotes.get(index).copied().unwrap_or(false)
            || field
                .bytes()
                .any(|byte| matches!(byte, b',' | b'"' | b'\r' | b'\n'));

        if requires_quotes {
            writer.write_all(b"\"")?;
            for byte in field.bytes() {
                if byte == b'"' {
                    writer.write_all(b"\"\"")?;
                } else {
                    writer.write_all(&[byte])?;
                }
            }
            writer.write_all(b"\"")?;
        } else {
            writer.write_all(field.as_bytes())?;
        }
    }

    writer.write_all(b"\r\n")
}

fn write_manifest(
    destination: &Path,
    schema_version: i64,
    exported_files: &[String],
    spreadsheet_safe: bool,
) -> PortabilityResult<()> {
    let manifest_path = destination.join("manifest.txt");
    let file = create_private_file(&manifest_path)?;
    let mut writer = BufWriter::new(file);
    let exported_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    writeln!(writer, "format_version={EXPORT_FORMAT_VERSION}")?;
    writeln!(writer, "database_schema_version={schema_version}")?;
    writeln!(writer, "exported_at_unix_seconds={exported_at}")?;
    writeln!(writer, "null_values=empty_csv_fields")?;
    writeln!(writer, "blob_values=lowercase_hex")?;
    writeln!(
        writer,
        "csv_mode={}",
        if spreadsheet_safe {
            "spreadsheet_safe"
        } else {
            "lossless_raw"
        }
    )?;
    writeln!(
        writer,
        "formula_like_text_cells={}",
        if spreadsheet_safe {
            "tab_prefix_or_apostrophe_prefix"
        } else {
            "unchanged"
        }
    )?;
    writeln!(writer, "files={}", exported_files.join(","))?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    fn temp_path(label: &str) -> PathBuf {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "madhab-portability-{label}-{}-{sequence}",
            std::process::id()
        ))
    }

    fn populated_database() -> Connection {
        let connection = Connection::open_in_memory().expect("open test database");
        connection
            .execute_batch(include_str!("../migrations/0001_initial.sql"))
            .expect("apply schema v1");
        connection
            .execute("INSERT INTO schema_migrations(version) VALUES (1)", [])
            .expect("record schema v1");
        connection
            .execute_batch(include_str!("../migrations/0002_pause_periods.sql"))
            .expect("apply schema v2");
        connection
            .execute("INSERT INTO schema_migrations(version) VALUES (2)", [])
            .expect("record schema v2");

        connection
            .execute(
                "INSERT INTO categories(name, color) VALUES ('Health', '#88aa44')",
                [],
            )
            .expect("insert category");
        connection
            .execute(
                "INSERT INTO habits(name, description, category_id)
                 VALUES (?1, ?2, 1)",
                [
                    "Drink \"water\", slowly\nand consistently",
                    "Line one\nLine two",
                ],
            )
            .expect("insert habit");
        connection
            .execute(
                "INSERT INTO habit_entries(habit_id, local_date, status, value, note)
                 VALUES (1, '2026-11-14', 'completed', 2, 'Hydration, done')",
                [],
            )
            .expect("insert entry");
        connection
            .execute(
                "INSERT INTO habit_pause_periods(habit_id, starts_on, ends_on)
                 VALUES (1, '2026-11-10', '2026-11-12')",
                [],
            )
            .expect("insert pause history");
        connection
            .execute("INSERT INTO habit_tags(name) VALUES ('Morning')", [])
            .expect("insert tag");
        connection
            .execute(
                "INSERT INTO habit_tag_links(habit_id, tag_id) VALUES (1, 1)",
                [],
            )
            .expect("link tag");
        connection
            .execute(
                "INSERT INTO goals(title, description) VALUES ('Better health', 'Long-term goal')",
                [],
            )
            .expect("insert goal");
        connection
            .execute(
                "INSERT INTO goal_habits(goal_id, habit_id) VALUES (1, 1)",
                [],
            )
            .expect("link goal");
        connection
            .execute_batch(
                "CREATE TABLE goal_schema_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                 );",
            )
            .expect("create goal migration ledger");
        connection
            .execute_batch(include_str!("../migrations/0003_goal_milestones.sql"))
            .expect("apply goal milestone migration");
        connection
            .execute("INSERT INTO goal_schema_migrations(version) VALUES (1)", [])
            .expect("record goal milestone migration");
        connection
            .execute(
                "INSERT INTO goal_milestones(goal_id, title, description, status)
                 VALUES (1, 'First milestone', 'A measurable checkpoint', 'completed')",
                [],
            )
            .expect("insert goal milestone");
        connection
            .execute(
                "INSERT INTO settings(key, value) VALUES ('language', 'bn')",
                [],
            )
            .expect("insert setting");

        connection
            .execute_batch(
                "CREATE TABLE routine_schema_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                 );",
            )
            .expect("create routine migration ledger");
        connection
            .execute_batch(include_str!("../migrations/0004_routines.sql"))
            .expect("apply routine schema");
        connection
            .execute(
                "INSERT INTO routine_schema_migrations(version) VALUES (1)",
                [],
            )
            .expect("record routine schema version");
        connection
            .execute(
                "INSERT INTO routines(title, start_date) VALUES ('Morning focus', '2026-11-14')",
                [],
            )
            .expect("insert routine");
        connection
            .execute(
                "INSERT INTO routine_steps(routine_id, title, sort_order) VALUES (1, 'Drink water', 0)",
                [],
            )
            .expect("insert routine step");
        connection
            .execute(
                "INSERT INTO routine_runs(routine_id, local_date, status) VALUES (1, '2026-11-14', 'completed')",
                [],
            )
            .expect("insert routine run");
        connection
            .execute(
                "INSERT INTO routine_run_steps
                    (run_id, step_id, title_snapshot, step_type_snapshot, is_required, sort_order, status)
                 VALUES (1, 1, 'Drink water', 'action', 1, 0, 'completed')",
                [],
            )
            .expect("insert routine run step");

        connection
    }

    #[test]
    fn backup_and_restore_preserve_user_data() {
        let source = populated_database();
        let backup_path = temp_path("backup.sqlite3");
        let restored_path = temp_path("restored.sqlite3");

        create_verified_backup(&source, &backup_path).expect("create verified backup");
        assert_eq!(verify_backup_file(&backup_path).unwrap(), 2);

        restore_backup_to_new_file(&backup_path, &restored_path)
            .expect("restore into a new database file");

        let restored = Connection::open(&restored_path).expect("open restored database");
        let habit_name: String = restored
            .query_row("SELECT name FROM habits WHERE id = 1", [], |row| row.get(0))
            .expect("read restored habit");
        let entry_count: i64 = restored
            .query_row("SELECT COUNT(*) FROM habit_entries", [], |row| row.get(0))
            .expect("count restored entries");
        let pause_count: i64 = restored
            .query_row("SELECT COUNT(*) FROM habit_pause_periods", [], |row| {
                row.get(0)
            })
            .expect("count restored pause periods");
        let goal_count: i64 = restored
            .query_row("SELECT COUNT(*) FROM goals", [], |row| row.get(0))
            .expect("count restored goals");
        let milestone_count: i64 = restored
            .query_row("SELECT COUNT(*) FROM goal_milestones", [], |row| row.get(0))
            .expect("count restored goal milestones");
        let routine_count: i64 = restored
            .query_row("SELECT COUNT(*) FROM routines", [], |row| row.get(0))
            .expect("count restored routines");
        let routine_run_step_count: i64 = restored
            .query_row("SELECT COUNT(*) FROM routine_run_steps", [], |row| {
                row.get(0)
            })
            .expect("count restored routine run steps");

        assert_eq!(habit_name, "Drink \"water\", slowly\nand consistently");
        assert_eq!(entry_count, 1);
        assert_eq!(pause_count, 1);
        assert_eq!(goal_count, 1);
        assert_eq!(milestone_count, 1);
        assert_eq!(routine_count, 1);
        assert_eq!(routine_run_step_count, 1);
        drop(restored);

        fs::remove_file(backup_path).expect("remove test backup");
        fs::remove_file(restored_path).expect("remove restored test database");
    }

    #[test]
    fn backup_never_overwrites_an_existing_file() {
        let source = populated_database();
        let destination = temp_path("keep-existing.sqlite3");
        fs::write(&destination, b"keep this file").expect("create existing destination");

        assert!(create_verified_backup(&source, &destination).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"keep this file");

        fs::remove_file(destination).expect("remove test destination");
    }

    #[test]
    fn restore_rejects_an_invalid_backup_and_creates_no_destination() {
        let invalid_backup = temp_path("invalid.sqlite3");
        let destination = temp_path("should-not-exist.sqlite3");
        fs::write(&invalid_backup, b"not a SQLite database").expect("write invalid backup");

        assert!(restore_backup_to_new_file(&invalid_backup, &destination).is_err());
        assert!(!destination.exists());

        fs::remove_file(invalid_backup).expect("remove invalid backup");
    }

    #[test]
    fn restore_never_overwrites_an_existing_database() {
        let source = populated_database();
        let backup_path = temp_path("restore-protected.sqlite3");
        let destination = temp_path("restore-existing.sqlite3");
        create_verified_backup(&source, &backup_path).expect("create backup");
        fs::write(&destination, b"existing data").expect("create destination");

        assert!(restore_backup_to_new_file(&backup_path, &destination).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"existing data");

        fs::remove_file(backup_path).expect("remove backup");
        fs::remove_file(destination).expect("remove destination");
    }

    #[test]
    fn csv_bundle_exports_all_user_tables_and_quotes_text_fields() {
        let mut source = populated_database();
        let destination = temp_path("csv-export");

        export_csv_bundle(&mut source, &destination).expect("export CSV bundle");

        let entries =
            fs::read_to_string(destination.join("habit_entries.csv")).expect("read entries CSV");
        let habits = fs::read_to_string(destination.join("habits.csv")).expect("read habits CSV");
        let manifest = fs::read_to_string(destination.join("manifest.txt")).expect("read manifest");

        assert!(entries.contains("habit_id,local_date,status,value,note"));
        assert!(habits.contains("\"Drink \"\"water\"\", slowly\nand consistently\""));
        assert!(manifest.contains("format_version=1"));
        assert!(manifest.contains("database_schema_version=2"));
        for filename in [
            "categories.csv",
            "habits.csv",
            "habit_entries.csv",
            "habit_pause_periods.csv",
            "habit_tags.csv",
            "habit_tag_links.csv",
            "goals.csv",
            "goal_habits.csv",
            "routines.csv",
            "routine_steps.csv",
            "routine_runs.csv",
            "routine_run_steps.csv",
            "settings.csv",
        ] {
            assert!(destination.join(filename).is_file(), "missing {filename}");
        }

        fs::remove_dir_all(destination).expect("remove test export");
    }

    #[test]
    fn csv_export_never_overwrites_an_existing_directory() {
        let mut source = populated_database();
        let destination = temp_path("keep-export");
        fs::create_dir(&destination).expect("create existing folder");
        fs::write(destination.join("keep.txt"), b"keep this file").expect("create marker");

        assert!(export_csv_bundle(&mut source, &destination).is_err());
        assert_eq!(
            fs::read(destination.join("keep.txt")).unwrap(),
            b"keep this file"
        );

        fs::remove_dir_all(destination).expect("remove test folder");
    }

    #[test]
    fn spreadsheet_safe_text_neutralizes_formula_and_control_prefixes() {
        assert_eq!(spreadsheet_safe_text("=1+1"), ("\t=1+1".to_string(), true));
        assert_eq!(spreadsheet_safe_text("  +1+1"), ("\t  +1+1".to_string(), true));
        assert_eq!(spreadsheet_safe_text("@SUM(1,1)"), ("\t@SUM(1,1)".to_string(), true));
        assert_eq!(spreadsheet_safe_text("＝1+1"), ("\t＝1+1".to_string(), true));
        assert_eq!(spreadsheet_safe_text("  ＠SUM(1,1)"), ("\t  ＠SUM(1,1)".to_string(), true));
        assert_eq!(spreadsheet_safe_text("\t=1+1"), ("'\t=1+1".to_string(), true));
        assert_eq!(spreadsheet_safe_text("\n=1+1"), ("'\n=1+1".to_string(), true));
        assert_eq!(spreadsheet_safe_text("Read 20 pages"), ("Read 20 pages".to_string(), false));
    }

    #[test]
    fn spreadsheet_safe_export_neutralizes_untrusted_text_and_quotes_the_cells() {
        let mut source = populated_database();
        source
            .execute(
                "UPDATE habits SET name = ?1, description = ?2 WHERE id = 1",
                ["=1+1", "  +SUM(1,1)"],
            )
            .expect("set formula-like habit text");
        source
            .execute(
                "UPDATE habit_entries SET note = ?1 WHERE habit_id = 1",
                ["\t=HYPERLINK(\"https://example.invalid\",\"open\")"],
            )
            .expect("set formula-like check-in note");
        source
            .execute(
                "UPDATE goals SET title = ?1 WHERE id = 1",
                ["＝SUM(1,1)"],
            )
            .expect("set full-width formula-like goal title");
        source
            .execute(
                "UPDATE routines SET title = ?1 WHERE id = 1",
                ["=1+1; \"quoted\",=2+2"],
            )
            .expect("set CSV breakout-like routine title");

        let destination = temp_path("spreadsheet-safe");
        export_csv_bundle(&mut source, &destination).expect("create spreadsheet-safe export");

        let habits = fs::read_to_string(destination.join("habits.csv")).expect("read habits CSV");
        let entries = fs::read_to_string(destination.join("habit_entries.csv"))
            .expect("read entries CSV");
        let goals = fs::read_to_string(destination.join("goals.csv")).expect("read goals CSV");
        let routines =
            fs::read_to_string(destination.join("routines.csv")).expect("read routines CSV");
        let manifest =
            fs::read_to_string(destination.join("manifest.txt")).expect("read manifest");

        assert!(habits.contains("\"\t=1+1\",\"\t  +SUM(1,1)\""));
        assert!(entries.contains("\"\t=HYPERLINK"));
        assert!(goals.contains("\"\t＝SUM(1,1)\""));
        assert!(routines.contains("\t=1+1; \"\"quoted\"\",=2+2"));
        assert!(manifest.contains("csv_mode=spreadsheet_safe"));
        assert!(manifest.contains("formula_like_text_cells=tab_prefix_or_apostrophe_prefix"));

        fs::remove_dir_all(destination).expect("remove safe export");
    }

    #[test]
    fn raw_csv_export_preserves_formula_like_text_exactly() {
        let mut source = populated_database();
        source
            .execute(
                "UPDATE habits SET name = ?1, description = ?2 WHERE id = 1",
                ["=1+1", "  +SUM(1,1)"],
            )
            .expect("set formula-like habit text");

        let destination = temp_path("raw-csv");
        export_csv_bundle_raw(&mut source, &destination).expect("create raw export");

        let habits = fs::read_to_string(destination.join("habits.csv")).expect("read habits CSV");
        let manifest =
            fs::read_to_string(destination.join("manifest.txt")).expect("read manifest");

        assert!(habits.contains(",=1+1,  +SUM(1,1),"));
        assert!(manifest.contains("csv_mode=lossless_raw"));
        assert!(manifest.contains("formula_like_text_cells=unchanged"));

        fs::remove_dir_all(destination).expect("remove raw export");
    }

    #[test]
    fn verification_rejects_corrupt_database_files() {
        let corrupt = temp_path("corrupt.sqlite3");
        fs::write(&corrupt, b"not sqlite").expect("write corrupt file");
        assert!(verify_backup_file(&corrupt).is_err());
        fs::remove_file(corrupt).expect("remove corrupt file");
    }
}
