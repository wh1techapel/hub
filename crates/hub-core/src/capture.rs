//! Catat Cepat: one line of the owner's words, read into a task, reminder, event, or note.
//!
//! Indonesian first, basic English too (owner's choice, CONTEXT.md): "laporan lab besok 19.00
//! #jaringan !!", "ingatkan aku kumpulkan tugas jumat jam 8 malam", "rapat OSIS jumat 13.00-14.30
//! di aula", "lab report tomorrow 7pm". The reading is pure (no database, no clock of its own),
//! so the UI shows exactly what will be saved before Enter, and the same reading is saved.
//!
//! Rules that are choices, not grammar:
//! - A task with a date only gets a do date; with a time (or "tenggat", "deadline", "by") the
//!   date and time are its deadline.
//! - "jam 7" with no part of day: 1 to 6 means afternoon, 7 to 12 as written; today, an hour
//!   already passed moves to the evening when that is still ahead.
//! - A reminder needs a time; an event with a date and no time is all-day.

use jiff::civil::{Date, Weekday};
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::cards::ProjectRef;
use crate::error::{invalid, Reason, Result};
use crate::events::{self, EventInput};
use crate::journal;
use crate::model::{Actor, Change, Entity, JournalKind};
use crate::reminders::{self, ReminderInput};
use crate::tasks::{self, TaskInput};
use crate::time::{clock_text, date_text, Day};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CaptureKind {
    Task,
    Reminder,
    Event,
    Note,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Missing {
    Title,
    Time,
    Date,
}

/// What the line will become. The UI draws its chips from these fields.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Reading {
    pub kind: CaptureKind,
    pub title: String,
    /// `YYYY-MM-DD`: a task's do date (or deadline when `due`), a reminder's or event's day, a note's date.
    pub date: Option<String>,
    /// `HH:MM`.
    pub time: Option<String>,
    /// Events: `HH:MM`, possibly the next morning (then `end_next_day`).
    pub end_time: Option<String>,
    pub end_next_day: bool,
    /// Tasks: the date (and time) is a deadline, not a do date.
    pub due: bool,
    pub priority: u8,
    pub project: Option<ProjectRef>,
    /// A `#tag` that matched no project.
    pub unknown_tag: Option<String>,
    pub estimate_min: Option<u32>,
    /// Events: "di aula".
    pub location: Option<String>,
    /// What must be added before it can be saved.
    pub missing: Option<Missing>,
}

/// Where and when the line is read.
pub struct Context<'a> {
    pub today: Date,
    /// Minutes from local midnight.
    pub now_minute: i32,
    pub projects: &'a [ProjectRef],
}

struct Tok {
    raw: String,
    norm: String,
    used: bool,
}

fn norm(raw: &str) -> String {
    let lower = raw.to_lowercase();
    let trimmed = lower.trim_matches(|c: char| matches!(c, ',' | ';' | '?' | '"' | '(' | ')' | '\'' | '\u{201C}' | '\u{201D}'));
    let trimmed = trimmed.strip_suffix(['.', ':']).unwrap_or(trimmed);
    trimmed.to_string()
}

