//! The REST client: `generativelanguage.googleapis.com/v1beta`, the key in `x-goog-api-key`.
//!
//! Models are read from `GET /models` and `GET /models/{model}`. Generation uses the Interactions
//! API (`POST /interactions`, recommended by Google for new work) with `store: false`, so nothing
//! of a Studio conversation is kept on Google's side: Hub is its record.

use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, RETRY_AFTER};
use serde::Serialize;
use serde_json::{json, Value};
use ts_rs::TS;

use crate::error::{FailKind, GeminiError};
use crate::key::ApiKey;

pub const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com";
const API: &str = "/v1beta";
/// Connecting takes seconds at most; a whole non-streaming answer may take longer.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(90);
/// A streamed reply, start to end.
const STREAM_TIMEOUT: Duration = Duration::from_secs(600);
/// How many pages of models are read at most (50 a page).
const MODEL_PAGES_MAX: usize = 10;
const MODEL_ID_MAX: usize = 80;

/// A model the key can see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelInfo {
    /// `gemini-3.8-flash` (Google's `models/` prefix removed).
    pub id: String,
    pub display_name: String,
    pub input_token_limit: Option<u32>,
    pub output_token_limit: Option<u32>,
    /// It can write text (`generateContent` among its methods).
    pub generates: bool,
}

/// What a short generation answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub text: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    /// Cut off at the output limit (status `incomplete`); the thinking counts toward that limit.
    pub incomplete: bool,
}

/// A model id as Google writes it: it goes into a URL path, so nothing else is allowed.
pub fn valid_model_id(model: &str) -> bool {
    !model.is_empty()
        && model.len() <= MODEL_ID_MAX
        && model.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
        && !model.starts_with('.')
}

/// A plain `http` address on this computer (`127.0.0.1` or `localhost`, no user part): the only
/// place a fake Gemini may stand in for Google.
pub fn is_loopback(url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else { return false };
    url.scheme() == "http"
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
}

pub struct Gemini {
    http: reqwest::Client,
    base: String,
}

impl Gemini {
    /// A client for `base` (Google's address, or a fake one in tests).
    pub fn new(base: &str, timeout: Duration) -> Result<Gemini, GeminiError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .user_agent(concat!("Hub-Studio/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(timeout)
            .https_only(base.starts_with("https://"))
            .build()
            .map_err(|_| GeminiError::new(FailKind::Offline, "the HTTP client could not start"))?;
        Ok(Gemini { http, base: base.trim_end_matches('/').to_string() })
    }

    /// The client for Google itself.
    pub fn google() -> Result<Gemini, GeminiError> {
        Gemini::new(DEFAULT_BASE_URL, DEFAULT_TIMEOUT)
    }

    async fn send(&self, request: reqwest::RequestBuilder, key: &ApiKey) -> Result<Value, GeminiError> {
        let header = HeaderValue::from_str(key.expose())
            .map_err(|_| GeminiError::new(FailKind::Key, "the key has characters a header cannot carry"))?;
        let answer = request
            .header("x-goog-api-key", header)
            .send()
            .await
            .map_err(|error| GeminiError::from_transport(&error))?;
        let status = answer.status().as_u16();
        let retry_after = answer
            .headers()
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u32>().ok());
        let body = answer.text().await.map_err(|error| GeminiError::from_transport(&error))?;
        if !(200..300).contains(&status) {
            return Err(GeminiError::from_answer(status, &body, retry_after));
        }
        serde_json::from_str(&body).map_err(|_| GeminiError::new(FailKind::BadResponse, "the answer is not JSON"))
    }

    /// One model, which also proves the key is accepted.
    pub async fn model(&self, key: &ApiKey, model: &str) -> Result<ModelInfo, GeminiError> {
        if !valid_model_id(model) {
            return Err(GeminiError::new(FailKind::Model, "not a model id"));
        }
        let url = format!("{}{API}/models/{model}", self.base);
        let value = self.send(self.http.get(url), key).await?;
        model_info(&value).ok_or_else(|| GeminiError::new(FailKind::BadResponse, "the model could not be read"))
    }

