# Engineering implementation and validation boundary — M2 to M6

## M2 — native foundation
- Rust + GTK 4 desktop app.
- Local SQLite with v1 base schema and v2 pause-period migration.
- Foreign keys, indexes, unique daily entries and data constraints.
- Absolute XDG/HOME data-directory checks.
- Read-only Linux Mint hardware/toolchain audit.

## M3 — schedule-aware habit workflows
- Persisted daily, weekday, every-N-days and weekly-frequency schedules.
- Today view distinguishes due and not-scheduled habits.
- Binary check-in/undo, skip, edit, pause/resume, archive and restore.
- Check-in validation at the DB layer.
- Pause history is retained and excluded from due/score behavior.

## M4 — scoring foundation
- Gregorian civil date conversion and deterministic period boundaries.
- At-least/at-most achievement normalization.
- Adherence, EMA habit-strength estimate and streak.
- Closed missed days count as zero; skipped/paused periods are omitted.
- Current open period is not scored prematurely.

## M5 — Goals and Milestones
- Additive `0003_goal_milestones.sql` migration.
- Existing goals and habit links remain intact.
- Goal lifecycle and milestone create/complete/reopen/skip.
- Visible progress based on completed vs non-skipped milestone count.
- Independent goal migration ledger.

## M6 — local data portability
- CLI for verified backup, verify, restore to a new file and complete CSV bundle export.
- SQLite online backup plus integrity/FK validation.
- Refuse existing destinations; Unix outputs are owner-only.
- Restore never replaces the running app database.

## Validation required before release
- Latest combined PR head must pass Rust formatting, compile, tests, Clippy, Python SQLite tests and shell validation.
- Actual launch and manual workflow validation on the supported Linux Mint workstation.
- Schedule editor/effective-date history, localization, accessibility, timezone/DST, packaging and license remain outstanding.
