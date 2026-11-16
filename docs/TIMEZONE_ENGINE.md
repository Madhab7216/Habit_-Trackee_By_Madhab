# Time-zone-safe civil dates and reminder times

## Why this module exists

The current foundation schema already has habits.time_zone_id, but the Today workflow previously derived the current date using SQLite's date('now', 'localtime'). That makes the host's present timezone the effective date source even when a habit has a different stored zone.

This increment adds a tested domain service. It does not change the active M3 schedule editor, persisted scoring, reminders, UI or historical records. Those active branches can adopt the service once their data contracts are integrated.

## Dependency decision

Use Jiff 1.0.0, pinned exactly for the engineering preview. It was released on 15 November 2026, has MSRV 1.85 (the project targets Rust 1.92), integrates with the IANA Time Zone Database, and exposes explicit APIs for classifying and resolving civil-time gaps and folds.

On Linux, Jiff reads the system time-zone database from /usr/share/zoneinfo (or TZDIR); the default feature set also provides a bundled platform fallback on platforms without a canonical system database. System timezone rules can be updated independently of the app binary.

## API contract

- IanaTimeZone::parse accepts a named IANA time-zone identifier and stores its canonicalized name. It rejects blank values, surrounding whitespace, unknown names, fixed offsets and unknown/non-IANA zones.
- IanaTimeZone::system resolves the current operating-system timezone to a named IANA identifier. If the system zone cannot be identified safely, it returns an error instead of silently storing an offset or Etc/Unknown.
- local_date_at converts an absolute instant to the date in the selected zone. The result is a civil date, not a UTC date.
- classify_local_datetime reports whether a reminder's wall-clock time is unique, nonexistent (gap) or repeated (fold).
- resolve_local_datetime requires a caller-selected policy. Reject is the safe default; Earlier, Later and Compatible are available only when the product UI explicitly chooses the behaviour.

## Correctness rules

1. A recurring habit is a civil-calendar rule in a named zone. Do not calculate a next local day by adding 86,400 seconds to a UTC instant.
2. Persist the IANA name (for example, Asia/Kolkata) with the habit/schedule revision, not a numeric UTC offset or a mutable local marker.
3. If the operating-system timezone changes, previously saved local_date check-ins must not be rewritten. A future schedule revision can adopt the new zone from a user-approved effective date.
4. A DST gap is not a valid wall-clock reminder time; a DST fold represents two possible instants. The application must not silently guess. The current default is to reject and let a future UI prompt the user.
5. A local day is not guaranteed to contain exactly 24 hours, and a jurisdiction can skip an entire civil date. Schedule evaluation must use the named zone's date rules rather than fixed-duration arithmetic.

## Automated test matrix

The Rust unit tests cover:

- IANA identifier canonicalization and invalid-zone rejection.
- Asia/Kolkata conversion across the UTC date boundary.
- The 2026 local midnight boundary in Asia/Kolkata.
- Pacific/Apia's skipped 30 December 2011 civil date.
- America/New_York's 10 March 2024 spring-forward gap.
- America/New_York's 3 November 2024 fall-back fold.
- Explicit rejection, earlier, later and compatible resolution strategies.

## Integration work still required

- Replace the Today workflow's implicit host-local local_today helper with a function that reads each habit's persisted zone or its effective-dated schedule revision.
- Add a zone selector and a migration-safe effective-dated schedule revision model; do not reinterpret existing check-in dates.
- Integrate the reminder branch with classify_local_datetime and an explicit user-visible gap/fold preference.
- Record which time-zone data version is used when expanding timed reminders if reproducibility is required; refresh future reminders after tzdb updates, never saved historical check-ins.
- Run DST, OS-timezone-change, upgrade and backup/restore tests on the target Linux Mint workstation.

This increment is the reusable engine and test suite, not a claim that the GTK application is already fully timezone-aware.
