# Security Policy

## Project status

Habit Trackers by Madhab is currently a pre-release engineering project. There is no stable v1.0 release or formal long-term-support branch yet. Until a release policy is published, security fixes are considered for the latest maintained development branch.

## Reporting a vulnerability

Please do not publish exploit details, proof-of-concept code, private habit records, database files, or screenshots containing personal information in public issues, pull requests, or discussions.

1. On the GitHub repository page, open **Security** and look for **Report a vulnerability**. Use that private reporting route when it is available.
2. If private vulnerability reporting is not enabled, open a minimal public issue asking the maintainers to provide a private reporting channel. Do not include technical details or a proof of concept in that issue.
3. After a private channel is established, include the affected commit or branch, environment, impact, reproduction steps, and any safe supporting evidence.

Please allow the maintainers a reasonable opportunity to investigate and prepare a fix before public disclosure. No fixed response or remediation time is promised while the project remains pre-release.

## Scope

Security reports are especially useful for issues affecting:

- Local SQLite storage, migrations, and database access.
- Backup, restore, CSV export, file permissions, or path handling.
- GTK application behavior that could expose or corrupt local data.
- Build scripts, third-party Rust crates, or GitHub Actions workflows.
- Privacy boundaries, unintended network activity, or diagnostic data disclosure.

## Protect personal data

Habit names, goals, routines, notes, and exported records may reveal intimate information. Do not attach a real application database or unredacted personal exports to a report. Use a synthetic database and remove home-directory names, account identifiers, and unrelated machine details.

## Supported versions

There are currently no stable tagged releases. The latest maintained development branch is the intended reporting target until the project publishes a versioned support policy.
