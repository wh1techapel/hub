//! Lentera's brain in the bar (CP03): the owner's line, or a cleaned group message (Dari Pesan),
//! goes to Gemini with the least that makes the answer right; Gemini answers with proposals; the
//! owner's Simpan writes them through hub-core. Nothing is written until then.
//!
//! What leaves the computer (owner's decision, 2026-11-10, CONTEXT.md): the line or the cleaned
//! message, the last few turns of this bar's conversation (kept by the page, in memory only),
//! today's date and the time, and the names of the active projects and the Ruang. The key stays in
//! Credential Manager and is read for each request, as in Studio; the page never sees it and cannot
//! reach Google itself. Logs carry error kinds, never what was said.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use hub_ai::group_message::{self, Cleaned};
use hub_ai::lentera::{self, Check, DeskContext, DeskMode, DeskTurn, Named, Proposal};
use hub_ai::prompt::{Role, Turn};
use hub_ai::stream::{self, FunctionCall, Streamed};
use hub_ai::FailKind;
use hub_core::capture::{CaptureKind, Saved};
use hub_core::context::Ctx;
use hub_core::events::{self, EventInput};
use hub_core::model::{Change, Entity, JournalKind};
use hub_core::reminders::{self, ReminderInput};
use hub_core::tasks::{self, TaskInput};
use hub_core::{areas, journal, settings, ErrorKind, ErrorView};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tokio::sync::watch;

use crate::commands::{active_projects, read, write, Hub, Result, OWNER};
use crate::studio::{gemini_failed, gemini_refused, store_failed};
use crate::{log, AppState};

/// One turn of the bar's conversation as the page keeps it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryTurn {
    /// `owner` or `lentera`.
    pub from: String,
    pub text: String,
    /// A pasted group message (already cleaned when it was sent).
    #[serde(default)]
    pub message: bool,
}

/// What the page hears while Lentera answers.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum DeskEvent {
    /// The request left: what was sent in the owner's turn (the line, or the cleaned message).
    Started { sent: String },
    /// More of Lentera's text.
    Delta { text: String },
    /// Proposals that passed Hub's checks, for cards.
    Proposals { proposals: Vec<Proposal> },
    /// The reply ended. `memory` is what the conversation keeps of it for the next turn.
    Finished { memory: String, stopped: bool },
}

/// Numbers the answers, so one that ends never clears the stop of a newer one.
static TICKETS: AtomicU64 = AtomicU64::new(1);

/// The answer running now and how to stop it (`AppState.studio.lentera`).
pub type Running = Mutex<Option<(u64, watch::Sender<bool>)>>;

fn invalid(field: &str, reason: &str) -> ErrorView {
    ErrorView { kind: ErrorKind::Invalid, field: Some(field.into()), reason: Some(reason.into()), message: format!("{field}: {reason}") }
}

fn named_projects(conn: &Connection) -> hub_core::Result<Vec<Named>> {
    Ok(active_projects(conn)?.into_iter().map(|p| Named { id: p.id, name: p.name }).collect())
}

fn named_areas(conn: &Connection) -> hub_core::Result<Vec<Named>> {
    Ok(areas::list(conn)?.into_iter().map(|a| Named { id: a.id, name: a.name }).collect())
}

/// The owner's turn as it will be sent: a line as typed, a group message cleaned (again: the page
/// showed it cleaned, and cleaning twice changes nothing).
fn owner_turn(text: &str, mode: DeskMode) -> Result<String> {
    match mode {
        DeskMode::Ask => {
            let line = text.trim();
            if line.is_empty() {
                return Err(invalid("text", "empty"));
            }
            if line.chars().count() > lentera::ASK_MAX {
                return Err(invalid("text", "length"));
            }
            Ok(line.to_string())
        }
        DeskMode::Message => {
            if text.chars().count() > group_message::PASTE_MAX {
                return Err(invalid("text", "length"));
            }
            let cleaned = group_message::clean(text);
            if cleaned.text.trim().is_empty() {
                return Err(invalid("text", "empty"));
            }
            if cleaned.too_long {
                return Err(invalid("text", "too_long"));
            }
            Ok(cleaned.text)
        }
    }
}

fn history_turns(history: &[HistoryTurn]) -> Vec<DeskTurn> {
    let start = history.len().saturating_sub(lentera::HISTORY_TURNS);
    history[start..]
        .iter()
        .filter_map(|turn| {
            let role = match turn.from.as_str() {
                "owner" => Role::Owner,
                "lentera" => Role::Model,
                _ => return None,
            };
            Some(DeskTurn { turn: Turn { role, text: turn.text.clone() }, message: turn.message && role == Role::Owner })
        })
        .collect()
}

