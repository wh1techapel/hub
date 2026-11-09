//! "Uji koneksi": which of the usual problems it is, without changing anything.
//!
//! 1. Read the chosen model (`GET /models/{model}`): reaches Google, proves the key, proves the
//!    model exists for this key.
//! 2. A very short generation (`store: false`): proves the model writes for this key, since
//!    free-tier verification and quota show up only when generating.
//!
//! No interaction is stored on Google's side and nothing in Hub is written.

use serde::Serialize;
use ts_rs::TS;

use crate::client::{Gemini, ModelInfo};
use crate::error::{FailKind, GeminiError};
use crate::key::ApiKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CheckOutcome {
    /// The model answered: Studio can use it.
    Connected,
    /// No key is saved yet.
    NoKey,
    /// Google does not accept the key.
    KeyInvalid,
    /// The key is known but may not be used: account verification, a blocked key, a region.
    Permission,
    /// The model does not exist or this key cannot use it.
    ModelUnavailable,
    /// Quota or rate limit reached.
    Quota,
    /// This model or request needs billing.
    Payment,
    /// No connection to Google (offline, DNS, refused, timed out).
    Offline,
    /// Google is having trouble.
    Service,
    /// Anything else (an answer Hub could not read, a request Google refused).
    Unexpected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CheckReport {
    pub outcome: CheckOutcome,
    /// The model, once it was read.
    pub model: Option<ModelInfo>,
    /// Google's own words, or what failed on the way, for the owner who wants the detail.
    pub detail: Option<String>,
    /// Google's error code, when it gave one.
    pub code: Option<String>,
    /// Seconds Google asked to wait (quota).
    pub retry_after: Option<u32>,
}

fn outcome_of(kind: FailKind) -> CheckOutcome {
    match kind {
        FailKind::Offline | FailKind::Timeout => CheckOutcome::Offline,
        FailKind::Key => CheckOutcome::KeyInvalid,
        FailKind::Permission => CheckOutcome::Permission,
        FailKind::Model => CheckOutcome::ModelUnavailable,
        FailKind::Quota => CheckOutcome::Quota,
        FailKind::Payment => CheckOutcome::Payment,
        FailKind::Service => CheckOutcome::Service,
        FailKind::BadRequest | FailKind::BadResponse | FailKind::Cancelled => CheckOutcome::Unexpected,
    }
}

fn failed(error: GeminiError, model: Option<ModelInfo>) -> CheckReport {
    let detail = if error.message.is_empty() { None } else { Some(error.message) };
    CheckReport { outcome: outcome_of(error.kind), model, detail, code: error.code, retry_after: error.retry_after }
}

pub async fn check_connection(gemini: &Gemini, key: Option<&ApiKey>, model: &str) -> CheckReport {
    let Some(key) = key else {
        return CheckReport { outcome: CheckOutcome::NoKey, model: None, detail: None, code: None, retry_after: None };
    };
    let info = match gemini.model(key, model).await {
        Ok(info) => info,
        Err(error) => return failed(error, None),
    };
    match gemini.probe(key, model).await {
        Ok(_) => CheckReport { outcome: CheckOutcome::Connected, model: Some(info), detail: None, code: None, retry_after: None },
        Err(error) => failed(error, Some(info)),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::http::StatusCode;
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use serde_json::{json, Value};

    use super::*;
    use crate::client::tests::{healthy, key, model_json, serve};

    const MODEL: &str = "gemini-3.8-flash";

    async fn check(router: Router) -> CheckReport {
        let base = serve(router).await;
        let gemini = Gemini::new(&base, Duration::from_secs(5)).unwrap();
        check_connection(&gemini, Some(&key()), MODEL).await
    }

    /// The model reads fine; generating answers with `status` and `body`.
    fn generation_fails(status: StatusCode, body: Value) -> Router {
        Router::new()
            .route("/v1beta/models/{model}", get(|| async { Json(model_json(MODEL)) }))
            .route("/v1beta/interactions", post(move || async move { (status, Json(body)) }))
    }

    fn model_fails(status: StatusCode, body: Value) -> Router {
        Router::new().route("/v1beta/models/{model}", get(move || async move { (status, Json(body)) }))
    }

    #[tokio::test]
    async fn a_working_key_and_model_connect() {
        let report = check(healthy()).await;
        assert_eq!(report.outcome, CheckOutcome::Connected);
        assert_eq!(report.model.unwrap().id, MODEL);
    }

    #[tokio::test]
    async fn no_key_is_said_without_asking_google() {
        let gemini = Gemini::new("http://127.0.0.1:9", Duration::from_secs(1)).unwrap();
        assert_eq!(check_connection(&gemini, None, MODEL).await.outcome, CheckOutcome::NoKey);
    }

    #[tokio::test]
    async fn each_problem_is_named() {
        let cases = [
            (model_fails(StatusCode::UNAUTHORIZED, json!({"error": {"code": "authentication", "message": "API key not valid."}})), CheckOutcome::KeyInvalid),
            (model_fails(StatusCode::BAD_REQUEST, json!({"error": {"code": 400, "status": "INVALID_ARGUMENT", "message": "API key not valid.",
                "details": [{"reason": "API_KEY_INVALID"}]}})), CheckOutcome::KeyInvalid),
            (model_fails(StatusCode::NOT_FOUND, json!({"error": {"code": "model_not_found", "message": "Model not found"}})), CheckOutcome::ModelUnavailable),
            (model_fails(StatusCode::SERVICE_UNAVAILABLE, json!({"error": {"code": "service_unavailable", "message": "overloaded"}})), CheckOutcome::Service),
            (model_fails(StatusCode::INTERNAL_SERVER_ERROR, json!({})), CheckOutcome::Service),
            (generation_fails(StatusCode::TOO_MANY_REQUESTS, json!({"error": {"code": "quota_exceeded", "message": "Daily quota reached"}})), CheckOutcome::Quota),
            (generation_fails(StatusCode::FORBIDDEN, json!({"error": {"code": "permission_denied", "message": "FREE_TIER_VERIFICATION_REQUIRED"}})), CheckOutcome::Permission),
            (generation_fails(StatusCode::PAYMENT_REQUIRED, json!({"error": {"code": "payment_required", "message": "billing"}})), CheckOutcome::Payment),
            (generation_fails(StatusCode::BAD_REQUEST, json!({"error": {"code": "invalid_request", "message": "bad field"}})), CheckOutcome::Unexpected),
        ];
        for (router, expected) in cases {
            let report = check(router).await;
            assert_eq!(report.outcome, expected, "{report:?}");
        }
    }

    #[tokio::test]
    async fn quota_keeps_google_s_words_and_wait() {
        let router = Router::new()
            .route("/v1beta/models/{model}", get(|| async { Json(model_json(MODEL)) }))
            .route(
                "/v1beta/interactions",
                post(|| async {
                    (StatusCode::TOO_MANY_REQUESTS, [("retry-after", "42")], Json(json!({"error": {"code": "rate_limit_exceeded", "message": "Too many requests"}})))
                }),
            );
        let report = check(router).await;
        assert_eq!(report.outcome, CheckOutcome::Quota);
        assert_eq!(report.retry_after, Some(42));
        assert_eq!(report.detail.as_deref(), Some("Too many requests"));
        assert_eq!(report.code.as_deref(), Some("rate_limit_exceeded"));
        assert!(report.model.is_some(), "the model itself was fine");
    }

    #[tokio::test]
    async fn no_route_to_google_is_offline() {
        // Nothing listens on port 9 of this computer: the connection is refused.
        let refused = Gemini::new("http://127.0.0.1:9", Duration::from_secs(2)).unwrap();
        assert_eq!(check_connection(&refused, Some(&key()), MODEL).await.outcome, CheckOutcome::Offline);
        // A name that never resolves (.invalid is reserved for that).
        let unknown = Gemini::new("http://gemini.hub-test.invalid", Duration::from_secs(5)).unwrap();
        assert_eq!(check_connection(&unknown, Some(&key()), MODEL).await.outcome, CheckOutcome::Offline);
    }

    #[tokio::test]
    async fn a_silent_google_times_out_as_offline() {
        let router = Router::new().route(
            "/v1beta/models/{model}",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(5)).await;
                Json(model_json(MODEL))
            }),
        );
        let base = serve(router).await;
        let gemini = Gemini::new(&base, Duration::from_millis(300)).unwrap();
        let report = check_connection(&gemini, Some(&key()), MODEL).await;
        assert_eq!(report.outcome, CheckOutcome::Offline);
        assert_eq!(report.detail.as_deref(), Some("no answer in time"));
    }

    #[tokio::test]
    async fn the_report_never_holds_the_key() {
        for router in [healthy(), model_fails(StatusCode::UNAUTHORIZED, json!({"error": {"code": "authentication", "message": "bad"}}))] {
            let report = check(router).await;
            let text = serde_json::to_string(&report).unwrap();
            assert!(!text.contains(crate::client::tests::KEY));
        }
    }
}
