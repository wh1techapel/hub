//! Lentera on the desktop (CP01): a capsule that grows into a bar, at the bottom right of the
//! primary screen, above the taskbar and beside the clock (ULTRON's island is top centre).
//!
//! The window is built the way ULTRON's island is (`../Ultron/docs/island-2.md`, CP-UI6): a fixed,
//! transparent rectangle with room for the bar at its tallest and its shadow. The capsule and the
//! bar are drawn inside it and change shape with one spring; the window itself never resizes,
//! because resizing a window in step with what its WebView draws is not possible on Windows and
//! jumps. Only the shape takes the mouse: a small thread watches the cursor while Lentera is shown
//! and makes the rest of the window click-through (WS_EX_TRANSPARENT, set directly), so the empty part
//! of the rectangle never covers what the owner is working on. The page reports its shape
//! (`set_lentera_shape`) whenever it changes.
//!
//! Lentera opens from Ctrl+Alt+L anywhere, the tray's "Tanya Lentera", and inside Hub (the sidebar,
//! Ctrl+L, the palette). The window that had the keyboard before is remembered, and gets it back
//! when the owner closes Lentera with Esc or saves a line, not when they clicked somewhere else
//! (that click already chose where to go). Hidden, the window is let go like the others
//! (window.rs, `RELEASE_AFTER`).

use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::{state, window};

pub const LENTERA: &str = "lentera";
/// Tells the page to open the bar with the keyboard in it (shown again, or called while showing
/// the capsule).
pub const OPEN_EVENT: &str = "hub://lentera-open";
/// Tells the page to close (the shortcut pressed while the bar has the keyboard).
pub const CLOSE_EVENT: &str = "hub://lentera-close";

/// Logical size of the window: the bar at its tallest (lentera.module.css, with a Sesi Fokus card
/// since CP02, a conversation with its cards since CP03) and its shadow. Only the shape takes the
/// mouse, so the taller window covers nothing more.
pub const WIDTH: f64 = 480.0;
pub const HEIGHT: f64 = 640.0;
/// How often the cursor is checked while Lentera is shown: entering the shape is noticed within it.
const POLL: Duration = Duration::from_millis(25);

/// A rectangle in physical pixels (a monitor's work area, the window's place).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// The window's place: its bottom-right corner on the work area's, so it sits above the taskbar
/// and beside the clock wherever the taskbar is. On a work area smaller than the window (a large
/// display scale on a small screen) the window takes what there is.
pub fn placement(work: Rect, scale: f64) -> Rect {
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let width = ((WIDTH * scale).round() as u32).min(work.width);
    let height = ((HEIGHT * scale).round() as u32).min(work.height);
    Rect {
        x: work.x + (work.width - width) as i32,
        y: work.y + (work.height - height) as i32,
        width,
        height,
    }
}

/// What takes the mouse: the capsule or the bar, in logical pixels from the window's top left, a
/// rounded rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Shape {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub radius: f64,
}

impl Shape {
    /// The shape as the page reported it, kept inside the window; nonsense (not a number, nothing
    /// left inside) is refused.
    pub fn checked(self) -> Option<Shape> {
        let values = [self.x, self.y, self.width, self.height, self.radius];
        if values.iter().any(|v| !v.is_finite()) {
            return None;
        }
        let x = self.x.clamp(0.0, WIDTH);
        let y = self.y.clamp(0.0, HEIGHT);
        let width = (self.x + self.width).clamp(0.0, WIDTH) - x;
        let height = (self.y + self.height).clamp(0.0, HEIGHT) - y;
        if width <= 0.0 || height <= 0.0 {
            return None;
        }
        let radius = self.radius.clamp(0.0, width.min(height) / 2.0);
        Some(Shape { x, y, width, height, radius })
    }

    /// The whole window: what takes the mouse before the page has said anything.
    fn whole() -> Shape {
        Shape { x: 0.0, y: 0.0, width: WIDTH, height: HEIGHT, radius: 0.0 }
    }
}

