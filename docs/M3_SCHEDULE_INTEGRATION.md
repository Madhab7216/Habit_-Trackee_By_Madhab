# M3 — schedule-aware habit workflow

## Schedule source of truth

The schedule is persisted in SQLite and evaluated through pure Rust domain types. The data layer rejects check-ins for archived, paused and not-scheduled dates; UI disabling is only a convenience.

Supported cadence types:
- Daily.
- Selected weekdays (ISO weekday mask).
- Every N days, anchored to the habit start date.
- Times per week, evaluated in a Monday-anchored ISO week with a quota from 1 to 7.

The GTK create form offers daily, Monday–Friday, every-two-days and weekly-frequency presets. The DB/domain layer supports arbitrary valid weekday masks and interval sizes.

## Daily workflow

- Add a habit with one of the schedule presets.
- Today shows scheduled and not-scheduled items separately.
- Mark binary habits done or undo.
- Skip an eligible date intentionally.
- Pause/resume; pause start is inclusive and resume date exclusive.
- Edit names, archive without deleting history, and restore archived items.
- Data-layer validation prevents invalid check-ins even when called outside the UI.

## Validation limits

The Edit dialog currently focuses on the name and description; full schedule editing is not yet present. Schedule edits must be versioned by effective date or explicitly confirm whether past history is reinterpreted. IANA timezone selection, DST/travel handling and a target Linux Mint UI launch test remain required before v1.0.
