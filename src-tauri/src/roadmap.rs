//! Studio's roadmap in the app (docs/studio.md, "Roadmap"): Gemini writes it from the approved
//! PRD (over a Tauri channel, stoppable with the conversation's Hentikan), the owner edits the
//! draft, picks the repository folder in Windows' folder dialog, and approves it. Every rule lives
//! in hub-core `roadmap`; these only call it and announce the change.

use std::sync::Arc;

use hub_core::model::Actor;
use hub_core::roadmap::{
    self, ApproveRoadmap, FolderCheck, PlanCheckpoint, ReportInput, RoadmapPlan, RoadmapView, StudioLink, StudioReport, StudioRequest,
    StudioTaskStatus,
};
use hub_core::studio::{ClaudeAccess, MessageState};
use hub_core::prd::Problem;
use hub_core::settings;
use serde::Serialize;
use serde_json::Value;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tokio::sync::watch;

use crate::commands::{read, write, write_many, Hub, Result};
use crate::studio::{gemini_refused, project_context, store_failed};
use crate::{log, AppState};

#[tauri::command(async)]
pub fn studio_roadmap(state: Hub<'_>, project_id: String) -> Result<RoadmapView> {
    read(&state, |conn, _| roadmap::view(conn, &project_id))
}

/// What the page hears while the roadmap is written.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RoadmapEvent {
    /// The request is out (or the repair request).
    Step { repair: bool },
    /// Done: complete (with the problems still in it), stopped, or failed.
    Finished { state: MessageState, error_kind: Option<String>, problems: usize },
}

/// The plan in Gemini's answer, normalized; `Err` with the reasons when it cannot be read.
pub(crate) fn plan_from(answer: &Value) -> std::result::Result<RoadmapPlan, Vec<Problem>> {
    roadmap::read_plan(answer).map(|plan| roadmap::normalize(&plan))
}

/// "Susun roadmap": Gemini writes the roadmap from the approved PRD; a roadmap that breaks the
/// rules is sent back once with the reasons. The result becomes the draft (replacing an earlier
/// draft), with any problem left shown for the owner to fix. The project moves to Roadmap.
#[tauri::command]
pub async fn generate_roadmap(app: AppHandle, state: State<'_, Arc<AppState>>, project_id: String, on_event: Channel<RoadmapEvent>) -> Result<()> {
    let state = state.inner().clone();
    let key = state.studio.keys.get().map_err(|error| store_failed(&state, error))?.ok_or_else(|| gemini_refused("no_key"))?;
    let (model, (prd_ref, prd), locked) = read(&state, |conn, _| {
        let view = roadmap::view(conn, &project_id)?;
        Ok((settings::load(conn)?.gemini_model, roadmap::source_prd(conn, &project_id)?, view.locked))
    })?;
    if locked {
        return Err(gemini_refused("locked"));
    }
    let (mut context, _) = project_context(&state, &project_id)?;
    // The roadmap is written from the PRD, not the conversation.
    context.history.clear();
    let (stop, mut cancel) = watch::channel(false);
    if let Ok(mut replies) = state.studio.replies.lock() {
        replies.insert(project_id.clone(), stop);
    }
    let mut on_step = {
        let channel = on_event.clone();
        move |step: hub_ai::roadmap::Step| {
            let _ = channel.send(RoadmapEvent::Step { repair: step.repair });
        }
    };
    let check = |value: &Value| roadmap::check_value(value, &prd);
    let prd_json = serde_json::to_value(&prd).unwrap_or_default();
    let outcome = hub_ai::roadmap::generate(&state.studio.gemini, &key, &model, &context, &prd_json, prd_ref.number, &mut cancel, &mut on_step, &check).await;
    if let Ok(mut replies) = state.studio.replies.lock() {
        replies.remove(&project_id);
    }
    let finished = match outcome {
        Ok(answer) => match plan_from(&answer) {
            Ok(plan) => {
                let problems = write(&app, &state, |conn, ctx| {
                    let (problems, change) = roadmap::save_generated(conn, &project_id, &prd_ref.id, &plan, ctx.now)?;
                    Ok((problems.len(), Some(change)))
                })?;
                RoadmapEvent::Finished { state: MessageState::Complete, error_kind: None, problems }
            }
            Err(problems) => {
                log::error(state.hub.paths(), &format!("studio roadmap: unreadable answer ({} problems)", problems.len()));
                RoadmapEvent::Finished { state: MessageState::Failed, error_kind: Some("bad_response".into()), problems: problems.len() }
            }
        },
        Err(error) if error.kind == hub_ai::FailKind::Cancelled => RoadmapEvent::Finished { state: MessageState::Stopped, error_kind: None, problems: 0 },
        Err(error) => {
            log::error(state.hub.paths(), &format!("studio roadmap: {}", error.log_line()));
            RoadmapEvent::Finished { state: MessageState::Failed, error_kind: Some(error.kind.as_str().into()), problems: 0 }
        }
    };
    let _ = on_event.send(finished);
    Ok(())
}