fn alnum(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

const MONTHS: &[(&str, i8)] = &[
    ("januari", 1), ("january", 1), ("jan", 1),
    ("februari", 2), ("february", 2), ("feb", 2), ("pebruari", 2),
    ("maret", 3), ("march", 3), ("mar", 3),
    ("april", 4), ("apr", 4),
    ("mei", 5), ("may", 5),
    ("juni", 6), ("june", 6), ("jun", 6),
    ("juli", 7), ("july", 7), ("jul", 7),
    ("agustus", 8), ("august", 8), ("agu", 8), ("agt", 8), ("ags", 8), ("aug", 8),
    ("september", 9), ("sept", 9), ("sep", 9),
    ("oktober", 10), ("october", 10), ("okt", 10), ("oct", 10),
    ("november", 11), ("nov", 11), ("nop", 11),
    ("desember", 12), ("december", 12), ("des", 12), ("dec", 12),
];

const WEEKDAYS: &[(&str, Weekday)] = &[
    ("senin", Weekday::Monday), ("monday", Weekday::Monday), ("mon", Weekday::Monday),
    ("selasa", Weekday::Tuesday), ("tuesday", Weekday::Tuesday), ("tue", Weekday::Tuesday), ("tues", Weekday::Tuesday),
    ("rabu", Weekday::Wednesday), ("wednesday", Weekday::Wednesday), ("wed", Weekday::Wednesday),
    ("kamis", Weekday::Thursday), ("thursday", Weekday::Thursday), ("thu", Weekday::Thursday), ("thur", Weekday::Thursday), ("thurs", Weekday::Thursday),
    ("jumat", Weekday::Friday), ("jum'at", Weekday::Friday), ("friday", Weekday::Friday), ("fri", Weekday::Friday),
    ("sabtu", Weekday::Saturday), ("saturday", Weekday::Saturday), ("sat", Weekday::Saturday),
    ("minggu", Weekday::Sunday), ("ahad", Weekday::Sunday), ("sunday", Weekday::Sunday), ("sun", Weekday::Sunday),
];

const DEADLINE_WORDS: &[&str] = &["tenggat", "deadline", "due", "dikumpul", "dikumpulkan", "sebelum", "before", "by", "maks", "maksimal"];
const TIME_MARKERS: &[&str] = &["jam", "pukul", "pk", "pkl", "at", "@"];
const RANGE_WORDS: &[&str] = &["-", "\u{2013}", "sampai", "sampe", "s/d", "sd", "hingga", "to", "until", "till"];
const DANGLING: &[&str] = &["pada", "pd", "di", "on", "at", "jam", "pukul", "tgl", "tanggal", "dan", "&", "-", "untuk", "utk", "for", "hari"];
const REMINDER_PREFIXES: &[&[&str]] = &[
    &["ingatkan", "aku", "untuk"],
    &["ingatkan", "saya", "untuk"],
    &["ingatkan", "aku"],
    &["ingatkan", "saya"],
    &["ingatkan", "untuk"],
    &["ingatkan"],
    &["remind", "me", "to"],
    &["remind", "me"],
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Period {
    Morning,
    Noon,
    Afternoon,
    Night,
    Am,
    Pm,
}

fn period_of(word: &str) -> Option<Period> {
    Some(match word {
        "pagi" => Period::Morning,
        "siang" => Period::Noon,
        "sore" => Period::Afternoon,
        "malam" | "mlm" => Period::Night,
        "am" | "a.m" | "a.m." => Period::Am,
        "pm" | "p.m" | "p.m." => Period::Pm,
        _ => return None,
    })
}

/// A clock written in the line: hours, minutes, and whether it was unambiguous (24-hour form or
/// a part of day given).
#[derive(Clone, Copy, Debug)]
struct Clock {
    hour: i32,
    minute: i32,
    fixed: bool,
}

/// "19.00", "19:00", "7pm", "7.30pm", or with `bare` a plain "7".
fn clock_token(word: &str, bare: bool) -> Option<(Clock, Option<Period>)> {
    let (body, ampm) = if let Some(body) = word.strip_suffix("pm") {
        (body, Some(Period::Pm))
    } else if let Some(body) = word.strip_suffix("am") {
        (body, Some(Period::Am))
    } else {
        (word, None)
    };
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ':') {
        return None;
    }
    let (hour, minute, separated) = match body.split_once(['.', ':']) {
        Some((h, m)) if m.len() == 2 => (h.parse().ok()?, m.parse().ok()?, true),
        Some(_) => return None,
        None if bare || ampm.is_some() => (body.parse().ok()?, 0, false),
        None => return None,
    };
    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) || (ampm.is_some() && !(1..=12).contains(&hour)) {
        return None;
    }
    let fixed = ampm.is_some() || (separated && (hour >= 13 || hour == 0 || body.starts_with('0')));
    Some((Clock { hour, minute, fixed }, ampm))
}

fn apply_period(clock: Clock, period: Option<Period>) -> Clock {
    let Some(period) = period else { return clock };
    let hour = clock.hour;
    let hour = match period {
        Period::Am => if hour == 12 { 0 } else { hour },
        Period::Pm => if hour < 12 { hour + 12 } else { hour },
        Period::Morning => if hour == 12 { 0 } else { hour },
        Period::Noon => if (1..=4).contains(&hour) { hour + 12 } else { hour },
        Period::Afternoon => if hour < 12 { hour + 12 } else { hour },
        Period::Night => match hour {
            6..=11 => hour + 12,
            12 => 0,
            _ => hour,
        },
    };
    Clock { hour, minute: clock.minute, fixed: true }
}

/// "2 jam", "45 menit", "1.5j", "90min", "2h".
fn duration_of(number: &str, unit: Option<&str>) -> Option<i32> {
    let (value, unit) = match unit {
        Some(unit) => (number, unit),
        None => {
            let split = number.find(|c: char| c.is_alphabetic())?;
            (&number[..split], &number[split..])
        }
    };
    let value: f64 = value.replace(',', ".").parse().ok()?;
    if value <= 0.0 || value > 24.0 * 60.0 {
        return None;
    }
    let minutes = match unit {
        "jam" | "j" | "hour" | "hours" | "hr" | "hrs" | "h" => value * 60.0,
        "menit" | "mnt" | "m" | "min" | "mins" | "minute" | "minutes" => value,
        _ => return None,
    };
    let minutes = minutes.round() as i32;
    (minutes > 0 && minutes <= 24 * 60).then_some(minutes)
}

struct Parse {
    date: Option<Date>,
    clock: Option<Clock>,
    end: Option<Clock>,
    length: Option<i32>,
    estimate: Option<i32>,
    from_now: Option<i32>,
    evening: bool,
    due: bool,
    priority: u8,
    project: Option<ProjectRef>,
    unknown_tag: Option<String>,
}

fn upcoming(today: Date, weekday: Weekday) -> Date {
    let ahead = (weekday.to_monday_zero_offset() - today.weekday().to_monday_zero_offset()).rem_euclid(7);
    today.checked_add(Span::new().days(i64::from(ahead))).unwrap_or(today)
}

fn add_days(date: Date, days: i64) -> Date {
    date.checked_add(Span::new().days(days)).unwrap_or(date)
}

fn month_of(word: &str) -> Option<i8> {
    MONTHS.iter().find(|(name, _)| *name == word).map(|(_, month)| *month)
}

/// A date with no year: this year, or next year when it has already passed.
fn day_month(today: Date, day: i8, month: i8, year: Option<i16>) -> Option<Date> {
    match year {
        Some(year) => Date::new(year, month, day).ok(),
        None => {
            let this = Date::new(today.year(), month, day).ok()?;
            if this < today { Date::new(today.year() + 1, month, day).ok() } else { Some(this) }
        }
    }
}