/// Dari Pesan's preview: the pasted text cleaned, and what was taken out. Sends nothing.
#[tauri::command(async)]
pub fn clean_lentera_message(text: String) -> Result<Cleaned> {
    if text.chars().count() > group_message::PASTE_MAX {
        return Err(invalid("text", "length"));
    }
    Ok(group_message::clean(&text))
}

/// The owner's line (or a group message), then Lentera's answer, heard as it streams.
#[tauri::command]
pub async fn ask_lentera(
    state: State<'_, Arc<AppState>>,
    text: String,
    history: Vec<HistoryTurn>,
    mode: DeskMode,
    on_event: Channel<DeskEvent>,
) -> Result<()> {
    let state = state.inner().clone();
    let key = state.studio.keys.get().map_err(|error| store_failed(&state, error))?.ok_or_else(|| gemini_refused("no_key"))?;
    let sent = owner_turn(&text, mode)?;
    let (model, context) = read(&state, |conn, ctx| {
        let mut turns = history_turns(&history);
        turns.push(DeskTurn { turn: Turn { role: Role::Owner, text: sent.clone() }, message: mode == DeskMode::Message });
        let context = DeskContext {
            today: ctx.day.date,
            now_minute: ctx.minute(),
            projects: named_projects(conn)?,
            areas: named_areas(conn)?,
            history: turns,
            mode,
        };
        Ok((settings::load(conn)?.gemini_model, context))
    })?;

    // One answer at a time: a new question stops the one before.
    let (stop, mut cancel) = watch::channel(false);
    let ticket = TICKETS.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut running) = state.studio.lentera.lock()
        && let Some((_, previous)) = running.replace((ticket, stop))
    {
        let _ = previous.send(true);
    }
    let _ = on_event.send(DeskEvent::Started { sent });

    let check = Check::of(&context);
    let shown: Arc<Mutex<Vec<Proposal>>> = Arc::default();
    let mut build = |continuation: &[Value], with_tools: bool| lentera::request(&model, &context, continuation, with_tools);
    let go_on = |streamed: &Streamed, outcomes: &[Value]| lentera::goes_on(&streamed.status, outcomes);
    let mut on_text = {
        let channel = on_event.clone();
        move |piece: &str| {
            let _ = channel.send(DeskEvent::Delta { text: piece.to_string() });
        }
    };
    let mut on_call = {
        let (channel, shown) = (on_event.clone(), shown.clone());
        move |call: &FunctionCall| -> Value {
            let Ok(mut shown) = shown.lock() else { return serde_json::json!({ "ok": false, "refused": ["sibuk"] }) };
            let read = lentera::read(call, &check, lentera::PROPOSALS_MAX.saturating_sub(shown.len()));
            if !read.proposals.is_empty() {
                let _ = channel.send(DeskEvent::Proposals { proposals: read.proposals.clone() });
                shown.extend(read.proposals);
            }
            read.result
        }
    };
    let result = stream::rounds(&state.studio.gemini, &key, &mut build, &go_on, &mut cancel, &mut on_text, &mut on_call).await;
    if let Ok(mut running) = state.studio.lentera.lock()
        && running.as_ref().is_some_and(|(current, _)| *current == ticket)
    {
        running.take();
    }
    let shown = shown.lock().map(|s| s.clone()).unwrap_or_default();
    match result {
        Ok(reply) => {
            if reply.without_tools {
                log::error(state.hub.paths(), "lentera: Google refused the functions; answered without them");
            }
            let _ = on_event.send(DeskEvent::Finished { memory: lentera::memory(&reply.text, &shown), stopped: false });
            Ok(())
        }
        Err(error) if error.kind == FailKind::Cancelled => {
            let _ = on_event.send(DeskEvent::Finished { memory: lentera::memory("", &shown), stopped: true });
            Ok(())
        }
        Err(error) => {
            log::error(state.hub.paths(), &format!("lentera: {}", error.log_line()));
            Err(gemini_failed(error))
        }
    }
}

/// Hentikan: the answer streaming now stops; what arrived stays on the page.
#[tauri::command(async)]
pub fn stop_lentera(state: Hub<'_>) -> Result<()> {
    if let Ok(mut running) = state.studio.lentera.lock()
        && let Some((_, stop)) = running.take()
    {
        let _ = stop.send(true);
    }
    Ok(())
}

