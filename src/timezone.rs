//! IANA time-zone handling for date-based habits and wall-clock reminders.
//!
//! Habit dates are civil dates in a named zone. They must not be derived by
//! adding fixed 24-hour durations to UTC instants. Reminder wall times are
//! separately classified as unique, a DST gap, or a DST fold.

use jiff::{
    Timestamp, ZonedDateTime,
    civil::{Date, DateTime},
    tz::{AmbiguousOffset, TimeZone},
};
use std::fmt;

/// Failure to resolve a stable IANA time-zone identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoneIdError {
    EmptyIdentifier,
    SurroundingWhitespace,
    UnknownIdentifier(String),
    NotIanaIdentifier(String),
    SystemZoneHasNoIanaIdentifier,
}

impl fmt::Display for ZoneIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIdentifier => write!(formatter, "time-zone identifier is empty"),
            Self::SurroundingWhitespace => {
                write!(
                    formatter,
                    "time-zone identifier must not contain surrounding whitespace"
                )
            }
            Self::UnknownIdentifier(identifier) => {
                write!(formatter, "unknown IANA time-zone identifier: {identifier}")
            }
            Self::NotIanaIdentifier(identifier) => {
                write!(
                    formatter,
                    "time-zone value is not a named IANA zone: {identifier}"
                )
            }
            Self::SystemZoneHasNoIanaIdentifier => {
                write!(
                    formatter,
                    "the system time zone could not be resolved to an IANA identifier"
                )
            }
        }
    }
}

impl std::error::Error for ZoneIdError {}

/// A validated, canonical IANA time-zone identifier and its transition rules.
///
/// The canonical identifier is suitable for persistence in the
/// habits.time_zone_id column. Persist the name, not today's UTC offset:
/// offsets change when time-zone rules change.
#[derive(Debug, Clone)]
pub struct IanaTimeZone {
    id: String,
    rules: TimeZone,
}

impl IanaTimeZone {
    /// Load and canonicalize an IANA identifier such as Asia/Kolkata.
    ///
    /// Fixed offsets, POSIX rule strings, Etc/Unknown, empty values and
    /// misspelled identifiers are rejected. Case-insensitive IANA aliases
    /// are normalized by Jiff's time-zone database.
    pub fn parse(identifier: &str) -> Result<Self, ZoneIdError> {
        if identifier.trim().is_empty() {
            return Err(ZoneIdError::EmptyIdentifier);
        }
        if identifier.trim() != identifier {
            return Err(ZoneIdError::SurroundingWhitespace);
        }

        let rules = TimeZone::get(identifier)
            .map_err(|_| ZoneIdError::UnknownIdentifier(identifier.to_owned()))?;
        let id = rules
            .iana_name()
            .filter(|name| !rules.is_unknown() && !name.is_empty())
            .ok_or_else(|| ZoneIdError::NotIanaIdentifier(identifier.to_owned()))?
            .to_owned();

        Ok(Self { id, rules })
    }

    /// Resolve the current system zone once and return its stable IANA name.
    ///
    /// Callers should persist the returned name for a habit or schedule
    /// revision. They should not keep the special local marker as the
    /// historical zone, because the OS setting can later change.
    pub fn system() -> Result<Self, ZoneIdError> {
        let rules = TimeZone::system();
        let id = rules
            .iana_name()
            .filter(|name| !rules.is_unknown() && !name.is_empty())
            .ok_or(ZoneIdError::SystemZoneHasNoIanaIdentifier)?
            .to_owned();

        Ok(Self { id, rules })
    }

    /// Return the canonical IANA identifier, for example Asia/Kolkata.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Convert an absolute instant to its civil calendar date in this zone.
    ///
    /// The result is a date, not a timestamp. For recurring habits, the
    /// schedule should evaluate this date directly and must not assume a
    /// civil day is always 86,400 seconds long.
    pub fn local_date_at(&self, timestamp: Timestamp) -> Date {
        self.rules.to_datetime(timestamp).date()
    }

