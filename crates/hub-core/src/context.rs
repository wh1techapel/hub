//! The owner's "now": the time, their zone, their day, and their waking hours, read once per
//! request from settings. The app and, in CP5, the local API build one before every call.

use jiff::tz::TimeZone;
use jiff::Timestamp;
use rusqlite::Connection;

use crate::calendar::Waking;
use crate::error::Result;
use crate::settings::{self, Settings};
use crate::time::{self, date_text, minutes_of, parse_clock, Day};

pub struct Ctx {
    pub now: Timestamp,
    pub tz: TimeZone,
    pub day: Day,
    /// Today, `YYYY-MM-DD`.
    pub date: String,
    pub waking: Waking,
    pub settings: Settings,
}

impl Ctx {
    pub fn load(conn: &Connection, now: Timestamp) -> Result<Ctx> {
        let settings = settings::load(conn)?;
        let tz = time::zone(settings.time_zone.as_deref());
        let day = Day::containing(now, &tz)?;
        let waking = Waking {
            wake: minutes_of(parse_clock("wakeTime", &settings.wake_time)?),
            sleep: minutes_of(parse_clock("sleepTime", &settings.sleep_time)?),
        };
        Ok(Ctx { now, date: date_text(day.date), day, tz, waking, settings })
    }

    /// Minutes from local midnight now.
    pub fn minute(&self) -> i32 {
        self.day.minute(self.now)
    }
}
