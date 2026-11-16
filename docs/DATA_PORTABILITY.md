# M6 — Local data portability

The companion CLI keeps private habit data local and provides explicit backup, verification, restore-to-new-file and CSV export actions.

## Commands

```bash
cargo run --bin madhab-data-tool -- backup "$HOME/Documents/madhab-habits-backup.sqlite3"
cargo run --bin madhab-data-tool -- verify "$HOME/Documents/madhab-habits-backup.sqlite3"
cargo run --bin madhab-data-tool -- restore "$HOME/Documents/madhab-habits-backup.sqlite3" "$HOME/Documents/madhab-habits-restored.sqlite3"

# Spreadsheet-safe export (recommended for opening in LibreOffice Calc or Excel)
cargo run --bin madhab-data-tool -- export-csv "$HOME/Documents/Madhab-Habit-Export"

# Exact, lossless text export (for trusted programmatic/data-processing workflows only)
cargo run --bin madhab-data-tool -- export-csv-raw "$HOME/Documents/Madhab-Habit-Raw-Export"
```

## CSV safety modes

The default `export-csv` command and the application CSV export use spreadsheet-safe mode. User-controlled text cells beginning with formula-like prefixes such as `=`, `+`, `-`, `@` and their full-width variants are prefixed with a tab inside a quoted CSV cell. Text beginning with a tab, carriage return or line feed is prefixed with an apostrophe. CSV quoting still escapes separators, quotes and line breaks so text cannot create a new cell by breaking out of its field.

This changes the exported representation of affected text cells. The original SQLite database is not modified. The export manifest records `csv_mode=spreadsheet_safe` and the applied prefix policy.

The explicit `export-csv-raw` command keeps text fields unchanged for data interchange and automation. Its output is **not** neutralized against spreadsheet formula interpretation. Do not open untrusted raw CSV exports in spreadsheet software. For exact backup and restoration, prefer the verified SQLite backup rather than a spreadsheet file.

There is no universal CSV sanitization that is guaranteed across every spreadsheet application and every save/re-open workflow. The selected prefixes follow OWASP's documented mitigation guidance, but the actual target LibreOffice Calc version and any Excel workflow must still be manually checked. Do not describe the mitigation as application-level protection or a guarantee against all spreadsheet parsers.

## Safety and validation

- Backup uses SQLite's online backup API, then runs integrity and foreign-key checks.
- Restore verifies the source and writes a separate new file. It never replaces the active database.
- CSV export takes one read snapshot and writes RFC 4180 CSVs plus a manifest.
- Existing output files and directories are not overwritten.
- On Unix, backup files and export files are owner-readable/writable only (0600); export directories are owner-only (0700).
- Failed operations remove incomplete outputs.
- Export covers habit, pause, tag, goal, milestone, routine-template and routine-run tables when they exist.
- A routine migration ledger is checked when present; verified older databases without routines remain supported.
- Regression tests cover formula prefixes, leading whitespace/control characters, full-width prefix variants, embedded CSV quotes/separators and the lossless raw mode.

## Scope boundary

No cloud upload or service call occurs. The user chooses the destination and remains responsible for the exported files. A GTK backup dialog, scheduled backups, encryption at rest for external media, retention policy and restore UI are still future work.

Manual release acceptance must open a synthetic export in the supported Linux Mint LibreOffice Calc build and verify that formula-looking values remain text. Test save/re-open behavior as well; do not use real personal habit records for this test.
