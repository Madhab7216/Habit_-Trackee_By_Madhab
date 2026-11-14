//! Local-first goal and milestone repository plus GTK window.
//!
//! Goal progress is derived from explicit milestones. Skipped milestones are
//! excluded from the progress denominator and shown separately; no progress is
//! fabricated when a goal has no eligible milestones.

use gtk::prelude::*;
use rusqlite::{Connection, Result, params};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

const MILESTONE_MIGRATION: &str = include_str!("../migrations/0003_goal_milestones.sql");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalSummary {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub target_date: Option<String>,
    pub status: String,
    pub milestone_total: u32,
    pub milestone_completed: u32,
    pub milestone_skipped: u32,
    pub linked_habits: u32,
    pub progress_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Milestone {
    pub id: i64,
    pub goal_id: i64,
    pub title: String,
    pub description: String,
    pub due_date: Option<String>,
    pub status: String,
}

fn validate_title(title: &str, label: &str) -> Result<&str> {
    let normalized = title.trim();
    if normalized.is_empty() || normalized.chars().count() > 120 {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "{label} must contain between 1 and 120 characters"
        )));
    }
    Ok(normalized)
}

fn validate_description(description: &str) -> Result<&str> {
    if description.chars().count() > 2_000 {
        return Err(rusqlite::Error::InvalidParameterName(
            "description must be 2,000 characters or fewer".to_string(),
        ));
    }
    Ok(description.trim())
}

fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn valid_iso_date(value: &str) -> bool {
    if value.len() != 10
        || value.as_bytes().get(4) != Some(&b'-')
        || value.as_bytes().get(7) != Some(&b'-')
    {
        return false;
    }
    let Some(year) = value.get(0..4).and_then(|part| part.parse::<i32>().ok()) else {
        return false;
    };
    let Some(month) = value.get(5..7).and_then(|part| part.parse::<u8>().ok()) else {
        return false;
    };
    let Some(day) = value.get(8..10).and_then(|part| part.parse::<u8>().ok()) else {
        return false;
    };
    if !(1..=9_999).contains(&year) || !(1..=12).contains(&month) {
        return false;
    }
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => return false,
    };
    (1..=max_day).contains(&day)
}

fn validate_optional_date(value: Option<&str>) -> Result<Option<&str>> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(date) if valid_iso_date(date) => Ok(Some(date)),
        Some(_) => Err(rusqlite::Error::InvalidParameterName(
            "date must be a real calendar date in YYYY-MM-DD format".to_string(),
        )),
        None => Ok(None),
    }
}

/// Apply the additive milestone migration without changing the core habit schema version.
pub fn ensure_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS goal_schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );",
    )?;
    let version: i64 = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM goal_schema_migrations",
        [],
        |row| row.get(0),
    )?;
    if version > 1 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    if version < 1 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(MILESTONE_MIGRATION)?;
        transaction.execute("INSERT INTO goal_schema_migrations(version) VALUES (1)", [])?;
        transaction.commit()?;
    }
    Ok(())
}

pub fn create_goal(
    connection: &Connection,
    title: &str,
    description: &str,
    target_date: Option<&str>,
) -> Result<i64> {
    let title = validate_title(title, "goal title")?;
    let description = validate_description(description)?;
    let target_date = validate_optional_date(target_date)?;
    connection.execute(
        "INSERT INTO goals(title, description, target_date, status)
         VALUES (?1, ?2, ?3, 'active')",
        params![title, description, target_date],
    )?;
    Ok(connection.last_insert_rowid())
}