fn number(word: &str) -> Option<i32> {
    (!word.is_empty() && word.len() <= 4 && word.chars().all(|c| c.is_ascii_digit())).then(|| word.parse().ok()).flatten()
}

fn match_project(tag: &str, projects: &[ProjectRef]) -> Option<ProjectRef> {
    let key = alnum(tag);
    if key.is_empty() {
        return None;
    }
    projects
        .iter()
        .filter(|project| {
            let name = alnum(&project.name);
            name.contains(&key) || name.split_whitespace().any(|w| w.starts_with(&key))
        })
        .min_by_key(|project| project.name.len())
        .cloned()
}

fn parse_tokens(toks: &mut [Tok], ctx: &Context<'_>) -> Parse {
    let today = ctx.today;
    let mut p = Parse {
        date: None,
        clock: None,
        end: None,
        length: None,
        estimate: None,
        from_now: None,
        evening: false,
        due: false,
        priority: 0,
        project: None,
        unknown_tag: None,
    };
    let n = toks.len();
    let word = |toks: &[Tok], i: usize| -> String { toks.get(i).filter(|t| !t.used).map(|t| t.norm.clone()).unwrap_or_default() };
    let mut i = 0;
    while i < n {
        if toks[i].used {
            i += 1;
            continue;
        }
        let w0 = word(toks, i);
        let w1 = word(toks, i + 1);
        let w2 = word(toks, i + 2);
        let mut take = 0usize;

        // Priority and project.
        if let Some(rest) = w0.strip_prefix('!') {
            let level = match rest {
                "" => Some(1),
                "!" => Some(2),
                "!!" => Some(3),
                "1" | "rendah" | "low" => Some(1),
                "2" | "penting" | "tinggi" | "high" => Some(2),
                "3" | "mendesak" | "urgent" => Some(3),
                _ => None,
            };
            if let Some(level) = level {
                p.priority = p.priority.max(level);
                take = 1;
            }
        } else if let Some(tag) = w0.strip_prefix('#').filter(|tag| !tag.is_empty()) {
            match match_project(tag, ctx.projects) {
                Some(project) => p.project = Some(project),
                None => p.unknown_tag = Some(tag.to_string()),
            }
            take = 1;
        }
        // Deadline words.
        else if DEADLINE_WORDS.contains(&w0.as_str()) || (w0 == "paling" && w1 == "lambat") {
            p.due = true;
            take = if w0 == "paling" { 2 } else { 1 };
        }
        // Relative days.
        else if (w0 == "hari" && w1 == "ini") || w0 == "today" {
            p.date = Some(today);
            take = if w0 == "hari" { 2 } else { 1 };
        } else if matches!(w0.as_str(), "besok" | "bsk" | "besuk" | "tomorrow" | "tmr" | "tmrw") {
            p.date = Some(add_days(today, 1));
            take = 1;
        } else if w0 == "lusa" {
            p.date = Some(add_days(today, 2));
            take = 1;
        } else if matches!(w0.as_str(), "kemarin" | "kmrn" | "yesterday") {
            p.date = Some(add_days(today, -1));
            take = 1;
        } else if (w0 == "nanti" && w1 == "malam") || (w0 == "malam" && w1 == "ini") || w0 == "tonight" {
            p.date = Some(today);
            p.evening = true;
            take = if w0 == "tonight" { 1 } else { 2 };
        } else if ((w0 == "minggu" || w0 == "pekan") && w1 == "depan") || (w0 == "next" && w1 == "week") {
            p.date = Some(upcoming(add_days(today, 1), Weekday::Monday));
            take = 2;
        }
        // Weekdays: "jumat", "hari jumat", "jumat depan", "next friday", "on friday".
        else if let Some((offset, weekday)) = {
            let skip = usize::from(matches!(w0.as_str(), "hari" | "on" | "pada"));
            let (skip, next) = if word(toks, i + skip) == "next" { (skip + 1, true) } else { (skip, false) };
            let name = word(toks, i + skip);
            WEEKDAYS.iter().find(|(day_name, _)| *day_name == name).map(|(_, day)| ((skip, next), *day))
        } {
            let (skip, next_word) = offset;
            let after = word(toks, i + skip + 1);
            let next = next_word || after == "depan";
            let mut date = upcoming(today, weekday);
            if next {
                // "jumat depan": the one in next week.
                date = upcoming(add_days(today, 7 - i64::from(today.weekday().to_monday_zero_offset())), weekday);
            }
            p.date = Some(date);
            take = skip + 1 + usize::from(after == "depan" && !next_word);
        }
        // "tgl 25", "tanggal 25 okt".
        else if matches!(w0.as_str(), "tgl" | "tanggal") && number(&w1).is_some_and(|d| (1..=31).contains(&d)) {
            let day = number(&w1).unwrap_or(1) as i8;
            if let Some(month) = month_of(&w2) {
                p.date = day_month(today, day, month, None);
                take = 3;
            } else {
                let this = Date::new(today.year(), today.month(), day).ok().filter(|d| *d >= today);
                p.date = this.or_else(|| {
                    let next = add_days(Date::new(today.year(), today.month(), 1).ok()?, 32);
                    Date::new(next.year(), next.month(), day).ok()
                });
                take = 2;
            }
        }
        // "25 okt", "25 oktober 2026".
        else if let (Some(day), Some(month)) = (number(&w0).filter(|d| (1..=31).contains(d)), month_of(&w1)) {
            let year = number(&w2).filter(|y| (2000..=2100).contains(y)).map(|y| y as i16);
            p.date = day_month(today, day as i8, month, year);
            take = 2 + usize::from(year.is_some());
        }
        // "oct 25".
        else if let (Some(month), Some(day)) = (month_of(&w0), number(&w1).filter(|d| (1..=31).contains(d))) {
            p.date = day_month(today, day as i8, month, None);
            take = 2;
        }
        // "25/10", "25-10-2026".
        else if let Some(date) = {
            let parts: Vec<&str> = w0.split(['/', '-']).collect();
            if (parts.len() == 2 || parts.len() == 3) && parts.iter().all(|part| number(part).is_some()) && !w0.contains('.') {
                let day = number(parts[0]).unwrap_or(0);
                let month = number(parts[1]).unwrap_or(0);
                let year = parts.get(2).and_then(|y| number(y)).map(|y| if y < 100 { 2000 + y } else { y });
                // "25-10" alone reads like a range ("soal 1-3"): with a dash, a year is required.
                let dashed_pair = parts.len() == 2 && w0.contains('-');
                if (1..=31).contains(&day) && (1..=12).contains(&month) && !dashed_pair {
                    day_month(today, day as i8, month as i8, year.map(|y| y as i16))
                } else {
                    None
                }
            } else {
                None
            }
        } {
            p.date = Some(date);
            take = 1;
        }
        // Relative times: "dalam 30 menit", "30 menit lagi", "in 2 hours".
        else if matches!(w0.as_str(), "dalam" | "in") && duration_of(&w1, Some(&w2)).is_some() {
            p.from_now = duration_of(&w1, Some(&w2));
            take = 3;
        } else if let Some(minutes) = duration_of(&w0, Some(&w1)).filter(|_| w2 == "lagi") {
            p.from_now = Some(minutes);
            take = 3;
        }
        // Estimates: "~45m", "~1j".
        else if let Some(minutes) = w0.strip_prefix('~').and_then(|rest| duration_of(rest, None)) {
            p.estimate = Some(minutes);
            take = 1;
        }
        // Durations: "selama 2 jam", "2 jam", "90 menit", "for 2 hours".
        else if let Some((minutes, used)) = {
            let (skip, a, b) = if matches!(w0.as_str(), "selama" | "for") { (1, w1.as_str(), w2.as_str()) } else { (0, w0.as_str(), w1.as_str()) };
            duration_of(a, Some(b)).map(|m| (m, skip + 2)).or_else(|| if skip == 1 { duration_of(a, None).map(|m| (m, 2)) } else { None })
        } {
            p.length = Some(minutes);
            take = used;
        }
        // Times: "jam 7 malam", "pukul 19.00", "19.00-20.30", "7pm", "setengah 8", "jam 1-3 siang".
        else if let Some((clock, end, used)) = {
            let marked = TIME_MARKERS.contains(&w0.as_str());
            let start = usize::from(marked);
            let first = word(toks, i + start);
            let mut out = None;
            if first == "setengah" {
                if let Some(h) = number(&word(toks, i + start + 1)).filter(|h| (1..=12).contains(h)) {
                    out = Some((Clock { hour: h - 1, minute: 30, fixed: false }, None, None, start + 2));
                }
            } else if let Some((a, b)) = first.split_once(['-', '\u{2013}']).filter(|(a, b)| !a.is_empty() && !b.is_empty()) {
                if let (Some((s, sp)), Some((e, ep))) = (clock_token(a, marked), clock_token(b, marked)) {
                    out = Some((s, sp.or(ep), Some((e, ep)), start + 1));
                }
            } else if let Some((clock, ampm)) = clock_token(&first, marked) {
                out = Some((clock, ampm, None, start + 1));
            }
            out.map(|(clock, ampm, end, used)| {
                // A part of day after the clock: "7 malam", "7 pm".
                let next = word(toks, i + used);
                let (period, used) = match (ampm, period_of(&next)) {
                    (Some(p), _) => (Some(p), used),
                    (None, Some(p)) => (Some(p), used + 1),
                    (None, None) => (None, used),
                };
                let clock = apply_period(clock, period);
                let mut end = end.map(|(e, ep)| apply_period(e, ep.or(period)));
                let mut used = used;
                // "13.00 sampai 14.30", "7 - 9 malam".
                if end.is_none() && RANGE_WORDS.contains(&word(toks, i + used).as_str()) {
                    let after = word(toks, i + used + 1);
                    let after = if TIME_MARKERS.contains(&after.as_str()) { (1, word(toks, i + used + 2)) } else { (0, after) };
                    if let Some((e, ep)) = clock_token(&after.1, true) {
                        let tail = word(toks, i + used + 2 + after.0);
                        let (ep, extra) = match (ep, period_of(&tail)) {
                            (Some(p), _) => (Some(p), 0),
                            (None, Some(p)) => (Some(p), 1),
                            (None, None) => (period, 0),
                        };
                        end = Some(apply_period(e, ep));
                        used += 2 + after.0 + extra;
                    }
                }
                (clock, end, used)
            })
        } {
            p.clock = Some(clock);
            p.end = end;
            take = used;
        }

        if take == 0 {
            i += 1;
            continue;
        }
        for tok in toks.iter_mut().skip(i).take(take) {
            tok.used = true;
        }
        i += take;
    }
    p
}

