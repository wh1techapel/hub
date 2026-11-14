//! Sesi Fokus (Lentera CP02): a Pomodoro Hub keeps itself, offline, for a task, a Belajar item, or
//! nothing in particular (CONTEXT.md, "Lentera on the desktop").
//!
//! One session runs at a time (`focus_run`). A focus block ends in a break that starts by itself
//! (systematic breaks, Biwer et al. 2023); a break ends in waiting, and the owner starts the next
//! block. Every few blocks the break is a long one. The phase is never ticked: it is read from the
//! times stored, so it survives a restart, and `settle` moves it on whenever Hub looks (the
//! scheduler wakes at each phase's end).
//!
//! Hub counts only time it saw. When it looks again after a gap (the computer asleep, Hub closed),
//! a focus block is paused where Hub last saw it and a break counts as over. A session left paused
//! or waiting for half an hour ends by itself.
//!
//! Each finished block goes into `focus_blocks` and Jejak. A block on a Belajar item is also that
//! item's learning session, written once, so the week in Belajar counts it; Belajar's own timer and
//! a session never run together.

use jiff::{SignedDuration, Timestamp};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::activity::{self, Record};
use crate::error::{invalid, Error, Reason, Result};
use crate::ids::{self, new_id};
use crate::learning;
use crate::model::{Actor, Change, Entity, FocusPhase, LearningStatus, Verb};
use crate::settings::Settings;
use crate::time::{instant, parse_instant};

/// A gap longer than this since Hub last looked is time Hub did not see. The scheduler looks at
/// least every 30 s while a session runs.
pub const UNSEEN_SECONDS: i64 = 120;
/// A session paused or waiting this long ends by itself.
pub const IDLE_LIMIT_SECONDS: i64 = 30 * 60;
/// A block stopped early is kept when it lasted at least this.
const BLOCK_MIN_SECONDS: i64 = 60;

/// The lengths of a session's phases, in seconds, and how many blocks come before a long break.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lengths {
    pub focus: i64,
    pub short_break: i64,
    pub long_break: i64,
    pub long_every: i64,
}

impl Lengths {
    /// From Pengaturan. `seconds_per_minute` is 60, except in a smoke of the real window, which
    /// makes a minute last a second.
    pub fn from_settings(settings: &Settings, seconds_per_minute: i64) -> Lengths {
        let unit = seconds_per_minute.max(1);
        Lengths {
            focus: i64::from(settings.focus_minutes) * unit,
            short_break: i64::from(settings.break_minutes) * unit,
            long_break: i64::from(settings.long_break_minutes) * unit,
            long_every: i64::from(settings.long_break_every),
        }
    }
}

/// What a session is for: a task, a Belajar item, or neither.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct FocusStart {
    #[ts(optional)]
    pub task_id: Option<String>,
    #[ts(optional)]
    pub item_id: Option<String>,
}

/// The running session as the windows show it. The time left is worked out by the window from
/// `endsAt` (or, paused, from `pausedAt`), so nothing ticks here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FocusView {
    pub phase: FocusPhase,
    /// The task's or item's title when the session started; empty for a session for nothing.
    pub title: String,
    pub task_id: Option<String>,
    pub item_id: Option<String>,
    /// When the phase began, moved on by any pause.
    pub phase_started_at: String,
    pub phase_seconds: u32,
    pub paused_at: Option<String>,
    /// When the phase ends (a block or a break that is running), else none.
    pub ends_at: Option<String>,
    pub blocks_done: u32,
    pub long_break_every: u32,
    /// Focus time kept so far, in seconds.
    pub focused_seconds: u32,
    /// The owner tucked the capsule away until the next phase.
    pub tucked: bool,
    pub started_at: String,
}

/// Something that happened by itself when Hub looked, for the desktop (a sound, the capsule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turn {
    /// A block ended and its break began.
    Break { long: bool },
    /// A break ended: the next block waits for the owner.
    BreakOver,
    /// Hub did not see the time pass, so the block waits where Hub last saw it.
    Paused,
    /// Paused or waiting too long: the session ended.
    Ended,
}

/// What `settle` did.
#[derive(Debug, Default)]
pub struct Settled {
    pub turns: Vec<Turn>,
    pub changes: Vec<Change>,
    /// When Hub should look again for this session (a phase's end, or its idle limit).
    pub next_at: Option<Timestamp>,
}

