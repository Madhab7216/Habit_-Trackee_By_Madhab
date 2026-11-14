mod db;
pub mod domain;

use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Button, Entry, HeaderBar, Label, ListBox, ListBoxRow,
    Orientation, ScrolledWindow,
};
use std::{cell::RefCell, rc::Rc};

const APP_ID: &str = "org.madhab.habittracker";

struct AppState {
    connection: RefCell<rusqlite::Connection>,
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
        connection: RefCell::new(connection),
    });
    let window = ApplicationWindow::builder()
        .application(application)
        .title("Habit Trackers by Madhab")
        .default_width(1040)
        .default_height(720)
        .build();

    let header = HeaderBar::new();
    header.set_title_widget(Some(&Label::new(Some("Madhab Habit Tracker"))));
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
    habit_entry.set_placeholder_text(Some("Add a daily habit…"));
    habit_entry.set_hexpand(true);
    let add_button = Button::with_label("Add habit");
    add_button.add_css_class("suggested-action");
    form.append(&habit_entry);
    form.append(&add_button);

    let list = ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");
    let scroller = ScrolledWindow::new();
    scroller.set_vexpand(true);
    scroller.set_child(Some(&list));

    let footer = Label::new(Some(
        "M3 foundation · Offline tracking · Your data stays on this device",
    ));
    footer.add_css_class("dim-label");
    footer.set_xalign(0.0);

    page.append(&title);
    page.append(&subtitle);
    page.append(&form);
    page.append(&scroller);
    page.append(&footer);
    window.set_child(Some(&page));

    let add_state = Rc::clone(&state);
    let add_list = list.clone();
    let add_entry = habit_entry.clone();
    add_button.connect_clicked(move |_| {
        let name = add_entry.text().trim().to_string();
        if name.is_empty() {
            return;
        }
        match db::create_habit(&add_state.connection.borrow(), &name) {
            Ok(()) => add_entry.set_text(""),
            Err(error) => eprintln!("Could not save habit: {error}"),
        }
        refresh_list(&add_list, &add_state);
    });

    refresh_list(&list, &state);
    window.present();
}

fn refresh_list(list: &ListBox, state: &Rc<AppState>) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    let habits = match db::today_habits(&state.connection.borrow()) {
        Ok(habits) => habits,
        Err(error) => {
            append_message(list, &format!("Could not load habits: {error}"));
            return;
        }
    };

    if habits.is_empty() {
        append_message(list, "No habits yet. Add one above to begin.");
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
        if habit.completed || habit.paused {
            name.add_css_class("dim-label");
        }

        let status = Label::new(Some(if habit.paused {
            "Paused"
        } else if habit.completed {
            "Completed"
        } else {
            "Due today"
        }));
        status.add_css_class("dim-label");

        let action = Button::with_label(if habit.completed { "Undo" } else { "Mark done" });
        action.set_sensitive(!habit.paused);

        let action_state = Rc::clone(state);
        let action_list = list.clone();
        let habit_id = habit.id;
        let completed = habit.completed;
        action.connect_clicked(move |_| {
            if let Err(error) =
                db::set_today_completed(&action_state.connection.borrow(), habit_id, !completed)
            {
                eprintln!("Could not update habit: {error}");
            }
            refresh_list(&action_list, &action_state);
        });

        let pause_action = Button::with_label(if habit.paused { "Resume" } else { "Pause" });
        let pause_state = Rc::clone(state);
        let pause_list = list.clone();
        let pause_habit_id = habit.id;
        let paused = habit.paused;
        pause_action.connect_clicked(move |_| {
            if let Err(error) =
                db::set_paused(&pause_state.connection.borrow(), pause_habit_id, !paused)
            {
                eprintln!("Could not change pause state: {error}");
            }
            refresh_list(&pause_list, &pause_state);
        });

        row_content.append(&name);
        row_content.append(&status);
        row_content.append(&action);
        row_content.append(&pause_action);
        let row = ListBoxRow::new();
        row.set_child(Some(&row_content));
        list.append(&row);
    }
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