pub fn update_goal(
    connection: &Connection,
    goal_id: i64,
    title: &str,
    description: &str,
    target_date: Option<&str>,
) -> Result<()> {
    let title = validate_title(title, "goal title")?;
    let description = validate_description(description)?;
    let target_date = validate_optional_date(target_date)?;
    let changed = connection.execute(
        "UPDATE goals
         SET title = ?1, description = ?2, target_date = ?3,
             updated_at = CURRENT_TIMESTAMP
         WHERE id = ?4 AND status <> 'archived'",
        params![title, description, target_date, goal_id],
    )?;
    if changed == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

pub fn set_goal_status(connection: &Connection, goal_id: i64, status: &str) -> Result<()> {
    if !matches!(status, "active" | "paused" | "completed" | "archived") {
        return Err(rusqlite::Error::InvalidParameterName(
            "unsupported goal status".to_string(),
        ));
    }
    let changed = connection.execute(
        "UPDATE goals SET status = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
        params![status, goal_id],
    )?;
    if changed == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

pub fn list_goals(connection: &Connection, include_archived: bool) -> Result<Vec<GoalSummary>> {
    let mut statement = connection.prepare(
        "SELECT
            g.id, g.title, g.description, g.target_date, g.status,
            (SELECT COUNT(*) FROM goal_milestones m WHERE m.goal_id = g.id),
            (SELECT COUNT(*) FROM goal_milestones m
                WHERE m.goal_id = g.id AND m.status = 'completed'),
            (SELECT COUNT(*) FROM goal_milestones m
                WHERE m.goal_id = g.id AND m.status = 'skipped'),
            (SELECT COUNT(*) FROM goal_habits gh
                WHERE gh.goal_id = g.id)
         FROM goals g
         WHERE ((?1 = 1 AND g.status = 'archived') OR (?1 = 0 AND g.status <> 'archived'))
         ORDER BY
            CASE g.status WHEN 'active' THEN 0 WHEN 'paused' THEN 1
                 WHEN 'completed' THEN 2 ELSE 3 END,
            CASE WHEN g.target_date IS NULL THEN 1 ELSE 0 END,
            g.target_date, g.id DESC",
    )?;

    let rows = statement.query_map([if include_archived { 1_i64 } else { 0_i64 }], |row| {
        let total: i64 = row.get(5)?;
        let completed: i64 = row.get(6)?;
        let skipped: i64 = row.get(7)?;
        let eligible = total.saturating_sub(skipped);
        let progress_percent = if eligible > 0 {
            ((completed * 100) / eligible).clamp(0, 100) as u8
        } else {
            0
        };
        Ok(GoalSummary {
            id: row.get(0)?,
            title: row.get(1)?,
            description: row.get(2)?,
            target_date: row.get(3)?,
            status: row.get(4)?,
            milestone_total: u32::try_from(total).unwrap_or(u32::MAX),
            milestone_completed: u32::try_from(completed).unwrap_or(u32::MAX),
            milestone_skipped: u32::try_from(skipped).unwrap_or(u32::MAX),
            linked_habits: row.get::<_, i64>(8)? as u32,
            progress_percent,
        })
    })?;
    rows.collect()
}

pub fn create_milestone(
    connection: &Connection,
    goal_id: i64,
    title: &str,
    description: &str,
    due_date: Option<&str>,
) -> Result<i64> {
    let title = validate_title(title, "milestone title")?;
    let description = validate_description(description)?;
    let due_date = validate_optional_date(due_date)?;
    let goal_is_available: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM goals WHERE id = ?1 AND status <> 'archived')",
        [goal_id],
        |row| row.get(0),
    )?;
    if !goal_is_available {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    connection.execute(
        "INSERT INTO goal_milestones(goal_id, title, description, due_date)
         VALUES (?1, ?2, ?3, ?4)",
        params![goal_id, title, description, due_date],
    )?;
    Ok(connection.last_insert_rowid())
}

pub fn list_milestones(connection: &Connection, goal_id: i64) -> Result<Vec<Milestone>> {
    let mut statement = connection.prepare(
        "SELECT id, goal_id, title, description, due_date, status
         FROM goal_milestones
         WHERE goal_id = ?1
         ORDER BY
            CASE status WHEN 'pending' THEN 0 WHEN 'completed' THEN 1 ELSE 2 END,
            CASE WHEN due_date IS NULL THEN 1 ELSE 0 END,
            due_date, sort_order, id",
    )?;
    let rows = statement.query_map([goal_id], |row| {
        Ok(Milestone {
            id: row.get(0)?,
            goal_id: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            due_date: row.get(4)?,
            status: row.get(5)?,
        })
    })?;
    rows.collect()
}

pub fn set_milestone_status(
    connection: &Connection,
    milestone_id: i64,
    status: &str,
) -> Result<()> {
    if !matches!(status, "pending" | "completed" | "skipped") {
        return Err(rusqlite::Error::InvalidParameterName(
            "unsupported milestone status".to_string(),
        ));
    }
    let changed = connection.execute(
        "UPDATE goal_milestones
         SET status = ?1,
             completed_at = CASE WHEN ?1 = 'completed' THEN CURRENT_TIMESTAMP ELSE NULL END,
             updated_at = CURRENT_TIMESTAMP
         WHERE id = ?2",
        params![status, milestone_id],
    )?;
    if changed == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

pub fn link_habit_to_goal(connection: &Connection, goal_id: i64, habit_id: i64) -> Result<()> {
    let eligible: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM goals g
            JOIN habits h ON h.id = ?2
            WHERE g.id = ?1
              AND g.status <> 'archived'
              AND h.archived_at IS NULL
        )",
        params![goal_id, habit_id],
        |row| row.get(0),
    )?;
    if !eligible {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    connection.execute(
        "INSERT OR IGNORE INTO goal_habits(goal_id, habit_id) VALUES (?1, ?2)",
        params![goal_id, habit_id],
    )?;
    Ok(())
}

pub fn unlink_habit_from_goal(connection: &Connection, goal_id: i64, habit_id: i64) -> Result<()> {
    let changed = connection.execute(
        "DELETE FROM goal_habits WHERE goal_id = ?1 AND habit_id = ?2",
        params![goal_id, habit_id],
    )?;
    if changed == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

pub fn open_window(parent: &gtk::ApplicationWindow, connection: Rc<RefCell<Connection>>) {
    if let Err(error) = ensure_schema(&connection.borrow()) {
        eprintln!("Could not prepare goal storage: {error}");
        return;
    }

    let window = gtk::Window::builder()
        .title("Goals & milestones")
        .transient_for(parent)
        .default_width(820)
        .default_height(680)
        .build();

    let root = gtk::Box::new(gtk::Orientation::Vertical, 14);
    root.set_margin_top(24);
    root.set_margin_bottom(24);
    root.set_margin_start(28);
    root.set_margin_end(28);

    let title = gtk::Label::new(Some("Goals & milestones"));
    title.add_css_class("title-1");
    title.set_xalign(0.0);
    let subtitle = gtk::Label::new(Some(
        "Turn long-term goals into clear, measurable milestones. Progress is based on completed milestones.",
    ));
    subtitle.add_css_class("dim-label");
    subtitle.set_xalign(0.0);
    subtitle.set_wrap(true);

    let form = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title_entry = gtk::Entry::new();
    title_entry.set_placeholder_text(Some("New goal title"));
    title_entry.set_hexpand(true);
    let date_entry = gtk::Entry::new();
    date_entry.set_placeholder_text(Some("Target date · YYYY-MM-DD"));
    date_entry.set_width_chars(18);
    let add_button = gtk::Button::with_label("Add goal");
    add_button.add_css_class("suggested-action");
    form.append(&title_entry);
    form.append(&date_entry);
    form.append(&add_button);

    let feedback = gtk::Label::new(None);
    feedback.set_xalign(0.0);
    feedback.add_css_class("error");

    let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let archived_button = gtk::Button::with_label("Show archived");
    toolbar.append(&archived_button);

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_vexpand(true);
    scroller.set_child(Some(&list));

    root.append(&title);
    root.append(&subtitle);
    root.append(&form);
    root.append(&feedback);
    root.append(&toolbar);
    root.append(&scroller);
    window.set_child(Some(&root));

    let archived_mode = Rc::new(Cell::new(false));
    let add_window = window.clone();
    let add_list = list.clone();
    let add_connection = Rc::clone(&connection);
    let add_title = title_entry.clone();
    let add_date = date_entry.clone();
    let add_feedback = feedback.clone();
    let add_mode = Rc::clone(&archived_mode);
    let archived_for_add = archived_button.clone();
    add_button.connect_clicked(move |_| {
        let goal_title = add_title.text().to_string();
        let goal_date = add_date.text().to_string();
        match create_goal(
            &add_connection.borrow(),
            &goal_title,
            "",
            Some(goal_date.trim()),
        ) {
            Ok(_) => {
                add_title.set_text("");
                add_date.set_text("");
                add_feedback.set_text("");
                add_mode.set(false);
                archived_for_add.set_label("Show archived");
                refresh_goal_list(&add_list, &add_window, &add_connection, false);
            }
            Err(error) => add_feedback.set_text(&format!("Could not create goal: {error}")),
        }
    });

    let toggle_window = window.clone();
    let toggle_list = list.clone();
    let toggle_connection = Rc::clone(&connection);
    let toggle_mode = Rc::clone(&archived_mode);
    let toggle_button = archived_button.clone();
    archived_button.connect_clicked(move |_| {
        let show_archived = !toggle_mode.get();
        toggle_mode.set(show_archived);
        toggle_button.set_label(if show_archived {
            "Back to goals"
        } else {
            "Show archived"
        });
        refresh_goal_list(
            &toggle_list,
            &toggle_window,
            &toggle_connection,
            show_archived,
        );
    });

    refresh_goal_list(&list, &window, &connection, false);
    window.present();
}

fn refresh_goal_list(
    list: &gtk::ListBox,
    parent: &gtk::Window,
    connection: &Rc<RefCell<Connection>>,
    archived_view: bool,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    let goals = match list_goals(&connection.borrow(), archived_view) {
        Ok(goals) => goals,
        Err(error) => {
            append_list_message(list, &format!("Could not load goals: {error}"));
            return;
        }
    };

    if goals.is_empty() {
        append_list_message(
            list,
            if archived_view {
                "No archived goals."
            } else {
                "No goals yet. Add your first goal above."
            },
        );
        return;
    }

    for goal in goals {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        card.set_margin_top(12);
        card.set_margin_bottom(12);
        card.set_margin_start(14);
        card.set_margin_end(14);

        let heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = gtk::Label::new(Some(&goal.title));
        title.add_css_class("heading");
        title.set_xalign(0.0);
        title.set_hexpand(true);
        let target = gtk::Label::new(Some(
            goal.target_date
                .as_deref()
                .map(|date| format!("Target {date}"))
                .unwrap_or_else(|| "No target date".to_string())
                .as_str(),
        ));
        target.add_css_class("dim-label");
        heading.append(&title);
        heading.append(&target);

        let progress = gtk::ProgressBar::new();
        progress.set_show_text(true);
        progress.set_fraction(f64::from(goal.progress_percent) / 100.0);
        progress.set_text(Some(&format!(
            "{}% · {}/{} milestones · {} skipped · {} linked habits",
            goal.progress_percent,
            goal.milestone_completed,
            goal.milestone_total.saturating_sub(goal.milestone_skipped),
            goal.milestone_skipped,
            goal.linked_habits
        )));

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let milestones_button = gtk::Button::with_label("Milestones");
        let edit_button = gtk::Button::with_label("Edit");
        edit_button.set_sensitive(!archived_view);
        actions.append(&milestones_button);
        actions.append(&edit_button);

        let milestone_parent = parent.clone();
        let milestone_connection = Rc::clone(connection);
        let goal_id = goal.id;
        let goal_title = goal.title.clone();
        milestones_button.connect_clicked(move |_| {
            open_milestones_window(
                &milestone_parent,
                Rc::clone(&milestone_connection),
                goal_id,
                &goal_title,
            );
        });

        let edit_parent = parent.clone();
        let edit_list = list.clone();
        let edit_connection = Rc::clone(connection);
        let edit_goal = goal.clone();
        let edit_archived = archived_view;
        edit_button.connect_clicked(move |_| {
            open_goal_editor(
                &edit_parent,
                &edit_list,
                &edit_connection,
                &edit_goal,
                edit_archived,
            );
        });

        if archived_view {
            let restore_button = gtk::Button::with_label("Restore");
            let restore_parent = parent.clone();
            let restore_list = list.clone();
            let restore_connection = Rc::clone(connection);
            let restore_id = goal.id;
            restore_button.connect_clicked(move |_| {
                if let Err(error) =
                    set_goal_status(&restore_connection.borrow(), restore_id, "active")
                {
                    eprintln!("Could not restore goal: {error}");
                }
                refresh_goal_list(&restore_list, &restore_parent, &restore_connection, true);
            });
            actions.append(&restore_button);
        } else {
            let state_action = gtk::Button::with_label(match goal.status.as_str() {
                "active" => "Pause",
                "paused" => "Resume",
                "completed" => "Reopen",
                _ => "Archive",
            });
            let new_status = match goal.status.as_str() {
                "active" => "paused",
                "paused" | "completed" => "active",
                _ => "archived",
            };
            let state_parent = parent.clone();
            let state_list = list.clone();
            let state_connection = Rc::clone(connection);
            let state_id = goal.id;
            state_action.connect_clicked(move |_| {
                if let Err(error) =
                    set_goal_status(&state_connection.borrow(), state_id, new_status)
                {
                    eprintln!("Could not update goal: {error}");
                }
                refresh_goal_list(&state_list, &state_parent, &state_connection, false);
            });
            actions.append(&state_action);

            let complete_button = gtk::Button::with_label("Complete goal");
            complete_button.set_sensitive(goal.status != "completed");
            let complete_parent = parent.clone();
            let complete_list = list.clone();
            let complete_connection = Rc::clone(connection);
            let complete_id = goal.id;
            complete_button.connect_clicked(move |_| {
                if let Err(error) =
                    set_goal_status(&complete_connection.borrow(), complete_id, "completed")
                {
                    eprintln!("Could not complete goal: {error}");
                }
                refresh_goal_list(
                    &complete_list,
                    &complete_parent,
                    &complete_connection,
                    false,
                );
            });
            actions.append(&complete_button);

            let archive_button = gtk::Button::with_label("Archive");
            let archive_parent = parent.clone();
            let archive_list = list.clone();
            let archive_connection = Rc::clone(connection);
            let archive_id = goal.id;
            archive_button.connect_clicked(move |_| {
                if let Err(error) =
                    set_goal_status(&archive_connection.borrow(), archive_id, "archived")
                {
                    eprintln!("Could not archive goal: {error}");
                }
                refresh_goal_list(&archive_list, &archive_parent, &archive_connection, false);
            });
            actions.append(&archive_button);
        }

        card.append(&heading);
        card.append(&progress);
        card.append(&actions);
        let row = gtk::ListBoxRow::new();
        row.set_child(Some(&card));
        list.append(&row);
    }
}

fn open_goal_editor(
    parent: &gtk::Window,
    list: &gtk::ListBox,
    connection: &Rc<RefCell<Connection>>,
    goal: &GoalSummary,
    archived_view: bool,
) {
    let editor = gtk::Window::builder()
        .title("Edit goal")
        .transient_for(parent)
        .modal(true)
        .default_width(440)
        .default_height(280)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(20);
    content.set_margin_end(20);

    let title = gtk::Entry::new();
    title.set_text(&goal.title);
    title.set_placeholder_text(Some("Goal title"));
    let description = gtk::Entry::new();
    description.set_text(&goal.description);
    description.set_placeholder_text(Some("Short description"));
    let target_date = gtk::Entry::new();
    target_date.set_text(goal.target_date.as_deref().unwrap_or(""));
    target_date.set_placeholder_text(Some("Target date · YYYY-MM-DD (optional)"));
    let feedback = gtk::Label::new(None);
    feedback.add_css_class("error");
    feedback.set_xalign(0.0);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    let save = gtk::Button::with_label("Save changes");
    save.add_css_class("suggested-action");
    actions.append(&cancel);
    actions.append(&save);

    content.append(&title);
    content.append(&description);
    content.append(&target_date);
    content.append(&feedback);
    content.append(&actions);
    editor.set_child(Some(&content));

    let save_editor = editor.clone();
    let save_list = list.clone();
    let save_parent = parent.clone();
    let save_connection = Rc::clone(connection);
    let save_title = title.clone();
    let save_description = description.clone();
    let save_date = target_date.clone();
    let save_feedback = feedback.clone();
    let goal_id = goal.id;
    save.connect_clicked(move |_| {
        let title_value = save_title.text().to_string();
        let description_value = save_description.text().to_string();
        let date_value = save_date.text().to_string();
        match update_goal(
            &save_connection.borrow(),
            goal_id,
            &title_value,
            &description_value,
            Some(date_value.trim()),
        ) {
            Ok(()) => {
                save_editor.close();
                refresh_goal_list(&save_list, &save_parent, &save_connection, archived_view);
            }
            Err(error) => save_feedback.set_text(&format!("Could not update goal: {error}")),
        }
    });
    let cancel_editor = editor.clone();
    cancel.connect_clicked(move |_| cancel_editor.close());
    editor.present();
    title.grab_focus();
}

fn open_milestones_window(
    parent: &gtk::Window,
    connection: Rc<RefCell<Connection>>,
    goal_id: i64,
    goal_title: &str,
) {
    let window = gtk::Window::builder()
        .title(format!("Milestones · {goal_title}"))
        .transient_for(parent)
        .default_width(680)
        .default_height(560)
        .build();
    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.set_margin_top(20);
    root.set_margin_bottom(20);
    root.set_margin_start(24);
    root.set_margin_end(24);

    let heading = gtk::Label::new(Some(goal_title));
    heading.add_css_class("title-2");
    heading.set_xalign(0.0);
    let form = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title_entry = gtk::Entry::new();
    title_entry.set_placeholder_text(Some("New milestone"));
    title_entry.set_hexpand(true);
    let date_entry = gtk::Entry::new();
    date_entry.set_placeholder_text(Some("Due · YYYY-MM-DD"));
    date_entry.set_width_chars(16);
    let add_button = gtk::Button::with_label("Add");
    add_button.add_css_class("suggested-action");
    form.append(&title_entry);
    form.append(&date_entry);
    form.append(&add_button);

    let feedback = gtk::Label::new(None);
    feedback.add_css_class("error");
    feedback.set_xalign(0.0);
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_vexpand(true);
    scroller.set_child(Some(&list));

    root.append(&heading);
    root.append(&form);
    root.append(&feedback);
    root.append(&scroller);
    window.set_child(Some(&root));

    let add_window = window.clone();
    let add_list = list.clone();
    let add_connection = Rc::clone(&connection);
    let add_title = title_entry.clone();
    let add_date = date_entry.clone();
    let add_feedback = feedback.clone();
    add_button.connect_clicked(move |_| {
        let milestone_title = add_title.text().to_string();
        let milestone_date = add_date.text().to_string();
        match create_milestone(
            &add_connection.borrow(),
            goal_id,
            &milestone_title,
            "",
            Some(milestone_date.trim()),
        ) {
            Ok(_) => {
                add_title.set_text("");
                add_date.set_text("");
                add_feedback.set_text("");
                refresh_milestone_list(&add_list, &add_window, &add_connection, goal_id);
            }
            Err(error) => add_feedback.set_text(&format!("Could not add milestone: {error}")),
        }
    });

    refresh_milestone_list(&list, &window, &connection, goal_id);
    window.present();
}

fn refresh_milestone_list(
    list: &gtk::ListBox,
    parent: &gtk::Window,
    connection: &Rc<RefCell<Connection>>,
    goal_id: i64,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    let milestones = match list_milestones(&connection.borrow(), goal_id) {
        Ok(milestones) => milestones,
        Err(error) => {
            append_list_message(list, &format!("Could not load milestones: {error}"));
            return;
        }
    };

    if milestones.is_empty() {
        append_list_message(list, "No milestones yet. Add the first step above.");
        return;
    }

    for milestone in milestones {
        let row_content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row_content.set_margin_top(10);
        row_content.set_margin_bottom(10);
        row_content.set_margin_start(12);
        row_content.set_margin_end(12);
        let name = gtk::Label::new(Some(&milestone.title));
        name.set_xalign(0.0);
        name.set_hexpand(true);
        let due = milestone.due_date.as_deref().unwrap_or("No due date");
        let status = gtk::Label::new(Some(match milestone.status.as_str() {
            "completed" => "Completed",
            "skipped" => "Skipped",
            _ => due,
        }));
        status.add_css_class("dim-label");
        let next_status = match milestone.status.as_str() {
            "completed" => "pending",
            _ => "completed",
        };
        let complete = gtk::Button::with_label(match milestone.status.as_str() {
            "completed" => "Reopen",
            _ => "Complete",
        });
        let milestone_connection = Rc::clone(connection);
        let milestone_list = list.clone();
        let milestone_parent = parent.clone();
        let milestone_id = milestone.id;
        complete.connect_clicked(move |_| {
            if let Err(error) =
                set_milestone_status(&milestone_connection.borrow(), milestone_id, next_status)
            {
                eprintln!("Could not update milestone: {error}");
            }
            refresh_milestone_list(
                &milestone_list,
                &milestone_parent,
                &milestone_connection,
                goal_id,
            );
        });

        let skip = gtk::Button::with_label(if milestone.status == "skipped" { "Reopen" } else { "Skip" });
        let skip_connection = Rc::clone(connection);
        let skip_list = list.clone();
        let skip_parent = parent.clone();
        let skip_id = milestone.id;
        let skip_status = if milestone.status == "skipped" {
            "pending"
        } else {
            "skipped"
        };
        skip.connect_clicked(move |_| {
            if let Err(error) =
                set_milestone_status(&skip_connection.borrow(), skip_id, skip_status)
            {
                eprintln!("Could not skip milestone: {error}");
            }
            refresh_milestone_list(&skip_list, &skip_parent, &skip_connection, goal_id);
        });

        row_content.append(&name);
        row_content.append(&status);
        row_content.append(&complete);
        row_content.append(&skip);
        let row = gtk::ListBoxRow::new();
        row.set_child(Some(&row_content));
        list.append(&row);
    }
}

fn append_list_message(list: &gtk::ListBox, message: &str) {
    let label = gtk::Label::new(Some(message));
    label.set_xalign(0.0);
    label.set_margin_top(16);
    label.set_margin_bottom(16);
    label.set_margin_start(16);
    label.set_margin_end(16);
    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&label));
    list.append(&row);
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE_MIGRATION: &str = include_str!("../migrations/0001_initial.sql");

    fn test_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(BASE_MIGRATION)
            .expect("create base schema");
        ensure_schema(&connection).expect("create milestone schema");
        connection
    }

    fn goal(connection: &Connection) -> i64 {
        create_goal(
            connection,
            "Learn Rust",
            "Build a small project",
            Some("2027-03-31"),
        )
        .expect("create goal")
    }

    #[test]
    fn accepts_real_calendar_dates_and_rejects_impossible_dates() {
        assert!(valid_iso_date("2028-02-29"));
        assert!(valid_iso_date("2000-02-29"));
        assert!(!valid_iso_date("2027-02-29"));
        assert!(!valid_iso_date("1900-02-29"));
        assert!(!valid_iso_date("2027-04-31"));
        assert!(!valid_iso_date("2027-4-01"));
    }

    #[test]
    fn validates_goal_titles_and_target_dates() {
        let connection = test_connection();
        assert!(create_goal(&connection, "   ", "", None).is_err());
        assert!(create_goal(&connection, &"x".repeat(121), "", None).is_err());
        assert!(create_goal(&connection, "Valid", "", Some("2027-02-29")).is_err());
        assert!(create_goal(&connection, "Valid", "", Some("2028-02-29")).is_ok());
    }

    #[test]
    fn milestone_progress_excludes_skipped_items_from_denominator() {
        let connection = test_connection();
        let goal_id = goal(&connection);
        let first = create_milestone(&connection, goal_id, "Read the book", "", None).unwrap();
        create_milestone(&connection, goal_id, "Build a project", "", None).unwrap();
        let third = create_milestone(&connection, goal_id, "Write notes", "", None).unwrap();
        set_milestone_status(&connection, first, "completed").unwrap();
        set_milestone_status(&connection, third, "skipped").unwrap();

        let summaries = list_goals(&connection, false).unwrap();
        assert_eq!(summaries[0].milestone_total, 3);
        assert_eq!(summaries[0].milestone_completed, 1);
        assert_eq!(summaries[0].milestone_skipped, 1);
        assert_eq!(summaries[0].progress_percent, 50);
    }

    #[test]
    fn supports_milestone_reopen_and_goal_archive_restore() {
        let connection = test_connection();
        let goal_id = goal(&connection);
        let milestone_id = create_milestone(&connection, goal_id, "First draft", "", None).unwrap();
        set_milestone_status(&connection, milestone_id, "completed").unwrap();
        assert_eq!(
            list_goals(&connection, false).unwrap()[0].progress_percent,
            100
        );

        set_milestone_status(&connection, milestone_id, "pending").unwrap();
        assert_eq!(
            list_goals(&connection, false).unwrap()[0].progress_percent,
            0
        );

        set_goal_status(&connection, goal_id, "archived").unwrap();
        assert!(list_goals(&connection, false).unwrap().is_empty());
        assert_eq!(list_goals(&connection, true).unwrap()[0].status, "archived");

        set_goal_status(&connection, goal_id, "active").unwrap();
        assert_eq!(list_goals(&connection, false).unwrap().len(), 1);
    }

    #[test]
    fn milestone_cannot_be_created_for_archived_or_missing_goal() {
        let connection = test_connection();
        let goal_id = goal(&connection);
        set_goal_status(&connection, goal_id, "archived").unwrap();
        assert!(create_milestone(&connection, goal_id, "No longer active", "", None).is_err());
        assert!(create_milestone(&connection, 999, "Missing goal", "", None).is_err());
    }

    #[test]
    fn goal_can_link_and_unlink_only_active_habits() {
        let connection = test_connection();
        let goal_id = goal(&connection);
        connection
            .execute("INSERT INTO habits(name) VALUES ('Read 20 pages')", [])
            .unwrap();
        let habit_id: i64 = connection
            .query_row(
                "SELECT id FROM habits WHERE name = 'Read 20 pages'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        link_habit_to_goal(&connection, goal_id, habit_id).unwrap();
        link_habit_to_goal(&connection, goal_id, habit_id).unwrap();
        assert_eq!(list_goals(&connection, false).unwrap()[0].linked_habits, 1);
        unlink_habit_from_goal(&connection, goal_id, habit_id).unwrap();
        assert_eq!(list_goals(&connection, false).unwrap()[0].linked_habits, 0);
    }

    #[test]
    fn schema_migration_is_idempotent() {
        let connection = test_connection();
        ensure_schema(&connection).unwrap();
        ensure_schema(&connection).unwrap();
        let version: i64 = connection
            .query_row(
                "SELECT MAX(version) FROM goal_schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 1);
    }
}
