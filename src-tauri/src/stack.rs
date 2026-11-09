//! Studio's stage 2 in the app (docs/studio.md, "Revision: three stages", R3): the tech stack the
//! owner picks or Gemini recommends (over a Tauri channel, stoppable with the conversation's
//! Hentikan), changed by the owner and approved. Every rule lives in hub-core `stack`; these only
//! call it and announce the change.

use std::sync::Arc;

use hub_core::model::Actor;
use hub_core::settings::{self, DevCost};
use hub_core::stack::{self, StackBody, StackMode, StudioStack};
use hub_core::studio::MessageState;
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tokio::sync::watch;

use crate::commands::{read, write, Hub, Result};
use crate::studio::{gemini_refused, project_context, store_failed};
use crate::{log, AppState};

#[tauri::command(async)]
pub fn studio_stack(state: Hub<'_>, project_id: String) -> Result<Option<StudioStack>> {
    read(&state, |conn, _| stack::get(conn, &project_id))
}

/// The owner's stack, or the owner's change to Gemini's: kept as a draft.
#[tauri::command(async)]
pub fn save_studio_stack(app: AppHandle, state: Hub<'_>, project_id: String, body: StackBody, mode: StackMode) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(stack::save(conn, &project_id, body, mode, Actor::Owner, ctx.now)?))))
}

#[tauri::command(async)]
pub fn approve_studio_stack(app: AppHandle, state: Hub<'_>, project_id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(stack::approve(conn, &project_id, ctx.now)?))))
}

/// What the page hears while Gemini recommends.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum StackEvent {
    /// The request is out (or the repair request).
    Step { repair: bool },
    /// Done: complete, stopped, or failed (with the error's kind).
    Finished { state: MessageState, error_kind: Option<String> },
}

fn profile_of(settings: &settings::Settings) -> hub_ai::stack::Profile {
    hub_ai::stack::Profile {
        knows: settings.dev_knows.clone(),
        learning: settings.dev_learning.clone(),
        cost: match settings.dev_cost {
            DevCost::Free => "free",
            DevCost::Low => "low",
            DevCost::Flexible => "flexible",
        }
        .into(),
        deploy: settings.dev_deploy.clone(),
    }
}

/// "Rekomendasikan dengan Gemini": the project, its ledger and conversation, and the owner's
/// developer profile go to Gemini (only now, because the owner asked); the answer is checked,
/// repaired once if needed, and kept as the draft. With `ask`, Gemini changes the current stack.
#[tauri::command]
pub async fn recommend_studio_stack(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    project_id: String,
    ask: String,
    on_event: Channel<StackEvent>,
) -> Result<()> {
    let state = state.inner().clone();
    let ask = ask.trim().to_string();
    if ask.chars().count() > 1000 {
        return Err(gemini_refused("length"));
    }
    let key = state.studio.keys.get().map_err(|error| store_failed(&state, error))?.ok_or_else(|| gemini_refused("no_key"))?;
    let (settings, current) = read(&state, |conn, _| Ok((settings::load(conn)?, stack::get(conn, &project_id)?)))?;
    let (context, _) = project_context(&state, &project_id)?;
    let current = if ask.is_empty() { None } else { current.map(|s| serde_json::to_value(&s.body).unwrap_or_default()) };
    let (stop, mut cancel) = watch::channel(false);
    if let Ok(mut replies) = state.studio.replies.lock() {
        replies.insert(project_id.clone(), stop);
    }
    let mut on_step = {
        let channel = on_event.clone();
        move |step: hub_ai::stack::Step| {
            let _ = channel.send(StackEvent::Step { repair: step.repair });
        }
    };
    let check = |value: &serde_json::Value| stack::read_recommendation(value).err().unwrap_or_default();
    let profile = profile_of(&settings);
    let outcome = hub_ai::stack::generate(
        &state.studio.gemini,
        &key,
        &settings.gemini_model,
        &context,
        &profile,
        current.as_ref(),
        &ask,
        &mut cancel,
        &mut on_step,
        &check,
    )
    .await;
    if let Ok(mut replies) = state.studio.replies.lock() {
        replies.remove(&project_id);
    }
    let finished = match outcome {
        Ok(answer) => match stack::read_recommendation(&answer) {
            Ok(body) => {
                write(&app, &state, |conn, ctx| Ok(((), Some(stack::save(conn, &project_id, body, StackMode::Gemini, Actor::Gemini, ctx.now)?))))?;
                StackEvent::Finished { state: MessageState::Complete, error_kind: None }
            }
            Err(problems) => {
                log::error(state.hub.paths(), &format!("studio stack: unreadable answer ({} problems)", problems.len()));
                StackEvent::Finished { state: MessageState::Failed, error_kind: Some("bad_response".into()) }
            }
        },
        Err(error) if error.kind == hub_ai::FailKind::Cancelled => StackEvent::Finished { state: MessageState::Stopped, error_kind: None },
        Err(error) => {
            log::error(state.hub.paths(), &format!("studio stack: {}", error.log_line()));
            StackEvent::Finished { state: MessageState::Failed, error_kind: Some(error.kind.as_str().into()) }
        }
    };
    let _ = on_event.send(finished);
    Ok(())
}
