# Habit Trackers by Madhab

A privacy-first, offline-first habit and personal-goal tracker for Linux Mint. The product is being built as a native Rust + GTK 4 desktop application with Bengali and English interfaces, flexible habit schedules, measurable goals, transparent progress scoring and user-owned data.

> **Development status — 14 November 2026:** the application is under active development on isolated feature branches. The basic Today workflow, pause/resume, rename, reversible archive/restore and first schedule-aware check-in integration exist, but this is not yet a complete app or a verified Linux Mint release.

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
- Prevent check-ins while a habit is paused or the persisted schedule says the date is not due.
- Display due, not-scheduled, completed and paused states separately in Today.
- Evaluate persisted daily, selected-weekday, every-N-days and times-per-week schedules with start/end date bounds.
- Versioned SQLite schema migrations (v1 and v2).
- Pure Rust date, schedule and scoring primitives.
- Python SQLite schema tests, Rust tests and GitHub Actions checks.
- Read-only Linux Mint environment audit script.

The current UI still creates daily binary habits only: the GTK schedule editor and numeric, duration, count, limit/avoid and frequency entry forms remain open work. Persisted schedule definitions now drive Today due/not-scheduled states and are enforced by the storage layer. The scoring engine is still not integrated with persisted history or analytics.

## Roadmap

1. **M3 — Habit management:** active-habit rename and reversible archive/restore are implemented in the foundation branch; persisted schedule evaluation and due/not-scheduled check-in guards are in the schedule follow-up. GTK schedule editing, numeric/count/duration/limit/frequency forms and partial/skipped entry flows remain.
2. **M4 — Scoring:** integrate adherence, habit strength and streaks with persisted, versioned history.
3. **M5 — Productivity:** calendar, heatmap, summaries, goals, routines, reviews and reminders.
4. **M6 — Trust and accessibility:** Bengali/English gettext, keyboard and screen-reader review, CSV export, consistent backup/restore and privacy hardening.
5. **M7 — Release:** validate on a real supported Linux Mint machine, then package and test install/update/uninstall.

See [Engineering Status](docs/ENGINEERING_STATUS.md), [Engineering Blueprint](docs/ENGINEERING_BLUEPRINT.md), [M2 Prototype Scope](docs/M2_PROTOTYPE.md) and [M3 Schedule Integration](docs/M3_SCHEDULE_INTEGRATION.md) for detailed acceptance criteria and validation boundaries.

## Build on Linux Mint

Install GTK 4 development prerequisites and the pinned Rust toolchain specified in `rust-toolchain.toml`, then run:

~~~bash
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo run
~~~

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

The foundation is tracked in [Draft PR #1](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/pull/1); schedule integration is proposed separately in [Draft PR #5](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/pull/5). Keep both draft until the relevant CI and target-machine validation gates are complete.

No license has been selected. Do not assume public visibility means permission to reuse or redistribute code. A license decision is required before inviting external contributions.
