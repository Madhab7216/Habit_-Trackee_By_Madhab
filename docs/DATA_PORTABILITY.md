# M6 — Local data portability

The companion CLI keeps private habit data local and provides explicit backup, verification, restore-to-new-file and CSV export actions.

## Commands

```bash
cargo run --bin madhab-data-tool -- backup "$HOME/Documents/madhab-habits-backup.sqlite3"
cargo run --bin madhab-data-tool -- verify "$HOME/Documents/madhab-habits-backup.sqlite3"
cargo run --bin madhab-data-tool -- restore   "$HOME/Documents/madhab-habits-backup.sqlite3"   "$HOME/Documents/madhab-habits-restored.sqlite3"
cargo run --bin madhab-data-tool -- export-csv "$HOME/Documents/Madhab-Habit-Export"
```

## Safety and validation

- Backup uses SQLite's online backup API, then runs integrity and foreign-key checks.
- Restore verifies the source and writes a separate new file. It never replaces the active database.
- CSV export takes one read snapshot and writes RFC 4180 CSVs plus a manifest.
- Existing output files and directories are not overwritten.
- On Unix, backup files and export files are owner-readable/writable only (0600); export directories are owner-only (0700).
- Failed operations remove incomplete outputs.
- Export covers habit, pause, tag, goal, milestone, routine-template and routine-run tables when they exist.
- A routine migration ledger is checked when present; verified older databases without routines remain supported.

## Scope boundary

No cloud upload or service call occurs. The user chooses the destination and remains responsible for the exported files. A GTK backup dialog, scheduled backups, encryption at rest for external media, retention policy and restore UI are still future work.
