pub mod db;
pub mod domain;
pub mod goals;
pub mod measurable_habits;

use crate::domain::Cadence;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Button, Entry, HeaderBar, Label, ListBox, ListBoxRow,
    Orientation, ScrolledWindow, SpinButton,
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
    let habit_row = gtk::Box::new(Orientation::Horizontal, 8);
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
    let weekly_target = SpinButton::with_range(1.0, 7.0, 1.0);
    weekly_target.set_value(3.0);
    weekly_target.set_tooltip_text(Some("Weekly session target (1–7), used for Times per week"));
    habit_row.append(&habit_entry);
    habit_row.append(&schedule_picker);
    habit_row.append(&weekly_target);

    let measurement_row = gtk::Box::new(Orientation::Horizontal, 8);
    let tracking_picker = gtk::DropDown::from_strings(&[
        "Binary",
        "Numeric target",
        "Duration",
        "Count",
        "Limit / avoid",
    ]);
    tracking_picker.set_tooltip_text(Some("Choose a checkbox or a measured habit"));
    let direction_picker = gtk::DropDown::from_strings(&["At least", "At most"]);
    direction_picker.set_tooltip_text(Some("At least = higher is better; At most = stay under a limit"));
    let target_input = SpinButton::with_range(0.1, 1_000_000.0, 0.1);
    target_input.set_value(1.0);
    target_input.set_digits(2);
    target_input.set_tooltip_text(Some("Target value; zero targets are not supported by scoring v1"));
    let unit_input = Entry::new();
    unit_input.set_placeholder_text(Some("Unit (ml, minutes, pages…)"));
    unit_input.set_width_chars(18);
    unit_input.set_hexpand(true);
    measurement_row.append(&tracking_picker);
    measurement_row.append(&direction_picker);
    measurement_row.append(&target_input);
    measurement_row.append(&unit_input);
    measurement_row.set_visible(false);

    let tracking_row = measurement_row.clone();
    let direction_for_tracking = direction_picker.clone();
    tracking_picker.connect_selected_notify(move |picker| {
        let is_measured = picker.selected() != 0;
        tracking_row.set_visible(is_measured);
        direction_for_tracking.set_sensitive(picker.selected() != 4);
        if picker.selected() == 4 {
            direction_for_tracking.set_selected(1);
        }
    });

    let action_row = gtk::Box::new(Orientation::Horizontal, 8);
    let add_button = Button::with_label("Add habit");
    add_button.add_css_class("suggested-action");
    let archive_view_button = Button::with_label("View archived");
    action_row.append(&add_button);
    action_row.append(&archive_view_button);
    let creation_feedback = Label::new(None);
    creation_feedback.set_xalign(0.0);
    creation_feedback.add_css_class("error");

    form.append(&habit_row);
    form.append(&measurement_row);
    form.append(&action_row);
    form.append(&creation_feedback);

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
    let add_tracking = tracking_picker.clone();
    let add_direction = direction_picker.clone();
    let add_target = target_input.clone();
    let add_unit = unit_input.clone();
    let add_feedback = creation_feedback.clone();
    let archive_button_for_add = archive_view_button.clone();
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

        let result: Result<(), String> = match add_tracking.selected() {
            0 => db::create_habit_with_cadence(
                &add_state.connection.borrow(),
                &name,
                cadence,
            )
            .map_err(|error| error.to_string()),
            1..=4 => {
                let kind = match add_tracking.selected() {
                    1 => measured_habits::MeasurementKind::Numeric,
                    2 => measured_habits::MeasurementKind::Duration,
                    3 => measured_habits::MeasurementKind::Count,
                    4 => measured_habits::MeasurementKind::Limit,
                    _ => unreachable!("tracking type was range-checked"),
                };
                let direction = if add_tracking.selected() == 4 || add_direction.selected() == 1 {
                    measured_habits::MeasurementDirection::AtMost
                } else {
                    measured_habits::MeasurementDirection::AtLeast
                };
                let spec = measured_habits::MeasurementSpec {
                    kind,
                    direction,
                    target: add_target.value(),
                    unit: add_unit.text().to_string(),
                };
                measured_habits::create_measurable_habit(
                    &mut add_state.connection.borrow_mut(),
                    &name,
                    cadence,
                    spec,
                )
                .map(|_| ())
                .map_err(|error| error.to_string())
            }
            _ => Err("Select a valid habit type.".to_string()),
        };

        match result {
            Ok(()) => {
                add_entry.set_text("");
                add_unit.set_text("");
                add_feedback.set_text("");
                add_mode.set(false);
                archive_button_for_add.set_label("View archived");
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
            let measurement_config =
                measured_habits::measurement_config(&state.connection.borrow(), habit.id).ok();
            let measurement_date =
                measured_habits::local_today_date(&state.connection.borrow()).ok();
            let measurement_record = if measurement_config.is_some() {
                measurement_date.and_then(|date| {
                    measured_habits::measurement_for_date(
                        &state.connection.borrow(),
                        habit.id,
                        date,
                    )
                    .ok()
                    .flatten()
                })
            } else {
                None
            };
            let status_text = if habit.paused {
                "Paused".to_string()
            } else if !habit.scheduled {
                "Not scheduled".to_string()
            } else if let (Some(config), Some(record)) =
                (measurement_config.as_ref(), measurement_record.as_ref())
            {
                let state = match record.status {
                    measured_habits::MeasurementStatus::Completed => "Completed",
                    measured_habits::MeasurementStatus::Partial => "Partial",
                };
                format!("{state} · {} {}", record.actual, config.unit)
            } else if habit.completed {
                "Completed".to_string()
            } else {
                "Due today".to_string()
            };
            let status = Label::new(Some(&status_text));
            status.add_css_class("dim-label");

            let edit_button = Button::with_label("Edit");
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

            if let Some(config) = measurement_config {
                let target_operator = if config.direction == measured_habits::MeasurementDirection::AtMost {
                    "≤"
                } else {
                    "≥"
                };
                let target_label = Label::new(Some(&format!(
                    "Target {target_operator} {} {}",
                    config.target, config.unit
                )));
                target_label.add_css_class("dim-label");
                row_content.append(&target_label);

                let log_action = Button::with_label(if measurement_record.is_some() {
                    "Update value"
                } else {
                    "Log value"
                });
                log_action.set_sensitive(!habit.paused && habit.scheduled);
                let log_state = Rc::clone(state);
                let log_list = list.clone();
                let log_parent = parent.clone();
                let log_habit_id = habit.id;
                let log_habit_name = habit.name.clone();
                log_action.connect_clicked(move |_| {
                    show_measurement_window(
                        &log_parent,
                        &log_list,
                        &log_state,
                        log_habit_id,
                        &log_habit_name,
                    );
                });
                row_content.append(&log_action);

                if measurement_record.is_some() {
                    let undo_action = Button::with_label("Undo value");
                    let undo_state = Rc::clone(state);
                    let undo_list = list.clone();
                    let undo_parent = parent.clone();
                    let undo_habit_id = habit.id;
                    undo_action.connect_clicked(move |_| {
                        match measured_habits::local_today_date(&undo_state.connection.borrow())
                            .and_then(|date| {
                                measured_habits::undo_measurement_for_date(
                                    &undo_state.connection.borrow(),
                                    undo_habit_id,
                                    date,
                                )
                            }) {
                            Ok(_) => refresh_list(&undo_list, &undo_state, &undo_parent, false),
                            Err(error) => eprintln!("Could not undo measurement: {error}"),
                        }
                    });
                    row_content.append(&undo_action);
                }
            } else {
                let action = Button::with_label(if habit.completed { "Undo" } else { "Mark done" });
                action.set_sensitive(habit.completed || (!habit.paused && habit.scheduled));
                let action_state = Rc::clone(state);
                let action_list = list.clone();
                let action_parent = parent.clone();
                let habit_id = habit.id;
                let completed = habit.completed;
                action.connect_clicked(move |_| {
                    if let Err(error) =
                        db::set_today_completed(&action_state.connection.borrow(), habit_id, !completed)
                    {
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

            row_content.append(&status);
            row_content.append(&edit_button);
            row_content.append(&pause_action);
            row_content.append(&archive_action);
        }

        let row = ListBoxRow::new();
        row.set_child(Some(&row_content));
        list.append(&row);
    }
}

fn show_measurement_window(
    parent: &ApplicationWindow,
    list: &ListBox,
    state: &Rc<AppState>,
    habit_id: i64,
    habit_name: &str,
) {
    let connection = state.connection.borrow();
    let config = match measured_habits::measurement_config(&connection, habit_id) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Could not load measurement settings: {error}");
            return;
        }
    };
    let date = match measured_habits::local_today_date(&connection) {
        Ok(date) => date,
        Err(error) => {
            eprintln!("Could not read today's local date: {error}");
            return;
        }
    };
    let saved = measured_habits::measurement_for_date(&connection, habit_id, date).ok().flatten();
    drop(connection);

    let editor = gtk::Window::builder()
        .title(format!("Record value · {habit_name}"))
        .transient_for(parent)
        .modal(true)
        .default_width(440)
        .default_height(280)
        .build();

    let content = gtk::Box::new(Orientation::Vertical, 12);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(20);
    content.set_margin_end(20);

    let operator = if config.direction == measured_habits::MeasurementDirection::AtMost {
        "≤"
    } else {
        "≥"
    };
    let target = Label::new(Some(&format!(
        "Target: {operator} {} {}",
        config.target, config.unit
    )));
    target.set_xalign(0.0);
    target.add_css_class("dim-label");

    let value_label = Label::new(Some(&format!("Today's value ({})", config.unit)));
    value_label.set_xalign(0.0);
    let step = if config.kind == measured_habits::MeasurementKind::Count {
        1.0
    } else {
        0.1
    };
    let value_input = SpinButton::with_range(0.0, 1_000_000_000.0, step);
    value_input.set_digits(if config.kind == measured_habits::MeasurementKind::Count {
        0
    } else {
        2
    });
    if let Some(record) = &saved {
        value_input.set_value(record.actual);
    } else {
        value_input.set_value(0.0);
    }

    let note_input = Entry::new();
    note_input.set_placeholder_text(Some("Optional private note (stored locally)"));
    note_input.set_max_length(500);
    if let Some(record) = &saved {
        note_input.set_text(&record.note);
    }

    let feedback = Label::new(None);
    feedback.set_xalign(0.0);
    feedback.add_css_class("dim-label");

    let actions = gtk::Box::new(Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel_button = Button::with_label("Cancel");
    let save_button = Button::with_label("Save value");
    save_button.add_css_class("suggested-action");
    actions.append(&cancel_button);
    actions.append(&save_button);

    content.append(&target);
    content.append(&value_label);
    content.append(&value_input);
    content.append(&note_input);
    content.append(&feedback);
    content.append(&actions);
    editor.set_child(Some(&content));

    let save_editor = editor.clone();
    let save_state = Rc::clone(state);
    let save_list = list.clone();
    let save_parent = parent.clone();
    let save_value = value_input.clone();
    let save_note = note_input.clone();
    let save_feedback = feedback.clone();
    save_button.connect_clicked(move |_| {
        match measured_habits::record_measurement_for_date(
            &save_state.connection.borrow(),
            habit_id,
            date,
            save_value.value(),
            &save_note.text(),
        ) {
            Ok(outcome) => {
                let status = match outcome.status {
                    measured_habits::MeasurementStatus::Completed => "Completed",
                    measured_habits::MeasurementStatus::Partial => "Partial",
                };
                save_feedback.set_text(&format!(
                    "{status} · {:.0}% of target saved",
                    outcome.achievement_ratio * 100.0
                ));
                refresh_list(&save_list, &save_state, &save_parent, false);
                save_editor.close();
            }
            Err(error) => save_feedback.set_text(&format!("Could not save value: {error}")),
        }
    });

    let cancel_editor = editor.clone();
    cancel_button.connect_clicked(move |_| cancel_editor.close());

    editor.present();
    value_input.grab_focus();
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
