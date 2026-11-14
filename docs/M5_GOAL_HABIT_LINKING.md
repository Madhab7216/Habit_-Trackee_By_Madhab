# M5 — Goal-to-habit linking UI

## User workflow

1. Open **Goals & milestones** from the Today window.
2. Choose **Link habits** on a goal card.
3. Select any non-archived habits that support that goal. Existing links are preselected.
4. Choose **Save links** to replace the selection, or **Cancel** to discard the dialog changes.

The picker does not show archived habits. Paused habits remain available so a temporary pause does not erase a user's planning relationship. Archived goals cannot be edited from the archived view.

## Data integrity

- The selected set is validated and replaced inside a single SQLite transaction.
- A missing/archived habit or archived/missing goal aborts the transaction; previous links remain intact.
- Duplicate IDs are normalized before writing.
- Removing a link does not delete the habit, its check-ins, goal, or milestones.
- Linking is organizational only. Goal progress remains milestone-based and does not increase automatically from habit check-ins.

## Accessibility and privacy

The picker uses GTK4 CheckButton controls, which expose checkbox semantics and support Space/Enter keyboard activation. All names are read from the local SQLite database; no habit labels or personal history are sent to a service.

## Validation

Rust tests cover archived-habit filtering, existing-selection state, deduplication, replacement, and rollback on invalid selections. GTK interactive behavior still requires manual launch verification on the target Linux Mint workstation.
