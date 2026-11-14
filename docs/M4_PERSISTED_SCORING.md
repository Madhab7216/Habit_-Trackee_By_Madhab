# M4 — Persisted scoring v1

## Purpose

The domain layer already contained pure achievement, adherence, EMA and streak functions. This increment connects those rules to the saved local SQLite records and exposes a read-only progress view in the GTK application.

Scoring is **versioned as v1** and rebuilt on demand from source records. No score cache is stored, so edits to check-ins or pause history cannot leave a stale materialized score behind.

## User experience

Open **Progress & scores** from the Today window. Each active habit shows:

- **Adherence** — mean achievement across scored, closed periods.
- **Habit strength** — EMA estimate with alpha 0.25.
- **Current streak** — successful eligible periods ending at the latest closed period.
- A detail line containing the scoring version and the number of closed, missed, skipped and paused periods.

A habit with no eligible scored history displays **No history**, not 0%. This avoids confusing a new habit with a habit that has failed every opportunity.

## Replay API

- `scoring::summaries_for_today(connection)` uses the computer's current local calendar date.
- `scoring::summaries_for_date(connection, as_of)` is deterministic. The `as_of` date is an exclusive boundary.
- The current open day or Monday–Sunday week is not counted as missed before its schedule period closes.
- Only active (non-archived) habits are included.
- Replay reads `habits`, `habit_entries` and `habit_pause_periods`; it does not modify them.

## Period rules

- Daily, selected-weekday and every-N-days habits are evaluated once per eligible local date.
- A completed binary check-in contributes 1.0.
- Numeric/duration/count/limit records are normalized by the existing at-least or at-most achievement function. For an at-most target, staying at or below the limit scores 1.0; exceeding the limit decreases the score.
- A daily intentional skip is omitted from adherence and does not break a streak.
- A closed eligible date with no entry is missed and contributes 0.0.
- A fully paused eligible day is excluded from adherence and streak calculation.
- Weekly frequency sessions accumulate toward the weekly quota. The target is prorated by the number of active days when the week is partially paused. A completely paused week is excluded.
- If every active day of a week is explicitly skipped, the whole weekly period is excluded. A mixed week containing skips but no completed/partial session is still a missed quota period.

## Validation added

Rust tests cover:

- Daily binary replay with missed periods, a skip, a recent completion and the open-date boundary.
- At-most limit scoring where higher consumption lowers the score.
- Exclusion of skipped and fully paused daily periods.
- Weekly session accumulation and partial-week pause proration.
- A weekly period remaining open until its full Monday–Sunday window closes.
- Mixed weekly skip/no-session semantics.
- No-history returning empty metrics instead of zero-percent scores.
- Gregorian ordinal/date round-trip for leap-year and century boundaries.

The existing M3, M5 and M6 workflows are kept separate; this increment does not edit the goals UI or the portability module.

## Privacy and performance

No network calls, telemetry, cloud sync or remote AI are introduced. No personal habit database, live labels or history is attached to CI. The score view reads local records only.

The first version favors correctness and a single source of truth over caching. Very long habit histories may make a full replay noticeable; if needed, a future optimization must preserve deterministic replay and include an explicit scoring-version migration strategy.

## Remaining boundaries

- The application still uses the operating system's local date; selectable IANA time zones and travel/DST boundaries remain a separate release blocker.
- Schedule edits currently reinterpret past periods using the active schedule. Effective-dated schedule history or an explicit retroactive-change confirmation is required before v1.0.
- Numeric, duration, count, limit/avoid and frequency data-entry forms are not all complete in GTK. This increment scores valid values already present in SQLite; it does not claim those forms are finished.
- Target Linux Mint launch, manual GTK validation and performance testing remain required.
