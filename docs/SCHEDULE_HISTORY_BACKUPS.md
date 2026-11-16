# Verified schedule-history backups

## Guarantee

The schedule-history service adds its own migration ledger and an append-only `habit_schedule_revisions` table without changing the existing core schema version. A normal SQLite online backup already copies every table, but a backup is not trustworthy if its schedule-history ledger says the migration is installed while the revision table or a habit's baseline is missing. The follow-up also prevents a time-limited newer revision from expiring and accidentally exposing an older superseded revision again.

The portability verifier now checks this additive schema alongside SQLite integrity and foreign-key consistency:

- If no schedule-history ledger exists, a pre-upgrade database remains valid.
- If the ledger exists, its version must match the version supported by this build.
- The revision table must exist.
- Every habit must have at least one baseline revision.
- Backup creation and restore run this validation; invalid/incomplete copies are rejected.

## Habit creation

The app initializes schedule history during database open. New habit insertion and creation of its legacy baseline revision happen inside the same SQLite transaction. A crash cannot leave a newly created habit committed without its initial schedule record.

Legacy baseline rows preserve the old timezone text as `legacy_unverified`; the system does not guess which IANA zone was in use historically.

## CSV export boundary

This change deliberately does not add the history table to CSV export. The separate spreadsheet-safe CSV work (PR #27) must be integrated into this base first so user-provided change reasons cannot become spreadsheet formulas. The verified SQLite backup always retains the full schedule-history table.

## Validation

Dedicated CI runs formatting, all-target compilation, Rust tests, strict Clippy, SQLite schema/migration tests and audit-script syntax checks. The backup/restore regression asserts that a restored database retains the schedule revision, while a corrupted history ledger/table pairing is rejected.
