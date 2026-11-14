//! The owner's settings: one row per changed key, defaults for the rest.

use jiff::Timestamp;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::activity::{self, Record};
use crate::error::{invalid, Reason, Result};
use crate::model::{text_enum, Actor, Change, Entity, Ink, ThemeMode, Verb};

text_enum!(
    /// What the owner is ready to pay for what Studio builds (R3's developer profile).
    DevCost {
        Free => "free",
        Low => "low",
        Flexible => "flexible",
    }
);
use crate::time::{instant, minutes_of, parse_clock};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Settings {
    /// The name in the greeting.
    pub owner_name: String,
    /// The Day Arc runs from wake to sleep (`HH:MM`), and the morning light starts half an hour
    /// before waking.
    pub wake_time: String,
    pub sleep_time: String,
    pub theme_mode: ThemeMode,
    pub ink: Ink,
    /// Start with Windows into the tray. The app keeps this in step with Windows' own list.
    pub autostart: bool,
    /// An IANA zone, or none for Windows' own zone. Not editable in the UI yet.
    pub time_zone: Option<String>,
    /// One notification at `close_nudge_time` when the day is not closed yet.
    pub close_nudge: bool,
    pub close_nudge_time: String,
    /// Belajar's weekly goal, in active days (1 to 7).
    pub learning_goal_days: u8,
    /// The local API for ULTRON (127.0.0.1 only, token always required).
    pub api_enabled: bool,
    pub api_port: u16,
    /// Claude Code may use the local API (Studio's MCP bridge, docs/studio.md). Off until the
    /// owner turns it on; the API runs while either client is on, and each has its own token.
    pub claude_enabled: bool,
    /// The Gemini model Studio uses (docs/studio.md). The key itself is in Windows Credential
    /// Manager, never here.
    pub gemini_model: String,
    /// The developer profile Gemini reads when it recommends a tech stack (R3), filled once by
    /// the owner: what they know, what they want to learn, cost, and where things run. Sent to
    /// Gemini only with a stack recommendation the owner asks for.
    pub dev_knows: String,
    pub dev_learning: String,
    pub dev_cost: DevCost,
    pub dev_deploy: String,
    /// P2: Hub's small sounds (made in the app, never files), on from the start and quiet. The
    /// volume is Hub's own, 0 to 100, under Windows' volume.
    pub sound_enabled: bool,
    pub sound_volume: u8,
    /// Lentera CP02, Sesi Fokus: a focus block, a short break, a long break (in minutes), and how
    /// many blocks come before the long one. The classic 25/5/15 after 4 (Biwer et al. 2023 tested
    /// 24/6 against breaks the students chose themselves: CONTEXT.md).
    pub focus_minutes: u8,
    pub break_minutes: u8,
    pub long_break_minutes: u8,
    pub long_break_every: u8,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            owner_name: "Glenvio".into(),
            wake_time: "05:00".into(),
            sleep_time: "22:30".into(),
            theme_mode: ThemeMode::Auto,
            ink: Ink::Biru,
            autostart: true,
            time_zone: None,
            close_nudge: true,
            close_nudge_time: "21:00".into(),
            learning_goal_days: 4,
            api_enabled: true,
            api_port: DEFAULT_API_PORT,
            claude_enabled: false,
            gemini_model: DEFAULT_GEMINI_MODEL.into(),
            dev_knows: String::new(),
            dev_learning: String::new(),
            dev_cost: DevCost::Free,
            dev_deploy: String::new(),
            sound_enabled: true,
            sound_volume: DEFAULT_SOUND_VOLUME,
            focus_minutes: 25,
            break_minutes: 5,
            long_break_minutes: 15,
            long_break_every: 4,
        }
    }
}

