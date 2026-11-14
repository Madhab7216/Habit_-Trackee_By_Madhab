//! Deterministic scoring replay over local, persisted habit history.
//!
//! Scores are computed from source entries on demand. No materialized score cache
//! can become stale when entries, pause periods or the active schedule change.
use crate::domain::{
    adherence, achievement_ratio, current_streak, update_strength, Cadence, CivilDate,
    Direction, HabitSchedule, PeriodResult,
};
use rusqlite::{params, Connection, Result};
use std::collections::BTreeMap;

pub const SCORING_VERSION: u8 = 1;
pub const EMA_ALPHA: f64 = 0.25;

#[derive(Debug, Clone, PartialEq)]
pub struct HabitScoreSummary {
    pub habit_id: i64,
    pub habit_name: String,
    pub scoring_version: u8,
    /// Ratio in 0.0..=1.0. None means no eligible scored periods exist.
    pub adherence: Option<f64>,
    /// EMA ratio in 0.0..=1.0. None means no eligible scored periods exist.
    pub habit_strength: Option<f64>,
    pub current_streak: u32,
    pub closed_periods: u32,
    pub scored_periods: u32,
    pub missed_periods: u32,
    pub skipped_periods: u32,
    pub paused_periods: u32,
}

#[derive(Debug, Clone)]
struct HabitConfig {
    id: i64,
    name: String,
    tracking_type: String,
    direction: Direction,
    target_value: Option<f64>,
    schedule: HabitSchedule,
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    status: EntryStatus,
    value: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryStatus {
    Completed,
    Partial,
    Skipped,
}

#[derive(Debug, Clone, Copy)]
struct PauseRange {
    starts_on: CivilDate,
    ends_on: Option<CivilDate>,
}

impl PauseRange {
    fn contains(self, date: CivilDate) -> bool {
        date >= self.starts_on && self.ends_on.is_none_or(|end| date < end)
    }
}

#[derive(Debug, Clone, Copy)]
struct ScheduledDay {
    paused: bool,
    entry: Option<Entry>,
}

#[derive(Debug, Clone, Copy)]
struct PeriodEvaluation {
    result: PeriodResult,
    missed: bool,
}

#[derive(Debug)]
struct PeriodBucket {
    period_start: i64,
    span_days: u8,
    days: Vec<ScheduledDay>,
}

/// Compute all active-habit scores as of the computer's current local date.
/// The current day/week is deliberately not scored as missed.
pub fn summaries_for_today(connection: &Connection) -> Result<Vec<HabitScoreSummary>> {
    let value: String =
        connection.query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))?;
    let as_of = parse_date(&value).ok_or_else(|| scoring_data_error("current local date"))?;
    summaries_for_date(connection, as_of)
}

/// Rebuild scores from persisted history at an explicit exclusive date boundary.
/// This API makes historical replay deterministic and testable without changing data.
pub fn summaries_for_date(
    connection: &Connection,
    as_of: CivilDate,
) -> Result<Vec<HabitScoreSummary>> {
    let configs = load_active_habits(connection)?;
    configs
        .into_iter()
        .map(|config| summarize_habit(connection, config, as_of))
        .collect()
}

fn load_active_habits(connection: &Connection) -> Result<Vec<HabitConfig>> {
    let mut statement = connection.prepare(
        "SELECT id, name, tracking_type, direction, target_value, schedule_type,
                weekday_mask, interval_days, target_per_period, start_date, end_date
         FROM habits
         WHERE archived_at IS NULL
         ORDER BY sort_order, id",
    )?;

    let rows = statement.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let name: String = row.get(1)?;
        let tracking_type: String = row.get(2)?;
        let direction_value: String = row.get(3)?;
        let direction = match direction_value.as_str() {
            "at_least" => Direction::AtLeast,
            "at_most" => Direction::AtMost,
            _ => return Err(scoring_data_error("direction")),
        };
        let target_value: Option<f64> = row.get(4)?;
        let cadence = cadence_from_row(row, 5)?;
        let start = date_from_row(row, 9, "start_date")?;
        let end = row
            .get::<_, Option<String>>(10)?
            .map(|value| parse_date(&value).ok_or_else(|| scoring_data_error("end_date")))
            .transpose()?;
        let schedule = HabitSchedule::new(cadence, start, end)
            .map_err(|_| scoring_data_error("schedule definition"))?;

        if cadence_is_weekly(cadence) && tracking_type != "frequency" {
            return Err(scoring_data_error("times_per_week requires frequency tracking"));
        }
        if !cadence_is_weekly(cadence)
            && tracking_type != "binary"
            && target_value.is_none_or(|target| !target.is_finite() || target <= 0.0)
        {
            return Err(scoring_data_error("measured habit requires a finite positive target"));
        }

        Ok(HabitConfig {
            id,
            name,
            tracking_type,
            direction,
            target_value,
            schedule,
        })
    })?;

    rows.collect()
}

