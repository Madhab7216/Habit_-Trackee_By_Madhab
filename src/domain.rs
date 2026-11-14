//! Pure, deterministic habit scheduling and scoring rules.
//! Keep these rules independent from GTK and persistence so they remain easy to test.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CivilDate {
    year: i32,
    month: u8,
    day: u8,
}

impl CivilDate {
    pub fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        if !(1..=9_999).contains(&year) || !(1..=12).contains(&month) {
            return None;
        }
        let max_day = days_in_month(year, month);
        if day == 0 || day > max_day {
            return None;
        }
        Some(Self { year, month, day })
    }

    pub fn year(self) -> i32 {
        self.year
    }

    pub fn month(self) -> u8 {
        self.month
    }

    pub fn day(self) -> u8 {
        self.day
    }

    /// Number of civil days relative to 1970-01-01.
    pub fn ordinal(self) -> i64 {
        let mut year = self.year;
        let month = i32::from(self.month);
        let day = i32::from(self.day);
        year -= if month <= 2 { 1 } else { 0 };
        let era = year.div_euclid(400);
        let year_of_era = year - era * 400;
        let adjusted_month = month + if month > 2 { -3 } else { 9 };
        let day_of_year = (153 * adjusted_month + 2) / 5 + day - 1;
        let day_of_era =
            year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        i64::from(era * 146_097 + day_of_era - 719_468)
    }

    /// ISO weekday index: Monday = 0, Sunday = 6.
    pub fn weekday_monday_zero(self) -> u8 {
        (self.ordinal() + 3).rem_euclid(7) as u8
    }
}

