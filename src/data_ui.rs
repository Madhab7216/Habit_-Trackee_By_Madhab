//! GTK user interface for local backup, verification, restore and CSV export.

use crate::portability;
use gtk::prelude::*;
use gtk::{
    ApplicationWindow, Box as GtkBox, Button, FileChooserAction, FileChooserNative, Label,
    Orientation, ResponseType, Window,
};
use rusqlite::Connection;
use std::{cell::RefCell, path::PathBuf, rc::Rc};

fn action_row(title: &str, description: &str, action: &str) -> (GtkBox, Button) {
    let row = GtkBox::new(Orientation::Horizontal, 14);
    row.set_spacing(14);

    let details = GtkBox::new(Orientation::Vertical, 4);
    details.set_hexpand(true);

    let heading = Label::new(Some(title));
    heading.add_css_class("heading");
    heading.set_xalign(0.0);

    let explanation = Label::new(Some(description));
    explanation.add_css_class("dim-label");
    explanation.set_xalign(0.0);
    explanation.set_wrap(true);

    let button = Button::with_label(action);
    button.set_valign(gtk::Align::Center);

    details.append(&heading);
    details.append(&explanation);
    row.append(&details);
    row.append(&button);
    (row, button)
}

fn chooser(
    parent: &Window,
    title: &str,
    action: FileChooserAction,
    accept_label: &str,
    default_name: Option<&str>,
) -> FileChooserNative {
    let dialog = FileChooserNative::new(
        Some(title),
        Some(parent),
        action,
        Some(accept_label),
        Some("Cancel"),
    );
    if let Some(name) = default_name {
        dialog.set_current_name(name);
    }
    dialog
}

fn selected_path(dialog: &FileChooserNative) -> Option<PathBuf> {
    dialog.file().and_then(|file| file.path())
}

fn set_status(status: &Label, message: &str, success: bool) {
    status.remove_css_class("success");
    status.remove_css_class("error");
    status.set_text(message);
    status.add_css_class(if success { "success" } else { "error" });
}

fn show_operation_result(
    status: &Label,
    success_message: &str,
    result: portability::PortabilityResult<()>,
) {
    match result {
        Ok(()) => set_status(status, success_message, true),
        Err(error) => set_status(status, &format!("Operation failed: {error}"), false),
    }
}

