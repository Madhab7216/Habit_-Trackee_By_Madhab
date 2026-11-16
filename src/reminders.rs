//! Deterministic reminder and quiet-hours policy.
//!
//! This module deliberately does not read the wall clock, perform timezone conversion,
//! persist delivery state, or call the desktop notification service. The caller must
//! supply a local date/time already resolved in the user's configured IANA timezone.
//! Keeping policy evaluation pure makes DST, quiet-hours and duplicate-delivery rules
//! testable without GTK or a running desktop session.

use crate::domain::CivilDate;
use std::fmt;

/// A validated wall-clock time in minutes after local midnight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalTime(u16);

impl LocalTime {
    pub fn new(hour: u8, minute: u8) -> Result<Self, ReminderConfigError> {
        if hour > 23 || minute > 59 {
            return Err(ReminderConfigError::InvalidTime);
        }
        Ok(Self(u16::from(hour) * 60 + u16::from(minute)))
    }

    /// Parse a strict 24-hour HH:MM value.
    pub fn parse_hhmm(value: &str) -> Result<Self, ReminderConfigError> {
        let bytes = value.as_bytes();
        if bytes.len() != 5
            || bytes[2] != b':'
            || !bytes[0].is_ascii_digit()
            || !bytes[1].is_ascii_digit()
            || !bytes[3].is_ascii_digit()
            || !bytes[4].is_ascii_digit()
        {
            return Err(ReminderConfigError::InvalidTime);
        }

        let hour = (bytes[0] - b'0') * 10 + (bytes[1] - b'0');
        let minute = (bytes[3] - b'0') * 10 + (bytes[4] - b'0');
        Self::new(hour, minute)
    }

    pub const fn minutes_after_midnight(self) -> u16 {
        self.0
    }

    pub const fn hour(self) -> u8 {
        (self.0 / 60) as u8
    }

    pub const fn minute(self) -> u8 {
        (self.0 % 60) as u8
    }
}

impl fmt::Display for LocalTime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:02}:{:02}", self.hour(), self.minute())
    }
}

/// A local wall-clock quiet interval. The start is inclusive and the end is exclusive.
/// Equal boundaries are rejected; users disable quiet hours explicitly instead of
/// relying on an ambiguous "all day" interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuietHours {
    start: LocalTime,
    end: LocalTime,
}

impl QuietHours {
    pub fn new(start: LocalTime, end: LocalTime) -> Result<Self, ReminderConfigError> {
        if start == end {
            return Err(ReminderConfigError::EmptyQuietHours);
        }
        Ok(Self { start, end })
    }

    pub const fn start(self) -> LocalTime {
        self.start
    }

    pub const fn end(self) -> LocalTime {
        self.end
    }

    /// Whether a local wall-clock time falls in this half-open quiet interval.
    /// Intervals crossing midnight are supported (for example, 22:00–07:00).
    pub fn contains(self, time: LocalTime) -> bool {
        if self.start < self.end {
            self.start <= time && time < self.end
        } else {
            time >= self.start || time < self.end
        }
    }
}

/// A weekly, local-wall-clock reminder rule.
///
/// Weekday bits use ISO order: Monday is bit 0 and Sunday is bit 6. The grace
/// window is measured from the scheduled minute on the same local date. A reminder
/// is not replayed on a later date after its grace window expires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReminderSchedule {
    local_time: LocalTime,
    weekday_mask: u8,
    grace_minutes: u16,
    enabled: bool,
}

impl ReminderSchedule {
    pub fn new(
        local_time: LocalTime,
        weekday_mask: u8,
        grace_minutes: u16,
    ) -> Result<Self, ReminderConfigError> {
        const VALID_WEEKDAYS: u8 = 0b0111_1111;
        if weekday_mask == 0 || weekday_mask & !VALID_WEEKDAYS != 0 {
            return Err(ReminderConfigError::InvalidWeekdayMask);
        }
        if !(1..=180).contains(&grace_minutes) {
            return Err(ReminderConfigError::InvalidGracePeriod);
        }

        Ok(Self {
            local_time,
            weekday_mask,
            grace_minutes,
            enabled: true,
        })
    }

    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub const fn local_time(self) -> LocalTime {
        self.local_time
    }

    pub const fn weekday_mask(self) -> u8 {
        self.weekday_mask
    }

    pub const fn grace_minutes(self) -> u16 {
        self.grace_minutes
    }

    pub const fn is_enabled(self) -> bool {
        self.enabled
    }