    /// Classify a user-entered wall-clock time before scheduling a reminder.
    ///
    /// Gap means that the local clock skipped the requested time.
    /// Fold means that the requested clock time occurs twice.
    pub fn classify_local_datetime(&self, local: DateTime) -> LocalDateTimeKind {
        match self.rules.to_ambiguous_zoned_datetime(local).offset() {
            AmbiguousOffset::Unambiguous { .. } => LocalDateTimeKind::Unique,
            AmbiguousOffset::Gap { .. } => LocalDateTimeKind::Gap,
            AmbiguousOffset::Fold { .. } => LocalDateTimeKind::Fold,
        }
    }

    /// Resolve a reminder's local wall-clock time using an explicit policy.
    ///
    /// Reject is the safe default for a new UI: the caller must ask the
    /// user to choose when the time is in a gap or fold. The other policies
    /// are provided for a future, explicitly user-selected preference.
    pub fn resolve_local_datetime(
        &self,
        local: DateTime,
        policy: DisambiguationPolicy,
    ) -> Result<ZonedDateTime, jiff::Error> {
        let candidate = self.rules.to_ambiguous_zoned_datetime(local);
        match policy {
            DisambiguationPolicy::Reject => candidate.unambiguous(),
            DisambiguationPolicy::Earlier => candidate.earlier(),
            DisambiguationPolicy::Later => candidate.later(),
            DisambiguationPolicy::Compatible => candidate.compatible(),
        }
    }
}

/// The three possible mappings from a wall-clock datetime to an instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalDateTimeKind {
    /// Exactly one instant corresponds to the requested local datetime.
    Unique,
    /// The clock jumped forward; this local datetime never occurred.
    Gap,
    /// The clock moved backward; this local datetime occurred twice.
    Fold,
}

