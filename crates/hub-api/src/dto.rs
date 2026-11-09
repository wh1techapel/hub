//! Request bodies and queries. Unknown fields are refused, not ignored, and a body must be JSON.
//! Values are checked by the core (lengths, dates, ids); these only give them a shape.

use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::request::Parts;
use hub_core::files::LinkTarget;
use hub_core::model::{JournalKind, LinkKind};
use hub_core::tasks::TaskView;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};

use crate::error::ApiError;

/// `Json<T>` with refusals in the API's own error shape.
pub struct Body<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequest<S> for Body<T> {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(request, state).await {
            Ok(axum::Json(value)) => Ok(Body(value)),
            Err(rejection) => Err(body_error(&rejection)),
        }
    }
}

fn body_error(rejection: &JsonRejection) -> ApiError {
    let reason = match rejection {
        JsonRejection::MissingJsonContentType(_) => "content_type",
        JsonRejection::BytesRejection(_) => "size",
        JsonRejection::JsonSyntaxError(_) => "syntax",
        _ => "shape",
    };
    ApiError { status: rejection.status(), kind: "invalid", field: Some("body".into()), reason: Some(reason.into()), message: rejection.body_text() }
}

/// `Query<T>` with refusals in the API's own error shape.
pub struct Params<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequestParts<S> for Params<T> {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Query::<T>::from_request_parts(parts, state).await {
            Ok(axum::extract::Query(value)) => Ok(Params(value)),
            Err(rejection) => Err(query_error(&rejection)),
        }
    }
}

fn query_error(rejection: &QueryRejection) -> ApiError {
    ApiError { status: rejection.status(), kind: "invalid", field: Some("query".into()), reason: Some("shape".into()), message: rejection.body_text() }
}

/// For fields that may be left out (keep), sent as null (clear), or given (set).
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(deserializer: D) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewTask {
    pub title: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub priority: u8,
    pub do_date: Option<String>,
    pub due_date: Option<String>,
    pub due_time: Option<String>,
    pub estimate_min: Option<u32>,
    pub project_id: Option<String>,
    pub area_id: Option<String>,
    /// For an import: the row's reference where it came from ("ultron:task:12").
    pub import_ref: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub notes: Option<String>,
    pub priority: Option<u8>,
    #[serde(default, deserialize_with = "present")]
    pub do_date: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    pub due_date: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    pub due_time: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    pub estimate_min: Option<Option<u32>>,
    #[serde(default, deserialize_with = "present")]
    pub project_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    pub area_id: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskList {
    pub view: Option<TaskView>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewReminder {
    pub text: String,
    /// An instant with its offset (RFC 3339), e.g. `2026-10-24T20:00:00+07:00`.
    pub due_at: String,
    pub task_id: Option<String>,
    pub import_ref: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReminderPatch {
    pub text: Option<String>,
    pub due_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snooze {
    pub minutes: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Range {
    /// `YYYY-MM-DD`, today when left out.
    pub from: Option<String>,
    /// Inclusive, `from` when left out.
    pub to: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Search {
    pub q: String,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileList {
    /// Files attached to this kind of thing...
    pub kind: Option<LinkKind>,
    /// ...with this id.
    pub id: Option<String>,
    /// Files of a project and its tasks.
    pub project_id: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewFile {
    /// An absolute local path (no network or device paths).
    pub path: String,
    pub attach: Option<LinkTarget>,
    pub note: Option<String>,
    pub import_ref: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewJournal {
    /// `YYYY-MM-DD`, today when left out.
    pub date: Option<String>,
    pub text: String,
    #[serde(default = "note")]
    pub kind: JournalKind,
    pub task_id: Option<String>,
    pub import_ref: Option<String>,
}

fn note() -> JournalKind {
    JournalKind::Note
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JournalPatch {
    #[serde(default, deserialize_with = "present")]
    pub task_id: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JournalList {
    /// Entries of one date.
    pub date: Option<String>,
    /// Entries whose words match (Hub's search).
    pub q: Option<String>,
    /// Notes that explain this task.
    pub task_id: Option<String>,
    pub kind: Option<JournalKind>,
    pub limit: Option<u32>,
}