    pub fn is_scheduled_on(self, date: CivilDate) -> bool {
        let bit = 1_u8 << date.weekday_monday_zero();
        self.weekday_mask & bit != 0
    }

    /// Evaluate a reminder at a caller-supplied local date/time.
    ///
    /// Persist DueOccurrence::date as the delivery idempotency key together with
    /// the reminder ID (unique reminder/date pair). The caller must record delivery
    /// transactionally before or with notification dispatch so restarts do not send
    /// the same occurrence twice.
    pub fn evaluate(
        self,
        now: LocalDateTime,
        last_delivered_on: Option<CivilDate>,
        quiet_hours: Option<QuietHours>,
    ) -> ReminderDecision {
        if !self.enabled {
            return ReminderDecision::Disabled;
        }
        if !self.is_scheduled_on(now.date) {
            return ReminderDecision::NotScheduledToday;
        }
        if last_delivered_on == Some(now.date) {
            return ReminderDecision::AlreadyDelivered;
        }

        let now_minutes = now.time.minutes_after_midnight();
        let scheduled_minutes = self.local_time.minutes_after_midnight();
        if now_minutes < scheduled_minutes {
            return ReminderDecision::BeforeScheduledTime;
        }

        let elapsed = now_minutes - scheduled_minutes;
        if elapsed >= self.grace_minutes {
            return ReminderDecision::OutsideGracePeriod;
        }
        if quiet_hours.is_some_and(|interval| interval.contains(now.time)) {
            return ReminderDecision::QuietHours;
        }

        ReminderDecision::Due(DueOccurrence {
            date: now.date,
            scheduled_time: self.local_time,
        })
    }
}

/// A local wall-clock reading resolved by the timezone adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDateTime {
    pub date: CivilDate,
    pub time: LocalTime,
}

impl LocalDateTime {
    pub const fn new(date: CivilDate, time: LocalTime) -> Self {
        Self { date, time }
    }
}