/// Whether the cursor (physical, screen) is on the shape of a window at `origin` (physical) drawn
/// at `scale`.
pub fn inside(cursor: (f64, f64), origin: (i32, i32), scale: f64, shape: Shape) -> bool {
    if !(scale.is_finite() && scale > 0.0) {
        return true;
    }
    let x = (cursor.0 - f64::from(origin.0)) / scale;
    let y = (cursor.1 - f64::from(origin.1)) / scale;
    if x < shape.x || y < shape.y || x > shape.x + shape.width || y > shape.y + shape.height {
        return false;
    }
    let r = shape.radius.clamp(0.0, shape.width.min(shape.height) / 2.0);
    // The nearest point of the rectangle shrunk by the radius: inside when within the radius of it.
    let cx = x.clamp(shape.x + r, shape.x + shape.width - r);
    let cy = y.clamp(shape.y + r, shape.y + shape.height - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r + f64::EPSILON
}

#[derive(Default)]
struct Memory {
    /// The page's shape, once reported.
    shape: Option<Shape>,
    /// The cursor watch is running.
    watching: bool,
    /// The window that had the keyboard before Lentera took it (a raw HWND).
    previous: Option<isize>,
    /// The window being built is for the bar (the owner called Lentera), not for a session's
    /// capsule: it takes the keyboard when shown, and the page opens the bar.
    pending_bar: bool,
    /// A window is being built (by the owner's call or for a session).
    building: bool,
    /// CP02: a Sesi Fokus runs, and whether the owner tucked its capsule away.
    session: bool,
    tucked: bool,
    /// The presence watch (fullscreen applications) is running.
    presence: bool,
}

fn memory() -> &'static Mutex<Memory> {
    static MEMORY: OnceLock<Mutex<Memory>> = OnceLock::new();
    MEMORY.get_or_init(Default::default)
}

fn shape() -> Shape {
    memory().lock().ok().and_then(|m| m.shape).unwrap_or_else(Shape::whole)
}

/// Ctrl+Alt+L: open the bar, or close it if it has the keyboard.
pub fn toggle(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LENTERA)
        && window.is_visible().unwrap_or(false)
        && has_keyboard(&window)
    {
        // The page closes itself (its exit, then hide_lentera with the keyboard given back).
        let _ = app.emit_to(LENTERA, CLOSE_EVENT, ());
        return;
    }
    open(app);
}

/// Open the bar with the keyboard in it, building the window if it does not exist.
pub fn open(app: &AppHandle) {
    remember_foreground(app);
    if let Some(window) = app.get_webview_window(LENTERA) {
        place(&window);
        window::reveal(&window);
        let _ = app.emit_to(LENTERA, OPEN_EVENT, ());
        return;
    }
    build(app, true);
}

/// Build the window, hidden: its page shows it once drawn (window_ready), taking the keyboard only
/// when it is built for the bar.
fn build(app: &AppHandle, for_bar: bool) {
    {
        let Ok(mut m) = memory().lock() else { return };
        if for_bar {
            m.pending_bar = true;
        }
        if m.building {
            return;
        }
        m.building = true;
    }
    let built = window::builder(app, LENTERA, "lentera.html")
        .title("Lentera")
        .inner_size(WIDTH, HEIGHT)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .transparent(true)
        // Windows draws a 1 px frame around an undecorated window with a shadow; the shadow is the
        // page's, around the shape, not around the whole rectangle.
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .build();
    if let Ok(mut m) = memory().lock() {
        m.building = false;
    }
    match built {
        // The page opens the bar once it is drawn (window_ready shows the window).
        Ok(window) => {
            place(&window);
            window::fallback_reveal(window);
        }
        Err(error) => crate::log::error(state(app).hub.paths(), &format!("lentera window: {error}")),
    }
}

/// Whether the window being shown is for the bar (see `Memory::pending_bar`).
pub fn bar_pending() -> bool {
    memory().lock().map(|m| m.pending_bar).unwrap_or(true)
}

// ---------- CP02: the capsule of a Sesi Fokus ----------

/// How often the capsule checks for a fullscreen application while a session runs.
const PRESENCE_POLL: Duration = Duration::from_secs(1);

/// A Sesi Fokus started, moved on, was tucked, or ended: the capsule follows. While a session runs
/// and its capsule is not tucked away, Lentera's window shows without taking the keyboard; the
/// page hides it once it has nothing left to show (a session ended or tucked, no bar, no note).
pub fn sync_session(app: &AppHandle) {
    let view = state(app).hub.read(hub_core::focus::view).ok().flatten();
    let (session, tucked) = view.map_or((false, false), |view| (true, view.tucked));
    let watch = {
        let Ok(mut m) = memory().lock() else { return };
        m.session = session;
        m.tucked = tucked;
        let watch = session && !m.presence;
        if watch {
            m.presence = true;
        }
        watch
    };
    // Off whatever thread announced the change: building a window waits on the main thread.
    let now = app.clone();
    thread::spawn(move || present(&now));
    if watch {
        let app = app.clone();
        thread::spawn(move || presence(app));
    }
}

