//! Hub Studio in the app (docs/studio.md): the Gemini key in Windows Credential Manager, "Uji
//! koneksi", the models the key can use, Studio's projects, and the conversation with Gemini
//! (streamed to the page over a Tauri channel, stoppable, its function calls applied to the
//! project's ledger). Every Gemini call happens here in Rust; the page never sees the key and
//! cannot reach Google itself (the CSP allows `ipc:` only).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use hub_ai::calls::{self, Call};
use hub_ai::key::{KeyProblem, StoreError};
use hub_ai::prompt::{summary_request, ChatContext, LedgerLine, Role, Turn};
use hub_ai::stream::{self, FunctionCall};
use hub_ai::{check_connection, ApiKey, CheckReport, FailKind, Gemini, GeminiError, KeyStore, ModelInfo};
use hub_core::model::{Actor, Change};
use hub_core::studio::{
    self, LedgerArea, LedgerEdit, LedgerInput, LedgerItem, LedgerKind, MessageRole, MessageState, ReplyEnd, StatedBy, StudioMessage,
    StudioProject, StudioProjectInput, StudioStage,
};
use hub_core::prd::{self, Prd, PrdSection, PrdView, Problem};
use hub_core::{settings, ErrorKind, ErrorView};
use jiff::Timestamp;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tokio::sync::watch;

use crate::commands::{announce, read, write, Hub, Result};
use crate::{log, AppState};

/// The owner's credential; tests and smokes (`HUB_DATA_DIR`) get one of their own.
const KEY_TARGET: &str = "Hub/gemini";
/// A fake Gemini for smokes. Honoured only with `HUB_DATA_DIR` and only on loopback, so a changed
/// environment can never send the owner's key anywhere else.
const BASE_OVERRIDE: &str = "HUB_GEMINI_BASE_URL";

/// Gemini as the app holds it: the client and where the key is kept.
pub struct Studio {
    pub gemini: Gemini,
    pub keys: Box<dyn KeyStore>,
    /// The reply streaming now in each project, so Hentikan can reach it.
    pub(crate) replies: Mutex<HashMap<String, watch::Sender<bool>>>,
    /// Lentera's answer in the bar (CP03), so Hentikan can reach it.
    pub(crate) lentera: crate::lentera_ai::Running,
}

impl Studio {
    /// `test_root` is the data folder when `HUB_DATA_DIR` is set.
    pub fn new(test_root: Option<&std::path::Path>) -> Studio {
        let base = test_root
            .and_then(|_| std::env::var(BASE_OVERRIDE).ok())
            .filter(|url| hub_ai::client::is_loopback(url))
            .unwrap_or_else(|| hub_ai::DEFAULT_BASE_URL.to_string());
        let gemini = Gemini::new(&base, hub_ai::client::DEFAULT_TIMEOUT)
            .or_else(|_| Gemini::google())
            .expect("an HTTP client without network access always builds");
        let target = match test_root {
            Some(root) => format!("Hub-test/{}/gemini", root.display()),
            None => KEY_TARGET.to_string(),
        };
        Studio { gemini, keys: key_store(&target), replies: Mutex::default(), lentera: Mutex::default() }
    }
}

#[cfg(windows)]
fn key_store(target: &str) -> Box<dyn KeyStore> {
    Box::new(hub_ai::key::CredentialStore::new(target))
}

#[cfg(not(windows))]
fn key_store(_target: &str) -> Box<dyn KeyStore> {
    Box::new(hub_ai::key::MemoryStore::default())
}

pub(crate) fn store_failed(state: &AppState, error: StoreError) -> ErrorView {
    log::error(state.hub.paths(), &format!("gemini key: {error}"));
    ErrorView { kind: ErrorKind::Internal, field: Some("keyStore".into()), reason: None, message: error.to_string() }
}