#[derive(Debug, Clone, PartialEq)]
struct Run {
    task_id: Option<String>,
    item_id: Option<String>,
    title: String,
    phase: FocusPhase,
    phase_started_at: Timestamp,
    phase_seconds: i64,
    paused_at: Option<Timestamp>,
    seen_at: Timestamp,
    blocks_done: i64,
    focused_seconds: i64,
    tucked: bool,
    started_at: Timestamp,
    lengths: Lengths,
}

impl Run {
    fn running(&self) -> bool {
        self.paused_at.is_none() && self.phase != FocusPhase::Waiting
    }

    fn ends_at(&self) -> Result<Option<Timestamp>> {
        if !self.running() {
            return Ok(None);
        }
        Ok(Some(plus(self.phase_started_at, self.phase_seconds)?))
    }

    /// Where an idle session ends by itself: paused or waiting for the idle limit.
    fn idle_end(&self) -> Result<Option<Timestamp>> {
        let since = match (self.paused_at, self.phase) {
            (Some(paused), _) => paused,
            (None, FocusPhase::Waiting) => self.phase_started_at,
            _ => return Ok(None),
        };
        Ok(Some(plus(since, IDLE_LIMIT_SECONDS)?))
    }

    fn view(&self) -> Result<FocusView> {
        let seconds = |n: i64| u32::try_from(n.max(0)).unwrap_or(u32::MAX);
        Ok(FocusView {
            phase: self.phase,
            title: self.title.clone(),
            task_id: self.task_id.clone(),
            item_id: self.item_id.clone(),
            phase_started_at: instant(self.phase_started_at),
            phase_seconds: seconds(self.phase_seconds),
            paused_at: self.paused_at.map(instant),
            ends_at: self.ends_at()?.map(instant),
            blocks_done: seconds(self.blocks_done),
            long_break_every: seconds(self.lengths.long_every),
            focused_seconds: seconds(self.focused_seconds),
            tucked: self.tucked,
            started_at: instant(self.started_at),
        })
    }
}

fn plus(at: Timestamp, seconds: i64) -> Result<Timestamp> {
    Ok(at.checked_add(SignedDuration::from_secs(seconds))?)
}

fn between(from: Timestamp, to: Timestamp) -> i64 {
    to.as_second() - from.as_second()
}

fn load(conn: &Connection) -> Result<Option<Run>> {
    let row = conn
        .query_row(
            "SELECT task_id, item_id, title, phase, phase_started_at, phase_seconds, paused_at, seen_at, blocks_done,
                    focused_seconds, tucked, started_at, focus_seconds, break_seconds, long_break_seconds, long_break_every
             FROM focus_run WHERE id = 1",
            [],
            |row| {
                Ok((
                    (row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, String>(2)?, row.get::<_, FocusPhase>(3)?),
                    (row.get::<_, String>(4)?, row.get::<_, i64>(5)?, row.get::<_, Option<String>>(6)?, row.get::<_, String>(7)?),
                    (row.get::<_, i64>(8)?, row.get::<_, i64>(9)?, row.get::<_, bool>(10)?, row.get::<_, String>(11)?),
                    (row.get::<_, i64>(12)?, row.get::<_, i64>(13)?, row.get::<_, i64>(14)?, row.get::<_, i64>(15)?),
                ))
            },
        )
        .optional()?;
    let Some(((task_id, item_id, title, phase), (phase_started_at, phase_seconds, paused_at, seen_at), (blocks_done, focused_seconds, tucked, started_at), lengths)) = row
    else {
        return Ok(None);
    };
    Ok(Some(Run {
        task_id,
        item_id,
        title,
        phase,
        phase_started_at: parse_instant("phaseStartedAt", &phase_started_at)?,
        phase_seconds,
        paused_at: paused_at.as_deref().map(|at| parse_instant("pausedAt", at)).transpose()?,
        seen_at: parse_instant("seenAt", &seen_at)?,
        blocks_done,
        focused_seconds,
        tucked,
        started_at: parse_instant("startedAt", &started_at)?,
        lengths: Lengths { focus: lengths.0, short_break: lengths.1, long_break: lengths.2, long_every: lengths.3 },
    }))
}

fn save(tx: &Transaction<'_>, run: &Run) -> Result<()> {
    tx.execute(
        "UPDATE focus_run SET phase = ?1, phase_started_at = ?2, phase_seconds = ?3, paused_at = ?4, seen_at = ?5,
                blocks_done = ?6, focused_seconds = ?7, tucked = ?8 WHERE id = 1",
        params![
            run.phase,
            instant(run.phase_started_at),
            run.phase_seconds,
            run.paused_at.map(instant),
            instant(run.seen_at),
            run.blocks_done,
            run.focused_seconds,
            run.tucked,
        ],
    )?;
    Ok(())
}

