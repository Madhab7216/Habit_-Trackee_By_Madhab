pub mod db;
pub mod domain;
pub mod goals;
pub mod schedule_history;
pub mod timezone;

use crate::domain::Cadence;
use crate::schedule_history::{
    NewScheduleRevision, PastEditConfirmation, Recurrence, TimeZoneStatus, append_revision,
    revisions_for_habit,
};
use crate::timezone::IanaTimeZone;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Button, CheckButton, ComboBoxText, Entry, HeaderBar, Label,
    ListBox, ListBoxRow, Orientation, ScrolledWindow,
};
use jiff::{Timestamp, civil::Date};
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

    let form = gtk::Box::new(Orientation::Horizontal, 8);
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
    let weekly_target = gtk::SpinButton::with_range(1.0, 7.0, 1.0);
    weekly_target.set_value(3.0);
    weekly_target.set_tooltip_text(Some("Weekly session target (1–7), used for Times per week"));
    let add_button = Button::with_label("Add habit");
    add_button.add_css_class("suggested-action");
    let archive_view_button = Button::with_label("View archived");
    form.append(&habit_entry);
    form.append(&schedule_picker);
    form.append(&weekly_target);
    form.append(&add_button);
    form.append(&archive_view_button);

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
    add_button.connect_clicked(move |_| {
        let name = add_entry.text().trim().to_string();
        if name.is_empty() {
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
                eprintln!("Select a valid habit schedule.");
                return;
            }
        };
        match db::create_habit_with_cadence(&add_state.connection.borrow(), &name, cadence) {
            Ok(()) => {
                add_entry.set_text("");
                add_mode.set(false);
                archive_button_for_add.set_label("View archived");
            }
            Err(error) => eprintln!("Could not save habit: {error}"),
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
            let status = Label::new(Some(if habit.paused {
                "Paused"
            } else if habit.completed {
                "Completed"
            } else if !habit.scheduled {
                "Not scheduled"
            } else {
                "Due today"
            }));
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

            let schedule_button = Button::with_label("Schedule");
            let schedule_state = Rc::clone(state);
            let schedule_list = list.clone();
            let schedule_parent = parent.clone();
            let schedule_habit_id = habit.id;
            let schedule_habit_name = habit.name.clone();
            schedule_button.connect_clicked(move |_| {
                show_schedule_window(
                    &schedule_parent,
                    &schedule_list,
                    &schedule_state,
                    schedule_habit_id,
                    &schedule_habit_name,
                );
            });

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
            row_content.append(&schedule_button);
            row_content.append(&action);
            row_content.append(&pause_action);
            row_content.append(&archive_action);
        }

        let row = ListBoxRow::new();
        row.set_child(Some(&row_content));
        list.append(&row);
    }
}

