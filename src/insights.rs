//! Local activity history, calendar and transparent progress summaries.
//!
//! These metrics describe saved check-ins only. They intentionally do not infer
//! missed periods or schedule adherence; schedule changes are not yet effective-dated.

use crate::domain::CivilDate;
use gtk::prelude::*;
use gtk::{ApplicationWindow, Button, Grid, Label, Orientation, ProgressBar};
use rusqlite::{Connection, params};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyActivity {
    pub date: String,
    pub day_of_month: u8,
    pub weekday_monday_zero: u8,
    pub completed: i64,
    pub partial: i64,
    pub skipped: i64,
}

impl DailyActivity {
    pub fn total_entries(&self) -> i64 {
        self.completed + self.partial + self.skipped
    }

    /// Percentage of saved entries marked completed, not schedule adherence.
    pub fn recorded_completion_percent(&self) -> f64 {
        percentage(self.completed, self.total_entries())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HabitActivity {
    pub id: i64,
    pub name: String,
    pub completed: i64,
    pub partial: i64,
    pub skipped: i64,
}

impl HabitActivity {
    pub fn total_entries(&self) -> i64 {
        self.completed + self.partial + self.skipped
    }

    /// Percentage of this habit's saved entries marked completed.
    pub fn recorded_completion_percent(&self) -> f64 {
        percentage(self.completed, self.total_entries())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActivityReport {
    pub start_date: String,
    pub end_date: String,
    pub range_days: u16,
    pub days: Vec<DailyActivity>,
    pub habits: Vec<HabitActivity>,
    pub completed: i64,
    pub partial: i64,
    pub skipped: i64,
    pub days_with_records: usize,
    /// Completed check-ins by weekday, Monday first.
    pub weekday_completed: [i64; 7],
}

impl ActivityReport {
    pub fn total_entries(&self) -> i64 {
        self.completed + self.partial + self.skipped
    }

    pub fn recorded_completion_percent(&self) -> f64 {
        percentage(self.completed, self.total_entries())
    }
}

fn percentage(completed: i64, total: i64) -> f64 {
    if total == 0 {
        0.0
    } else {
        completed as f64 / total as f64 * 100.0
    }
}

fn parse_iso_date(value: &str) -> Option<CivilDate> {
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

fn invalid_input(message: &str) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(message.to_string())
}

/// Build an inclusive activity report for 1–366 calendar days ending on end_date.
/// Archived habits remain in the report because their historical entries are retained.
pub fn load_report(
    connection: &Connection,
    end_date: &str,
    range_days: u16,
) -> rusqlite::Result<ActivityReport> {
    if !(1..=366).contains(&range_days) {
        return Err(invalid_input("report range must be between 1 and 366 days"));
    }
    if parse_iso_date(end_date).is_none() {
        return Err(invalid_input("end date must be a real YYYY-MM-DD date"));
    }

    let offset = format!("-{} days", range_days - 1);
    let start_date: String =
        connection.query_row("SELECT date(?1, ?2)", params![end_date, offset], |row| {
            row.get(0)
        })?;
    if parse_iso_date(&start_date).is_none() {
        return Err(invalid_input("could not calculate report start date"));
    }

    let raw_days = {
        let mut statement = connection.prepare(
            "WITH RECURSIVE calendar(day) AS (
                SELECT ?1
                UNION ALL
                SELECT date(day, '+1 day') FROM calendar WHERE day < ?2
             ),
             grouped AS (
                SELECT local_date,
                    SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) AS completed,
                    SUM(CASE WHEN status = 'partial' THEN 1 ELSE 0 END) AS partial,
                    SUM(CASE WHEN status = 'skipped' THEN 1 ELSE 0 END) AS skipped
                FROM habit_entries
                WHERE local_date BETWEEN ?1 AND ?2
                GROUP BY local_date
             )
             SELECT calendar.day,
                    COALESCE(grouped.completed, 0),
                    COALESCE(grouped.partial, 0),
                    COALESCE(grouped.skipped, 0)
             FROM calendar
             LEFT JOIN grouped ON grouped.local_date = calendar.day
             ORDER BY calendar.day",
        )?;
        let rows = statement.query_map(params![start_date, end_date], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        let collected = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        collected
    };

    let mut days = Vec::with_capacity(usize::from(range_days));
    for (date_text, completed, partial, skipped) in raw_days {
        let date = parse_iso_date(&date_text)
            .ok_or_else(|| invalid_input("database returned an invalid calendar date"))?;
        days.push(DailyActivity {
            date: date_text,
            day_of_month: date.day(),
            weekday_monday_zero: date.weekday_monday_zero(),
            completed,
            partial,
            skipped,
        });
    }

    let habits = {
        let mut statement = connection.prepare(
            "SELECT h.id, h.name,
                    SUM(CASE WHEN e.status = 'completed' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN e.status = 'partial' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN e.status = 'skipped' THEN 1 ELSE 0 END)
             FROM habit_entries e
             JOIN habits h ON h.id = e.habit_id
             WHERE e.local_date BETWEEN ?1 AND ?2
             GROUP BY h.id, h.name
             ORDER BY COUNT(e.id) DESC, h.name COLLATE NOCASE",
        )?;
        let rows = statement.query_map(params![start_date, end_date], |row| {
            Ok(HabitActivity {
                id: row.get(0)?,
                name: row.get(1)?,
                completed: row.get(2)?,
                partial: row.get(3)?,
                skipped: row.get(4)?,
            })
        })?;
        let collected = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        collected
    };

    let mut weekday_completed = [0_i64; 7];
    {
        let mut statement = connection.prepare(
            "SELECT CAST(strftime('%w', local_date) AS INTEGER), COUNT(*)
             FROM habit_entries
             WHERE local_date BETWEEN ?1 AND ?2 AND status = 'completed'
             GROUP BY CAST(strftime('%w', local_date) AS INTEGER)",
        )?;
        let rows = statement.query_map(params![start_date, end_date], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (sqlite_weekday, count) = row?;
            // SQLite uses Sunday = 0; the UI and Rust domain use Monday = 0.
            let monday_index = ((sqlite_weekday + 6) % 7) as usize;
            weekday_completed[monday_index] = count;
        }
    }

    let completed = days.iter().map(|day| day.completed).sum();
    let partial = days.iter().map(|day| day.partial).sum();
    let skipped = days.iter().map(|day| day.skipped).sum();
    let days_with_records = days.iter().filter(|day| day.total_entries() > 0).count();

    Ok(ActivityReport {
        start_date,
        end_date: end_date.to_string(),
        range_days,
        days,
        habits,
        completed,
        partial,
        skipped,
        days_with_records,
        weekday_completed,
    })
}

pub fn open_window(parent: &ApplicationWindow, connection: Rc<RefCell<Connection>>) {
    let window = gtk::Window::builder()
        .title("Progress calendar & insights")
        .transient_for(parent)
        .default_width(900)
        .default_height(720)
        .build();

    let page = gtk::Box::new(Orientation::Vertical, 12);
    page.set_margin_top(22);
    page.set_margin_bottom(22);
    page.set_margin_start(24);
    page.set_margin_end(24);

    let title = Label::new(Some("Progress calendar & insights"));
    title.add_css_class("title-1");
    title.set_xalign(0.0);
    let subtitle = Label::new(Some(
        "Explore saved check-ins over the last 7, 30 or 90 days.",
    ));
    subtitle.add_css_class("dim-label");
    subtitle.set_xalign(0.0);

    let range_row = gtk::Box::new(Orientation::Horizontal, 8);
    let summary = Label::new(None);
    summary.set_xalign(0.0);
    summary.set_wrap(true);

    let calendar_title = Label::new(Some("Activity calendar"));
    calendar_title.add_css_class("title-3");
    calendar_title.set_xalign(0.0);
    let calendar = Grid::new();
    calendar.set_column_homogeneous(true);
    calendar.set_row_spacing(6);
    calendar.set_column_spacing(6);
    let selected_day = Label::new(Some("Select a day to inspect its saved check-ins."));
    selected_day.set_xalign(0.0);
    selected_day.set_wrap(true);

    let weekday_title = Label::new(Some("Completed check-ins by weekday"));
    weekday_title.add_css_class("title-3");
    weekday_title.set_xalign(0.0);
    let weekday_row = gtk::Box::new(Orientation::Horizontal, 8);

    let habits_title = Label::new(Some("Habit breakdown"));
    habits_title.add_css_class("title-3");
    habits_title.set_xalign(0.0);
    let habits_list = gtk::Box::new(Orientation::Vertical, 10);
    let habits_scroller = gtk::ScrolledWindow::new();
    habits_scroller.set_min_content_height(150);
    habits_scroller.set_child(Some(&habits_list));

    let explanation = Label::new(Some(
        "Important: percentages describe saved entries only. A blank day means no check-in was saved; it is not labelled as missed. Schedule-based adherence will be added after schedule history is versioned.",
    ));
    explanation.add_css_class("dim-label");
    explanation.set_wrap(true);
    explanation.set_xalign(0.0);

    page.append(&title);
    page.append(&subtitle);
    page.append(&range_row);
    page.append(&summary);
    page.append(&calendar_title);
    page.append(&calendar);
    page.append(&selected_day);
    page.append(&weekday_title);
    page.append(&weekday_row);
    page.append(&habits_title);
    page.append(&habits_scroller);
    page.append(&explanation);

    // The 90-day grid can be taller than a compact laptop display.
    let page_scroller = gtk::ScrolledWindow::new();
    page_scroller.set_child(Some(&page));
    window.set_child(Some(&page_scroller));

    for (label, range_days) in [("7 days", 7_u16), ("30 days", 30), ("90 days", 90)] {
        let button = Button::with_label(label);
        if range_days == 30 {
            button.add_css_class("suggested-action");
        }
        let view_connection = Rc::clone(&connection);
        let view_summary = summary.clone();
        let view_calendar = calendar.clone();
        let view_selected = selected_day.clone();
        let view_weekdays = weekday_row.clone();
        let view_habits = habits_list.clone();
        button.connect_clicked(move |_| {
            refresh_view(
                &view_connection,
                range_days,
                &view_summary,
                &view_calendar,
                &view_selected,
                &view_weekdays,
                &view_habits,
            );
        });
        range_row.append(&button);
    }

    refresh_view(
        &connection,
        30,
        &summary,
        &calendar,
        &selected_day,
        &weekday_row,
        &habits_list,
    );
    window.present();
}

fn refresh_view(
    connection: &Rc<RefCell<Connection>>,
    range_days: u16,
    summary: &Label,
    calendar: &Grid,
    selected_day: &Label,
    weekday_row: &gtk::Box,
    habits_list: &gtk::Box,
) {
    let database = connection.borrow();
    let end_date = match database.query_row("SELECT date('now', 'localtime')", [], |row| {
        row.get::<_, String>(0)
    }) {
        Ok(value) => value,
        Err(error) => {
            summary.set_text(&format!("Could not read the local date: {error}"));
            return;
        }
    };

    let report = match load_report(&database, &end_date, range_days) {
        Ok(report) => report,
        Err(error) => {
            summary.set_text(&format!("Could not load progress history: {error}"));
            return;
        }
    };

    summary.set_text(&format!(
        "{}–{} · {} completed · {} partial · {} skipped · {:.1}% of saved entries completed · {} of {} days with records",
        report.start_date,
        report.end_date,
        report.completed,
        report.partial,
        report.skipped,
        report.recorded_completion_percent(),
        report.days_with_records,
        report.range_days
    ));

    populate_calendar(&report, calendar, selected_day);
    populate_weekday_summary(&report, weekday_row);
    populate_habit_summary(&report, habits_list);
}

fn populate_calendar(report: &ActivityReport, calendar: &Grid, selected_day: &Label) {
    while let Some(child) = calendar.first_child() {
        calendar.remove(&child);
    }

    const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    for (column, weekday) in WEEKDAYS.iter().enumerate() {
        let label = Label::new(Some(weekday));
        label.add_css_class("dim-label");
        calendar.attach(&label, column as i32, 0, 1, 1);
    }

    let first_weekday = report
        .days
        .first()
        .map(|day| usize::from(day.weekday_monday_zero))
        .unwrap_or(0);

    for (index, activity) in report.days.iter().enumerate() {
        let position = first_weekday + index;
        let column = (position % 7) as i32;
        let row = (position / 7 + 1) as i32;
        let symbol = heat_symbol(activity);
        let button = Button::with_label(&format!("{}\n{:02}", symbol, activity.day_of_month));
        button.set_hexpand(true);
        button.set_tooltip_text(Some(&format!(
            "{} — {} completed, {} partial, {} skipped",
            activity.date, activity.completed, activity.partial, activity.skipped
        )));
        let detail = if activity.total_entries() == 0 {
            format!(
                "{}: no saved check-ins. This day is not classified as missed.",
                activity.date
            )
        } else {
            format!(
                "{}: {} completed, {} partial and {} skipped check-ins.",
                activity.date, activity.completed, activity.partial, activity.skipped
            )
        };
        let selected = selected_day.clone();
        button.connect_clicked(move |_| selected.set_text(&detail));
        calendar.attach(&button, column, row, 1, 1);
    }
}

fn heat_symbol(activity: &DailyActivity) -> &'static str {
    if activity.total_entries() == 0 {
        "·"
    } else if activity.completed == 0 {
        "░"
    } else {
        let percent = activity.recorded_completion_percent();
        if percent < 34.0 {
            "▂"
        } else if percent < 67.0 {
            "▄"
        } else if percent < 100.0 {
            "▆"
        } else {
            "█"
        }
    }
}

fn populate_weekday_summary(report: &ActivityReport, weekday_row: &gtk::Box) {
    while let Some(child) = weekday_row.first_child() {
        weekday_row.remove(&child);
    }
    const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    for (index, weekday) in WEEKDAYS.iter().enumerate() {
        let value = report.weekday_completed[index];
        let label = Label::new(Some(&format!("{weekday}\n{value}")));
        label.set_hexpand(true);
        label.set_tooltip_text(Some(&format!("{value} completed check-ins on {weekday}")));
        weekday_row.append(&label);
    }
}

fn populate_habit_summary(report: &ActivityReport, habits_list: &gtk::Box) {
    while let Some(child) = habits_list.first_child() {
        habits_list.remove(&child);
    }

    if report.habits.is_empty() {
        let empty = Label::new(Some("No saved check-ins in this period."));
        empty.add_css_class("dim-label");
        empty.set_xalign(0.0);
        habits_list.append(&empty);
        return;
    }

    for habit in &report.habits {
        let row = gtk::Box::new(Orientation::Vertical, 4);
        let label = Label::new(Some(&format!(
            "{} · {} completed / {} saved entries ({} partial, {} skipped)",
            habit.name,
            habit.completed,
            habit.total_entries(),
            habit.partial,
            habit.skipped
        )));
        label.set_xalign(0.0);
        label.set_wrap(true);
        let progress = ProgressBar::new();
        progress.set_show_text(true);
        progress.set_fraction((habit.recorded_completion_percent() / 100.0).clamp(0.0, 1.0));
        progress.set_text(Some(&format!(
            "{:.1}% recorded completion",
            habit.recorded_completion_percent()
        )));
        row.append(&label);
        row.append(&progress);
        habits_list.append(&row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE habits (
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    archived_at TEXT
                 );
                 CREATE TABLE habit_entries (
                    id INTEGER PRIMARY KEY,
                    habit_id INTEGER NOT NULL REFERENCES habits(id),
                    local_date TEXT NOT NULL,
                    status TEXT NOT NULL CHECK(status IN ('completed','partial','skipped'))
                 );",
            )
            .expect("create test schema");
        connection
    }

    fn add_habit(connection: &Connection, id: i64, name: &str, archived: bool) {
        connection
            .execute(
                "INSERT INTO habits(id, name, archived_at) VALUES (?1, ?2, ?3)",
                params![id, name, archived.then_some("2026-01-01")],
            )
            .expect("insert habit");
    }

    fn add_entry(connection: &Connection, habit_id: i64, date: &str, status: &str) {
        connection
            .execute(
                "INSERT INTO habit_entries(habit_id, local_date, status) VALUES (?1, ?2, ?3)",
                params![habit_id, date, status],
            )
            .expect("insert check-in");
    }

    #[test]
    fn fills_empty_calendar_days_and_separates_entry_states() {
        let connection = memory_connection();
        add_habit(&connection, 1, "Read", false);
        add_entry(&connection, 1, "2026-03-02", "completed");
        add_entry(&connection, 1, "2026-03-03", "partial");
        add_entry(&connection, 1, "2026-03-03", "skipped");

        let report = load_report(&connection, "2026-03-03", 3).expect("load report");
        assert_eq!(report.start_date, "2026-03-01");
        assert_eq!(report.days.len(), 3);
        assert_eq!(report.days[0].total_entries(), 0);
        assert_eq!(report.days[1].completed, 1);
        assert_eq!(report.days[2].partial, 1);
        assert_eq!(report.days[2].skipped, 1);
        assert_eq!(report.completed, 1);
        assert_eq!(report.partial, 1);
        assert_eq!(report.skipped, 1);
        assert_eq!(report.days_with_records, 2);
    }

    #[test]
    fn weekday_counts_are_monday_first() {
        let connection = memory_connection();
        add_habit(&connection, 1, "Walk", false);
        add_entry(&connection, 1, "2026-03-01", "completed"); // Sunday
        add_entry(&connection, 1, "2026-03-02", "completed"); // Monday
        add_entry(&connection, 1, "2026-03-03", "partial");

        let report = load_report(&connection, "2026-03-03", 3).expect("load report");
        assert_eq!(report.weekday_completed, [1, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn archived_habits_keep_their_history_in_analytics() {
        let connection = memory_connection();
        add_habit(&connection, 1, "Archived reading", true);
        add_entry(&connection, 1, "2026-03-03", "completed");

        let report = load_report(&connection, "2026-03-03", 1).expect("load report");
        assert_eq!(report.habits.len(), 1);
        assert_eq!(report.habits[0].name, "Archived reading");
    }

    #[test]
    fn rejects_invalid_dates_and_unsupported_ranges() {
        let connection = memory_connection();
        assert!(load_report(&connection, "2026-02-29", 7).is_err());
        assert!(load_report(&connection, "2028-02-29", 7).is_ok());
        assert!(load_report(&connection, "2026-03-03", 0).is_err());
        assert!(load_report(&connection, "2026-03-03", 367).is_err());
    }

    #[test]
    fn empty_period_has_zero_safe_percentages() {
        let connection = memory_connection();
        let report = load_report(&connection, "2026-03-03", 7).expect("load report");
        assert_eq!(report.total_entries(), 0);
        assert_eq!(report.recorded_completion_percent(), 0.0);
        assert_eq!(report.days_with_records, 0);
        assert!(report.habits.is_empty());
        assert_eq!(report.days.len(), 7);
    }
}
