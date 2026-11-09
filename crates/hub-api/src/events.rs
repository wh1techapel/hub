//! What `/v1/events` streams: every change (whoever made it) and every fired reminder.

use hub_core::model::Entity;
use hub_core::reminders::Fired;
use hub_core::Change;
use serde::Serialize;
use tokio::sync::broadcast;

/// Events kept for a slow reader before it is told to read everything again (`resync`).
const BUFFER: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ApiEvent {
    /// Something was written. `ids` empty means "the whole kind".
    #[serde(rename_all = "camelCase")]
    Changed { entity: Entity, ids: Vec<String> },
    /// A reminder came due. `late`: it came due while Hub was not running or the computer slept.
    #[serde(rename_all = "camelCase")]
    Reminder { id: String, text: String, due_at: String, late: bool },
    /// The token was replaced: streams opened with the old one end (not sent to clients).
    #[serde(skip)]
    TokenRotated,
}

impl From<&Change> for ApiEvent {
    fn from(change: &Change) -> ApiEvent {
        ApiEvent::Changed { entity: change.entity, ids: change.ids.clone() }
    }
}

impl From<&Fired> for ApiEvent {
    fn from(fired: &Fired) -> ApiEvent {
        ApiEvent::Reminder { id: fired.id.clone(), text: fired.text.clone(), due_at: fired.due_at.clone(), late: fired.late }
    }
}

impl ApiEvent {
    /// The SSE event name.
    pub fn name(&self) -> &'static str {
        match self {
            ApiEvent::Changed { .. } => "changed",
            ApiEvent::Reminder { .. } => "reminder",
            ApiEvent::TokenRotated => "rotated",
        }
    }
}

/// One sender, cloned wherever something happens; each stream subscribes.
#[derive(Clone)]
pub struct Events(broadcast::Sender<ApiEvent>);

impl Default for Events {
    fn default() -> Events {
        Events(broadcast::channel(BUFFER).0)
    }
}

impl Events {
    /// Send to every open stream. Nobody listening is fine.
    pub fn publish(&self, event: ApiEvent) {
        let _ = self.0.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ApiEvent> {
        self.0.subscribe()
    }

    pub fn listeners(&self) -> usize {
        self.0.receiver_count()
    }
}
