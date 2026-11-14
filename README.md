# Habit Trackers by Madhab

A privacy-first, offline-first native habit and personal productivity app for Linux Mint. Built with **Rust, GTK 4 and SQLite**. Personal records stay on the device; the MVP does not require an account, cloud sync, telemetry, ads or remote AI.

> **Engineering preview (14 November 2026):** M2 foundation with M3 schedule-aware habit workflows, M5 Goals & Milestones, and M6 local data portability on the feature branch. This is not a stable v1.0 release.

## Product decisions

- **First platform:** Linux Mint desktop.
- **Interface:** native GTK 4; focused Today view plus deeper management and goals.
- **Languages:** Bengali and English are planned; complete localization remains outstanding.
- **Storage:** local-first SQLite, versioned migrations and user-controlled data.
- **Scoring:** transparent adherence, habit-strength estimate and current streak, displayed separately.
- **Privacy:** no background upload or telemetry.

## Current feature-branch capabilities

### Habit workflows (M2–M4 foundation)
- Add daily binary habits.
- Select a recurrence: every day, Monday–Friday, every two days, or a weekly frequency target (1–7 sessions).
- Mark a binary habit done/undo and intentionally skip an eligible date.
- View scheduled, not-scheduled, completed, skipped and paused states.
- Edit a habit name, pause/resume, archive with confirmation and restore it without deleting history.
- Date and schedule rules are enforced in the data layer, not only by disabled UI controls.
- The domain layer includes higher-is-better/at-least and lower-is-better/at-most measurement normalization and separate adherence, EMA habit-strength and streak metrics.

### Persisted progress scoring (M4)
- Open **Progress & scores** from the Today window.
- Rebuilds adherence, EMA habit-strength estimate and current streak from local saved check-ins and pause history.
- Displays scoring version, closed/scored periods, missed, skipped and paused counts.
- Does not count an open current day/week as missed.
- Handles at-least and at-most measures; lower-is-better limits never reward higher consumption.
- Weekly-frequency sessions accumulate toward the quota; a partially paused week has a proportionally reduced target.
- Scores are derived on demand from the source records; no score cache, network request or telemetry is introduced.

### Goals and milestones (M5)
- Create, edit, pause/resume, complete, archive and restore goals.
- Add dated or undated milestones.
- Complete, reopen or skip milestones.
- See milestone progress and habit links.
- Skipped milestones are excluded from the progress denominator and displayed separately.
- Existing goal and habit-link records are preserved by an additive milestone migration.

### Local data portability (M6)
Build the companion CLI with the application:

```bash
cargo run --bin madhab-data-tool -- backup "$HOME/Documents/madhab-habits-backup.sqlite3"
cargo run --bin madhab-data-tool -- verify "$HOME/Documents/madhab-habits-backup.sqlite3"
cargo run --bin madhab-data-tool -- restore   "$HOME/Documents/madhab-habits-backup.sqlite3"   "$HOME/Documents/madhab-habits-restored.sqlite3"
cargo run --bin madhab-data-tool -- export-csv "$HOME/Documents/Madhab-Habit-Export"
```

Backup uses SQLite's online backup API and integrity/foreign-key checks. Restore always writes a separate new database; it never replaces the active database. CSV export writes a consistent snapshot of the supported user-data tables. Explicit destinations must not already exist. All operations are local.

## Linux Mint build

Install build prerequisites and GTK 4 development files:

```bash
sudo apt update
sudo apt install build-essential pkg-config libgtk-4-dev
```

The Rust toolchain is pinned in `rust-toolchain.toml`. Run the read-only environment audit:

```bash
bash scripts/audit-linux-mint.sh
```

Then build, test, lint and launch:

```bash
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo run
```

The audit script does not install packages, invoke sudo, modify system settings or access the network.

## Local data and safety

The application database is stored at:

`$XDG_DATA_HOME/madhab-habit-tracker/habit-tracker.sqlite3`

Fallback:

`~/.local/share/madhab-habit-tracker/habit-tracker.sqlite3`

If neither path is an absolute usable location, the app refuses to place personal data in the current working directory. Backup and CSV outputs may contain intimate habit names and notes; keep them private and do not commit them.

## Engineering notes

- [Engineering blueprint](docs/ENGINEERING_BLUEPRINT.md)
- [Engineering status and release blockers](docs/ENGINEERING_STATUS.md)
- [M3 schedule integration](docs/M3_SCHEDULE_INTEGRATION.md)
- [Goals and milestones](docs/M5_GOALS_MILESTONES.md)
- [Scoring policy](docs/SCORING_POLICY.md)
- [Data portability](docs/DATA_PORTABILITY.md)

## Release blockers

- Fresh GitHub Actions run must pass on the latest pull-request head.
- Launch and exercise the app on the actual supported Linux Mint workstation; CI compile is not a substitute.
- Complete Bengali/English localization, keyboard/screen-reader accessibility, timezone/DST behavior and packaging.
- Add effective-dated schedule history or explicit confirmation before schedule edits change historical scoring.
- Complete and validate the M4 persisted scoring PR on its latest head; hosted CI does not replace a target Linux Mint GUI test.
- Choose and add an explicit open-source license before accepting external contributions.

The primary feature branch is tracked by open Draft PR #1. It has not been merged into `main`.