/// The running session, if any, as it was when Hub last looked.
pub fn view(conn: &Connection) -> Result<Option<FocusView>> {
    load(conn)?.map(|run| run.view()).transpose()
}

/// A block kept: its row, and Jejak's (a block on a Belajar item is that item's session instead).
fn keep_block(tx: &Transaction<'_>, run: &Run, started: Timestamp, seconds: i64, completed: bool, at: Timestamp, changes: &mut Vec<Change>) -> Result<()> {
    let id = new_id();
    let minutes = u32::try_from(((seconds + 30) / 60).max(1)).unwrap_or(u32::MAX);
    let mut session_id = None;
    if let Some(item) = &run.item_id {
        let alive: bool = tx
            .query_row("SELECT deleted_at IS NULL FROM learning_items WHERE id = ?1", [item], |row| row.get(0))
            .optional()?
            .unwrap_or(false);
        if alive {
            session_id = Some(learning::insert_session(tx, item, started, minutes, "", Actor::Owner, at)?);
            changes.push(Change::one(Entity::LearningItem, item.clone()));
            changes.push(Change::all(Entity::LearningSession));
        }
    }
    tx.execute(
        "INSERT INTO focus_blocks (id, started_at, seconds, planned_seconds, completed, task_id, item_id, session_id, title, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![id, instant(started), seconds, run.lengths.focus, completed, run.task_id, run.item_id, session_id, run.title, instant(at)],
    )?;
    if session_id.is_none() && completed {
        activity::record(
            tx,
            at,
            Record {
                actor: Actor::Owner,
                verb: Verb::Completed,
                entity: Entity::FocusSession,
                entity_id: Some(&id),
                summary: &run.title,
                details: serde_json::json!({ "minutes": minutes, "taskId": run.task_id }),
            },
        )?;
    }
    Ok(())
}

/// Move the session on to `at`: a block that ended becomes its break, a break that ended waits.
fn advance(tx: &Transaction<'_>, run: &mut Run, at: Timestamp, turns: &mut Vec<Turn>, changes: &mut Vec<Change>) -> Result<()> {
    while let Some(end) = run.ends_at()? {
        if at < end {
            break;
        }
        match run.phase {
            FocusPhase::Focus => {
                keep_block(tx, run, run.phase_started_at, run.phase_seconds, true, end, changes)?;
                run.blocks_done += 1;
                run.focused_seconds += run.phase_seconds;
                let long = run.lengths.long_every > 0 && run.blocks_done % run.lengths.long_every == 0;
                run.phase = if long { FocusPhase::LongBreak } else { FocusPhase::ShortBreak };
                run.phase_seconds = if long { run.lengths.long_break } else { run.lengths.short_break };
                turns.push(Turn::Break { long });
            }
            FocusPhase::ShortBreak | FocusPhase::LongBreak => {
                run.phase = FocusPhase::Waiting;
                run.phase_seconds = 0;
                turns.push(Turn::BreakOver);
            }
            FocusPhase::Waiting => break,
        }
        run.phase_started_at = end;
        run.tucked = false;
    }
    Ok(())
}

/// End the session at `at`: a block under way is kept if it lasted a minute, and Jejak says how
/// much was done.
fn end(tx: &Transaction<'_>, run: &Run, actor: Actor, at: Timestamp, changes: &mut Vec<Change>) -> Result<()> {
    let mut focused = run.focused_seconds;
    let mut blocks = run.blocks_done;
    if run.phase == FocusPhase::Focus {
        let until = run.paused_at.unwrap_or(at);
        let done = between(run.phase_started_at, until).clamp(0, run.phase_seconds);
        if done >= BLOCK_MIN_SECONDS {
            keep_block(tx, run, run.phase_started_at, done, false, until, changes)?;
            focused += done;
            blocks += 1;
        }
    }
    tx.execute("DELETE FROM focus_run", [])?;
    activity::record(
        tx,
        at,
        Record {
            actor,
            verb: Verb::Closed,
            entity: Entity::FocusSession,
            entity_id: None,
            summary: &run.title,
            details: serde_json::json!({ "blocks": blocks, "minutes": (focused + 30) / 60, "taskId": run.task_id, "itemId": run.item_id }),
        },
    )?;
    changes.push(Change::all(Entity::FocusSession));
    Ok(())
}

