# M5 — Goals and Milestones

## Scope

A local-first Goals workspace integrated into the GTK4 desktop app. It reuses the existing `goals` and `goal_habits` tables and adds one additive migration for milestone records. It does not change habit schedule/check-in logic.

## User workflows

- Create a goal with title and optional target date.
- Edit title, description and target date.
- Pause, resume, complete, archive and restore a goal.
- Add dated or undated milestones.
- Mark a milestone complete, reopen it, or skip it.
- View milestone progress and linked habit count.
- Link/unlink habits through the repository API; the GTK window does not yet expose habit linking.

## Progress policy

`eligible = total_milestones - skipped_milestones`

`progress = completed_milestones / eligible_milestones × 100`

Skipped milestones are excluded from the denominator but remain visible. A goal with no eligible milestones shows 0%, not 100%. Goal status is separate from progress; the user explicitly completes the goal.

## Migration and safety

- `0003_goal_milestones.sql` is additive; existing goals and links remain.
- A separate `goal_schema_migrations` ledger avoids conflict with the habit schema version.
- Milestones reference goals with `ON DELETE CASCADE`.
- Status is constrained to pending/completed/skipped.
- Dates are validated in Rust and again in SQLite.
- SQL values use bound parameters.
- Data remains local and offline.

## Tests and deferred work

Rust tests cover validation, progress, skipped milestones, reopen, archive/restore, links and idempotent migration. Python tests check SQLite constraints and migration preservation.

Goal analytics/history charts, task/routine builder, reminders, automatic scoring contributions, Bengali/English localization and permanent deletion remain deferred. Automatic habit-to-goal contributions need a separate explicit and versioned policy.