#[tauri::command(async)]
pub fn update_roadmap_checkpoint(app: AppHandle, state: Hub<'_>, id: String, value: PlanCheckpoint) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(roadmap::update_checkpoint(conn, &id, &value, ctx.now)?))))
}

#[tauri::command(async)]
pub fn add_roadmap_checkpoint(app: AppHandle, state: Hub<'_>, project_id: String, after: Option<String>) -> Result<String> {
    write(&app, &state, |conn, ctx| {
        let (id, change) = roadmap::add_checkpoint(conn, &project_id, after.as_deref(), ctx.now)?;
        Ok((id, Some(change)))
    })
}

#[tauri::command(async)]
pub fn remove_roadmap_checkpoint(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(roadmap::remove_checkpoint(conn, &id, ctx.now)?))))
}

#[tauri::command(async)]
pub fn move_roadmap_checkpoint(app: AppHandle, state: Hub<'_>, id: String, to: u32) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(roadmap::move_checkpoint(conn, &id, to, ctx.now)?))))
}

/// "Ubah roadmap": a draft copied from the approved roadmap, while it may still be replaced.
#[tauri::command(async)]
pub fn edit_roadmap(app: AppHandle, state: Hub<'_>, project_id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(roadmap::draft_from_approved(conn, &project_id, ctx.now)?))))
}

#[tauri::command(async)]
pub fn discard_roadmap_draft(app: AppHandle, state: Hub<'_>, project_id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(roadmap::discard_draft(conn, &project_id, ctx.now)?))))
}

/// A checkpoint added at the end of the approved roadmap. Its problems, if any, come back and
/// nothing is saved.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Appended {
    pub id: Option<String>,
    pub problems: Vec<Problem>,
}

#[tauri::command(async)]
pub fn append_roadmap_checkpoint(app: AppHandle, state: Hub<'_>, project_id: String, value: PlanCheckpoint) -> Result<Appended> {
    write(&app, &state, |conn, ctx| {
        let (id, problems, change) = roadmap::append_checkpoint(conn, &project_id, &value, ctx.now)?;
        let saved = problems.is_empty();
        Ok((Appended { id: saved.then_some(id), problems }, saved.then_some(change)))
    })
}

/// Windows' folder dialog for the repository folder, then what Hub can tell about it. None when
/// the owner cancelled.
#[tauri::command(async)]
pub fn pick_repo_folder(app: AppHandle) -> Result<Option<FolderCheck>> {
    let owner = crate::library::main_hwnd(&app);
    let Some(path) = crate::shell::on_com_thread(move || crate::shell::pick_folder(owner, "Pilih folder repo proyek")).flatten() else {
        return Ok(None);
    };
    Ok(Some(roadmap::inspect_folder(&path.display().to_string())?))
}

/// A folder typed or pasted by the owner: whether it exists and holds `.git`.
#[tauri::command(async)]
pub fn check_repo_folder(path: String) -> Result<FolderCheck> {
    Ok(roadmap::inspect_folder(&path)?)
}

#[tauri::command(async)]
pub fn approve_roadmap(app: AppHandle, state: Hub<'_>, project_id: String, input: ApproveRoadmap) -> Result<()> {
    write_many(&app, &state, |conn, ctx| Ok(((), roadmap::approve(conn, &project_id, &input, ctx.now)?)))
}

#[tauri::command(async)]
pub fn set_repo_path(app: AppHandle, state: Hub<'_>, project_id: String, path: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(roadmap::set_repo_path(conn, &project_id, &path, ctx.now)?))))
}

#[tauri::command(async)]
pub fn set_claude_access(app: AppHandle, state: Hub<'_>, project_id: String, access: ClaudeAccess) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(roadmap::set_claude_access(conn, &project_id, access, ctx.now)?))))
}

// ---------- checkpoints at work (the owner's side; Claude's comes over MCP in S5) ----------

#[tauri::command(async)]
pub fn studio_requests(state: Hub<'_>, project_id: String, open_only: bool) -> Result<Vec<StudioRequest>> {
    read(&state, |conn, _| roadmap::requests(conn, &project_id, open_only))
}

