pub mod db;
pub mod domain;
pub mod goals;
pub mod i18n;
pub mod reviews;

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
    let preferred_language = db::open()
        .ok()
        .and_then(|connection| db::get_setting(&connection, "ui_language").ok().flatten());
    i18n::initialize(preferred_language.as_deref());

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
                .title(i18n::tr("Habit Trackers by Madhab"))
                .default_width(620)
                .default_height(180)
                .build();
            window.set_child(Some(&Label::new(Some(&i18n::tr(
                "The local database could not be opened. See the terminal for details.",
            )))));
            window.present();
            return;
        }
    };

    let state = Rc::new(AppState {
        connection: Rc::new(RefCell::new(connection)),
    });
    let window = ApplicationWindow::builder()
        .application(application)
        .title(i18n::tr("Habit Trackers by Madhab"))
        .default_width(1040)
        .default_height(720)
        .build();

    let header = HeaderBar::new();
    header.set_title_widget(Some(&Label::new(Some(&i18n::tr("Madhab Habit Tracker")))));
    let goals_button = Button::with_label(&i18n::tr("Goals & milestones"));
    let goals_parent = window.clone();
    let goals_connection = Rc::clone(&state.connection);
    goals_button.connect_clicked(move |_| {
        goals::open_window(&goals_parent, Rc::clone(&goals_connection));
    });
    header.pack_end(&goals_button);

    let reviews_button = Button::with_label(&i18n::tr("Weekly & monthly reviews"));
    let reviews_parent = window.clone();
    let reviews_connection = Rc::clone(&state.connection);
    reviews_button.connect_clicked(move |_| {
        reviews::open_window(&reviews_parent, Rc::clone(&reviews_connection));
    });
    header.pack_end(&reviews_button);
    window.set_titlebar(Some(&header));

    let page = gtk::Box::new(Orientation::Vertical, 14);
    page.set_margin_top(24);
    page.set_margin_bottom(24);
    page.set_margin_start(28);
    page.set_margin_end(28);

    let title = Label::new(Some(&i18n::tr("Today")));
    title.add_css_class("title-1");
    title.set_xalign(0.0);
    let subtitle = Label::new(Some(&i18n::tr(
        "Small actions, steady progress. Your data stays on this device.",
    )));
    subtitle.add_css_class("dim-label");
    subtitle.set_xalign(0.0);

    let form = gtk::Box::new(Orientation::Horizontal, 8);
    let habit_entry = Entry::new();
    habit_entry.set_placeholder_text(Some(&i18n::tr("Add a habit…")));
    habit_entry.set_hexpand(true);
    let schedule_labels = [
        i18n::tr("Every day"),
        i18n::tr("Monday to Friday"),
        i18n::tr("Every 2 days"),
        i18n::tr("Times per week"),
    ];
    let schedule_refs = schedule_labels
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let schedule_picker = gtk::DropDown::from_strings(&schedule_refs);
    schedule_picker.set_selected(0);
    let weekly_target = gtk::SpinButton::with_range(1.0, 7.0, 1.0);
    weekly_target.set_value(3.0);
    weekly_target.set_tooltip_text(Some(&i18n::tr(
        "Weekly session target (1–7), used for Times per week",
    )));
    let add_button = Button::with_label(&i18n::tr("Add habit"));
    add_button.add_css_class("suggested-action");
    let archive_view_button = Button::with_label(&i18n::tr("View archived"));
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

    let footer = Label::new(Some(&i18n::tr(
        "M3 · Reversible habit management · Offline tracking · Your data stays on this device",
    )));
    footer.add_css_class("dim-label");
    footer.set_xalign(0.0);

    let language_row = gtk::Box::new(Orientation::Horizontal, 8);
    let language_label = Label::new(Some(&i18n::tr("Language")));
    let language_picker = gtk::DropDown::from_strings(&["English", "বাংলা"]);
    language_picker.set_selected(match i18n::current_language() {
        i18n::Language::English => 0,
        i18n::Language::Bengali => 1,
    });
    language_row.append(&language_label);
    language_row.append(&language_picker);

    page.append(&title);
    page.append(&subtitle);
    page.append(&language_row);
    page.append(&form);
    page.append(&scroller);
    page.append(&footer);
    window.set_child(Some(&page));

    let language_connection = Rc::clone(&state.connection);
    let language_feedback = footer.clone();
    language_picker.connect_selected_notify(move |picker| {
        let code = if picker.selected() == 1 { "bn" } else { "en" };
        match db::set_setting(&language_connection.borrow(), "ui_language", code) {
            Ok(()) => {
                language_feedback.set_text(&i18n::tr("Language saved. Restart the app to apply."))
            }
            Err(error) => language_feedback.set_text(&i18n::format(
                "Could not save language preference: {0}",
                &[&i18n::error(&error)],
            )),
        }
    });

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
                archive_button_for_add.set_label(&i18n::tr("View archived"));
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
        let label = if show_archived {
            i18n::tr("Back to today")
        } else {
            i18n::tr("View archived")
        };
        toggle_label.set_label(&label);
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
            append_message(
                list,
                &i18n::format("Could not load habits: {0}", &[&i18n::error(&error)]),
            );
            return;
        }
    };

    if habits.is_empty() {
        let empty_message = if archived_view {
            i18n::tr("No archived habits.")
        } else {
            i18n::tr("No habits yet. Add one above to begin.")
        };
        append_message(list, &empty_message);
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
            let status = Label::new(Some(&i18n::tr("Archived")));
            status.add_css_class("dim-label");
            let restore_button = Button::with_label(&i18n::tr("Restore"));
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
            let status_text = if habit.paused {
                i18n::tr("Paused")
            } else if habit.completed {
                i18n::tr("Completed")
            } else if !habit.scheduled {
                i18n::tr("Not scheduled")
            } else {
                i18n::tr("Due today")
            };
            let status = Label::new(Some(&status_text));
            status.add_css_class("dim-label");

            let edit_button = Button::with_label(&i18n::tr("Edit"));
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

            let action_label = if habit.completed {
                i18n::tr("Undo")
            } else {
                i18n::tr("Mark done")
            };
            let action = Button::with_label(&action_label);
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

            let pause_label = if habit.paused {
                i18n::tr("Resume")
            } else {
                i18n::tr("Pause")
            };
            let pause_action = Button::with_label(&pause_label);
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

            let archive_action = Button::with_label(&i18n::tr("Archive"));
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
            row_content.append(&action);
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
        .title(i18n::tr("Edit habit"))
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

    let label = Label::new(Some(&i18n::tr("Habit name")));
    label.set_xalign(0.0);
    let entry = Entry::new();
    entry.set_text(current_name);
    entry.set_activates_default(true);
    let feedback = Label::new(None);
    feedback.set_xalign(0.0);
    feedback.add_css_class("error");

    let actions = gtk::Box::new(Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel_button = Button::with_label(&i18n::tr("Cancel"));
    let save_button = Button::with_label(&i18n::tr("Save"));
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
            Err(error) => save_feedback.set_text(&i18n::format(
                "Could not save habit: {0}",
                &[&i18n::error(&error)],
            )),
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
