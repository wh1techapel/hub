//! The tray icon: the way back to Hub, Lentera's bar, and the only way to quit Hub.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

use crate::{lentera, window};

const OPEN: &str = "open";
const LENTERA: &str = "lentera";
const QUIT: &str = "quit";

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, OPEN, "Buka Hub", true, None::<&str>)?;
    let lentera = MenuItem::with_id(app, LENTERA, "Tanya Lentera", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT, "Keluar dari Hub", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &lentera, &separator, &quit])?;

    let mut tray = TrayIconBuilder::with_id("hub")
        .tooltip("Hub")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            OPEN => window::show(app),
            LENTERA => lentera::open(app),
            QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                window::show(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}