impl fmt::Display for CivilDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cadence {
    Daily,
    /// Bit 0 = Monday through bit 6 = Sunday.
    Weekdays { weekday_mask: u8 },
    EveryNDays { interval: u16 },
    /// The user may complete the habit on any day, but is evaluated once per ISO week.
    TimesPerWeek { target: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HabitSchedule {
    cadence: Cadence,
    start: CivilDate,
    end: Option<CivilDate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeriodKey {
    /// Ordinal of the first calendar day in this evaluation period.
    pub start_day: i64,
    pub span_days: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleError {
    InvalidWeekdayMask,
    InvalidInterval,
    InvalidWeeklyTarget,
    EndBeforeStart,
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ScheduleError {}

impl HabitSchedule {
    pub fn new(
        cadence: Cadence,
        start: CivilDate,
        end: Option<CivilDate>,
    ) -> Result<Self, ScheduleError> {
        if end.is_some_and(|end_date| end_date < start) {
            return Err(ScheduleError::EndBeforeStart);
        }

        match cadence {
            Cadence::Weekdays { weekday_mask }
                if weekday_mask == 0 || weekday_mask & !0b0111_1111 != 0 =>
            {
                return Err(ScheduleError::InvalidWeekdayMask);
            }
            Cadence::EveryNDays { interval: 0 } => {
                return Err(ScheduleError::InvalidInterval);
            }
            Cadence::TimesPerWeek { target } if !(1..=7).contains(&target) => {
                return Err(ScheduleError::InvalidWeeklyTarget);
            }
            _ => {}
        }

        Ok(Self { cadence, start, end })
    }

    pub fn cadence(self) -> Cadence {
        self.cadence
    }

    pub fn start(self) -> CivilDate {
        self.start
    }

    pub fn end(self) -> Option<CivilDate> {
        self.end
    }

    /// Returns the evaluation period for a date, or None when the date is not scheduled.
    pub fn period_for(self, date: CivilDate) -> Option<PeriodKey> {
        if date < self.start || self.end.is_some_and(|end_date| date > end_date) {
            return None;
        }

        match self.cadence {
            Cadence::Daily => Some(PeriodKey {
                start_day: date.ordinal(),
                span_days: 1,
            }),
            Cadence::Weekdays { weekday_mask } => {
                let bit = 1_u8 << date.weekday_monday_zero();
                (weekday_mask & bit != 0).then_some(PeriodKey {
                    start_day: date.ordinal(),
                    span_days: 1,
                })
            }
            Cadence::EveryNDays { interval } => {
                let offset = date.ordinal() - self.start.ordinal();
                (offset % i64::from(interval) == 0).then_some(PeriodKey {
                    start_day: date.ordinal(),
                    span_days: 1,
                })
            }
            Cadence::TimesPerWeek { .. } => {
                let monday = date.ordinal() - i64::from(date.weekday_monday_zero());
                Some(PeriodKey {
                    start_day: monday,
                    span_days: 7,
                })
            }
        }
    }

    pub fn target_per_period(self) -> u8 {
        match self.cadence {
            Cadence::TimesPerWeek { target } => target,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    AtLeast,
    AtMost,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PeriodResult {
    /// Achievement ratio, normalized to the inclusive range 0.0..=1.0.
    Achievement(f64),
    /// An intentional skip does not count as failure or contribute to the average.
    Skipped,
    /// A paused period is excluded from scoring and streak calculations.
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreError {
    InvalidTarget,
    InvalidActual,
    InvalidRatio,
    InvalidAlpha,
}

impl fmt::Display for ScoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ScoreError {}

/// Normalize a measured value against a target; higher-is-better and cap/limit habits
/// use different directional rules. The result is capped at 1.0.
pub fn achievement_ratio(
    actual: f64,
    target: f64,
    direction: Direction,
) -> Result<f64, ScoreError> {
    if !target.is_finite() || target <= 0.0 {
        return Err(ScoreError::InvalidTarget);
    }
    if !actual.is_finite() || actual < 0.0 {
        return Err(ScoreError::InvalidActual);
    }

    let ratio = match direction {
        Direction::AtLeast => actual / target,
        Direction::AtMost if actual <= target => 1.0,
        Direction::AtMost => target / actual,
    };
    Ok(ratio.clamp(0.0, 1.0))
}

/// Mean achievement across scored, closed periods. Skipped and paused periods are omitted.
pub fn adherence(results: &[PeriodResult]) -> Result<Option<f64>, ScoreError> {
    let mut total = 0.0;
    let mut count = 0_u32;
    for result in results {
        if let PeriodResult::Achievement(value) = result {
            validate_ratio(*value)?;
            total += *value;
            count += 1;
        }
    }
    Ok((count > 0).then_some(total / f64::from(count)))
}

/// Update the explainable habit-strength estimate using exponential smoothing.
/// The first eligible period initializes the estimate. Skips/pauses leave it unchanged.
pub fn update_strength(
    previous: Option<f64>,
    result: PeriodResult,
    alpha: f64,
) -> Result<Option<f64>, ScoreError> {
    if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
        return Err(ScoreError::InvalidAlpha);
    }
    if let Some(previous_value) = previous {
        validate_ratio(previous_value)?;
    }

    match result {
        PeriodResult::Skipped | PeriodResult::Paused => Ok(previous),
        PeriodResult::Achievement(value) => {
            validate_ratio(value)?;
            let updated = match previous {
                Some(previous_value) => alpha * value + (1.0 - alpha) * previous_value,
                None => value,
            };
            Ok(Some(updated.clamp(0.0, 1.0)))
        }
    }
}

/// Count the current run of successful eligible periods. Skips and pauses do not
/// break the run; an eligible period below target (including a missed period = 0.0) does.
pub fn current_streak(results: &[PeriodResult]) -> Result<u32, ScoreError> {
    let mut streak = 0_u32;
    for result in results {
        match result {
            PeriodResult::Skipped | PeriodResult::Paused => {}
            PeriodResult::Achievement(value) => {
                validate_ratio(*value)?;
                if *value >= 1.0 {
                    streak = streak.saturating_add(1);
                } else {
                    streak = 0;
                }
            }
        }
    }
    Ok(streak)
}

fn validate_ratio(value: f64) -> Result<(), ScoreError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(ScoreError::InvalidRatio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u8, day: u8) -> CivilDate {
        CivilDate::new(year, month, day).expect("test date should be valid")
    }

    #[test]
    fn validates_gregorian_dates_and_leap_years() {
        assert!(CivilDate::new(2024, 2, 29).is_some());
        assert!(CivilDate::new(2025, 2, 29).is_none());
        assert!(CivilDate::new(1900, 2, 29).is_none());
        assert!(CivilDate::new(2000, 2, 29).is_some());
    }

    #[test]
    fn calculates_iso_weekday() {
        assert_eq!(date(1970, 1, 1).weekday_monday_zero(), 3);
        assert_eq!(date(2026, 11, 14).weekday_monday_zero(), 5);
        assert_eq!(date(2026, 11, 16).weekday_monday_zero(), 0);
    }

    #[test]
    fn daily_schedule_has_one_day_periods() {
        let schedule = HabitSchedule::new(Cadence::Daily, date(2026, 11, 1), None).unwrap();
        assert_eq!(
            schedule.period_for(date(2026, 11, 14)),
            Some(PeriodKey {
                start_day: date(2026, 11, 14).ordinal(),
                span_days: 1,
            })
        );
    }

    #[test]
    fn weekday_schedule_uses_monday_to_sunday_mask() {
        let monday_and_friday = (1 << 0) | (1 << 4);
        let schedule = HabitSchedule::new(
            Cadence::Weekdays {
                weekday_mask: monday_and_friday,
            },
            date(2026, 11, 1),
            None,
        )
        .unwrap();

        assert!(schedule.period_for(date(2026, 11, 9)).is_some()); // Monday
        assert!(schedule.period_for(date(2026, 11, 13)).is_some()); // Friday
        assert!(schedule.period_for(date(2026, 11, 14)).is_none()); // Saturday
    }

    #[test]
    fn every_n_days_is_anchored_to_start_date() {
        let schedule = HabitSchedule::new(
            Cadence::EveryNDays { interval: 2 },
            date(2026, 11, 1),
            None,
        )
        .unwrap();
        assert!(schedule.period_for(date(2026, 11, 1)).is_some());
        assert!(schedule.period_for(date(2026, 11, 3)).is_some());
        assert!(schedule.period_for(date(2026, 11, 4)).is_none());
    }

    #[test]
    fn times_per_week_uses_one_monday_anchored_period() {
        let schedule = HabitSchedule::new(
            Cadence::TimesPerWeek { target: 3 },
            date(2026, 11, 1),
            None,
        )
        .unwrap();
        let monday = schedule.period_for(date(2026, 11, 9)).unwrap();
        let friday = schedule.period_for(date(2026, 11, 13)).unwrap();
        assert_eq!(monday, friday);
        assert_eq!(monday.span_days, 7);
        assert_eq!(schedule.target_per_period(), 3);
    }

    #[test]
    fn date_bounds_are_respected() {
        let schedule = HabitSchedule::new(
            Cadence::Daily,
            date(2026, 11, 10),
            Some(date(2026, 11, 12)),
        )
        .unwrap();
        assert!(schedule.period_for(date(2026, 11, 9)).is_none());
        assert!(schedule.period_for(date(2026, 11, 12)).is_some());
        assert!(schedule.period_for(date(2026, 11, 13)).is_none());
    }

    #[test]
    fn rejects_invalid_schedule_definitions() {
        assert_eq!(
            HabitSchedule::new(
                Cadence::Weekdays { weekday_mask: 0 },
                date(2026, 11, 1),
                None
            )
            .unwrap_err(),
            ScheduleError::InvalidWeekdayMask
        );
        assert_eq!(
            HabitSchedule::new(
                Cadence::EveryNDays { interval: 0 },
                date(2026, 11, 1),
                None
            )
            .unwrap_err(),
            ScheduleError::InvalidInterval
        );
        assert_eq!(
            HabitSchedule::new(
                Cadence::TimesPerWeek { target: 0 },
                date(2026, 11, 1),
                None
            )
            .unwrap_err(),
            ScheduleError::InvalidWeeklyTarget
        );
        assert_eq!(
            HabitSchedule::new(
                Cadence::TimesPerWeek { target: 8 },
                date(2026, 11, 1),
                None
            )
            .unwrap_err(),
            ScheduleError::InvalidWeeklyTarget
        );
    }

    #[test]
    fn achievement_ratio_handles_cap_habits() {
        assert_eq!(
            achievement_ratio(6.0, 4.0, Direction::AtLeast).unwrap(),
            1.0
        );
        assert_eq!(
            achievement_ratio(150.0, 120.0, Direction::AtMost).unwrap(),
            0.8
        );
        assert_eq!(
            achievement_ratio(60.0, 120.0, Direction::AtMost).unwrap(),
            1.0
        );
        assert_eq!(
            achievement_ratio(0.0, 120.0, Direction::AtMost).unwrap(),
            1.0
        );
    }

    #[test]
    fn adherence_excludes_skips_and_pauses() {
        let values = [
            PeriodResult::Achievement(1.0),
            PeriodResult::Skipped,
            PeriodResult::Achievement(0.5),
            PeriodResult::Paused,
        ];
        assert_eq!(adherence(&values).unwrap(), Some(0.75));
        assert_eq!(
            adherence(&[PeriodResult::Skipped, PeriodResult::Paused]).unwrap(),
            None
        );
    }

    #[test]
    fn strength_uses_ema_and_does_not_change_on_skip() {
        assert_eq!(
            update_strength(None, PeriodResult::Achievement(0.8), 0.25).unwrap(),
            Some(0.8)
        );
        let updated =
            update_strength(Some(0.8), PeriodResult::Achievement(0.4), 0.25).unwrap().unwrap();
        assert!((updated - 0.7).abs() < 1e-12);
        assert_eq!(
            update_strength(Some(0.7), PeriodResult::Skipped, 0.25).unwrap(),
            Some(0.7)
        );
        assert_eq!(
            update_strength(Some(0.7), PeriodResult::Paused, 0.25).unwrap(),
            Some(0.7)
        );
    }

    #[test]
    fn missed_period_breaks_streak_but_skip_and_pause_do_not() {
        let values = [
            PeriodResult::Achievement(1.0),
            PeriodResult::Skipped,
            PeriodResult::Achievement(1.0),
            PeriodResult::Paused,
            PeriodResult::Achievement(0.0),
            PeriodResult::Achievement(1.0),
            PeriodResult::Achievement(1.0),
        ];
        assert_eq!(current_streak(&values).unwrap(), 2);
    }

    #[test]
    fn rejects_invalid_score_inputs() {
        assert_eq!(
            achievement_ratio(1.0, 0.0, Direction::AtLeast).unwrap_err(),
            ScoreError::InvalidTarget
        );
        assert_eq!(
            achievement_ratio(f64::NAN, 1.0, Direction::AtLeast).unwrap_err(),
            ScoreError::InvalidActual
        );
        assert_eq!(
            adherence(&[PeriodResult::Achievement(1.2)]).unwrap_err(),
            ScoreError::InvalidRatio
        );
        assert_eq!(
            update_strength(None, PeriodResult::Achievement(0.5), 2.0).unwrap_err(),
            ScoreError::InvalidAlpha
        );
    }
}
