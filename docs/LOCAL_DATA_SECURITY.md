# Local data security baseline

## Scope

Habit names, descriptions, check-ins, notes, pause history, goals and settings are private user data. The app stores them locally and does not need network access for its core tracking workflow.

On Unix/Linux, application startup now enforces:

| Resource | Required mode | Purpose |
|---|---:|---|
| App data directory | `0700` | Only the owning account can traverse or list the directory |
| SQLite database | `0600` | Only the owning account can read or write the database |
| SQLite WAL/SHM sidecars | Protected by the `0700` parent directory | Prevent other local accounts from opening sidecar files |

The directory is tightened after it is created, including when an older installation already has a permissive directory. The database is checked before SQLite opens it; symbolic links and non-regular paths are rejected. The database file is then restricted to owner read/write access.

Backup and CSV export already use owner-only file/directory creation in the portability module. Keep exported files private; do not commit real habit records or database files.

## Verification on Linux Mint

After launching the app once, inspect the app-owned data directory:

```bash
stat -c '%a %n' "$XDG_DATA_HOME/madhab-habit-tracker" \
  "$XDG_DATA_HOME/madhab-habit-tracker/habit-tracker.sqlite3"
```

When `XDG_DATA_HOME` is unset, use:

```bash
stat -c '%a %n' "$HOME/.local/share/madhab-habit-tracker" \
  "$HOME/.local/share/madhab-habit-tracker/habit-tracker.sqlite3"
```

Expected modes on Linux/Unix are `700` for the directory and `600` for the database. The automated Rust tests also verify these modes and reject directory/database symlinks.

## Security boundary

These permissions reduce exposure to other unprivileged local accounts. They do **not** protect data from:

- another process running as the same user;
- root or an administrator with sufficient privileges;
- malware or a compromised desktop session running as the user;
- physical access to an unencrypted disk, backups or exported files;
- filesystem snapshots or external backup systems that copy the data elsewhere.

For device-loss protection, use full-disk encryption supported by the Linux Mint installation. This change does not add application-level encryption or claim protection against a compromised account.

## Release acceptance

- [ ] Confirm directory mode `0700` and database mode `0600` on the supported Linux Mint release.
- [ ] Confirm an existing permissive data directory is tightened on launch.
- [ ] Confirm a database symlink or non-regular database path causes a clear startup error and is not followed.
- [ ] Confirm backup and CSV export retain their owner-only permissions.
- [ ] Confirm no personal SQLite database, habit labels or notes are present in CI artifacts or repository history.
