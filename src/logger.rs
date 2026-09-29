//! A small logger that writes diagnostics to standard error.
//!
//! The output mirrors the one of the Python implementation:
//!
//! ```text
//! 2026-09-29,12:00:00.000 [builder] >> rpsql.builder [ERROR] -> recursive include: a.sql
//! ```

use std::fmt::Write as _;
use std::io::Write as _;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Severity of a log record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Verbose tracing, only shown in debug mode.
    Debug = 10,
    /// Failures that stop the build.
    Error = 40,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Debug => "DEBUG",
            Level::Error => "ERROR",
        }
    }
}

static LEVEL: AtomicU8 = AtomicU8::new(Level::Error as u8);

/// Sets the lowest severity that is written out.
pub fn set_level(level: Level) {
    LEVEL.store(level as u8, Ordering::Relaxed);
}

/// The lowest severity that is currently written out.
pub fn level() -> Level {
    match LEVEL.load(Ordering::Relaxed) {
        10 => Level::Debug,
        _ => Level::Error,
    }
}

/// Writes a record when `level` passes the configured threshold.
pub fn log(level: Level, module: &str, message: &str) {
    if level < self::level() {
        return;
    }
    let mut line = format!(
        "{} [{module}] >> rpsql.{module} [{}] -> {message}",
        timestamp(),
        level.as_str()
    );
    line.push('\n');
    let _ = std::io::stderr().write_all(line.as_bytes());
}

/// Formats the current UTC time as `YYYY-MM-DD,HH:MM:SS.mmm`.
fn timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format_utc(now.as_secs() as i64, now.subsec_millis())
}

/// Formats a unix timestamp as `YYYY-MM-DD,HH:MM:SS.mmm`.
fn format_utc(seconds: i64, millis: u32) -> String {
    let days = seconds.div_euclid(86_400);
    let time_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second) = (
        time_of_day / 3_600,
        (time_of_day % 3_600) / 60,
        time_of_day % 60,
    );
    let mut out = String::with_capacity(23);
    let _ = write!(
        out,
        "{year:04}-{month:02}-{day:02},{hour:02}:{minute:02}:{second:02}.{millis:03}"
    );
    out
}

/// Converts a count of days since 1970-01-01 into a proleptic Gregorian date.
///
/// Howard Hinnant's `civil_from_days`, shifted so that the era starts in March
/// and every 400 year cycle starts on the first of March.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_the_unix_epoch() {
        assert_eq!(format_utc(0, 0), "1970-01-01,00:00:00.000");
    }

    #[test]
    fn formats_a_known_date() {
        assert_eq!(format_utc(1_757_000_000, 123), "2025-09-04,15:33:20.123");
    }

    #[test]
    fn formats_a_leap_day() {
        assert_eq!(format_utc(1_709_164_800, 0), "2024-02-29,00:00:00.000");
    }

    #[test]
    fn formats_a_century_boundary() {
        assert_eq!(format_utc(4_107_542_400, 999), "2100-03-01,00:00:00.999");
    }

    #[test]
    fn civil_from_days_handles_negative_days() {
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }

    #[test]
    fn level_round_trips() {
        set_level(Level::Debug);
        assert_eq!(level(), Level::Debug);
        set_level(Level::Error);
        assert_eq!(level(), Level::Error);
    }

    #[test]
    fn the_level_is_clamped_to_a_known_value() {
        set_level(Level::Debug);
        set_level(Level::Error);
        assert_eq!(level(), Level::Error);
    }
}