/// Bring the session up to `now` inside a transaction. None when no session runs (any more).
fn settle_in(tx: &Transaction<'_>, now: Timestamp, turns: &mut Vec<Turn>, changes: &mut Vec<Change>) -> Result<Option<Run>> {
    let Some(mut run) = load(tx)? else { return Ok(None) };
    let before = run.clone();
    // Time Hub did not see: the phases go on up to the moment Hub last looked, and stop there.
    if run.running() && between(run.seen_at, now) > UNSEEN_SECONDS {
        let seen = run.seen_at;
        advance(tx, &mut run, seen, turns, changes)?;
        match run.phase {
            FocusPhase::Focus if run.paused_at.is_none() => {
                run.paused_at = Some(seen);
                turns.push(Turn::Paused);
            }
            FocusPhase::ShortBreak | FocusPhase::LongBreak => {
                run.phase = FocusPhase::Waiting;
                run.phase_seconds = 0;
                run.phase_started_at = now;
                run.tucked = false;
                turns.push(Turn::BreakOver);
            }
            _ => {}
        }
    }
    advance(tx, &mut run, now, turns, changes)?;
    if let Some(idle_end) = run.idle_end()?
        && now >= idle_end
    {
        end(tx, &run, Actor::Hub, idle_end, changes)?;
        turns.push(Turn::Ended);
        return Ok(None);
    }
    run.seen_at = now;
    save(tx, &run)?;
    if run.phase != before.phase || run.paused_at != before.paused_at || run.blocks_done != before.blocks_done {
        changes.push(Change::all(Entity::FocusSession));
    }
    Ok(Some(run))
}

/// Look at the session now (the scheduler, at every phase's end and at least every 30 s).
pub fn settle(conn: &mut Connection, now: Timestamp) -> Result<Settled> {
    let tx = conn.transaction()?;
    let mut settled = Settled::default();
    let run = settle_in(&tx, now, &mut settled.turns, &mut settled.changes)?;
    tx.commit()?;
    if let Some(run) = run {
        settled.next_at = run.ends_at()?.or(run.idle_end()?);
    }
    dedupe(&mut settled.changes);
    Ok(settled)
}

fn dedupe(changes: &mut Vec<Change>) {
    let mut seen = Vec::with_capacity(changes.len());
    changes.retain(|change| {
        let fresh = !seen.contains(change);
        if fresh {
            seen.push(change.clone());
        }
        fresh
    });
}

/// End any session running (another session starts, or Belajar's own timer does).
pub(crate) fn stop_in(tx: &Transaction<'_>, actor: Actor, now: Timestamp) -> Result<Vec<Change>> {
    let mut turns = Vec::new();
    let mut changes = Vec::new();
    if let Some(run) = settle_in(tx, now, &mut turns, &mut changes)? {
        end(tx, &run, actor, now, &mut changes)?;
    }
    Ok(changes)
}

/// The task's or item's title, checked; a session for nothing has none.
fn target(tx: &Transaction<'_>, input: &FocusStart) -> Result<(Option<String>, Option<String>, String)> {
    match (&input.task_id, &input.item_id) {
        (Some(_), Some(_)) => invalid("itemId", Reason::Format),
        (Some(task), None) => {
            let task = ids::parse("taskId", task)?;
            let title: String = tx
                .query_row("SELECT title FROM tasks WHERE id = ?1 AND deleted_at IS NULL", [&task], |row| row.get(0))
                .optional()?
                .ok_or(Error::NotFound("task"))?;
            Ok((Some(task), None, title))
        }
        (None, Some(item)) => {
            let item = ids::parse("itemId", item)?;
            let title: String = tx
                .query_row("SELECT title FROM learning_items WHERE id = ?1 AND deleted_at IS NULL", [&item], |row| row.get(0))
                .optional()?
                .ok_or(Error::NotFound("learning item"))?;
            Ok((None, Some(item), title))
        }
        (None, None) => Ok((None, None, String::new())),
    }
}