/// While a session runs: the capsule steps aside for a fullscreen application and comes back
/// after it.
fn presence(app: AppHandle) {
    loop {
        thread::sleep(PRESENCE_POLL);
        {
            let Ok(mut m) = memory().lock() else { return };
            if !m.session {
                m.presence = false;
                return;
            }
        }
        present(&app);
    }
}

/// Show the capsule quietly if the session wants it; hide it while a fullscreen application is in
/// front (unless the owner is using Lentera).
fn present(app: &AppHandle) {
    let wanted = memory().lock().map(|m| m.session && !m.tucked).unwrap_or(false);
    if !wanted {
        return;
    }
    let fullscreen = fullscreen::in_front(app);
    match app.get_webview_window(LENTERA) {
        Some(window) => {
            let visible = window.is_visible().unwrap_or(false);
            if fullscreen && visible && !has_keyboard(&window) {
                window::conceal(&window);
            } else if !fullscreen && !visible {
                place(&window);
                window::show_quietly(&window);
            }
        }
        None if !fullscreen => build(app, false),
        None => {}
    }
}

/// Put the window at the bottom right of the primary screen's work area, at that screen's scale.
pub fn place(window: &WebviewWindow) {
    let Ok(Some(monitor)) = window.primary_monitor() else { return };
    let work = monitor.work_area();
    let area = Rect { x: work.position.x, y: work.position.y, width: work.size.width, height: work.size.height };
    let to = placement(area, monitor.scale_factor());
    let _ = window.set_position(PhysicalPosition::new(to.x, to.y));
    let _ = window.set_size(PhysicalSize::new(to.width, to.height));
}

/// Called once the window is shown: the cursor watch runs while it stays shown.
pub fn shown(window: &WebviewWindow) {
    let Ok(mut m) = memory().lock() else { return };
    if m.watching {
        return;
    }
    m.watching = true;
    drop(m);
    let app = window.app_handle().clone();
    thread::spawn(move || watch(app));
}

/// Make the window click-through wherever the shape is not, until the window is hidden or gone.
fn watch(app: AppHandle) {
    loop {
        thread::sleep(POLL);
        let window = app.get_webview_window(LENTERA);
        let visible = window.as_ref().is_some_and(|w| w.is_visible().unwrap_or(false));
        if !visible {
            // The lock is never held while asking the window anything: that question waits on the
            // main thread, which may itself be waiting for the lock (window_ready, a shape), and
            // Hub would stop answering.
            if let Ok(mut m) = memory().lock() {
                m.watching = false;
            }
            // Shown again meanwhile: `shown` saw the watch running and started none, so this one
            // goes on, unless another has started since.
            let again = app.get_webview_window(LENTERA).is_some_and(|w| w.is_visible().unwrap_or(false));
            if again {
                let Ok(mut m) = memory().lock() else { return };
                if m.watching {
                    return;
                }
                m.watching = true;
                continue;
            }
            if let Some(window) = window {
                click_through(&window, false);
            }
            return;
        }
        let Some(window) = window else { continue };
        let cursor = app.cursor_position().ok();
        let origin = window.outer_position().ok();
        let scale = window.scale_factor().ok();
        // When the cursor cannot be read, the window keeps taking clicks.
        let on_shape = match (cursor, origin, scale) {
            (Some(c), Some(o), Some(s)) => inside((c.x, c.y), (o.x, o.y), s, shape()),
            _ => true,
        };
        // Set every time it is not as wanted: showing the bar through Tauri rewrites the styles.
        click_through(&window, !on_shape);
    }
}

/// The window lets clicks through (or takes them again), the way Tauri's
/// `set_ignore_cursor_events` does (WS_EX_TRANSPARENT and WS_EX_LAYERED) but set directly: a Tauri
/// flag change would hide a capsule shown quietly (`window::show_quietly`). Nothing is written when
/// the window already is as wanted.
fn click_through(window: &WebviewWindow, ignore: bool) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_LAYERED, WS_EX_TRANSPARENT};
        let Ok(hwnd) = window.hwnd() else { return };
        let hwnd = hwnd.0 as windows_sys::Win32::Foundation::HWND;
        let bits = (WS_EX_TRANSPARENT | WS_EX_LAYERED) as isize;
        // SAFETY: a live window handle from Tauri; reading and writing its extended style.
        unsafe {
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let wanted = if ignore { ex | bits } else { ex & !bits };
            if wanted != ex {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted);
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = window.set_ignore_cursor_events(ignore);
    }
}