/// A change to some settings. Unknown fields are refused, not ignored.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SettingsPatch {
    #[ts(optional)]
    pub owner_name: Option<String>,
    #[ts(optional)]
    pub wake_time: Option<String>,
    #[ts(optional)]
    pub sleep_time: Option<String>,
    #[ts(optional)]
    pub theme_mode: Option<ThemeMode>,
    #[ts(optional)]
    pub ink: Option<Ink>,
    #[ts(optional)]
    pub autostart: Option<bool>,
    #[ts(optional)]
    pub close_nudge: Option<bool>,
    #[ts(optional)]
    pub close_nudge_time: Option<String>,
    #[ts(optional)]
    pub learning_goal_days: Option<u8>,
    #[ts(optional)]
    pub api_enabled: Option<bool>,
    #[ts(optional)]
    pub api_port: Option<u16>,
    #[ts(optional)]
    pub claude_enabled: Option<bool>,
    #[ts(optional)]
    pub gemini_model: Option<String>,
    #[ts(optional)]
    pub dev_knows: Option<String>,
    #[ts(optional)]
    pub dev_learning: Option<String>,
    #[ts(optional)]
    pub dev_cost: Option<DevCost>,
    #[ts(optional)]
    pub dev_deploy: Option<String>,
    #[ts(optional)]
    pub sound_enabled: Option<bool>,
    #[ts(optional)]
    pub sound_volume: Option<u8>,
    #[ts(optional)]
    pub focus_minutes: Option<u8>,
    #[ts(optional)]
    pub break_minutes: Option<u8>,
    #[ts(optional)]
    pub long_break_minutes: Option<u8>,
    #[ts(optional)]
    pub long_break_every: Option<u8>,
}

/// What Pengaturan allows for Sesi Fokus (inclusive).
pub const FOCUS_MINUTES: (u8, u8) = (5, 90);
pub const BREAK_MINUTES: (u8, u8) = (1, 30);
pub const LONG_BREAK_MINUTES: (u8, u8) = (5, 60);
pub const LONG_BREAK_EVERY: (u8, u8) = (2, 8);

/// Quiet: the sounds sit well under what Windows itself plays (P2).
pub const DEFAULT_SOUND_VOLUME: u8 = 30;

/// The local API's port unless the owner picks another (docs/architecture.md, "Local API").
pub const DEFAULT_API_PORT: u16 = 47731;

/// Studio's model until the owner picks another: stable, on the free tier, structured output and
/// function calling (checked 2026-11-06).
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-3.8-flash";
const MODEL_MAX: usize = 80;

const NAME_MAX: usize = 40;
/// Each line of the developer profile.
const PROFILE_MAX: usize = 300;
/// The shortest day the Day Arc draws.
const DAY_MIN_MINUTES: i32 = 6 * 60;

pub fn load(conn: &Connection) -> Result<Settings> {
    let mut settings = Settings::default();
    let mut statement = conn.prepare("SELECT key, value FROM settings")?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
    for row in rows {
        let (key, value) = row?;
        // A stored value this version cannot read falls back to the default instead of failing.
        let value: serde_json::Value = match serde_json::from_str(&value) {
            Ok(value) => value,
            Err(_) => continue,
        };
        match key.as_str() {
            "owner_name" => set_from(&mut settings.owner_name, value),
            "wake_time" => set_from(&mut settings.wake_time, value),
            "sleep_time" => set_from(&mut settings.sleep_time, value),
            "theme_mode" => set_from(&mut settings.theme_mode, value),
            "ink" => set_from(&mut settings.ink, value),
            "autostart" => set_from(&mut settings.autostart, value),
            "time_zone" => set_from(&mut settings.time_zone, value),
            "close_nudge" => set_from(&mut settings.close_nudge, value),
            "close_nudge_time" => set_from(&mut settings.close_nudge_time, value),
            "learning_goal_days" => set_from(&mut settings.learning_goal_days, value),
            "api_enabled" => set_from(&mut settings.api_enabled, value),
            "api_port" => set_from(&mut settings.api_port, value),
            "claude_enabled" => set_from(&mut settings.claude_enabled, value),
            "gemini_model" => set_from(&mut settings.gemini_model, value),
            "dev_knows" => set_from(&mut settings.dev_knows, value),
            "dev_learning" => set_from(&mut settings.dev_learning, value),
            "dev_cost" => set_from(&mut settings.dev_cost, value),
            "dev_deploy" => set_from(&mut settings.dev_deploy, value),
            "sound_enabled" => set_from(&mut settings.sound_enabled, value),
            "sound_volume" => set_from(&mut settings.sound_volume, value),
            "focus_minutes" => set_from(&mut settings.focus_minutes, value),
            "break_minutes" => set_from(&mut settings.break_minutes, value),
            "long_break_minutes" => set_from(&mut settings.long_break_minutes, value),
            "long_break_every" => set_from(&mut settings.long_break_every, value),
            _ => {}
        }
    }
    Ok(settings)
}