/// Start a session: its first block begins now. A session already running ends first, and so does
/// Belajar's timer; an item not started yet becomes "doing".
pub fn start(conn: &mut Connection, input: FocusStart, lengths: Lengths, actor: Actor, now: Timestamp) -> Result<(FocusView, Vec<Change>)> {
    if lengths.focus <= 0 || lengths.short_break <= 0 || lengths.long_break <= 0 || lengths.long_every <= 0 {
        return invalid("focusMinutes", Reason::Format);
    }
    let tx = conn.transaction()?;
    let (task_id, item_id, title) = target(&tx, &input)?;
    let mut changes = stop_in(&tx, actor, now)?;
    if let Some((stopped, _)) = learning::stop_in(&tx, actor, now)? {
        changes.push(Change::one(Entity::LearningItem, stopped));
        changes.push(Change::all(Entity::LearningSession));
    }
    if let Some(item) = &item_id {
        let status: LearningStatus = tx.query_row("SELECT status FROM learning_items WHERE id = ?1", [item], |row| row.get(0))?;
        if status == LearningStatus::Todo {
            learning::apply_status(&tx, item, LearningStatus::Doing, now)?;
            changes.push(Change::one(Entity::LearningItem, item.clone()));
        }
    }
    let run = Run {
        task_id,
        item_id,
        title,
        phase: FocusPhase::Focus,
        phase_started_at: now,
        phase_seconds: lengths.focus,
        paused_at: None,
        seen_at: now,
        blocks_done: 0,
        focused_seconds: 0,
        tucked: false,
        started_at: now,
        lengths,
    };
    tx.execute(
        "INSERT INTO focus_run (id, task_id, item_id, title, phase, phase_started_at, phase_seconds, seen_at, started_at,
                                focus_seconds, break_seconds, long_break_seconds, long_break_every)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?5, ?5, ?7, ?8, ?9, ?10)",
        params![
            run.task_id,
            run.item_id,
            run.title,
            run.phase,
            instant(now),
            run.phase_seconds,
            lengths.focus,
            lengths.short_break,
            lengths.long_break,
            lengths.long_every,
        ],
    )?;
    tx.commit()?;
    changes.push(Change::all(Entity::FocusSession));
    dedupe(&mut changes);
    Ok((run.view()?, changes))
}

/// An action on the running session: brought up to now first, then changed by `act`.
fn act(conn: &mut Connection, now: Timestamp, act: impl FnOnce(&mut Run)) -> Result<Vec<Change>> {
    let tx = conn.transaction()?;
    let mut turns = Vec::new();
    let mut changes = Vec::new();
    let Some(mut run) = settle_in(&tx, now, &mut turns, &mut changes)? else {
        tx.commit()?;
        return if changes.is_empty() { Err(Error::NotFound("focus session")) } else { Ok(changes) };
    };
    act(&mut run);
    save(&tx, &run)?;
    tx.commit()?;
    changes.push(Change::all(Entity::FocusSession));
    dedupe(&mut changes);
    Ok(changes)
}

/// Jeda: a running block waits.
pub fn pause(conn: &mut Connection, now: Timestamp) -> Result<Vec<Change>> {
    act(conn, now, |run| {
        if run.phase == FocusPhase::Focus && run.paused_at.is_none() {
            run.paused_at = Some(now);
        }
    })
}

/// Lanjut: a paused block goes on where it stopped.
pub fn resume(conn: &mut Connection, now: Timestamp) -> Result<Vec<Change>> {
    act(conn, now, |run| {
        if let Some(paused) = run.paused_at.take()
            && let Ok(moved) = plus(run.phase_started_at, between(paused, now).max(0))
        {
            run.phase_started_at = moved;
        }
    })
}

/// Lanjut fokus after a break, or Lewati istirahat during one: the next block starts now.
pub fn next(conn: &mut Connection, now: Timestamp) -> Result<Vec<Change>> {
    act(conn, now, |run| {
        if run.phase != FocusPhase::Focus {
            run.phase = FocusPhase::Focus;
            run.phase_started_at = now;
            run.phase_seconds = run.lengths.focus;
            run.paused_at = None;
            run.tucked = false;
        }
    })
}

/// Selipkan: the capsule steps away until the next phase (or comes back).
pub fn tuck(conn: &mut Connection, tucked: bool, now: Timestamp) -> Result<Vec<Change>> {
    act(conn, now, |run| run.tucked = tucked)
}

