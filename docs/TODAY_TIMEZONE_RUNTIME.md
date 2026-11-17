# Per-habit timezone runtime for Today and check-ins

## Purpose

The reusable IANA timezone engine and immutable schedule revisions are already present in the parent PR chain. Before this increment, the GTK Today screen still used one machine-local civil date for every habit. This meant a habit configured for another IANA zone could be marked due, paused, completed, or skipped on the wrong calendar date.

This change connects the Today/check-in path to the timezone stored on each currently effective schedule revision.

## Runtime contract

- Resolve the current absolute instant once for the Today refresh or user action.
- Read each active habit's immutable schedule revisions in effective-date order.
- Evaluate a revision's effective date in that revision's own IANA timezone. A future revision does not take effect until its own local civil date reaches `effective_from`; until then the previous revision remains current.
- Once a revision becomes effective, it supersedes older revisions permanently. If its inclusive `end_date` later expires, the app reports the habit as unscheduled instead of reviving an older schedule.
- Convert the instant to the selected zone's civil date with Jiff. Do not convert by adding or subtracting fixed UTC-hour offsets.
- Resolve completion state, pause state, check-in writes, pause starts and resume boundaries using that same per-habit civil date.
- The Today row exposes the resolved civil date and canonical zone in its tooltip so dates such as tomorrow in Kolkata are not silently presented as the machine's date.

## Legacy schedule behavior

Existing revisions tagged `legacy_unverified` retain their original timezone text. The app does not guess or rewrite their historical zone. At runtime, a legacy-unverified revision uses the current system IANA zone as a compatibility fallback. It remains marked legacy-unverified in SQLite until the user saves a new schedule revision with a validated IANA zone.

This fallback is intentionally explicit: the application cannot reconstruct which timezone a historical `local` marker represented before the machine's timezone changed.

## Automated coverage

The Rust tests use explicit instants to verify:

- Asia/Kolkata and America/Los_Angeles can show different local dates at the same instant.
- Check-ins are stored against each habit's own local date.
- Pause and resume boundaries use the same habit-local date.
- A future timezone revision activates exactly when its own timezone reaches the effective civil date.
- A legacy-unverified revision uses the system IANA zone without rewriting its stored status.

## Remaining acceptance

- Run the complete GTK Today, check-in, pause/resume and schedule-editor flow on the target Linux Mint workstation.
- Change the operating-system timezone while the app is closed and verify that validated schedule revisions continue to use their saved IANA zones.
- Confirm legacy-unverified habits follow the current system IANA zone without altering prior check-in dates.
- Run Orca/keyboard and Bengali rendering checks on the complete integrated branch.
- The M4 persisted scoring consumer and timed reminder delivery are separate follow-up integrations; this PR does not claim they are timezone-complete.
