//! Sesi Fokus's commands (Lentera CP02). The rules are hub-core's (`focus.rs`); the scheduler moves
//! the phases on, and every change also tells Lentera's window whether its capsule is wanted
//! (`commands::announce`, `lentera::sync_session`).

use hub_core::focus::{self, FocusStart, FocusView, Lengths};
use jiff::Timestamp;
use tauri::AppHandle;

use crate::commands::{read, write_many, Hub, Result, OWNER};

/// How long a minute of Pengaturan lasts. A smoke of the real window (`HUB_DATA_DIR` set) may make
/// it a second with `HUB_FOCUS_SECONDS_PER_MINUTE=1`; the owner's Hub always counts real minutes.
pub fn seconds_per_minute() -> i64 {
    if !crate::uses_override() {
        return 60;
    }
    std::env::var("HUB_FOCUS_SECONDS_PER_MINUTE")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| (1..=60).contains(value))
        .unwrap_or(60)
}

#[tauri::command(async)]
pub fn get_focus(state: Hub<'_>) -> Result<Option<FocusView>> {
    read(&state, |conn, _| focus::view(conn))
}

#[tauri::command(async)]
pub fn start_focus(app: AppHandle, state: Hub<'_>, input: FocusStart) -> Result<FocusView> {
    write_many(&app, &state, |conn, ctx| {
        let lengths = Lengths::from_settings(&ctx.settings, seconds_per_minute());
        focus::start(conn, input, lengths, OWNER, Timestamp::now())
    })
}

#[tauri::command(async)]
pub fn pause_focus(app: AppHandle, state: Hub<'_>) -> Result<()> {
    write_many(&app, &state, |conn, _| Ok(((), focus::pause(conn, Timestamp::now())?)))
}

#[tauri::command(async)]
pub fn resume_focus(app: AppHandle, state: Hub<'_>) -> Result<()> {
    write_many(&app, &state, |conn, _| Ok(((), focus::resume(conn, Timestamp::now())?)))
}

/// Lanjut fokus after a break, or Lewati istirahat during one.
#[tauri::command(async)]
pub fn next_focus(app: AppHandle, state: Hub<'_>) -> Result<()> {
    write_many(&app, &state, |conn, _| Ok(((), focus::next(conn, Timestamp::now())?)))
}

#[tauri::command(async)]
pub fn stop_focus(app: AppHandle, state: Hub<'_>) -> Result<()> {
    write_many(&app, &state, |conn, _| Ok(((), focus::stop(conn, OWNER, Timestamp::now())?)))
}

/// Selipkan: the capsule steps away until the next phase (`false` brings it back).
#[tauri::command(async)]
pub fn tuck_focus(app: AppHandle, state: Hub<'_>, tucked: bool) -> Result<()> {
    write_many(&app, &state, |conn, _| Ok(((), focus::tuck(conn, tucked, Timestamp::now())?)))
}
