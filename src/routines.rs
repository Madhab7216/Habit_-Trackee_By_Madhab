//! Local-first routine templates, ordered steps, daily runs and preserved run history.
//!
//! Run items are immutable snapshots of the step title/type at run start. Editing a
//! template never rewrites history. Habit links are organizational and never create
//! habit check-ins automatically.

use crate::domain::CivilDate;
use gtk::prelude::*;
use rusqlite::{Connection, OptionalExtension, Result, params};
use std::{cell::RefCell, rc::Rc};

const ROUTINE_MIGRATION: &str = include_str!("../migrations/0004_routines.sql");
type Refresh = Rc<dyn Fn()>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutineSchedule {
    Daily,
    /// Monday is bit 0; Sunday is bit 6.
    Weekdays {
        weekday_mask: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutineStepKind {
    Action,
    Habit { habit_id: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRoutineStep {
    pub title: String,
    pub details: String,
    pub kind: RoutineStepKind,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutineDraft {
    pub title: String,
    pub description: String,
    pub schedule: RoutineSchedule,
    pub start_date: String,
    pub end_date: Option<String>,
    pub steps: Vec<NewRoutineStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutineSummary {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub schedule: RoutineSchedule,
    pub start_date: String,
    pub end_date: Option<String>,
    pub status: String,
    pub step_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutineStep {
    pub id: i64,
    pub routine_id: i64,
    pub title: String,
    pub details: String,
    pub kind: RoutineStepKind,
    pub linked_habit_name: Option<String>,
    pub required: bool,
    pub sort_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutineDefinition {
    pub routine: RoutineSummary,
    pub steps: Vec<RoutineStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkableHabit {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutineRunItem {
    pub step_id: i64,
    pub title_snapshot: String,
    pub step_type_snapshot: String,
    pub linked_habit_id_snapshot: Option<i64>,
    pub required: bool,
    pub sort_order: i64,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutineRun {
    pub id: i64,
    pub routine_id: i64,
    pub local_date: String,
    pub status: String,
    pub note: String,
    pub completed_at: Option<String>,
    pub items: Vec<RoutineRunItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutineRunSummary {
    pub id: i64,
    pub local_date: String,
    pub status: String,
    pub completed: u32,
    pub skipped: u32,
    pub pending: u32,
}

fn invalid_input(message: &str) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(message.to_string())
}

fn validate_title(value: &str, label: &str, max: usize) -> Result<&str> {
    let normalized = value.trim();
    if normalized.is_empty() || normalized.chars().count() > max {
        return Err(invalid_input(&format!(
            "{label} must contain between 1 and {max} characters"
        )));
    }
    Ok(normalized)
}

fn parse_date(value: &str) -> Option<CivilDate> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u8>().ok()?;
    let day = parts.next()?.parse::<u8>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    let date = CivilDate::new(year, month, day)?;
    (date.to_string() == value).then_some(date)
}

fn validate_draft(draft: &RoutineDraft) -> Result<()> {
    validate_title(&draft.title, "routine title", 120)?;
    if draft.description.chars().count() > 2_000 {
        return Err(invalid_input(
            "routine description must be 2,000 characters or fewer",
        ));
    }
    let start = parse_date(&draft.start_date)
        .ok_or_else(|| invalid_input("start date must be a real YYYY-MM-DD date"))?;
    if let Some(end_text) = draft
        .end_date
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let end = parse_date(end_text)
            .ok_or_else(|| invalid_input("end date must be a real YYYY-MM-DD date"))?;
        if end < start {
            return Err(invalid_input("end date cannot be before start date"));
        }
    }
    if let RoutineSchedule::Weekdays { weekday_mask } = draft.schedule {
        if weekday_mask == 0 || weekday_mask & !0b0111_1111 != 0 {
            return Err(invalid_input("select at least one valid weekday"));
        }
    }
    if draft.steps.is_empty() || draft.steps.len() > 40 {
        return Err(invalid_input(
            "a routine must contain between 1 and 40 steps",
        ));
    }
    if !draft.steps.iter().any(|step| step.required) {
        return Err(invalid_input("at least one routine step must be required"));
    }
    for step in &draft.steps {
        validate_title(&step.title, "step title", 160)?;
        if step.details.chars().count() > 1_000 {
            return Err(invalid_input(
                "step details must be 1,000 characters or fewer",
            ));
        }
    }
    Ok(())
}

/// Apply additive routine tables using a separate migration ledger.
pub fn ensure_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS routine_schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );",
    )?;
    let version: i64 = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM routine_schema_migrations",
        [],
        |row| row.get(0),
    )?;
    if version > 1 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    if version < 1 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(ROUTINE_MIGRATION)?;
        transaction.execute(
            "INSERT INTO routine_schema_migrations(version) VALUES (1)",
            [],
        )?;
        transaction.commit()?;
    }
    Ok(())
}

fn schedule_parts(schedule: RoutineSchedule) -> (&'static str, Option<i64>) {
    match schedule {
        RoutineSchedule::Daily => ("daily", None),
        RoutineSchedule::Weekdays { weekday_mask } => ("weekdays", Some(i64::from(weekday_mask))),
    }
}

fn validate_step_links(connection: &Connection, steps: &[NewRoutineStep]) -> Result<()> {
    for step in steps {
        if let RoutineStepKind::Habit { habit_id } = &step.kind {
            let available: bool = connection.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM habits WHERE id = ?1
                )",
                [habit_id],
                |row| row.get(0),
            )?;
            if !available {
                return Err(invalid_input(
                    "linked habit must exist and must not be archived",
                ));
            }
        }
    }
    Ok(())
}

fn insert_steps(connection: &Connection, routine_id: i64, steps: &[NewRoutineStep]) -> Result<()> {
    for (index, step) in steps.iter().enumerate() {
        let (kind, habit_id) = match &step.kind {
            RoutineStepKind::Action => ("action", None),
            RoutineStepKind::Habit { habit_id } => ("habit", Some(*habit_id)),
        };
        connection.execute(
            "INSERT INTO routine_steps
                (routine_id, title, details, step_type, linked_habit_id, is_required, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                routine_id,
                step.title.trim(),
                step.details.trim(),
                kind,
                habit_id,
                step.required,
                index as i64
            ],
        )?;
    }
    Ok(())
}

pub fn create_routine(connection: &Connection, draft: &RoutineDraft) -> Result<i64> {
    validate_draft(draft)?;
    validate_step_links(connection, &draft.steps)?;
    let (schedule_type, weekday_mask) = schedule_parts(draft.schedule);
    let end_date = draft
        .end_date
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let transaction = connection.unchecked_transaction()?;
    transaction.execute(
        "INSERT INTO routines
            (title, description, schedule_type, weekday_mask, start_date, end_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            draft.title.trim(),
            draft.description.trim(),
            schedule_type,
            weekday_mask,
            draft.start_date,
            end_date
        ],
    )?;
    let routine_id = transaction.last_insert_rowid();
    insert_steps(&transaction, routine_id, &draft.steps)?;
    transaction.commit()?;
    Ok(routine_id)
}

/// Replace the editable template atomically. Existing routine-run snapshots are untouched.
pub fn update_routine(
    connection: &Connection,
    routine_id: i64,
    draft: &RoutineDraft,
) -> Result<()> {
    validate_draft(draft)?;
    validate_step_links(connection, &draft.steps)?;
    let (schedule_type, weekday_mask) = schedule_parts(draft.schedule);
    let end_date = draft
        .end_date
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let transaction = connection.unchecked_transaction()?;
    let changed = transaction.execute(
        "UPDATE routines
         SET title = ?1, description = ?2, schedule_type = ?3, weekday_mask = ?4,
             start_date = ?5, end_date = ?6, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?7 AND status != 'archived'",
        params![
            draft.title.trim(),
            draft.description.trim(),
            schedule_type,
            weekday_mask,
            draft.start_date,
            end_date,
            routine_id
        ],
    )?;
    if changed == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    transaction.execute(
        "DELETE FROM routine_steps WHERE routine_id = ?1",
        [routine_id],
    )?;
    insert_steps(&transaction, routine_id, &draft.steps)?;
    transaction.commit()?;
    Ok(())
}

pub fn set_routine_status(connection: &Connection, routine_id: i64, status: &str) -> Result<()> {
    if !["active", "paused", "archived"].contains(&status) {
        return Err(invalid_input("invalid routine status"));
    }
    let changed = connection.execute(
        "UPDATE routines SET status = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
        params![status, routine_id],
    )?;
    if changed == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

pub fn list_routines(
    connection: &Connection,
    include_archived: bool,
) -> Result<Vec<RoutineSummary>> {
    let mut statement = connection.prepare(
        "SELECT r.id, r.title, r.description, r.schedule_type, r.weekday_mask,
                r.start_date, r.end_date, r.status, COUNT(s.id)
         FROM routines r
         LEFT JOIN routine_steps s ON s.routine_id = r.id
         WHERE (?1 = 1 OR r.status != 'archived')
         GROUP BY r.id
         ORDER BY CASE r.status WHEN 'active' THEN 0 WHEN 'paused' THEN 1 ELSE 2 END,
                  r.sort_order, r.title COLLATE NOCASE",
    )?;
    let rows = statement.query_map([include_archived], |row| {
        let schedule_type: String = row.get(3)?;
        let mask: Option<i64> = row.get(4)?;
        let schedule = match schedule_type.as_str() {
            "daily" => RoutineSchedule::Daily,
            "weekdays" => RoutineSchedule::Weekdays {
                weekday_mask: mask.and_then(|value| u8::try_from(value).ok()).unwrap_or(0),
            },
            _ => return Err(rusqlite::Error::InvalidQuery),
        };
        Ok(RoutineSummary {
            id: row.get(0)?,
            title: row.get(1)?,
            description: row.get(2)?,
            schedule,
            start_date: row.get(5)?,
            end_date: row.get(6)?,
            status: row.get(7)?,
            step_count: row.get::<_, i64>(8)?.max(0) as u32,
        })
    })?;
    rows.collect()
}

pub fn load_routine(connection: &Connection, routine_id: i64) -> Result<RoutineDefinition> {
    let routine = connection
        .query_row(
            "SELECT r.id, r.title, r.description, r.schedule_type, r.weekday_mask,
                    r.start_date, r.end_date, r.status,
                    (SELECT COUNT(*) FROM routine_steps s WHERE s.routine_id = r.id)
             FROM routines r WHERE r.id = ?1",
            [routine_id],
            |row| {
                let schedule_type: String = row.get(3)?;
                let mask: Option<i64> = row.get(4)?;
                let schedule = match schedule_type.as_str() {
                    "daily" => RoutineSchedule::Daily,
                    "weekdays" => RoutineSchedule::Weekdays {
                        weekday_mask: mask.and_then(|value| u8::try_from(value).ok()).unwrap_or(0),
                    },
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                Ok(RoutineSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    schedule,
                    start_date: row.get(5)?,
                    end_date: row.get(6)?,
                    status: row.get(7)?,
                    step_count: row.get::<_, i64>(8)?.max(0) as u32,
                })
            },
        )
        .optional()?
        .ok_or(rusqlite::Error::QueryReturnedNoRows)?;

    let steps = {
        let mut statement = connection.prepare(
            "SELECT s.id, s.routine_id, s.title, s.details, s.step_type, s.linked_habit_id,
                    h.name, s.is_required, s.sort_order
             FROM routine_steps s
             LEFT JOIN habits h ON h.id = s.linked_habit_id
             WHERE s.routine_id = ?1
             ORDER BY s.sort_order, s.id",
        )?;
        let rows = statement.query_map([routine_id], |row| {
            let step_type: String = row.get(4)?;
            let linked_habit_id: Option<i64> = row.get(5)?;
            let kind = match step_type.as_str() {
                "action" => RoutineStepKind::Action,
                "habit" => RoutineStepKind::Habit {
                    habit_id: linked_habit_id.ok_or(rusqlite::Error::InvalidQuery)?,
                },
                _ => return Err(rusqlite::Error::InvalidQuery),
            };
            Ok(RoutineStep {
                id: row.get(0)?,
                routine_id: row.get(1)?,
                title: row.get(2)?,
                details: row.get(3)?,
                kind,
                linked_habit_name: row.get(6)?,
                required: row.get::<_, i64>(7)? != 0,
                sort_order: row.get(8)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()?
    };

    Ok(RoutineDefinition { routine, steps })
}

pub fn list_linkable_habits(connection: &Connection) -> Result<Vec<LinkableHabit>> {
    let mut statement = connection.prepare(
        "SELECT id, name FROM habits WHERE archived_at IS NULL ORDER BY name COLLATE NOCASE",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(LinkableHabit {
            id: row.get(0)?,
            name: row.get(1)?,
        })
    })?;
    rows.collect()
}

fn is_scheduled(routine: &RoutineSummary, date: CivilDate) -> bool {
    let Some(start) = parse_date(&routine.start_date) else {
        return false;
    };
    if date < start
        || routine
            .end_date
            .as_deref()
            .and_then(parse_date)
            .is_some_and(|end| date > end)
    {
        return false;
    }
    match routine.schedule {
        RoutineSchedule::Daily => true,
        RoutineSchedule::Weekdays { weekday_mask } => {
            weekday_mask & (1_u8 << date.weekday_monday_zero()) != 0
        }
    }
}

pub fn local_today(connection: &Connection) -> Result<String> {
    connection.query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))
}

/// Start a single run per routine per local date and snapshot the current ordered template.
pub fn start_run(connection: &Connection, routine_id: i64, local_date: &str) -> Result<RoutineRun> {
    let date = parse_date(local_date)
        .ok_or_else(|| invalid_input("run date must be a real YYYY-MM-DD date"))?;
    let definition = load_routine(connection, routine_id)?;
    if definition.routine.status != "active" {
        return Err(invalid_input("only active routines can be started"));
    }
    if !is_scheduled(&definition.routine, date) {
        return Err(invalid_input(
            "this routine is not scheduled for the selected date",
        ));
    }
    if definition.steps.is_empty() {
        return Err(invalid_input(
            "add at least one step before starting a routine",
        ));
    }

    let transaction = connection.unchecked_transaction()?;
    transaction.execute(
        "INSERT OR IGNORE INTO routine_runs(routine_id, local_date) VALUES (?1, ?2)",
        params![routine_id, local_date],
    )?;
    let run_id: i64 = transaction.query_row(
        "SELECT id FROM routine_runs WHERE routine_id = ?1 AND local_date = ?2",
        params![routine_id, local_date],
        |row| row.get(0),
    )?;
    let existing_items: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM routine_run_steps WHERE run_id = ?1",
        [run_id],
        |row| row.get(0),
    )?;
    if existing_items == 0 {
        for step in &definition.steps {
            let (step_type, linked_habit_id) = match &step.kind {
                RoutineStepKind::Action => ("action", None),
                RoutineStepKind::Habit { habit_id } => ("habit", Some(*habit_id)),
            };
            transaction.execute(
                "INSERT INTO routine_run_steps
                    (run_id, step_id, title_snapshot, step_type_snapshot,
                     linked_habit_id_snapshot, is_required, sort_order)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    run_id,
                    step.id,
                    step.title,
                    step_type,
                    linked_habit_id,
                    step.required,
                    step.sort_order
                ],
            )?;
        }
    }
    transaction.commit()?;
    load_run(connection, run_id)
}

pub fn load_run(connection: &Connection, run_id: i64) -> Result<RoutineRun> {
    let (routine_id, local_date, status, note, completed_at) = connection.query_row(
        "SELECT routine_id, local_date, status, note, completed_at
         FROM routine_runs WHERE id = ?1",
        [run_id],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        },
    )?;
    let items = {
        let mut statement = connection.prepare(
            "SELECT step_id, title_snapshot, step_type_snapshot,
                    linked_habit_id_snapshot, is_required, sort_order, status
             FROM routine_run_steps WHERE run_id = ?1 ORDER BY sort_order, id",
        )?;
        let rows = statement.query_map([run_id], |row| {
            Ok(RoutineRunItem {
                step_id: row.get(0)?,
                title_snapshot: row.get(1)?,
                step_type_snapshot: row.get(2)?,
                linked_habit_id_snapshot: row.get(3)?,
                required: row.get::<_, i64>(4)? != 0,
                sort_order: row.get(5)?,
                status: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()?
    };
    Ok(RoutineRun {
        id: run_id,
        routine_id,
        local_date,
        status,
        note,
        completed_at,
        items,
    })
}

pub fn set_run_step_status(
    connection: &Connection,
    run_id: i64,
    step_id: i64,
    status: &str,
) -> Result<()> {
    if !["pending", "completed", "skipped"].contains(&status) {
        return Err(invalid_input("invalid routine step status"));
    }
    let transaction = connection.unchecked_transaction()?;
    let changed = transaction.execute(
        "UPDATE routine_run_steps SET status = ?1, updated_at = CURRENT_TIMESTAMP
         WHERE run_id = ?2 AND step_id = ?3",
        params![status, run_id, step_id],
    )?;
    if changed == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    transaction.execute(
        "UPDATE routine_runs
         SET status = CASE WHEN NOT EXISTS (
                SELECT 1 FROM routine_run_steps
                WHERE run_id = ?1 AND is_required = 1 AND status = 'pending'
             ) THEN 'completed' ELSE 'in_progress' END,
             completed_at = CASE WHEN NOT EXISTS (
                SELECT 1 FROM routine_run_steps
                WHERE run_id = ?1 AND is_required = 1 AND status = 'pending'
             ) THEN COALESCE(completed_at, CURRENT_TIMESTAMP) ELSE NULL END
         WHERE id = ?1",
        [run_id],
    )?;
    transaction.commit()?;
    Ok(())
}

pub fn list_run_history(
    connection: &Connection,
    routine_id: i64,
    limit: u16,
) -> Result<Vec<RoutineRunSummary>> {
    let limit = i64::from(limit.clamp(1, 100));
    let mut statement = connection.prepare(
        "SELECT r.id, r.local_date, r.status,
                SUM(CASE WHEN s.status = 'completed' THEN 1 ELSE 0 END),
                SUM(CASE WHEN s.status = 'skipped' THEN 1 ELSE 0 END),
                SUM(CASE WHEN s.status = 'pending' THEN 1 ELSE 0 END)
         FROM routine_runs r
         LEFT JOIN routine_run_steps s ON s.run_id = r.id
         WHERE r.routine_id = ?1
         GROUP BY r.id
         ORDER BY r.local_date DESC
         LIMIT ?2",
    )?;
    let rows = statement.query_map(params![routine_id, limit], |row| {
        Ok(RoutineRunSummary {
            id: row.get(0)?,
            local_date: row.get(1)?,
            status: row.get(2)?,
            completed: row.get::<_, Option<i64>>(3)?.unwrap_or(0).max(0) as u32,
            skipped: row.get::<_, Option<i64>>(4)?.unwrap_or(0).max(0) as u32,
            pending: row.get::<_, Option<i64>>(5)?.unwrap_or(0).max(0) as u32,
        })
    })?;
    rows.collect()
}

fn schedule_label(schedule: RoutineSchedule) -> String {
    match schedule {
        RoutineSchedule::Daily => "Every day".to_string(),
        RoutineSchedule::Weekdays { weekday_mask } => {
            const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
            let selected = DAYS
                .iter()
                .enumerate()
                .filter(|(index, _)| weekday_mask & (1_u8 << *index) != 0)
                .map(|(_, day)| *day)
                .collect::<Vec<_>>();
            format!("Every {}", selected.join(", "))
        }
    }
}

pub fn open_window(parent: &ApplicationWindow, connection: Rc<RefCell<Connection>>) {
    if let Err(error) = ensure_schema(&connection.borrow()) {
        eprintln!("Could not initialize routine storage: {error}");
        return;
    }

    let window = gtk::Window::builder()
        .title("Routine Builder")
        .transient_for(parent)
        .default_width(860)
        .default_height(660)
        .build();
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.set_margin_top(22);
    page.set_margin_bottom(22);
    page.set_margin_start(24);
    page.set_margin_end(24);

    let heading = gtk::Label::new(Some("Routine Builder"));
    heading.add_css_class("title-1");
    heading.set_xalign(0.0);
    let subtitle = gtk::Label::new(Some(
        "Create repeatable checklists for your morning, study, work or evening flow.",
    ));
    subtitle.add_css_class("dim-label");
    subtitle.set_xalign(0.0);
    let create = gtk::Button::with_label("Create routine");
    create.add_css_class("suggested-action");
    create.set_halign(gtk::Align::Start);
    let message = gtk::Label::new(Some(
        "Routine steps are separate from habit check-ins. Linking a habit does not change its score.",
    ));
    message.set_wrap(true);
    message.add_css_class("dim-label");
    message.set_xalign(0.0);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 10);
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_vexpand(true);
    scroller.set_child(Some(&list));

    page.append(&heading);
    page.append(&subtitle);
    page.append(&create);
    page.append(&message);
    page.append(&scroller);
    window.set_child(Some(&page));

    let refresh_parent = window.clone();
    let refresh_list = list.clone();
    let refresh_connection = Rc::clone(&connection);
    let refresh: Refresh = Rc::new(move || {
        refresh_routine_list(&refresh_parent, &refresh_list, &refresh_connection);
    });

    let editor_parent = window.clone();
    let editor_connection = Rc::clone(&connection);
    let editor_refresh = Rc::clone(&refresh);
    create.connect_clicked(move |_| {
        show_editor(
            &editor_parent,
            Rc::clone(&editor_connection),
            None,
            Rc::clone(&editor_refresh),
        );
    });

    refresh();
    window.present();
}

fn refresh_routine_list(
    parent: &gtk::Window,
    list: &gtk::Box,
    connection: &Rc<RefCell<Connection>>,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let routines = match list_routines(&connection.borrow(), false) {
        Ok(items) => items,
        Err(error) => {
            append_message(list, &format!("Could not load routines: {error}"));
            return;
        }
    };
    if routines.is_empty() {
        append_message(
            list,
            "No routines yet. Create your first repeatable checklist.",
        );
        return;
    }

    for routine in routines {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
        card.add_css_class("card");
        card.set_margin_top(4);
        card.set_margin_bottom(4);
        card.set_margin_start(4);
        card.set_margin_end(4);
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = gtk::Label::new(Some(&routine.title));
        title.add_css_class("title-3");
        title.set_xalign(0.0);
        title.set_hexpand(true);
        let state = gtk::Label::new(Some(&routine.status));
        state.add_css_class("dim-label");
        top.append(&title);
        top.append(&state);

        let detail = gtk::Label::new(Some(&format!(
            "{} · {} steps · starts {}{}",
            schedule_label(routine.schedule),
            routine.step_count,
            routine.start_date,
            routine
                .end_date
                .as_ref()
                .map(|end| format!(" · ends {end}"))
                .unwrap_or_default()
        )));
        detail.set_xalign(0.0);
        detail.add_css_class("dim-label");
        detail.set_wrap(true);

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let edit = gtk::Button::with_label("Edit");
        let run = gtk::Button::with_label("Run today");
        let history = gtk::Button::with_label("History");
        let toggle = gtk::Button::with_label(if routine.status == "paused" {
            "Resume"
        } else {
            "Pause"
        });
        let archive = gtk::Button::with_label("Archive");
        run.set_sensitive(routine.status == "active");
        for button in [&edit, &run, &history, &toggle, &archive] {
            actions.append(button);
        }
        card.append(&top);
        if !routine.description.is_empty() {
            let description = gtk::Label::new(Some(&routine.description));
            description.set_xalign(0.0);
            description.set_wrap(true);
            card.append(&description);
        }
        card.append(&detail);
        card.append(&actions);
        list.append(&card);

        let edit_parent = parent.clone();
        let edit_connection = Rc::clone(connection);
        let edit_refresh = {
            let list = list.clone();
            let parent = parent.clone();
            let connection = Rc::clone(connection);
            Rc::new(move || refresh_routine_list(&parent, &list, &connection)) as Refresh
        };
        let edit_id = routine.id;
        edit.connect_clicked(move |_| {
            let definition = match load_routine(&edit_connection.borrow(), edit_id) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("Could not load routine editor: {error}");
                    return;
                }
            };
            show_editor(
                &edit_parent,
                Rc::clone(&edit_connection),
                Some(definition),
                Rc::clone(&edit_refresh),
            );
        });

        let run_parent = parent.clone();
        let run_connection = Rc::clone(connection);
        let run_id = routine.id;
        run.connect_clicked(move |_| {
            show_run_window(&run_parent, Rc::clone(&run_connection), run_id);
        });

        let history_parent = parent.clone();
        let history_connection = Rc::clone(connection);
        let history_id = routine.id;
        let history_title = routine.title.clone();
        history.connect_clicked(move |_| {
            show_history_window(
                &history_parent,
                &history_connection,
                history_id,
                &history_title,
            );
        });

        let toggle_connection = Rc::clone(connection);
        let toggle_id = routine.id;
        let toggle_status = routine.status.clone();
        let toggle_list = list.clone();
        let toggle_parent = parent.clone();
        toggle.connect_clicked(move |_| {
            let next = if toggle_status == "paused" {
                "active"
            } else {
                "paused"
            };
            if let Err(error) = set_routine_status(&toggle_connection.borrow(), toggle_id, next) {
                eprintln!("Could not update routine status: {error}");
            }
            refresh_routine_list(&toggle_parent, &toggle_list, &toggle_connection);
        });

        let archive_connection = Rc::clone(connection);
        let archive_id = routine.id;
        let archive_list = list.clone();
        let archive_parent = parent.clone();
        archive.connect_clicked(move |_| {
            if let Err(error) =
                set_routine_status(&archive_connection.borrow(), archive_id, "archived")
            {
                eprintln!("Could not archive routine: {error}");
            }
            refresh_routine_list(&archive_parent, &archive_list, &archive_connection);
        });
    }
}

fn show_editor(
    parent: &gtk::Window,
    connection: Rc<RefCell<Connection>>,
    existing: Option<RoutineDefinition>,
    on_saved: Refresh,
) {
    let editing = existing.is_some();
    let current_date = local_today(&connection.borrow()).unwrap_or_default();
    let routine = existing.as_ref().map(|value| value.routine.clone());
    let initial_steps = existing
        .as_ref()
        .map(|value| {
            value
                .steps
                .iter()
                .map(|step| NewRoutineStep {
                    title: step.title.clone(),
                    details: step.details.clone(),
                    kind: step.kind.clone(),
                    required: step.required,
                })
                .collect()
        })
        .unwrap_or_else(Vec::new);
    let drafts = Rc::new(RefCell::new(initial_steps));

    let editor = gtk::Window::builder()
        .title(if editing {
            "Edit routine"
        } else {
            "Create routine"
        })
        .transient_for(parent)
        .modal(true)
        .default_width(720)
        .default_height(700)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(20);
    content.set_margin_end(20);

    let title_label = gtk::Label::new(Some("Routine name"));
    title_label.set_xalign(0.0);
    let title_entry = gtk::Entry::new();
    title_entry.set_placeholder_text(Some("e.g. Morning focus"));
    if let Some(value) = routine.as_ref() {
        title_entry.set_text(&value.title);
    }
    let description_label = gtk::Label::new(Some("Description"));
    description_label.set_xalign(0.0);
    let description_entry = gtk::Entry::new();
    description_entry.set_placeholder_text(Some("What is this routine for?"));
    if let Some(value) = routine.as_ref() {
        description_entry.set_text(&value.description);
    }

    let schedule_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let schedule_label = gtk::Label::new(Some("Repeat"));
    schedule_label.set_xalign(0.0);
    let schedule_picker = gtk::DropDown::from_strings(&["Every day", "Selected weekdays"]);
    let existing_schedule = routine.as_ref().map(|value| value.schedule);
    schedule_picker.set_selected(
        if matches!(existing_schedule, Some(RoutineSchedule::Weekdays { .. })) {
            1
        } else {
            0
        },
    );
    schedule_row.append(&schedule_label);
    schedule_row.append(&schedule_picker);

    let weekday_row = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    let weekday_names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let mut weekday_checks = Vec::new();
    let initial_mask = match existing_schedule {
        Some(RoutineSchedule::Weekdays { weekday_mask }) => weekday_mask,
        _ => 0b001_1111,
    };
    for (index, name) in weekday_names.iter().enumerate() {
        let check = gtk::CheckButton::with_label(name);
        check.set_active(initial_mask & (1_u8 << index) != 0);
        check.set_tooltip_text(Some(&format!("Include {name} in this routine's schedule")));
        weekday_row.append(&check);
        weekday_checks.push(check);
    }

    let date_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let start_date = gtk::Entry::new();
    start_date.set_placeholder_text(Some("Start date (YYYY-MM-DD)"));
    start_date.set_text(
        routine
            .as_ref()
            .map(|value| value.start_date.as_str())
            .unwrap_or(&current_date),
    );
    start_date.set_hexpand(true);
    let end_date = gtk::Entry::new();
    end_date.set_placeholder_text(Some("Optional end date"));
    end_date.set_hexpand(true);
    if let Some(value) = routine.as_ref() {
        if let Some(end) = value.end_date.as_deref() {
            end_date.set_text(end);
        }
    }
    date_row.append(&start_date);
    date_row.append(&end_date);

    let steps_heading = gtk::Label::new(Some("Ordered steps"));
    steps_heading.add_css_class("title-3");
    steps_heading.set_xalign(0.0);
    let step_add_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let step_title = gtk::Entry::new();
    step_title.set_placeholder_text(Some("Add a step"));
    step_title.set_hexpand(true);
    let habits = list_linkable_habits(&connection.borrow()).unwrap_or_default();
    let mut habit_options = vec!["Action (not linked to a habit)".to_string()];
    habit_options.extend(habits.iter().map(|habit| format!("Habit: {}", habit.name)));
    let habit_picker =
        gtk::DropDown::from_strings(&habit_options.iter().map(String::as_str).collect::<Vec<_>>());
    let add_step = gtk::Button::with_label("Add step");
    step_add_row.append(&step_title);
    step_add_row.append(&habit_picker);
    step_add_row.append(&add_step);

    let step_list = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let step_scroller = gtk::ScrolledWindow::new();
    step_scroller.set_min_content_height(140);
    step_scroller.set_vexpand(true);
    step_scroller.set_child(Some(&step_list));
    render_step_drafts(&step_list, Rc::clone(&drafts));

    let feedback = gtk::Label::new(None);
    feedback.set_xalign(0.0);
    feedback.set_wrap(true);
    feedback.add_css_class("error");
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    let save = gtk::Button::with_label(if editing {
        "Save changes"
    } else {
        "Create routine"
    });
    save.add_css_class("suggested-action");
    actions.append(&cancel);
    actions.append(&save);

    content.append(&title_label);
    content.append(&title_entry);
    content.append(&description_label);
    content.append(&description_entry);
    content.append(&schedule_row);
    content.append(&weekday_row);
    content.append(&date_row);
    content.append(&steps_heading);
    content.append(&step_add_row);
    content.append(&step_scroller);
    content.append(&feedback);
    content.append(&actions);
    let editor_scroller = gtk::ScrolledWindow::new();
    editor_scroller.set_child(Some(&content));
    editor.set_child(Some(&editor_scroller));

    let add_entry = step_title.clone();
    let add_picker = habit_picker.clone();
    let add_drafts = Rc::clone(&drafts);
    let add_list = step_list.clone();
    let add_habits = habits.clone();
    let add_feedback = feedback.clone();
    add_step.connect_clicked(move |_| {
        let title = add_entry.text().trim().to_string();
        if title.is_empty() {
            add_feedback.set_text("Enter a step title first.");
            return;
        }
        if add_drafts.borrow().len() >= 40 {
            add_feedback.set_text("A routine can contain at most 40 steps.");
            return;
        }
        let selected = add_picker.selected() as usize;
        let kind = if selected == 0 {
            RoutineStepKind::Action
        } else if let Some(habit) = add_habits.get(selected - 1) {
            RoutineStepKind::Habit { habit_id: habit.id }
        } else {
            add_feedback.set_text("Select a valid step type.");
            return;
        };
        add_drafts.borrow_mut().push(NewRoutineStep {
            title,
            details: String::new(),
            kind,
            required: true,
        });
        add_entry.set_text("");
        add_feedback.set_text("");
        render_step_drafts(&add_list, Rc::clone(&add_drafts));
    });

    let save_editor = editor.clone();
    let save_connection = Rc::clone(&connection);
    let save_title = title_entry.clone();
    let save_description = description_entry.clone();
    let save_schedule = schedule_picker.clone();
    let save_weekdays = weekday_checks.clone();
    let save_start = start_date.clone();
    let save_end = end_date.clone();
    let save_drafts = Rc::clone(&drafts);
    let save_feedback = feedback.clone();
    let save_callback = Rc::clone(&on_saved);
    let routine_id = routine.as_ref().map(|value| value.id);
    save.connect_clicked(move |_| {
        let schedule = if save_schedule.selected() == 0 {
            RoutineSchedule::Daily
        } else {
            let mut mask = 0_u8;
            for (index, check) in save_weekdays.iter().enumerate() {
                if check.is_active() {
                    mask |= 1_u8 << index;
                }
            }
            RoutineSchedule::Weekdays { weekday_mask: mask }
        };
        let end_text = save_end.text().trim().to_string();
        let draft = RoutineDraft {
            title: save_title.text().to_string(),
            description: save_description.text().to_string(),
            schedule,
            start_date: save_start.text().trim().to_string(),
            end_date: (!end_text.is_empty()).then_some(end_text),
            steps: save_drafts.borrow().clone(),
        };
        let result = match routine_id {
            Some(id) => update_routine(&save_connection.borrow(), id, &draft).map(|_| id),
            None => create_routine(&save_connection.borrow(), &draft),
        };
        match result {
            Ok(_) => {
                save_editor.close();
                save_callback();
            }
            Err(error) => save_feedback.set_text(&format!("Could not save routine: {error}")),
        }
    });

    let cancel_editor = editor.clone();
    cancel.connect_clicked(move |_| cancel_editor.close());
    editor.present();
    title_entry.grab_focus();
}

fn render_step_drafts(list: &gtk::Box, drafts: Rc<RefCell<Vec<NewRoutineStep>>>) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    if drafts.borrow().is_empty() {
        append_message(
            list,
            "No steps yet. Add an action or link an existing habit.",
        );
        return;
    }
    let count = drafts.borrow().len();
    for index in 0..count {
        let step = drafts.borrow()[index].clone();
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let number = gtk::Label::new(Some(&format!("{}.", index + 1)));
        let name = gtk::Label::new(Some(&format!(
            "{}{}",
            step.title,
            match step.kind {
                RoutineStepKind::Action => " · Action",
                RoutineStepKind::Habit { .. } => " · Linked habit",
            }
        )));
        name.set_xalign(0.0);
        name.set_hexpand(true);
        let required = gtk::CheckButton::with_label("Required");
        required.set_active(step.required);
        let state = Rc::clone(&drafts);
        required.connect_toggled(move |check| {
            if let Some(item) = state.borrow_mut().get_mut(index) {
                item.required = check.is_active();
            }
        });
        let up = gtk::Button::with_label("↑");
        up.set_tooltip_text(Some("Move this step earlier"));
        up.set_sensitive(index > 0);
        let up_state = Rc::clone(&drafts);
        let up_list = list.clone();
        up.connect_clicked(move |_| {
            if index > 0 {
                up_state.borrow_mut().swap(index, index - 1);
                render_step_drafts(&up_list, Rc::clone(&up_state));
            }
        });
        let down = gtk::Button::with_label("↓");
        down.set_tooltip_text(Some("Move this step later"));
        down.set_sensitive(index + 1 < count);
        let down_state = Rc::clone(&drafts);
        let down_list = list.clone();
        down.connect_clicked(move |_| {
            let len = down_state.borrow().len();
            if index + 1 < len {
                down_state.borrow_mut().swap(index, index + 1);
                render_step_drafts(&down_list, Rc::clone(&down_state));
            }
        });
        let remove = gtk::Button::with_label("Remove");
        let remove_state = Rc::clone(&drafts);
        let remove_list = list.clone();
        remove.connect_clicked(move |_| {
            if index < remove_state.borrow().len() {
                remove_state.borrow_mut().remove(index);
                render_step_drafts(&remove_list, Rc::clone(&remove_state));
            }
        });
        row.append(&number);
        row.append(&name);
        row.append(&required);
        row.append(&up);
        row.append(&down);
        row.append(&remove);
        list.append(&row);
    }
}

fn show_run_window(parent: &gtk::Window, connection: Rc<RefCell<Connection>>, routine_id: i64) {
    let date = match local_today(&connection.borrow()) {
        Ok(value) => value,
        Err(error) => {
            show_message(parent, &format!("Could not read local date: {error}"));
            return;
        }
    };
    let run = match start_run(&connection.borrow(), routine_id, &date) {
        Ok(value) => value,
        Err(error) => {
            show_message(parent, &format!("Could not start routine: {error}"));
            return;
        }
    };
    let title = load_routine(&connection.borrow(), routine_id)
        .map(|value| value.routine.title)
        .unwrap_or_else(|_| "Routine".to_string());

    let window = gtk::Window::builder()
        .title(&format!("{title} · {date}"))
        .transient_for(parent)
        .default_width(640)
        .default_height(560)
        .build();
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.set_margin_top(20);
    page.set_margin_bottom(20);
    page.set_margin_start(20);
    page.set_margin_end(20);
    let heading = gtk::Label::new(Some(&title));
    heading.add_css_class("title-1");
    heading.set_xalign(0.0);
    let summary = gtk::Label::new(None);
    summary.set_xalign(0.0);
    let items = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_vexpand(true);
    scroller.set_child(Some(&items));
    page.append(&heading);
    page.append(&summary);
    page.append(&scroller);
    window.set_child(Some(&page));
    populate_run_view(&connection, run.id, &summary, &items);
    window.present();
}

fn populate_run_view(
    connection: &Rc<RefCell<Connection>>,
    run_id: i64,
    summary: &gtk::Label,
    items_box: &gtk::Box,
) {
    while let Some(child) = items_box.first_child() {
        items_box.remove(&child);
    }
    let run = match load_run(&connection.borrow(), run_id) {
        Ok(value) => value,
        Err(error) => {
            summary.set_text(&format!("Could not reload routine: {error}"));
            return;
        }
    };
    let completed = run
        .items
        .iter()
        .filter(|item| item.status == "completed")
        .count();
    let skipped = run
        .items
        .iter()
        .filter(|item| item.status == "skipped")
        .count();
    let pending = run
        .items
        .iter()
        .filter(|item| item.status == "pending")
        .count();
    summary.set_text(&format!(
        "{} · {} completed · {} skipped · {} pending{}",
        run.status,
        completed,
        skipped,
        pending,
        if run.completed_at.is_some() {
            " · finished"
        } else {
            ""
        }
    ));

    for item in run.items {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let check = gtk::CheckButton::with_label(&item.title_snapshot);
        check.set_active(item.status == "completed");
        check.set_hexpand(true);
        if !item.required {
            check.set_tooltip_text(Some("Optional step"));
        }
        let step_id = item.step_id;
        let run_id_for_check = run.id;
        let check_connection = Rc::clone(connection);
        let check_summary = summary.clone();
        let check_box = items_box.clone();
        check.connect_toggled(move |button| {
            let status = if button.is_active() {
                "completed"
            } else {
                "pending"
            };
            if let Err(error) = set_run_step_status(
                &check_connection.borrow(),
                run_id_for_check,
                step_id,
                status,
            ) {
                eprintln!("Could not update routine step: {error}");
            }
            populate_run_view(
                &check_connection,
                run_id_for_check,
                &check_summary,
                &check_box,
            );
        });
        let skip = gtk::Button::with_label(if item.status == "skipped" {
            "Undo skip"
        } else {
            "Skip"
        });
        let skip_id = item.step_id;
        let skip_status = item.status.clone();
        let skip_connection = Rc::clone(connection);
        let skip_summary = summary.clone();
        let skip_box = items_box.clone();
        let skip_run = run.id;
        skip.connect_clicked(move |_| {
            let status = if skip_status == "skipped" {
                "pending"
            } else {
                "skipped"
            };
            if let Err(error) =
                set_run_step_status(&skip_connection.borrow(), skip_run, skip_id, status)
            {
                eprintln!("Could not skip routine step: {error}");
            }
            populate_run_view(&skip_connection, skip_run, &skip_summary, &skip_box);
        });
        row.append(&check);
        row.append(&skip);
        items_box.append(&row);
    }
}

fn show_history_window(
    parent: &gtk::Window,
    connection: &Rc<RefCell<Connection>>,
    routine_id: i64,
    title: &str,
) {
    let window = gtk::Window::builder()
        .title(&format!("{title} · History"))
        .transient_for(parent)
        .default_width(520)
        .default_height(420)
        .build();
    let list = gtk::Box::new(gtk::Orientation::Vertical, 8);
    list.set_margin_top(20);
    list.set_margin_bottom(20);
    list.set_margin_start(20);
    list.set_margin_end(20);
    match list_run_history(&connection.borrow(), routine_id, 50) {
        Ok(history) if history.is_empty() => append_message(&list, "No routine runs saved yet."),
        Ok(history) => {
            for run in history {
                let label = gtk::Label::new(Some(&format!(
                    "{} · {} · {} completed, {} skipped, {} pending",
                    run.local_date, run.status, run.completed, run.skipped, run.pending
                )));
                label.set_xalign(0.0);
                list.append(&label);
            }
        }
        Err(error) => append_message(&list, &format!("Could not load routine history: {error}")),
    }
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_child(Some(&list));
    window.set_child(Some(&scroller));
    window.present();
}

fn show_message(parent: &gtk::Window, message: &str) {
    let window = gtk::Window::builder()
        .title("Routine")
        .transient_for(parent)
        .modal(true)
        .default_width(440)
        .default_height(150)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(20);
    content.set_margin_end(20);
    let label = gtk::Label::new(Some(message));
    label.set_wrap(true);
    let close = gtk::Button::with_label("Close");
    let close_window = window.clone();
    close.connect_clicked(move |_| close_window.close());
    content.append(&label);
    content.append(&close);
    window.set_child(Some(&content));
    window.present();
}

fn append_message(list: &gtk::Box, message: &str) {
    let label = gtk::Label::new(Some(message));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_margin_top(12);
    label.set_margin_bottom(12);
    list.append(&label);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("open memory database");
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .expect("enable foreign keys");
        connection
            .execute_batch(include_str!("../migrations/0001_initial.sql"))
            .expect("apply base migration");
        connection
            .execute_batch(include_str!("../migrations/0002_pause_periods.sql"))
            .expect("apply pause migration");
        ensure_schema(&connection).expect("apply routine migration");
        connection
    }

    fn draft(schedule: RoutineSchedule, steps: Vec<NewRoutineStep>) -> RoutineDraft {
        RoutineDraft {
            title: "Morning focus".to_string(),
            description: "Start the day intentionally".to_string(),
            schedule,
            start_date: "2026-11-01".to_string(),
            end_date: None,
            steps,
        }
    }

    fn action(title: &str, required: bool) -> NewRoutineStep {
        NewRoutineStep {
            title: title.to_string(),
            details: String::new(),
            kind: RoutineStepKind::Action,
            required,
        }
    }

    #[test]
    fn creates_lists_and_updates_an_ordered_routine() {
        let connection = memory_connection();
        let id = create_routine(
            &connection,
            &draft(
                RoutineSchedule::Daily,
                vec![action("Drink water", true), action("Plan the day", true)],
            ),
        )
        .expect("create routine");
        let loaded = load_routine(&connection, id).expect("load routine");
        assert_eq!(loaded.steps[0].title, "Drink water");
        assert_eq!(loaded.steps[1].sort_order, 1);
        assert_eq!(
            list_routines(&connection, false)
                .expect("list routines")
                .len(),
            1
        );

        update_routine(
            &connection,
            id,
            &draft(
                RoutineSchedule::Weekdays {
                    weekday_mask: 0b001_1111,
                },
                vec![action("Plan the day", true), action("Drink water", true)],
            ),
        )
        .expect("update routine");
        let updated = load_routine(&connection, id).expect("reload routine");
        assert_eq!(updated.steps[0].title, "Plan the day");
        assert_eq!(
            updated.routine.schedule,
            RoutineSchedule::Weekdays {
                weekday_mask: 0b001_1111
            }
        );
    }

    #[test]
    fn selected_weekdays_and_date_bounds_are_enforced() {
        let connection = memory_connection();
        let id = create_routine(
            &connection,
            &draft(
                RoutineSchedule::Weekdays {
                    weekday_mask: 0b000_0010,
                },
                vec![action("Read", true)],
            ),
        )
        .expect("create weekday routine");
        assert!(start_run(&connection, id, "2026-11-02").is_ok()); // Monday
        assert!(start_run(&connection, id, "2026-11-03").is_err()); // Tuesday
    }

    #[test]
    fn routine_run_is_idempotent_and_keeps_a_step_snapshot_after_template_edit() {
        let connection = memory_connection();
        let id = create_routine(
            &connection,
            &draft(RoutineSchedule::Daily, vec![action("Original step", true)]),
        )
        .expect("create routine");
        let first = start_run(&connection, id, "2026-11-02").expect("start run");
        let again = start_run(&connection, id, "2026-11-02").expect("resume same run");
        assert_eq!(first.id, again.id);

        update_routine(
            &connection,
            id,
            &draft(RoutineSchedule::Daily, vec![action("Renamed step", true)]),
        )
        .expect("edit routine");
        let history = load_run(&connection, first.id).expect("load saved run");
        assert_eq!(history.items[0].title_snapshot, "Original step");
    }

    #[test]
    fn required_steps_control_run_completion_and_skips_remain_visible() {
        let connection = memory_connection();
        let id = create_routine(
            &connection,
            &draft(
                RoutineSchedule::Daily,
                vec![action("Required", true), action("Optional", false)],
            ),
        )
        .expect("create routine");
        let run = start_run(&connection, id, "2026-11-02").expect("start run");
        set_run_step_status(&connection, run.id, run.items[0].step_id, "skipped")
            .expect("skip required step");
        let saved = load_run(&connection, run.id).expect("reload run");
        assert_eq!(saved.status, "completed");
        assert_eq!(saved.items[0].status, "skipped");
        assert_eq!(saved.items[1].status, "pending");
        set_run_step_status(&connection, run.id, run.items[0].step_id, "pending")
            .expect("reopen required step");
        assert_eq!(load_run(&connection, run.id).unwrap().status, "in_progress");
    }

    #[test]
    fn linked_habit_must_exist_and_be_active() {
        let connection = memory_connection();
        connection
            .execute("INSERT INTO habits(name) VALUES ('Read 20 pages')", [])
            .expect("insert habit");
        let id = connection.last_insert_rowid();
        let linked = NewRoutineStep {
            title: "Read".to_string(),
            details: String::new(),
            kind: RoutineStepKind::Habit { habit_id: id },
            required: true,
        };
        let routine_id = create_routine(&connection, &draft(RoutineSchedule::Daily, vec![linked]))
            .expect("link active habit");
        assert_eq!(
            load_routine(&connection, routine_id).unwrap().steps.len(),
            1
        );
        connection
            .execute(
                "UPDATE habits SET archived_at = '2026-11-14' WHERE id = ?1",
                [id],
            )
            .expect("archive habit");
        assert!(
            update_routine(
                &connection,
                routine_id,
                &draft(
                    RoutineSchedule::Daily,
                    vec![NewRoutineStep {
                        title: "Read".to_string(),
                        details: String::new(),
                        kind: RoutineStepKind::Habit { habit_id: 999 },
                        required: true,
                    }]
                )
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_invalid_dates_masks_and_empty_required_step_sets() {
        let connection = memory_connection();
        let mut invalid = draft(RoutineSchedule::Daily, vec![action("Read", true)]);
        invalid.start_date = "2027-02-29".to_string();
        assert!(create_routine(&connection, &invalid).is_err());
        let invalid_mask = draft(
            RoutineSchedule::Weekdays { weekday_mask: 0 },
            vec![action("Read", true)],
        );
        assert!(create_routine(&connection, &invalid_mask).is_err());
        let optional_only = draft(RoutineSchedule::Daily, vec![action("Optional", false)]);
        assert!(create_routine(&connection, &optional_only).is_err());
    }

    #[test]
    fn archived_routines_are_hidden_from_default_list_but_can_be_restored() {
        let connection = memory_connection();
        let id = create_routine(
            &connection,
            &draft(RoutineSchedule::Daily, vec![action("Read", true)]),
        )
        .expect("create routine");
        set_routine_status(&connection, id, "archived").expect("archive");
        assert!(list_routines(&connection, false).unwrap().is_empty());
        assert_eq!(list_routines(&connection, true).unwrap().len(), 1);
        set_routine_status(&connection, id, "active").expect("restore");
        assert_eq!(list_routines(&connection, false).unwrap().len(), 1);
    }
}