pub fn open_window(parent: &ApplicationWindow, connection: Rc<RefCell<Connection>>) {
    let window = Window::builder()
        .title("Data & backups")
        .transient_for(parent)
        .default_width(760)
        .default_height(520)
        .build();

    let root = GtkBox::new(Orientation::Vertical, 16);
    root.set_margin_top(24);
    root.set_margin_bottom(24);
    root.set_margin_start(28);
    root.set_margin_end(28);

    let title = Label::new(Some("Your data stays under your control"));
    title.add_css_class("title-1");
    title.set_xalign(0.0);

    let subtitle = Label::new(Some(
        "Create a verified backup, check an existing backup, restore to a separate file, or export your records as CSV.",
    ));
    subtitle.add_css_class("dim-label");
    subtitle.set_xalign(0.0);
    subtitle.set_wrap(true);

    let (backup_row, backup_button) = action_row(
        "Create verified backup",
        "Create a new SQLite backup and verify its integrity before reporting success.",
        "Create backup…",
    );
    let (verify_row, verify_button) = action_row(
        "Verify backup",
        "Read a selected backup without changing it and check its database integrity.",
        "Verify file…",
    );
    let (restore_row, restore_button) = action_row(
        "Restore to a new file",
        "Choose a backup and a new destination. The active application database is never replaced.",
        "Restore…",
    );
    let (export_row, export_button) = action_row(
        "Export CSV bundle",
        "Write a consistent snapshot of supported habits, entries, goals, milestones and settings.",
        "Export…",
    );

    let status = Label::new(Some(
        "Choose an operation. Existing destination files and export folders are never overwritten.",
    ));
    status.set_xalign(0.0);
    status.set_wrap(true);
    status.add_css_class("dim-label");

    let privacy_note = Label::new(Some(
        "Privacy note: exported habit names and notes can be sensitive. Choose a private destination and keep backup files secure.",
    ));
    privacy_note.set_xalign(0.0);
    privacy_note.set_wrap(true);
    privacy_note.add_css_class("dim-label");

    root.append(&title);
    root.append(&subtitle);
    root.append(&backup_row);
    root.append(&verify_row);
    root.append(&restore_row);
    root.append(&export_row);
    root.append(&status);
    root.append(&privacy_note);
    window.set_child(Some(&root));

    let backup_parent = window.clone();
    let backup_status = status.clone();
    let backup_connection = Rc::clone(&connection);
    backup_button.connect_clicked(move |_| {
        let dialog = chooser(
            &backup_parent,
            "Save verified database backup",
            FileChooserAction::Save,
            "Save backup",
            Some("madhab-habits-backup.sqlite3"),
        );
        let status = backup_status.clone();
        let connection = Rc::clone(&backup_connection);
        dialog.connect_response(move |dialog, response| {
            if response != ResponseType::Accept {
                return;
            }
            let Some(destination) = selected_path(dialog) else {
                set_status(&status, "No valid backup destination was selected.", false);
                return;
            };
            let result = portability::create_verified_backup(&connection.borrow(), &destination);
            show_operation_result(&status, "Backup created and verified.", result);
        });
        dialog.show();
    });

    let verify_parent = window.clone();
    let verify_status = status.clone();
    verify_button.connect_clicked(move |_| {
        let dialog = chooser(
            &verify_parent,
            "Choose a database backup to verify",
            FileChooserAction::Open,
            "Verify backup",
            None,
        );
        let status = verify_status.clone();
        dialog.connect_response(move |dialog, response| {
            if response != ResponseType::Accept {
                return;
            }
            let Some(path) = selected_path(dialog) else {
                set_status(&status, "No valid backup file was selected.", false);
                return;
            };
            match portability::verify_backup_file(&path) {
                Ok(version) => set_status(
                    &status,
                    &format!("Backup verified. Supported schema version: {version}."),
                    true,
                ),
                Err(error) => set_status(
                    &status,
                    &format!("Backup verification failed: {error}"),
                    false,
                ),
            }
        });
        dialog.show();
    });

    let restore_parent = window.clone();
    let restore_status = status.clone();
    restore_button.connect_clicked(move |_| {
        let source_dialog = chooser(
            &restore_parent,
            "Choose the backup to restore",
            FileChooserAction::Open,
            "Choose backup",
            None,
        );
        let parent_for_destination = restore_parent.clone();
        let status_for_source = restore_status.clone();
        source_dialog.connect_response(move |source_dialog, response| {
            if response != ResponseType::Accept {
                return;
            }
            let Some(source) = selected_path(source_dialog) else {
                set_status(&status_for_source, "No valid backup file was selected.", false);
                return;
            };

            let destination_dialog = chooser(
                &parent_for_destination,
                "Choose a new restore destination",
                FileChooserAction::Save,
                "Restore to new file",
                Some("madhab-habits-restored.sqlite3"),
            );
            let status = status_for_source.clone();
            destination_dialog.connect_response(move |destination_dialog, response| {
                if response != ResponseType::Accept {
                    return;
                }
                let Some(destination) = selected_path(destination_dialog) else {
                    set_status(&status, "No valid restore destination was selected.", false);
                    return;
                };
                let result = portability::restore_backup_to_new_file(&source, &destination);
                show_operation_result(
                    &status,
                    "Backup restored to a separate new database file. The active app database was not changed.",
                    result,
                );
            });
            destination_dialog.show();
        });
        source_dialog.show();
    });

    let export_parent = window.clone();
    let export_status = status.clone();
    let export_connection = Rc::clone(&connection);
    export_button.connect_clicked(move |_| {
        let dialog = chooser(
            &export_parent,
            "Choose a new CSV export folder name",
            FileChooserAction::Save,
            "Create CSV export",
            Some("Madhab-Habit-Export"),
        );
        let status = export_status.clone();
        let connection = Rc::clone(&export_connection);
        dialog.connect_response(move |dialog, response| {
            if response != ResponseType::Accept {
                return;
            }
            let Some(destination) = selected_path(dialog) else {
                set_status(
                    &status,
                    "No valid CSV export destination was selected.",
                    false,
                );
                return;
            };
            let result = portability::export_csv_bundle(&mut connection.borrow_mut(), &destination);
            show_operation_result(&status, "CSV export created successfully.", result);
        });
        dialog.show();
    });

    window.present();
}
