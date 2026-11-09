//! Studio's stage 2, the tech stack (docs/studio.md, "Revision: three stages", R3).
//!
//! One stack per project, in seven layers, always all seven and in this order: platform, frontend,
//! backend, database, auth, hosting, extras. A layer the project does not need says so ("Tidak
//! perlu") rather than being left out. The owner picks the stack, or Gemini recommends it and the
//! owner changes what they like; each layer remembers who chose it. A draft may have empty choices;
//! approval needs every layer chosen. Any later change makes the stack a draft again.
//!
//! The PRD follows the approved stack: a project that has not reached the PRD stage cannot have its
//! PRD written until its stack is approved ([`require_for_prd`]). Projects that reached it before
//! R3 have no stack and are not held back.

use jiff::Timestamp;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::activity::{self, Record};
use crate::error::{invalid, Error, Reason, Result};
use crate::ids;
use crate::model::{text_enum, Actor, Change, Entity, Verb};
use crate::studio::{StatedBy, StudioStage};
use crate::time::instant;

text_enum!(
    StackLayerKind {
        Platform => "platform",
        Frontend => "frontend",
        Backend => "backend",
        Database => "database",
        Auth => "auth",
        Hosting => "hosting",
        Extras => "extras",
    }
);

text_enum!(
    StackStatus {
        Draft => "draft",
        Approved => "approved",
    }
);

text_enum!(
    /// How the stack was made last: picked by the owner, or recommended by Gemini.
    StackMode {
        Own => "own",
        Gemini => "gemini",
    }
);