fn remember_foreground(app: &AppHandle) {
    let own = app.get_webview_window(LENTERA).and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize);
    let current = focus::foreground();
    if let (Some(current), Ok(mut m)) = (current, memory().lock())
        && Some(current) != own
    {
        m.previous = Some(current);
    }
}

/// Whether Lentera has the keyboard: Tauri's word, or Windows' (the WebView inside can hold the
/// focus while Tauri still reads the window as unfocused).
fn has_keyboard(window: &WebviewWindow) -> bool {
    let own = window.hwnd().ok().map(|h| h.0 as isize);
    window.is_focused().unwrap_or(false) || (own.is_some() && focus::foreground() == own)
}

/// Lentera lost the keyboard (a click elsewhere, or given back): where it came from no longer
/// matters, so a later Esc does not pull the owner back there.
pub fn forget() {
    if let Ok(mut m) = memory().lock() {
        m.previous = None;
    }
}

/// Give the keyboard back to the window that had it, if Lentera still has it.
fn give_back(window: &WebviewWindow) {
    let previous = memory().lock().ok().and_then(|mut m| m.previous.take());
    if let Some(previous) = previous
        && has_keyboard(window)
    {
        focus::bring(previous);
    }
}

// ---------- commands (the Lentera window, and open_lentera for the main window) ----------

/// Inside Hub: the sidebar's Lentera, Ctrl+L, and the palette's "Tanya Lentera". Async: a window
/// built inside a synchronous command waits on the main thread that runs the command, and on
/// Windows that never returns.
#[tauri::command(async)]
pub fn open_lentera(app: AppHandle) {
    open(&app);
}

/// The page's capsule or bar, where it takes the mouse.
#[tauri::command]
pub fn set_lentera_shape(window: WebviewWindow, shape: Shape) {
    if window.label() != LENTERA {
        return;
    }
    if let (Some(shape), Ok(mut m)) = (shape.checked(), memory().lock()) {
        m.shape = Some(shape);
    }
}

/// Lentera is done (Esc, its close button, a click elsewhere). `give_back` returns the keyboard to
/// where it was before; a click elsewhere already moved it.
#[tauri::command]
pub fn hide_lentera(window: WebviewWindow, give_back: bool) {
    if window.label() != LENTERA {
        return;
    }
    // Given back before hiding, while Hub may still bring a window forward.
    if give_back {
        self::give_back(&window);
    }
    forget();
    window::hide(&window);
}

/// Whether the page should open the bar (the window was built because the owner called Lentera),
/// once.
#[tauri::command]
pub fn take_lentera_open(window: WebviewWindow) -> bool {
    if window.label() != LENTERA {
        return false;
    }
    memory().lock().map(|mut m| std::mem::take(&mut m.pending_bar)).unwrap_or(false)
}

/// The owner opened the bar from a capsule shown quietly: the window may take the keyboard now.
/// Their click on the capsule lets Windows bring it forward.
#[tauri::command]
pub fn focus_lentera(window: WebviewWindow) {
    if window.label() != LENTERA {
        return;
    }
    // Where the owner was, so Esc or a save gives the keyboard back there.
    remember_foreground(window.app_handle());
    // Shown through Tauri now (it drops WS_EX_NOACTIVATE), and given the keyboard.
    window::reveal(&window);
}

/// A line was saved: the owner gets the keyboard back at once while the capsule says so.
#[tauri::command]
pub fn return_lentera_focus(window: WebviewWindow) {
    if window.label() == LENTERA {
        give_back(&window);
    }
}

/// A fullscreen application in front of the primary screen, the way ULTRON's island reads it
/// (`../Ultron/shell/src-tauri/src/fullscreen.rs`): Windows says it is busy with one (a fullscreen
/// app, a Direct3D game, or presentation mode), and the window in front covers the whole screen
/// without being merely maximised.
mod fullscreen {
    use tauri::AppHandle;

    use super::Rect;

    /// QUNS_BUSY (2), QUNS_RUNNING_D3D_FULL_SCREEN (3), QUNS_PRESENTATION_MODE (4).
    pub fn busy(raw: i32) -> bool {
        (2..=4).contains(&raw)
    }

