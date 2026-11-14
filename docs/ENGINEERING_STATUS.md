# Habit Trackers by Madhab — Engineering Status

Audit date: 2026-11-14

## Repository state

- Repository: `Madhab7216/Habit_-Trackee_By_Madhab`
- Default branch: `main`; kept unchanged.
- Primary implementation branch: `work/linux-mint-habit-tracker-foundation`
- Primary pull request: [#1 — M0–M3 foundation: GTK4 habit tracker, scheduling, scoring and pause/resume](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/pull/1), open and draft.
- Stacked work branches/PRs for M3 and M5 retain their own history; their implementation is being consolidated into PR #1. No active M3/M5 branch has been force-pushed.
- Duplicate M6 draft PR #6 was closed as superseded after the consolidated foundation gained the newer portability implementation (including milestone export).
- M7 desktop launch smoke test is isolated in draft PR #9.
- Target-machine validation issue: [#2 — validate on the target Linux Mint workstation](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/issues/2).

## Implemented in the current feature increment

### M7 — virtual desktop launch smoke test (PR #9)
- Builds the GTK application and launches it in an Xvfb virtual display.
- Verifies the expected main window appears.
- Uses a temporary XDG data directory and confirms the application creates a valid SQLite database there.
- Deletes temporary test data after completion; no personal user records are accessed or uploaded.


### M2 — native foundation
- Rust + GTK 4 application and local SQLite repository.
- Versioned schema (v1 base, v2 pause-period migration).
- Foreign keys, indexes, unique habit/date entries and constrained status/type values.
- Local data directory via absolute XDG/HOME resolution.
- Read-only Linux Mint environment audit.
- GitHub Actions for format, compilation, Rust tests, Clippy, schema tests and shell syntax.

### M3 — schedule-aware habits
- Persisted daily, selected-weekday, every-N-days and weekly-frequency cadence.
- Today view can distinguish scheduled/not-scheduled, completed, paused, partial and skipped items.
- Binary completion/undo and explicit skip actions.
- Database validation rejects invalid check-ins for archived, paused or unscheduled dates.
- Pause/resume history is retained; pause start is inclusive and resume date exclusive.
- Habit rename/edit, archive with history preservation and restore.
- Weekly frequency quota is bounded to 1–7 sessions; same-day frequency check-ins accumulate.

### M4 — scoring foundation
- Deterministic Gregorian civil date and schedule-period handling.
- At-least and at-most measurement ratio functions.
- Adherence, EMA habit-strength estimate and current streak as distinct measures.
- Closed missed periods contribute zero; skipped and fully paused periods are excluded.
- An open current day/week is not counted as missed early.
- Weekly frequency targets are prorated when some days of that week are paused.
- Full behavior contract in [SCORING_POLICY.md](SCORING_POLICY.md).

### M5 — Goals & Milestones
- Additive goal-milestone table migration; existing goals and habit links remain.
- Goal create/edit/pause/resume/complete/archive/restore workflows.
- Milestone create, complete, reopen and skip workflows.
- Progress uses completed milestones divided by non-skipped milestones; skipped count remains visible.
- Separate goal migration ledger avoids collision with the habit schema version.

### M6 — local data portability
- CLI commands for verified SQLite backup, backup verification, separate-file restore and CSV bundle export.
- Online SQLite backup API, integrity and foreign-key checks.
- Explicit destination paths; existing files/directories are never overwritten.
- Unix backup/output permissions are owner-only.
- Restore never replaces the live app database.
- Export is a read-transaction snapshot and includes the milestones table when present.

## Validation boundary

Validation results observed on 14 November 2026:

- Foundation head `d8e17b60c39908451088f375ba3a553e057c7faf`: Engineering Checks run 43597541792 passed; Data Portability checks run 43597541806 passed.
- M7 head `4c32302b91ad31f8f9f232f47d0ce41616ca9c58`: Linux desktop smoke run 43616211122 passed, including binary build, Xvfb window detection and isolated SQLite integrity check.
- The earlier standalone M6 PR run failed its formatting gate; the consolidated Foundation version is formatted and passed its latest checks.

These results are specific to the listed commit SHAs. Repeat CI on any newer PR #1 head before release. The actual GTK application still needs to be launched and exercised on the target Linux Mint workstation (Issue #2); a virtual-display CI run is not a substitute.

## Remaining v1.0 blockers

1. Verify newest-head GitHub Actions results.
2. Complete target Linux Mint install/build/launch and upgrade tests.
3. Add a full schedule editor and effective-date versioning/confirmation so schedule changes do not silently reinterpret past periods.
4. Finish Bengali/English localization and keyboard/screen-reader accessibility.
5. Add goal-to-task/routine workflows, weekly reviews and richer calendar analytics.
6. Add a GTK export/backup dialog, retention guidance and recovery UX around the CLI.
7. Implement IANA timezone selection and DST/travel-safe scheduling.
8. Test package installation, updates and uninstall.
9. Choose and add an explicit open-source license before accepting outside contributions.

## Privacy boundaries

- Separate from Madhab OS.
- Core use is local and offline. No telemetry, cloud sync or remote AI is included.
- Habit names and notes may be sensitive. Do not attach live SQLite files or personal records to public issues/PRs.
- PR #1 remains draft until current CI and target-machine validation are reviewed.
