# Reminders and Quiet Hours — Engineering Design

**Status:** first implementation slice; deterministic domain policy only  
**Reviewed:** 2026-11-17  
**Target:** Linux Mint desktop; local-first and offline

This workstream is separate from the active measured-habit, scoring, calendar, goals, routine, localization, data-portability, local-file-permission and supply-chain PRs. The first code increment adds a pure Rust policy module only. It does not yet persist reminders, display GTK controls, create a background service or claim notifications work while the app is closed.

## Product principles

1. Reminders are opt-in. A habit without an enabled reminder must never generate a notification.
2. Quiet hours are a user-owned local-time policy, including intervals that cross midnight.
3. One reminder occurrence is delivered at most once per configured local date.
4. A late reminder is useful only inside a bounded grace window; stale reminders are dropped, not replayed in a burst.
5. Notification contents must not expose habit names or notes by default. Start with generic text such as “A habit reminder is due”; detailed text requires an explicit privacy preference.
6. No cloud service, telemetry, ads, remote AI or network permission is required.
7. A successful database decision is not the same as a notification displayed by the desktop shell.

## Implemented in this increment

src/reminders.rs provides a deterministic, side-effect-free policy engine:

- strict 24-hour HH:MM parsing and validation;
- ISO weekday mask (Monday = bit 0 through Sunday = bit 6);
- configurable 1–180 minute late-delivery grace window;
- same-day and overnight quiet-hours intervals with inclusive-start/exclusive-end boundaries;
- explicit decisions for disabled, not scheduled, before time, already delivered, stale, quiet hours and due;
- an occurrence result with the local date and scheduled time, suitable for a persistent deduplication key;
- unit tests for parsing, invalid settings, weekday selection, interval boundaries, deduplication and quiet-hours behaviour.

The policy accepts a caller-supplied local date/time. It intentionally does not convert UTC instants to local time or resolve DST gaps/folds. The timezone adapter must use the selected IANA zone and resolve those conditions before calling this module.

## Required persistence model before UI release

Use an additive migration and a separate reminder schema ledger if needed to avoid collisions with concurrently developed feature migrations.

Suggested tables:

- reminders: stable ID, optional habit/routine reference, local wall-clock time, ISO weekday mask, IANA timezone ID, enabled flag, grace minutes, created/updated timestamps.
- reminder_deliveries: reminder ID, scheduled local date, actual dispatch timestamp and dispatch outcome. Enforce UNIQUE(reminder_id, scheduled_local_date) so restarts cannot send the same occurrence twice.
- reminder_settings: timezone selection, quiet-hours enabled/start/end and generic-vs-detailed notification preference.

Migrations must be additive, transactional and tested against a copied database. Do not upload real habit data. Disabling or deleting a reminder must not delete habit history.

## Delivery architecture: do not confuse sending with scheduling

GTK's GIO ApplicationExt::send_notification sends a notification to the desktop shell. It does not schedule future notification times and the API explicitly does not guarantee that the shell will display a notification. GNotification may persist after the application exits when the desktop integration supports it; the application should have a matching desktop file and D-Bus activation metadata.

For dependable reminders while the main window is closed, a separately supervised per-user background component is required. Linux Mint is the initial target, so a systemd user service/timer is a candidate, but it must be tested with the supported Mint desktop session and session D-Bus before adoption.

A future background adapter should:
- run only in the logged-in user session, never as root or a system daemon;
- read only the local reminder schedule and minimal display-safe fields;
- use the shared policy engine before dispatch;
- write the unique delivery ledger before reporting an occurrence as delivered;
- re-check the grace window after resume/login so powered-off reminders do not flood the user;
- follow the configured timezone and detect clock/timezone changes;
- expose an obvious disable/uninstall path and never leave orphaned timers;
- fail closed if the graphical notification session is unavailable.

Do not introduce per-reminder systemd units until scale and lifecycle are tested. Prefer one bounded scheduler service that evaluates due rows in a transaction and dispatches the resulting occurrences.

## Timezone and DST contract

Persist an IANA timezone identifier, not a numeric UTC offset. The configured zone must be explicit and changing the OS timezone must not silently rewrite historical check-ins.

For each scheduled local wall-clock occurrence:
- if the local time is nonexistent due to a spring-forward transition, skip that occurrence and record the skip reason;
- if it is ambiguous during a fall-back transition, select the earlier instant once by default, and never dispatch twice for the same reminder/local date;
- show the selected timezone and next occurrence in the reminder editor;
- test the same recurrence across Asia/Kolkata, America/New_York spring-forward and fall-back dates.

Any alternate user policy for ambiguous/nonexistent times must be visible and deterministic. No code in this first slice guesses an offset.

## Quiet-hours contract

The interval is half-open: start is quiet; end is allowed. Both daytime (13:00–14:00) and overnight (22:00–07:00) windows are supported. Equal start/end is rejected; disable quiet hours explicitly instead of interpreting equal boundaries as either zero or 24 hours.

An occurrence evaluated during quiet hours is suppressed for that pass. It may be delivered after quiet hours only while the original same-date grace window remains open. An occurrence that passes its grace window is dropped. No reminder is carried over to the following day.

## Validation and release gates

- Unit-test the pure policy module on every supported Rust target.
- Add SQLite tests for migration, unique occurrence insertion, concurrent dispatch and rollback.
- Test app closed, app open, session logout/login, suspend/resume, reboot, timezone change, clock correction and notification permission disabled on the actual target Linux Mint release.
- Verify generic notification mode does not disclose habit labels in lock-screen previews.
- Verify disabling reminders removes/turns off the user-level background component and no delivery occurs afterwards.
- Verify no notification is sent while quiet hours are active.
- Treat delivery as best-effort: desktop notification daemons may suppress or defer display. Show reminder delivery status inside the app rather than claiming the popup was seen.

## Research references

- [Gio Application.send_notification](https://docs.gtk.org/gio/method.Application.send_notification.html): sends a notification through the desktop shell; display is not guaranteed, and applications may be D-Bus activated.
- [gtk-rs ApplicationExt::send_notification](https://gtk-rs.org/gtk-rs-core/stable/latest/docs/gio/prelude/trait.ApplicationExt.html): Rust binding semantics.
- [GNotification](https://gtk-rs.org/gtk-rs-core/stable/latest/docs/gio/struct.Notification.html): desktop-file and application-ID integration requirements.
- [Desktop Notifications Specification 1.3](https://specifications.freedesktop.org/notification/latest-single/): session-scoped D-Bus notification service.
- [systemd.timer](https://www.freedesktop.org/software/systemd/man/latest/systemd.timer.html): calendar timers, accuracy windows and persistent catch-up behaviour; use a user timer only after session integration tests.
- [systemd desktop environment integration](https://systemd.io/DESKTOP_ENVIRONMENTS/): graphical-session integration considerations.

## Explicit non-goals

This increment does not implement notification delivery, snooze, recurring routine notifications, mobile push, calendar sync, cloud reminders or health/medical prompting. It does not wake a sleeping computer or guarantee a notification popup is seen.
