//! Hub's windows: the main window, the small Catat Cepat window of Ctrl+Alt+H, and Lentera's
//! capsule and bar of Ctrl+Alt+L (lentera.rs).
//!
//! All are built hidden and shown by their page once it is drawn in the right light. Closing the
//! main window hides it; the capture window hides when it loses focus or is done; Lentera's page
//! decides when it folds into its capsule or hides. Measured in CP2
//! (docs/architecture.md, "Resources"): a window holds about 100 MB in WebView2's processes
//! whether shown or hidden, Hub alone in the tray about 4 MB, so a window hidden for
//! `RELEASE_AFTER` is destroyed and built again on demand. Everything it shows lives in the
//! database, so nothing is lost.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Window, WindowEvent};

use crate::state;

pub const MAIN: &str = "main";
pub const CAPTURE: &str = "capture";

/// Asks the main window's page to go to a route (a notification or tray action).
pub const NAVIGATE_EVENT: &str = "hub://navigate";

/// If the page never says it is ready (a crash in the UI), show the window anyway after this.
const READY_FALLBACK: Duration = Duration::from_millis(2500);
/// How long a hidden window is kept for a quick return before its memory is given back.
const RELEASE_AFTER: Duration = Duration::from_secs(10 * 60);

/// Changes whenever a window is shown or hidden, so a pending release or fallback only acts on
/// the state it was scheduled for.
fn generations() -> &'static Mutex<HashMap<String, u64>> {
    static GENERATIONS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    GENERATIONS.get_or_init(Default::default)
}

fn bump(label: &str) -> u64 {
    let Ok(mut map) = generations().lock() else { return 0 };
    let entry = map.entry(label.to_string()).or_insert(0);
    *entry += 1;
    *entry
}

fn generation(label: &str) -> u64 {
    generations().lock().ok().and_then(|map| map.get(label).copied()).unwrap_or(0)
}

/// `HUB_RELEASE_AFTER_SECS` shortens the wait for measurements (scripts/measure.ps1).
fn release_after() -> Duration {
    std::env::var("HUB_RELEASE_AFTER_SECS")
        .ok()
        .and_then(|secs| secs.parse().ok())
        .map(Duration::from_secs)
        .unwrap_or(RELEASE_AFTER)
}

/// Bring the main window forward, building it if it does not exist.
pub fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.unminimize();
        reveal(&window);
        return;
    }
    if let Err(error) = build_main(app) {
        crate::log::error(state(app).hub.paths(), &format!("window: {error}"));
    }
}

/// Show the main window on a page ("/", "/?review=1").
pub fn show_at(app: &AppHandle, path: &str) {
    show(app);
    let _ = app.emit_to(MAIN, NAVIGATE_EVENT, path);
    // A window still being built picks the path up from its URL instead.
    if let Ok(mut pending) = pending_path().lock() {
        *pending = Some(path.to_string());
    }
}

fn pending_path() -> &'static Mutex<Option<String>> {
    static PENDING: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    PENDING.get_or_init(Default::default)
}

/// The path a new main window should open on, once.
pub fn take_pending_path() -> Option<String> {
    pending_path().lock().ok().and_then(|mut pending| pending.take())
}

pub(crate) fn builder<'a>(app: &'a AppHandle, label: &'a str, page: &str) -> WebviewWindowBuilder<'a, tauri::Wry, AppHandle> {
    let data = state(app).hub.paths().root.join("webview");
    WebviewWindowBuilder::new(app, label, WebviewUrl::App(page.into()))
        .visible(false)
        // WebView2's own profile sits with the rest of Hub's data (and follows HUB_DATA_DIR).
        .data_directory(data)
}

fn build_main(app: &AppHandle) -> tauri::Result<()> {
    let window = builder(app, MAIN, "index.html")
        .title("Hub")
        .inner_size(1280.0, 820.0)
        .min_inner_size(960.0, 640.0)
        .center()
        .build()?;
    fallback_reveal(window);
    Ok(())
}

pub(crate) fn fallback_reveal(window: WebviewWindow) {
    let label = window.label().to_string();
    let at = generation(&label);
    thread::spawn(move || {
        thread::sleep(READY_FALLBACK);
        if generation(&label) == at && !window.is_visible().unwrap_or(true) {
            reveal(&window);
        }
    });
}

/// Ctrl+Alt+H: show the capture window, or hide it if it is in front.
pub fn toggle_capture(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(CAPTURE) {
        if window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false) {
            hide(&window);
        } else {
            let _ = window.center();
            reveal(&window);
            let _ = window.emit("hub://capture-shown", ());
        }
        return;
    }
    let built = builder(app, CAPTURE, "capture.html")
        .title("Catat Cepat")
        .inner_size(620.0, 236.0)
        .resizable(false)
        .decorations(false)
        .shadow(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .center()
        .build();
    match built {
        Ok(window) => fallback_reveal(window),
        Err(error) => crate::log::error(state(app).hub.paths(), &format!("capture window: {error}")),
    }
}

pub fn reveal(window: &WebviewWindow) {
    bump(window.label());
    let _ = window.show();
    let _ = window.set_focus();
    if window.label() == crate::lentera::LENTERA {
        crate::lentera::shown(window);
    }
}