/// The occurrence that may be delivered once for a reminder on a local date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DueOccurrence {
    pub date: CivilDate,
    pub scheduled_time: LocalTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReminderDecision {
    Disabled,
    NotScheduledToday,
    BeforeScheduledTime,
    AlreadyDelivered,
    OutsideGracePeriod,
    QuietHours,
    Due(DueOccurrence),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReminderConfigError {
    InvalidTime,
    InvalidWeekdayMask,
    InvalidGracePeriod,
    EmptyQuietHours,
}

impl fmt::Display for ReminderConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ReminderConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u8, day: u8) -> CivilDate {
        CivilDate::new(year, month, day).expect("test date should be valid")
    }

    fn time(hour: u8, minute: u8) -> LocalTime {
        LocalTime::new(hour, minute).expect("test time should be valid")
    }

    fn schedule(hour: u8, minute: u8, mask: u8, grace: u16) -> ReminderSchedule {
        ReminderSchedule::new(time(hour, minute), mask, grace)
            .expect("test reminder should be valid")
    }

    fn at(date: CivilDate, hour: u8, minute: u8) -> LocalDateTime {
        LocalDateTime::new(date, time(hour, minute))
    }

    #[test]
    fn parses_and_formats_strict_24_hour_times() {
        let parsed = LocalTime::parse_hhmm("09:05").unwrap();
        assert_eq!(parsed, time(9, 5));
        assert_eq!(parsed.to_string(), "09:05");
        assert_eq!(LocalTime::parse_hhmm("23:59").unwrap(), time(23, 59));

        for invalid in ["9:05", "09:5", "24:00", "12:60", "12-30", "", "１２:３０"] {
            assert_eq!(
                LocalTime::parse_hhmm(invalid),
                Err(ReminderConfigError::InvalidTime),
                "unexpectedly accepted {invalid:?}"
            );
        }
    }

    #[test]
    fn quiet_hours_support_daytime_and_overnight_half_open_ranges() {
        let overnight = QuietHours::new(time(22, 0), time(7, 0)).unwrap();
        assert!(!overnight.contains(time(21, 59)));
        assert!(overnight.contains(time(22, 0)));
        assert!(overnight.contains(time(23, 59)));
        assert!(overnight.contains(time(0, 0)));
        assert!(overnight.contains(time(6, 59)));
        assert!(!overnight.contains(time(7, 0)));

        let daytime = QuietHours::new(time(13, 0), time(14, 0)).unwrap();
        assert!(!daytime.contains(time(12, 59)));
        assert!(daytime.contains(time(13, 0)));
        assert!(daytime.contains(time(13, 59)));
        assert!(!daytime.contains(time(14, 0)));
    }

    #[test]
    fn rejects_invalid_quiet_hours_and_schedule_configuration() {
        assert_eq!(
            QuietHours::new(time(8, 0), time(8, 0)),
            Err(ReminderConfigError::EmptyQuietHours)
        );
        assert_eq!(
            ReminderSchedule::new(time(8, 0), 0, 15),
            Err(ReminderConfigError::InvalidWeekdayMask)
        );
        assert_eq!(
            ReminderSchedule::new(time(8, 0), 0b1000_0000, 15),
            Err(ReminderConfigError::InvalidWeekdayMask)
        );
        assert_eq!(
            ReminderSchedule::new(time(8, 0), 0b0000_0001, 0),
            Err(ReminderConfigError::InvalidGracePeriod)
        );
        assert_eq!(
            ReminderSchedule::new(time(8, 0), 0b0000_0001, 181),
            Err(ReminderConfigError::InvalidGracePeriod)
        );
        assert_eq!(
            LocalTime::new(24, 0),
            Err(ReminderConfigError::InvalidTime)
        );
    }

    #[test]
    fn only_selected_iso_weekdays_are_eligible() {
        let monday = date(2026, 11, 16);
        let tuesday = date(2026, 11, 17);
        let monday_and_friday = (1 << 0) | (1 << 4);
        let rule = schedule(8, 0, monday_and_friday, 15);

        assert!(rule.is_scheduled_on(monday));
        assert!(!rule.is_scheduled_on(tuesday));
        assert_eq!(
            rule.evaluate(at(tuesday, 8, 0), None, None),
            ReminderDecision::NotScheduledToday
        );
    }

    #[test]
    fn due_window_is_inclusive_at_start_and_exclusive_at_expiry() {
        let today = date(2026, 11, 16);
        let rule = schedule(9, 0, 0b0111_1111, 15);

        assert!(matches!(
            rule.evaluate(at(today, 9, 0), None, None),
            ReminderDecision::Due(_)
        ));
        assert!(matches!(
            rule.evaluate(at(today, 9, 14), None, None),
            ReminderDecision::Due(_)
        ));
        assert_eq!(
            rule.evaluate(at(today, 9, 15), None, None),
            ReminderDecision::OutsideGracePeriod
        );
        assert_eq!(
            rule.evaluate(at(today, 8, 59), None, None),
            ReminderDecision::BeforeScheduledTime
        );
    }

    #[test]
    fn delivery_is_idempotent_for_the_same_local_date() {
        let today = date(2026, 11, 16);
        let rule = schedule(9, 0, 0b0111_1111, 30);

        assert_eq!(
            rule.evaluate(at(today, 9, 5), Some(today), None),
            ReminderDecision::AlreadyDelivered
        );
    }

    #[test]
    fn quiet_hours_suppress_delivery_and_allow_only_in_window_catch_up() {
        let today = date(2026, 11, 16);
        let overnight = QuietHours::new(time(22, 0), time(7, 0)).unwrap();
        let early_rule = schedule(6, 55, 0b0111_1111, 15);

        assert_eq!(
            early_rule.evaluate(at(today, 6, 59), None, Some(overnight)),
            ReminderDecision::QuietHours
        );
        assert!(matches!(
            early_rule.evaluate(at(today, 7, 5), None, Some(overnight)),
            ReminderDecision::Due(_)
        ));

        let night_rule = schedule(22, 30, 0b0111_1111, 30);
        assert_eq!(
            night_rule.evaluate(at(today, 22, 30), None, Some(overnight)),
            ReminderDecision::QuietHours
        );
        assert_eq!(
            night_rule.evaluate(at(today, 23, 0), None, Some(overnight)),
            ReminderDecision::OutsideGracePeriod
        );
    }

    #[test]
    fn disabled_reminders_never_become_due() {
        let today = date(2026, 11, 16);
        let rule = schedule(9, 0, 0b0111_1111, 15).with_enabled(false);

        assert_eq!(
            rule.evaluate(at(today, 10, 0), None, None),
            ReminderDecision::Disabled
        );
    }

    #[test]
    fn due_result_carries_a_stable_date_and_scheduled_time() {
        let today = date(2026, 11, 16);
        let rule = schedule(9, 30, 0b0111_1111, 20);

        assert_eq!(
            rule.evaluate(at(today, 9, 35), None, None),
            ReminderDecision::Due(DueOccurrence {
                date: today,
                scheduled_time: time(9, 30),
            })
        );
    }
}