#[tauri::command(async)]
pub fn checkpoint_reports(state: Hub<'_>, id: String) -> Result<Vec<StudioReport>> {
    read(&state, |conn, _| roadmap::reports(conn, &id))
}

/// "Mulai CP-03": Planned to Ready.
#[tauri::command(async)]
pub fn make_checkpoint_ready(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(roadmap::make_ready(conn, &id, Actor::Owner, ctx.now)?))))
}

/// The owner works on it alone: Ready to In Progress.
#[tauri::command(async)]
pub fn start_checkpoint(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(roadmap::start_checkpoint(conn, &id, Actor::Owner, ctx.now)?))))
}

#[tauri::command(async)]
pub fn block_checkpoint(app: AppHandle, state: Hub<'_>, id: String, reason: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(roadmap::block_checkpoint(conn, &id, &reason, Actor::Owner, ctx.now)?))))
}

#[tauri::command(async)]
pub fn resume_checkpoint(app: AppHandle, state: Hub<'_>, id: String, answer: Option<String>) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(roadmap::resume_checkpoint(conn, &id, answer.as_deref(), Actor::Owner, ctx.now)?))))
}

#[tauri::command(async)]
pub fn answer_studio_request(app: AppHandle, state: Hub<'_>, id: String, answer: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(roadmap::answer_request(conn, &id, &answer, ctx.now)?))))
}

/// The owner's own report, for a checkpoint they built: In Progress to In Review.
#[tauri::command(async)]
pub fn submit_checkpoint_report(app: AppHandle, state: Hub<'_>, id: String, report: ReportInput) -> Result<()> {
    write(&app, &state, |conn, ctx| {
        let (_, change) = roadmap::submit_report(conn, &id, &report, Actor::Owner, ctx.now)?;
        Ok(((), Some(change)))
    })
}

/// What a report breaks, before it is sent (the form shows it in place).
#[tauri::command(async)]
pub fn check_checkpoint_report(state: Hub<'_>, id: String, report: ReportInput) -> Result<Vec<Problem>> {
    read(&state, |conn, _| Ok(roadmap::check_report(&report, &roadmap::checkpoint(conn, &id)?.required_tests)))
}

#[tauri::command(async)]
pub fn approve_checkpoint(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(roadmap::approve_checkpoint(conn, &id, Actor::Owner, ctx.now)?))))
}

#[tauri::command(async)]
pub fn request_checkpoint_changes(app: AppHandle, state: Hub<'_>, id: String, note: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(roadmap::request_changes(conn, &id, &note, Actor::Owner, ctx.now)?))))
}

#[tauri::command(async)]
pub fn set_studio_task_status(app: AppHandle, state: Hub<'_>, id: String, status: StudioTaskStatus, note: Option<String>) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(roadmap::set_task_status(conn, &id, status, note.as_deref(), Actor::Owner, ctx.now)?))))
}

/// "Mandiri" (R4): the whole of it in one text, for working without the MCP bridge. No AI.
#[tauri::command(async)]
pub fn standalone_prompt(state: Hub<'_>, id: String) -> Result<String> {
    read(&state, |conn, _| roadmap::standalone_prompt(conn, &id))
}

/// "Buat prompt implementasi" ("Dengan Hub"): filled by Hub from the checkpoint, no AI.
#[tauri::command(async)]
pub fn implementation_prompt(state: Hub<'_>, id: String) -> Result<String> {
    read(&state, |conn, _| roadmap::implementation_prompt(conn, &id))
}

