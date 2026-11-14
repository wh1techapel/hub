//! Dari Pesan (CP03): a pasted group message (a WhatsApp class group, usually) cleaned on this
//! computer before any of it is sent to Google.
//!
//! On Gemini's free tier Google may use what is sent and people may read it, and a group message
//! carries other people's names and numbers. So the senders' names, the times of sending, phone
//! numbers, mentions, and email addresses are taken out here, and the owner sees the cleaned text
//! before pressing Kirim. The date a message was sent stays, as a "(dikirim ...)" line, because
//! "besok" in a message means the day after it was sent. Media, deleted messages, and the group's
//! own notices are dropped. Names written inside a message's text are taken out only where they
//! are a sender's whole name; the page says so.
//!
//! The formats read: WhatsApp's export on Android (`10/11/26 19.02 - Budi: ...`, also with
//! `7:02 PM`), on iPhone (`[10/11/26 19.02.33] Budi: ...`), and messages copied from WhatsApp on
//! the desktop or the web (`[19.02, 10/11/2026] Budi: ...`). Text without any of these is cleaned
//! the same way, line by line. Cleaning cleaned text changes nothing, so the native side cleans
//! again before sending whatever the page hands over.

use std::collections::BTreeSet;

use jiff::civil::{Date, Weekday};
use serde::Serialize;
use ts_rs::TS;

/// What the page may hand over at once.
pub const PASTE_MAX: usize = 20_000;
/// What Lentera reads at once, after cleaning.
pub const SEND_MAX: usize = 8_000;
/// A sender's name is at most this long (the part before ": ").
const SENDER_MAX: usize = 64;
/// A phone number has at least this many digits.
const PHONE_DIGITS: usize = 9;

/// The cleaned text and what was taken out of it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Cleaned {
    pub text: String,
    /// Different senders whose names were taken out.
    pub senders: u32,
    pub phones: u32,
    pub emails: u32,
    pub mentions: u32,
    /// Times of sending taken out (the dates stay).
    pub times: u32,
    /// Lines dropped: media, deleted messages, the group's notices.
    pub dropped: u32,
    /// Longer than Lentera reads at once (`SEND_MAX` characters).
    pub too_long: bool,
}

impl Cleaned {
    /// Whether anything was taken out.
    pub fn removed_anything(&self) -> bool {
        self.senders + self.phones + self.emails + self.mentions + self.times + self.dropped > 0
    }
}

fn day_name(day: Weekday) -> &'static str {
    match day {
        Weekday::Monday => "Senin",
        Weekday::Tuesday => "Selasa",
        Weekday::Wednesday => "Rabu",
        Weekday::Thursday => "Kamis",
        Weekday::Friday => "Jumat",
        Weekday::Saturday => "Sabtu",
        Weekday::Sunday => "Minggu",
    }
}

/// `Selasa, 2026-11-10`.
pub fn day_text(date: Date) -> String {
    format!("{}, {date}", day_name(date.weekday()))
}

#[derive(Debug, PartialEq)]
struct Header {
    date: Option<Date>,
    timed: bool,
    /// `None` for the group's own notices ("Budi menambahkan Sari").
    sender: Option<String>,
    rest: String,
}