/// Hentikan: the session ends now.
pub fn stop(conn: &mut Connection, actor: Actor, now: Timestamp) -> Result<Vec<Change>> {
    let tx = conn.transaction()?;
    let mut changes = stop_in(&tx, actor, now)?;
    tx.commit()?;
    dedupe(&mut changes);
    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::learning::{self, LearningInput};
    use crate::model::Domain;
    use crate::tasks::{self, TaskInput};

    const LENGTHS: Lengths = Lengths { focus: 25 * 60, short_break: 5 * 60, long_break: 15 * 60, long_every: 4 };

    fn at(minute: i64) -> Timestamp {
        plus("2026-11-10T02:00:00Z".parse().unwrap(), minute * 60).unwrap()
    }

    fn task(conn: &mut Connection, title: &str) -> String {
        let input = TaskInput { title: title.into(), ..Default::default() };
        tasks::create(conn, input, Actor::Owner, at(0)).unwrap().0
    }

    fn item(conn: &mut Connection) -> String {
        learning::ensure_default_tracks(conn, at(0)).unwrap();
        let track = learning::tracks(conn).unwrap().into_iter().next().unwrap().id;
        let input = LearningInput { track_id: track, title: "THM: Nmap".into(), domain: Some(Domain::Network), ..Default::default() };
        learning::create_item(conn, input, Actor::Owner, at(0)).unwrap().0
    }

    fn for_task(id: &str) -> FocusStart {
        FocusStart { task_id: Some(id.into()), item_id: None }
    }

    /// Settle every 30 s up to `minute`, as the scheduler would.
    fn run_until(conn: &mut Connection, from: i64, minute: i64) -> Vec<Turn> {
        let mut turns = Vec::new();
        let mut second = from * 60;
        while second <= minute * 60 {
            let now = plus(at(0), second).unwrap();
            turns.extend(settle(conn, now).unwrap().turns);
            second += 30;
        }
        turns
    }

    fn blocks(conn: &Connection) -> Vec<(i64, bool)> {
        let mut statement = conn.prepare("SELECT seconds, completed FROM focus_blocks ORDER BY started_at").unwrap();
        statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?))).unwrap().map(|row| row.unwrap()).collect()
    }

    fn jejak(conn: &Connection) -> Vec<(String, String)> {
        let mut statement = conn
            .prepare("SELECT verb, summary FROM activity WHERE entity_type = 'focus_session' ORDER BY at, rowid")
            .unwrap();
        statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?))).unwrap().map(|row| row.unwrap()).collect()
    }

    #[test]
    fn a_block_becomes_its_break_by_itself_and_the_break_waits_for_the_owner() {
        let mut conn = open_in_memory().unwrap();
        let id = task(&mut conn, "Laporan lab");
        let (view, changes) = start(&mut conn, for_task(&id), LENGTHS, Actor::Owner, at(0)).unwrap();
        assert_eq!(view.phase, FocusPhase::Focus);
        assert_eq!(view.title, "Laporan lab");
        assert_eq!(view.ends_at.as_deref(), Some(instant(at(25)).as_str()));
        assert!(changes.contains(&Change::all(Entity::FocusSession)));

        let turns = run_until(&mut conn, 0, 25);
        assert_eq!(turns, vec![Turn::Break { long: false }]);
        let now = view_of(&conn);
        assert_eq!((now.phase, now.blocks_done, now.focused_seconds), (FocusPhase::ShortBreak, 1, 1500));
        assert_eq!(now.ends_at.as_deref(), Some(instant(at(30)).as_str()));
        assert_eq!(blocks(&conn), vec![(1500, true)]);
        assert_eq!(jejak(&conn), vec![("completed".into(), "Laporan lab".into())]);

        assert_eq!(run_until(&mut conn, 25, 31), vec![Turn::BreakOver]);
        let waiting = view_of(&conn);
        assert_eq!((waiting.phase, waiting.ends_at), (FocusPhase::Waiting, None));
        // Waiting counts nothing; the owner starts the next block.
        next(&mut conn, at(33)).unwrap();
        let again = view_of(&conn);
        assert_eq!((again.phase, again.ends_at.clone()), (FocusPhase::Focus, Some(instant(at(58)))));
    }

    fn view_of(conn: &Connection) -> FocusView {
        view(conn).unwrap().expect("a session")
    }

    #[test]
    fn every_fourth_block_ends_in_a_long_break() {
        let mut conn = open_in_memory().unwrap();
        start(&mut conn, FocusStart::default(), LENGTHS, Actor::Owner, at(0)).unwrap();
        let mut minute = 0;
        let mut breaks = Vec::new();
        for _ in 0..4 {
            breaks.extend(run_until(&mut conn, minute, minute + 25));
            minute += 25;
            let length = i64::from(view_of(&conn).phase_seconds) / 60;
            breaks.extend(run_until(&mut conn, minute, minute + length));
            minute += length;
            next(&mut conn, at(minute)).unwrap();
        }
        breaks.retain(|turn| matches!(turn, Turn::Break { .. }));
        assert_eq!(
            breaks,
            vec![Turn::Break { long: false }, Turn::Break { long: false }, Turn::Break { long: false }, Turn::Break { long: true }]
        );
        assert_eq!(view_of(&conn).blocks_done, 4);
    }

    #[test]
    fn a_pause_moves_the_end_by_as_long_as_it_lasted() {
        let mut conn = open_in_memory().unwrap();
        start(&mut conn, FocusStart::default(), LENGTHS, Actor::Owner, at(0)).unwrap();
        run_until(&mut conn, 0, 10);
        pause(&mut conn, at(10)).unwrap();
        let paused = view_of(&conn);
        assert_eq!((paused.paused_at.clone(), paused.ends_at.clone()), (Some(instant(at(10))), None));
        // Paused, nothing moves on, however often Hub looks.
        assert!(run_until(&mut conn, 10, 20).is_empty());
        resume(&mut conn, at(20)).unwrap();
        assert_eq!(view_of(&conn).ends_at, Some(instant(at(35))));
        // A break cannot be paused: only a block can.
        assert!(run_until(&mut conn, 20, 35).contains(&Turn::Break { long: false }));
        pause(&mut conn, at(36)).unwrap();
        assert_eq!(view_of(&conn).paused_at, None);
    }

    #[test]
    fn time_hub_did_not_see_pauses_a_block_where_hub_last_looked() {
        let mut conn = open_in_memory().unwrap();
        start(&mut conn, FocusStart::default(), LENGTHS, Actor::Owner, at(0)).unwrap();
        run_until(&mut conn, 0, 5);
        // The lid closed at minute 5; Hub looks again at minute 15.
        let settled = settle(&mut conn, at(15)).unwrap();
        assert_eq!(settled.turns, vec![Turn::Paused]);
        let view = view_of(&conn);
        assert_eq!(view.paused_at, Some(instant(at(5))));
        assert!(blocks(&conn).is_empty(), "the 25 minutes did not pass for the owner");
        resume(&mut conn, at(16)).unwrap();
        assert_eq!(view_of(&conn).ends_at, Some(instant(at(36))), "20 minutes were left");
    }

    #[test]
    fn a_break_hub_did_not_see_is_over() {
        let mut conn = open_in_memory().unwrap();
        start(&mut conn, FocusStart::default(), LENGTHS, Actor::Owner, at(0)).unwrap();
        run_until(&mut conn, 0, 26);
        let settled = settle(&mut conn, at(60)).unwrap();
        assert_eq!(settled.turns, vec![Turn::BreakOver]);
        let view = view_of(&conn);
        assert_eq!(view.phase, FocusPhase::Waiting);
        // The idle limit counts from the owner's return, not from the break's planned end.
        assert_eq!(settled.next_at, Some(at(90)));
    }

    #[test]
    fn a_session_left_waiting_or_paused_for_half_an_hour_ends_by_itself() {
        let mut conn = open_in_memory().unwrap();
        start(&mut conn, FocusStart::default(), LENGTHS, Actor::Owner, at(0)).unwrap();
        run_until(&mut conn, 0, 30);
        assert_eq!(view_of(&conn).phase, FocusPhase::Waiting);
        let turns = run_until(&mut conn, 30, 61);
        assert_eq!(turns, vec![Turn::Ended]);
        assert!(view(&conn).unwrap().is_none());
        let rows = jejak(&conn);
        assert_eq!(rows.last().map(|(verb, _)| verb.as_str()), Some("closed"));
        let actor: String = conn
            .query_row("SELECT actor FROM activity WHERE verb = 'closed' AND entity_type = 'focus_session'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(actor, "hub");

        start(&mut conn, FocusStart::default(), LENGTHS, Actor::Owner, at(100)).unwrap();
        run_until(&mut conn, 100, 110);
        pause(&mut conn, at(110)).unwrap();
        assert_eq!(run_until(&mut conn, 110, 141), vec![Turn::Ended]);
        assert_eq!(blocks(&conn).last(), Some(&(600, false)), "the 10 minutes before the pause are kept");
    }

    #[test]
    fn stopping_keeps_a_block_of_at_least_a_minute() {
        let mut conn = open_in_memory().unwrap();
        start(&mut conn, FocusStart::default(), LENGTHS, Actor::Owner, at(0)).unwrap();
        stop(&mut conn, Actor::Owner, plus(at(0), 40).unwrap()).unwrap();
        assert!(blocks(&conn).is_empty(), "40 seconds is not a block");
        start(&mut conn, FocusStart::default(), LENGTHS, Actor::Owner, at(10)).unwrap();
        run_until(&mut conn, 10, 22);
        let changes = stop(&mut conn, Actor::Owner, at(22)).unwrap();
        assert_eq!(changes, vec![Change::all(Entity::FocusSession)]);
        assert_eq!(blocks(&conn), vec![(720, false)]);
        assert!(view(&conn).unwrap().is_none());
        // Stopping with nothing running changes nothing.
        assert!(stop(&mut conn, Actor::Owner, at(30)).unwrap().is_empty());
        assert!(matches!(pause(&mut conn, at(30)), Err(Error::NotFound(_))));
    }

    #[test]
    fn a_new_session_ends_the_one_running() {
        let mut conn = open_in_memory().unwrap();
        let a = task(&mut conn, "A");
        let b = task(&mut conn, "B");
        start(&mut conn, for_task(&a), LENGTHS, Actor::Owner, at(0)).unwrap();
        run_until(&mut conn, 0, 5);
        start(&mut conn, for_task(&b), LENGTHS, Actor::Owner, at(5)).unwrap();
        assert_eq!(view_of(&conn).title, "B");
        assert_eq!(blocks(&conn), vec![(300, false)]);
    }

    #[test]
    fn a_block_on_a_belajar_item_is_its_learning_session_and_the_two_clocks_never_overlap() {
        let mut conn = open_in_memory().unwrap();
        let nmap = item(&mut conn);
        // Belajar's timer was running: the session stops it into its own session first.
        learning::start_timer(&mut conn, &nmap, Actor::Owner, at(0)).unwrap();
        start(&mut conn, FocusStart { task_id: None, item_id: Some(nmap.clone()) }, LENGTHS, Actor::Owner, at(10)).unwrap();
        assert!(learning::timer(&conn).unwrap().is_none());
        run_until(&mut conn, 10, 35);
        let sessions = learning::sessions(&conn, &nmap).unwrap();
        let minutes: Vec<u32> = sessions.iter().map(|s| s.duration_min).collect();
        assert_eq!(minutes, vec![25, 10], "the block, then the timer's ten minutes");
        let linked: Option<String> = conn.query_row("SELECT session_id FROM focus_blocks", [], |row| row.get(0)).unwrap();
        assert_eq!(linked.as_deref(), Some(sessions[0].id.as_str()));
        assert!(jejak(&conn).is_empty(), "Jejak has it once, as the learning session");
        // Belajar's timer started again ends the session.
        learning::start_timer(&mut conn, &nmap, Actor::Owner, at(36)).unwrap();
        assert!(view(&conn).unwrap().is_none());
    }

    #[test]
    fn a_session_is_for_one_thing_that_exists() {
        let mut conn = open_in_memory().unwrap();
        let id = task(&mut conn, "A");
        let both = FocusStart { task_id: Some(id.clone()), item_id: Some(id.clone()) };
        assert!(start(&mut conn, both, LENGTHS, Actor::Owner, at(0)).is_err());
        let missing = for_task("4b0c1a3e-0000-4000-8000-000000000000");
        assert!(matches!(start(&mut conn, missing, LENGTHS, Actor::Owner, at(0)), Err(Error::NotFound("task"))));
        let zero = Lengths { focus: 0, ..LENGTHS };
        assert!(start(&mut conn, for_task(&id), zero, Actor::Owner, at(0)).is_err());
    }

    #[test]
    fn tucking_lasts_until_the_next_phase() {
        let mut conn = open_in_memory().unwrap();
        start(&mut conn, FocusStart::default(), LENGTHS, Actor::Owner, at(0)).unwrap();
        tuck(&mut conn, true, at(1)).unwrap();
        assert!(view_of(&conn).tucked);
        run_until(&mut conn, 1, 25);
        assert!(!view_of(&conn).tucked, "the break brings the capsule back");
    }

    #[test]
    fn the_lengths_come_from_pengaturan() {
        let settings = Settings { focus_minutes: 50, break_minutes: 10, long_break_minutes: 20, long_break_every: 3, ..Settings::default() };
        assert_eq!(Lengths::from_settings(&settings, 60), Lengths { focus: 3000, short_break: 600, long_break: 1200, long_every: 3 });
        // A smoke of the real window makes a minute a second.
        assert_eq!(Lengths::from_settings(&Settings::default(), 1), Lengths { focus: 25, short_break: 5, long_break: 15, long_every: 4 });
    }
}