fn show_schedule_window(
    parent: &ApplicationWindow,
    list: &ListBox,
    state: &Rc<AppState>,
    habit_id: i64,
    habit_name: &str,
) {
    let revisions = {
        let mut connection = state.connection.borrow_mut();
        revisions_for_habit(&mut connection, habit_id)
    };
    let latest = match revisions {
        Ok(items) => items.last().cloned(),
        Err(error) => {
            eprintln!("Could not read schedule history: {error}");
            None
        }
    };

    let system_zone = IanaTimeZone::system().ok();
    let default_zone = latest
        .as_ref()
        .filter(|revision| revision.time_zone_status == TimeZoneStatus::ValidatedIana)
        .map(|revision| revision.time_zone_id.clone())
        .or_else(|| system_zone.as_ref().map(|zone| zone.id().to_owned()))
        .unwrap_or_default();
    let selected_zone = IanaTimeZone::parse(&default_zone).ok();
    let today_in_zone = selected_zone
        .as_ref()
        .map(|zone| zone.local_date_at(Timestamp::now()));

    let default_effective_date = {
        let base_date = latest
            .as_ref()
            .map(|revision| revision.end_date.unwrap_or(revision.effective_from))
            .into_iter()
            .chain(today_in_zone)
            .max();
        base_date
            .and_then(|date| {
                state
                    .connection
                    .borrow()
                    .query_row("SELECT date(?1, '+1 day')", [date.to_string()], |row| {
                        row.get::<_, String>(0)
                    })
                    .ok()
            })
            .unwrap_or_default()
    };

    let editor = gtk::Window::builder()
        .title(format!("Change schedule — {habit_name}"))
        .transient_for(parent)
        .modal(true)
        .default_width(520)
        .default_height(680)
        .build();

    let content = gtk::Box::new(Orientation::Vertical, 10);
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    content.set_margin_start(20);
    content.set_margin_end(20);

    let current_label = Label::new(Some(
        &latest
            .as_ref()
            .map(|revision| {
                format!(
                    "Latest saved rule: {:?}, from {}, timezone {} ({:?})",
                    revision.recurrence,
                    revision.effective_from,
                    revision.time_zone_id,
                    revision.time_zone_status
                )
            })
            .unwrap_or_else(|| "No schedule revision found.".to_owned()),
    ));
    current_label.set_xalign(0.0);
    current_label.set_wrap(true);

    let intro = Label::new(Some(
        "A saved schedule is immutable. New rules start on the selected date; earlier check-ins are retained.",
    ));
    intro.set_xalign(0.0);
    intro.set_wrap(true);
    intro.add_css_class("dim-label");

    let effective_label = Label::new(Some("Effective from (YYYY-MM-DD)"));
    effective_label.set_xalign(0.0);
    let effective_entry = Entry::new();
    effective_entry.set_placeholder_text(Some("2026-11-18"));
    effective_entry.set_text(&default_effective_date);

    let cadence_label = Label::new(Some("Recurrence"));
    cadence_label.set_xalign(0.0);
    let cadence_combo = ComboBoxText::new();
    cadence_combo.append(Some("daily"), "Daily");
    cadence_combo.append(Some("weekdays"), "Selected weekdays");
    cadence_combo.append(Some("every_n_days"), "Every N days");
    cadence_combo.append(Some("times_per_week"), "N times per week");

    let current_recurrence = latest
        .as_ref()
        .map(|revision| revision.recurrence)
        .unwrap_or(Recurrence::Daily);
    let initial_cadence_id = match current_recurrence {
        Recurrence::Daily => "daily",
        Recurrence::Weekdays { .. } => "weekdays",
        Recurrence::EveryNDays { .. } => "every_n_days",
        Recurrence::TimesPerWeek { .. } => "times_per_week",
    };
    cadence_combo.set_active_id(Some(initial_cadence_id));

    let weekday_label = Label::new(Some("Weekdays (Monday to Sunday)"));
    weekday_label.set_xalign(0.0);
    let weekday_row = gtk::Box::new(Orientation::Horizontal, 6);
    let weekday_names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let initial_mask = match current_recurrence {
        Recurrence::Weekdays { weekday_mask } => weekday_mask,
        _ => 0b001_1111,
    };
    let weekday_checks: Vec<CheckButton> = weekday_names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let check = CheckButton::with_label(name);
            check.set_active(initial_mask & (1 << index) != 0);
            weekday_row.append(&check);
            check
        })
        .collect();

    let interval_label = Label::new(Some("Every N days"));
    interval_label.set_xalign(0.0);
    let interval_entry = Entry::new();
    interval_entry.set_placeholder_text(Some("2"));
    let interval = match current_recurrence {
        Recurrence::EveryNDays { interval_days } => interval_days.to_string(),
        _ => "2".to_owned(),
    };
    interval_entry.set_text(&interval);

    let weekly_label = Label::new(Some("Weekly session target (1–7)"));
    weekly_label.set_xalign(0.0);
    let weekly_entry = Entry::new();
    weekly_entry.set_placeholder_text(Some("3"));
    let weekly_target = match current_recurrence {
        Recurrence::TimesPerWeek { target_per_week } => target_per_week.to_string(),
        _ => "3".to_owned(),
    };
    weekly_entry.set_text(&weekly_target);

    let end_label = Label::new(Some("End date (optional, inclusive)"));
    end_label.set_xalign(0.0);
    let end_entry = Entry::new();
    end_entry.set_placeholder_text(Some("Leave blank for no end date"));
    let timezone_label = Label::new(Some("IANA time zone"));
    timezone_label.set_xalign(0.0);
    let timezone_entry = Entry::new();
    timezone_entry.set_placeholder_text(Some("e.g. Asia/Kolkata"));
    timezone_entry.set_text(&default_zone);

    let reason_label = Label::new(Some("Reason for this change"));
    reason_label.set_xalign(0.0);
    let reason_entry = Entry::new();
    reason_entry.set_placeholder_text(Some("Why are you changing the schedule?"));

    let historical_confirmation = CheckButton::with_label(
        "I understand: a past-effective rule changes historical due-state interpretation; saved check-ins are not deleted.",
    );
    historical_confirmation.set_active(false);

    let feedback = Label::new(None);
    feedback.set_xalign(0.0);
    feedback.set_wrap(true);
    feedback.add_css_class("error");

    let actions = gtk::Box::new(Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel_button = Button::with_label("Cancel");
    let save_button = Button::with_label("Save new schedule");
    save_button.add_css_class("suggested-action");
    actions.append(&cancel_button);
    actions.append(&save_button);

    content.append(&current_label);
    content.append(&intro);
    content.append(&effective_label);
    content.append(&effective_entry);
    content.append(&cadence_label);
    content.append(&cadence_combo);
    content.append(&weekday_label);
    content.append(&weekday_row);
    content.append(&interval_label);
    content.append(&interval_entry);
    content.append(&weekly_label);
    content.append(&weekly_entry);
    content.append(&end_label);
    content.append(&end_entry);
    content.append(&timezone_label);
    content.append(&timezone_entry);
    content.append(&reason_label);
    content.append(&reason_entry);
    content.append(&historical_confirmation);
    content.append(&feedback);
    content.append(&actions);
    editor.set_child(Some(&content));

    let save_editor = editor.clone();
    let save_state = Rc::clone(state);
    let save_list = list.clone();
    let save_parent = parent.clone();
    let save_effective = effective_entry.clone();
    let save_cadence = cadence_combo.clone();
    let save_weekdays = weekday_checks.clone();
    let save_interval = interval_entry.clone();
    let save_weekly = weekly_entry.clone();
    let save_end = end_entry.clone();
    let save_timezone = timezone_entry.clone();
    let save_reason = reason_entry.clone();
    let save_confirmation = historical_confirmation.clone();
    let save_feedback = feedback.clone();

    save_button.connect_clicked(move |_| {
        let effective_from = match save_effective.text().parse::<Date>() {
            Ok(date) => date,
            Err(_) => {
                save_feedback.set_text("Enter a valid effective date in YYYY-MM-DD format.");
                return;
            }
        };

        let timezone_id = save_timezone.text().to_string();
        let zone = match IanaTimeZone::parse(&timezone_id) {
            Ok(zone) => zone,
            Err(error) => {
                save_feedback.set_text(&format!("Invalid IANA time zone: {error}"));
                return;
            }
        };
        let today = zone.local_date_at(Timestamp::now());
        let confirmation = if effective_from < today {
            if !save_confirmation.is_active() {
                save_feedback.set_text(&format!(
                    "This change would reinterpret schedule history from {effective_from} through {today}. Review the impact and tick the confirmation before saving."
                ));
                return;
            }
            PastEditConfirmation::Confirmed
        } else {
            PastEditConfirmation::NotConfirmed
        };

        let cadence_id = save_cadence.active_id().map(|value| value.to_string());
        let recurrence = match cadence_id.as_deref() {
            Some("daily") => Recurrence::Daily,
            Some("weekdays") => {
                let mut mask = 0_u8;
                for (index, check) in save_weekdays.iter().enumerate() {
                    if check.is_active() {
                        mask |= 1 << index;
                    }
                }
                Recurrence::Weekdays { weekday_mask: mask }
            }
            Some("every_n_days") => match save_interval.text().parse::<u16>() {
                Ok(interval_days) => Recurrence::EveryNDays { interval_days },
                Err(_) => {
                    save_feedback.set_text("Every N days must be an integer from 1 to 65535.");
                    return;
                }
            },
            Some("times_per_week") => match save_weekly.text().parse::<u8>() {
                Ok(target_per_week) => Recurrence::TimesPerWeek { target_per_week },
                Err(_) => {
                    save_feedback.set_text("Weekly target must be an integer from 1 to 7.");
                    return;
                }
            },
            _ => {
                save_feedback.set_text("Select a recurrence rule.");
                return;
            }
        };

        let end_date = if save_end.text().trim().is_empty() {
            None
        } else {
            match save_end.text().parse::<Date>() {
                Ok(date) => Some(date),
                Err(_) => {
                    save_feedback.set_text("End date must be blank or a valid YYYY-MM-DD date.");
                    return;
                }
            }
        };

        let revision = NewScheduleRevision {
            effective_from,
            end_date,
            recurrence,
            time_zone_id,
            change_reason: save_reason.text().to_string(),
        };

        let result = {
            let mut connection = save_state.connection.borrow_mut();
            append_revision(
                &mut connection,
                habit_id,
                &revision,
                today,
                confirmation,
            )
        };
        match result {
            Ok(revision_number) => {
                save_feedback.set_text(&format!("Saved schedule revision #{revision_number}."));
                save_editor.close();
                refresh_list(&save_list, &save_state, &save_parent, false);
            }
            Err(error) => save_feedback.set_text(&format!("Could not save schedule: {error}")),
        }
    });

    let cancel_editor = editor.clone();
    cancel_button.connect_clicked(move |_| cancel_editor.close());

    editor.present();
    effective_entry.grab_focus();
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
