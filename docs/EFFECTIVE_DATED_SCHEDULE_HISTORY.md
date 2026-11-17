# Effective-dated schedule history

## Purpose

A habit can change its recurrence or timezone over time. Replacing the current schedule fields in place loses the rule that governed earlier dates and can silently alter historical adherence or streak calculations.

This increment adds an append-only revision ledger on top of the IANA timezone engine in PR #28. Each revision has an inclusive effective date, recurrence rule, optional inclusive end date and validated IANA timezone. The next revision's effective date is the previous revision's exclusive upper boundary.

## Data model

The migration creates a separate schedule-history ledger and the table habit_schedule_revisions. It does not increment the existing core schema version or rewrite habit_entries.

For existing habits, one baseline revision is backfilled from the current habits schedule fields. The original timezone value is copied verbatim and marked legacy_unverified. In particular, a historical time_zone_id value of local is not guessed or silently rewritten: the old database does not prove which IANA zone was in effect when earlier check-ins were recorded.

New revisions must use a valid named IANA timezone through the IanaTimeZone service. Fixed offsets, unknown zone names and the mutable local marker are rejected.

Revision rows are immutable. Database triggers reject UPDATE and DELETE; corrections are new revisions, not edits to old records. Revisions are selected by effective_from, not by creation order. revision_no records creation order for audit purposes.

## Effective-date rules

- effective_from is inclusive.
- A revision applies until the next revision's effective_from date, exclusive.
- An optional end_date is inclusive. After that date and before the next revision, there is no active schedule.
- A new revision cannot start before the habit's original start date.
- A revision effective today or later can be added without retroactive confirmation.
- A past-effective revision requires the caller to pass PastEditConfirmation::Confirmed. The UI must first show the affected historical period and obtain explicit user confirmation.
- One revision per habit per effective date is permitted. If a revision already exists for a date, choose a new effective date rather than mutating the existing row.

Past-effective revisions can change how historical performance is interpreted even though they do not alter stored check-ins. This is why confirmation is required.

## API

The Rust library module exposes:

- initialize(connection): apply the additive migration and backfill missing legacy baselines. It is safe to call more than once.
- append_revision(...): validate and append a new immutable revision, including the IANA zone and retroactive-confirmation guard.
- schedule_for_date(...): retrieve the rule that governed a given date, including its next-revision exclusive boundary.
- revisions_for_habit(...): return the full effective-date history.

Consumers should use schedule_for_date when evaluating a historical or future date. The old habits schedule columns remain as a compatibility snapshot for existing screens until their callers are migrated. This PR does not claim that Today, persisted scoring or reminder scheduling has already switched to the revision service.

## Validation

Rust tests cover:
- baseline backfill and repeated initialization;
- future revision boundaries and IANA zone persistence;
- past-effective edit confirmation;
- preservation of existing check-in rows;
- invalid timezone, recurrence, end date and change-reason rejection;
- duplicate effective dates and revisions before habit start;
- immutable update/delete enforcement;
- missing and archived habit rejection.

Hosted CI must pass formatting, build, all Rust tests and Clippy on the exact branch head. Real Linux Mint timezone-change, DST, backup/restore and UI integration tests remain release gates.
