# Habit Trackers by Madhab — Engineering Status

Audit date: 2026-11-14

## Repository state

- Repository: `Madhab7216/Habit_-Trackee_By_Madhab`
- Default branch: `main`; kept unchanged.
- Primary implementation branch: `work/linux-mint-habit-tracker-foundation`
- Primary pull request: [#1 — M0–M3 foundation: GTK4 habit tracker, scheduling, scoring and pause/resume](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/pull/1), open and draft.
- The M3 schedule integration, M5 Goals & Milestones and M6 local portability increments are present on the current foundation feature branch.
- Separate M4 persisted scoring work is proposed on `work/m4-persisted-scoring` with [Draft PR #12](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/pull/12); it is stacked on the foundation and does not edit the M5 goals or M6 portability modules.
- Target-machine validation issue: [#2 — validate on the target Linux Mint workstation](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/issues/2).

## Implemented in the current feature increment

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

### M4 — persisted scoring v1 (separate M4 branch)
- Connect the existing domain achievement, adherence, EMA and streak functions to SQLite `habit_entries` and `habit_pause_periods`.
- Read-only deterministic replay via `scoring::summaries_for_date(connection, as_of)`; Today uses `summaries_for_today`.
- Display separate adherence, EMA habit-strength estimate and current streak in the GTK **Progress & scores** workspace.
- Score version is shown with closed/scored, missed, skipped and paused period counts.
- Closed missed periods contribute zero; daily skips and fully paused periods are excluded; current open day/week is not counted as missed.
- Weekly frequency sessions aggregate toward the quota and partial-week pauses prorate the target.
- Replay rebuilds from source records on demand; no stale score cache, network request or telemetry is added.
- Implementation and test boundaries: [M4 persisted scoring](M4_PERSISTED_SCORING.md); behavior contract: [SCORING_POLICY.md](SCORING_POLICY.md). [Draft PR #12](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/pull/12) passes exact-head hosted CI: 87 Rust tests plus formatting, build, Clippy, schema and data-portability checks. Target Linux Mint GUI validation remains open.

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

M4 persisted scoring has passed exact-head hosted CI on commit `ea3f27b3cc59358c2e792dadcfb437a09c6d5cb8`:
- Engineering Checks push run [#82](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/actions/runs/43617899103): format, compile, Rust tests (87 passed), Clippy, schema tests and audit-script syntax all passed.
- Engineering Checks PR run [#83](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/actions/runs/43617903711): all three jobs passed.
- Data Portability Checks PR run [#29](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/actions/runs/43617903718): backup, restore and CSV export checks passed.

These hosted Ubuntu checks do not replace the actual GTK application launch, persistence and manual interaction test on the target Linux Mint workstation (Issue #2). PR #1 also needs validation on its own latest head.

## Remaining v1.0 blockers

1. Complete exact-head CI on PR #1; separate M4 persisted scoring CI is passing on its latest PR head.
2. Complete target Linux Mint install/build/launch and upgrade tests.
3. Add effective-dated schedule versioning/confirmation so schedule edits do not silently reinterpret past scoring; the recurrence editor still needs a complete editing workflow.
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
