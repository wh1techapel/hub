//! Windows notifications: reminders (with "Tunda 10 m" and "Selesai"), what came due while Hub was
//! away, and the evening Tutup Hari nudge.
//!
//! An application without a package needs an AppUserModelID that Windows can name: it is
//! registered for the current user under `HKCU\Software\Classes\AppUserModelId\dev.hub.desktop`
//! (a display name and the icon), as ULTRON's shell does. The buttons use protocol activation:
//! Windows opens `hubapp://...`, which `protocol.rs` registers to start Hub, and the running Hub
//! receives it through single instance. Windows itself holds notifications back during Focus,
//! Do Not Disturb, or a fullscreen game.

use std::path::{Path, PathBuf};

pub const APP_ID: &str = "dev.hub.desktop";
pub const DISPLAY_NAME: &str = "Hub";
pub const REGISTRY_KEY: &str = r"Software\Classes\AppUserModelId\dev.hub.desktop";

const MAX_TITLE: usize = 80;
const MAX_BODY: usize = 300;

/// Text for the notification's XML: markup escaped, control characters dropped, cut to `limit`.
/// Reminder texts are the owner's words, never markup.
pub fn escape(text: &str, limit: usize) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars().take(limit) {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\n' | '\t' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// A reminder that stays on screen until handled, with its two buttons.
pub fn reminder_xml(id: &str, text: &str, when: &str) -> String {
    format!(
        "<toast scenario=\"reminder\" activationType=\"protocol\" launch=\"hubapp://open/today\">\
         <visual><binding template=\"ToastGeneric\"><text>Pengingat</text><text>{}</text>\
         <text placement=\"attribution\">{}</text></binding></visual>\
         <actions>\
         <action content=\"Tunda 10 m\" activationType=\"protocol\" arguments=\"hubapp://reminder/{id}/snooze\"/>\
         <action content=\"Selesai\" activationType=\"protocol\" arguments=\"hubapp://reminder/{id}/done\"/>\
         </actions>\
         <audio src=\"ms-winsoundevent:Notification.Reminder\"/></toast>",
        escape(text, MAX_BODY),
        escape(when, MAX_TITLE),
    )
}

/// A plain notification that opens Hub at `target` (`today`, `review`).
pub fn plain_xml(title: &str, body: &str, target: &str) -> String {
    format!(
        "<toast activationType=\"protocol\" launch=\"hubapp://open/{target}\">\
         <visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual>\
         <audio src=\"ms-winsoundevent:Notification.Reminder\"/></toast>",
        escape(title, MAX_TITLE),
        escape(body, MAX_BODY),
    )
}

/// `icons/128x128.png` in the nearest folder above `start` that has one.
pub fn find_icon(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .flat_map(|dir| [dir.join("icons").join("128x128.png"), dir.join("src-tauri").join("icons").join("128x128.png")])
        .find(|path| path.is_file())
}

#[cfg(windows)]
mod imp {
    use std::path::Path;

    use windows::core::HSTRING;
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

    use super::{APP_ID, DISPLAY_NAME, REGISTRY_KEY};
    use crate::registry;

    pub fn register(icon: Option<&Path>) -> bool {
        let mut values = vec![("DisplayName", DISPLAY_NAME.to_string())];
        if let Some(icon) = icon {
            values.push(("IconUri", icon.to_string_lossy().into_owned()));
        }
        registry::set_values(REGISTRY_KEY, &values)
    }

    pub fn show(xml: String) {
        let _ = std::thread::Builder::new().name("hub-toast".into()).spawn(move || {
            // SAFETY: initialising the WinRT apartment of this new thread, once.
            let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
            if let Err(error) = toast(&xml) {
                eprintln!("[hub] Windows notification failed: {error}");
            }
        });
    }

    pub(super) fn toast(xml: &str) -> windows::core::Result<()> {
        let document = XmlDocument::new()?;
        document.LoadXml(&HSTRING::from(xml))?;
        let notification = ToastNotification::CreateToastNotification(&document)?;
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))?.Show(&notification)
    }
}

/// Register the notification id with Hub's icon (at start; Windows only).
pub fn register() {
    #[cfg(windows)]
    {
        let icon = std::env::current_exe().ok().and_then(|exe| find_icon(&exe)).or_else(|| find_icon(Path::new(env!("CARGO_MANIFEST_DIR"))));
        if !imp::register(icon.as_deref()) {
            eprintln!("[hub] the notification id could not be registered");
        }
    }
}

pub fn show(xml: String) {
    #[cfg(windows)]
    imp::show(xml);
    #[cfg(not(windows))]
    let _ = xml;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_owners_words_are_text_never_markup() {
        assert_eq!(escape("<b>call</b> & \"pay\" 'rent'", 100), "&lt;b&gt;call&lt;/b&gt; &amp; &quot;pay&quot; &apos;rent&apos;");
        assert_eq!(escape("a\nb\u{0007}c", 100), "a bc");
        let xml = reminder_xml("0190-x", "<script>alert(1)</script>", "21.00");
        assert!(xml.contains("<text>&lt;script&gt;alert(1)&lt;/script&gt;</text>"));
        assert!(xml.contains("arguments=\"hubapp://reminder/0190-x/snooze\""));
        assert!(xml.starts_with("<toast") && xml.ends_with("</toast>"));
    }

    #[test]
    fn every_notification_is_valid_xml_shape() {
        for xml in [reminder_xml("id", "Kumpulkan laporan", "21.00"), plain_xml("Hub", "Waktunya Tutup Hari", "review")] {
            assert_eq!(xml.matches("<toast").count(), 1);
            assert_eq!(xml.matches("</toast>").count(), 1);
        }
    }

    /// Shows a real notification: `cargo test -p hub -- --ignored a_real_notification`.
    #[cfg(windows)]
    #[test]
    #[ignore = "shows a real Windows notification"]
    fn a_real_notification_reaches_windows() {
        register();
        // SAFETY: initialising the WinRT apartment of the test's thread.
        let _ = unsafe { windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED) };
        imp::toast(&reminder_xml("test", "Notifikasi uji dari Hub (CP3)", "sekarang")).expect("Windows accepted it");
    }
}