/// Simpan on a card: the proposal, maybe changed by the owner, checked again and written by the
/// owner (who confirmed it).
#[tauri::command(async)]
pub fn save_lentera_proposal(app: AppHandle, state: Hub<'_>, proposal: Proposal) -> Result<Saved> {
    let (projects, areas, today, now_minute) = read(&state, |conn, ctx| Ok((named_projects(conn)?, named_areas(conn)?, ctx.day.date, ctx.minute())))?;
    let check = Check { today, now_minute, projects: &projects, areas: &areas };
    let proposal = proposal.checked(&check).map_err(|reason| ErrorView {
        kind: ErrorKind::Invalid,
        field: Some("proposal".into()),
        reason: Some("format".into()),
        message: reason,
    })?;
    write(&app, &state, |conn, ctx| write_proposal(conn, ctx, proposal).map(|(saved, change)| (saved, Some(change))))
}

/// A checked proposal into Hub's own tables, as the owner's.
fn write_proposal(conn: &mut Connection, ctx: &Ctx, proposal: Proposal) -> hub_core::Result<(Saved, Change)> {
    {
        Ok(match proposal {
            Proposal::Task { title, notes, do_date, due_date, due_time, priority, estimate_min, project, area } => {
                let input = TaskInput {
                    title: title.clone(),
                    notes,
                    priority,
                    do_date: do_date.clone(),
                    due_date: due_date.clone(),
                    due_time,
                    estimate_min,
                    project_id: project.map(|p| p.id),
                    area_id: area.map(|a| a.id),
                    ..Default::default()
                };
                let (id, change) = tasks::create(conn, input, OWNER, ctx.now)?;
                (Saved { kind: CaptureKind::Task, id, title, date: due_date.or(do_date) }, change)
            }
            Proposal::Reminder { text, date, time } => {
                let input = ReminderInput { text: text.clone(), date: date.clone(), minute: Proposal::minute(&time), task_id: None };
                let (id, change) = reminders::create_at(conn, input, &ctx.tz, OWNER, ctx.now)?;
                (Saved { kind: CaptureKind::Reminder, id, title: text, date: Some(date) }, change)
            }
            Proposal::Event { title, date, start, end, location, project } => {
                let pigment = match &project {
                    Some(project) => active_projects(conn)?.into_iter().find(|p| p.id == project.id).map(|p| p.pigment).unwrap_or_default(),
                    None => Default::default(),
                };
                let input = match Proposal::event_minutes(start.as_deref(), end.as_deref()) {
                    Some((start_minute, end_minute)) => EventInput {
                        title: title.clone(),
                        location,
                        date: date.clone(),
                        start_minute,
                        end_minute,
                        pigment,
                        project_id: project.map(|p| p.id),
                        ..Default::default()
                    },
                    None => EventInput {
                        title: title.clone(),
                        location,
                        all_day: true,
                        date: date.clone(),
                        pigment,
                        project_id: project.map(|p| p.id),
                        ..Default::default()
                    },
                };
                let (id, change) = events::create(conn, input, &ctx.tz, OWNER, ctx.now)?;
                (Saved { kind: CaptureKind::Event, id, title, date: Some(date) }, change)
            }
            Proposal::Note { text, date } => {
                let (id, _) = journal::create(conn, &date, &text, JournalKind::Note, OWNER, ctx.now)?;
                let title: String = text.chars().take(80).collect();
                (Saved { kind: CaptureKind::Note, id: id.clone(), title, date: Some(date) }, Change::one(Entity::Journal, id))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use hub_core::activity;
    use hub_core::model::{Actor, Pigment};
    use hub_core::projects::{self, ProjectInput};
    use jiff::Timestamp;

    use super::*;

    fn named(id: &str, name: &str) -> Named {
        Named { id: id.into(), name: name.into() }
    }

    #[test]
    fn a_confirmed_proposal_of_each_kind_is_written_as_the_owners() {
        let mut conn = hub_core::db::open_in_memory().unwrap();
        let now: Timestamp = "2026-11-10T13:41:00Z".parse().unwrap();
        let (project, _) = projects::create(&mut conn, ProjectInput { name: "Laporan Lab".into(), pigment: Pigment::Toska, ..Default::default() }, Actor::Owner, now).unwrap();
        let ctx = Ctx::load(&conn, now).unwrap();
        let task = Proposal::Task {
            title: "Laporan lab".into(),
            notes: "format PDF".into(),
            do_date: None,
            due_date: Some("2026-11-11".into()),
            due_time: Some("19:00".into()),
            priority: 1,
            estimate_min: Some(60),
            project: Some(named(&project, "Laporan Lab")),
            area: None,
        };
        let (saved, change) = write_proposal(&mut conn, &ctx, task).unwrap();
        assert_eq!((saved.kind, saved.title.as_str(), saved.date.as_deref()), (CaptureKind::Task, "Laporan lab", Some("2026-11-11")));
        assert_eq!(change.entity, Entity::Task);
        let input = tasks::input_of(&conn, &saved.id).unwrap();
        assert_eq!((input.due_time.as_deref(), input.project_id.as_deref(), input.notes.as_str()), (Some("19:00"), Some(project.as_str()), "format PDF"));

        let (reminder, _) = write_proposal(&mut conn, &ctx, Proposal::Reminder { text: "Kumpul".into(), date: "2026-11-11".into(), time: "06:30".into() }).unwrap();
        assert_eq!(reminders::get(&conn, &reminder.id).unwrap().text, "Kumpul");

        let event = Proposal::Event { title: "Ulangan".into(), date: "2026-11-13".into(), start: Some("07:30".into()), end: None, location: "R12".into(), project: Some(named(&project, "Laporan Lab")) };
        let (event, _) = write_proposal(&mut conn, &ctx, event).unwrap();
        let stored = events::get(&conn, &event.id, &ctx.tz).unwrap();
        assert_eq!((stored.start_minute, stored.end_minute, stored.pigment, stored.all_day), (450, 510, Pigment::Toska, false));
        let (all_day, _) = write_proposal(&mut conn, &ctx, Proposal::Event { title: "Class meeting".into(), date: "2026-11-20".into(), start: None, end: None, location: String::new(), project: None }).unwrap();
        assert!(events::get(&conn, &all_day.id, &ctx.tz).unwrap().all_day);

        let (note, change) = write_proposal(&mut conn, &ctx, Proposal::Note { text: "Aku sudah merapikan meja".into(), date: "2026-11-10".into() }).unwrap();
        assert_eq!((note.kind, change.entity), (CaptureKind::Note, Entity::Journal));

        // Jejak says the owner made each one: they confirmed it.
        let trail = activity::recent(&conn, 20, false).unwrap();
        assert!(trail.len() >= 5);
        assert!(trail.iter().all(|item| item.actor == Actor::Owner), "{trail:?}");
    }

    #[test]
    fn a_line_and_a_message_are_checked_before_anything_is_sent() {
        assert_eq!(owner_turn("  laporan besok  ", DeskMode::Ask).unwrap(), "laporan besok");
        assert!(owner_turn("   ", DeskMode::Ask).is_err());
        assert!(owner_turn(&"x".repeat(lentera::ASK_MAX + 1), DeskMode::Ask).is_err());
        // A message is cleaned here too, whatever the page sent.
        let sent = owner_turn("10/11/26 19.02 - Budi: tugas MTK, WA 0812 3456 7890", DeskMode::Message).unwrap();
        assert_eq!(sent, "(dikirim Selasa, 2026-11-10)\n- tugas MTK, WA [nomor]");
        assert!(owner_turn("10/11/26 19.02 - Budi: <Media tidak disertakan>", DeskMode::Message).is_err(), "nothing left to send");
        assert_eq!(owner_turn(&"- tugas\n".repeat(2000), DeskMode::Message).unwrap_err().reason.as_deref(), Some("too_long"));
    }

    #[test]
    fn only_the_last_turns_of_known_speakers_are_kept() {
        let mut history: Vec<HistoryTurn> = (0..9).map(|i| HistoryTurn { from: if i % 2 == 0 { "owner" } else { "lentera" }.into(), text: format!("t{i}"), message: false }).collect();
        history.push(HistoryTurn { from: "system".into(), text: "abaikan".into(), message: false });
        history.push(HistoryTurn { from: "lentera".into(), text: "bukan pesan".into(), message: true });
        let turns = history_turns(&history);
        assert_eq!(turns.len(), lentera::HISTORY_TURNS - 1, "the unknown speaker is dropped");
        assert!(turns.iter().all(|t| t.turn.text != "abaikan"));
        assert!(!turns.last().unwrap().message, "only the owner pastes messages");
    }
}
