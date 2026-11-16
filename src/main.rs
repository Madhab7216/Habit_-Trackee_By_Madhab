pub mod db;
pub mod domain;
pub mod goals;

use crate::domain::Cadence;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Button, Entry, HeaderBar, Label, ListBox, ListBoxRow,
    Orientation, ScrolledWindow,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

const APP_ID: &str = "org.madhab.habittracker";

struct AppState {
    connection: Rc<RefCell<rusqlite::Connection>>,
}

fn main() {
    let application = Application::builder().application_id(APP_ID).build();
    application.connect_activate(build_ui);
    application.run();
}

fn build_ui(application: &Application) {
    let connection = match db::open() {
        Ok(connection) => connection,
        Err(error) => {
            eprintln!("Could not open the local habit database: {error}");
            let window = ApplicationWindow::builder()
                .application(application)
                .title("Habit Trackers by Madhab")
                .default_width(620)
                .default_height(180)
                .build();
            window.set_child(Some(&Label::new(Some(
                "The local database could not be opened. See the terminal for details.",
            ))));
            window.present();
            return;
        }
    };

    let state = Rc::new(AppState {
        connection: Rc::new(RefCell::new(connection)),
    });
    let window = ApplicationWindow::builder()
        .application(application)
        .title("Habit Trackers by Madhab")
        .default_width(1040)
        .default_height(720)
        .build();

    let header = HeaderBar::new();
    header.set_title_widget(Some(&Label::new(Some("Madhab Habit Tracker"))));
    let goals_button = Button::with_label("Goals & milestones");
    let goals_parent = window.clone();
    let goals_connection = Rc::clone(&state.connection);
    goals_button.connect_clicked(move |_| {
        goals::open_window(&goals_parent, Rc::clone(&goals_connection));
    });
    header.pack_end(&goals_button);
    window.set_titlebar(Some(&header));

    let page = gtk::Box::new(Orientation::Vertical, 14);
    page.set_margin_top(24);
    page.set_margin_bottom(24);
    page.set_margin_start(28);
    page.set_margin_end(28);

    let title = Label::new(Some("Today"));
    title.add_css_class("title-1");
    title.set_xalign(0.0);
    let subtitle = Label::new(Some(
        "Small actions, steady progress. Your data stays on this device.",
    ));
    subtitle.add_css_class("dim-label");
    subtitle.set_xalign(0.0);

    let form = gtk::Box::new(Orientation::Vertical, 8);
    let primary_form = gtk::Box::new(Orientation::Horizontal, 8);
    let habit_entry = Entry::new();
    habit_entry.set_placeholder_text(Some("Add a habit…"));
    habit_entry.set_hexpand(true);
    let schedule_picker = gtk::DropDown::from_strings(&[
        "Every day",
        "Monday to Friday",
        "Every 2 days",
        "Times per week",
    ]);
    schedule_picker.set_selected(0);
    schedule_picker.set_tooltip_text(Some("Choose when this habit is due"));
    let weekly_target = gtk::SpinButton::with_range(1.0, 7.0, 1.0);
    weekly_target.set_value(3.0);
    weekly_target.set_tooltip_text(Some("Weekly session target (1–7)"));
    primary_form.append(&habit_entry);
    primary_form.append(&schedule_picker);
    primary_form.append(&weekly_target);

    let detail_form = gtk::Box::new(Orientation::Horizontal, 8);
    let tracking_picker =
        gtk::DropDown::from_strings(&["Binary", "Number", "Count", "Duration", "Limit"]);
    tracking_picker.set_selected(0);
    tracking_picker.set_tooltip_text(Some("Choose how progress is measured"));
    let tracking_group = gtk::Box::new(Orientation::Vertical, 2);
    tracking_group.append(&Label::new(Some("Tracking type")));
    tracking_group.append(&tracking_picker);

    let measurement_fields = gtk::Box::new(Orientation::Horizontal, 8);
    let target_value = gtk::SpinButton::with_range(0.001, 1_000_000_000.0, 0.25);
    target_value.set_digits(2);
    target_value.set_value(1.0);
    target_value.set_tooltip_text(Some("Positive target value"));
    let target_group = gtk::Box::new(Orientation::Vertical, 2);
    target_group.append(&Label::new(Some("Target")));
    target_group.append(&target_value);

    let unit_entry = Entry::new();
    unit_entry.set_placeholder_text(Some("pages, minutes, glasses…"));
    unit_entry.set_width_chars(16);
    unit_entry.set_tooltip_text(Some("Unit for the measured value (up to 24 characters)"));
    let unit_group = gtk::Box::new(Orientation::Vertical, 2);
    unit_group.append(&Label::new(Some("Unit")));
    unit_group.append(&unit_entry);

    let direction_picker = gtk::DropDown::from_strings(&["At least", "At most"]);
    direction_picker.set_selected(0);
    direction_picker.set_tooltip_text(Some(
        "Whether progress should meet or stay under the target",
    ));
    let direction_group = gtk::Box::new(Orientation::Vertical, 2);
    direction_group.append(&Label::new(Some("Target direction")));
    direction_group.append(&direction_picker);

    measurement_fields.append(&target_group);
    measurement_fields.append(&unit_group);
    measurement_fields.append(&direction_group);
    let add_button = Button::with_label("Add habit");
    add_button.add_css_class("suggested-action");
    let archive_view_button = Button::with_label("View archived");
    detail_form.append(&tracking_group);
    detail_form.append(&measurement_fields);
    detail_form.append(&add_button);
    detail_form.append(&archive_view_button);
    let form_feedback = Label::new(None);
    form_feedback.set_xalign(0.0);
    form_feedback.add_css_class("error");

    form.append(&primary_form);
    form.append(&detail_form);
    form.append(&form_feedback);
    update_create_form_visibility(
        &schedule_picker,
        &tracking_picker,
        &weekly_target,
        &measurement_fields,
    );
    let tracking_schedule = schedule_picker.clone();
    let tracking_weekly = weekly_target.clone();
    let tracking_fields = measurement_fields.clone();
    tracking_picker.connect_selected_notify(move |picker| {
        update_create_form_visibility(
            &tracking_schedule,
            picker,
            &tracking_weekly,
            &tracking_fields,
        );
    });
    let schedule_tracking = tracking_picker.clone();
    let schedule_weekly = weekly_target.clone();
    let schedule_fields = measurement_fields.clone();
    schedule_picker.connect_selected_notify(move |picker| {
        update_create_form_visibility(
            picker,
            &schedule_tracking,
            &schedule_weekly,
            &schedule_fields,
        );
    });

    let list = ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");
    let scroller = ScrolledWindow::new();
    scroller.set_vexpand(true);
    scroller.set_child(Some(&list));

    let footer = Label::new(Some(
        "M3 · Reversible habit management · Offline tracking · Your data stays on this device",
    ));
    footer.add_css_class("dim-label");
    footer.set_xalign(0.0);

    page.append(&title);
    page.append(&subtitle);
    page.append(&form);
    page.append(&scroller);
    page.append(&footer);
    window.set_child(Some(&page));

    let archived_mode = Rc::new(Cell::new(false));
    let add_state = Rc::clone(&state);
    let add_list = list.clone();
    let add_entry = habit_entry.clone();
    let add_parent = window.clone();
    let add_mode = Rc::clone(&archived_mode);
    let add_schedule = schedule_picker.clone();
    let add_weekly_target = weekly_target.clone();
    let archive_button_for_add = archive_view_button.clone();
    let add_tracking = tracking_picker.clone();
    let add_target = target_value.clone();
    let add_unit = unit_entry.clone();
    let add_direction = direction_picker.clone();
    let add_feedback = form_feedback.clone();
    add_button.connect_clicked(move |_| {
        let name = add_entry.text().trim().to_string();
        if name.is_empty() {
            add_feedback.set_text("Enter a habit name.");
            return;
        }
        let cadence = match add_schedule.selected() {
            0 => Cadence::Daily,
            1 => Cadence::Weekdays {
                weekday_mask: 0b001_1111,
            },
            2 => Cadence::EveryNDays { interval: 2 },
            3 => Cadence::TimesPerWeek {
                target: add_weekly_target.value().round().clamp(1.0, 7.0) as u8,
            },
            _ => {
                add_feedback.set_text("Select a valid habit schedule.");
                return;
            }
        };
        let tracking_type = match add_tracking.selected() {
            0 => "binary",
            1 => "numeric",
            2 => "count",
            3 => "duration",
            4 => "limit",
            _ => {
                add_feedback.set_text("Select a valid tracking type.");
                return;
            }
        };
        let unit_text = add_unit.text().trim().to_string();
        let is_measured = !matches!(tracking_type, "binary") && add_schedule.selected() != 3;
        let target = is_measured.then_some(add_target.value());
        let unit = is_measured.then_some(unit_text.as_str());
        let direction = if tracking_type == "limit" {
            "at_most"
        } else if add_direction.selected() == 1 {
            "at_most"
        } else {
            "at_least"
        };

        match db::create_habit_with_tracking(
            &add_state.connection.borrow(),
            &name,
            cadence,
            tracking_type,
            target,
            unit,
            direction,
        ) {
            Ok(()) => {
                add_entry.set_text("");
                add_unit.set_text("");
                add_tracking.set_selected(0);
                add_direction.set_selected(0);
                add_feedback.set_text("");
                add_mode.set(false);
                archive_button_for_add.set_label("View archived");
                update_create_form_visibility(
                    &add_schedule,
                    &add_tracking,
                    &add_weekly_target,
                    &measurement_fields,
                );
            }
            Err(error) => add_feedback.set_text(&format!("Could not save habit: {error}")),
        }
        refresh_list(&add_list, &add_state, &add_parent, false);
    });

    let toggle_mode = Rc::clone(&archived_mode);
    let toggle_state = Rc::clone(&state);
    let toggle_list = list.clone();
    let toggle_parent = window.clone();
    let toggle_label = archive_view_button.clone();
    archive_view_button.connect_clicked(move |_| {
        let show_archived = !toggle_mode.get();
        toggle_mode.set(show_archived);
        toggle_label.set_label(if show_archived {
            "Back to today"
        } else {
            "View archived"
        });
        refresh_list(&toggle_list, &toggle_state, &toggle_parent, show_archived);
    });

    refresh_list(&list, &state, &window, false);
    window.present();
}