/// The minute a written clock means on `date`, by the rules at the top of this file.
fn resolve(clock: Clock, evening: bool, date: Option<Date>, ctx: &Context<'_>) -> i32 {
    let hour = clock.hour;
    let today = date.is_none_or(|d| d == ctx.today);
    let passed = hour * 60 + clock.minute <= ctx.now_minute && (hour + 12) * 60 + clock.minute > ctx.now_minute;
    let afternoon = !clock.fixed && ((evening && hour < 12) || (1..=6).contains(&hour) || (today && hour < 12 && passed));
    (if afternoon { hour + 12 } else { hour }) * 60 + clock.minute
}

/// Read a line. Pure: the same line in the same context always reads the same.
pub fn read(text: &str, kind: CaptureKind, ctx: &Context<'_>) -> Reading {
    let mut toks: Vec<Tok> = text
        .split_whitespace()
        .map(|raw| Tok { raw: raw.to_string(), norm: norm(raw), used: false })
        .collect();
    if kind == CaptureKind::Reminder {
        for prefix in REMINDER_PREFIXES {
            if toks.len() >= prefix.len() && toks.iter().zip(prefix.iter()).all(|(tok, word)| tok.norm == *word) {
                for tok in toks.iter_mut().take(prefix.len()) {
                    tok.used = true;
                }
                break;
            }
        }
    }
    let p = parse_tokens(&mut toks, ctx);

    // An event's place: "di aula" after the words of its title.
    let mut location = None;
    if kind == CaptureKind::Event
        && let Some(at) = toks.iter().rposition(|t| !t.used && t.norm == "di")
    {
        let rest: Vec<String> = toks[at + 1..].iter().filter(|t| !t.used).map(|t| t.raw.clone()).collect();
        let before = toks[..at].iter().any(|t| !t.used);
        if !rest.is_empty() && before {
            location = Some(rest.join(" "));
            for tok in toks[at..].iter_mut() {
                tok.used = true;
            }
        }
    }

    let mut words: Vec<&Tok> = toks.iter().filter(|t| !t.used).collect();
    while words.last().is_some_and(|t| DANGLING.contains(&t.norm.as_str())) {
        words.pop();
    }
    while words.first().is_some_and(|t| matches!(t.norm.as_str(), "pada" | "on" | "-" | "&")) {
        words.remove(0);
    }
    let title = words.iter().map(|t| t.raw.as_str()).collect::<Vec<_>>().join(" ");
    let title = title.trim_matches(|c: char| c == ',' || c == '-' || c.is_whitespace()).to_string();

    // When.
    let mut date = p.date;
    let mut minute = p.clock.map(|clock| resolve(clock, p.evening, date, ctx));
    if let Some(from_now) = p.from_now {
        let at = ctx.now_minute + from_now;
        date = Some(add_days(ctx.today, i64::from(at.div_euclid(1440))));
        minute = Some(at.rem_euclid(1440));
    }
    if p.evening && minute.is_none() && kind != CaptureKind::Task {
        minute = Some(19 * 60);
    }
    if let (None, Some(m)) = (date, minute) {
        // A time with no day: today if it is still ahead, else tomorrow.
        date = Some(if m > ctx.now_minute { ctx.today } else { add_days(ctx.today, 1) });
    }
    if kind == CaptureKind::Note && date.is_none() {
        date = Some(ctx.today);
    }
    let mut end_minute = match (minute, p.end, p.length) {
        (Some(start), Some(end), _) => {
            let mut end = end.hour * 60 + end.minute;
            if !p.end.is_some_and(|e| e.fixed) && end <= start && end + 720 > start {
                end += 720;
            }
            Some(end)
        }
        (Some(start), None, Some(length)) => Some(start + length),
        _ => None,
    };
    if let (Some(start), Some(end)) = (minute, end_minute)
        && end <= start
    {
        end_minute = Some(end + 1440);
    }

    let missing = if title.is_empty() {
        Some(Missing::Title)
    } else {
        match kind {
            CaptureKind::Reminder if minute.is_none() => Some(Missing::Time),
            CaptureKind::Event if date.is_none() => Some(Missing::Date),
            _ => None,
        }
    };
    let as_clock = |m: i32| -> String {
        let m = m.rem_euclid(1440);
        clock_text(jiff::civil::Time::new((m / 60) as i8, (m % 60) as i8, 0, 0).unwrap_or(jiff::civil::Time::midnight()))
    };
    let task_estimate = if kind == CaptureKind::Task { p.estimate.or(p.length) } else { p.estimate };
    Reading {
        kind,
        title,
        date: date.map(date_text),
        time: minute.map(as_clock),
        end_time: if kind == CaptureKind::Event { end_minute.map(as_clock) } else { None },
        end_next_day: kind == CaptureKind::Event && end_minute.is_some_and(|e| e > 1440),
        due: kind == CaptureKind::Task && (p.due || minute.is_some()),
        priority: if kind == CaptureKind::Task { p.priority } else { 0 },
        project: p.project,
        unknown_tag: p.unknown_tag,
        estimate_min: task_estimate.map(|m| m as u32),
        location,
        missing,
    }
}