/// A deliberate policy for turning a local reminder time into an instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisambiguationPolicy {
    /// Reject gaps and folds so the application can ask the user.
    Reject,
    /// Select the earlier interpretation.
    Earlier,
    /// Select the later interpretation.
    Later,
    /// For gaps, move forward; for folds, choose the earlier occurrence.
    /// This follows the common RFC 5545-compatible strategy.
    Compatible,
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    fn zone(identifier: &str) -> IanaTimeZone {
        IanaTimeZone::parse(identifier).expect("test time zone must be valid")
    }

    #[test]
    fn canonicalizes_case_insensitive_iana_identifiers() {
        let zone = zone("america/NEW_YORK");
        assert_eq!(zone.id(), "America/New_York");
    }

    #[test]
    fn rejects_empty_whitespace_unknown_and_non_iana_values() {
        assert_eq!(
            IanaTimeZone::parse(" ").unwrap_err(),
            ZoneIdError::EmptyIdentifier
        );
        assert_eq!(
            IanaTimeZone::parse(" Asia/Kolkata").unwrap_err(),
            ZoneIdError::SurroundingWhitespace
        );
        assert!(matches!(
            IanaTimeZone::parse("Mars/OlympusMons"),
            Err(ZoneIdError::UnknownIdentifier(_))
        ));
        assert!(matches!(
            IanaTimeZone::parse("Etc/Unknown"),
            Err(ZoneIdError::NotIanaIdentifier(_))
        ));
    }

    #[test]
    fn local_date_is_computed_from_the_instant_in_the_selected_zone() {
        let kolkata = zone("Asia/Kolkata");
        let utc: Timestamp = "2024-01-01T20:00:00Z"
            .parse()
            .expect("valid RFC 3339 instant");

        assert_eq!(kolkata.local_date_at(utc).to_string(), "2024-01-02");
    }

    #[test]
    fn local_midnight_boundary_is_not_assumed_to_be_utc_midnight() {
        let kolkata = zone("Asia/Kolkata");
        let before: Timestamp = "2026-11-17T18:29:59Z".parse().expect("valid instant");
        let at_midnight: Timestamp = "2026-11-17T18:30:00Z".parse().expect("valid instant");

        assert_eq!(kolkata.local_date_at(before).to_string(), "2026-11-17");
        assert_eq!(kolkata.local_date_at(at_midnight).to_string(), "2026-11-18");
    }

    #[test]
    fn skipped_civil_date_is_not_emulated_by_adding_24_hours() {
        let apia = zone("Pacific/Apia");
        let before_skip: Timestamp = "2011-12-30T09:59:59Z".parse().expect("valid instant");
        let after_skip: Timestamp = "2011-12-30T10:00:00Z".parse().expect("valid instant");

        assert_eq!(apia.local_date_at(before_skip).to_string(), "2011-12-29");
        assert_eq!(apia.local_date_at(after_skip).to_string(), "2011-12-31");
    }

    #[test]
    fn new_york_spring_forward_is_classified_as_a_gap() {
        let new_york = zone("America/New_York");
        let gap = date(2024, 3, 10).at(2, 30, 0, 0);

        assert_eq!(
            new_york.classify_local_datetime(gap),
            LocalDateTimeKind::Gap
        );
        assert!(
            new_york
                .resolve_local_datetime(gap, DisambiguationPolicy::Reject)
                .is_err()
        );
    }

    #[test]
    fn new_york_fall_back_is_classified_as_a_fold() {
        let new_york = zone("America/New_York");
        let fold = date(2024, 11, 3).at(1, 30, 0, 0);

        assert_eq!(
            new_york.classify_local_datetime(fold),
            LocalDateTimeKind::Fold
        );
        assert!(
            new_york
                .resolve_local_datetime(fold, DisambiguationPolicy::Reject)
                .is_err()
        );
    }

    #[test]
    fn unique_wall_clock_time_resolves_without_guessing() {
        let new_york = zone("America/New_York");
        let unique = date(2024, 3, 10).at(1, 30, 0, 0);

        assert_eq!(
            new_york.classify_local_datetime(unique),
            LocalDateTimeKind::Unique
        );
        assert_eq!(
            new_york
                .resolve_local_datetime(unique, DisambiguationPolicy::Reject)
                .expect("unique time must resolve")
                .to_string(),
            "2024-03-10T01:30:00-05:00[America/New_York]"
        );
    }

    #[test]
    fn explicit_gap_resolution_has_deterministic_results() {
        let new_york = zone("America/New_York");
        let gap = date(2024, 3, 10).at(2, 30, 0, 0);

        let earlier = new_york
            .resolve_local_datetime(gap, DisambiguationPolicy::Earlier)
            .expect("earlier gap resolution");
        let later = new_york
            .resolve_local_datetime(gap, DisambiguationPolicy::Later)
            .expect("later gap resolution");
        let compatible = new_york
            .resolve_local_datetime(gap, DisambiguationPolicy::Compatible)
            .expect("compatible gap resolution");

        assert_eq!(
            earlier.to_string(),
            "2024-03-10T01:30:00-05:00[America/New_York]"
        );
        assert_eq!(
            later.to_string(),
            "2024-03-10T03:30:00-04:00[America/New_York]"
        );
        assert_eq!(compatible, later);
    }

    #[test]
    fn explicit_fold_resolution_selects_the_first_or_second_occurrence() {
        let new_york = zone("America/New_York");
        let fold = date(2024, 11, 3).at(1, 30, 0, 0);

        let earlier = new_york
            .resolve_local_datetime(fold, DisambiguationPolicy::Earlier)
            .expect("earlier fold resolution");
        let later = new_york
            .resolve_local_datetime(fold, DisambiguationPolicy::Later)
            .expect("later fold resolution");
        let compatible = new_york
            .resolve_local_datetime(fold, DisambiguationPolicy::Compatible)
            .expect("compatible fold resolution");

        assert_eq!(
            earlier.to_string(),
            "2024-11-03T01:30:00-04:00[America/New_York]"
        );
        assert_eq!(
            later.to_string(),
            "2024-11-03T01:30:00-05:00[America/New_York]"
        );
        assert_eq!(compatible, earlier);
    }
}