/// A failed Gemini call as the page receives it: the reason is the kind (`offline`, `key`, ...).
pub(crate) fn gemini_failed(error: GeminiError) -> ErrorView {
    ErrorView { kind: ErrorKind::Internal, field: Some("gemini".into()), reason: Some(error.kind.as_str().into()), message: error.message }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiStatus {
    /// A key is saved in Credential Manager. The key itself is never sent to the page.
    pub has_key: bool,
    pub model: String,
}

#[tauri::command(async)]
pub fn gemini_status(state: Hub<'_>) -> Result<GeminiStatus> {
    let has_key = state.studio.keys.get().map_err(|error| store_failed(&state, error))?.is_some();
    let model = read(&state, |conn, _| Ok(settings::load(conn)?.gemini_model))?;
    Ok(GeminiStatus { has_key, model })
}

/// Save (or replace) the key. Checked for its shape only; "Uji koneksi" asks Google.
#[tauri::command(async)]
pub fn gemini_save_key(app: AppHandle, state: Hub<'_>, key: String) -> Result<()> {
    let key = ApiKey::parse(&key).map_err(|problem| {
        let reason = match problem {
            KeyProblem::Length => "length",
            KeyProblem::Characters => "format",
        };
        ErrorView { kind: ErrorKind::Invalid, field: Some("key".into()), reason: Some(reason.into()), message: "invalid key".into() }
    })?;
    state.studio.keys.set(&key).map_err(|error| store_failed(&state, error))?;
    write(&app, &state, |conn, ctx| Ok(((), Some(settings::note_gemini_key(conn, true, Actor::Owner, ctx.now)?))))
}

#[tauri::command(async)]
pub fn gemini_delete_key(app: AppHandle, state: Hub<'_>) -> Result<()> {
    state.studio.keys.delete().map_err(|error| store_failed(&state, error))?;
    write(&app, &state, |conn, ctx| Ok(((), Some(settings::note_gemini_key(conn, false, Actor::Owner, ctx.now)?))))
}

/// "Uji koneksi" for the model in Pengaturan. Changes nothing, here or at Google.
#[tauri::command]
pub async fn gemini_check(state: State<'_, Arc<AppState>>) -> Result<CheckReport> {
    let key = state.studio.keys.get().map_err(|error| store_failed(&state, error))?;
    let model = read(&state, |conn, _| Ok(settings::load(conn)?.gemini_model))?;
    let report = check_connection(&state.studio.gemini, key.as_ref(), &model).await;
    if report.outcome != hub_ai::CheckOutcome::Connected {
        let code = report.code.as_deref().unwrap_or("-");
        // Google's message or the shape of an answer Hub did not recognise; never a key or content.
        let detail = report.detail.as_deref().unwrap_or("-");
        log::error(state.hub.paths(), &format!("gemini check: {:?} ({code}): {detail}", report.outcome));
    }
    Ok(report)
}

/// The Gemini models the saved key can use to write text.
#[tauri::command]
pub async fn gemini_models(state: State<'_, Arc<AppState>>) -> Result<Vec<ModelInfo>> {
    let Some(key) = state.studio.keys.get().map_err(|error| store_failed(&state, error))? else {
        return Ok(Vec::new());
    };
    state.studio.gemini.models(&key).await.map_err(gemini_failed)
}

#[tauri::command(async)]
pub fn list_studio_projects(state: Hub<'_>) -> Result<Vec<StudioProject>> {
    read(&state, |conn, _| studio::list_projects(conn))
}

/// Replies left half-written by a Hub that stopped mid-answer are marked failed. Run at start.
pub fn recover(state: &AppState) {
    if let Err(error) = state.hub.write(|conn| studio::recover_interrupted(conn, Timestamp::now())) {
        log::error(state.hub.paths(), &format!("studio recover: {error}"));
    }
}

// ---------- projects and the ledger ----------

/// Start a project from the owner's first words: its name comes from the idea.
#[tauri::command(async)]
pub fn start_studio_project(app: AppHandle, state: Hub<'_>, idea: String) -> Result<String> {
    write(&app, &state, |conn, ctx| {
        let input = StudioProjectInput { name: studio::name_from_idea(&idea), idea: idea.chars().take(2000).collect() };
        let (id, change) = studio::create_project(conn, input, Actor::Owner, ctx.now)?;
        Ok((id, Some(change)))
    })
}

#[tauri::command(async)]
pub fn get_studio_project(state: Hub<'_>, id: String) -> Result<StudioProject> {
    read(&state, |conn, _| studio::get_project(conn, &id))
}

#[tauri::command(async)]
pub fn rename_studio_project(app: AppHandle, state: Hub<'_>, id: String, name: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(studio::rename_project(conn, &id, &name, Actor::Owner, ctx.now)?))))
}

#[tauri::command(async)]
pub fn delete_studio_project(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(studio::delete_project(conn, &id, Actor::Owner, ctx.now)?))))
}

