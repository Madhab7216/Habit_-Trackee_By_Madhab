# M6 — Local data portability

This slice adds a local command-line companion. It does not change the GTK lifecycle, persisted scheduling rules, scoring engine, Linux Mint environment audit or main application UI.

## Operations

### Export every supported user-data table to CSV

```bash
cargo run --bin madhab-data-tool -- export-csv "$HOME/Documents/Madhab-Habit-Export"
```

The output directory must not already exist. The exporter writes one RFC 4180 CSV per available user-data table plus `manifest.txt`. It covers categories, habits, entries, pause periods, tags, tag links, goals, goal links and settings. Schema migration bookkeeping is represented by the manifest's schema version, not exported as user content.

The export uses one SQLite read transaction so all CSV files come from the same logical snapshot. Null values become empty fields and BLOB values become lowercase hex. CSV text is exported as data without spreadsheet-formula rewriting; when opening files in spreadsheet software, import columns as text and review cells beginning with `=`, `+`, `-` or `@`.

### Create a consistent database backup

```bash
cargo run --bin madhab-data-tool -- backup "$HOME/Documents/madhab-habits-2026-11-14.sqlite3"
```

The backup uses SQLite's online backup API, then runs integrity and foreign-key checks. Existing destination files are refused, not overwritten.

### Verify an existing backup

```bash
cargo run --bin madhab-data-tool -- verify "$HOME/Documents/madhab-habits-2026-11-14.sqlite3"
```

### Restore to a separate new database file

```bash
cargo run --bin madhab-data-tool -- restore \
  "$HOME/Documents/madhab-habits-2026-11-14.sqlite3" \
  "$HOME/Documents/madhab-habits-restored.sqlite3"
```

Restore first verifies the source. It then writes a new database file and verifies the result. It never overwrites or switches the currently active application database. Inspect the restored file before choosing to use it.

## Safety properties

- All commands are local. No data is uploaded or sent to a service.
- Explicit output paths are required; there is no silent automatic export.
- Existing backup files and export directories are never overwritten.
- On Unix, backup files are created with owner read/write permissions (`0600`); export directories use owner-only permissions (`0700`), and CSV/manifest files use `0600`.
- Failed backup/restore operations remove the incomplete destination file. Failed CSV exports remove the incomplete output directory.
- Verification checks SQLite `integrity_check`, `foreign_key_check`, required app tables and supported migration versions.
- Restore targets a new file only; it does not replace the live database or perform an implicit app migration.

## Validation

The module includes tests for backup/restore round trips, preservation of entries/goals/pause history, corrupt backup rejection, refusal to overwrite existing destinations, complete CSV table coverage and RFC 4180 escaping of comma, quote and newline characters.

## Scope boundary

This does not implement a GTK export dialog, scheduled backups, cloud sync, encryption-at-rest for external backup media or backup retention policies. The user chooses the destination and remains responsible for protecting and securely storing the exported files. The GTK app's existing database path and lifecycle workflows are unchanged.