/// Show a window without taking the keyboard from whatever has it: Lentera's capsule appearing
/// for a Sesi Fokus must never pull the owner out of what they are typing, nor close a menu they
/// have open. Tauri cannot show a window without activating it, so this goes to Windows directly:
/// shown without activation, and not activated by a click either (WS_EX_NOACTIVATE), so the
/// capsule's buttons work without taking the keyboard. Tauri still thinks the window hidden, so
/// nothing may change its window flags while it is shown this way (the click-through of
/// lentera.rs is set directly too); `reveal` (the bar) shows it through Tauri again, which also
/// drops WS_EX_NOACTIVATE.
pub fn show_quietly(window: &WebviewWindow) {
    bump(window.label());
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, ShowWindow, GWL_EXSTYLE, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
            SW_SHOWNOACTIVATE, WS_EX_NOACTIVATE,
        };
        if let Ok(hwnd) = window.hwnd() {
            let hwnd = hwnd.0 as windows_sys::Win32::Foundation::HWND;
            // SAFETY: a live window handle from Tauri; these calls read and set its styles and
            // show it, and take ownership of nothing.
            unsafe {
                let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_NOACTIVATE as isize);
                ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = window.show();
    }
    if window.label() == crate::lentera::LENTERA {
        crate::lentera::shown(window);
    }
}

/// Hidden for Tauri and for Windows alike: a window shown by `show_quietly` is visible only as far
/// as Windows knows, and Tauri, thinking it hidden already, would do nothing.
pub fn conceal(window: &WebviewWindow) {
    let _ = window.hide();
    #[cfg(windows)]
    if let Ok(hwnd) = window.hwnd() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
        // SAFETY: a live window handle from Tauri; hiding it takes ownership of nothing.
        unsafe {
            ShowWindow(hwnd.0 as _, SW_HIDE);
        }
    }
}

/// Hide a window, and let it go if it stays hidden.
pub fn hide(window: &WebviewWindow) {
    conceal(window);
    schedule_release(window.app_handle().clone(), window.label().to_string());
}

fn schedule_release(app: AppHandle, label: String) {
    let at = bump(&label);
    let delay = release_after();
    thread::spawn(move || {
        thread::sleep(delay);
        if generation(&label) != at {
            return;
        }
        if let Some(window) = app.get_webview_window(&label)
            && !window.is_visible().unwrap_or(true)
        {
            let _ = window.destroy();
        }
    });
}

/// Closing the main window hides it (Hub keeps running in the tray). The capture window hides
/// when the owner clicks elsewhere. Lentera keeps its corner when the display scale changes.
pub fn on_event(window: &Window, event: &WindowEvent) {
    let label = window.label();
    match event {
        WindowEvent::CloseRequested { api, .. } if label == MAIN || label == CAPTURE || label == crate::lentera::LENTERA => {
            api.prevent_close();
            let _ = window.hide();
            schedule_release(window.app_handle().clone(), window.label().to_string());
        }
        WindowEvent::Focused(false) if window.label() == CAPTURE && window.is_visible().unwrap_or(false) => {
            let _ = window.hide();
            schedule_release(window.app_handle().clone(), window.label().to_string());
        }
        WindowEvent::Focused(false) if label == crate::lentera::LENTERA => crate::lentera::forget(),
        WindowEvent::ScaleFactorChanged { .. } if label == crate::lentera::LENTERA => {
            if let Some(lentera) = window.app_handle().get_webview_window(label) {
                crate::lentera::place(&lentera);
            }
        }
        WindowEvent::DragDrop(drag) => crate::library::on_drag(window, drag),
        _ => {}
    }
}

/// Title bar colours (Windows 11 22000 and later; earlier versions ignore them).
#[cfg(windows)]
pub fn paint_caption(window: &WebviewWindow, background: (u8, u8, u8), text: (u8, u8, u8)) {
    use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR};
    let Ok(hwnd) = window.hwnd() else { return };
    let colorref = |(r, g, b): (u8, u8, u8)| u32::from(r) | u32::from(g) << 8 | u32::from(b) << 16;
    let caption = colorref(background);
    let ink = colorref(text);
    for (attribute, value) in [(DWMWA_CAPTION_COLOR, caption), (DWMWA_BORDER_COLOR, caption), (DWMWA_TEXT_COLOR, ink)] {
        // SAFETY: a live window handle from Tauri, a 4-byte COLORREF and its size.
        unsafe {
            DwmSetWindowAttribute(hwnd.0 as _, attribute as u32, (&value as *const u32).cast(), std::mem::size_of::<u32>() as u32);
        }
    }
}

#[cfg(not(windows))]
pub fn paint_caption(_window: &WebviewWindow, _background: (u8, u8, u8), _text: (u8, u8, u8)) {}

/// A startup failure the owner must see: there is no window yet to show it in.
#[cfg(windows)]
pub fn fatal(message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let wide = |text: &str| text.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (text, title) = (wide(message), wide("Hub"));
    // SAFETY: both strings are NUL-terminated UTF-16 that outlive the call.
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR);
    }
}

#[cfg(not(windows))]
pub fn fatal(message: &str) {
    eprintln!("{message}");
}
