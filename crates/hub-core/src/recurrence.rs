//! When a routine happens: a small, checked subset of iCalendar's RRULE.
//!
//! `FREQ=DAILY|WEEKLY|MONTHLY`, `INTERVAL=n`, and for weekly rules `BYDAY=MO,WE,...`. That covers a
//! school timetable, "belajar cyber Senin Rabu Jumat", and "every other week". The start and end
//! dates live on the routine itself (`starts_on`, `ends_on`). Stored as canonical text so another
//! tool reading the database sees a familiar rule.

use jiff::civil::{Date, Weekday};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{invalid, Reason, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Freq {
    Daily,
    Weekly,
    Monthly,
}

/// Days of the week, Monday first (Indonesia's week).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Day {
    Mo,
    Tu,
    We,
    Th,
    Fr,
    Sa,
    Su,
}

impl Day {
    const ALL: [Day; 7] = [Day::Mo, Day::Tu, Day::We, Day::Th, Day::Fr, Day::Sa, Day::Su];

    fn code(self) -> &'static str {
        match self {
            Day::Mo => "MO",
            Day::Tu => "TU",
            Day::We => "WE",
            Day::Th => "TH",
            Day::Fr => "FR",
            Day::Sa => "SA",
            Day::Su => "SU",
        }
    }

    pub fn of(date: Date) -> Day {
        Day::ALL[usize::from(date.weekday().to_monday_zero_offset() as u8)]
    }

    pub fn weekday(self) -> Weekday {
        Weekday::from_monday_zero_offset(self as i8).expect("seven days")
    }
}

/// A rule as the UI edits it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Rule {
    pub freq: Freq,
    pub interval: u16,
    /// Weekly rules only; empty means "the weekday of the first date".
    pub by_day: Vec<Day>,
}

const INTERVAL_MAX: u16 = 52;

impl Rule {
    pub fn weekly(days: &[Day]) -> Rule {
        Rule {
            freq: Freq::Weekly,
            interval: 1,
            by_day: days.to_vec(),
        }
    }

    /// Checked and in canonical order.
    pub fn validated(mut self) -> Result<Rule> {
        if self.interval == 0 || self.interval > INTERVAL_MAX {
            return invalid("rule", Reason::Format);
        }
        if self.freq != Freq::Weekly && !self.by_day.is_empty() {
            return invalid("rule", Reason::Format);
        }
        self.by_day.sort();
        self.by_day.dedup();
        Ok(self)
    }

    pub fn to_text(&self) -> String {
        let freq = match self.freq {
            Freq::Daily => "DAILY",
            Freq::Weekly => "WEEKLY",
            Freq::Monthly => "MONTHLY",
        };
        let mut text = format!("FREQ={freq}");
        if self.interval != 1 {
            text.push_str(&format!(";INTERVAL={}", self.interval));
        }
        if !self.by_day.is_empty() {
            let days: Vec<&str> = self.by_day.iter().map(|day| day.code()).collect();
            text.push_str(&format!(";BYDAY={}", days.join(",")));
        }
        text
    }

    pub fn parse(text: &str) -> Result<Rule> {
        let mut freq = None;
        let mut interval = 1;
        let mut by_day = Vec::new();
        for part in text.split(';').filter(|part| !part.is_empty()) {
            let Some((key, value)) = part.split_once('=') else {
                return invalid("rule", Reason::Format);
            };
            match key {
                "FREQ" => {
                    freq = Some(match value {
                        "DAILY" => Freq::Daily,
                        "WEEKLY" => Freq::Weekly,
                        "MONTHLY" => Freq::Monthly,
                        _ => return invalid("rule", Reason::Format),
                    })
                }
                "INTERVAL" => interval = value.parse().or_else(|_| invalid("rule", Reason::Format))?,
                "BYDAY" => {
                    for code in value.split(',') {
                        let day = Day::ALL.iter().find(|day| day.code() == code).copied();
                        match day {
                            Some(day) => by_day.push(day),
                            None => return invalid("rule", Reason::Format),
                        }
                    }
                }
                _ => return invalid("rule", Reason::Format),
            }
        }
        let Some(freq) = freq else {
            return invalid("rule", Reason::Format);
        };
        Rule { freq, interval, by_day }.validated()
    }