fn update_create_form_visibility(
    schedule: &gtk::DropDown,
    tracking: &gtk::DropDown,
    weekly_target: &gtk::SpinButton,
    measurement_fields: &gtk::Box,
) {
    let is_weekly_frequency = schedule.selected() == 3;
    weekly_target.set_visible(is_weekly_frequency);
    tracking.set_sensitive(!is_weekly_frequency);
    measurement_fields.set_visible(!is_weekly_frequency && tracking.selected() != 0);
}

fn refresh_list(
    list: &ListBox,
    state: &Rc<AppState>,
    parent: &ApplicationWindow,
    archived_view: bool,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    let habits = if archived_view {
        db::archived_habits(&state.connection.borrow())
    } else {
        db::today_habits(&state.connection.borrow())
    };

    let habits = match habits {
        Ok(habits) => habits,
        Err(error) => {
            append_message(list, &format!("Could not load habits: {error}"));
            return;
        }
    };

    if habits.is_empty() {
        append_message(
            list,
            if archived_view {
                "No archived habits."
            } else {
                "No habits yet. Add one above to begin."
            },
        );
        return;
    }

    for habit in habits {
        let row_content = gtk::Box::new(Orientation::Horizontal, 12);
        row_content.set_margin_top(12);
        row_content.set_margin_bottom(12);
        row_content.set_margin_start(14);
        row_content.set_margin_end(14);

        let name = Label::new(Some(&habit.name));
        name.set_xalign(0.0);
        name.set_hexpand(true);
        row_content.append(&name);

        if archived_view {
            let status = Label::new(Some("Archived"));
            status.add_css_class("dim-label");
            let restore_button = Button::with_label("Restore");
            let restore_state = Rc::clone(state);
            let restore_list = list.clone();
            let restore_parent = parent.clone();
            let habit_id = habit.id;
            restore_button.connect_clicked(move |_| {
                if let Err(error) = db::restore_habit(&restore_state.connection.borrow(), habit_id)
                {
                    eprintln!("Could not restore habit: {error}");
                }
                refresh_list(&restore_list, &restore_state, &restore_parent, true);
            });
            row_content.append(&status);
            row_content.append(&restore_button);
        } else {
            let is_measured = matches!(
                habit.tracking_type.as_str(),
                "numeric" | "count" | "duration" | "limit"
            );
            let status_text = if habit.paused {
                "Paused"
            } else if habit.entry_status.as_deref() == Some("partial") {
                "Partial"
            } else if habit.entry_status.as_deref() == Some("skipped") {
                "Skipped"
            } else if habit.completed {
                "Completed"
            } else if !habit.scheduled {
                "Not scheduled"
            } else {
                "Due today"
            };
            let status = Label::new(Some(status_text));
            status.add_css_class("dim-label");
            row_content.append(&status);

            let edit_button = Button::with_label("Edit");
            edit_button.set_tooltip_text(Some("Edit the habit name"));
            let edit_state = Rc::clone(state);
            let edit_list = list.clone();
            let edit_parent = parent.clone();
            let habit_id = habit.id;
            let current_name = habit.name.clone();
            edit_button.connect_clicked(move |_| {
                show_edit_window(
                    &edit_parent,
                    &edit_list,
                    &edit_state,
                    habit_id,
                    &current_name,
                );
            });

            if is_measured {
                let current = habit
                    .recorded_value
                    .map(|value| format!("{value:.2}"))
                    .unwrap_or_else(|| "—".to_string());
                let target = habit
                    .target_value
                    .map(|value| format!("{value:.2}"))
                    .unwrap_or_else(|| "—".to_string());
                let unit = habit.unit.as_deref().unwrap_or("");
                let detail = Label::new(Some(&format!("{current} / {target} {unit}")));
                detail.add_css_class("dim-label");
                row_content.append(&detail);

                let record_button = Button::with_label(if habit.recorded_value.is_some() {
                    "Update"
                } else {
                    "Record"
                });
                record_button.set_sensitive(!habit.paused && habit.scheduled);
                let record_state = Rc::clone(state);
                let record_list = list.clone();
                let record_parent = parent.clone();
                let habit_id = habit.id;
                let habit_name = habit.name.clone();
                let target_value = habit.target_value.unwrap_or(1.0);
                let unit = habit.unit.clone().unwrap_or_default();
                let direction = habit.direction.clone();
                let current_value = habit.recorded_value;
                record_button.connect_clicked(move |_| {
                    show_measurement_window(
                        &record_parent,
                        &record_list,
                        &record_state,
                        habit_id,
                        &habit_name,
                        target_value,
                        &unit,
                        &direction,
                        current_value,
                    );
                });
                row_content.append(&record_button);
            } else {
                let action = Button::with_label(if habit.completed { "Undo" } else { "Mark done" });
                action.set_sensitive(habit.completed || (!habit.paused && habit.scheduled));
                let action_state = Rc::clone(state);
                let action_list = list.clone();
                let action_parent = parent.clone();
                let habit_id = habit.id;
                let completed = habit.completed;
                action.connect_clicked(move |_| {
                    if let Err(error) = db::set_today_completed(
                        &action_state.connection.borrow(),
                        habit_id,
                        !completed,
                    ) {
                        eprintln!("Could not update habit: {error}");
                    }
                    refresh_list(&action_list, &action_state, &action_parent, false);
                });
                row_content.append(&action);
            }

            let pause_action = Button::with_label(if habit.paused { "Resume" } else { "Pause" });
            let pause_state = Rc::clone(state);
            let pause_list = list.clone();
            let pause_parent = parent.clone();
            let pause_habit_id = habit.id;
            let paused = habit.paused;
            pause_action.connect_clicked(move |_| {
                if let Err(error) =
                    db::set_paused(&pause_state.connection.borrow(), pause_habit_id, !paused)
                {
                    eprintln!("Could not change pause state: {error}");
                }
                refresh_list(&pause_list, &pause_state, &pause_parent, false);
            });

            let archive_action = Button::with_label("Archive");
            let archive_state = Rc::clone(state);
            let archive_list = list.clone();
            let archive_parent = parent.clone();
            let archive_habit_id = habit.id;
            archive_action.connect_clicked(move |_| {
                if let Err(error) =
                    db::archive_habit(&archive_state.connection.borrow(), archive_habit_id)
                {
                    eprintln!("Could not archive habit: {error}");
                }
                refresh_list(&archive_list, &archive_state, &archive_parent, false);
            });

            row_content.append(&edit_button);
            row_content.append(&pause_action);
            row_content.append(&archive_action);
        }

        let row = ListBoxRow::new();
        row.set_child(Some(&row_content));
        list.append(&row);
    }
}

