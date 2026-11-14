# Habit Trackers by Madhab — Engineering Blueprint

## Product scope
A native Linux Mint desktop application for habit tracking, long-term goals, routines, and transparent personal progress. It is not part of Madhab OS and has no cloud, account, ads, or telemetry requirement for its first release.

## Frozen product decisions
- First platform: Linux Mint desktop.
- UI direction: hybrid native desktop interaction with a modern, dashboard-led interface.
- Languages: Bengali and English.
- Storage: local-first SQLite.
- Scoring: hybrid model with adherence, habit-strength estimate, and streak displayed independently.
- First release: reliable offline tracking before analytics and automation.

## Habit types
Binary, numeric target, duration, count, limit/avoid, and frequency-goal habits. A measurable habit requires an explicit unit and direction (higher-is-better or lower-is-better).

## Scheduling and check-in states
Daily, selected weekdays, every-N-days, and N-times-per-week schedules. Entries distinguish completed, partial, and skipped; missed is a derived state after an eligible period closes. Paused and not-scheduled periods are excluded from failure calculations. A unique habit/date constraint prevents duplicate daily entries.

## Hybrid scoring v1 proposal
1. Target adherence: arithmetic mean of achievement ratios for eligible closed periods.
2. Habit strength estimate: exponential smoothing, alpha 0.25. First eligible period initializes to its achievement; skipped and fully paused periods are excluded; missed periods contribute zero.
3. Streak: consecutive eligible schedule periods meeting the target; skips and unscheduled periods do not break a streak.

These are transparent product metrics, not clinical or psychological measurements. Keep scoring versioned and test deterministic replay before release.

## Architecture
- UI: GTK 4 using gtk-rs.
- Application/domain: Rust, with rules independent from GTK callbacks.
- Storage: SQLite via rusqlite; parameterized SQL, foreign keys on every connection, versioned migrations, transactions.
- Localization: gettext with Bengali and English catalogs.
- Packaging: decide after install/update testing on the target Linux Mint release.

## Privacy and safety
All essential features work offline. Habit labels and notes can be intimate; do not include them in diagnostics by default. No remote AI, cloud sync, or analytics SDK in MVP. Export and backup are user initiated. Any future assistant needs explicit scoped permission and must not silently transmit or alter habit records.

## MVP acceptance criteria
- Create, edit, pause/resume, archive, and complete habits.
- Daily and flexible schedules behave deterministically.
- Partial, skipped, missed, and paused opportunities remain distinct.
- Data survives restart and supports export plus verified backup/restore.
- Bengali/English and keyboard/screen-reader access cover all shipped screens.
- CI passes formatting, compilation, Rust tests, SQLite invariants, and shell checks.
- Install and update succeed on a real supported Linux Mint machine.

## Roadmap
- M0: requirements freeze and threat model.
- M1: Linux Mint environment audit and Rust/GTK compile smoke test.
- M2: app shell, SQLite migrations, basic daily tracking.
- M3: habit CRUD, measurement types, flexible schedules, pause and skip.
- M4: scoring engine and deterministic edge-case tests.
- M5: calendar, analytics, goals, routines, weekly review.
- M6: localization, accessibility, export, verified backup/restore, security review.
- M7: package, install/update test, release candidate.

## Research baseline (checked 14 November 2026)
- Rust 1.100.0 stable announcement, 12 November 2026: https://blog.rust-lang.org/2026/11/12/Rust-1.100.0/
- GTK 4 Rust bindings 0.11.5 document Rust 1.83 as their minimum supported compiler: https://docs.rs/crate/gtk4/0.11.5
- rusqlite 0.42.0 release, 11 November 2026: https://github.com/rusqlite/rusqlite/releases
- SQLite backup guidance: https://sqlite.org/backup.html
- GTK Rust guide: https://gtk-rs.org/gtk4-rs/stable/latest/book/
- Loop Habit Tracker reference implementation: https://github.com/iSoron/uhabits

## Scope and licensing warning
The repository was README-only when first audited. This feature branch adds an initial app slice, not a complete application. The repository currently has no selected open-source license; public visibility alone does not grant a reuse license. Choose a license before inviting external contributions.