fn set_from<T: serde::de::DeserializeOwned>(slot: &mut T, value: serde_json::Value) {
    if let Ok(parsed) = serde_json::from_value(value) {
        *slot = parsed;
    }
}

/// Whether the owner (or Hub on first run) ever stored this key.
pub fn is_stored(conn: &Connection, key: &str) -> Result<bool> {
    Ok(conn
        .query_row("SELECT 1 FROM settings WHERE key = ?1", [key], |_| Ok(()))
        .optional()?
        .is_some())
}

/// Check a patch against the current settings and store what changed, with one activity row.
pub fn update(
    conn: &mut Connection,
    patch: SettingsPatch,
    actor: Actor,
    now: Timestamp,
) -> Result<(Settings, Option<Change>)> {
    let current = load(conn)?;
    let mut next = current.clone();
    if let Some(name) = patch.owner_name {
        let name = name.trim().to_string();
        if name.is_empty() || name.chars().count() > NAME_MAX {
            return invalid("ownerName", Reason::Length);
        }
        next.owner_name = name;
    }
    if let Some(wake) = patch.wake_time {
        parse_clock("wakeTime", &wake)?;
        next.wake_time = wake;
    }
    if let Some(sleep) = patch.sleep_time {
        parse_clock("sleepTime", &sleep)?;
        next.sleep_time = sleep;
    }
    let wake = minutes_of(parse_clock("wakeTime", &next.wake_time)?);
    let sleep = minutes_of(parse_clock("sleepTime", &next.sleep_time)?);
    if sleep - wake < DAY_MIN_MINUTES {
        return invalid("sleepTime", Reason::Order);
    }
    if let Some(mode) = patch.theme_mode {
        next.theme_mode = mode;
    }
    if let Some(ink) = patch.ink {
        next.ink = ink;
    }
    if let Some(autostart) = patch.autostart {
        next.autostart = autostart;
    }
    if let Some(nudge) = patch.close_nudge {
        next.close_nudge = nudge;
    }
    if let Some(time) = patch.close_nudge_time {
        parse_clock("closeNudgeTime", &time)?;
        next.close_nudge_time = time;
    }
    if let Some(days) = patch.learning_goal_days {
        if !(1..=7).contains(&days) {
            return invalid("learningGoalDays", Reason::Format);
        }
        next.learning_goal_days = days;
    }
    if let Some(enabled) = patch.api_enabled {
        next.api_enabled = enabled;
    }
    if let Some(enabled) = patch.claude_enabled {
        next.claude_enabled = enabled;
    }
    if let Some(port) = patch.api_port {
        // Below 1024 needs an administrator on some systems; 0 means "any port".
        if port < 1024 {
            return invalid("apiPort", Reason::Format);
        }
        next.api_port = port;
    }
    if let Some(model) = patch.gemini_model {
        let model = model.trim().to_string();
        // A model id as Google writes it ("gemini-3.8-flash"): it ends up in a URL path.
        let shape = model.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.');
        if model.is_empty() || model.chars().count() > MODEL_MAX || !shape {
            return invalid("geminiModel", Reason::Format);
        }
        next.gemini_model = model;
    }
    for (field, value, slot) in [
        ("devKnows", patch.dev_knows, &mut next.dev_knows),
        ("devLearning", patch.dev_learning, &mut next.dev_learning),
        ("devDeploy", patch.dev_deploy, &mut next.dev_deploy),
    ] {
        if let Some(value) = value {
            let value = value.trim().to_string();
            if value.chars().count() > PROFILE_MAX {
                return invalid(field, Reason::Length);
            }
            *slot = value;
        }
    }
    if let Some(cost) = patch.dev_cost {
        next.dev_cost = cost;
    }
    if let Some(enabled) = patch.sound_enabled {
        next.sound_enabled = enabled;
    }
    if let Some(volume) = patch.sound_volume {
        if volume > 100 {
            return invalid("soundVolume", Reason::Format);
        }
        next.sound_volume = volume;
    }
    for (field, value, (low, high), slot) in [
        ("focusMinutes", patch.focus_minutes, FOCUS_MINUTES, &mut next.focus_minutes),
        ("breakMinutes", patch.break_minutes, BREAK_MINUTES, &mut next.break_minutes),
        ("longBreakMinutes", patch.long_break_minutes, LONG_BREAK_MINUTES, &mut next.long_break_minutes),
        ("longBreakEvery", patch.long_break_every, LONG_BREAK_EVERY, &mut next.long_break_every),
    ] {
        if let Some(value) = value {
            if !(low..=high).contains(&value) {
                return invalid(field, Reason::Format);
            }
            *slot = value;
        }
    }

    let changed = changed_values(&current, &next)?;
    if changed.is_empty() {
        return Ok((next, None));
    }
    let tx = conn.transaction()?;
    for (key, value) in &changed {
        tx.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value.to_string(), instant(now)],
        )?;
    }
    let keys: Vec<&str> = changed.iter().map(|(key, _)| *key).collect();
    activity::record(
        &tx,
        now,
        Record {
            actor,
            verb: Verb::Updated,
            entity: Entity::Settings,
            entity_id: None,
            summary: &keys.join(", "),
            details: serde_json::json!({ "keys": keys }),
        },
    )?;
    tx.commit()?;
    Ok((next, Some(Change::all(Entity::Settings))))
}

