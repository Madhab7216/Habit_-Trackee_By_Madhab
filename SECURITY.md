# Security Policy

## Reporting a vulnerability

Please do not publish exploit details, proof-of-concept code, personal habit records, database files, backups, or private notes in a public issue or pull request.

Use GitHub's **Security → Advisories → Report a vulnerability** flow to submit a private report when it is enabled for this repository. Include the affected commit or release, Linux distribution, relevant environment details, impact, and reproducible steps. Remove usernames, home-directory paths, habit names, and other personal data.

If private vulnerability reporting is not available, do not disclose technical exploit details publicly. Open a minimal public issue asking the maintainer to enable a private reporting channel.

## Supported versions

This repository is an engineering preview, not a stable v1.0 release. Development branches are provided without a security-support SLA. Once stable releases are published, this section must be updated with an explicit supported-version policy.

## Security boundaries

- The application is intended to keep habit and productivity data on the user's device.
- Do not include live SQLite databases, personal exports, habit labels, or notes in bug reports or CI artifacts.
- CI workflows should use the least permissions required and must not receive secrets for untrusted pull-request code.
- Static-analysis and dependency-scanning results are signals for review; they are not a guarantee that the application is secure.