fn numbers(token: &str, separators: &[char]) -> Option<Vec<u32>> {
    let parts: Vec<&str> = token.split(|c| separators.contains(&c)).collect();
    if parts.iter().any(|p| p.is_empty() || p.len() > 4 || !p.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    parts.iter().map(|p| p.parse().ok()).collect()
}

fn time_token(token: &str) -> bool {
    match numbers(token, &[':', '.']) {
        Some(parts) if parts.len() == 2 || parts.len() == 3 => parts[0] <= 23 && parts[1..].iter().all(|n| *n <= 59),
        _ => false,
    }
}

fn date_token(token: &str, twelve_hour: bool) -> Option<Date> {
    let parts = numbers(token, &['/', '-'])?;
    let &[a, b, c] = parts.as_slice() else { return None };
    let (year, month, day) = if a >= 1000 {
        (a, b, c)
    } else {
        let year = if c < 100 { 2000 + c } else { c };
        // Indonesian (and British) day first; a 12-hour clock is the American order, month first,
        // when the numbers alone cannot tell.
        let (day, month) = if a > 12 {
            (a, b)
        } else if b > 12 || twelve_hour {
            (b, a)
        } else {
            (a, b)
        };
        (year, month, day)
    };
    Date::new(i16::try_from(year).ok()?, i8::try_from(month).ok()?, i8::try_from(day).ok()?).ok()
}

/// A time of sending and its date, when the whole text is one ("19.02, 10/11/2026", "10/11/26
/// 7:02 PM"); `None` when any part of it is something else.
fn stamp(text: &str) -> Option<(Option<Date>, bool)> {
    let tokens: Vec<&str> = text.split([',', ' ']).filter(|t| !t.is_empty()).collect();
    if tokens.is_empty() {
        return None;
    }
    let twelve_hour = tokens.iter().any(|t| matches!(t.to_ascii_lowercase().as_str(), "am" | "pm" | "a.m." | "p.m."));
    let (mut date, mut timed) = (None, false);
    for token in &tokens {
        let lower = token.to_ascii_lowercase();
        if matches!(lower.as_str(), "am" | "pm" | "a.m." | "p.m.") {
            continue;
        }
        if time_token(token) {
            timed = true;
        } else {
            date = Some(date_token(token, twelve_hour)?);
        }
    }
    (date.is_some() || timed).then_some((date, timed))
}

/// "Budi: pesan" into the sender and the message; `None` when there is no sender.
fn sender(rest: &str) -> Option<(String, String)> {
    let at = rest.find(": ")?;
    let name = rest[..at].trim();
    if name.is_empty() || name.chars().count() > SENDER_MAX {
        return None;
    }
    Some((name.to_string(), rest[at + 2..].to_string()))
}

fn header(line: &str) -> Option<Header> {
    let (stamp_text, rest) = if let Some(inside) = line.strip_prefix('[') {
        let end = inside.find(']').filter(|&end| end <= 40)?;
        (&inside[..end], inside[end + 1..].trim_start())
    } else if line.starts_with(|c: char| c.is_ascii_digit()) {
        let end = line.find(" - ").filter(|&end| end <= 32)?;
        (&line[..end], &line[end + 3..])
    } else {
        return None;
    };
    let (date, timed) = stamp(stamp_text)?;
    Some(match sender(rest) {
        Some((name, message)) => Header { date, timed, sender: Some(name), rest: message },
        None => Header { date, timed, sender: None, rest: rest.to_string() },
    })
}

/// What WhatsApp writes in place of media, deleted messages, and the like.
fn placeholder(message: &str) -> bool {
    let lower = message.trim().to_lowercase();
    const WHOLE: &[&str] = &[
        "<media omitted>",
        "<media tidak disertakan>",
        "media omitted",
        "pesan ini telah dihapus",
        "pesan ini dihapus",
        "this message was deleted",
        "you deleted this message",
        "anda menghapus pesan ini",
        "kamu menghapus pesan ini",
        "image omitted",
        "video omitted",
        "audio omitted",
        "sticker omitted",
        "gif omitted",
        "document omitted",
        "gambar tidak disertakan",
        "video tidak disertakan",
        "audio tidak disertakan",
        "stiker tidak disertakan",
        "dokumen tidak disertakan",
        "null",
    ];
    WHOLE.contains(&lower.as_str()) || lower.starts_with("<attached:") || lower.starts_with("<lampiran:")
}

fn strip_edited(message: &str) -> &str {
    let trimmed = message.trim_end();
    for mark in ["<pesan ini diedit>", "<this message was edited>", "<pesan diedit>"] {
        if trimmed.len() >= mark.len() {
            let start = trimmed.len() - mark.len();
            if trimmed.is_char_boundary(start) && trimmed[start..].eq_ignore_ascii_case(mark) {
                return trimmed[..start].trim_end();
            }
        }
    }
    trimmed
}

#[derive(Default)]
struct Counts {
    phones: u32,
    emails: u32,
    mentions: u32,
}

fn is_email(token: &str) -> bool {
    let Some((local, domain)) = token.split_once('@') else { return false };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && local.chars().chain(domain.chars()).all(|c| c.is_alphanumeric() || "._%+-".contains(c))
}

/// Phone numbers: a `+` or a digit at the start of a word, then digits with spaces, dashes, and
/// brackets; at least nine digits, starting with `+`, `0`, or `62`.
fn scrub_phones(text: &str, counts: &mut Counts) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let word_start = i == 0 || !chars[i - 1].is_alphanumeric();
        let starts = word_start && (c == '+' || c == '(' || c.is_ascii_digit()) && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit() || c.is_ascii_digit());
        if starts {
            let mut j = i;
            let mut digits = String::new();
            let mut last_digit = i;
            while j < chars.len() {
                let d = chars[j];
                if d.is_ascii_digit() {
                    digits.push(d);
                    last_digit = j;
                } else if !(matches!(d, ' ' | '-' | '(' | ')') || (d == '+' && j == i)) || (d == ' ' && chars.get(j + 1) == Some(&' ')) {
                    break;
                }
                j += 1;
            }
            let next_is_word = chars.get(last_digit + 1).is_some_and(|n| n.is_alphanumeric());
            let plus = chars[i] == '+' || (chars[i] == '(' && chars.get(i + 1) == Some(&'+'));
            if digits.len() >= PHONE_DIGITS && !next_is_word && (plus || digits.starts_with('0') || digits.starts_with("62")) {
                out.push_str("[nomor]");
                counts.phones += 1;
                i = last_digit + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Emails and mentions, word by word (the spaces between words are kept as they were).
fn scrub_words(text: &str, counts: &mut Counts) -> String {
    let mut out = String::with_capacity(text.len());
    for (index, word) in text.split(' ').enumerate() {
        if index > 0 {
            out.push(' ');
        }
        let core = word.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', '"', '\'']);
        let tail = &word[core.len()..];
        let lead = core.len() - core.trim_start_matches(['(', '"', '\'']).len();
        let (opening, core) = core.split_at(lead);
        if is_email(core) {
            out.push_str(opening);
            out.push_str("[email]");
            out.push_str(tail);
            counts.emails += 1;
        } else if core.strip_prefix('@').map(|n| n.trim_start_matches('~')).is_some_and(|n| n.chars().next().is_some_and(char::is_alphanumeric)) {
            out.push_str(opening);
            out.push_str("[nama]");
            out.push_str(tail);
            counts.mentions += 1;
        } else {
            out.push_str(word);
        }
    }
    out
}

/// A sender's whole name written inside a message, as a word of its own, case aside.
fn scrub_names(text: &str, names: &BTreeSet<String>) -> String {
    let mut out = text.to_string();
    for name in names {
        if name.chars().count() < 3 || name.chars().any(|c| c.is_ascii_digit()) {
            continue;
        }
        let lower_name = name.to_lowercase();
        loop {
            let lower = out.to_lowercase();
            // Only where lower-casing keeps every byte where it was (Latin text, as here).
            if lower.len() != out.len() || lower_name.len() != name.len() {
                break;
            }
            let Some(found) = find_word(&lower, &lower_name) else { break };
            out.replace_range(found..found + name.len(), "[nama]");
        }
    }
    out
}

fn find_word(haystack: &str, needle: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(at) = haystack[from..].find(needle).map(|i| i + from) {
        let before = haystack[..at].chars().next_back();
        let after = haystack[at + needle.len()..].chars().next();
        if !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric) {
            return Some(at);
        }
        from = at + needle.len();
    }
    None
}

fn normalize(line: &str) -> String {
    line.chars()
        .filter(|c| !matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}'))
        .map(|c| if matches!(c, '\u{a0}' | '\u{202f}' | '\u{2009}') { ' ' } else { c })
        .collect()
}

/// Clean a pasted message. Pure: the same text always gives the same result.
pub fn clean(raw: &str) -> Cleaned {
    let mut out = Cleaned::default();
    let mut counts = Counts::default();
    let mut senders: BTreeSet<String> = BTreeSet::new();
    // First the lines as messages: (the date it was sent, the message's lines).
    let mut messages: Vec<(Option<Date>, Vec<String>)> = Vec::new();
    // Inside a message (its next lines without a header belong to it), and whether any line had
    // a header at all.
    let mut open = false;
    let mut headed = false;
    for line in raw.replace("\r\n", "\n").replace('\r', "\n").lines() {
        let line = normalize(line);
        match header(line.trim_start()) {
            Some(found) => {
                headed = true;
                if found.timed {
                    out.times += 1;
                }
                let Some(name) = found.sender else {
                    out.dropped += 1;
                    open = false;
                    continue;
                };
                senders.insert(name);
                let message = strip_edited(&found.rest).to_string();
                if placeholder(&message) {
                    out.dropped += 1;
                    open = false;
                    continue;
                }
                messages.push((found.date, vec![message]));
                open = true;
            }
            None => {
                if open && let Some((_, lines)) = messages.last_mut() {
                    lines.push(line.trim_end().to_string());
                } else if placeholder(&line) {
                    out.dropped += 1;
                } else {
                    messages.push((None, vec![line.trim_end().to_string()]));
                }
            }
        }
    }
    let names: BTreeSet<String> = senders.iter().filter(|n| !n.starts_with('+') && !n.starts_with(|c: char| c.is_ascii_digit())).cloned().collect();
    out.senders = u32::try_from(senders.len()).unwrap_or(u32::MAX);

    let mut text = String::new();
    let mut current: Option<Date> = None;
    for (date, lines) in messages {
        if let Some(date) = date
            && current != Some(date)
        {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&format!("(dikirim {})\n", day_text(date)));
            current = Some(date);
        }
        for (index, line) in lines.iter().enumerate() {
            let line = scrub_words(&scrub_phones(&scrub_names(line, &names), &mut counts), &mut counts);
            let prefix = match (headed, index) {
                (true, 0) => "- ",
                (true, _) => "  ",
                (false, _) => "",
            };
            if line.trim().is_empty() && headed {
                continue;
            }
            text.push_str(prefix);
            text.push_str(line.trim_end());
            text.push('\n');
        }
    }
    out.text = text.trim().to_string();
    out.phones = counts.phones;
    out.emails = counts.emails;
    out.mentions = counts.mentions;
    out.too_long = out.text.chars().count() > SEND_MAX;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANDROID: &str = "\
10/11/26 19.02 - Pesan dan panggilan terenkripsi secara end-to-end.
10/11/26 19.02 - Budi Santoso: Guys tugas MTK hal 45 no 1-10 dikumpulkan besok jam 7 ya
10/11/26 19.03 - +62 812-3456-7890: oke siap
10/11/26 19.05 - Sari: <Media tidak disertakan>
11/11/26 06.30 - Budi Santoso: jangan lupa bawa jas lab, praktikum Jumat
kirim laporan ke budi.santoso@gmail.com ya, atau WA 0812 3456 7890
11/11/26 06.31 - Sari: @Budi makasih Budi Santoso";

    #[test]
    fn an_android_export_loses_names_times_numbers_and_media_but_keeps_its_dates() {
        let cleaned = clean(ANDROID);
        assert_eq!(
            cleaned.text,
            "(dikirim Selasa, 2026-11-10)\n\
             - Guys tugas MTK hal 45 no 1-10 dikumpulkan besok jam 7 ya\n\
             - oke siap\n\
             \n\
             (dikirim Rabu, 2026-11-11)\n\
             - jangan lupa bawa jas lab, praktikum Jumat\n\
             \x20 kirim laporan ke [email] ya, atau WA [nomor]\n\
             - [nama] makasih [nama]"
        );
        assert_eq!(cleaned.senders, 3, "Budi Santoso, the number, Sari");
        assert_eq!(cleaned.phones, 1, "the one in the text; the sender's number went with the sender");
        assert_eq!(cleaned.emails, 1);
        assert_eq!(cleaned.mentions, 1);
        assert_eq!(cleaned.times, 6);
        assert_eq!(cleaned.dropped, 2, "the encryption notice and the media");
        assert!(!cleaned.too_long);
        for secret in ["Budi", "Santoso", "Sari", "812", "3456", "gmail", "19.02", "06.30"] {
            assert!(!cleaned.text.contains(secret), "{secret} left in: {}", cleaned.text);
        }
    }

    #[test]
    fn iphone_exports_copied_messages_and_twelve_hour_clocks_are_read() {
        let iphone = clean("[10/11/26 19.02.33] Budi: PR fisika bab 3\n[10/11/26 19.04.01] Sari: \u{200e}image omitted");
        assert_eq!(iphone.text, "(dikirim Selasa, 2026-11-10)\n- PR fisika bab 3");
        assert_eq!((iphone.senders, iphone.dropped, iphone.times), (2, 1, 2));

        let copied = clean("[19.02, 10/11/2026] Budi Santoso: Ulangan bahasa Inggris Senin\n[19.05, 10/11/2026] Ketua Kelas: noted");
        assert_eq!(copied.text, "(dikirim Selasa, 2026-11-10)\n- Ulangan bahasa Inggris Senin\n- noted");

        // en-US: month first with a 12-hour clock, and the narrow space WhatsApp puts before PM.
        let american = clean("11/10/26, 7:02\u{202f}PM - Budi: quiz tomorrow");
        assert_eq!(american.text, "(dikirim Selasa, 2026-11-10)\n- quiz tomorrow");
        // A day above 12 tells the order by itself.
        let british = clean("25/11/2026, 07:02 - Budi: ujian");
        assert!(british.text.starts_with("(dikirim Rabu, 2026-11-25)"), "{}", british.text);
    }

    #[test]
    fn text_without_headers_is_cleaned_line_by_line() {
        let cleaned = clean("Tugas: buat poster K3\nkumpul ke wa.me/6281234567890 atau +62 857 1234 5678 sebelum Jumat\n[PENTING] bawa laptop");
        assert_eq!(cleaned.text, "Tugas: buat poster K3\nkumpul ke wa.me/[nomor] atau [nomor] sebelum Jumat\n[PENTING] bawa laptop");
        assert_eq!((cleaned.senders, cleaned.phones, cleaned.times), (0, 2, 0));
    }

    #[test]
    fn numbers_that_are_not_phones_stay() {
        let text = "Kelas 10 11 12, Rp 1.500.000, NIS 2026-11-10, jam 19.00, soal 1-10, 123456789";
        let cleaned = clean(text);
        assert_eq!(cleaned.text, text);
        assert_eq!(cleaned.phones, 0);
        // A local number and one with brackets are phones.
        assert_eq!(clean("hubungi (021) 555 1234 5 atau 0857-1234-5678.").text, "hubungi [nomor] atau [nomor].");
    }

    #[test]
    fn cleaning_twice_changes_nothing() {
        for raw in [ANDROID, "[19.02, 10/11/2026] Budi: hai @Sari, email sari@x.id\nlanjut", "biasa saja\n\n(dikirim Senin, 2026-11-09)\n- [nomor]"] {
            let once = clean(raw);
            let twice = clean(&once.text);
            assert_eq!(twice.text, once.text, "{raw}");
            assert!(!twice.removed_anything(), "{raw}: {twice:?}");
        }
    }

    #[test]
    fn a_long_paste_says_so() {
        let long = "- tugas\n".repeat(SEND_MAX / 4);
        assert!(clean(&long).too_long);
        assert!(!clean("pendek").too_long);
    }

    #[test]
    fn stamps_must_be_whole_dates_and_times() {
        assert_eq!(stamp("19.02, 10/11/2026"), Some((Date::new(2026, 11, 10).ok(), true)));
        assert_eq!(stamp("PENTING"), None);
        assert_eq!(stamp("10/11/26 besok"), None);
        assert_eq!(stamp("31/02/2026"), None, "no such day");
        assert!(header("10/11/26 19.02 - tanpa pengirim").is_some_and(|h| h.sender.is_none()));
        assert_eq!(header("- Budi: bukan header"), None);
    }
}
