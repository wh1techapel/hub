//! Time as stored and as the owner lives it.
//!
//! Instants are UTC text with second precision, always the same shape, so that text order in
//! SQLite is time order. Dates and clock times are local to the owner's time zone.

use jiff::civil::{Date, Time};
use jiff::tz::TimeZone;
use jiff::Timestamp;

use crate::error::{invalid, Reason, Result};

/// `2026-10-22T05:00:00Z`.
pub fn instant(ts: Timestamp) -> String {
    ts.strftime("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn parse_instant(field: &'static str, text: &str) -> Result<Timestamp> {
    text.parse::<Timestamp>()
        .or_else(|_| invalid(field, Reason::Format))
}

/// A local date, strictly `YYYY-MM-DD`.
pub fn parse_date(field: &'static str, text: &str) -> Result<Date> {
    if text.len() != 10 {
        return invalid(field, Reason::Format);
    }
    text.parse::<Date>().or_else(|_| invalid(field, Reason::Format))
}

/// A local clock time, strictly `HH:MM`.
pub fn parse_clock(field: &'static str, text: &str) -> Result<Time> {
    let bytes = text.as_bytes();
    if bytes.len() != 5 || bytes[2] != b':' || !text.is_char_boundary(2) {
        return invalid(field, Reason::Format);
    }
    let hour: i8 = text[..2].parse().or_else(|_| invalid(field, Reason::Format))?;
    let minute: i8 = text[3..].parse().or_else(|_| invalid(field, Reason::Format))?;
    Time::new(hour, minute, 0, 0).or_else(|_| invalid(field, Reason::Format))
}

pub fn date_text(date: Date) -> String {
    date.to_string()
}

pub fn clock_text(time: Time) -> String {
    format!("{:02}:{:02}", time.hour(), time.minute())
}

pub fn minutes_of(time: Time) -> i32 {
    i32::from(time.hour()) * 60 + i32::from(time.minute())
}

/// One local day: its date and the instants where it starts and ends (end exclusive). A day is
/// not always 24 hours where clocks change, so both ends are computed, never added.
#[derive(Debug, Clone)]
pub struct Day {
    pub date: Date,
    pub start: Timestamp,
    pub end: Timestamp,
}

impl Day {
    pub fn containing(now: Timestamp, tz: &TimeZone) -> Result<Day> {
        let start = now.to_zoned(tz.clone()).start_of_day()?;
        let end = start.tomorrow()?.start_of_day()?;
        Ok(Day {
            date: start.date(),
            start: start.timestamp(),
            end: end.timestamp(),
        })
    }

    pub fn of(date: Date, tz: &TimeZone) -> Result<Day> {
        let start = date.to_zoned(tz.clone())?.start_of_day()?;
        let end = start.tomorrow()?.start_of_day()?;
        Ok(Day {
            date,
            start: start.timestamp(),
            end: end.timestamp(),
        })
    }

    /// Minutes from the start of this day, clamped to the day.
    pub fn minute(&self, ts: Timestamp) -> i32 {
        let length = (self.end.as_second() - self.start.as_second()) / 60;
        let offset = (ts.as_second() - self.start.as_second()) / 60;
        offset.clamp(0, length) as i32
    }
}

/// The owner's time zone: the one in settings when it names a real zone, else Windows' own.
pub fn zone(name: Option<&str>) -> TimeZone {
    name.and_then(|name| TimeZone::get(name).ok())
        .unwrap_or_else(TimeZone::system)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instants_keep_one_shape() {
        let ts: Timestamp = "2026-10-22T05:00:00.123Z".parse().unwrap();
        assert_eq!(instant(ts), "2026-10-22T05:00:00Z");
    }

    #[test]
    fn clock_times_are_strict() {
        assert_eq!(clock_text(parse_clock("t", "05:00").unwrap()), "05:00");
        assert_eq!(clock_text(parse_clock("t", "22:30").unwrap()), "22:30");
        for bad in ["5:00", "05.00", "24:00", "05:60", "0500", "", "05:0a"] {
            assert!(parse_clock("t", bad).is_err(), "{bad} should be refused");
        }
    }

    #[test]
    fn dates_are_strict() {
        assert!(parse_date("d", "2026-10-22").is_ok());
        for bad in ["2026-1-22", "22-10-2026", "2026-02-30", "2026-10-22T00:00"] {
            assert!(parse_date("d", bad).is_err(), "{bad} should be refused");
        }
    }

    #[test]
    fn a_jakarta_day_starts_at_local_midnight() {
        let tz = TimeZone::get("Asia/Jakarta").unwrap();
        let now: Timestamp = "2026-10-21T18:30:00Z".parse().unwrap(); // 01:30 WIB on the 22nd
        let day = Day::containing(now, &tz).unwrap();
        assert_eq!(date_text(day.date), "2026-10-22");
        assert_eq!(instant(day.start), "2026-10-21T17:00:00Z");
        assert_eq!(instant(day.end), "2026-10-22T17:00:00Z");
        assert_eq!(day.minute(now), 90);
        assert_eq!(day.minute("2026-10-20T00:00:00Z".parse().unwrap()), 0);
        assert_eq!(day.minute("2026-10-25T00:00:00Z".parse().unwrap()), 1440);
    }
}
