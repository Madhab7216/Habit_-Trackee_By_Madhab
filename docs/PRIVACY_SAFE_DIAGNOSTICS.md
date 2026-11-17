# Privacy-safe diagnostics

## Finding

The GTK application and local data CLI previously formatted raw SQLite and filesystem errors directly into terminal output and, in one case, the Edit habit dialog. Underlying error strings may contain SQL details, invalid values, database filenames or absolute filesystem paths. This is unnecessary for the user-facing workflow and can leak local context if terminal output or screenshots are shared.

## Policy

- User-facing messages are static, operation-specific text.
- Terminal diagnostics contain only a fixed operation identifier and an allow-listed error class.
- Raw `rusqlite::Error`, `std::io::Error`, source chains, SQL text, habit names and filesystem paths are never formatted.
- The data CLI does not echo user-supplied source/destination paths in success or failure output.
- No telemetry, remote logging or new dependency is added.

## Implementation

`src/diagnostics.rs` classifies known SQLite and I/O failure kinds into a small vocabulary, including `database_busy`, `database_integrity`, `permission_denied`, `disk_full`, `constraint_violation`, `record_not_found` and generic fallback codes.

GTK actions show static user messages and write safe operation/code pairs to stderr. The local data CLI uses the same classification and no longer prints raw error messages or supplied paths.

## Automated coverage

Rust unit tests verify that diagnostic output does not contain private habit labels, SQL messages, absolute home-directory paths or database filenames. Existing engineering CI runs these tests using `cargo test --all-targets`.

## Research basis

OWASP's Logging Cheat Sheet recommends removing, masking or sanitizing sensitive personal data and treating file paths and database connection details carefully. Rusqlite exposes typed error variants and SQLite error codes, which allow a small allow-listed classification instead of forwarding the message string.

References:
- [OWASP Logging Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html)
- [rusqlite Error](https://docs.rs/rusqlite/0.42.0/rusqlite/enum.Error.html)
- [rusqlite ErrorCode](https://docs.rs/rusqlite/0.42.0/rusqlite/enum.ErrorCode.html)

## Limitations

This change hardens diagnostic output paths in the GTK main application and local CLI. It does not guarantee that every future feature, third-party library, crash reporter or operating-system journal is privacy-safe; new logging paths must follow the same policy and be reviewed.