/// The stages the owner can move into so far: the roadmap comes with its own checkpoint (S4).
const STAGES_OPEN: &[StudioStage] = &[StudioStage::Clarify, StudioStage::Prd];

#[tauri::command(async)]
pub fn advance_studio_stage(app: AppHandle, state: Hub<'_>, id: String, to: StudioStage) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(studio::advance_stage(conn, &id, to, STAGES_OPEN, Actor::Owner, ctx.now)?))))
}

#[tauri::command(async)]
pub fn studio_messages(state: Hub<'_>, project_id: String) -> Result<Vec<StudioMessage>> {
    read(&state, |conn, _| studio::list_messages(conn, &project_id))
}

#[tauri::command(async)]
pub fn studio_ledger(state: Hub<'_>, project_id: String) -> Result<Vec<LedgerItem>> {
    read(&state, |conn, _| studio::list_ledger(conn, &project_id))
}

#[tauri::command(async)]
pub fn add_studio_ledger(app: AppHandle, state: Hub<'_>, project_id: String, input: LedgerInput) -> Result<String> {
    write(&app, &state, |conn, ctx| {
        let (id, change) = studio::add_ledger_item(conn, &project_id, input, StatedBy::Owner, None, ctx.now)?;
        Ok((id, Some(change)))
    })
}

#[tauri::command(async)]
pub fn edit_studio_ledger(app: AppHandle, state: Hub<'_>, id: String, edit: LedgerEdit) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(studio::edit_ledger_item(conn, &id, edit, Actor::Owner, ctx.now)?))))
}

// ---------- the conversation ----------

/// What the page hears while Gemini answers.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ReplyEvent {
    /// The reply's message exists (empty, streaming).
    Started { message_id: String },
    /// More text.
    Delta { text: String },
    /// Questions or a stage proposal arrived.
    Parts { parts: Value },
    /// The reply ended: complete, stopped, or failed (with the error's kind).
    Finished { state: MessageState, error_kind: Option<String> },
}

/// Messages Gemini reads in full; older ones are summarized.
const WINDOW: usize = 40;
/// How many messages past the window wait before they are summarized (not on every turn).
const SUMMARY_BATCH: usize = 10;

pub(crate) fn gemini_refused(reason: &str) -> ErrorView {
    ErrorView { kind: ErrorKind::Invalid, field: Some("gemini".into()), reason: Some(reason.into()), message: reason.into() }
}

/// The owner's message, then Gemini's reply.
#[tauri::command]
pub async fn send_studio_message(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    project_id: String,
    text: String,
    on_event: Channel<ReplyEvent>,
) -> Result<()> {
    let state = state.inner().clone();
    // Checked before the message is kept, so a missing key leaves no unanswered message.
    let key = state.studio.keys.get().map_err(|error| store_failed(&state, error))?.ok_or_else(|| gemini_refused("no_key"))?;
    write(&app, &state, |conn, ctx| {
        let (_, change) = studio::add_owner_message(conn, &project_id, &text, ctx.now)?;
        Ok(((), Some(change)))
    })?;
    run_reply(app, state, project_id, key, on_event).await
}

