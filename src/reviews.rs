//! Local weekly and monthly review notes.
//!
//! Reflections are private user content stored only in the existing SQLite database.
//! Activity summaries count persisted records only; they do not estimate adherence,
//! missed opportunities, or psychological/clinical outcomes.

use gtk::prelude::*;
use rusqlite::{Connection, OptionalExtension, Result, params};
use std::{cell::RefCell, rc::Rc};

const REVIEW_MIGRATION: &str = include_str!("../migrations/0004_period_reviews.sql");
const MAX_NOTE_CHARS: usize = 8_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodReview {
    pub id: i64,
    pub period_kind: String,
    pub period_start: String,
    pub went_well: String,
    pub challenges: String,
    pub lessons: String,
    pub next_focus: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivitySummary {
    pub completed: u32,
    pub partial: u32,
    pub skipped: u32,
}

fn invalid_field(message: &str) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(message.to_string())
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

    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=max_day).contains(&day)
}

fn validate_kind(kind: &str) -> Result<&str> {
    match kind {
        "week" | "month" => Ok(kind),
        _ => Err(invalid_field("review period must be week or month")),
    }
}

fn validate_start(connection: &Connection, kind: &str, value: &str) -> Result<String> {
    let kind = validate_kind(kind)?;
    let value = value.trim();
    if !valid_iso_date(value) {
        return Err(invalid_field("period start must be a real YYYY-MM-DD date"));
    }

    let valid: bool = connection.query_row(
        "SELECT CASE
            WHEN ?1 = 'week' THEN strftime('%w', ?2) = '1'
            WHEN ?1 = 'month' THEN strftime('%d', ?2) = '01'
            ELSE 0
         END",
        params![kind, value],
        |row| row.get(0),
    )?;

    if !valid {
        return Err(invalid_field(
            "weekly reviews must start on Monday; monthly reviews must start on day 01",
        ));
    }
    Ok(value.to_string())
}

fn validate_note(value: &str, field: &str) -> Result<&str> {
    if value.chars().count() > MAX_NOTE_CHARS {
        return Err(invalid_field(&format!(
            "{field} must be 8,000 characters or fewer"
        )));
    }
    Ok(value)
}

/// Apply the review schema independently of the core habit migration version.
pub fn ensure_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS review_schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );",
    )?;

    let version: i64 = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM review_schema_migrations",
        [],
        |row| row.get(0),
    )?;
    if version > 1 {
        return Err(rusqlite::Error::InvalidQuery);
    }

    if version < 1 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(REVIEW_MIGRATION)?;
        transaction.execute(
            "INSERT INTO review_schema_migrations(version) VALUES (1)",
            [],
        )?;
        transaction.commit()?;
    }

    Ok(())
}

pub fn current_period_start(connection: &Connection, kind: &str) -> Result<String> {
    ensure_schema(connection)?;
    let kind = validate_kind(kind)?;
    connection.query_row(
        "SELECT CASE
            WHEN ?1 = 'week' THEN date(
                'now', 'localtime',
                '-' || ((CAST(strftime('%w', 'now', 'localtime') AS INTEGER) + 6) % 7) || ' days'
            )
            ELSE date('now', 'localtime', 'start of month')
         END",
        [kind],
        |row| row.get(0),
    )
}

pub fn load_review(
    connection: &Connection,
    kind: &str,
    period_start: &str,
) -> Result<Option<PeriodReview>> {
    ensure_schema(connection)?;
    let kind = validate_kind(kind)?;
    let start = validate_start(connection, kind, period_start)?;
    connection
        .query_row(
            "SELECT id, period_kind, period_start, went_well, challenges, lessons, next_focus
             FROM period_reviews
             WHERE period_kind = ?1 AND period_start = ?2",
            params![kind, start],
            |row| {
                Ok(PeriodReview {
                    id: row.get(0)?,
                    period_kind: row.get(1)?,
                    period_start: row.get(2)?,
                    went_well: row.get(3)?,
                    challenges: row.get(4)?,
                    lessons: row.get(5)?,
                    next_focus: row.get(6)?,
                })
            },
        )
        .optional()
}

