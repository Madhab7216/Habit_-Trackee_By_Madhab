# Habit Trackers by Madhab — Engineering Status

Audit date: 2026-11-14

## Live GitHub state

- Repository: `Madhab7216/Habit_-Trackee_By_Madhab`
- Default branch: `main`; it remains the original README-only baseline.
- Foundation branch: `work/linux-mint-habit-tracker-foundation` with [Draft PR #1](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/pull/1).
- M3 schedule follow-up: `work/m3-schedule-aware-checkins` with [Draft PR #5](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/pull/5), targeting the foundation branch.
- Both PRs remain unmerged; `main` is unchanged.
- [Issue #2](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/issues/2): target Linux Mint workstation validation — open.
- [Issue #3](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/issues/3): remaining advanced workflows — open.

Keep PR #1 in draft. Do not merge until the latest Rust checks and the actual Linux Mint workstation validation pass.

## Implemented and present in the feature branch

- Native Rust + GTK 4 Linux desktop starter.
- Local SQLite persistence under the user's XDG data directory / home data directory.
- Versioned migrations: initial schema (v1) and pause-period history (v2).
- Initial habit types and schedule fields in the schema: binary, numeric, duration, count, limit and frequency; daily, selected weekdays, every-N-days and times-per-week.
- Pure Rust domain rules for civil dates, schedule evaluation, adherence, EMA habit-strength estimate and streak calculation.
- Today screen: add a daily binary habit, rename it, mark complete, undo, pause, resume, archive and restore it from a dedicated Archived view.
- M3 schedule follow-up: evaluate persisted daily, selected-weekday, every-N-days and times-per-week definitions with start/end bounds; show Due today / Not scheduled states and enforce due-date rules in the storage layer.
- Reversible archive and restore operations preserve existing check-ins and pause history; database tests cover rename, archive/restore, and archived check-in protection.
- Pause history is retained; only one open pause per habit is allowed; check-ins are blocked while paused.
- Unit and schema tests, GitHub Actions workflow, and read-only Linux Mint environment audit script.
- Product blueprint, scope decisions and prototype-boundary documentation.

## Important boundary: implemented is not the same as integrated

Persisted schedule evaluation is now connected to the Today view and check-in storage guard in Draft PR #5. The UI still creates daily binary habits only: a GTK schedule editor, numeric/duration/count/limit/frequency forms, partial/skipped workflows and weekly quota progress remain open. The scoring engine is not yet integrated with persisted history or analytics. Habit rename and reversible archive/restore are implemented; permanent deletion remains deferred.

The current pause/resume implementation is a first M3 slice, not a finished habit-management system.

## CI status

Latest completed validation: [GitHub Actions run #32](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/actions/runs/43595801293), on feature-branch head `94e15ddbbd7acc7fc6a8aad93a2732f8be4bb3d3` — all jobs passed:
- `cargo fmt --all -- --check`: passed.
- `cargo check --all-targets`: passed.
- `cargo test --all-targets`: passed, including the added lifecycle regression tests.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Python SQLite schema tests: passed.
- `bash -n scripts/audit-linux-mint.sh`: passed.

This is hosted Ubuntu 24.04 CI evidence, not a substitute for the actual target Linux Mint workstation launch/persistence smoke test in Issue #2. The schedule follow-up changes Rust source and must pass its own latest-head CI before review. The branch must be revalidated after any later code change.

## Remaining engineering work

### M3 — Complete habit management and measurement
1. Active-habit rename and reversible archive/restore are implemented. Permanent deletion remains deferred; any future deletion must include an explicit export/confirmation policy.
2. Binary, numeric/count, duration, limit/avoid and frequency-goal check-ins with unit and direction validation.
3. Persisted schedule evaluation and due/unscheduled check-in guards are implemented in Draft PR #5; GTK schedule editing, explicit timezone selection and weekly quota progress remain open.
4. Distinguish completed, partial, skipped, missed, paused and not-scheduled periods.
5. Add migration and integration tests for every workflow; the new schedule integration tests await the latest-head Rust CI result.

### M4 — Scoring integration
1. Feed persisted eligible periods to the scoring engine.
2. Show adherence, habit-strength estimate and streak separately, with clear explanations.
3. Version scoring and test deterministic history replay, schedule changes and pauses.
4. Ensure lower-is-better limit habits never reward higher consumption.

### M5 — Productivity and analytics
- Calendar / heatmap, 7-day and monthly summaries, trends and target progress.
- Link habits to long-term goals; add routines, milestones and review notes.
- User-controlled reminders, quiet hours and notification preferences.

### M6 — Data ownership, privacy, localization and accessibility
- Bengali and English gettext catalogs for every shipped screen.
- Keyboard navigation, accessible names, focus order, scalable text, contrast and screen-reader review.
- User-initiated CSV export and consistent SQLite backup/restore.
- Protect local data and keep habit names/notes out of logs. No telemetry or cloud requirement for core tracking.

### M7 — Release engineering
- Read-only audit and build/launch/persistence smoke test on the actual supported Linux Mint machine (Issue #2).
- Package, install, update and uninstall verification; document supported Linux Mint versions.
- Add dependency/security review and release checklist.
- Select and add a license before accepting outside contributions.

## Product research baseline

- [Loop Habit Tracker](https://github.com/iSoron/uhabits): a strong open-source reference for flexible schedules, reminders, widgets, history, CSV/SQLite export and offline privacy.
- [OpenHabitTracker](https://openhabittracker.net/): a useful reference for calendar/history/statistical charts, target progress and daily summary reminders.
- [GTK 4 accessibility guidance](https://docs.gtk.org/gtk4/section-accessibility.html): prefer accessible GTK widgets and provide labels/descriptions for custom UI.
- [GTK Rust book](https://gtk-rs.org/gtk4-rs/stable/latest/book/): use gettext-based localization for Bengali and English.
- [SQLite Online Backup API](https://sqlite.org/backup.html): use a consistent database snapshot rather than copying a live SQLite file.
- [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/latest/): keep user data in an absolute XDG data directory (default `$HOME/.local/share`).

## Privacy and safety boundaries

- This app is separate from Madhab OS.
- MVP core functions stay offline and local-only; no remote AI, cloud sync or telemetry.
- Habit labels, notes and history can be sensitive. Never upload the user's live database or real habit history into issues, PR comments or CI artifacts.
- Do not merge into `main` based only on schema-test success.
- The repository has no selected license. Public visibility alone does not grant permission to reuse or redistribute the code.