/// Answer the owner's last message again (after a reply was stopped or failed).
#[tauri::command]
pub async fn retry_studio_reply(app: AppHandle, state: State<'_, Arc<AppState>>, project_id: String, on_event: Channel<ReplyEvent>) -> Result<()> {
    let state = state.inner().clone();
    let key = state.studio.keys.get().map_err(|error| store_failed(&state, error))?.ok_or_else(|| gemini_refused("no_key"))?;
    let messages = read(&state, |conn, _| studio::list_messages(conn, &project_id))?;
    let last = messages.iter().rev().find(|m| m.role == MessageRole::Owner || m.state == MessageState::Complete);
    if !last.is_some_and(|m| m.role == MessageRole::Owner) {
        return Err(gemini_refused("nothing_to_answer"));
    }
    run_reply(app, state, project_id, key, on_event).await
}

/// Hentikan: the reply streaming in this project stops; what arrived is kept.
#[tauri::command(async)]
pub fn stop_studio_reply(state: Hub<'_>, project_id: String) -> Result<()> {
    if let Ok(replies) = state.studio.replies.lock()
        && let Some(stop) = replies.get(&project_id)
    {
        let _ = stop.send(true);
    }
    Ok(())
}

/// The conversation Gemini reads: complete messages after the summary.
fn history<'a>(messages: &'a [StudioMessage], after: Option<&str>) -> Vec<&'a StudioMessage> {
    let start = after.and_then(|id| messages.iter().position(|m| m.id == id)).map(|i| i + 1).unwrap_or(0);
    messages[start..].iter().filter(|m| m.role == MessageRole::Owner || m.state == MessageState::Complete).collect()
}

fn turn(message: &StudioMessage) -> Turn {
    Turn { role: if message.role == MessageRole::Owner { Role::Owner } else { Role::Model }, text: message.text.clone() }
}

/// When enough messages sit before the window, Gemini summarizes them into the project's
/// summary. A failure here only means Gemini reads the window and the older summary.
async fn summarize_if_due(state: &AppState, key: &ApiKey, model: &str, project_id: &str) {
    let Ok((messages, (summary, until))) =
        read(state, |conn, _| Ok((studio::list_messages(conn, project_id)?, studio::summary(conn, project_id)?)))
    else {
        return;
    };
    let pending = history(&messages, until.as_deref());
    if pending.len() < WINDOW + SUMMARY_BATCH {
        return;
    }
    let older = &pending[..pending.len() - WINDOW];
    let turns: Vec<Turn> = older.iter().map(|m| turn(m)).collect();
    let Some(last) = older.last() else { return };
    match state.studio.gemini.generate(key, &summary_request(model, &summary, &turns)).await {
        Ok(answer) if !answer.text.trim().is_empty() => {
            if let Err(error) = state.hub.write(|conn| studio::set_summary(conn, project_id, &answer.text, &last.id)) {
                log::error(state.hub.paths(), &format!("studio summary: {error}"));
            }
        }
        Ok(_) => {}
        Err(error) => log::error(state.hub.paths(), &format!("studio summary: {}", error.kind.as_str())),
    }
}

/// What Gemini reads about a project: its frame, its ledger (with the codes Gemini uses for
/// items), the summary, and the conversation window. Nothing else of Hub.
pub(crate) fn project_context(state: &AppState, project_id: &str) -> Result<(ChatContext, Vec<(String, String)>)> {
    let (project, ledger, messages, (summary, until), tech) = read(state, |conn, _| {
        Ok((
            studio::get_project(conn, project_id)?,
            studio::list_ledger(conn, project_id)?,
            studio::list_messages(conn, project_id)?,
            studio::summary(conn, project_id)?,
            hub_core::stack::get(conn, project_id)?,
        ))
    })?;
    let codes: Vec<(String, String)> = ledger.iter().enumerate().map(|(i, item)| (format!("C{}", i + 1), item.id.clone())).collect();
    let lines = ledger
        .iter()
        .zip(&codes)
        .map(|(item, (code, _))| LedgerLine {
            code: code.clone(),
            kind: item.kind.as_str().into(),
            topic: item.topic.clone(),
            statement: item.statement.clone(),
            status: item.status.as_str().into(),
            by: item.stated_by.as_str().into(),
            area: item.area.map(|a| a.as_str().to_string()).unwrap_or_default(),
        })
        .collect();
    let recent = history(&messages, until.as_deref());
    let window = &recent[recent.len().saturating_sub(WINDOW)..];
    let context = ChatContext {
        project_name: project.name.clone(),
        idea: project.idea.clone(),
        stage: project.stage.as_str().into(),
        ledger: lines,
        summary,
        stack: tech.map(|s| stack_lines(&s)).unwrap_or_default(),
        history: window.iter().map(|m| turn(m)).collect(),
    };
    Ok((context, codes))
}