fn show_edit_window(
    parent: &ApplicationWindow,
    list: &ListBox,
    state: &Rc<AppState>,
    habit_id: i64,
    current_name: &str,
) {
    let editor = gtk::Window::builder()
        .title("Edit habit")
        .transient_for(parent)
        .modal(true)
        .default_width(420)
        .default_height(180)
        .build();

    let content = gtk::Box::new(Orientation::Vertical, 12);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(20);
    content.set_margin_end(20);

    let label = Label::new(Some("Habit name"));
    label.set_xalign(0.0);
    let entry = Entry::new();
    entry.set_text(current_name);
    entry.set_activates_default(true);
    let feedback = Label::new(None);
    feedback.set_xalign(0.0);
    feedback.add_css_class("error");

    let actions = gtk::Box::new(Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel_button = Button::with_label("Cancel");
    let save_button = Button::with_label("Save");
    save_button.add_css_class("suggested-action");
    actions.append(&cancel_button);
    actions.append(&save_button);

    content.append(&label);
    content.append(&entry);
    content.append(&feedback);
    content.append(&actions);
    editor.set_child(Some(&content));

    let save_editor = editor.clone();
    let save_state = Rc::clone(state);
    let save_list = list.clone();
    let save_parent = parent.clone();
    let save_entry = entry.clone();
    let save_feedback = feedback.clone();
    save_button.connect_clicked(move |_| {
        let name = save_entry.text().to_string();
        match db::rename_habit(&save_state.connection.borrow(), habit_id, &name) {
            Ok(()) => {
                save_editor.close();
                refresh_list(&save_list, &save_state, &save_parent, false);
            }
            Err(error) => save_feedback.set_text(&format!("Could not save habit: {error}")),
        }
    });

    let cancel_editor = editor.clone();
    cancel_button.connect_clicked(move |_| cancel_editor.close());

    editor.present();
    entry.grab_focus();
}

fn show_measurement_window(
    parent: &ApplicationWindow,
    list: &ListBox,
    state: &Rc<AppState>,
    habit_id: i64,
    habit_name: &str,
    target_value: f64,
    unit: &str,
    direction: &str,
    current_value: Option<f64>,
) {
    let editor = gtk::Window::builder()
        .title("Record habit progress")
        .transient_for(parent)
        .modal(true)
        .default_width(460)
        .default_height(260)
        .build();

    let content = gtk::Box::new(Orientation::Vertical, 12);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(20);
    content.set_margin_end(20);

    let title = Label::new(Some(&format!("Record {habit_name}")));
    title.add_css_class("title-2");
    title.set_xalign(0.0);
    let direction_text = if direction == "at_most" {
        "at most"
    } else {
        "at least"
    };
    let target_label = Label::new(Some(&format!(
        "Target: {direction_text} {target_value:.2} {unit}"
    )));
    target_label.set_xalign(0.0);
    target_label.add_css_class("dim-label");

    let value_label = Label::new(Some(&format!("Actual value ({unit})")));
    value_label.set_xalign(0.0);
    let actual_value = gtk::SpinButton::with_range(0.0, 1_000_000_000.0, 0.25);
    actual_value.set_digits(2);
    actual_value.set_value(current_value.unwrap_or(0.0));
    actual_value.set_tooltip_text(Some("Enter the value achieved today"));

    let feedback = Label::new(None);
    feedback.set_xalign(0.0);
    feedback.add_css_class("error");

    let actions = gtk::Box::new(Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel_button = Button::with_label("Cancel");
    let save_button = Button::with_label("Save progress");
    save_button.add_css_class("suggested-action");
    actions.append(&cancel_button);
    actions.append(&save_button);

    content.append(&title);
    content.append(&target_label);
    content.append(&value_label);
    content.append(&actual_value);
    content.append(&feedback);
    content.append(&actions);
    editor.set_child(Some(&content));

    let save_editor = editor.clone();
    let save_state = Rc::clone(state);
    let save_list = list.clone();
    let save_parent = parent.clone();
    let save_feedback = feedback.clone();
    let save_value = actual_value.clone();
    save_button.connect_clicked(move |_| {
        match db::record_measurement_for_today(
            &save_state.connection.borrow(),
            habit_id,
            save_value.value(),
        ) {
            Ok(_) => {
                save_editor.close();
                refresh_list(&save_list, &save_state, &save_parent, false);
            }
            Err(error) => save_feedback.set_text(&format!("Could not save progress: {error}")),
        }
    });

    let cancel_editor = editor.clone();
    cancel_button.connect_clicked(move |_| cancel_editor.close());

    editor.present();
    actual_value.grab_focus();
}

fn append_message(list: &ListBox, message: &str) {
    let label = Label::new(Some(message));
    label.set_xalign(0.0);
    label.set_margin_top(16);
    label.set_margin_bottom(16);
    label.set_margin_start(16);
    label.set_margin_end(16);
    let row = ListBoxRow::new();
    row.set_child(Some(&label));
    list.append(&row);
}
