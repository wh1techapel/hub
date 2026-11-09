//! `hubapp://` links: what a notification button or click asks of Hub.
//!
//! Windows starts `hub.exe "hubapp://..."`; a Hub already running receives the argument through
//! single instance. The set of links is closed and every id is checked, because any program (or
//! a web page, after the browser asks) can open such a link: at worst it can snooze or finish
//! one of the owner's reminders by its id, and the change is recorded in Jejak.

use hub_core::ids;

pub const SCHEME: &str = "hubapp";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    Snooze(String),
    Done(String),
    /// Open Hub on a page: `today`, `review` (Tutup Hari), or a Studio project.
    Open(Page),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Page {
    Today,
    Review,
    /// A Studio project, by id (toasts about Claude's work open its Pantau).
    Studio(String),
}

pub fn parse(arg: &str) -> Option<Link> {
    let rest = arg.trim().strip_prefix("hubapp://")?;
    let parts: Vec<&str> = rest.trim_end_matches('/').split('/').collect();
    match parts.as_slice() {
        ["reminder", id, "snooze"] => ids::parse("id", id).ok().map(Link::Snooze),
        ["reminder", id, "done"] => ids::parse("id", id).ok().map(Link::Done),
        ["open", "today"] => Some(Link::Open(Page::Today)),
        ["open", "review"] => Some(Link::Open(Page::Review)),
        ["open", "studio", id] => ids::parse("id", id).ok().map(|id| Link::Open(Page::Studio(id))),
        _ => None,
    }
}

/// Register `hubapp://` for the current user, pointing at this executable.
pub fn register() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let exe = exe.to_string_lossy().into_owned();
    let base = format!(r"Software\Classes\{SCHEME}");
    crate::registry::set_values(&base, &[("", "URL:Hub".to_string()), ("URL Protocol", String::new())])
        && crate::registry::set_values(&format!(r"{base}\DefaultIcon"), &[("", format!("\"{exe}\",0"))])
        && crate::registry::set_values(&format!(r"{base}\shell\open\command"), &[("", format!("\"{exe}\" \"%1\""))])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_known_links_with_real_ids() {
        let id = ids::new_id();
        assert_eq!(parse(&format!("hubapp://reminder/{id}/snooze")), Some(Link::Snooze(id.clone())));
        assert_eq!(parse(&format!("hubapp://reminder/{id}/done/")), Some(Link::Done(id.clone())));
        assert_eq!(parse("hubapp://open/review"), Some(Link::Open(Page::Review)));
        assert_eq!(parse(&format!("hubapp://open/studio/{id}")), Some(Link::Open(Page::Studio(id.clone()))));
        assert_eq!(parse("hubapp://open/studio/../settings"), None);
        for bad in [
            "hubapp://reminder/not-an-id/done",
            "hubapp://reminder/../../done",
            "hubapp://tasks/delete-all",
            "https://example.com",
            "hubapp://open/../../etc",
            "",
        ] {
            assert_eq!(parse(bad), None, "{bad}");
        }
    }
}