/// The tech stack as Gemini reads it in every later request: approved or not, then each layer.
fn stack_lines(stack: &hub_core::stack::StudioStack) -> String {
    let status = match stack.status {
        hub_core::stack::StackStatus::Approved => "disetujui pemilik",
        hub_core::stack::StackStatus::Draft => "draf, belum disetujui",
    };
    let mut lines = vec![format!("Status: {status}")];
    for layer in &stack.body.layers {
        let choice = if layer.choice.is_empty() { "(belum dipilih)" } else { layer.choice.as_str() };
        lines.push(format!("{}: {choice}", layer.layer.as_str()));
    }
    lines.join("\n")
}

async fn run_reply(app: AppHandle, state: Arc<AppState>, project_id: String, key: ApiKey, on_event: Channel<ReplyEvent>) -> Result<()> {
    let model = read(&state, |conn, _| Ok(settings::load(conn)?.gemini_model))?;
    summarize_if_due(&state, &key, &model, &project_id).await;
    let (context, codes) = project_context(&state, &project_id)?;

    let reply_id = write(&app, &state, |conn, ctx| {
        let (id, change) = studio::begin_reply(conn, &project_id, &model, ctx.now)?;
        Ok((id, Some(change)))
    })?;
    let (stop, mut cancel) = watch::channel(false);
    if let Ok(mut replies) = state.studio.replies.lock() {
        replies.insert(project_id.clone(), stop);
    }
    let _ = on_event.send(ReplyEvent::Started { message_id: reply_id.clone() });

    let written = Arc::new(Mutex::new(String::new()));
    let parts = Arc::new(Mutex::new(json!({})));
    let mut on_text = {
        let (written, channel) = (written.clone(), on_event.clone());
        move |piece: &str| {
            if let Ok(mut text) = written.lock() {
                text.push_str(piece);
            }
            let _ = channel.send(ReplyEvent::Delta { text: piece.to_string() });
        }
    };
    let mut on_call = {
        let (app, state, parts, channel) = (app.clone(), state.clone(), parts.clone(), on_event.clone());
        let (project_id, reply_id) = (project_id.clone(), reply_id.clone());
        move |call: &FunctionCall| -> Value {
            let outcome = apply_call(&app, &state, &project_id, &reply_id, &codes, &parts, call);
            if call.name != "catat"
                && let Ok(parts) = parts.lock()
            {
                let _ = channel.send(ReplyEvent::Parts { parts: parts.clone() });
            }
            calls::result(&outcome)
        }
    };
    let result = stream::reply(&state.studio.gemini, &key, &model, &context, &mut cancel, &mut on_text, &mut on_call).await;
    if let Ok(mut replies) = state.studio.replies.lock() {
        replies.remove(&project_id);
    }

    let text = written.lock().map(|t| t.clone()).unwrap_or_default();
    let parts = parts.lock().map(|p| p.clone()).unwrap_or_else(|_| json!({}));
    let end = match &result {
        Ok(reply) => {
            if reply.without_tools {
                log::error(state.hub.paths(), "studio reply: Google refused the functions; answered without them");
            }
            ReplyEnd {
                text: reply.text.clone(),
                parts,
                state: MessageState::Complete,
                error_kind: None,
                input_tokens: Some(reply.input_tokens),
                output_tokens: Some(reply.output_tokens),
            }
        }
        Err(error) if error.kind == FailKind::Cancelled => {
            ReplyEnd { text, parts, state: MessageState::Stopped, error_kind: None, input_tokens: None, output_tokens: None }
        }
        Err(error) => {
            log::error(state.hub.paths(), &format!("studio reply: {}", error.log_line()));
            ReplyEnd { text, parts, state: MessageState::Failed, error_kind: Some(error.kind.as_str().into()), input_tokens: None, output_tokens: None }
        }
    };
    let (finished_state, error_kind) = (end.state, end.error_kind.clone());
    write(&app, &state, |conn, ctx| Ok(((), Some(studio::finish_reply(conn, &reply_id, end, ctx.now)?))))?;
    let _ = on_event.send(ReplyEvent::Finished { state: finished_state, error_kind });
    Ok(())
}

