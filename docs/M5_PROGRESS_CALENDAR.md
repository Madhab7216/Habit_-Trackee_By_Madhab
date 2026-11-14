# M5 — Progress Calendar & Insights

## Scope

This increment adds a local-only calendar and activity summary to Habit Trackers by Madhab. It reads existing habit_entries and habits records; it introduces no new tables, telemetry, network calls or additional database migration.

The window provides:
- 7-, 30- and 90-day calendar ranges.
- A calendar grid with day-level completed, partial and skipped counts.
- A clear selected-day detail view.
- Total saved-entry counts and the number of days with at least one saved check-in.
- Per-habit completed / partial / skipped breakdowns and recorded-completion bars.
- Completed check-in counts by weekday, Monday through Sunday.
- Historical activity for archived habits, since archiving must not erase a user's history.

## Metric contract and important limitation

A saved-entry completion percentage is:

completed saved entries / all saved entries (completed + partial + skipped) × 100

This is deliberately **not** called target adherence. A date with no saved entry is shown as empty activity, not as a missed habit. The current product does not yet store effective-dated schedule revisions, so reconstructing which historical days were due after a schedule edit would be unreliable. Similarly, streaks and missed-period totals are deferred until schedule-period replay and pause history can be applied consistently.

The UI keeps the three persisted statuses separate. Partial and skipped entries are never silently converted into a completed entry. An empty period safely reports 0% recorded completion rather than dividing by zero.

## Data and query behavior

- The selected range is inclusive and limited to 1–366 days; the UI exposes 7, 30 and 90 days.
- A recursive SQLite calendar query returns every date in the range, including dates with no habit_entries.
- All report queries are parameterized.
- Per-habit analytics include archived habits so historical records remain visible.
- Weekday counts are normalized from SQLite's Sunday-first numbering to Monday-first UI order.
- Machine-local current date is read from SQLite (date('now', 'localtime')) for the default report end date.

## Validation

Rust unit tests cover:
- Calendar gap-filling and distinct completed / partial / skipped states.
- Monday-first weekday aggregation.
- Archived-habit history preservation.
- Invalid Gregorian dates, leap days and range bounds.
- Empty periods and zero-safe percentages.

CI must pass cargo fmt, cargo check, cargo test and cargo clippy. A real Linux Mint launch and screen-reader/keyboard pass remain required before release.

## Research notes

The feature baseline was compared with established habit-tracking products. OpenHabitTracker documents history charts, a continuous calendar, longest streaks and weekday-frequency summaries; this supports treating calendar history and weekday analysis as distinct, useful views rather than collapsing everything into a single score:
- https://www.openhabittracker.net/habit-tracker-statistics.html
- https://github.com/Jinjinov/OpenHabitTracker

Loop Habit Tracker is another open-source reference for long-term habit history and statistics:
- https://github.com/iSoron/uhabits

GTK accessibility guidance recommends explicit accessible labels for controls and testing custom UI with assistive technologies. This implementation uses text-labelled date buttons and descriptive tooltips, but a real screen-reader audit is still open:
- https://docs.gtk.org/gtk4/section-accessibility.html
- https://gtk-rs.org/gtk4-rs/stable/latest/book/accessibility.html

## Deferred scope

- Schedule-aware adherence, missed-period classification and true streaks.
- Historical schedule revision/effective-date model.
- Per-habit date-range filter and trend charts.
- Editable daily notes and weekly review.
- Bengali/English localization of this window.
