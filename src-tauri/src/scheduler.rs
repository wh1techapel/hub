//! The reminder clock: fires what came due, and nudges Tutup Hari once in the evening.
//!
//! One thread waits until the next reminder, the nudge time, or at most 30 seconds, whichever is
//! first, then compares against the wall clock. Waking every 30 seconds keeps it right after
//! sleep, resume, or a clock change without listening for any of them. A write that touches
//! reminders or settings pokes it to look again at once.

use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use hub_core::context::Ctx;
use hub_core::model::{Change, Entity};
use hub_core::{dayclose, reminders};
use jiff::{Span, Timestamp};
use tauri::AppHandle;

use crate::{commands, log, notify, AppState};

const MAX_WAIT: Duration = Duration::from_secs(30);
/// Settings key: the date the nudge was last shown, so a restart never shows it twice.
const NUDGED_KEY: &str = "close_nudged_on";

#[derive(Default)]
pub struct Scheduler {
    poked: Mutex<bool>,
    wake: Condvar,
}

impl Scheduler {
    /// Look again now (reminders or settings changed).
    pub fn poke(&self) {
        if let Ok(mut poked) = self.poked.lock() {
            *poked = true;
            self.wake.notify_one();
        }
    }

    fn wait(&self, timeout: Duration) {
        let Ok(poked) = self.poked.lock() else { return };
        if let Ok((mut poked, _)) = self.wake.wait_timeout_while(poked, timeout, |poked| !*poked) {
            *poked = false;
        }
    }
}

pub fn start(app: AppHandle, state: Arc<AppState>) {
    let spawned = thread::Builder::new().name("hub-scheduler".into()).spawn(move || loop {
        let wait = match tick(&app, &state) {
            Ok(wait) => wait,
            Err(error) => {
                log::error(state.hub.paths(), &format!("scheduler: {error}"));
                MAX_WAIT
            }
        };
        state.scheduler.wait(wait);
    });
    if let Err(error) = spawned {
        eprintln!("[hub] scheduler thread: {error}");
    }
}

/// One look at the clock. Returns how long to wait before the next.
fn tick(app: &AppHandle, state: &AppState) -> hub_core::Result<Duration> {
    let now = Timestamp::now();
    let fired = state.hub.write(|conn| reminders::fire_due(conn, now))?;
    if !fired.is_empty() {
        announce_fired(state, &fired);
        // ULTRON, when listening, says them aloud (Hub's notification shows either way).
        for reminder in &fired {
            state.events.publish(hub_api::ApiEvent::from(reminder));
        }
        commands::announce(app, &Change::all(Entity::Reminder));
    }

    let (next_due, nudge_at) = state.hub.write(|conn| {
        let ctx = Ctx::load(conn, now)?;
        let nudge_at = nudge(conn, &ctx)?;
        Ok((reminders::next_due(conn)?, nudge_at))
    })?;

    let mut wait = MAX_WAIT;
    for at in [next_due, nudge_at].into_iter().flatten() {
        let seconds = (at.as_second() - now.as_second()).max(0) as u64;
        wait = wait.min(Duration::from_secs(seconds));
    }
    Ok(wait.max(Duration::from_millis(250)))
}

fn announce_fired(state: &AppState, fired: &[reminders::Fired]) {
    let (late, on_time): (Vec<_>, Vec<_>) = fired.iter().partition(|f| f.late);
    for reminder in on_time {
        let when = state
            .hub
            .read(|conn| Ok(Ctx::load(conn, Timestamp::now())?.tz))
            .ok()
            .and_then(|tz| reminder.due_at.parse::<Timestamp>().ok().map(|t| t.to_zoned(tz)))
            .map(|z| format!("{:02}.{:02}", z.hour(), z.minute()))
            .unwrap_or_default();
        notify::show(notify::reminder_xml(&reminder.id, &reminder.text, &when));
    }
    match late.as_slice() {
        [] => {}
        [one] => notify::show(notify::reminder_xml(&one.id, &one.text, "terlewat")),
        many => {
            let names: Vec<&str> = many.iter().take(3).map(|f| f.text.as_str()).collect();
            let more = many.len().saturating_sub(3);
            let body = if more > 0 { format!("{}, dan {more} lagi.", names.join(", ")) } else { format!("{}.", names.join(", ")) };
            notify::show(notify::plain_xml("Selama kamu pergi", &body, "today"));
        }
    }
}

/// Show the Tutup Hari nudge if it is time and the day is still open. Returns when it is due
/// later today (for the wait), or None.
fn nudge(conn: &mut rusqlite::Connection, ctx: &Ctx) -> hub_core::Result<Option<Timestamp>> {
    if !ctx.settings.close_nudge {
        return Ok(None);
    }
    let Ok(at) = hub_core::time::parse_clock("closeNudgeTime", &ctx.settings.close_nudge_time) else {
        return Ok(None);
    };
    let at_minute = hub_core::time::minutes_of(at);
    let shown_on: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [NUDGED_KEY], |row| row.get::<_, String>(0))
        .ok()
        .and_then(|value| serde_json::from_str(&value).ok());
    if shown_on.as_deref() == Some(ctx.date.as_str()) || dayclose::is_closed(conn, &ctx.date)? {
        return Ok(None);
    }
    if ctx.minute() < at_minute {
        return Ok(Some(ctx.day.start.checked_add(Span::new().minutes(i64::from(at_minute)))?));
    }
    // Not after sleep time: a nudge at 02.00 for a day that is long over helps no one.
    if ctx.minute() >= ctx.waking.sleep.max(at_minute + 60) {
        return Ok(None);
    }
    conn.execute(
        "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        rusqlite::params![NUDGED_KEY, serde_json::json!(ctx.date).to_string(), hub_core::time::instant(ctx.now)],
    )?;
    notify::show(notify::plain_xml("Tutup Hari", "Dua menit untuk menutup hari dan menyiapkan besok.", "review"));
    Ok(None)
}