pub fn save_review(
    connection: &Connection,
    kind: &str,
    period_start: &str,
    went_well: &str,
    challenges: &str,
    lessons: &str,
    next_focus: &str,
) -> Result<i64> {
    ensure_schema(connection)?;
    let kind = validate_kind(kind)?;
    let start = validate_start(connection, kind, period_start)?;
    let went_well = validate_note(went_well, "wins")?;
    let challenges = validate_note(challenges, "challenges")?;
    let lessons = validate_note(lessons, "lessons")?;
    let next_focus = validate_note(next_focus, "next focus")?;

    connection.query_row(
        "INSERT INTO period_reviews
            (period_kind, period_start, went_well, challenges, lessons, next_focus)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(period_kind, period_start) DO UPDATE SET
            went_well = excluded.went_well,
            challenges = excluded.challenges,
            lessons = excluded.lessons,
            next_focus = excluded.next_focus,
            updated_at = CURRENT_TIMESTAMP
         RETURNING id",
        params![kind, start, went_well, challenges, lessons, next_focus],
        |row| row.get(0),
    )
}

pub fn activity_summary(
    connection: &Connection,
    kind: &str,
    period_start: &str,
) -> Result<ActivitySummary> {
    ensure_schema(connection)?;
    let kind = validate_kind(kind)?;
    let start = validate_start(connection, kind, period_start)?;
    let end: String = connection.query_row(
        "SELECT CASE
            WHEN ?1 = 'week' THEN date(?2, '+6 days')
            ELSE date(?2, '+1 month', '-1 day')
         END",
        params![kind, start],
        |row| row.get(0),
    )?;

    connection.query_row(
        "SELECT
            COALESCE(SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN status = 'partial' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN status = 'skipped' THEN 1 ELSE 0 END), 0)
         FROM habit_entries
         WHERE local_date >= ?1 AND local_date <= ?2",
        params![start, end],
        |row| {
            Ok(ActivitySummary {
                completed: row.get::<_, i64>(0)?.try_into().unwrap_or(0),
                partial: row.get::<_, i64>(1)?.try_into().unwrap_or(0),
                skipped: row.get::<_, i64>(2)?.try_into().unwrap_or(0),
            })
        },
    )
}

fn set_text(view: &gtk::TextView, value: &str) {
    view.buffer().set_text(value);
}

fn read_text(view: &gtk::TextView) -> String {
    let buffer = view.buffer();
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string()
}

fn note_field(title: &str, hint: &str) -> (gtk::Box, gtk::TextView) {
    let field = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let label = gtk::Label::new(Some(title));
    label.set_xalign(0.0);
    label.add_css_class("heading");

    let view = gtk::TextView::new();
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.set_left_margin(8);
    view.set_right_margin(8);
    view.set_top_margin(8);
    view.set_bottom_margin(8);
    view.set_tooltip_text(Some(hint));
    view.set_accessible_role(gtk::AccessibleRole::TextBox);
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_min_content_height(86);
    scroller.set_child(Some(&view));

    field.append(&label);
    field.append(&scroller);
    (field, view)
}

fn refresh_editor(
    connection: &Connection,
    kind: &str,
    start: &str,
    wins: &gtk::TextView,
    challenges: &gtk::TextView,
    lessons: &gtk::TextView,
    next_focus: &gtk::TextView,
    summary: &gtk::Label,
    feedback: &gtk::Label,
) {
    let review = match load_review(connection, kind, start) {
        Ok(review) => review,
        Err(error) => {
            feedback.set_text(&format!("Could not load review: {error}"));
            return;
        }
    };

    match review {
        Some(review) => {
            set_text(wins, &review.went_well);
            set_text(challenges, &review.challenges);
            set_text(lessons, &review.lessons);
            set_text(next_focus, &review.next_focus);
            feedback.set_text("Saved review loaded from this device.");
        }
        None => {
            set_text(wins, "");
            set_text(challenges, "");
            set_text(lessons, "");
            set_text(next_focus, "");
            feedback.set_text("No review saved for this period yet.");
        }
    }

    match activity_summary(connection, kind, start) {
        Ok(counts) => summary.set_text(&format!(
            "Saved records only · {} completed · {} partial · {} skipped. This is not an adherence score.",
            counts.completed, counts.partial, counts.skipped
        )),
        Err(error) => summary.set_text(&format!("Activity summary unavailable: {error}")),
    }
}