/// Apply one of Gemini's function calls to this project only. `Err` is the reason Gemini hears.
fn apply_call(
    app: &AppHandle,
    state: &AppState,
    project_id: &str,
    reply_id: &str,
    codes: &[(String, String)],
    parts: &Mutex<Value>,
    call: &FunctionCall,
) -> std::result::Result<String, String> {
    match calls::read(call)? {
        Call::Notes(notes) => {
            let mut changes: Vec<Change> = Vec::new();
            for note in notes {
                let replaces = match &note.replaces {
                    Some(code) => Some(
                        codes.iter().find(|(c, _)| c == code).map(|(_, id)| id.clone()).ok_or_else(|| format!("kode {code} tidak ada"))?,
                    ),
                    None => None,
                };
                let kind = LedgerKind::parse(&note.kind).ok_or("jenis tidak dikenal")?;
                let area = note.area.as_deref().and_then(LedgerArea::parse);
                let input = LedgerInput { kind, topic: note.topic, statement: note.statement, rationale: note.rationale, area, replaces_id: replaces };
                let now = Timestamp::now();
                let (_, change) = state
                    .hub
                    .write(|conn| studio::add_ledger_item(conn, project_id, input, StatedBy::Gemini, Some(reply_id), now))
                    .map_err(|error| format!("tidak tersimpan: {error}"))?;
                changes.push(change);
            }
            for change in &changes {
                announce(app, change);
            }
            Ok(format!("{} catatan tersimpan", changes.len()))
        }
        Call::Questions(questions) => {
            let mut parts = parts.lock().map_err(|_| "sibuk".to_string())?;
            Ok(calls::put_questions(&mut parts, questions))
        }
        Call::Stage(proposal) => {
            let mut parts = parts.lock().map_err(|_| "sibuk".to_string())?;
            calls::put_stage(&mut parts, proposal)
        }
    }
}

// ---------- the PRD ----------

#[tauri::command(async)]
pub fn studio_prd(state: Hub<'_>, project_id: String) -> Result<PrdView> {
    read(&state, |conn, _| prd::view(conn, &project_id))
}

#[tauri::command(async)]
pub fn get_prd_version(state: Hub<'_>, id: String) -> Result<Prd> {
    read(&state, |conn, _| prd::version_body(conn, &id))
}

/// What the page hears while the PRD is written.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PrdEvent {
    /// Part `part` of `total` is being written (or repaired).
    Step { part: usize, total: usize, repair: bool },
    /// Done: complete (with the problems still in it), stopped, or failed.
    Finished { state: MessageState, error_kind: Option<String>, problems: usize },
}

