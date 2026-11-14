# M3 — Persisted schedule integration

## Delivered in this follow-up

This change connects the schedule fields already stored in SQLite to the existing pure Rust scheduling engine. It is isolated from the foundation branch's rename and reversible archive/restore work.

- Daily habits are due every day in their active date range.
- Selected-weekday habits are due only on selected ISO weekdays (Monday through Sunday).
- Every-N-days habits use the persisted start date as the interval anchor.
- Times-per-week habits are evaluated in a Monday-anchored weekly period and are eligible on any date in the active range.
- Start and end dates are applied before the schedule is considered due.
- The Today screen distinguishes Due today, Not scheduled, Completed and Paused.
- Check-ins are rejected in the storage layer for paused, archived and not-scheduled dates. Button disabling is only a UI aid.

## Deterministic database API

`db::habits_for_date(connection, CivilDate)` evaluates persisted schedules for an explicit date. This provides a stable seam for schedule tests and future calendar/history views.

`db::today_habits(connection)` reads the computer's current local date once and delegates to the deterministic API so all Today rows use one date boundary.

`db::set_today_completed` routes completion through the schedule guard. Undo remains available for an active habit even if its schedule was subsequently edited.

## Tests added

- Selected Monday-only weekdays versus Sunday.
- Every-two-days cadence anchored at the stored start date.
- Start-date and end-date bounds.
- Monday-anchored times-per-week eligibility.
- Rejection of off-schedule check-ins and acceptance on a due date.
- Archived habits are excluded from schedule date queries.
- Existing pause/resume, rename and archive/restore tests are preserved.

## Remaining limitations

- The GTK creation form still creates daily binary habits; a schedule editor is not included.
- Numeric, duration, count, limit/avoid, frequency-goal and partial/skipped check-in forms are still open.
- Times-per-week eligibility exists, but weekly quota progress and a target-met state are not yet displayed.
- Missed-period derivation, persisted scoring replay, analytics, goals, routines and reminders remain separate work.
- Bengali/English gettext, accessibility review, CSV export, verified SQLite backup/restore and packaging remain open.
- The app still uses the machine's current local date; selectable IANA time zones and travel-safe date boundaries are not included.
- The real Linux Mint workstation build/launch/persistence check belongs to the separate M1 validation task and is not claimed as passed here.

## Privacy

No network calls, telemetry, cloud storage or remote AI are introduced. No personal habit labels or check-in history are included in tests or CI artifacts.