fn cadence_from_row(row: &rusqlite::Row<'_>, offset: usize) -> Result<Cadence> {
    let schedule_type: String = row.get(offset)?;
    match schedule_type.as_str() {
        "daily" => Ok(Cadence::Daily),
        "weekdays" => {
            let raw: Option<i64> = row.get(offset + 1)?;
            let mask = raw
                .and_then(|value| u8::try_from(value).ok())
                .ok_or_else(|| scoring_data_error("weekday_mask"))?;
            Ok(Cadence::Weekdays { weekday_mask: mask })
        }
        "every_n_days" => {
            let raw: Option<i64> = row.get(offset + 2)?;
            let interval = raw
                .and_then(|value| u16::try_from(value).ok())
                .ok_or_else(|| scoring_data_error("interval_days"))?;
            Ok(Cadence::EveryNDays { interval })
        }
        "times_per_week" => {
            let raw: Option<i64> = row.get(offset + 3)?;
            let target = raw
                .and_then(|value| u8::try_from(value).ok())
                .ok_or_else(|| scoring_data_error("target_per_period"))?;
            Ok(Cadence::TimesPerWeek { target })
        }
        _ => Err(scoring_data_error("schedule_type")),
    }
}

fn date_from_row(row: &rusqlite::Row<'_>, index: usize, field: &str) -> Result<CivilDate> {
    let value: String = row.get(index)?;
    parse_date(&value).ok_or_else(|| scoring_data_error(field))
}

fn parse_date(value: &str) -> Option<CivilDate> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u8>().ok()?;
    let day = parts.next()?.parse::<u8>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    CivilDate::new(year, month, day)
}

fn cadence_is_weekly(cadence: Cadence) -> bool {
    matches!(cadence, Cadence::TimesPerWeek { .. })
}

fn scoring_data_error(field: &str) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(format!("invalid persisted scoring data: {field}"))
}

fn summarize_habit(
    connection: &Connection,
    config: HabitConfig,
    as_of: CivilDate,
) -> Result<HabitScoreSummary> {
    let entries = load_entries(connection, config.id)?;
    let pauses = load_pause_ranges(connection, config.id)?;
    let buckets = build_period_buckets(config.schedule, &entries, &pauses, as_of)?;

    let mut evaluations = Vec::new();
    for bucket in buckets.into_values() {
        if bucket.days.is_empty() || !period_is_closed(config.schedule, &bucket, as_of) {
            continue;
        }
        evaluations.push(evaluate_period(&config, &bucket)?);
    }

    let results: Vec<PeriodResult> = evaluations.iter().map(|item| item.result).collect();
    let adherence_value = adherence(&results)
        .map_err(|error| scoring_data_error(&format!("adherence: {error}")))?;

    let mut strength = None;
    for result in &results {
        strength = update_strength(strength, *result, EMA_ALPHA)
            .map_err(|error| scoring_data_error(&format!("strength: {error}")))?;
    }

    let streak = current_streak(&results)
        .map_err(|error| scoring_data_error(&format!("streak: {error}")))?;

    let scored_periods = results
        .iter()
        .filter(|result| matches!(result, PeriodResult::Achievement(_)))
        .count() as u32;
    let skipped_periods = results
        .iter()
        .filter(|result| matches!(result, PeriodResult::Skipped))
        .count() as u32;
    let paused_periods = results
        .iter()
        .filter(|result| matches!(result, PeriodResult::Paused))
        .count() as u32;
    let missed_periods = evaluations.iter().filter(|item| item.missed).count() as u32;

    Ok(HabitScoreSummary {
        habit_id: config.id,
        habit_name: config.name,
        scoring_version: SCORING_VERSION,
        adherence: adherence_value,
        habit_strength: strength,
        current_streak: streak,
        closed_periods: evaluations.len() as u32,
        scored_periods,
        missed_periods,
        skipped_periods,
        paused_periods,
    })
}