/// What was made.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Saved {
    pub kind: CaptureKind,
    pub id: String,
    pub title: String,
    pub date: Option<String>,
}

fn minutes(text: &str) -> i32 {
    let (h, m) = text.split_once(':').unwrap_or(("0", "0"));
    h.parse::<i32>().unwrap_or(0) * 60 + m.parse::<i32>().unwrap_or(0)
}

/// Read the line again (on the core's own clock) and save what it reads.
pub fn save(
    conn: &mut Connection,
    text: &str,
    kind: CaptureKind,
    now: Timestamp,
    tz: &TimeZone,
    actor: Actor,
) -> Result<(Saved, Change)> {
    let day = Day::containing(now, tz)?;
    let projects: Vec<ProjectRef> = {
        let mut statement = conn.prepare(
            "SELECT id, name, pigment FROM projects WHERE deleted_at IS NULL AND status IN ('active', 'paused') ORDER BY order_key",
        )?;
        statement
            .query_map([], |row| Ok(ProjectRef { id: row.get(0)?, name: row.get(1)?, pigment: row.get(2)? }))?
            .collect::<rusqlite::Result<_>>()?
    };
    let ctx = Context { today: day.date, now_minute: day.minute(now), projects: &projects };
    let reading = read(text, kind, &ctx);
    match reading.missing {
        Some(Missing::Title) => return invalid("title", Reason::Length),
        Some(Missing::Time) => return invalid("time", Reason::Format),
        Some(Missing::Date) => return invalid("date", Reason::Format),
        None => {}
    }
    let (id, change) = match kind {
        CaptureKind::Task => {
            let (do_date, due_date) = if reading.due { (None, reading.date.clone()) } else { (reading.date.clone(), None) };
            let input = TaskInput {
                title: reading.title.clone(),
                priority: reading.priority,
                do_date,
                due_date,
                due_time: if reading.due { reading.time.clone() } else { None },
                estimate_min: reading.estimate_min,
                project_id: reading.project.as_ref().map(|p| p.id.clone()),
                ..Default::default()
            };
            tasks::create(conn, input, actor, now)?
        }
        CaptureKind::Reminder => {
            let input = ReminderInput {
                text: reading.title.clone(),
                date: reading.date.clone().unwrap_or_default(),
                minute: reading.time.as_deref().map(minutes).unwrap_or(0),
                task_id: None,
            };
            reminders::create_at(conn, input, tz, actor, now)?
        }
        CaptureKind::Event => {
            let start = reading.time.as_deref().map(minutes);
            let input = match start {
                Some(start) => {
                    let mut end = reading.end_time.as_deref().map(minutes).unwrap_or(start + 60);
                    if end <= start {
                        end += 1440;
                    }
                    EventInput {
                        title: reading.title.clone(),
                        location: reading.location.clone().unwrap_or_default(),
                        date: reading.date.clone().unwrap_or_default(),
                        start_minute: start,
                        end_minute: end,
                        project_id: reading.project.as_ref().map(|p| p.id.clone()),
                        pigment: reading.project.as_ref().map(|p| p.pigment).unwrap_or_default(),
                        ..Default::default()
                    }
                }
                None => EventInput {
                    title: reading.title.clone(),
                    location: reading.location.clone().unwrap_or_default(),
                    all_day: true,
                    date: reading.date.clone().unwrap_or_default(),
                    project_id: reading.project.as_ref().map(|p| p.id.clone()),
                    pigment: reading.project.as_ref().map(|p| p.pigment).unwrap_or_default(),
                    ..Default::default()
                },
            };
            events::create(conn, input, tz, actor, now)?
        }
        CaptureKind::Note => {
            let date = reading.date.clone().unwrap_or_else(|| date_text(day.date));
            let (id, _) = journal::create(conn, &date, &reading.title, JournalKind::Note, actor, now)?;
            (id.clone(), Change::one(Entity::Journal, id))
        }
    };
    Ok((Saved { kind, id, title: reading.title, date: reading.date }, change))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Pigment;

    fn projects() -> Vec<ProjectRef> {
        vec![
            ProjectRef { id: "p1".into(), name: "Laporan Lab Jaringan".into(), pigment: Pigment::Toska },
            ProjectRef { id: "p2".into(), name: "Hub".into(), pigment: Pigment::Kunyit },
            ProjectRef { id: "p3".into(), name: "Belajar Cyber".into(), pigment: Pigment::Lavender },
        ]
    }

    /// Thursday 22 October 2026, 10.00.
    fn at(minute: i32, text: &str, kind: CaptureKind) -> Reading {
        let projects = projects();
        let ctx = Context { today: "2026-10-22".parse().unwrap(), now_minute: minute, projects: &projects };
        read(text, kind, &ctx)
    }

    fn task(text: &str) -> Reading {
        at(600, text, CaptureKind::Task)
    }

    #[test]
    fn a_task_with_a_day_gets_a_do_date() {
        let r = task("belajar subnetting besok");
        assert_eq!((r.title.as_str(), r.date.as_deref(), r.due, r.time.as_deref()), ("belajar subnetting", Some("2026-10-23"), false, None));
        assert_eq!(task("revisi slide lusa").date.as_deref(), Some("2026-10-24"));
        assert_eq!(task("Laporan hari ini").date.as_deref(), Some("2026-10-22"));
        assert_eq!(task("ide liar").date, None);
    }

    #[test]
    fn a_task_with_a_time_or_a_deadline_word_gets_a_deadline() {
        let r = task("laporan lab besok 19.00 #jaringan !!");
        assert_eq!(r.title, "laporan lab");
        assert_eq!((r.date.as_deref(), r.time.as_deref(), r.due), (Some("2026-10-23"), Some("19:00"), true));
        assert_eq!(r.project.as_ref().map(|p| p.id.as_str()), Some("p1"));
        assert_eq!(r.priority, 2);
        let r = task("makalah PKN tenggat jumat");
        assert_eq!((r.title.as_str(), r.date.as_deref(), r.due), ("makalah PKN", Some("2026-10-23"), true));
        let r = task("essay by monday !");
        assert_eq!((r.title.as_str(), r.date.as_deref(), r.due, r.priority), ("essay", Some("2026-10-26"), true, 1));
    }

    #[test]
    fn weekdays_count_from_today() {
        assert_eq!(task("x kamis").date.as_deref(), Some("2026-10-22"), "today is Thursday");
        assert_eq!(task("x jumat").date.as_deref(), Some("2026-10-23"));
        assert_eq!(task("x senin").date.as_deref(), Some("2026-10-26"));
        assert_eq!(task("x hari rabu").date.as_deref(), Some("2026-10-28"));
        assert_eq!(task("x jumat depan").date.as_deref(), Some("2026-10-30"));
        assert_eq!(task("x next friday").date.as_deref(), Some("2026-10-30"));
        assert_eq!(task("x minggu depan").date.as_deref(), Some("2026-10-26"), "next week starts Monday");
        assert_eq!(task("x minggu").date.as_deref(), Some("2026-10-25"), "minggu alone is Sunday");
    }

    #[test]
    fn written_dates() {
        assert_eq!(task("x 25 okt").date.as_deref(), Some("2026-10-25"));
        assert_eq!(task("x 5 januari").date.as_deref(), Some("2027-01-05"), "already passed this year");
        assert_eq!(task("x oct 30").date.as_deref(), Some("2026-10-30"));
        assert_eq!(task("x 31/10").date.as_deref(), Some("2026-10-31"));
        assert_eq!(task("x 3-11-2026").date.as_deref(), Some("2026-11-03"));
        assert_eq!(task("x tgl 25").date.as_deref(), Some("2026-10-25"));
        assert_eq!(task("x tanggal 5").date.as_deref(), Some("2026-11-05"), "the 5th has passed this month");
        assert_eq!(task("x 12 desember 2027").date.as_deref(), Some("2027-12-12"));
    }

    #[test]
    fn clocks_and_parts_of_day() {
        let time = |text: &str| at(600, text, CaptureKind::Reminder).time;
        assert_eq!(time("x besok jam 7 malam").as_deref(), Some("19:00"));
        assert_eq!(time("x besok jam 7 pagi").as_deref(), Some("07:00"));
        assert_eq!(time("x besok jam 1 siang").as_deref(), Some("13:00"));
        assert_eq!(time("x besok jam 4 sore").as_deref(), Some("16:00"));
        assert_eq!(time("x besok pukul 20.00").as_deref(), Some("20:00"));
        assert_eq!(time("x besok 07:30").as_deref(), Some("07:30"));
        assert_eq!(time("x besok setengah 8 malam").as_deref(), Some("19:30"));
        assert_eq!(time("x tomorrow 7pm").as_deref(), Some("19:00"));
        assert_eq!(time("x tomorrow at 7.30am").as_deref(), Some("07:30"));
        assert_eq!(time("x besok jam 3").as_deref(), Some("15:00"), "1 to 6 is afternoon");
        assert_eq!(time("x besok jam 8").as_deref(), Some("08:00"));
        assert_eq!(time("x jam 8").as_deref(), Some("20:00"), "today, 08.00 has passed");
        assert_eq!(time("x jam 12 malam").as_deref(), Some("00:00"));
        assert_eq!(time("x nanti malam").as_deref(), Some("19:00"));
        assert_eq!(time("x nanti malam jam 9").as_deref(), Some("21:00"));
    }

    #[test]
    fn a_time_without_a_day_is_the_next_one() {
        let r = at(600, "kumpulkan tugas jam 7 malam", CaptureKind::Reminder);
        assert_eq!((r.date.as_deref(), r.time.as_deref()), (Some("2026-10-22"), Some("19:00")));
        let r = at(20 * 60, "minum obat 07.00", CaptureKind::Reminder);
        assert_eq!((r.date.as_deref(), r.time.as_deref()), (Some("2026-10-23"), Some("07:00")));
    }

    #[test]
    fn reminders_drop_their_lead_in_and_need_a_time() {
        let r = at(600, "ingatkan aku untuk kumpulkan laporan besok jam 8 malam", CaptureKind::Reminder);
        assert_eq!((r.title.as_str(), r.date.as_deref(), r.time.as_deref(), r.missing), ("kumpulkan laporan", Some("2026-10-23"), Some("20:00"), None));
        let r = at(600, "remind me to call mom in 30 minutes", CaptureKind::Reminder);
        assert_eq!((r.title.as_str(), r.time.as_deref()), ("call mom", Some("10:30")));
        let r = at(1430, "tidur 30 menit lagi", CaptureKind::Reminder);
        assert_eq!((r.date.as_deref(), r.time.as_deref()), (Some("2026-10-23"), Some("00:20")), "past midnight");
        let r = at(600, "beli kabel besok", CaptureKind::Reminder);
        assert_eq!(r.missing, Some(Missing::Time));
    }

    #[test]
    fn events_with_ranges_places_and_all_day() {
        let r = at(600, "rapat OSIS jumat 13.00-14.30 di aula", CaptureKind::Event);
        assert_eq!(r.title, "rapat OSIS");
        assert_eq!((r.date.as_deref(), r.time.as_deref(), r.end_time.as_deref()), (Some("2026-10-23"), Some("13:00"), Some("14:30")));
        assert_eq!(r.location.as_deref(), Some("aula"));
        let r = at(600, "les gitar sabtu jam 1-3 siang", CaptureKind::Event);
        assert_eq!((r.time.as_deref(), r.end_time.as_deref()), (Some("13:00"), Some("15:00")));
        let r = at(600, "nonton film besok 19.00 sampai 21.30", CaptureKind::Event);
        assert_eq!(r.end_time.as_deref(), Some("21:30"));
        let r = at(600, "belajar CTF besok jam 9 malam selama 3 jam", CaptureKind::Event);
        assert_eq!((r.time.as_deref(), r.end_time.as_deref(), r.end_next_day), (Some("21:00"), Some("00:00"), false));
        let r = at(600, "begadang jumat 23.00-01.00", CaptureKind::Event);
        assert!(r.end_next_day);
        let r = at(600, "ujian tengah semester senin", CaptureKind::Event);
        assert_eq!((r.date.as_deref(), r.time, r.missing), (Some("2026-10-26"), None, None));
        assert_eq!(at(600, "rapat", CaptureKind::Event).missing, Some(Missing::Date));
    }

    #[test]
    fn numbers_that_are_not_times_stay_in_the_title() {
        assert_eq!(task("kerjakan soal 1-3 bab 2").title, "kerjakan soal 1-3 bab 2");
        assert_eq!(task("baca 2 bab").title, "baca 2 bab");
        assert_eq!(task("THM room 7").title, "THM room 7");
    }

    #[test]
    fn tags_priorities_and_estimates() {
        let r = task("rapikan repo #hub ~45m !penting");
        assert_eq!((r.title.as_str(), r.project.as_ref().map(|p| p.name.as_str()), r.estimate_min, r.priority), ("rapikan repo", Some("Hub"), Some(45), 2));
        let r = task("baca writeup #cyber");
        assert_eq!(r.project.as_ref().map(|p| p.id.as_str()), Some("p3"));
        let r = task("x #tidakada");
        assert_eq!((r.project, r.unknown_tag.as_deref()), (None, Some("tidakada")));
        assert_eq!(task("belajar 2 jam besok").estimate_min, Some(120));
    }

    #[test]
    fn notes_default_to_today_and_take_yesterday() {
        let r = at(600, "aku sudah membersihkan kamar", CaptureKind::Note);
        assert_eq!((r.title.as_str(), r.date.as_deref()), ("aku sudah membersihkan kamar", Some("2026-10-22")));
        assert_eq!(at(600, "lari pagi kemarin", CaptureKind::Note).date.as_deref(), Some("2026-10-21"));
    }

    #[test]
    fn an_empty_title_cannot_be_saved() {
        assert_eq!(task("besok jam 7").missing, Some(Missing::Title));
        assert_eq!(task("").missing, Some(Missing::Title));
    }

    #[test]
    fn saving_makes_what_was_read() {
        use crate::db::open_in_memory;
        let mut conn = open_in_memory().unwrap();
        let tz = TimeZone::get("Asia/Jakarta").unwrap();
        let now: Timestamp = "2026-10-22T03:00:00Z".parse().unwrap(); // 10.00 WIB
        crate::projects::create(&mut conn, crate::projects::ProjectInput { name: "Laporan Lab Jaringan".into(), ..Default::default() }, Actor::Owner, now).unwrap();
        let (saved, _) = save(&mut conn, "laporan lab besok 19.00 #jaringan !!", CaptureKind::Task, now, &tz, Actor::Owner).unwrap();
        let detail = tasks::detail(&conn, &saved.id, "2026-10-22").unwrap();
        assert_eq!((detail.card.due_date.as_deref(), detail.card.due_time.as_deref(), detail.card.priority), (Some("2026-10-23"), Some("19:00"), 2));
        assert_eq!(detail.card.project.unwrap().name, "Laporan Lab Jaringan");

        save(&mut conn, "ingatkan aku minum obat jam 8 malam", CaptureKind::Reminder, now, &tz, Actor::Owner).unwrap();
        assert_eq!(reminders::next_due(&conn).unwrap(), Some("2026-10-22T13:00:00Z".parse().unwrap()));

        let (event, _) = save(&mut conn, "rapat OSIS jumat 13.00-14.30 di aula", CaptureKind::Event, now, &tz, Actor::Owner).unwrap();
        let (start, location): (String, String) = conn
            .query_row("SELECT start_at, location FROM events WHERE id = ?1", [&event.id], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap();
        assert_eq!((start.as_str(), location.as_str()), ("2026-10-23T06:00:00Z", "aula"));

        save(&mut conn, "aku sudah membersihkan kamar", CaptureKind::Note, now, &tz, Actor::Owner).unwrap();
        assert_eq!(journal::on(&conn, "2026-10-22").unwrap().len(), 1);

        assert!(save(&mut conn, "beli kabel besok", CaptureKind::Reminder, now, &tz, Actor::Owner).is_err());
    }
}
