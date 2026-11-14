# M5 — Routine Builder

## Product goal

Routines provide reusable, ordered checklists for repeated parts of a user's day: morning preparation, study blocks, work shutdown, exercise warm-up or evening reflection. They sit alongside habits and goals but do not replace either system.

## Supported template capabilities

- Create, edit, pause, resume and archive routines.
- Choose an every-day or selected-weekday schedule, with a start date and optional end date.
- Add 1–40 ordered steps.
- Each step can be a free-form action or an organizational link to an existing non-archived habit.
- Mark steps required or optional.
- Move steps up/down and remove steps while editing.
- Run a routine once per local date; opening it again resumes the same run.
- Mark each run item completed, skipped or pending.
- Required items must be resolved (completed or skipped) for the run to become completed. The UI continues to show skipped and pending counts.
- Review the latest routine runs.

## Example

A user can build **Morning focus** as:

1. Drink water — action, required.
2. Read 10 pages — linked to the existing reading habit, required.
3. Review today's top three priorities — action, required.
4. Stretch — action, optional.

On 14 November 2026, the user starts the routine, completes steps 1 and 2, skips step 3 and leaves step 4 pending. The saved run records exactly that state. It does not mark the linked reading habit complete; the user still records the habit in the Today workflow, avoiding duplicate or accidental scoring.

## Data integrity and history

The additive 0004_routines.sql migration creates:
- routines — template title, description, recurrence, date bounds and lifecycle state.
- routine_steps — ordered editable template steps and optional habit links.
- routine_runs — one run per routine and local date.
- routine_run_steps — immutable title/type/required/order snapshots for each run with completed/skipped/pending state.

Run step step_id is intentionally not a foreign key to routine_steps. Editing or removing a template step therefore cannot rewrite an earlier run. The run itself is retained when a routine is archived; routines are not hard-deleted through the UI.

The routine migration has its own ledger and is applied during application database startup. This keeps the existing habit schema version and the separate goal milestone ledger independent. Routine records remain in the local SQLite database.

## Privacy and scoring boundaries

- Everything is processed locally; no cloud request, telemetry or remote AI.
- Linking a habit is organizational only. A routine step does not silently create or modify a habit_entries row, streak, adherence or goal progress.
- Paused habits may remain linked; archived habits are omitted from the new-link picker. Existing links are retained.
- The active Today view remains the source of truth for habit check-ins.
- No notification scheduling is introduced in this increment.

## Backup and export

The SQLite online backup naturally preserves all routine tables. The CSV bundle table allowlist now includes routines, routine_steps, routine_runs and routine_run_steps. Backup verification checks the routine migration ledger when present; older valid databases without the routine ledger remain supported.

## Validation

Rust tests cover:
- Template create, load, reorder/update and listing.
- Weekday-mask evaluation and start/end date boundaries.
- Same routine/date run idempotency.
- Run step snapshots surviving template edits.
- Required/optional completion behavior and skip/reopen behavior.
- Existing habit link validation and archived routine filtering/restoration.

Python SQLite schema tests cover constraints, foreign keys, unique run dates, valid dates and snapshot preservation. The data-portability test fixture also covers backup/restore and CSV export of routine history.

CI must pass formatting, all-target compilation, all Rust tests, Clippy, Python schema tests, backup/restore tests and desktop smoke checks. The actual Linux Mint machine still needs interactive run, pause/archive and history review.

## Research basis

OpenHabitTracker documents task planning with planned dates, durations, checklists and repeat settings, and habit rows that can carry checklists. That influenced the choice to use ordered repeatable steps, but this implementation keeps routine runs separate from the habit check-in table:
- https://www.openhabittracker.net/customizable-habit-tracker.html
- https://www.openhabittracker.net/

The reference product also emphasizes keyboard access and day-cell announcements for its calendar. Routine controls use native GTK widgets, but a real keyboard and screen-reader audit is still required:
- https://docs.gtk.org/gtk4/section-accessibility.html

## Deliberately deferred

- Multiple daily runs for the same routine.
- Time-of-day planning, timers and reminders.
- Routine templates that spawn independent task records.
- Drag-and-drop reorder and richer routine analytics.
- Bengali/English localization of this window.
- Auto-completing linked habit check-ins; intentionally excluded to avoid double entry and implicit score changes.