fn load_entries(connection: &Connection, habit_id: i64) -> Result<BTreeMap<i64, Entry>> {
    let mut statement = connection.prepare(
        "SELECT local_date, status, value
         FROM habit_entries
         WHERE habit_id = ?1
         ORDER BY local_date",
    )?;
    let rows = statement.query_map([habit_id], |row| {
        let value: String = row.get(0)?;
        let date = parse_date(&value).ok_or_else(|| scoring_data_error("entry date"))?;
        let status_value: String = row.get(1)?;
        let status = match status_value.as_str() {
            "completed" => EntryStatus::Completed,
            "partial" => EntryStatus::Partial,
            "skipped" => EntryStatus::Skipped,
            _ => return Err(scoring_data_error("entry status")),
        };
        Ok((
            date.ordinal(),
            Entry {
                status,
                value: row.get(2)?,
            },
        ))
    })?;
    let mut entries = BTreeMap::new();
    for row in rows {
        let (ordinal, entry) = row?;
        entries.insert(ordinal, entry);
    }
    Ok(entries)
}

fn load_pause_ranges(connection: &Connection, habit_id: i64) -> Result<Vec<PauseRange>> {
    let mut statement = connection.prepare(
        "SELECT starts_on, ends_on
         FROM habit_pause_periods
         WHERE habit_id = ?1
         ORDER BY starts_on, id",
    )?;
    let rows = statement.query_map([habit_id], |row| {
        let start: String = row.get(0)?;
        let end: Option<String> = row.get(1)?;
        Ok(PauseRange {
            starts_on: parse_date(&start).ok_or_else(|| scoring_data_error("pause start"))?,
            ends_on: end
                .map(|value| parse_date(&value).ok_or_else(|| scoring_data_error("pause end")))
                .transpose()?,
        })
    })?;
    rows.collect()
}

fn build_period_buckets(
    schedule: HabitSchedule,
    entries: &BTreeMap<i64, Entry>,
    pauses: &[PauseRange],
    as_of: CivilDate,
) -> Result<BTreeMap<i64, PeriodBucket>> {
    let start = schedule.start().ordinal();
    let stop = as_of.ordinal().min(
        schedule
            .end()
            .map(|date| date.ordinal() + 1)
            .unwrap_or(as_of.ordinal()),
    );
    let mut buckets: BTreeMap<i64, PeriodBucket> = BTreeMap::new();

    for ordinal in start..stop {
        let date = CivilDate::from_ordinal(ordinal)
            .ok_or_else(|| scoring_data_error("date range"))?;
        if let Some(period) = schedule.period_for(date) {
            let day = ScheduledDay {
                paused: pauses.iter().any(|pause| pause.contains(date)),
                entry: entries.get(&ordinal).copied(),
            };
            buckets
                .entry(period.start_day)
                .or_insert_with(|| PeriodBucket {
                    period_start: period.start_day,
                    span_days: period.span_days,
                    days: Vec::new(),
                })
                .days
                .push(day);
        }
    }

    Ok(buckets)
}


fn period_is_closed(schedule: HabitSchedule, bucket: &PeriodBucket, as_of: CivilDate) -> bool {
    let natural_end = bucket.period_start + i64::from(bucket.span_days) - 1;
    let scheduled_end = schedule
        .end()
        .map(|end| end.ordinal().min(natural_end))
        .unwrap_or(natural_end);
    scheduled_end < as_of.ordinal()
}

fn evaluate_period(config: &HabitConfig, bucket: &PeriodBucket) -> Result<PeriodEvaluation> {
    if bucket.span_days == 7 {
        return evaluate_weekly_period(config, bucket);
    }
    evaluate_daily_period(config, bucket)
}

fn evaluate_daily_period(
    config: &HabitConfig,
    bucket: &PeriodBucket,
) -> Result<PeriodEvaluation> {
    let day = bucket.days.first().ok_or_else(|| scoring_data_error("empty daily period"))?;
    if day.paused {
        return Ok(PeriodEvaluation {
            result: PeriodResult::Paused,
            missed: false,
        });
    }

    match day.entry {
        None => Ok(PeriodEvaluation {
            result: PeriodResult::Achievement(0.0),
            missed: true,
        }),
        Some(Entry {
            status: EntryStatus::Skipped,
            ..
        }) => Ok(PeriodEvaluation {
            result: PeriodResult::Skipped,
            missed: false,
        }),
        Some(entry) => {
            let ratio = entry_ratio(config, entry)?;
            Ok(PeriodEvaluation {
                result: PeriodResult::Achievement(ratio),
                missed: false,
            })
        }
    }
}

