//! Hub Studio (docs/studio.md): ideas talked through with Gemini on their way to a PRD, a roadmap,
//! and Claude Code's work. This module holds the projects, the conversation, and the ledger of
//! what the conversation settled; PRDs, roadmaps, and reports follow in later checkpoints.
//!
//! The conversation is its own record: messages leave no row in `activity` (a chat would bury
//! Jejak). Projects and ledger items do, like every other write in Hub.

use jiff::Timestamp;
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::activity::{self, Record};
use crate::error::{invalid, Error, Reason, Result};
use crate::ids::{self, new_id};
use crate::model::{text_enum, Actor, Change, Entity, Verb};
use crate::time::instant;

const NAME_MAX: usize = 120;
const IDEA_MAX: usize = 2000;
/// What the owner may type in one message.
const OWNER_TEXT_MAX: usize = 20_000;
/// A reply is cut by Gemini's output limit long before this; the check guards the table.
const REPLY_TEXT_MAX: usize = 100_000;
const TOPIC_MAX: usize = 120;
const STATEMENT_MAX: usize = 2000;
/// An error kind stored with a failed reply (`offline`, `quota`, ...), not a message.
const ERROR_KIND_MAX: usize = 40;
const MODEL_MAX: usize = 80;

text_enum!(
    /// Where a Studio project is on its way from idea to built.
    StudioStage {
        Explore => "explore",
        Clarify => "clarify",
        Prd => "prd",
        Roadmap => "roadmap",
        Build => "build",
    }
);

text_enum!(
    /// What Claude Code may do with a project (docs/studio.md, "Access").
    ClaudeAccess {
        NoAccess => "none",
        Read => "read",
        Report => "report",
    }
);

text_enum!(
    MessageRole {
        Owner => "owner",
        Model => "model",
    }
);

text_enum!(
    /// A reply is `streaming` from its first word until it is finished, stopped, or failed.
    MessageState {
        Streaming => "streaming",
        Complete => "complete",
        Stopped => "stopped",
        Failed => "failed",
    }
);

text_enum!(
    LedgerKind {
        Decision => "decision",
        Assumption => "assumption",
        Question => "question",
        Constraint => "constraint",
    }
);

text_enum!(
    /// `confirmed` is for assumptions (now a decision), `answered` for questions.
    LedgerStatus {
        Active => "active",
        Confirmed => "confirmed",
        Retired => "retired",
        Answered => "answered",
    }
);

text_enum!(
    /// The part of the idea a ledger item settles (R2): what "Yang sudah jelas" counts.
    LedgerArea {
        Problem => "problem",
        Users => "users",
        Features => "features",
        Constraints => "constraints",
        Success => "success",
    }
);