    /// The Gemini models the key can use to write text, in Google's order.
    pub async fn models(&self, key: &ApiKey) -> Result<Vec<ModelInfo>, GeminiError> {
        let mut found = Vec::new();
        let mut token: Option<String> = None;
        for _ in 0..MODEL_PAGES_MAX {
            let mut request = self.http.get(format!("{}{API}/models", self.base)).query(&[("pageSize", "50")]);
            if let Some(token) = &token {
                request = request.query(&[("pageToken", token.as_str())]);
            }
            let page = self.send(request, key).await?;
            for model in page.get("models").and_then(Value::as_array).into_iter().flatten() {
                if let Some(info) = model_info(model).filter(|info| info.generates && info.id.starts_with("gemini")) {
                    found.push(info);
                }
            }
            token = page.get("nextPageToken").and_then(Value::as_str).filter(|t| !t.is_empty()).map(str::to_string);
            if token.is_none() {
                break;
            }
        }
        Ok(found)
    }

    /// A `POST /interactions` with the key in its header. A streamed answer may take minutes, so
    /// it gets a longer limit than the client's default; Hentikan ends it sooner.
    pub(crate) fn post_interaction(&self, body: &Value, key: &ApiKey) -> Result<reqwest::RequestBuilder, GeminiError> {
        let header = HeaderValue::from_str(key.expose())
            .map_err(|_| GeminiError::new(FailKind::Key, "the key has characters a header cannot carry"))?;
        let mut request = self.http.post(format!("{}{API}/interactions", self.base)).header("x-goog-api-key", header).json(body);
        if body.get("stream").and_then(Value::as_bool) == Some(true) {
            request = request.timeout(STREAM_TIMEOUT);
        }
        Ok(request)
    }

    /// A whole (not streamed) interaction, for the summary.
    pub async fn generate(&self, key: &ApiKey, body: &Value) -> Result<Answer, GeminiError> {
        self.generate_with(key, body, DEFAULT_TIMEOUT).await
    }

    /// A whole interaction with its own time limit (a PRD part takes longer than a summary).
    pub async fn generate_with(&self, key: &ApiKey, body: &Value, timeout: Duration) -> Result<Answer, GeminiError> {
        let request = self.http.post(format!("{}{API}/interactions", self.base)).json(body).timeout(timeout);
        let value = self.send(request, key).await?;
        interaction_answer(&value)
    }

    /// A very short generation: proves the model writes for this key (free-tier verification and
    /// quota show up only here). Stored nowhere (`store: false`); costs a handful of tokens.
    pub async fn probe(&self, key: &ApiKey, model: &str) -> Result<Answer, GeminiError> {
        if !valid_model_id(model) {
            return Err(GeminiError::new(FailKind::Model, "not a model id"));
        }
        let body = json!({
            "model": model,
            "input": "Balas dengan satu kata saja: siap.",
            "store": false,
            // Room for a thinking model to think a little and still answer; an answer cut off by
            // the limit still proves the key and the model work.
            "generation_config": { "max_output_tokens": 512, "thinking_level": "low" },
        });
        let value = self.send(self.http.post(format!("{}{API}/interactions", self.base)).json(&body), key).await?;
        interaction_answer(&value)
    }
}

fn model_info(value: &Value) -> Option<ModelInfo> {
    let name = value.get("name")?.as_str()?;
    let id = name.strip_prefix("models/").unwrap_or(name).to_string();
    let methods = value.get("supportedGenerationMethods").and_then(Value::as_array);
    Some(ModelInfo {
        display_name: value.get("displayName").and_then(Value::as_str).unwrap_or(&id).to_string(),
        input_token_limit: value.get("inputTokenLimit").and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()),
        output_token_limit: value.get("outputTokenLimit").and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()),
        generates: methods.is_some_and(|m| m.iter().any(|v| v.as_str() == Some("generateContent"))),
        id,
    })
}

/// What an answer looked like, for the log and the owner: its status and its field names, never
/// its content.
fn shape(value: &Value) -> String {
    let status = value.get("status").and_then(Value::as_str).unwrap_or("-");
    let fields: Vec<&str> = value.as_object().map(|o| o.keys().map(String::as_str).take(12).collect()).unwrap_or_default();
    format!("status {status}; fields {}", if fields.is_empty() { "-".to_string() } else { fields.join(", ") })
}

