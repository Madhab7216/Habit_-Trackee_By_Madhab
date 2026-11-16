# M3 — Measured habit entry

## Purpose

The first GTK workflow supported binary completion only, even though the SQLite model already had `numeric`, `count`, `duration` and `limit` tracking types. This increment exposes those existing types in the desktop workflow without adding a new database migration.

## Supported entry model

| Type | Example | Direction |
|---|---|---|
| Number | Read 20 pages | At least / At most |
| Count | Do 30 repetitions | At least / At most |
| Duration | Study for 45 minutes | At least / At most |
| Limit | Keep screen time below 2 hours | At most |
| Frequency | Exercise 3 times per week | Existing weekly-session cadence |

The unit is a short user-defined label (for example, pages, repetitions, minutes or hours). The target must be a finite positive number. Actual values may be zero but cannot be negative, NaN or infinite.

A measured check-in is stored in the existing `habit_entries.value` column. The status is `completed` when the selected target direction is met; otherwise it is `partial`. Saving again on the same local date updates the existing unique habit/date row instead of creating a duplicate. Limit habits always use the at-most direction.

Weekly frequency remains a session-based workflow: each completed session is a check-in, while the existing weekly quota remains the target. It is not converted into an amount/unit measurement.

## Data and privacy boundaries

- No schema migration is needed; the current habits table already contains tracking type, direction, target and unit fields.
- Values remain in the local SQLite database.
- No network, telemetry, account, cloud sync or personal-data fixture is introduced.
- The data-layer write checks active, scheduled and unpaused state; UI disabling is not treated as the security boundary.

## Automated coverage

The database tests cover persisted type/target/unit/direction, partial-to-completed updates, one-row-per-day behavior, at-most limit semantics, invalid configuration/value rejection, and paused-habit rejection.

## Manual acceptance still required

On the target Linux Mint workstation:

1. Create a number habit (target 20 pages, at least) and a duration habit (target 30 minutes).
2. Record 10, confirm the row shows Partial; update to 20, confirm it shows Completed and the value is updated rather than duplicated.
3. Create a limit habit (target 2 hours), record 3 and confirm Partial; record 2 and confirm Completed.
4. Restart the app and verify values remain visible.
5. Confirm weekly-frequency habits still use the existing session check-in flow.
6. Exercise the form with keyboard-only navigation, screen reader and enlarged text.

## Explicit follow-up boundaries

- Editing an existing measured habit's target/unit is not included; it needs effective-dated history semantics before it can safely reinterpret prior records.
- IANA timezone/DST/travel-safe local-date boundaries remain a separate release blocker.
- Bengali gettext catalog coverage for these newly added labels must be completed when this branch is integrated with the M6 localization branch.
- Measured history charts and period scoring remain in the separate M4/M5 work.