fn changed_values(
    current: &Settings,
    next: &Settings,
) -> Result<Vec<(&'static str, serde_json::Value)>> {
    let pairs = [
        ("owner_name", serde_json::to_value(&current.owner_name)?, serde_json::to_value(&next.owner_name)?),
        ("wake_time", serde_json::to_value(&current.wake_time)?, serde_json::to_value(&next.wake_time)?),
        ("sleep_time", serde_json::to_value(&current.sleep_time)?, serde_json::to_value(&next.sleep_time)?),
        ("theme_mode", serde_json::to_value(current.theme_mode)?, serde_json::to_value(next.theme_mode)?),
        ("ink", serde_json::to_value(current.ink)?, serde_json::to_value(next.ink)?),
        ("autostart", serde_json::to_value(current.autostart)?, serde_json::to_value(next.autostart)?),
        ("close_nudge", serde_json::to_value(current.close_nudge)?, serde_json::to_value(next.close_nudge)?),
        ("close_nudge_time", serde_json::to_value(&current.close_nudge_time)?, serde_json::to_value(&next.close_nudge_time)?),
        ("learning_goal_days", serde_json::to_value(current.learning_goal_days)?, serde_json::to_value(next.learning_goal_days)?),
        ("api_enabled", serde_json::to_value(current.api_enabled)?, serde_json::to_value(next.api_enabled)?),
        ("api_port", serde_json::to_value(current.api_port)?, serde_json::to_value(next.api_port)?),
        ("claude_enabled", serde_json::to_value(current.claude_enabled)?, serde_json::to_value(next.claude_enabled)?),
        ("gemini_model", serde_json::to_value(&current.gemini_model)?, serde_json::to_value(&next.gemini_model)?),
        ("dev_knows", serde_json::to_value(&current.dev_knows)?, serde_json::to_value(&next.dev_knows)?),
        ("dev_learning", serde_json::to_value(&current.dev_learning)?, serde_json::to_value(&next.dev_learning)?),
        ("dev_cost", serde_json::to_value(current.dev_cost)?, serde_json::to_value(next.dev_cost)?),
        ("dev_deploy", serde_json::to_value(&current.dev_deploy)?, serde_json::to_value(&next.dev_deploy)?),
        ("sound_enabled", serde_json::to_value(current.sound_enabled)?, serde_json::to_value(next.sound_enabled)?),
        ("sound_volume", serde_json::to_value(current.sound_volume)?, serde_json::to_value(next.sound_volume)?),
        ("focus_minutes", serde_json::to_value(current.focus_minutes)?, serde_json::to_value(next.focus_minutes)?),
        ("break_minutes", serde_json::to_value(current.break_minutes)?, serde_json::to_value(next.break_minutes)?),
        ("long_break_minutes", serde_json::to_value(current.long_break_minutes)?, serde_json::to_value(next.long_break_minutes)?),
        ("long_break_every", serde_json::to_value(current.long_break_every)?, serde_json::to_value(next.long_break_every)?),
    ];
    Ok(pairs
        .into_iter()
        .filter(|(_, before, after)| before != after)
        .map(|(key, _, after)| (key, after))
        .collect())
}

