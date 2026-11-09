//! Work Hub does by itself in the background: the daily backup, and the slow pass that checks the
//! Library's files are still where Hub saw them (moved ones are followed, lost ones marked).

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use hub_core::{backup, settings, time};
use jiff::Timestamp;

use tauri::AppHandle;

use crate::{library, log, AppState};

/// Wait a little after start, so the backup never competes with opening the window.
const FIRST_DELAY: Duration = Duration::from_secs(30);
const EVERY: Duration = Duration::from_secs(60 * 60);

pub fn start(app: AppHandle, state: Arc<AppState>) {
    let spawned = thread::Builder::new()
        .name("hub-maintenance".into())
        .spawn(move || {
            thread::sleep(FIRST_DELAY);
            loop {
                if let Err(error) = backup_if_due(&state) {
                    log::error(state.hub.paths(), &format!("backup: {error}"));
                }
                library::check_some(&app, &state);
                thread::sleep(EVERY);
            }
        });
    if let Err(error) = spawned {
        eprintln!("[hub] maintenance thread: {error}");
    }
}

fn backup_if_due(state: &AppState) -> hub_core::Result<()> {
    let backups = state.hub.paths().backups.clone();
    state.hub.read(|conn| {
        let zone = time::zone(settings::load(conn)?.time_zone.as_deref());
        let today = Timestamp::now().to_zoned(zone).date();
        backup::run_if_due(conn, &backups, today)?;
        Ok(())
    })
}
