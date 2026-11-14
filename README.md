# Habit Trackers by Madhab

A privacy-first, offline-first habit and personal-goal tracker for Linux Mint. The product is being built as a native Rust + GTK 4 desktop application with Bengali and English interfaces, flexible habit schedules, measurable goals, transparent progress scoring and user-owned data.

> **Development status — 14 November 2026:** the application is under active development on a feature branch. The basic Today workflow and a first pause/resume slice exist, but this is not yet a complete app or a verified Linux Mint release.

## Product decisions

- **Platform:** Linux Mint desktop first.
- **Design:** modern hybrid native desktop UI.
- **Languages:** Bengali and English; localization work remains.
- **Scoring:** show adherence, habit-strength estimate and streak separately.
- **Storage:** local-first SQLite; no cloud account or network dependency for core tracking.
- **Privacy:** no telemetry or remote AI in the MVP.

## Implemented prototype

The current feature branch includes:

- GTK 4 Today screen.
- Add a daily binary habit.
- Mark today's habit complete and undo it.
- Rename active habits without changing history.
- Archive and restore habits from a dedicated Archived view; archival preserves check-ins and pause history.
- Pause and resume a habit; retain pause history in SQLite.
- Prevent check-ins while a habit is paused.
- Versioned SQLite schema migrations (v1 and v2).
- Pure Rust date, schedule and scoring primitives.
- Python SQLite schema tests, Rust tests and GitHub Actions checks.
- Read-only Linux Mint environment audit script.
- Local data portability tool: CSV bundle export, verified SQLite online backup, backup verification and restore to a separate new database file.

The current GTK UI still creates daily binary habits only. The data-portability commands are currently provided by a separate CLI companion, not a GTK dialog. The pure schedule/scoring engine is not yet integrated into the GTK forms, persisted schedule evaluation or analytics.

## Roadmap

1. **M3 — Habit management:** active-habit rename and reversible archive/restore are implemented in the foundation branch; numeric/count/duration/limit/frequency forms, flexible schedules and distinct daily states remain.
2. **M4 — Scoring:** integrate adherence, habit strength and streaks with persisted, versioned history.
3. **M5 — Productivity:** calendar, heatmap, summaries, goals, routines, reviews and reminders.
4. **M6 — Trust and accessibility:** Bengali/English gettext, keyboard and screen-reader review, CSV export, consistent backup/restore and privacy hardening.
5. **M7 — Release:** validate on a real supported Linux Mint machine, then package and test install/update/uninstall.

See [Engineering Status](docs/ENGINEERING_STATUS.md), [Engineering Blueprint](docs/ENGINEERING_BLUEPRINT.md), [M2 Prototype Scope](docs/M2_PROTOTYPE.md) and [Data Portability](docs/DATA_PORTABILITY.md) for detailed acceptance criteria and validation boundaries.

## Build on Linux Mint

Install GTK 4 development prerequisites and the pinned Rust toolchain specified in `rust-toolchain.toml`, then run:

~~~bash
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo run
~~~

### Local export and backup

The companion tool writes only to an explicit destination and refuses to overwrite existing files:

~~~bash
# Full CSV bundle
cargo run --bin madhab-data-tool -- export-csv "$HOME/Documents/Madhab-Habit-Export"

# Verified SQLite backup
cargo run --bin madhab-data-tool -- backup "$HOME/Documents/madhab-habits-backup.sqlite3"

# Verify a backup, then restore it to a separate new file
cargo run --bin madhab-data-tool -- verify "$HOME/Documents/madhab-habits-backup.sqlite3"
cargo run --bin madhab-data-tool -- restore "$HOME/Documents/madhab-habits-backup.sqlite3" "$HOME/Documents/madhab-habits-restored.sqlite3"
~~~

See [Data Portability](docs/DATA_PORTABILITY.md) for the file format and safety behavior.

Run the read-only environment audit:

~~~bash
bash scripts/audit-linux-mint.sh | tee madhab-habit-environment-audit.txt
~~~

The audit script does not install packages or change system settings. Review its output and remove personal/home-path details before sharing it.

## Tests

~~~bash
python3 -m unittest discover -s tests -p 'test_*.py' -v
bash -n scripts/audit-linux-mint.sh
~~~

## Contribution and license

The current implementation is on [Draft PR #1](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/pull/1). It must remain draft until current CI and target-machine validation are complete.

No license has been selected. Do not assume public visibility means permission to reuse or redistribute code. A license decision is required before inviting external contributions.