    /// Whether a window's rectangle covers the whole of a screen's.
    pub fn covers(window: Rect, screen: Rect) -> bool {
        window.x <= screen.x
            && window.y <= screen.y
            && i64::from(window.x) + i64::from(window.width) >= i64::from(screen.x) + i64::from(screen.width)
            && i64::from(window.y) + i64::from(window.height) >= i64::from(screen.y) + i64::from(screen.height)
    }

    pub fn in_front(app: &AppHandle) -> bool {
        let Ok(Some(monitor)) = app.primary_monitor() else { return false };
        let screen = Rect { x: monitor.position().x, y: monitor.position().y, width: monitor.size().width, height: monitor.size().height };
        foreground().is_some_and(|(window, maximized)| busy(attention()) && !maximized && covers(window, screen))
    }

    #[cfg(windows)]
    fn attention() -> i32 {
        use windows_sys::Win32::UI::Shell::SHQueryUserNotificationState;
        let mut state: i32 = 0;
        // SAFETY: the call writes one i32 to the pointer it is given, which lives for the call.
        let result = unsafe { SHQueryUserNotificationState(&mut state) };
        if result == 0 { state } else { 0 }
    }

    #[cfg(windows)]
    fn foreground() -> Option<(Rect, bool)> {
        use windows_sys::Win32::Foundation::RECT;
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect, IsZoomed};
        // SAFETY: GetForegroundWindow takes nothing and may return null, which is checked;
        // GetWindowRect writes one RECT it is given, which lives for the call; IsZoomed only reads.
        unsafe {
            let window = GetForegroundWindow();
            if window.is_null() {
                return None;
            }
            let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            if GetWindowRect(window, &mut rect) == 0 {
                return None;
            }
            let size = |a: i32, b: i32| u32::try_from(b - a).unwrap_or(0);
            Some((Rect { x: rect.left, y: rect.top, width: size(rect.left, rect.right), height: size(rect.top, rect.bottom) }, IsZoomed(window) != 0))
        }
    }

    #[cfg(not(windows))]
    fn attention() -> i32 {
        0
    }

    #[cfg(not(windows))]
    fn foreground() -> Option<(Rect, bool)> {
        None
    }
}

#[cfg(windows)]
mod focus {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, IsWindow, SetForegroundWindow};

    /// The taskbar and the tray's overflow: Lentera opened from the tray menu has nothing to give
    /// the keyboard back to there, so Windows picks the next window as usual.
    const SHELL: [&str; 4] = ["Shell_TrayWnd", "Shell_SecondaryTrayWnd", "NotifyIconOverflowWindow", "TopLevelWindowForOverflowXamlIsland"];

    pub fn foreground() -> Option<isize> {
        // SAFETY: no arguments; returns a window handle or null.
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_null() {
            return None;
        }
        let mut name = [0u16; 64];
        // SAFETY: a live window handle and a buffer of the length passed.
        let length = unsafe { GetClassNameW(hwnd, name.as_mut_ptr(), name.len() as i32) };
        let class = String::from_utf16_lossy(&name[..usize::try_from(length).unwrap_or(0)]);
        (!SHELL.contains(&class.as_str())).then_some(hwnd as isize)
    }

    pub fn bring(hwnd: isize) {
        let hwnd = hwnd as windows_sys::Win32::Foundation::HWND;
        // SAFETY: IsWindow accepts any value and says whether it is still a window; only then is it
        // brought forward. Hub has the foreground (Lentera has the keyboard), so Windows allows it.
        unsafe {
            if IsWindow(hwnd) != 0 {
                SetForegroundWindow(hwnd);
            }
        }
    }
}

#[cfg(not(windows))]
mod focus {
    pub fn foreground() -> Option<isize> {
        None
    }

    pub fn bring(_hwnd: isize) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    const FHD: Rect = Rect { x: 0, y: 0, width: 1920, height: 1032 };

    #[test]
    fn the_window_sits_in_the_work_areas_bottom_right_corner() {
        // 1920 x 1080 with a 48 px taskbar at the bottom, 100 %.
        assert_eq!(placement(FHD, 1.0), Rect { x: 1440, y: 392, width: 480, height: 640 });
    }

    #[test]
    fn the_display_scale_sizes_the_window_in_physical_pixels() {
        // 2560 x 1440 at 150 %, taskbar 72 px.
        let work = Rect { x: 0, y: 0, width: 2560, height: 1368 };
        assert_eq!(placement(work, 1.5), Rect { x: 1840, y: 408, width: 720, height: 960 });
        // 125 % rounds to whole pixels.
        assert_eq!(placement(FHD, 1.25), Rect { x: 1320, y: 232, width: 600, height: 800 });
    }

