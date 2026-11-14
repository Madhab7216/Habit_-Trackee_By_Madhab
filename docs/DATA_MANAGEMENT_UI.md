# M8 — GTK data management window

The main application header exposes a **Data & backups** window so local data portability does not require a terminal.

## Available operations

- **Create backup:** choose a destination filename; the SQLite Online Backup API creates and verifies a new backup.
- **Verify backup:** select an existing backup for read-only validation.
- **Restore to a new file:** select a verified source backup and a separate new destination. This does not switch or overwrite the active application database.
- **Export CSV:** choose a new folder name for a consistent CSV snapshot of supported habit, entry, goal, milestone and settings tables.

All operations reuse the existing portability service and its safety rules. The UI does not upload records or make network requests. Existing output paths are rejected instead of overwritten. The user chooses a private destination because exported labels and notes may contain sensitive personal information.

## Manual acceptance checks

On Linux Mint, open **Data & backups** from the main application header and verify the four actions:

1. Create a backup in a new folder, then verify it.
2. Try to create a second backup at the same path and confirm the operation is rejected without changing the original.
3. Restore to a new destination and verify the restored file; confirm the app continues using the original active database.
4. Export CSV to a new folder, open the manifest and check that the milestone table is included when present.

This PR provides the UI and CI compilation/testing. Interactive file chooser behavior and end-to-end manual acceptance still need testing on the target Linux Mint workstation.
