# Today UI schedule-history integration

## Data source of truth

After schedule-history migration, the immutable `habit_schedule_revisions` ledger is the source of truth for recurrence evaluation. The recurrence columns retained on `habits` are legacy compatibility fields; the Today query must not use them to decide whether a habit is due.

For a requested civil date D, the database selects the revision satisfying all of these rules:

1. `effective_from <= D` (inclusive).
2. No later revision for the same habit has `effective_from <= D`.
3. The selected revision's own `end_date` is NULL or `end_date >= D` (inclusive).

The second rule prevents a superseded schedule from being revived after a newer, time-limited revision expires. If the latest effective revision has expired, there is no active schedule for that date.

The selected revision's recurrence and effective_from anchor are evaluated by the existing deterministic `HabitSchedule` domain logic. Weekly frequency remains a Monday-anchored ISO-week period.

## Check-in behavior

- `today_habits` and `habits_for_date` use the schedule revision selected for the requested date.
- `set_completed_for_date` calls the same date query before writing, so an unscheduled date or paused habit cannot be checked in against a stale legacy recurrence.
- Undo remains allowed for a saved completed entry as before.
- The migration installs a database trigger preventing in-place changes to legacy recurrence columns after a baseline revision exists. Schedule changes must be appended through the immutable schedule-history API.

## Timezone boundary

This increment makes Today and check-in validation revision-aware for the civil date supplied by the current caller. It does **not** yet resolve "today" independently in each habit's saved IANA timezone: `today_habits` still starts from the machine-local date, and the GTK interaction still uses that date for the entry. A separate UI/runtime integration must carry per-habit local dates end-to-end before claiming travel-safe daily check-ins.

## Scoring boundary

The M4 persisted-scoring PR is a separate active branch. Its current schedule loader still reads legacy recurrence columns. It must be rebased/integrated with the schedule-history branch and load the revision valid for each scored period before the product can claim historical scoring is schedule-version-correct. This PR does not edit that active branch.

## Validation

Regression tests cover weekday, every-N-days and times-per-week schedules through revision rows, future-effective schedule changes, expiry without baseline resurrection, and rejection of check-ins on dates excluded by the active revision.