/// Open an independent review window. Notes remain local and are never logged or uploaded.
pub fn open_window(parent: &gtk::ApplicationWindow, connection: Rc<RefCell<Connection>>) {
    if let Err(error) = ensure_schema(&connection.borrow()) {
        eprintln!("Could not prepare local review storage: {error}");
        return;
    }

    let window = gtk::Window::builder()
        .title("Weekly & Monthly Reviews")
        .transient_for(parent)
        .default_width(820)
        .default_height(760)
        .build();

    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.set_margin_top(20);
    root.set_margin_bottom(20);
    root.set_margin_start(24);
    root.set_margin_end(24);

    let heading = gtk::Label::new(Some("Reflect and plan"));
    heading.add_css_class("title-1");
    heading.set_xalign(0.0);
    let description = gtk::Label::new(Some(
        "Private reflections stored on this device. Choose a Monday for a weekly review or the first day of a month for a monthly review.",
    ));
    description.set_wrap(true);
    description.set_xalign(0.0);
    description.add_css_class("dim-label");

    let period_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let period_kind = gtk::DropDown::from_strings(&["Weekly · Monday to Sunday", "Monthly"]);
    let period_start = gtk::Entry::new();
    period_start.set_width_chars(14);
    period_start.set_placeholder_text(Some("YYYY-MM-DD"));
    period_start.set_tooltip_text(Some(
        "Week: Monday start date. Month: first calendar day.",
    ));
    let use_current = gtk::Button::with_label("Current period");
    let load = gtk::Button::with_label("Load");
    period_row.append(&period_kind);
    period_row.append(&period_start);
    period_row.append(&use_current);
    period_row.append(&load);

    let summary = gtk::Label::new(Some("Activity summary will appear here."));
    summary.set_xalign(0.0);
    summary.add_css_class("dim-label");
    summary.set_wrap(true);
    let (wins_field, wins) = note_field(
        "What went well?",
        "Record helpful routines, completed actions, or progress you want to remember.",
    );
    let (challenges_field, challenges) = note_field(
        "What was difficult?",
        "Note obstacles or friction without treating a missed action as a personal failure.",
    );
    let (lessons_field, lessons) = note_field(
        "What did I learn?",
        "Capture what worked, what did not, and one adjustment worth testing.",
    );
    let (focus_field, next_focus) = note_field(
        "What will I focus on next?",
        "Write a small, actionable focus for the next week or month.",
    );

    let notes_scroller = gtk::ScrolledWindow::new();
    notes_scroller.set_vexpand(true);
    let notes = gtk::Box::new(gtk::Orientation::Vertical, 12);
    notes.append(&wins_field);
    notes.append(&challenges_field);
    notes.append(&lessons_field);
    notes.append(&focus_field);
    notes_scroller.set_child(Some(&notes));

    let feedback = gtk::Label::new(None);
    feedback.set_xalign(0.0);
    feedback.set_wrap(true);
    feedback.add_css_class("dim-label");

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let save = gtk::Button::with_label("Save review");
    save.add_css_class("suggested-action");
    actions.append(&save);

    root.append(&heading);
    root.append(&description);
    root.append(&period_row);
    root.append(&summary);
    root.append(&notes_scroller);
    root.append(&feedback);
    root.append(&actions);
    window.set_child(Some(&root));

    let current_connection = Rc::clone(&connection);
    let current_kind = period_kind.clone();
    let current_start = period_start.clone();
    let current_wins = wins.clone();
    let current_challenges = challenges.clone();
    let current_lessons = lessons.clone();
    let current_next_focus = next_focus.clone();
    let current_summary = summary.clone();
    let current_feedback = feedback.clone();
    use_current.connect_clicked(move |_| {
        let kind = if current_kind.selected() == 1 { "month" } else { "week" };
        match current_period_start(&current_connection.borrow(), kind) {
            Ok(start) => {
                current_start.set_text(&start);
                refresh_editor(
                    &current_connection.borrow(),
                    kind,
                    &start,
                    &current_wins,
                    &current_challenges,
                    &current_lessons,
                    &current_next_focus,
                    &current_summary,
                    &current_feedback,
                );
            }
            Err(error) => current_feedback.set_text(&format!("Could not find current period: {error}")),
        }
    });

    let load_connection = Rc::clone(&connection);
    let load_kind = period_kind.clone();
    let load_start = period_start.clone();
    let load_wins = wins.clone();
    let load_challenges = challenges.clone();
    let load_lessons = lessons.clone();
    let load_next_focus = next_focus.clone();
    let load_summary = summary.clone();
    let load_feedback = feedback.clone();
    load.connect_clicked(move |_| {
        let kind = if load_kind.selected() == 1 { "month" } else { "week" };
        refresh_editor(
            &load_connection.borrow(),
            kind,
            &load_start.text(),
            &load_wins,
            &load_challenges,
            &load_lessons,
            &load_next_focus,
            &load_summary,
            &load_feedback,
        );
    });

    let save_connection = Rc::clone(&connection);
    let save_kind = period_kind.clone();
    let save_start = period_start.clone();
    let save_wins = wins.clone();
    let save_challenges = challenges.clone();
    let save_lessons = lessons.clone();
    let save_next_focus = next_focus.clone();
    let save_summary = summary.clone();
    let save_feedback = feedback.clone();
    save.connect_clicked(move |_| {
        let kind = if save_kind.selected() == 1 { "month" } else { "week" };
        match save_review(
            &save_connection.borrow(),
            kind,
            &save_start.text(),
            &read_text(&save_wins),
            &read_text(&save_challenges),
            &read_text(&save_lessons),
            &read_text(&save_next_focus),
        ) {
            Ok(_) => {
                save_feedback.set_text("Review saved locally.");
                match activity_summary(&save_connection.borrow(), kind, &save_start.text()) {
                    Ok(counts) => save_summary.set_text(&format!(
                        "Saved records only · {} completed · {} partial · {} skipped. This is not an adherence score.",
                        counts.completed, counts.partial, counts.skipped
                    )),
                    Err(error) => save_summary.set_text(&format!("Activity summary unavailable: {error}")),
                }
            }
            Err(error) => save_feedback.set_text(&format!("Could not save review: {error}")),
        }
    });

    match current_period_start(&connection.borrow(), "week") {
        Ok(start) => {
            period_start.set_text(&start);
            refresh_editor(
                &connection.borrow(),
                "week",
                &start,
                &wins,
                &challenges,
                &lessons,
                &next_focus,
                &summary,
                &feedback,
            );
        }
        Err(error) => feedback.set_text(&format!("Could not initialize review window: {error}")),
    }

    window.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection() -> Connection {
        Connection::open_in_memory().expect("open in-memory SQLite database")
    }

    #[test]
    fn review_schema_migration_is_idempotent_and_separate() {
        let db = connection();
        ensure_schema(&db).unwrap();
        ensure_schema(&db).unwrap();

        let review_version: i64 = db
            .query_row("SELECT MAX(version) FROM review_schema_migrations", [], |row| row.get(0))
            .unwrap();
        let core_schema_exists: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_migrations')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(review_version, 1);
        assert!(!core_schema_exists);
    }

    #[test]
    fn saves_updates_and_loads_one_review_per_period() {
        let db = connection();
        let first = save_review(&db, "week", "2026-11-16", "Walked", "Rain", "Earlier start helps", "Plan walks").unwrap();
        let second = save_review(&db, "week", "2026-11-16", "Walked twice", "", "", "Keep going").unwrap();
        assert_eq!(first, second);

        let stored = load_review(&db, "week", "2026-11-16").unwrap().unwrap();
        assert_eq!(stored.went_well, "Walked twice");
        assert_eq!(stored.next_focus, "Keep going");

        let count: i64 = db.query_row("SELECT COUNT(*) FROM period_reviews", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn requires_monday_for_week_and_first_day_for_month() {
        let db = connection();
        assert!(save_review(&db, "week", "2026-11-17", "", "", "", "").is_err());
        assert!(save_review(&db, "month", "2026-11-02", "", "", "", "").is_err());
        assert!(save_review(&db, "week", "2026-02-30", "", "", "", "").is_err());
        assert!(save_review(&db, "week", "2026-11-16", "", "", "", "").is_ok());
        assert!(save_review(&db, "month", "2026-11-01", "", "", "", "").is_ok());
    }

    #[test]
    fn rejects_oversized_private_notes_without_truncation() {
        let db = connection();
        let too_long = "x".repeat(MAX_NOTE_CHARS + 1);
        assert!(save_review(&db, "week", "2026-11-16", &too_long, "", "", "").is_err());
        let stored: i64 = db.query_row("SELECT COUNT(*) FROM period_reviews", [], |row| row.get(0)).unwrap();
        assert_eq!(stored, 0);
    }

    #[test]
    fn activity_summary_counts_saved_statuses_without_inventing_missed_days() {
        let db = connection();
        db.execute_batch(include_str!("../migrations/0001_initial.sql")).unwrap();
        db.execute_batch(include_str!("../migrations/0002_pause_periods.sql")).unwrap();
        let habit_id: i64 = db.query_row("INSERT INTO habits(name) VALUES ('Test') RETURNING id", [], |row| row.get(0)).unwrap();
        db.execute("INSERT INTO habit_entries(habit_id, local_date, status) VALUES (?1, '2026-11-16', 'completed')", [habit_id]).unwrap();
        db.execute("INSERT INTO habit_entries(habit_id, local_date, status) VALUES (?1, '2026-11-17', 'partial')", [habit_id]).unwrap();
        db.execute("INSERT INTO habit_entries(habit_id, local_date, status) VALUES (?1, '2026-11-18', 'skipped')", [habit_id]).unwrap();
        let summary = activity_summary(&db, "week", "2026-11-16").unwrap();
        assert_eq!(summary, ActivitySummary { completed: 1, partial: 1, skipped: 1 });
    }
}