text_enum!(
    StatedBy {
        Owner => "owner",
        Gemini => "gemini",
    }
);

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct StudioProjectInput {
    pub name: String,
    #[serde(default)]
    pub idea: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StudioProject {
    pub id: String,
    pub name: String,
    pub idea: String,
    pub stage: StudioStage,
    /// The Hub project it is linked to, once its roadmap is approved.
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub repo_path: Option<String>,
    pub claude_access: ClaudeAccess,
    /// Open requests waiting for the owner (questions, blockers, reports to review).
    pub waiting: u32,
    /// Stage 2's tech stack (R3): none yet, a draft, or approved.
    pub stack_status: Option<crate::stack::StackStatus>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StudioMessage {
    pub id: String,
    pub role: MessageRole,
    pub stage: StudioStage,
    pub text: String,
    /// Structured parts of a reply (questions with options, a proposed stage), checked when
    /// they were written.
    #[ts(type = "unknown")]
    pub parts: serde_json::Value,
    pub state: MessageState,
    pub error_kind: Option<String>,
    pub model: Option<String>,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub created_at: String,
}

/// How a reply ended.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplyEnd {
    pub text: String,
    pub parts: serde_json::Value,
    pub state: MessageState,
    pub error_kind: Option<String>,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LedgerItem {
    pub id: String,
    pub kind: LedgerKind,
    pub topic: String,
    pub statement: String,
    pub rationale: String,
    pub status: LedgerStatus,
    /// The part of the idea it settles; `None` for items from before R2 or that fit no part.
    pub area: Option<LedgerArea>,
    pub stated_by: StatedBy,
    /// The message it came from.
    pub message_id: Option<String>,
    /// The item it replaced, when a decision changed.
    pub replaces_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct LedgerInput {
    pub kind: LedgerKind,
    pub topic: String,
    pub statement: String,
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    #[ts(optional)]
    pub area: Option<LedgerArea>,
    #[serde(default)]
    #[ts(optional)]
    pub replaces_id: Option<String>,
}

/// A change to a ledger item; fields left out stay as they are.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct LedgerEdit {
    #[ts(optional)]
    pub topic: Option<String>,
    #[ts(optional)]
    pub statement: Option<String>,
    #[ts(optional)]
    pub rationale: Option<String>,
    #[ts(optional)]
    pub status: Option<LedgerStatus>,
    #[ts(optional)]
    pub area: Option<LedgerArea>,
}

// ---------- checks ----------

fn text(field: &'static str, value: &str, min: usize, max: usize) -> Result<String> {
    let value = value.trim();
    let count = value.chars().count();
    if count < min || count > max {
        return invalid(field, Reason::Length);
    }
    Ok(value.to_string())
}

fn status_fits(kind: LedgerKind, status: LedgerStatus) -> Result<()> {
    match status {
        LedgerStatus::Confirmed if kind != LedgerKind::Assumption => invalid("status", Reason::Format),
        LedgerStatus::Answered if kind != LedgerKind::Question => invalid("status", Reason::Format),
        _ => Ok(()),
    }
}

/// A project that exists and is not removed: its name and stage.
fn live_project(conn: &Connection, id: &str) -> Result<(String, StudioStage)> {
    conn.query_row(
        "SELECT name, stage FROM studio_projects WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()?
    .ok_or(Error::NotFound("studioProject"))
}

fn touch(tx: &Transaction<'_>, id: &str, now: Timestamp) -> Result<()> {
    tx.execute("UPDATE studio_projects SET updated_at = ?2 WHERE id = ?1", params![id, instant(now)])?;
    Ok(())
}

fn record(tx: &Transaction<'_>, now: Timestamp, actor: Actor, verb: Verb, entity: Entity, id: &str, summary: &str) -> Result<()> {
    activity::record(tx, now, Record { actor, verb, entity, entity_id: Some(id), summary, details: serde_json::json!({}) })
}

// ---------- projects ----------

const PROJECT_COLUMNS: &str = "s.id, s.name, s.idea, s.stage, s.project_id, p.name, s.repo_path, s.claude_access,
    (SELECT count(*) FROM studio_requests r WHERE r.studio_project_id = s.id AND r.status = 'open' AND r.made_by <> 'owner'),
    s.created_at, s.updated_at, (SELECT t.status FROM studio_stacks t WHERE t.studio_project_id = s.id)";
const PROJECT_FROM: &str = "studio_projects s LEFT JOIN projects p ON p.id = s.project_id AND p.deleted_at IS NULL";

fn project_row(row: &Row<'_>) -> rusqlite::Result<StudioProject> {
    Ok(StudioProject {
        id: row.get(0)?,
        name: row.get(1)?,
        idea: row.get(2)?,
        stage: row.get(3)?,
        project_id: row.get(4)?,
        project_name: row.get(5)?,
        repo_path: row.get(6)?,
        claude_access: row.get(7)?,
        waiting: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        stack_status: row.get(11)?,
    })
}

pub fn create_project(conn: &mut Connection, input: StudioProjectInput, actor: Actor, now: Timestamp) -> Result<(String, Change)> {
    let name = text("name", &input.name, 1, NAME_MAX)?;
    let idea = text("idea", &input.idea, 0, IDEA_MAX)?;
    let tx = conn.transaction()?;
    let id = new_id();
    let stamp = instant(now);
    tx.execute(
        "INSERT INTO studio_projects (id, name, idea, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        params![id, name, idea, stamp],
    )?;
    record(&tx, now, actor, Verb::Created, Entity::StudioProject, &id, &name)?;
    tx.commit()?;
    Ok((id.clone(), Change::one(Entity::StudioProject, id)))
}

/// The projects, the one touched last first.
pub fn list_projects(conn: &Connection) -> Result<Vec<StudioProject>> {
    let mut statement = conn.prepare(&format!(
        "SELECT {PROJECT_COLUMNS} FROM {PROJECT_FROM} WHERE s.deleted_at IS NULL ORDER BY s.updated_at DESC, s.id DESC"
    ))?;
    let rows = statement.query_map([], project_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get_project(conn: &Connection, id: &str) -> Result<StudioProject> {
    let id = ids::parse("id", id)?;
    conn.query_row(
        &format!("SELECT {PROJECT_COLUMNS} FROM {PROJECT_FROM} WHERE s.id = ?1 AND s.deleted_at IS NULL"),
        [id],
        project_row,
    )
    .optional()?
    .ok_or(Error::NotFound("studioProject"))
}

pub fn rename_project(conn: &mut Connection, id: &str, name: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let name = text("name", name, 1, NAME_MAX)?;
    let tx = conn.transaction()?;
    live_project(&tx, &id)?;
    tx.execute(
        "UPDATE studio_projects SET name = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, name, instant(now)],
    )?;
    record(&tx, now, actor, Verb::Updated, Entity::StudioProject, &id, &name)?;
    tx.commit()?;
    Ok(Change::one(Entity::StudioProject, id))
}

/// Remove a project (soft, like everything in Hub). Its conversation and documents stay with it.
pub fn delete_project(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let (name, _) = live_project(&tx, &id)?;
    let stamp = instant(now);
    tx.execute("UPDATE studio_projects SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1", params![id, stamp])?;
    record(&tx, now, actor, Verb::Deleted, Entity::StudioProject, &id, &name)?;
    tx.commit()?;
    Ok(Change::one(Entity::StudioProject, id))
}

/// The stage after `stage`, if any.
pub fn next_stage(stage: StudioStage) -> Option<StudioStage> {
    match stage {
        StudioStage::Explore => Some(StudioStage::Clarify),
        StudioStage::Clarify => Some(StudioStage::Prd),
        StudioStage::Prd => Some(StudioStage::Roadmap),
        StudioStage::Roadmap => Some(StudioStage::Build),
        StudioStage::Build => None,
    }
}

/// Move a project to the next stage. Stages only move forward, one at a time, when the owner
/// says so (Gemini may only propose). Moving into PRD, Roadmap, and Bangun also needs their
/// documents, which later checkpoints add; `allowed` lists what this Hub can do so far.
pub fn advance_stage(conn: &mut Connection, id: &str, to: StudioStage, allowed: &[StudioStage], actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let (name, stage) = live_project(&tx, &id)?;
    if next_stage(stage) != Some(to) || !allowed.contains(&to) {
        return invalid("stage", Reason::Order);
    }
    tx.execute("UPDATE studio_projects SET stage = ?2, updated_at = ?3 WHERE id = ?1", params![id, to, instant(now)])?;
    activity::record(
        &tx,
        now,
        Record { actor, verb: Verb::Moved, entity: Entity::StudioProject, entity_id: Some(&id), summary: &name, details: serde_json::json!({ "from": stage, "to": to }) },
    )?;
    tx.commit()?;
    Ok(Change::one(Entity::StudioProject, id))
}

/// The owner asked for something a later stage does (Susun PRD from the conversation): the project
/// moves forward to `to` if it is not there yet. Never backward.
pub fn reach_stage(conn: &mut Connection, id: &str, to: StudioStage, actor: Actor, now: Timestamp) -> Result<Option<Change>> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let (name, stage) = live_project(&tx, &id)?;
    let order = |s: StudioStage| StudioStage::ALL.iter().position(|x| *x == s).unwrap_or(0);
    if order(stage) >= order(to) {
        return Ok(None);
    }
    tx.execute("UPDATE studio_projects SET stage = ?2, updated_at = ?3 WHERE id = ?1", params![id, to, instant(now)])?;
    activity::record(
        &tx,
        now,
        Record { actor, verb: Verb::Moved, entity: Entity::StudioProject, entity_id: Some(&id), summary: &name, details: serde_json::json!({ "from": stage, "to": to }) },
    )?;
    tx.commit()?;
    Ok(Some(Change::one(Entity::StudioProject, id)))
}

/// A name for a project started from an idea: its first line, cut at a word near 60 characters.
pub fn name_from_idea(idea: &str) -> String {
    const MAX: usize = 60;
    let line = idea.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("Ide baru");
    if line.chars().count() <= MAX {
        return line.to_string();
    }
    let cut: String = line.chars().take(MAX).collect();
    let at_word = cut.rfind(' ').filter(|&i| i > MAX / 2).map(|i| cut[..i].to_string()).unwrap_or(cut);
    format!("{}…", at_word.trim_end_matches([',', '.', ';', ':', ' ']))
}

/// Keep Gemini's summary of the conversation up to `until` (a message id) with the project.
pub fn set_summary(conn: &mut Connection, id: &str, summary: &str, until: &str) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let until = ids::parse("until", until)?;
    let summary = text("summary", summary, 1, 20_000)?;
    let tx = conn.transaction()?;
    live_project(&tx, &id)?;
    let belongs = tx
        .query_row("SELECT 1 FROM studio_messages WHERE id = ?1 AND studio_project_id = ?2", params![until, id], |_| Ok(()))
        .optional()?;
    if belongs.is_none() {
        return invalid("until", Reason::Unknown);
    }
    tx.execute(
        "UPDATE studio_projects SET summary = ?2, summary_until = ?3 WHERE id = ?1",
        params![id, summary, until],
    )?;
    tx.commit()?;
    Ok(Change::one(Entity::StudioProject, id))
}

/// The summary kept so far and the message it covers up to.
pub fn summary(conn: &Connection, id: &str) -> Result<(String, Option<String>)> {
    let id = ids::parse("id", id)?;
    live_project(conn, &id)?;
    Ok(conn.query_row("SELECT summary, summary_until FROM studio_projects WHERE id = ?1", [id], |row| Ok((row.get(0)?, row.get(1)?)))?)
}

// ---------- the conversation ----------

fn message_row(row: &Row<'_>) -> rusqlite::Result<StudioMessage> {
    let parts: String = row.get(4)?;
    Ok(StudioMessage {
        id: row.get(0)?,
        role: row.get(1)?,
        stage: row.get(2)?,
        text: row.get(3)?,
        parts: serde_json::from_str(&parts).unwrap_or_else(|_| serde_json::json!({})),
        state: row.get(5)?,
        error_kind: row.get(6)?,
        model: row.get(7)?,
        input_tokens: row.get(8)?,
        output_tokens: row.get(9)?,
        created_at: row.get(10)?,
    })
}

/// The owner's message, in the project's current stage.
pub fn add_owner_message(conn: &mut Connection, project_id: &str, message: &str, now: Timestamp) -> Result<(String, Change)> {
    let project_id = ids::parse("projectId", project_id)?;
    let message = text("text", message, 1, OWNER_TEXT_MAX)?;
    let tx = conn.transaction()?;
    let (_, stage) = live_project(&tx, &project_id)?;
    let id = new_id();
    let stamp = instant(now);
    tx.execute(
        "INSERT INTO studio_messages (id, studio_project_id, role, stage, text, created_at, updated_at)
         VALUES (?1, ?2, 'owner', ?3, ?4, ?5, ?5)",
        params![id, project_id, stage, message, stamp],
    )?;
    touch(&tx, &project_id, now)?;
    tx.commit()?;
    Ok((id.clone(), Change::one(Entity::StudioMessage, id)))
}

/// A reply from `model` begins: an empty message in the `streaming` state. Only one reply per
/// project streams at a time.
pub fn begin_reply(conn: &mut Connection, project_id: &str, model: &str, now: Timestamp) -> Result<(String, Change)> {
    let project_id = ids::parse("projectId", project_id)?;
    let model = text("model", model, 1, MODEL_MAX)?;
    let tx = conn.transaction()?;
    let (_, stage) = live_project(&tx, &project_id)?;
    let busy: Option<String> = tx
        .query_row(
            "SELECT id FROM studio_messages WHERE studio_project_id = ?1 AND state = 'streaming' LIMIT 1",
            [&project_id],
            |row| row.get(0),
        )
        .optional()?;
    if busy.is_some() {
        return invalid("reply", Reason::TooMany);
    }
    let id = new_id();
    let stamp = instant(now);
    tx.execute(
        "INSERT INTO studio_messages (id, studio_project_id, role, stage, text, state, model, created_at, updated_at)
         VALUES (?1, ?2, 'model', ?3, '', 'streaming', ?4, ?5, ?5)",
        params![id, project_id, stage, model, stamp],
    )?;
    tx.commit()?;
    Ok((id.clone(), Change::one(Entity::StudioMessage, id)))
}

/// Finish a streaming reply, once: complete, stopped by the owner (what arrived is kept), or
/// failed (with the error's kind).
pub fn finish_reply(conn: &mut Connection, id: &str, end: ReplyEnd, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    if end.state == MessageState::Streaming {
        return invalid("state", Reason::Format);
    }
    if end.text.chars().count() > REPLY_TEXT_MAX {
        return invalid("text", Reason::Length);
    }
    if !end.parts.is_object() {
        return invalid("parts", Reason::Format);
    }
    let error_kind = match &end.error_kind {
        Some(kind) => Some(text("errorKind", kind, 1, ERROR_KIND_MAX)?),
        None => None,
    };
    let tx = conn.transaction()?;
    let project: Option<String> = tx
        .query_row(
            "SELECT studio_project_id FROM studio_messages WHERE id = ?1 AND state = 'streaming'",
            [&id],
            |row| row.get(0),
        )
        .optional()?;
    let project = project.ok_or(Error::NotFound("reply"))?;
    tx.execute(
        "UPDATE studio_messages SET text = ?2, parts = ?3, state = ?4, error_kind = ?5, input_tokens = ?6,
                output_tokens = ?7, updated_at = ?8
         WHERE id = ?1",
        params![
            id,
            end.text,
            end.parts.to_string(),
            end.state,
            error_kind,
            end.input_tokens,
            end.output_tokens,
            instant(now)
        ],
    )?;
    touch(&tx, &project, now)?;
    tx.commit()?;
    Ok(Change::one(Entity::StudioMessage, id))
}

/// Replies left `streaming` by a Hub that stopped mid-answer are marked failed (`interrupted`).
/// Run once at start. Returns how many.
pub fn recover_interrupted(conn: &Connection, now: Timestamp) -> Result<usize> {
    Ok(conn.execute(
        "UPDATE studio_messages SET state = 'failed', error_kind = 'interrupted', updated_at = ?1 WHERE state = 'streaming'",
        [instant(now)],
    )?)
}

/// The conversation, oldest first.
pub fn list_messages(conn: &Connection, project_id: &str) -> Result<Vec<StudioMessage>> {
    let project_id = ids::parse("projectId", project_id)?;
    live_project(conn, &project_id)?;
    let mut statement = conn.prepare(
        "SELECT id, role, stage, text, parts, state, error_kind, model, input_tokens, output_tokens, created_at
         FROM studio_messages WHERE studio_project_id = ?1 ORDER BY id",
    )?;
    let rows = statement.query_map([project_id], message_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

// ---------- the ledger ----------

const LEDGER_COLUMNS: &str =
    "id, kind, topic, statement, rationale, status, stated_by, message_id, replaces_id, created_at, updated_at, area";

fn ledger_row(row: &Row<'_>) -> rusqlite::Result<LedgerItem> {
    Ok(LedgerItem {
        id: row.get(0)?,
        kind: row.get(1)?,
        topic: row.get(2)?,
        statement: row.get(3)?,
        rationale: row.get(4)?,
        status: row.get(5)?,
        stated_by: row.get(6)?,
        message_id: row.get(7)?,
        replaces_id: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        area: row.get(11)?,
    })
}

fn actor_of(stated_by: StatedBy) -> Actor {
    match stated_by {
        StatedBy::Owner => Actor::Owner,
        StatedBy::Gemini => Actor::Gemini,
    }
}

/// Add an item to a project's ledger. An item that `replaces` another retires it in the same
/// write, so the ledger never holds two versions of one decision as active.
pub fn add_ledger_item(
    conn: &mut Connection,
    project_id: &str,
    input: LedgerInput,
    stated_by: StatedBy,
    message_id: Option<&str>,
    now: Timestamp,
) -> Result<(String, Change)> {
    let project_id = ids::parse("projectId", project_id)?;
    let topic = text("topic", &input.topic, 1, TOPIC_MAX)?;
    let statement = text("statement", &input.statement, 1, STATEMENT_MAX)?;
    let rationale = text("rationale", &input.rationale, 0, STATEMENT_MAX)?;
    let replaces = input.replaces_id.as_deref().map(|id| ids::parse("replacesId", id)).transpose()?;
    let message_id = message_id.map(|id| ids::parse("messageId", id)).transpose()?;
    let tx = conn.transaction()?;
    live_project(&tx, &project_id)?;
    if let Some(message) = &message_id {
        let found = tx
            .query_row(
                "SELECT 1 FROM studio_messages WHERE id = ?1 AND studio_project_id = ?2",
                params![message, project_id],
                |_| Ok(()),
            )
            .optional()?;
        if found.is_none() {
            return invalid("messageId", Reason::Unknown);
        }
    }
    let stamp = instant(now);
    if let Some(old) = &replaces {
        let changed = tx.execute(
            "UPDATE studio_ledger SET status = 'retired', updated_at = ?3
             WHERE id = ?1 AND studio_project_id = ?2 AND status = 'active'",
            params![old, project_id, stamp],
        )?;
        if changed == 0 {
            return invalid("replacesId", Reason::Unknown);
        }
    }
    let id = new_id();
    tx.execute(
        "INSERT INTO studio_ledger (id, studio_project_id, kind, topic, statement, rationale, stated_by, message_id,
                                    replaces_id, created_at, updated_at, area)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10, ?11)",
        params![id, project_id, input.kind, topic, statement, rationale, stated_by, message_id, replaces, stamp, input.area],
    )?;
    touch(&tx, &project_id, now)?;
    record(&tx, now, actor_of(stated_by), Verb::Created, Entity::StudioLedger, &id, &topic)?;
    tx.commit()?;
    Ok((id.clone(), Change::one(Entity::StudioLedger, id)))
}

/// The owner edits an item, or settles it: an assumption confirmed, a question answered, an item
/// retired.
pub fn edit_ledger_item(conn: &mut Connection, id: &str, edit: LedgerEdit, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    type Found = (String, LedgerKind, String, String, String, LedgerStatus, Option<LedgerArea>);
    let found: Option<Found> = tx
        .query_row(
            "SELECT l.studio_project_id, l.kind, l.topic, l.statement, l.rationale, l.status, l.area
             FROM studio_ledger l JOIN studio_projects s ON s.id = l.studio_project_id
             WHERE l.id = ?1 AND s.deleted_at IS NULL",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
        )
        .optional()?;
    let (project, kind, topic, statement, rationale, status, area) = found.ok_or(Error::NotFound("ledgerItem"))?;
    let topic = match &edit.topic {
        Some(value) => text("topic", value, 1, TOPIC_MAX)?,
        None => topic,
    };
    let statement = match &edit.statement {
        Some(value) => text("statement", value, 1, STATEMENT_MAX)?,
        None => statement,
    };
    let rationale = match &edit.rationale {
        Some(value) => text("rationale", value, 0, STATEMENT_MAX)?,
        None => rationale,
    };
    let status = edit.status.unwrap_or(status);
    status_fits(kind, status)?;
    let area = edit.area.or(area);
    tx.execute(
        "UPDATE studio_ledger SET topic = ?2, statement = ?3, rationale = ?4, status = ?5, updated_at = ?6, area = ?7 WHERE id = ?1",
        params![id, topic, statement, rationale, status, instant(now), area],
    )?;
    touch(&tx, &project, now)?;
    record(&tx, now, actor, Verb::Updated, Entity::StudioLedger, &id, &topic)?;
    tx.commit()?;
    Ok(Change::one(Entity::StudioLedger, id))
}

/// A project's ledger, by kind, oldest first within a kind.
pub fn list_ledger(conn: &Connection, project_id: &str) -> Result<Vec<LedgerItem>> {
    let project_id = ids::parse("projectId", project_id)?;
    live_project(conn, &project_id)?;
    let mut statement = conn.prepare(&format!(
        "SELECT {LEDGER_COLUMNS} FROM studio_ledger WHERE studio_project_id = ?1
         ORDER BY CASE kind WHEN 'decision' THEN 0 WHEN 'assumption' THEN 1 WHEN 'question' THEN 2 ELSE 3 END, id"
    ))?;
    let rows = statement.query_map([project_id], ledger_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    fn now() -> Timestamp {
        "2026-11-06T03:00:00Z".parse().unwrap()
    }

    fn later(minutes: i64) -> Timestamp {
        now() + jiff::SignedDuration::from_mins(minutes)
    }

    fn project(conn: &mut Connection) -> String {
        create_project(conn, StudioProjectInput { name: "Absensi lab".into(), idea: "absen pakai kartu".into() }, Actor::Owner, now())
            .unwrap()
            .0
    }

    fn activity_count(conn: &Connection, entity: &str) -> i64 {
        conn.query_row("SELECT count(*) FROM activity WHERE entity_type = ?1", [entity], |row| row.get(0)).unwrap()
    }

    #[test]
    fn a_project_is_created_listed_renamed_and_removed() {
        let mut conn = open_in_memory().unwrap();
        let id = project(&mut conn);
        let found = get_project(&conn, &id).unwrap();
        assert_eq!(found.name, "Absensi lab");
        assert_eq!(found.stage, StudioStage::Explore);
        assert_eq!(found.claude_access, ClaudeAccess::NoAccess);
        assert_eq!(found.waiting, 0);
        assert_eq!(found.project_id, None);

        let other = create_project(&mut conn, StudioProjectInput { name: "Kedua".into(), idea: String::new() }, Actor::Owner, later(5)).unwrap().0;
        let names: Vec<String> = list_projects(&conn).unwrap().into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["Kedua", "Absensi lab"], "the one touched last first");

        rename_project(&mut conn, &id, "  Absensi lab SMK  ", Actor::Owner, later(10)).unwrap();
        assert_eq!(get_project(&conn, &id).unwrap().name, "Absensi lab SMK");
        assert_eq!(list_projects(&conn).unwrap()[0].id, id);

        delete_project(&mut conn, &other, Actor::Owner, later(11)).unwrap();
        assert!(matches!(get_project(&conn, &other), Err(Error::NotFound(_))));
        assert_eq!(list_projects(&conn).unwrap().len(), 1);
        assert_eq!(activity_count(&conn, "studio_project"), 4, "created twice, renamed, removed");
    }

    #[test]
    fn project_input_is_checked() {
        let mut conn = open_in_memory().unwrap();
        let empty = create_project(&mut conn, StudioProjectInput { name: "   ".into(), idea: String::new() }, Actor::Owner, now());
        assert!(matches!(empty, Err(Error::Invalid { field: "name", reason: Reason::Length })));
        let long = create_project(&mut conn, StudioProjectInput { name: "x".repeat(121), idea: String::new() }, Actor::Owner, now());
        assert!(long.is_err());
        let idea = create_project(&mut conn, StudioProjectInput { name: "Ok".into(), idea: "x".repeat(2001) }, Actor::Owner, now());
        assert!(matches!(idea, Err(Error::Invalid { field: "idea", .. })));
        assert!(matches!(get_project(&conn, "not-an-id"), Err(Error::Invalid { field: "id", .. })));
    }

    #[test]
    fn a_conversation_keeps_its_order_and_its_replies_finish_once() {
        let mut conn = open_in_memory().unwrap();
        let id = project(&mut conn);
        add_owner_message(&mut conn, &id, "Aku mau bikin absensi lab", now()).unwrap();
        let (reply, _) = begin_reply(&mut conn, &id, "gemini-3.8-flash", now()).unwrap();
        assert!(begin_reply(&mut conn, &id, "gemini-3.8-flash", now()).is_err(), "one reply streams at a time");

        let end = ReplyEnd {
            text: "Siapa yang akan absen?".into(),
            parts: serde_json::json!({"questions": []}),
            state: MessageState::Complete,
            error_kind: None,
            input_tokens: Some(120),
            output_tokens: Some(30),
        };
        finish_reply(&mut conn, &reply, end.clone(), later(1)).unwrap();
        assert!(matches!(finish_reply(&mut conn, &reply, end, later(2)), Err(Error::NotFound("reply"))), "finished once");

        add_owner_message(&mut conn, &id, "Siswa TJKT", later(3)).unwrap();
        let messages = list_messages(&conn, &id).unwrap();
        let roles: Vec<MessageRole> = messages.iter().map(|m| m.role).collect();
        assert_eq!(roles, [MessageRole::Owner, MessageRole::Model, MessageRole::Owner]);
        assert_eq!(messages[1].text, "Siapa yang akan absen?");
        assert_eq!(messages[1].state, MessageState::Complete);
        assert_eq!(messages[1].input_tokens, Some(120));
        assert_eq!(messages[1].model.as_deref(), Some("gemini-3.8-flash"));
        assert_eq!(messages[0].stage, StudioStage::Explore);
        assert_eq!(activity_count(&conn, "studio_message"), 0, "the conversation is its own record");
    }

    #[test]
    fn a_stopped_reply_keeps_what_arrived_and_a_crash_leaves_no_reply_streaming() {
        let mut conn = open_in_memory().unwrap();
        let id = project(&mut conn);
        add_owner_message(&mut conn, &id, "Halo", now()).unwrap();
        let (stopped, _) = begin_reply(&mut conn, &id, "m", now()).unwrap();
        finish_reply(
            &mut conn,
            &stopped,
            ReplyEnd { text: "Separuh".into(), parts: serde_json::json!({}), state: MessageState::Stopped, error_kind: None, input_tokens: None, output_tokens: None },
            later(1),
        )
        .unwrap();
        let (cut, _) = begin_reply(&mut conn, &id, "m", later(2)).unwrap();
        assert_eq!(recover_interrupted(&conn, later(3)).unwrap(), 1);
        let messages = list_messages(&conn, &id).unwrap();
        assert_eq!(messages[1].text, "Separuh");
        assert_eq!(messages[1].state, MessageState::Stopped);
        let last = messages.iter().find(|m| m.id == cut).unwrap();
        assert_eq!(last.state, MessageState::Failed);
        assert_eq!(last.error_kind.as_deref(), Some("interrupted"));
        // And a new reply may start again.
        begin_reply(&mut conn, &id, "m", later(4)).unwrap();
    }

    #[test]
    fn messages_are_checked() {
        let mut conn = open_in_memory().unwrap();
        let id = project(&mut conn);
        assert!(add_owner_message(&mut conn, &id, "  ", now()).is_err());
        assert!(add_owner_message(&mut conn, &id, &"x".repeat(20_001), now()).is_err());
        let missing = new_id();
        assert!(matches!(add_owner_message(&mut conn, &missing, "Halo", now()), Err(Error::NotFound(_))));
        let (reply, _) = begin_reply(&mut conn, &id, "m", now()).unwrap();
        let bad = |state, parts| ReplyEnd { text: String::new(), parts, state, error_kind: None, input_tokens: None, output_tokens: None };
        assert!(finish_reply(&mut conn, &reply, bad(MessageState::Streaming, serde_json::json!({})), now()).is_err());
        assert!(finish_reply(&mut conn, &reply, bad(MessageState::Complete, serde_json::json!([1])), now()).is_err());
        delete_project(&mut conn, &id, Actor::Owner, now()).unwrap();
        assert!(add_owner_message(&mut conn, &id, "Halo", now()).is_err(), "a removed project takes no messages");
    }

    #[test]
    fn the_ledger_records_replaces_and_settles_items() {
        let mut conn = open_in_memory().unwrap();
        let id = project(&mut conn);
        let (message, _) = add_owner_message(&mut conn, &id, "Pakai Android saja", now()).unwrap();
        let input = |kind, topic: &str, statement: &str| LedgerInput {
            kind,
            topic: topic.into(),
            statement: statement.into(),
            rationale: String::new(),
            area: None,
            replaces_id: None,
        };
        let (platform, _) = add_ledger_item(&mut conn, &id, input(LedgerKind::Decision, "Platform", "Android"), StatedBy::Owner, Some(&message), now()).unwrap();
        let users_input = LedgerInput { area: Some(LedgerArea::Users), ..input(LedgerKind::Assumption, "Pengguna", "Siswa punya HP") };
        let (users, _) = add_ledger_item(&mut conn, &id, users_input, StatedBy::Gemini, None, now()).unwrap();
        let (question, _) = add_ledger_item(&mut conn, &id, input(LedgerKind::Question, "Data", "Disimpan di mana?"), StatedBy::Gemini, None, now()).unwrap();

        // A changed decision retires the old one in the same write.
        let replacement = LedgerInput { replaces_id: Some(platform.clone()), ..input(LedgerKind::Decision, "Platform", "Web yang bisa dibuka di HP") };
        let (web, _) = add_ledger_item(&mut conn, &id, replacement, StatedBy::Owner, None, later(1)).unwrap();
        let replaced_again = LedgerInput { replaces_id: Some(platform.clone()), ..input(LedgerKind::Decision, "Platform", "Desktop") };
        assert!(add_ledger_item(&mut conn, &id, replaced_again, StatedBy::Owner, None, later(2)).is_err(), "only an active item can be replaced");

        edit_ledger_item(&mut conn, &users, LedgerEdit { status: Some(LedgerStatus::Confirmed), ..Default::default() }, Actor::Owner, later(3)).unwrap();
        edit_ledger_item(&mut conn, &question, LedgerEdit { status: Some(LedgerStatus::Answered), statement: Some("Di server sekolah".into()), ..Default::default() }, Actor::Owner, later(4)).unwrap();
        assert!(edit_ledger_item(&mut conn, &web, LedgerEdit { status: Some(LedgerStatus::Answered), ..Default::default() }, Actor::Owner, later(5)).is_err(), "only questions are answered");
        assert!(edit_ledger_item(&mut conn, &question, LedgerEdit { status: Some(LedgerStatus::Confirmed), ..Default::default() }, Actor::Owner, later(5)).is_err(), "only assumptions are confirmed");

        let ledger = list_ledger(&conn, &id).unwrap();
        let summary: Vec<(LedgerKind, &str, LedgerStatus)> = ledger.iter().map(|i| (i.kind, i.statement.as_str(), i.status)).collect();
        assert_eq!(
            summary,
            [
                (LedgerKind::Decision, "Android", LedgerStatus::Retired),
                (LedgerKind::Decision, "Web yang bisa dibuka di HP", LedgerStatus::Active),
                (LedgerKind::Assumption, "Siswa punya HP", LedgerStatus::Confirmed),
                (LedgerKind::Question, "Di server sekolah", LedgerStatus::Answered),
            ]
        );
        assert_eq!(ledger[1].replaces_id.as_deref(), Some(platform.as_str()));
        // The part of the idea an item settles is kept, and an edit can place an item that had none.
        assert_eq!(ledger[2].area, Some(LedgerArea::Users));
        assert_eq!(ledger[1].area, None);
        edit_ledger_item(&mut conn, &web, LedgerEdit { area: Some(LedgerArea::Constraints), ..Default::default() }, Actor::Owner, later(6)).unwrap();
        assert_eq!(list_ledger(&conn, &id).unwrap()[1].area, Some(LedgerArea::Constraints));
        assert_eq!(ledger[0].message_id.as_deref(), Some(message.as_str()));
        let gemini_rows: i64 = conn
            .query_row("SELECT count(*) FROM activity WHERE entity_type = 'studio_ledger' AND actor = 'gemini'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(gemini_rows, 2, "what Gemini recorded is Gemini's in Jejak");
    }

    #[test]
    fn stages_move_forward_one_at_a_time_when_allowed() {
        let mut conn = open_in_memory().unwrap();
        let id = project(&mut conn);
        let allowed = [StudioStage::Clarify, StudioStage::Prd];
        assert!(advance_stage(&mut conn, &id, StudioStage::Prd, &allowed, Actor::Owner, now()).is_err(), "no skipping");
        advance_stage(&mut conn, &id, StudioStage::Clarify, &allowed, Actor::Owner, now()).unwrap();
        assert!(advance_stage(&mut conn, &id, StudioStage::Clarify, &allowed, Actor::Owner, now()).is_err(), "not twice");
        advance_stage(&mut conn, &id, StudioStage::Prd, &allowed, Actor::Owner, now()).unwrap();
        assert!(advance_stage(&mut conn, &id, StudioStage::Roadmap, &allowed, Actor::Owner, now()).is_err(), "not yet possible");
        assert_eq!(get_project(&conn, &id).unwrap().stage, StudioStage::Prd);
        let (_, stage) = live_project(&conn, &id).unwrap();
        assert_eq!(stage, StudioStage::Prd);
        let moved: i64 = conn.query_row("SELECT count(*) FROM activity WHERE verb = 'moved' AND entity_type = 'studio_project'", [], |r| r.get(0)).unwrap();
        assert_eq!(moved, 2);
        // A message written now carries the new stage.
        add_owner_message(&mut conn, &id, "Halo", now()).unwrap();
        assert_eq!(list_messages(&conn, &id).unwrap()[0].stage, StudioStage::Prd);
    }

    #[test]
    fn a_stage_is_reached_forward_only() {
        let mut conn = open_in_memory().unwrap();
        let id = project(&mut conn);
        assert!(reach_stage(&mut conn, &id, StudioStage::Prd, Actor::Owner, now()).unwrap().is_some());
        assert_eq!(get_project(&conn, &id).unwrap().stage, StudioStage::Prd);
        assert!(reach_stage(&mut conn, &id, StudioStage::Clarify, Actor::Owner, now()).unwrap().is_none(), "never backward");
        assert!(reach_stage(&mut conn, &id, StudioStage::Prd, Actor::Owner, now()).unwrap().is_none());
        assert_eq!(get_project(&conn, &id).unwrap().stage, StudioStage::Prd);
    }

    #[test]
    fn a_name_comes_from_the_idea() {
        assert_eq!(name_from_idea("\n  Absensi lab pakai kartu\nlainnya"), "Absensi lab pakai kartu");
        let long = "Aplikasi untuk membantu siswa TJKT mencatat praktikum jaringan dan membagikan hasilnya ke guru";
        let name = name_from_idea(long);
        assert!(name.ends_with('…') && name.chars().count() <= 61, "{name}");
        assert!(name.starts_with("Aplikasi untuk membantu siswa TJKT mencatat"));
        assert_eq!(name_from_idea("   "), "Ide baru");
    }

    #[test]
    fn the_summary_is_kept_with_the_message_it_covers() {
        let mut conn = open_in_memory().unwrap();
        let id = project(&mut conn);
        let other = project(&mut conn);
        let (first, _) = add_owner_message(&mut conn, &id, "Halo", now()).unwrap();
        let (foreign, _) = add_owner_message(&mut conn, &other, "Lain", now()).unwrap();
        assert_eq!(summary(&conn, &id).unwrap(), (String::new(), None));
        set_summary(&mut conn, &id, "Pemilik menyapa.", &first).unwrap();
        assert_eq!(summary(&conn, &id).unwrap(), ("Pemilik menyapa.".into(), Some(first)));
        assert!(set_summary(&mut conn, &id, "x", &foreign).is_err(), "only its own messages");
    }

    #[test]
    fn a_ledger_item_cannot_point_at_another_project_s_message() {
        let mut conn = open_in_memory().unwrap();
        let first = project(&mut conn);
        let second = project(&mut conn);
        let (message, _) = add_owner_message(&mut conn, &first, "Halo", now()).unwrap();
        let input = LedgerInput { kind: LedgerKind::Constraint, topic: "Waktu".into(), statement: "Dua minggu".into(), rationale: String::new(), area: Some(LedgerArea::Constraints), replaces_id: None };
        assert!(matches!(
            add_ledger_item(&mut conn, &second, input, StatedBy::Owner, Some(&message), now()),
            Err(Error::Invalid { field: "messageId", .. })
        ));
    }
}
