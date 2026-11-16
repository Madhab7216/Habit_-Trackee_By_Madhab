//! Shared domain services for Habit Trackers by Madhab.
//!
//! The desktop binary can adopt these services without coupling timezone rules
//! to GTK callbacks or SQLite query code.

pub mod schedule_history;
pub mod timezone;