/// The Gemini key was saved (or replaced) or removed. The key itself lives in Windows Credential
/// Manager (hub-ai); Jejak only learns that it changed.
pub fn note_gemini_key(conn: &Connection, saved: bool, actor: Actor, now: Timestamp) -> Result<Change> {
    let verb = if saved { Verb::Updated } else { Verb::Deleted };
    activity::record(
        conn,
        now,
        Record { actor, verb, entity: Entity::Settings, entity_id: None, summary: "gemini_key", details: serde_json::json!({}) },
    )?;
    Ok(Change::all(Entity::Settings))
}

/// Store a value without an activity row: for what Hub decides itself on first run (autostart),
/// not for the owner's choices.
pub fn store_initial(conn: &Connection, key: &'static str, value: serde_json::Value, now: Timestamp) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (key) DO NOTHING",
        params![key, value.to_string(), instant(now)],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity;
    use crate::db::open_in_memory;
    use crate::error::Error;

    fn now() -> Timestamp {
        "2026-10-22T05:00:00Z".parse().unwrap()
    }

    #[test]
    fn defaults_until_changed() {
        let conn = open_in_memory().unwrap();
        assert_eq!(load(&conn).unwrap(), Settings::default());
    }

    #[test]
    fn a_change_is_stored_and_recorded_once() {
        let mut conn = open_in_memory().unwrap();
        let patch = SettingsPatch {
            ink: Some(Ink::Hijau),
            owner_name: Some("  Vio ".into()),
            ..Default::default()
        };
        let (settings, change) = update(&mut conn, patch, Actor::Owner, now()).unwrap();
        assert_eq!(settings.ink, Ink::Hijau);
        assert_eq!(settings.owner_name, "Vio");
        assert_eq!(change, Some(Change::all(Entity::Settings)));
        assert_eq!(load(&conn).unwrap(), settings);
        let rows = activity::recent(&conn, 10, true).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].summary, "owner_name, ink");

        // The same values again change nothing and record nothing.
        let again = SettingsPatch {
            ink: Some(Ink::Hijau),
            ..Default::default()
        };
        let (_, change) = update(&mut conn, again, Actor::Owner, now()).unwrap();
        assert_eq!(change, None);
        assert_eq!(activity::recent(&conn, 10, true).unwrap().len(), 1);
    }

    #[test]
    fn the_developer_profile_is_kept_trimmed_and_limited() {
        let mut conn = open_in_memory().unwrap();
        let patch = SettingsPatch {
            dev_knows: Some(" HTML, CSS, JavaScript, PHP dasar ".into()),
            dev_cost: Some(DevCost::Low),
            dev_deploy: Some("Laptop dulu, nanti VPS sekolah".into()),
            ..Default::default()
        };
        let (settings, _) = update(&mut conn, patch, Actor::Owner, now()).unwrap();
        assert_eq!(settings.dev_knows, "HTML, CSS, JavaScript, PHP dasar");
        assert_eq!((settings.dev_cost, settings.dev_learning.as_str()), (DevCost::Low, ""));
        assert_eq!(load(&conn).unwrap(), settings);
        let long = SettingsPatch { dev_learning: Some("x".repeat(301)), ..Default::default() };
        assert!(matches!(update(&mut conn, long, Actor::Owner, now()), Err(Error::Invalid { field: "devLearning", .. })));
    }

    #[test]
    fn sounds_are_on_and_quiet_until_the_owner_changes_them() {
        let mut conn = open_in_memory().unwrap();
        let first = load(&conn).unwrap();
        assert_eq!((first.sound_enabled, first.sound_volume), (true, DEFAULT_SOUND_VOLUME));
        let patch = SettingsPatch { sound_enabled: Some(false), sound_volume: Some(55), ..Default::default() };
        let (settings, _) = update(&mut conn, patch, Actor::Owner, now()).unwrap();
        assert_eq!((settings.sound_enabled, settings.sound_volume), (false, 55));
        assert_eq!(load(&conn).unwrap(), settings);
        let loud = SettingsPatch { sound_volume: Some(101), ..Default::default() };
        assert!(matches!(update(&mut conn, loud, Actor::Owner, now()), Err(Error::Invalid { field: "soundVolume", .. })));
    }

    #[test]
    fn bad_values_are_refused() {
        let mut conn = open_in_memory().unwrap();
        let cases = [
            (SettingsPatch { owner_name: Some("   ".into()), ..Default::default() }, "ownerName", Reason::Length),
            (SettingsPatch { owner_name: Some("x".repeat(41)), ..Default::default() }, "ownerName", Reason::Length),
            (SettingsPatch { wake_time: Some("5:00".into()), ..Default::default() }, "wakeTime", Reason::Format),
            (SettingsPatch { sleep_time: Some("09:00".into()), ..Default::default() }, "sleepTime", Reason::Order),
        ];
        for (patch, field, reason) in cases {
            match update(&mut conn, patch, Actor::Owner, now()) {
                Err(Error::Invalid { field: f, reason: r }) => {
                    assert_eq!((f, r), (field, reason));
                }
                other => panic!("expected Invalid for {field}, got {other:?}"),
            }
        }
        assert_eq!(load(&conn).unwrap(), Settings::default());
    }

    #[test]
    fn unknown_fields_are_refused() {
        let parsed: std::result::Result<SettingsPatch, _> =
            serde_json::from_str(r#"{ "ink": "biru", "dataDir": "C:\\" }"#);
        assert!(parsed.is_err());
    }

    #[test]
    fn the_gemini_model_is_an_id_and_the_key_change_leaves_no_trace_of_the_key() {
        let mut conn = crate::db::open_in_memory().unwrap();
        let now: Timestamp = "2026-11-06T03:00:00Z".parse().unwrap();
        assert_eq!(load(&conn).unwrap().gemini_model, DEFAULT_GEMINI_MODEL);
        let patch = |model: &str| SettingsPatch { gemini_model: Some(model.into()), ..Default::default() };
        update(&mut conn, patch(" gemini-3.5-flash-lite "), Actor::Owner, now).unwrap();
        assert_eq!(load(&conn).unwrap().gemini_model, "gemini-3.5-flash-lite");
        for bad in ["", "../models", "Gemini 3", "gemini?x=1"] {
            assert!(matches!(update(&mut conn, patch(bad), Actor::Owner, now), Err(crate::Error::Invalid { field: "geminiModel", .. })), "{bad}");
        }
        note_gemini_key(&conn, true, Actor::Owner, now).unwrap();
        note_gemini_key(&conn, false, Actor::Owner, now).unwrap();
        let rows: Vec<(String, String)> = conn
            .prepare("SELECT verb, summary FROM activity WHERE summary = 'gemini_key' ORDER BY id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(rows, [("updated".to_string(), "gemini_key".to_string()), ("deleted".to_string(), "gemini_key".to_string())]);
    }

    #[test]
    fn a_value_from_a_future_version_falls_back() {
        let conn = open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES ('ink', '\"emas\"', 'x')",
            [],
        )
        .unwrap();
        assert_eq!(load(&conn).unwrap().ink, Ink::Biru);
    }

    #[test]
    fn first_run_values_do_not_overwrite_choices() {
        let mut conn = open_in_memory().unwrap();
        assert!(!is_stored(&conn, "autostart").unwrap());
        let patch = SettingsPatch { autostart: Some(false), ..Default::default() };
        update(&mut conn, patch, Actor::Owner, now()).unwrap();
        store_initial(&conn, "autostart", serde_json::json!(true), now()).unwrap();
        assert!(!load(&conn).unwrap().autostart);
    }
}