    #[test]
    fn a_taskbar_on_the_left_or_top_still_leaves_the_corner_at_the_bottom_right() {
        let left = Rect { x: 48, y: 0, width: 1872, height: 1080 };
        assert_eq!(placement(left, 1.0), Rect { x: 1440, y: 440, width: 480, height: 640 });
        let top = Rect { x: 0, y: 48, width: 1920, height: 1032 };
        assert_eq!(placement(top, 1.0), Rect { x: 1440, y: 440, width: 480, height: 640 });
    }

    #[test]
    fn a_primary_screen_not_at_the_origin_is_followed() {
        // A primary display to the left of another (negative coordinates are allowed).
        let work = Rect { x: -1920, y: 0, width: 1920, height: 1032 };
        assert_eq!(placement(work, 1.0), Rect { x: -480, y: 392, width: 480, height: 640 });
    }

    #[test]
    fn a_work_area_smaller_than_the_window_is_not_overrun() {
        let small = Rect { x: 0, y: 0, width: 600, height: 500 };
        assert_eq!(placement(small, 2.0), Rect { x: 0, y: 0, width: 600, height: 500 });
        // A nonsense scale is read as 100 %.
        assert_eq!(placement(FHD, f64::NAN), placement(FHD, 1.0));
    }

    fn capsule() -> Shape {
        // A 200 x 40 pill at the bottom right of the window, 16 px from its edges.
        Shape { x: 264.0, y: 344.0, width: 200.0, height: 40.0, radius: 20.0 }
    }

    #[test]
    fn only_the_shape_takes_the_mouse() {
        let origin = (1440, 632);
        let at = |x: f64, y: f64| inside((1440.0 + x, 632.0 + y), origin, 1.0, capsule());
        assert!(at(364.0, 364.0), "the middle of the capsule");
        assert!(at(284.0, 346.0), "inside its rounded left end");
        assert!(!at(265.0, 345.0), "just outside the rounded corner");
        assert!(!at(100.0, 100.0), "the empty part of the window");
        assert!(!at(364.0, 390.0), "below it, in the window's margin");
    }

    #[test]
    fn the_shape_is_read_in_logical_pixels() {
        // At 150 % the window's physical origin is the same corner, the shape 1.5 times larger.
        let origin = (1840, 768);
        assert!(inside((1840.0 + 364.0 * 1.5, 768.0 + 364.0 * 1.5), origin, 1.5, capsule()));
        assert!(!inside((1840.0 + 364.0, 768.0 + 364.0), origin, 1.5, capsule()));
    }

    #[test]
    fn a_reported_shape_is_kept_inside_the_window() {
        let wild = Shape { x: -10.0, y: 480.0, width: 600.0, height: 200.0, radius: 500.0 };
        let kept = wild.checked().expect("a shape");
        assert_eq!(kept, Shape { x: 0.0, y: 480.0, width: WIDTH, height: 160.0, radius: 80.0 });
        assert_eq!(Shape { x: f64::NAN, ..capsule() }.checked(), None);
        assert_eq!(Shape { x: 500.0, ..capsule() }.checked(), None, "nothing of it inside the window");
        assert_eq!(capsule().checked(), Some(capsule()));
    }

    #[test]
    fn only_a_busy_shell_and_a_window_over_the_whole_screen_is_fullscreen() {
        assert!(fullscreen::busy(2) && fullscreen::busy(3) && fullscreen::busy(4));
        // Free, quiet time, a locked screen, and an ordinary Store app are not.
        assert!(!fullscreen::busy(5) && !fullscreen::busy(1) && !fullscreen::busy(7));
        let screen = Rect { x: 0, y: 0, width: 2560, height: 1440 };
        assert!(fullscreen::covers(screen, screen));
        assert!(fullscreen::covers(Rect { x: -8, y: -8, width: 2576, height: 1456 }, screen));
        assert!(!fullscreen::covers(Rect { x: 0, y: 0, width: 2560, height: 1392 }, screen), "a window above the taskbar");
    }

    #[test]
    fn a_bad_scale_lets_the_window_take_the_mouse() {
        assert!(inside((0.0, 0.0), (0, 0), 0.0, capsule()));
    }
}