const CHOICE_MAX: usize = 120;
const NOTE_MAX: usize = 600;
const ALTERNATIVES_MAX: usize = 3;
const ALTERNATIVE_MAX: usize = 80;
const SUMMARY_MAX: usize = 1000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct StackAlternative {
    pub name: String,
    /// When it would be the better choice.
    #[serde(default)]
    pub when: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct StackLayer {
    pub layer: StackLayerKind,
    /// The technology, or "Tidak perlu". Empty only in a draft.
    pub choice: String,
    /// Why this choice.
    #[serde(default)]
    pub why: String,
    /// How it fits the product's features.
    #[serde(default)]
    pub fit: String,
    /// How easy it is to build with and keep up.
    #[serde(default)]
    pub ease: String,
    /// What it costs, in money and in complexity.
    #[serde(default)]
    pub cost: String,
    #[serde(default)]
    pub alternatives: Vec<StackAlternative>,
    /// Who chose it: the owner, or Gemini (and the owner kept it).
    pub by: StatedBy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct StackBody {
    pub layers: Vec<StackLayer>,
    /// Gemini's word on the whole, or the owner's note.
    #[serde(default)]
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StudioStack {
    pub mode: StackMode,
    pub status: StackStatus,
    pub body: StackBody,
    pub approved_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn clean(value: &str) -> String {
    value.trim().to_string()
}

fn fits(field: &'static str, value: &str, max: usize) -> Result<()> {
    if value.chars().count() > max {
        return invalid(field, Reason::Length);
    }
    Ok(())
}

/// A body as Hub keeps it: every text trimmed and within its limit, the seven layers each once, in
/// order. Choices may be empty (a draft).
pub fn checked(body: StackBody) -> Result<StackBody> {
    fits("summary", body.summary.trim(), SUMMARY_MAX)?;
    let mut layers = Vec::with_capacity(StackLayerKind::ALL.len());
    for kind in StackLayerKind::ALL {
        let mut found = body.layers.iter().filter(|l| l.layer == *kind);
        let Some(layer) = found.next() else { return invalid("layers", Reason::Unknown) };
        if found.next().is_some() {
            return invalid("layers", Reason::TooMany);
        }
        let layer = StackLayer {
            layer: *kind,
            choice: clean(&layer.choice),
            why: clean(&layer.why),
            fit: clean(&layer.fit),
            ease: clean(&layer.ease),
            cost: clean(&layer.cost),
            alternatives: layer
                .alternatives
                .iter()
                .map(|a| StackAlternative { name: clean(&a.name), when: clean(&a.when) })
                .filter(|a| !a.name.is_empty())
                .collect(),
            by: layer.by,
        };
        fits("choice", &layer.choice, CHOICE_MAX)?;
        for note in [&layer.why, &layer.fit, &layer.ease, &layer.cost] {
            fits("note", note, NOTE_MAX)?;
        }
        if layer.alternatives.len() > ALTERNATIVES_MAX {
            return invalid("alternatives", Reason::TooMany);
        }
        for alternative in &layer.alternatives {
            fits("alternative", &alternative.name, ALTERNATIVE_MAX)?;
            fits("note", &alternative.when, NOTE_MAX)?;
        }
        layers.push(layer);
    }
    if body.layers.len() != layers.len() {
        return invalid("layers", Reason::Unknown);
    }
    Ok(StackBody { layers, summary: clean(&body.summary) })
}

fn clip(value: &Value, max: usize) -> String {
    value.as_str().unwrap_or("").trim().chars().take(max).collect()
}

/// Gemini's recommendation read into a body: what it must get right (every layer, once, with a
/// choice) is a problem it is told about once; what is merely long is clipped, and extra
/// alternatives are dropped. `Err` lists the problems, in Indonesian, for the repair request.
pub fn read_recommendation(answer: &Value) -> std::result::Result<StackBody, Vec<String>> {
    let mut problems = Vec::new();
    let given = answer.get("layers").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut layers = Vec::new();
    for kind in StackLayerKind::ALL {
        let found: Vec<&Value> = given.iter().filter(|l| l.get("layer").and_then(Value::as_str) == Some(kind.as_str())).collect();
        let Some(layer) = found.first() else {
            problems.push(format!("lapisan {} tidak ada", kind.as_str()));
            continue;
        };
        if found.len() > 1 {
            problems.push(format!("lapisan {} muncul lebih dari sekali", kind.as_str()));
        }
        let choice = clip(&layer["choice"], CHOICE_MAX);
        if choice.is_empty() {
            problems.push(format!("lapisan {} tanpa pilihan (tulis \"Tidak perlu\" bila tidak dibutuhkan)", kind.as_str()));
        }
        let alternatives = layer["alternatives"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|a| StackAlternative { name: clip(&a["name"], ALTERNATIVE_MAX), when: clip(&a["when"], NOTE_MAX) })
            .filter(|a| !a.name.is_empty())
            .take(ALTERNATIVES_MAX)
            .collect();
        layers.push(StackLayer {
            layer: *kind,
            choice,
            why: clip(&layer["why"], NOTE_MAX),
            fit: clip(&layer["fit"], NOTE_MAX),
            ease: clip(&layer["ease"], NOTE_MAX),
            cost: clip(&layer["cost"], NOTE_MAX),
            alternatives,
            by: StatedBy::Gemini,
        });
    }
    if given.len() > StackLayerKind::ALL.len() {
        problems.push("ada lapisan yang tidak dikenal".into());
    }
    if problems.is_empty() { Ok(StackBody { layers, summary: clip(&answer["summary"], SUMMARY_MAX) }) } else { Err(problems) }
}

/// What still keeps a stack from approval: the layers with no choice.
pub fn unchosen(body: &StackBody) -> Vec<StackLayerKind> {
    body.layers.iter().filter(|l| l.choice.is_empty()).map(|l| l.layer).collect()
}

fn live_stage(conn: &Connection, project_id: &str) -> Result<(String, StudioStage)> {
    conn.query_row(
        "SELECT name, stage FROM studio_projects WHERE id = ?1 AND deleted_at IS NULL",
        [project_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()?
    .ok_or(Error::NotFound("studioProject"))
}

pub fn get(conn: &Connection, project_id: &str) -> Result<Option<StudioStack>> {
    let project_id = ids::parse("projectId", project_id)?;
    live_stage(conn, &project_id)?;
    conn.query_row(
        "SELECT mode, status, body, approved_at, created_at, updated_at FROM studio_stacks WHERE studio_project_id = ?1",
        [&project_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, String>(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
    )
    .optional()?
    .map(|(mode, status, body, approved_at, created_at, updated_at)| {
        let body: StackBody = serde_json::from_str(&body).map_err(|_| Error::Invalid { field: "body", reason: Reason::Format })?;
        Ok(StudioStack { mode, status, body, approved_at, created_at, updated_at })
    })
    .transpose()
}

/// Keep a stack as the project's draft (a new one, or a change to it): checked, and never approved
/// by a save.
pub fn save(conn: &mut Connection, project_id: &str, body: StackBody, mode: StackMode, actor: Actor, now: Timestamp) -> Result<Change> {
    let project_id = ids::parse("projectId", project_id)?;
    let body = checked(body)?;
    let tx = conn.transaction()?;
    let (name, _) = live_stage(&tx, &project_id)?;
    let stamp = instant(now);
    let json = serde_json::to_string(&body).map_err(|_| Error::Invalid { field: "body", reason: Reason::Format })?;
    tx.execute(
        "INSERT INTO studio_stacks (studio_project_id, mode, body, status, approved_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'draft', NULL, ?4, ?4)
         ON CONFLICT (studio_project_id) DO UPDATE SET mode = excluded.mode, body = excluded.body, status = 'draft',
           approved_at = NULL, updated_at = excluded.updated_at",
        params![project_id, mode, json, stamp],
    )?;
    tx.execute("UPDATE studio_projects SET updated_at = ?2 WHERE id = ?1", params![project_id, stamp])?;
    activity::record(
        &tx,
        now,
        Record { actor, verb: Verb::Updated, entity: Entity::StudioStack, entity_id: Some(&project_id), summary: &name, details: serde_json::json!({ "mode": mode }) },
    )?;
    tx.commit()?;
    Ok(Change::one(Entity::StudioStack, project_id))
}

/// The owner approves the stack: only one with every layer chosen.
pub fn approve(conn: &mut Connection, project_id: &str, now: Timestamp) -> Result<Change> {
    let project_id = ids::parse("projectId", project_id)?;
    let stack = get(conn, &project_id)?.ok_or(Error::NotFound("stack"))?;
    if !unchosen(&stack.body).is_empty() {
        return invalid("choice", Reason::Length);
    }
    let tx = conn.transaction()?;
    let (name, _) = live_stage(&tx, &project_id)?;
    let stamp = instant(now);
    tx.execute(
        "UPDATE studio_stacks SET status = 'approved', approved_at = ?2, updated_at = ?2 WHERE studio_project_id = ?1",
        params![project_id, stamp],
    )?;
    tx.execute("UPDATE studio_projects SET updated_at = ?2 WHERE id = ?1", params![project_id, stamp])?;
    activity::record(
        &tx,
        now,
        Record { actor: Actor::Owner, verb: Verb::Approved, entity: Entity::StudioStack, entity_id: Some(&project_id), summary: &name, details: serde_json::json!({}) },
    )?;
    tx.commit()?;
    Ok(Change::one(Entity::StudioStack, project_id))
}

/// The PRD follows the approved stack: a project before the PRD stage needs one (R3). A project
/// already at the PRD or later (made before R3, or rewriting its PRD) is not held back.
pub fn require_for_prd(conn: &Connection, project_id: &str) -> Result<()> {
    let project_id = ids::parse("projectId", project_id)?;
    let (_, stage) = live_stage(conn, &project_id)?;
    if !matches!(stage, StudioStage::Explore | StudioStage::Clarify) {
        return Ok(());
    }
    match get(conn, &project_id)? {
        Some(stack) if stack.status == StackStatus::Approved => Ok(()),
        _ => invalid("stack", Reason::Order),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::db::open_in_memory;
    use crate::studio::{self, StudioProjectInput};

    fn now() -> Timestamp {
        "2026-11-08T03:00:00Z".parse().unwrap()
    }

    fn project(conn: &mut Connection) -> String {
        studio::create_project(conn, StudioProjectInput { name: "Absensi".into(), idea: "kartu".into() }, Actor::Owner, now()).unwrap().0
    }

    fn layer(kind: StackLayerKind, choice: &str) -> StackLayer {
        StackLayer {
            layer: kind,
            choice: choice.into(),
            why: String::new(),
            fit: String::new(),
            ease: String::new(),
            cost: String::new(),
            alternatives: vec![],
            by: StatedBy::Owner,
        }
    }

    fn body(choice: &str) -> StackBody {
        StackBody { layers: StackLayerKind::ALL.iter().rev().map(|k| layer(*k, choice)).collect(), summary: String::new() }
    }

    #[test]
    fn a_stack_has_the_seven_layers_once_in_order() {
        let ordered = checked(body(" Web ")).unwrap();
        assert_eq!(ordered.layers.iter().map(|l| l.layer).collect::<Vec<_>>(), StackLayerKind::ALL);
        assert_eq!(ordered.layers[0].choice, "Web");
        let mut missing = body("Web");
        missing.layers.pop();
        assert!(checked(missing).is_err());
        let mut twice = body("Web");
        twice.layers.push(layer(StackLayerKind::Auth, "Lagi"));
        assert!(checked(twice).is_err());
        let mut long = body("Web");
        long.layers[0].alternatives = (0..4).map(|i| StackAlternative { name: format!("A{i}"), when: String::new() }).collect();
        assert!(checked(long).is_err(), "at most three alternatives");
    }

    #[test]
    fn a_save_is_a_draft_and_approval_needs_every_layer_chosen() {
        let mut conn = open_in_memory().unwrap();
        let id = project(&mut conn);
        assert_eq!(get(&conn, &id).unwrap(), None);
        assert!(require_for_prd(&conn, &id).is_err(), "no stack, no PRD");

        save(&mut conn, &id, body(""), StackMode::Own, Actor::Owner, now()).unwrap();
        assert!(approve(&mut conn, &id, now()).is_err(), "nothing chosen yet");
        let mut picked = body("React");
        picked.layers[4].choice = "Tidak perlu".into();
        save(&mut conn, &id, picked, StackMode::Own, Actor::Owner, now()).unwrap();
        assert!(require_for_prd(&conn, &id).is_err(), "a draft is not enough");
        approve(&mut conn, &id, now()).unwrap();
        let stack = get(&conn, &id).unwrap().unwrap();
        assert_eq!((stack.status, stack.mode), (StackStatus::Approved, StackMode::Own));
        assert!(stack.approved_at.is_some());
        require_for_prd(&conn, &id).unwrap();
        assert_eq!(studio::get_project(&conn, &id).unwrap().stack_status, Some(StackStatus::Approved));

        // A change after approval makes it a draft again.
        save(&mut conn, &id, body("Vue"), StackMode::Own, Actor::Owner, now()).unwrap();
        let again = get(&conn, &id).unwrap().unwrap();
        assert_eq!((again.status, again.approved_at), (StackStatus::Draft, None));
        let rows: i64 = conn.query_row("SELECT count(*) FROM activity WHERE entity_type = 'studio_stack'", [], |r| r.get(0)).unwrap();
        assert_eq!(rows, 4);
    }

    #[test]
    fn a_project_already_at_the_prd_is_not_held_back() {
        let mut conn = open_in_memory().unwrap();
        let id = project(&mut conn);
        studio::reach_stage(&mut conn, &id, StudioStage::Prd, Actor::Owner, now()).unwrap();
        require_for_prd(&conn, &id).unwrap();
    }

    #[test]
    fn gemini_s_recommendation_is_read_and_its_mistakes_named() {
        let full: Vec<Value> = StackLayerKind::ALL
            .iter()
            .map(|k| json!({"layer": k.as_str(), "choice": "X", "why": "karena", "alternatives": [{"name": "A"}, {"name": "B"}, {"name": "C"}, {"name": "D"}, {"name": " "}]}))
            .collect();
        let read = read_recommendation(&json!({ "layers": full, "summary": "Ringkas" })).unwrap();
        assert_eq!(read.layers.len(), 7);
        assert!(read.layers.iter().all(|l| l.by == StatedBy::Gemini && l.alternatives.len() == 3));
        assert_eq!(read.summary, "Ringkas");
        checked(read).unwrap();

        let mut broken = full.clone();
        broken.remove(2);
        broken[0]["choice"] = json!("");
        broken.push(json!({"layer": "auth", "choice": "Lagi"}));
        let problems = read_recommendation(&json!({ "layers": broken })).unwrap_err();
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("backend tidak ada")));
        assert!(problems.iter().any(|p| p.contains("platform tanpa pilihan")));
        assert!(problems.iter().any(|p| p.contains("auth muncul lebih dari sekali")));
    }
}