    /// Whether the routine that starts on `start` happens on `date`.
    pub fn occurs_on(&self, start: Date, date: Date) -> bool {
        if date < start {
            return false;
        }
        let interval = i64::from(self.interval.max(1));
        match self.freq {
            Freq::Daily => days_between(start, date) % interval == 0,
            Freq::Weekly => {
                let day = Day::of(date);
                let on_day = if self.by_day.is_empty() {
                    day == Day::of(start)
                } else {
                    self.by_day.contains(&day)
                };
                on_day && (days_between(monday_of(start), monday_of(date)) / 7) % interval == 0
            }
            Freq::Monthly => {
                let months = (i64::from(date.year()) * 12 + i64::from(date.month()))
                    - (i64::from(start.year()) * 12 + i64::from(start.month()));
                date.day() == start.day() && months % interval == 0
            }
        }
    }
}

fn days_between(from: Date, to: Date) -> i64 {
    from.until(to).map(|span| i64::from(span.get_days())).unwrap_or(0)
}

fn monday_of(date: Date) -> Date {
    let back = i64::from(date.weekday().to_monday_zero_offset());
    date.checked_sub(jiff::Span::new().days(back)).unwrap_or(date)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(text: &str) -> Date {
        text.parse().unwrap()
    }

    #[test]
    fn text_round_trips_in_canonical_form() {
        let rule = Rule::parse("FREQ=WEEKLY;BYDAY=WE,MO,WE").unwrap();
        assert_eq!(rule.to_text(), "FREQ=WEEKLY;BYDAY=MO,WE");
        assert_eq!(Rule::parse("FREQ=DAILY;INTERVAL=2").unwrap().to_text(), "FREQ=DAILY;INTERVAL=2");
        for bad in ["", "FREQ=HOURLY", "FREQ=DAILY;BYDAY=MO", "FREQ=WEEKLY;INTERVAL=0", "FREQ=WEEKLY;BYDAY=XX", "FREQ=WEEKLY;COUNT=3"] {
            assert!(Rule::parse(bad).is_err(), "{bad} should be refused");
        }
    }

    #[test]
    fn a_school_timetable_repeats_on_its_days() {
        // Starts Thursday 22 October 2026: Monday and Thursday lessons.
        let rule = Rule::weekly(&[Day::Mo, Day::Th]);
        let start = d("2026-10-22");
        assert!(rule.occurs_on(start, d("2026-10-22")));
        assert!(!rule.occurs_on(start, d("2026-10-23")));
        assert!(rule.occurs_on(start, d("2026-10-26")));
        assert!(rule.occurs_on(start, d("2026-10-29")));
        assert!(!rule.occurs_on(start, d("2026-10-19")), "before the start");
    }

    #[test]
    fn every_other_week_counts_whole_weeks() {
        let rule = Rule { freq: Freq::Weekly, interval: 2, by_day: vec![Day::Fr] };
        let start = d("2026-10-21"); // Wednesday
        assert!(rule.occurs_on(start, d("2026-10-23")));
        assert!(!rule.occurs_on(start, d("2026-10-30")));
        assert!(rule.occurs_on(start, d("2026-11-06")));
    }

    #[test]
    fn empty_by_day_means_the_first_weekday() {
        let rule = Rule::weekly(&[]);
        assert!(rule.occurs_on(d("2026-10-22"), d("2026-10-29")));
        assert!(!rule.occurs_on(d("2026-10-22"), d("2026-10-28")));
    }

    #[test]
    fn daily_and_monthly() {
        let daily = Rule { freq: Freq::Daily, interval: 3, by_day: vec![] };
        assert!(daily.occurs_on(d("2026-10-01"), d("2026-10-04")));
        assert!(!daily.occurs_on(d("2026-10-01"), d("2026-10-05")));
        let monthly = Rule { freq: Freq::Monthly, interval: 1, by_day: vec![] };
        assert!(monthly.occurs_on(d("2026-01-31"), d("2026-03-31")));
        assert!(!monthly.occurs_on(d("2026-01-31"), d("2026-02-28")), "no 31st in February");
    }

    #[test]
    fn days_are_monday_first() {
        assert_eq!(Day::of(d("2026-10-26")), Day::Mo);
        assert_eq!(Day::of(d("2026-10-25")), Day::Su);
        assert_eq!(Day::Su.weekday(), Weekday::Sunday);
    }
}
