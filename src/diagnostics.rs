//! Privacy-safe error classification for local database and file operations.
//!
//! Never format an underlying SQLite or I/O error directly for a user-facing
//! message or terminal diagnostic. Those errors can contain habit values,
//! SQL fragments, absolute paths or filesystem details.

use std::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    OpenDatabase,
    CreateHabit,
    LoadHabits,
    RestoreHabit,
    CompleteHabit,
    ChangePauseState,
    ArchiveHabit,
    RenameHabit,
    PrepareGoalStorage,
    CreateGoal,
    LoadGoals,
    RestoreGoal,
    UpdateGoal,
    CompleteGoal,
    ArchiveGoal,
    CreateMilestone,
    LoadMilestones,
    UpdateMilestone,
    SkipMilestone,
}

impl Operation {
    const fn label(self) -> &'static str {
        match self {
            Self::OpenDatabase => "database_open",
            Self::CreateHabit => "habit_create",
            Self::LoadHabits => "habit_load",
            Self::RestoreHabit => "habit_restore",
            Self::CompleteHabit => "habit_check_in",
            Self::ChangePauseState => "habit_pause",
            Self::ArchiveHabit => "habit_archive",
            Self::RenameHabit => "habit_rename",
            Self::PrepareGoalStorage => "goal_storage_prepare",
            Self::CreateGoal => "goal_create",
            Self::LoadGoals => "goal_load",
            Self::RestoreGoal => "goal_restore",
            Self::UpdateGoal => "goal_update",
            Self::CompleteGoal => "goal_complete",
            Self::ArchiveGoal => "goal_archive",
            Self::CreateMilestone => "milestone_create",
            Self::LoadMilestones => "milestone_load",
            Self::UpdateMilestone => "milestone_update",
            Self::SkipMilestone => "milestone_skip",
        }
    }

    pub const fn user_message(self) -> &'static str {
        match self {
            Self::OpenDatabase => {
                "The local database could not be opened. Check storage permissions and available disk space."
            }
            Self::CreateHabit => "The habit could not be saved. Please review the input and try again.",
            Self::LoadHabits => "Habits could not be loaded. Your local data has not been intentionally changed.",
            Self::RestoreHabit => "The archived habit could not be restored. Please try again.",
            Self::CompleteHabit => "The habit check-in could not be saved. Please try again.",
            Self::ChangePauseState => "The habit pause state could not be changed. Please try again.",
            Self::ArchiveHabit => "The habit could not be archived. Please try again.",
            Self::RenameHabit => "The habit name could not be saved. Please review the name and try again.",
            Self::PrepareGoalStorage => "Goal storage could not be prepared. Check local storage permissions and available disk space.",
            Self::CreateGoal => "The goal could not be created. Please review the input and try again.",
            Self::LoadGoals => "Goals could not be loaded. Your local data has not been intentionally changed.",
            Self::RestoreGoal => "The archived goal could not be restored. Please try again.",
            Self::UpdateGoal => "The goal could not be updated. Please try again.",
            Self::CompleteGoal => "The goal could not be completed. Please try again.",
            Self::ArchiveGoal => "The goal could not be archived. Please try again.",
            Self::CreateMilestone => "The milestone could not be added. Please review the input and try again.",
            Self::LoadMilestones => "Milestones could not be loaded. Please try again.",
            Self::UpdateMilestone => "The milestone could not be updated. Please try again.",
            Self::SkipMilestone => "The milestone could not be skipped. Please try again.",
        }
    }
}

