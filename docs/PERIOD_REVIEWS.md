# Weekly and Monthly Reviews

## Purpose

The review window provides a private place to reflect on a completed week or month and write one practical focus for the next period. It complements, rather than changes, habit check-ins, goals, milestones or routine runs.

## Period contract

- A weekly review is keyed by a Monday start date and covers that Monday through Sunday.
- A monthly review is keyed by the first calendar day and covers that month.
- SQLite constraints reject a weekly period that does not start on Monday and a monthly period that does not start on day 01.
- There is at most one review per period. Saving again updates the existing record.
- Four note fields are available: wins, challenges, lessons and next focus. Each is limited to 8,000 Unicode characters.
- Notes are not copied into logs or diagnostics.

## Activity summary

The window shows the count of persisted `completed`, `partial` and `skipped` habit-entry records inside the chosen date range.

It deliberately does not call these figures adherence, success rate, streak or missed days. Empty dates are not interpreted as failures. Schedule history is not yet effective-dated, so review summaries avoid reconstructing schedule-based performance.

## Data ownership

The `period_reviews` table is added through its own additive migration ledger. It does not increment or rewrite the core habit schema version. SQLite online backups include the table automatically; the CSV export adds `period_reviews.csv` to the manifest when present. The migration ledger is schema metadata and is not exported as user content.

Review text can be sensitive. Use private device storage, protect backup destinations, and do not attach review exports or the live database to public issues or pull requests. No cloud service, telemetry or remote AI is used.

## Accessibility and localization boundary

The editor uses native GTK TextView controls, whose accessible role is text-box. Keyboard navigation, focus-order, Orca and contrast checks still need to be performed on the target Linux Mint desktop. The new strings are currently English; include them in the active Bengali/English gettext catalog work before v1.0.

## Validation

Automated checks cover:
- Separate migration idempotency and versioning.
- Monday/first-of-month validation and duplicate-period upsert.
- Oversized-note rejection without truncation.
- Activity summary status separation.
- CSV export of review notes and verified SQLite backup/restore preservation.

The actual GTK interactions, Bengali text rendering and target-device behavior remain manual release checks.
