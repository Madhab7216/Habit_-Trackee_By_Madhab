# GTK schedule editor

## User flow

The Today list exposes a separate **Schedule** action for each active habit. It opens a modal editor that appends a new schedule revision through the canonical immutable schedule-history API.

The editor supports:
- Daily recurrence.
- Selected weekdays (Monday through Sunday mask).
- Every N days, anchored to the new revision's effective date.
- N sessions per ISO week (target 1–7).
- An inclusive optional end date.
- A validated IANA time-zone identifier.
- A short reason describing the change.
- An explicit effective-from civil date.

The timezone field defaults to the latest validated revision's zone. If the previous revision was imported with a legacy/unverified timezone, the editor defaults to the currently resolved system IANA zone instead of silently treating the legacy marker as verified.

## Historical-change confirmation

A date earlier than today's date in the selected timezone requires the user to tick an explicit acknowledgement. The editor displays the exact affected interval, from the chosen effective date through today's date, and states that historical due-state interpretation changes while stored check-ins are not deleted. The append API independently enforces the confirmation token.

The editor does not update old schedule rows or mutate the `habits` recurrence columns. A database trigger rejects direct in-place schedule changes once history exists.

## Limitations

- The editor writes the chosen IANA timezone into the new revision, but the Today page and check-in path still use a single machine-local civil date. A separate runtime integration must compute each habit's civil date in its own IANA timezone before claiming full travel-safe behavior.
- Historical scoring remains a separate active consumer task (PR #12); it must replay the revision valid for each scored date.
- Manual keyboard navigation, screen-reader and scalable-text checks remain part of the existing accessibility work and target Linux Mint acceptance.