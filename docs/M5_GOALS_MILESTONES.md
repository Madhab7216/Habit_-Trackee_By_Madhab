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

## Research rationale

The feature emphasises short, observable milestones and a visible progress record rather than a single distant target. A meta-analysis of 138 experimental studies (19,951 participants) found that interventions that increased goal-progress monitoring also improved goal attainment on average; the reported pooled effect on attainment was d+ = 0.40. A separate meta-analysis of 141 papers found a small positive effect for goal setting (d = 0.34). These findings support explicit milestones and visible progress, but they do not prove that this specific UI will produce the same effects for every user.

- Harkin et al., *Does Monitoring Goal Progress Promote Goal Attainment?*, Psychological Bulletin (2016): https://pubmed.ncbi.nlm.nih.gov/26479070/
- Epton et al., *Unique Effects of Setting Goals on Behavior Change*, Journal of Consulting and Clinical Psychology (2017): https://pubmed.ncbi.nlm.nih.gov/29189034/

The date guard uses SQLite `strftime('%Y-%m-%d', ...)` plus an explicit non-NULL test. SQLite documents that date/time functions return NULL for unsupported or invalid time-values; the Rust layer additionally validates Gregorian leap days and month lengths before persistence: https://www.sqlite.org/lang_datefunc.html. The progress widget uses GTK's native `ProgressBar`, which exposes progress values through GTK's accessibility role/properties: https://gtk-rs.org/gtk4-rs/stable/latest/docs/gtk4/struct.ProgressBar.html.