/// "Susun PRD": Gemini writes the PRD from the conversation in three parts; a part that breaks
/// the rules is sent back once with the reasons. The result becomes the draft and version N, with
/// any problem left shown for the owner to fix. The project moves to the PRD stage.
#[tauri::command]
pub async fn generate_prd(app: AppHandle, state: State<'_, Arc<AppState>>, project_id: String, on_event: Channel<PrdEvent>) -> Result<()> {
    let state = state.inner().clone();
    let key = state.studio.keys.get().map_err(|error| store_failed(&state, error))?.ok_or_else(|| gemini_refused("no_key"))?;
    let model = read(&state, |conn, _| {
        // The PRD follows the approved tech stack (R3).
        hub_core::stack::require_for_prd(conn, &project_id)?;
        Ok(settings::load(conn)?.gemini_model)
    })?;
    write(&app, &state, |conn, ctx| Ok(((), studio::reach_stage(conn, &project_id, StudioStage::Prd, Actor::Owner, ctx.now)?)))?;
    let (context, _) = project_context(&state, &project_id)?;
    let (stop, mut cancel) = watch::channel(false);
    if let Ok(mut replies) = state.studio.replies.lock() {
        replies.insert(project_id.clone(), stop);
    }
    let total = hub_ai::prd::PARTS.len();
    let mut on_step = {
        let channel = on_event.clone();
        move |step: hub_ai::prd::Step| {
            let _ = channel.send(PrdEvent::Step { part: step.part, total, repair: step.repair });
        }
    };
    let check = |doc: &Value| -> Vec<hub_ai::prd::Found> {
        let problems = match prd::assemble(doc) {
            Ok(draft) => prd::check(&draft),
            Err(problems) => problems,
        };
        problems.into_iter().map(|p| (p.at, p.message)).collect()
    };
    // The technologies are the approved stack's (R4), whatever Gemini wrote there.
    let stack = read(&state, |conn, _| hub_core::stack::get(conn, &project_id))?
        .filter(|s| s.status == hub_core::stack::StackStatus::Approved)
        .map(|s| s.body);
    let outcome: std::result::Result<Prd, (GeminiError, usize)> =
        match hub_ai::prd::generate(&state.studio.gemini, &key, &model, &context, &mut cancel, &mut on_step, &check).await {
            Ok(doc) => prd::assemble(&doc).map(|draft| match &stack {
                Some(stack) => prd::with_stack(&draft, stack),
                None => draft,
            }).map_err(|problems| {
                (GeminiError { kind: FailKind::BadResponse, status: None, code: None, message: "PRD tidak bisa dibaca".into(), retry_after: None }, problems.len())
            }),
            Err(error) => Err((error, 0)),
        };
    if let Ok(mut replies) = state.studio.replies.lock() {
        replies.remove(&project_id);
    }
    match outcome {
        Ok(draft) => {
            let problems = prd::check(&draft).len();
            write(&app, &state, |conn, ctx| {
                let (_, change) = prd::save_generated(conn, &project_id, &draft, ctx.now)?;
                Ok(((), Some(change)))
            })?;
            let _ = on_event.send(PrdEvent::Finished { state: MessageState::Complete, error_kind: None, problems });
        }
        Err((error, problems)) => {
            let state_now = if error.kind == FailKind::Cancelled { MessageState::Stopped } else { MessageState::Failed };
            if error.kind != FailKind::Cancelled {
                log::error(state.hub.paths(), &format!("studio prd: {}", error.log_line()));
            }
            let kind = (error.kind != FailKind::Cancelled).then(|| error.kind.as_str().to_string());
            let _ = on_event.send(PrdEvent::Finished { state: state_now, error_kind: kind, problems });
        }
    }
    Ok(())
}

/// "Perbarui dari tech stack": the draft's technologies follow the approved stack again (R4).
#[tauri::command(async)]
pub fn apply_stack_to_prd(app: AppHandle, state: Hub<'_>, project_id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(prd::refresh_stack(conn, &project_id, ctx.now)?))))
}

/// The owner's edit of one section of the draft.
#[tauri::command(async)]
pub fn update_prd_section(app: AppHandle, state: Hub<'_>, project_id: String, section: PrdSection, value: Value) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(prd::update_section(conn, &project_id, section, value, StatedBy::Owner, ctx.now)?))))
}

/// Gemini's revision of one section, as a proposal: nothing is saved until the owner accepts it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrdProposal {
    pub section: PrdSection,
    pub value: Value,
    pub change_note: String,
    pub impacts: Vec<String>,
    /// The draft's problems if the proposal were accepted.
    pub problems: Vec<Problem>,
}