/// Return a small allow-listed classification, never an underlying error string.
pub fn safe_error_code(error: &(dyn Error + 'static)) -> &'static str {
    if let Some(sqlite) = error.downcast_ref::<rusqlite::Error>() {
        return sqlite_error_code(sqlite);
    }

    if let Some(io_error) = error.downcast_ref::<std::io::Error>() {
        return match io_error.kind() {
            std::io::ErrorKind::NotFound => "file_not_found",
            std::io::ErrorKind::PermissionDenied => "permission_denied",
            std::io::ErrorKind::AlreadyExists => "destination_exists",
            std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => "invalid_input",
            std::io::ErrorKind::WriteZero => "write_failed",
            std::io::ErrorKind::Interrupted => "operation_interrupted",
            std::io::ErrorKind::TimedOut => "operation_timed_out",
            std::io::ErrorKind::WouldBlock => "resource_busy",
            _ => "io_error",
        };
    }

    "operation_failed"
}

fn sqlite_error_code(error: &rusqlite::Error) -> &'static str {
    match error {
        rusqlite::Error::SqliteFailure(failure, _) => match failure.code {
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => {
                "database_busy"
            }
            rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase => {
                "database_integrity"
            }
            rusqlite::ErrorCode::PermissionDenied | rusqlite::ErrorCode::ReadOnly => {
                "database_access_denied"
            }
            rusqlite::ErrorCode::DiskFull => "disk_full",
            rusqlite::ErrorCode::SystemIoFailure | rusqlite::ErrorCode::CannotOpen => {
                "database_io_error"
            }
            rusqlite::ErrorCode::ConstraintViolation => "constraint_violation",
            _ => "database_error",
        },
        rusqlite::Error::QueryReturnedNoRows => "record_not_found",
        rusqlite::Error::InvalidParameterName(_) => "invalid_input",
        _ => "database_error",
    }
}

/// Stable diagnostic suitable for a terminal or support log.
pub fn format_diagnostic(operation: Operation, error: &(dyn Error + 'static)) -> String {
    format!(
        "habit_tracker operation={} code={}",
        operation.label(),
        safe_error_code(error)
    )
}

/// Stable CLI diagnostic. The command's arguments and filesystem paths are
/// deliberately not echoed into error output.
pub fn format_cli_diagnostic(error: &(dyn Error + 'static)) -> String {
    format!("madhab-data-tool code={}", safe_error_code(error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_diagnostics_never_include_private_values_or_paths() {
        let error = rusqlite::Error::InvalidParameterName(
            "Private habit: evening medication at /home/madhab/private.sqlite3".to_string(),
        );

        let diagnostic = format_diagnostic(Operation::CreateHabit, &error);
        assert_eq!(
            diagnostic,
            "habit_tracker operation=habit_create code=invalid_input"
        );
        assert!(!diagnostic.contains("Private habit"));
        assert!(!diagnostic.contains("/home/"));
        assert!(!diagnostic.contains("private.sqlite3"));
    }

    #[test]
    fn filesystem_diagnostics_only_expose_a_safe_error_class() {
        let error = std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "/home/madhab/private-habits/database.sqlite3",
        );

        assert_eq!(safe_error_code(&error), "permission_denied");
        let diagnostic = format_cli_diagnostic(&error);
        assert_eq!(diagnostic, "madhab-data-tool code=permission_denied");
        assert!(!diagnostic.contains("/home/"));
        assert!(!diagnostic.contains("private-habits"));
    }

    #[test]
    fn sqlite_failure_messages_are_not_forwarded() {
        let connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
        let error = connection
            .execute("SELECT * FROM table_that_does_not_exist", [])
            .expect_err("unknown table must fail");

        let diagnostic = format_diagnostic(Operation::LoadHabits, &error);
        assert_eq!(
            diagnostic,
            "habit_tracker operation=habit_load code=database_error"
        );
        assert!(!diagnostic.contains("no such table"));
        assert!(!diagnostic.contains("SELECT"));
    }

    #[test]
    fn user_messages_are_static_and_do_not_require_error_details() {
        assert_eq!(
            Operation::RenameHabit.user_message(),
            "The habit name could not be saved. Please review the name and try again."
        );
        assert!(!Operation::RenameHabit.user_message().contains("{"));
    }
}
