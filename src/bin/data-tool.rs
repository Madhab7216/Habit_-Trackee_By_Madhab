#[path = "../db.rs"]
pub mod db;
#[path = "../domain.rs"]
pub mod domain;
#[path = "../portability.rs"]
pub mod portability;
#[path = "../routines.rs"]
pub mod routines;

use std::{env, error::Error, io, path::Path, process::ExitCode};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("madhab-data-tool: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    let arguments = env::args().collect::<Vec<_>>();

    match arguments.as_slice() {
        [_, command, destination] if command == "export-csv" => {
            let mut connection = db::open()?;
            portability::export_csv_bundle(&mut connection, Path::new(destination))?;
            println!("Spreadsheet-safe CSV export created and synced to: {destination}");
        }
        [_, command, destination] if command == "export-csv-raw" => {
            let mut connection = db::open()?;
            portability::export_csv_bundle_raw(&mut connection, Path::new(destination))?;
            println!("Lossless raw CSV export created: {destination}");
            eprintln!("Warning: raw CSV text is not neutralized against spreadsheet formulas.");
        }
        [_, command, destination] if command == "backup" => {
            let connection = db::open()?;
            portability::create_verified_backup(&connection, Path::new(destination))?;
            println!("Verified database backup created: {destination}");
        }
        [_, command, backup, destination] if command == "restore" => {
            portability::restore_backup_to_new_file(Path::new(backup), Path::new(destination))?;
            println!("Backup verified and restored to a new database: {destination}");
            println!("The active application database was not changed.");
        }
        [_, command, backup] if command == "verify" => {
            let version = portability::verify_backup_file(Path::new(backup))?;
            println!("Backup is valid. Database schema version: {version}");
        }
        [_, command] if command == "--help" || command == "help" => {
            print_usage();
        }
        _ => {
            print_usage();
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid command or argument count",
            )
            .into());
        }
    }

    Ok(())
}

fn print_usage() {
    eprintln!(
        "Habit Trackers by Madhab — local data tools

Usage:
  madhab-data-tool export-csv <new-output-directory>
  madhab-data-tool export-csv-raw <new-output-directory>
  madhab-data-tool backup <new-backup-file>
  madhab-data-tool verify <backup-file>
  madhab-data-tool restore <backup-file> <new-database-file>

Safety:
  - Existing destination files/directories are never overwritten.
  - Restore writes a separate new database; it never replaces the active database.
  - export-csv neutralizes formula-like text for spreadsheet viewing.
  - export-csv-raw preserves exact text and must only be opened in trusted workflows.
  - All operations are local and do not upload your habit data."
    );
}
