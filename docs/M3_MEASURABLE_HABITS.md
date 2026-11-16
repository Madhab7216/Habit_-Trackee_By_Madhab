# M3 — Measurable habit setup and check-ins

## Purpose

The initial Today screen recorded binary checkboxes only. This increment adds a typed path for habits whose daily result is a quantity or duration, while keeping the existing binary and weekly-frequency flows unchanged.

## Supported measured types

| Type | Example | Direction |
|---|---|---|
| Numeric | Drink 1,800 ml of water | At least |
| Duration | Study for 30 minutes | At least |
| Count | Read 10 pages or do 20 repetitions | At least or at most |
| Limit / avoid | No more than 2 cups | At most |

Each measured habit stores one target and one display unit. Measurement units are bounded to 1–24 non-control characters. A recorded check-in stores the actual value and an optional local note.

## Check-in semantics

- **Completed:** the recorded value satisfies the target direction.
- **Partial:** a value was recorded but did not satisfy the target.
- **Skipped:** remains a separate intentional state and is never generated merely because a measured value is below target.
- **Missed:** is derived by the existing schedule/scoring workflow after an eligible period closes; it is not written by the value-entry form.
- One record per habit and local date is preserved by the existing unique key. Updating today's value replaces that date's value instead of creating duplicate rows.

At-least values use the existing scoring v1 ratio: min(actual / target, 1). At-most values use 1 while actual is within the cap, otherwise min(target / actual, 1). Higher consumption cannot improve an at-most score.

## Data integrity

- Habit creation and measured configuration are written in one SQLite transaction. Invalid inputs roll the entire insert back.
- The storage layer rejects NaN, infinity, negative actual values, invalid targets, unknown units, and measured check-ins for paused or unscheduled dates.
- A limit habit must use the at-most direction.
- Weekly-frequency habits remain a distinct type because their target is a number of sessions in a weekly period, not a quantity for each check-in.
- Binary habits cannot accept numeric measurement entries.
- Undo removes only a measured completed/partial row. It does not delete an intentional skipped row.
- No migration is needed: the existing schema already has tracking_type, direction, target_value, unit, and numeric habit_entries.value fields. Existing binary rows are not rewritten.

## Explicit scoring v1 limitation

A zero target is rejected. The existing scoring v1 implementation treats zero as an invalid target, so accepting a “must be exactly zero” limit now would create invalid or undefined scores. A true zero-tolerance avoid habit should be added only alongside a versioned scoring-policy change and matching persisted-replay tests. A small positive epsilon is not an acceptable substitute for zero.

## Validation

Rust tests cover configuration persistence, at-least and at-most outcomes, partial versus completed values, count storage, value replacement, paused/unscheduled guards, invalid input rejection, binary/frequency separation, undo semantics, and transaction rollback.

Hosted formatting, compilation, Rust tests, Clippy, SQLite schema tests and the existing data-portability tests must pass on the exact PR head. A real Linux Mint test must still check the new GTK flow, including Bengali text sizing after localization is integrated.