/// The Studio project a Hub Proyek is linked to, for the line on the Proyek page.
#[tauri::command(async)]
pub fn studio_link(state: Hub<'_>, project_id: String) -> Result<Option<StudioLink>> {
    read(&state, |conn, _| roadmap::link_of(conn, &project_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A roadmap shaped exactly as `hub_ai::roadmap::schema()` asks for (every required field, the
    /// enums' words), the way Gemini's structured output returns it.
    fn recorded_answer() -> Value {
        serde_json::json!({
            "checkpoints": [
                {
                    "code": "CP-01", "title": "Fondasi data dan kartu", "goal": "Kartu terdaftar tersimpan dan bisa dibaca.",
                    "description": "Skema data siswa dan pembacaan kartu.", "features": ["FEAT-001"],
                    "tasks": [
                        { "title": "Tentukan pembaca kartu", "detail": "", "feature": "", "subtasks": [] },
                        { "title": "Buat tabel siswa", "detail": "Nama, kelas, UID kartu.", "feature": "FEAT-001", "subtasks": ["Migrasi", "Seed contoh"] }
                    ],
                    "dependsOn": [], "difficulty": "medium", "risks": ["Pembaca tidak cocok"],
                    "acceptance": ["Kartu terdaftar tercatat dalam 2 detik"],
                    "requiredTests": [{ "name": "Unit test kartu", "kind": "unit", "how": "npm test" }],
                    "deliverables": ["Tabel siswa"], "doneWhen": ["Demo ke guru"]
                },
                {
                    "code": "CP-02", "title": "Rekap guru", "goal": "Guru melihat daftar hadir.", "description": "",
                    "features": ["FEAT-002"],
                    "tasks": [{ "title": "Buat halaman rekap", "detail": "", "feature": "FEAT-002", "subtasks": [] }],
                    "dependsOn": ["CP-01"], "difficulty": "low", "risks": [],
                    "acceptance": ["Rekap sesuai kartu yang ditempel"],
                    "requiredTests": [{ "name": "E2E rekap", "kind": "e2e", "how": "npx playwright test" }],
                    "deliverables": [], "doneWhen": []
                }
            ]
        })
    }

    /// The PRD the answer was written from: two features, Must and Should.
    fn prd() -> hub_core::prd::Prd {
        let feature = |id: &str, priority: &str| {
            serde_json::json!({
                "id": id, "name": id, "description": "d", "rationale": "", "priority": priority, "dependsOn": [],
                "story": { "asA": "siswa", "want": "absen", "soThat": "cepat" },
                "acceptance": [{ "id": format!("AC-{}-1", &id[5..]), "given": "g", "when": "w", "then": "t" }], "tests": ["t"]
            })
        };
        serde_json::from_value(serde_json::json!({
            "identity": { "name": "Absensi", "summary": "s", "owner": "" },
            "background": { "problem": "p", "targetUsers": ["Siswa"] },
            "goals": { "primary": "g", "metrics": [{ "metric": "m", "target": "t" }] },
            "experience": { "mainFlows": [{ "name": "Absen", "steps": ["Tempel"] }] },
            "features": [feature("FEAT-001", "must"), feature("FEAT-002", "should")],
            "delivery": { "mvp": ["FEAT-001", "FEAT-002"] }
        }))
        .unwrap()
    }

    #[test]
    fn an_answer_in_the_schema_is_read_by_the_core_without_problems() {
        let plan = plan_from(&recorded_answer()).unwrap();
        assert_eq!(plan.checkpoints.len(), 2);
        assert_eq!(plan.checkpoints[0].tasks[1].code, "CP-01.T02");
        assert_eq!(roadmap::check(&plan, &prd()), vec![]);
        assert_eq!(roadmap::check_value(&recorded_answer(), &prd()), vec![]);
    }

    #[test]
    fn the_schema_asks_for_every_field_the_core_reads_and_no_other() {
        let schema = hub_ai::roadmap::schema();
        let item = &schema["properties"]["checkpoints"]["items"];
        let mut asked: Vec<String> = item["properties"].as_object().unwrap().keys().cloned().collect();
        let mut read: Vec<String> = serde_json::to_value(PlanCheckpoint::default()).unwrap().as_object().unwrap().keys().cloned().collect();
        asked.sort();
        read.sort();
        assert_eq!(asked, read);
        let mut task_asked: Vec<String> = item["properties"]["tasks"]["items"]["properties"].as_object().unwrap().keys().cloned().collect();
        task_asked.sort();
        // `code` is Hub's, never Gemini's.
        assert_eq!(task_asked, ["detail", "feature", "subtasks", "title"]);
        for kind in item["properties"]["requiredTests"]["items"]["properties"]["kind"]["enum"].as_array().unwrap() {
            assert!(serde_json::from_value::<roadmap::TestKind>(kind.clone()).is_ok(), "{kind}");
        }
        for level in item["properties"]["difficulty"]["enum"].as_array().unwrap() {
            assert!(serde_json::from_value::<roadmap::CheckpointDifficulty>(level.clone()).is_ok(), "{level}");
        }
    }

    #[test]
    fn a_broken_answer_is_named_not_saved() {
        let mut answer = recorded_answer();
        answer["checkpoints"][1]["difficulty"] = serde_json::json!("extreme");
        let problems = plan_from(&answer).unwrap_err();
        assert_eq!(problems[0].at, "CP-02");
    }
}
