# M5 — Goals and Milestones

## Scope

This slice adds a local-first goals workspace to the GTK4 application. It reuses the existing `goals` and `goal_habits` tables and adds one additive migration for milestone records. It does not change habit scheduling, check-in logic, scoring, backup/export, or the main habit lifecycle implementation.

## User workflows

- Create a goal with a title and optional target date.
- Edit the title, short description, and target date.
- Pause, resume, manually complete, archive, and restore goals.
- Add dated or undated milestones to a non-archived goal.
- Mark milestones complete, reopen them, or skip them.
- View milestone progress and the number of habits linked to each goal.
- Link/unlink a habit to a goal through the repository API. The UI does not yet expose habit linking.

## Progress policy

Goal progress is derived from milestones only:

`eligible = total_milestones - skipped_milestones`

`progress = completed_milestones / eligible_milestones × 100`

Skipped milestones are excluded from the denominator and displayed separately. A goal with no eligible milestones displays 0%, rather than claiming completion. Goal status is separate from milestone progress: the user may mark a goal completed explicitly.

## Data and migration safety

- Migration `0003_goal_milestones.sql` is additive; existing goals and habit links remain intact.
- A separate `goal_schema_migrations` table owns this module's migration version so it does not conflict with the foundation branch's habit-schema migration counter.
- Milestones reference goals with `ON DELETE CASCADE`; milestones are not permanently deleted through the UI.
- Status values are constrained to pending, completed, and skipped.
- Target/due dates are validated in Rust as real Gregorian dates in `YYYY-MM-DD` format and constrained again in SQLite using `strftime`.
- All SQL values are bound parameters. No cloud service, telemetry, or network dependency is introduced.

## Test coverage

Rust tests cover date validation, title limits, progress math, skipped milestones, reopen, archive/restore, milestone creation restrictions, habit linking, and idempotent schema migration. Python SQLite tests cover database constraints, foreign keys, status validation, real-date validation, and additive migration behaviour.

## Deliberately deferred

- Goals dashboard analytics / historical charts.
- Task and routine builder.
- Reminders and quiet hours.
- Automatic habit-completion contribution to goal progress (requires the separate M4 scoring integration and an explicit, versioned contribution policy).
- Bengali/English localization of this new window.
- Permanent goal/milestone deletion.