#[tauri::command]
pub async fn revise_prd_section(state: State<'_, Arc<AppState>>, project_id: String, section: PrdSection, ask: String) -> Result<PrdProposal> {
    let state = state.inner().clone();
    let request = ask.trim().to_string();
    if request.is_empty() || request.chars().count() > 2000 {
        return Err(ErrorView { kind: ErrorKind::Invalid, field: Some("ask".into()), reason: Some("length".into()), message: "ask".into() });
    }
    let key = state.studio.keys.get().map_err(|error| store_failed(&state, error))?.ok_or_else(|| gemini_refused("no_key"))?;
    let model = read(&state, |conn, _| Ok(settings::load(conn)?.gemini_model))?;
    let draft = read(&state, |conn, _| prd::view(conn, &project_id))?.draft.ok_or_else(|| gemini_refused("no_prd"))?;
    let (context, _) = project_context(&state, &project_id)?;
    let current = prd::section_of(&draft, section);
    let body = hub_ai::prd::revise_request(&model, &context, section.as_str(), &current, &request);
    let (answer, _, _) = hub_ai::prd::ask_json(&state.studio.gemini, &key, &body).await.map_err(|error| {
        log::error(state.hub.paths(), &format!("studio revise: {}", error.log_line()));
        gemini_failed(error)
    })?;
    let value = answer.get("value").cloned().unwrap_or(Value::Null);
    let next = prd::with_section(&draft, section, value.clone()).map_err(|_| gemini_refused("bad_response"))?;
    let text = |v: &Value| v.as_str().unwrap_or("").trim().chars().take(1000).collect::<String>();
    Ok(PrdProposal {
        section,
        value,
        change_note: text(&answer["changeNote"]),
        impacts: answer["impacts"].as_array().into_iter().flatten().map(text).filter(|s| !s.is_empty()).take(10).collect(),
        problems: prd::check(&next),
    })
}

/// The owner accepts Gemini's revision: the section is Gemini's, accepted.
#[tauri::command(async)]
pub fn accept_prd_revision(app: AppHandle, state: Hub<'_>, project_id: String, section: PrdSection, value: Value) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(prd::update_section(conn, &project_id, section, value, StatedBy::Gemini, ctx.now)?))))
}

#[tauri::command(async)]
pub fn save_prd_version(app: AppHandle, state: Hub<'_>, project_id: String, note: String) -> Result<u32> {
    write(&app, &state, |conn, ctx| {
        let (number, change) = prd::save_version(conn, &project_id, &note, ctx.now)?;
        Ok((number, Some(change)))
    })
}

#[tauri::command(async)]
pub fn approve_prd(app: AppHandle, state: Hub<'_>, project_id: String) -> Result<u32> {
    write(&app, &state, |conn, ctx| {
        let (number, change) = prd::approve(conn, &project_id, ctx.now)?;
        Ok((number, Some(change)))
    })
}

/// "Ekspor Markdown": the draft, or a numbered version, as a .md file where the owner chooses.
#[tauri::command(async)]
pub fn export_prd(app: AppHandle, state: Hub<'_>, project_id: String, version_id: Option<String>) -> Result<Option<String>> {
    let (doc, meta, name) = read(&state, |conn, _| {
        let view = prd::view(conn, &project_id)?;
        let name = studio::get_project(conn, &project_id)?.name;
        match &version_id {
            Some(id) => {
                let meta = view.versions.iter().find(|v| &v.id == id).cloned().ok_or(hub_core::Error::NotFound("prdVersion"))?;
                Ok((prd::version_body(conn, id)?, Some(meta), name))
            }
            None => Ok((view.draft.ok_or(hub_core::Error::NotFound("prd"))?, view.draft_meta, name)),
        }
    })?;
    let text = prd::markdown(&doc, meta.as_ref());
    let number = meta.as_ref().and_then(|m| m.number).map(|n| format!("-v{n}")).unwrap_or_default();
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '-' }).collect();
    let file = format!("PRD {}{number}.md", safe.trim());
    let owner = crate::library::main_hwnd(&app);
    let Some(path) = crate::shell::on_com_thread(move || crate::shell::save_file(owner, "Ekspor PRD", &file, "md")).flatten() else {
        return Ok(None);
    };
    std::fs::write(&path, text).map_err(|error| ErrorView { kind: ErrorKind::Internal, field: None, reason: None, message: error.to_string() })?;
    Ok(Some(path.display().to_string()))
}