fn evaluate_weekly_period(
    config: &HabitConfig,
    bucket: &PeriodBucket,
) -> Result<PeriodEvaluation> {
    let target = match config.schedule.cadence() {
        Cadence::TimesPerWeek { target } => f64::from(target),
        _ => return Err(scoring_data_error("weekly cadence")),
    };

    let active_days = bucket.days.iter().filter(|day| !day.paused).count();
    if active_days == 0 {
        return Ok(PeriodEvaluation {
            result: PeriodResult::Paused,
            missed: false,
        });
    }

    let all_days_skipped = bucket.days.iter().filter(|day| !day.paused).all(|day| {
        matches!(day.entry, Some(Entry { status: EntryStatus::Skipped, .. }))
    });
    if all_days_skipped {
        return Ok(PeriodEvaluation {
            result: PeriodResult::Skipped,
            missed: false,
        });
    }

    let mut actual_sessions = 0.0;
    let mut has_progress_entry = false;
    for day in bucket.days.iter().filter(|day| !day.paused) {
        match day.entry {
            Some(Entry {
                status: EntryStatus::Completed,
                value,
            }) => {
                actual_sessions += value.unwrap_or(1.0);
                has_progress_entry = true;
            }
            Some(Entry {
                status: EntryStatus::Partial,
                value: Some(value),
            }) => {
                actual_sessions += value;
                has_progress_entry = true;
            }
            Some(Entry {
                status: EntryStatus::Partial,
                value: None,
            }) => return Err(scoring_data_error("weekly partial entry value")),
            Some(Entry {
                status: EntryStatus::Skipped,
                ..
            })
            | None => {}
        }
    }

    let effective_target = target * active_days as f64 / f64::from(bucket.span_days);
    let ratio = achievement_ratio(actual_sessions, effective_target, Direction::AtLeast)
        .map_err(|error| scoring_data_error(&format!("weekly ratio: {error}")))?;

    Ok(PeriodEvaluation {
        result: PeriodResult::Achievement(ratio),
        missed: !has_progress_entry,
    })
}

