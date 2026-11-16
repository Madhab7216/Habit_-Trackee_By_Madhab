# Effective-dated habit schedule history

## Why this exists

A habit's recurrence rule is part of the meaning of its past records. If an application overwrites a habit's current schedule in place, replaying an old date against today's rule can turn a previously due date into an unscheduled date (or the reverse). This makes historical adherence, streaks and reports unstable.

Schema version 3 adds `habit_schedule_revisions`. It stores the recurrence rule that was valid for each effective-date range rather than treating the latest rule as if it had always applied.

## Date-range contract

Each revision has:

- `effective_from_date`: inclusive first local civil date for the rule.
- `effective_until_date`: exclusive first date on which that revision no longer applies; NULL means the open/current or planned-future revision.
- `schedule_end_date`: optional inclusive last scheduled date within that revision.
- The complete cadence fields, period unit and the schedule's timezone identifier.

Effective ranges use half-open intervals `[effective_from_date, effective_until_date)`. This makes adjacent revisions unambiguous: when one revision ends on date D, the next revision can begin on D. The cadence's own end date remains inclusive to preserve the existing schedule contract.

The database rejects overlapping revisions, more than one open revision per habit, attempts to delete history, and edits to a revision once its effective date has arrived. A still-future open revision may be adjusted before it takes effect. Legacy schedule columns on `habits` are protected against changed values; schedule changes must go through the revision API.

## Application API

- New habit insertion automatically creates the initial revision through a database trigger.
- `db::habit_schedule_for_date(connection, habit_id, date)` returns the exact `HabitSchedule` valid for that date, including its original cadence anchor.
- `db::habits_for_date` now evaluates due/not-due state against the revision valid for the requested date.
- `db::change_habit_schedule(connection, habit_id, cadence, effective_from)` appends a new schedule for a future date. It rejects today/past dates, archived/unknown habits and out-of-order revisions. A pending revision with the same future effective date can be revised before that date arrives.

A schedule change does not rewrite check-ins, notes, pause periods or existing history. Any analytics/scoring component that replays historical periods must load the revision for each period's date; reading the current recurrence fields from `habits` is not a valid historical replay strategy.

## Existing database migration and its limit

Migration 0003 copies each existing habit's last-known recurrence columns into one initial revision, starting at the legacy `habits.start_date`. The old schema did not retain prior schedule edits, so the migration cannot reconstruct schedule versions that were overwritten before this upgrade. It must not claim those earlier periods are historically accurate. For upgraded databases, earlier reports should be treated as based on the last-known pre-upgrade rule unless the user has independently confirmed the historical schedule.

New installations and habits created after migration record each schedule revision from the beginning.

## Portability and privacy

The revision table is included in the verified schema checks, SQLite backup/restore (which already snapshots the whole database), and the CSV export bundle. CSV exports therefore carry both habit records and the dated recurrence history needed to interpret them. Existing local-only storage permissions and refuse-to-overwrite behavior remain unchanged.

## Integration still required before release

This PR supplies the schema, migration, persistence API and date-based schedule evaluation. It does not yet add a schedule-editing GTK dialog. The M4 historical scoring/replay implementation must use this API for every scored date, and the IANA timezone/DST engine must be integrated before claiming travel-safe local-date behavior. The current default UI timezone remains machine-local. Keep this change in Draft until those consumer integrations and target Linux Mint tests are reviewed.