/// The text of an interaction: the `text` parts of its `model_output` steps, in order.
pub(crate) fn interaction_answer(value: &Value) -> Result<Answer, GeminiError> {
    match value.get("status").and_then(Value::as_str) {
        Some("failed") => return Err(GeminiError::new(FailKind::Service, "the interaction failed")),
        Some("cancelled") => return Err(GeminiError::new(FailKind::Cancelled, "the interaction was cancelled")),
        _ => {}
    }
    // Google's JSON leaves out an empty list: an interaction whose output limit went to thinking
    // (status `incomplete`) comes back without `steps` at all. That is an answer with no text, not
    // a broken one. Anything else in its place is.
    let steps: &[Value] = match value.get("steps") {
        None | Some(Value::Null) => &[],
        Some(Value::Array(steps)) => steps,
        Some(_) => return Err(GeminiError::new(FailKind::BadResponse, format!("the answer's steps are not a list ({})", shape(value)))),
    };
    if value.get("id").is_none() && value.get("status").is_none() {
        return Err(GeminiError::new(FailKind::BadResponse, format!("not an interaction ({})", shape(value))));
    }
    let text: String = steps
        .iter()
        .filter(|step| step.get("type").and_then(Value::as_str) == Some("model_output"))
        .flat_map(|step| step.get("content").and_then(Value::as_array).into_iter().flatten())
        .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect();
    let usage = value.get("usage");
    Ok(Answer {
        text,
        input_tokens: usage.and_then(|u| u.get("total_input_tokens")).and_then(Value::as_u64),
        output_tokens: usage.and_then(|u| u.get("total_output_tokens")).and_then(Value::as_u64),
        incomplete: value.get("status").and_then(Value::as_str) == Some("incomplete"),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use std::net::SocketAddr;

    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::{get, post};
    use axum::{Json, Router};

    use super::*;

    pub const KEY: &str = "AIzaTestKeyForHubStudio0123456789abcd";

    pub fn key() -> ApiKey {
        ApiKey::parse(KEY).unwrap()
    }

    pub fn model_json(id: &str) -> Value {
        json!({
            "name": format!("models/{id}"), "baseModelId": id, "version": "001",
            "displayName": "Gemini 3.8 Flash", "inputTokenLimit": 1048576, "outputTokenLimit": 65536,
            "supportedGenerationMethods": ["generateContent", "countTokens"], "thinking": true
        })
    }

    /// Serve `router` on a free port of 127.0.0.1 and return its address.
    pub async fn serve(router: Router) -> String {
        let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        format!("http://{address}")
    }

    pub fn has_key(headers: &HeaderMap) -> bool {
        headers.get("x-goog-api-key").and_then(|v| v.to_str().ok()) == Some(KEY)
    }

    /// A Gemini that knows one model and answers the way the documentation shows.
    pub fn healthy() -> Router {
        Router::new()
            .route(
                "/v1beta/models/{model}",
                get(|headers: HeaderMap, axum::extract::Path(model): axum::extract::Path<String>| async move {
                    if !has_key(&headers) {
                        return (StatusCode::UNAUTHORIZED, Json(json!({"error": {"code": "authentication", "message": "API key not valid."}})));
                    }
                    if model != "gemini-3.8-flash" {
                        return (StatusCode::NOT_FOUND, Json(json!({"error": {"code": "model_not_found", "message": "not found"}})));
                    }
                    (StatusCode::OK, Json(model_json(&model)))
                }),
            )
            .route(
                "/v1beta/interactions",
                post(|headers: HeaderMap, Json(body): Json<Value>| async move {
                    assert!(has_key(&headers));
                    assert_eq!(body["store"], json!(false), "nothing is stored on Google's side");
                    (StatusCode::OK, Json(json!({
                        "id": "v1_abc", "object": "interaction", "status": "completed", "model": body["model"],
                        "steps": [
                            {"type": "thought", "signature": "x"},
                            {"type": "model_output", "content": [{"type": "text", "text": "Siap"}, {"type": "text", "text": "."}]}
                        ],
                        "usage": {"total_input_tokens": 9, "total_output_tokens": 2, "total_thought_tokens": 12, "total_tokens": 23}
                    })))
                }),
            )
    }

    #[tokio::test]
    async fn a_model_is_read_and_the_key_travels_in_the_header() {
        let base = serve(healthy()).await;
        let gemini = Gemini::new(&base, Duration::from_secs(5)).unwrap();
        let model = gemini.model(&key(), "gemini-3.8-flash").await.unwrap();
        assert_eq!(model.id, "gemini-3.8-flash");
        assert_eq!(model.output_token_limit, Some(65536));
        assert!(model.generates);
        let wrong = ApiKey::parse("AIzaSomeOtherKey0000000000000000000").unwrap();
        assert_eq!(gemini.model(&wrong, "gemini-3.8-flash").await.unwrap_err().kind, FailKind::Key);
        assert_eq!(gemini.model(&key(), "gemini-0").await.unwrap_err().kind, FailKind::Model);
    }

    #[tokio::test]
    async fn model_ids_never_reach_the_path_unchecked() {
        let gemini = Gemini::new("http://127.0.0.1:9", Duration::from_secs(1)).unwrap();
        for bad in ["../files", "gemini/../../x", "Gemini", "", "a?b=c"] {
            assert_eq!(gemini.model(&key(), bad).await.unwrap_err().kind, FailKind::Model, "{bad}");
        }
    }

    #[tokio::test]
    async fn the_probe_reads_the_text_of_model_output_steps() {
        let base = serve(healthy()).await;
        let gemini = Gemini::new(&base, Duration::from_secs(5)).unwrap();
        let answer = gemini.probe(&key(), "gemini-3.8-flash").await.unwrap();
        assert_eq!(answer.text, "Siap.");
        assert_eq!(answer.input_tokens, Some(9));
        assert_eq!(answer.output_tokens, Some(2));
    }

    #[tokio::test]
    async fn models_are_read_page_by_page_and_only_writers_kept() {
        let router = Router::new().route(
            "/v1beta/models",
            get(|axum::extract::Query(query): axum::extract::Query<std::collections::HashMap<String, String>>| async move {
                match query.get("pageToken").map(String::as_str) {
                    None => Json(json!({"models": [model_json("gemini-3.8-flash"),
                        {"name": "models/text-embedding-004", "supportedGenerationMethods": ["embedContent"]}],
                        "nextPageToken": "p2"})),
                    Some("p2") => Json(json!({"models": [model_json("gemini-3.5-flash-lite"),
                        {"name": "models/imagen-4", "supportedGenerationMethods": ["predict"]}]})),
                    Some(_) => Json(json!({"models": []})),
                }
            }),
        );
        let base = serve(router).await;
        let gemini = Gemini::new(&base, Duration::from_secs(5)).unwrap();
        let ids: Vec<String> = gemini.models(&key()).await.unwrap().into_iter().map(|m| m.id).collect();
        assert_eq!(ids, ["gemini-3.8-flash", "gemini-3.5-flash-lite"]);
    }

    #[test]
    fn only_loopback_may_stand_in_for_google() {
        assert!(is_loopback("http://127.0.0.1:4010"));
        assert!(is_loopback("http://localhost:4010/"));
        for bad in [
            "https://evil.example",
            "http://127.0.0.1.evil.example:80",
            "http://localhost.evil.example:80",
            "http://127.0.0.1:80@evil.example/",
            "http://user@127.0.0.1:80/",
            "https://127.0.0.1:443",
            "not a url",
        ] {
            assert!(!is_loopback(bad), "{bad}");
        }
    }

    #[test]
    fn a_failed_or_empty_interaction_is_not_an_answer() {
        assert_eq!(interaction_answer(&json!({"status": "failed"})).unwrap_err().kind, FailKind::Service);
        let incomplete = interaction_answer(&json!({"status": "incomplete", "steps": [{"type": "thought"}]})).unwrap();
        assert_eq!(incomplete.text, "", "cut by the output limit, but the model answered");
        // U5a: and it says it was cut, so a PRD part can name why its JSON is missing.
        assert!(incomplete.incomplete);
        assert!(!interaction_answer(&json!({"id": "v1_x", "status": "completed"})).unwrap().incomplete);
        // Google leaves an empty list out: the whole limit went to thinking (seen with
        // gemini-3.5-flash in Uji koneksi, 2026-11-08).
        let thought_only = interaction_answer(&json!({"id": "v1_x", "status": "incomplete", "usage": {"total_thought_tokens": 64}})).unwrap();
        assert_eq!(thought_only.text, "");
        assert_eq!(interaction_answer(&json!({"id": "v1_x", "status": "completed"})).unwrap().text, "");
        // Not an interaction, or steps that are not a list: broken, and the message names its shape.
        let broken = interaction_answer(&json!({"candidates": []})).unwrap_err();
        assert_eq!(broken.kind, FailKind::BadResponse);
        assert!(broken.message.contains("fields candidates"), "{}", broken.message);
        assert_eq!(interaction_answer(&json!({"id": "v1_x", "status": "completed", "steps": "x"})).unwrap_err().kind, FailKind::BadResponse);
    }
}
