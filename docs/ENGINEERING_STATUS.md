# Madhab Habit Tracker — Engineering Status

Audit date: 2026-11-14

## Verified repository baseline
- Default branch: `main`
- Existing application source before this change: none; README only.
- Existing feature branch: `work/linux-mint-habit-tracker-foundation`
- Existing pull requests, issues, and GitHub Actions runs: none.
- This work is intentionally isolated from `main`.

## Product scope
A native Linux desktop habit and personal productivity application, initially targeting Linux Mint. The product is local-first, bilingual (Bengali and English), accessible, and designed to remain usable without network access.

## Frozen M0 decisions
- Desktop UI: GTK4 using Rust bindings.
- Core language: Rust, with safe database access.
- Local persistence: SQLite; no account or cloud dependency in MVP.
- Habit scoring: show raw completion/adherence separately from streak and consistency; never hide missed days or reward unsafe over-tracking.
- Schedules: daily, selected weekdays, weekly target, interval, and paused periods.
- Entry types: boolean, count, duration, and bounded/limit habits.
- Data ownership: user-controlled export and backup; no telemetry by default.
- Languages: Bengali and English from the first user-facing release.

## MVP acceptance criteria
1. Create, edit, archive, and delete a habit with a confirmation step.
2. Support daily and weekly schedules, including selected weekdays.
3. Record complete, partial, skipped, and missed states distinctly.
4. Prevent duplicate records for the same habit and local calendar date.
5. Display today's plan and a 7-day adherence summary.
6. Persist data after application restart.
7. Export a portable CSV and create a consistent SQLite backup.
8. Handle empty, invalid, and interrupted input without losing existing data.
9. Run unit, integration, formatting, lint, and dependency checks in CI.
10. Keep all user data local unless a future opt-in sync design is approved.

## Engineering principles
- Use migrations for every schema change.
- Store timestamps in UTC and the user's IANA timezone separately; derive local habit dates explicitly.
- Use parameterized SQL only.
- Enable SQLite foreign keys on every connection.
- Keep database operations behind a repository/service layer.
- Do not store health-sensitive or intimate habit labels in logs.
- Never claim a feature is implemented until its tests pass on the supported Linux target.

## Milestones
- M0: requirements and architecture freeze.
- M1: Linux Mint toolchain and GTK dependency audit.
- M2: app shell, database migrations, and habit repository.
- M3: schedules, daily check-ins, and habit editor.
- M4: scoring and streak semantics.
- M5: analytics and dashboards.
- M6: localization, accessibility, backup/export, and security review.
- M7: packaging, release candidate, and install/update verification.

## Current status
This document freezes product scope and baseline. Application source code and verified Linux Mint build are still pending. GitHub branch writes are enabled for this feature branch; no changes have been made to `main`.
