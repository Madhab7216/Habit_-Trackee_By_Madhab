# Security Policy

## Project status

Habit Trackers by Madhab is an engineering-preview project. No stable release or supported release series has been declared. Do not treat an unmerged work branch as a production release.

## Reporting a vulnerability

Please report suspected security vulnerabilities privately. Use GitHub's **Report a vulnerability** / private vulnerability reporting option on this repository when it is enabled.

If private vulnerability reporting is unavailable, contact the repository owner privately before sharing technical details publicly. Do not open a public issue containing an exploit, personal data, or a complete reproduction of a sensitive flaw.

A useful report includes:
- The affected commit, branch, or release (if one exists).
- The operating system and relevant runtime versions.
- A concise impact description and minimal reproduction steps.
- Any proposed mitigation, where known.

Please do not include a live habit database, real habit names or notes, backups, CSV exports, home-directory paths, credentials, access tokens, or other personal information. Use synthetic data in reproductions.

## Scope

Security reports are relevant to the desktop application, its Rust dependencies, SQLite schema and migration code, local backup/restore/export tools, and repository automation.

The current product direction is local-first and offline. A report about unexpected network access, sensitive data written to logs, unsafe file replacement, or unauthorized modification of local habit data is in scope.

## Coordinated disclosure

Please allow the maintainer a reasonable opportunity to investigate and prepare a fix before public disclosure. No response-time or remediation-time service level has been promised for this pre-release project.
