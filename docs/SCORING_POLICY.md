# Habit scoring policy — v1

The scoring engine replays local SQLite check-ins and pause periods on demand. It exposes transparent adherence, an EMA habit-strength estimate and current streak separately, labelled **scoring version 1**. These are personal tracking indicators, not clinical or psychological measurements.

## Achievement ratio

- At least target: `min(actual / target, 1)`.
- At most target: 1.0 when actual is at or below target; otherwise `min(target / actual, 1)`.
- Targets must be finite and greater than zero; actual measurements must be finite and non-negative.
- A zero actual value is a full achievement for an at-most limit.

For example, a 120-minute screen-time limit scores 1.0 at 90 minutes and 0.667 at 180 minutes.

## Eligible periods

- Daily: each local calendar day.
- Weekdays: only the selected ISO weekdays, Monday bit 0 through Sunday bit 6.
- Every N days: anchored to the habit's stored start date.
- Weekly frequency: Monday–Sunday ISO week with a target from 1 to 7 sessions.

## Entry and pause semantics

Completed means a binary habit was checked or its measured target was achieved. Partial is a measured below-target result. Skipped is an intentional non-failure and is omitted from the score. Missed is derived for a closed eligible period without an entry and contributes zero. A fully paused period is excluded.

Pause starts are inclusive; end/resume dates are exclusive. Skips and pauses do not break streaks. Missed or below-target partial periods reset a streak. The current day or week is not marked missed before its period closes.

Same-day weekly sessions accumulate. Partial-week pauses reduce the weekly target in proportion to active scheduled days; fully paused weeks are excluded.

## Metrics

- Adherence: arithmetic mean of achievement ratios for scored, closed periods.
- Habit strength estimate: EMA with alpha 0.25; first eligible period initializes the value and later periods use `0.25 × current + 0.75 × previous`.
- Current streak: consecutive successful eligible periods ending at the latest closed period. A period must reach 1.0.

## Persisted replay implementation

- `scoring::summaries_for_today(connection)` evaluates active habits using the computer's current local date.
- `scoring::summaries_for_date(connection, as_of)` is deterministic; `as_of` is an exclusive boundary, so an open current day or week is not marked missed.
- Scores are rebuilt from `habit_entries` and `habit_pause_periods` on demand. No materialized score cache or remote service is used.
- The summary includes the scoring version, adherence, EMA, streak, closed/scored periods, missed periods, skipped periods and paused periods.
- Archived habits are excluded. Invalid stored dates, schedules, measurement directions and targets return a data error rather than a fabricated score.
- For a weekly frequency habit, a whole active week that is explicitly skipped is excluded. If skips are mixed with other uncompleted active days and there is no completed/partial session, the weekly quota is missed. Completed/partial session values accumulate toward the weekly target.

## Known limitations

Schedule edits currently update the active definition and can change how historical periods are interpreted. Before v1.0, effective-dated schedule definitions or an explicit retroactive-change confirmation are required. The application uses the OS local date; user-selected IANA timezones and DST/travel behavior remain outstanding. Numeric entry forms and edit-time schedule controls are still being built; this scoring integration evaluates the measurement values already present in the local database.