fn entry_ratio(config: &HabitConfig, entry: Entry) -> Result<f64> {
    if config.tracking_type == "binary" {
        return match entry.status {
            EntryStatus::Completed => Ok(1.0),
            EntryStatus::Partial => Err(scoring_data_error("binary partial entry")),
            EntryStatus::Skipped => Err(scoring_data_error("skipped entry should not be scored")),
        };
    }

    if config.tracking_type == "frequency" {
        return Err(scoring_data_error("frequency tracking needs a weekly schedule"));
    }
    let actual = entry
        .value
        .ok_or_else(|| scoring_data_error("measured entry requires a value"))?;
    let target = config
        .target_value
        .ok_or_else(|| scoring_data_error("measured habit target"))?;

    achievement_ratio(actual, target, config.direction)
        .map_err(|error| scoring_data_error(&format!("achievement ratio: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u8, day: u8) -> CivilDate {
        CivilDate::new(year, month, day).expect("valid test date")
    }

    fn test_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("open test database");
        connection
            .execute_batch(include_str!("../migrations/0001_initial.sql"))
            .expect("apply initial migration");
        connection
            .execute_batch(include_str!("../migrations/0002_pause_periods.sql"))
            .expect("apply pause migration");
        connection
    }

    fn add_habit(
        connection: &Connection,
        name: &str,
        tracking_type: &str,
        direction: &str,
        target: Option<f64>,
        schedule: &str,
        start: &str,
        target_per_period: Option<i64>,
    ) -> i64 {
        connection.execute(
            "INSERT INTO habits
                (name, tracking_type, direction, target_value, schedule_type,
                 target_per_period, start_date)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![name, tracking_type, direction, target, schedule, target_per_period, start],
        ).expect("insert test habit");
        connection.last_insert_rowid()
    }

    fn add_entry(
        connection: &Connection,
        habit_id: i64,
        date: &str,
        status: &str,
        value: Option<f64>,
    ) {
        connection.execute(
            "INSERT INTO habit_entries(habit_id, local_date, status, value)
             VALUES (?1, ?2, ?3, ?4)",
            params![habit_id, date, status, value],
        ).expect("insert test entry");
    }

    #[test]
    fn daily_binary_replay_counts_missed_and_excludes_open_today() {
        let connection = test_connection();
        let id = add_habit(&connection, "Read", "binary", "at_least", None, "daily", "2026-11-01", None);
        add_entry(&connection, id, "2026-11-01", "completed", Some(1.0));
        add_entry(&connection, id, "2026-11-02", "skipped", None);
        add_entry(&connection, id, "2026-11-04", "completed", Some(1.0));
        add_entry(&connection, id, "2026-11-14", "completed", Some(1.0));

        let result = summarize_habit(
            &connection,
            load_active_habits(&connection).unwrap().remove(0),
            date(2026, 11, 15),
        ).unwrap();

        assert_eq!(result.scoring_version, 1);
        assert_eq!(result.closed_periods, 14);
        assert_eq!(result.missed_periods, 10);
        assert_eq!(result.skipped_periods, 1);
        assert_eq!(result.adherence, Some(3.0 / 13.0));
        assert_eq!(result.current_streak, 1);
    }

    #[test]
    fn at_most_measurement_scores_higher_consumption_lower() {
        let connection = test_connection();
        let id = add_habit(&connection, "Screen time", "limit", "at_most", Some(120.0), "daily", "2026-11-01", None);
        add_entry(&connection, id, "2026-11-01", "completed", Some(90.0));
        add_entry(&connection, id, "2026-11-02", "partial", Some(180.0));

        let config = load_active_habits(&connection).unwrap().remove(0);
        let result = summarize_habit(&connection, config, date(2026, 11, 4)).unwrap();

        assert!((result.adherence.unwrap() - (1.0 + 2.0 / 3.0 + 0.0) / 3.0).abs() < 1e-12);
        assert_eq!(result.current_streak, 0);
    }

    #[test]
    fn skips_and_full_pause_periods_are_excluded_from_adherence() {
        let connection = test_connection();
        let id = add_habit(&connection, "Exercise", "binary", "at_least", None, "daily", "2026-11-01", None);
        add_entry(&connection, id, "2026-11-01", "completed", Some(1.0));
        add_entry(&connection, id, "2026-11-02", "skipped", None);
        connection.execute(
            "INSERT INTO habit_pause_periods(habit_id, starts_on, ends_on)
             VALUES (?1, '2026-11-03', '2026-11-05')",
            [id],
        ).unwrap();

        let result = summarize_habit(
            &connection,
            load_active_habits(&connection).unwrap().remove(0),
            date(2026, 11, 6),
        ).unwrap();

        assert_eq!(result.closed_periods, 5);
        assert_eq!(result.scored_periods, 2);
        assert_eq!(result.skipped_periods, 1);
        assert_eq!(result.paused_periods, 2);
        assert_eq!(result.adherence, Some(0.5));
    }

    #[test]
    fn weekly_frequency_accumulates_sessions_and_prorates_paused_days() {
        let connection = test_connection();
        let id = add_habit(&connection, "Workouts", "frequency", "at_least", None, "times_per_week", "2026-11-02", Some(3));
        add_entry(&connection, id, "2026-11-02", "completed", Some(1.0));
        add_entry(&connection, id, "2026-11-04", "completed", Some(1.0));
        connection.execute(
            "INSERT INTO habit_pause_periods(habit_id, starts_on, ends_on)
             VALUES (?1, '2026-11-03', '2026-11-04')",
            [id],
        ).unwrap();

        let result = summarize_habit(
            &connection,
            load_active_habits(&connection).unwrap().remove(0),
            date(2026, 11, 10),
        ).unwrap();

        // Six active days out of seven means a target of 3 * 6/7.
        assert!((result.adherence.unwrap() - (2.0 / (3.0 * 6.0 / 7.0))).abs() < 1e-12);
    }

    #[test]
    fn mixed_weekly_skip_and_no_sessions_is_still_missed() {
        let connection = test_connection();
        let id = add_habit(
            &connection,
            "Training",
            "frequency",
            "at_least",
            None,
            "times_per_week",
            "2026-11-02",
            Some(3),
        );
        add_entry(&connection, id, "2026-11-02", "skipped", None);

        let result = summarize_habit(
            &connection,
            load_active_habits(&connection).unwrap().remove(0),
            date(2026, 11, 10),
        )
        .unwrap();

        assert_eq!(result.closed_periods, 1);
        assert_eq!(result.missed_periods, 1);
        assert_eq!(result.skipped_periods, 0);
        assert_eq!(result.adherence, Some(0.0));
    }

    #[test]
    fn weekly_current_period_is_not_missed_before_it_closes() {
        let connection = test_connection();
        let id = add_habit(&connection, "Weekly writing", "frequency", "at_least", None, "times_per_week", "2026-11-02", Some(3));
        let result = summarize_habit(
            &connection,
            load_active_habits(&connection).unwrap().remove(0),
            date(2026, 11, 6),
        ).unwrap();

        assert_eq!(result.closed_periods, 0);
        assert_eq!(result.missed_periods, 0);
        assert_eq!(result.adherence, None);
    }

    #[test]
    fn no_history_returns_empty_metrics_not_zero_percent() {
        let connection = test_connection();
        add_habit(&connection, "New habit", "binary", "at_least", None, "daily", "2026-11-14", None);
        let summaries = summaries_for_date(&connection, date(2026, 11, 14)).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].adherence, None);
        assert_eq!(summaries[0].habit_strength, None);
        assert_eq!(summaries[0].current_streak, 0);
    }
}
