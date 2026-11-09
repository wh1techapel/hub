//! Google's errors, sorted into what the owner can do about them.
//!
//! The Interactions API answers `{"error": {"code": "authentication", "message": "..."}}` with a
//! string code (aistudio.google.com/docs/api-errors); older endpoints may answer the google.rpc
//! shape with a number and a `status`. Both are read; the HTTP status decides when the code is
//! unknown. Google's message is kept short and plain for the log line and the UI's detail. It
//! never contains the key, and no request header or body is ever kept.

use serde::Serialize;
use ts_rs::TS;

/// What went wrong, as the owner would put it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum FailKind {
    /// No route to Google: no network, DNS failed, refused, or TLS could not start.
    Offline,
    /// Google did not answer in time.
    Timeout,
    /// The key is missing, wrong, or expired (401).
    Key,
    /// The key is known but not allowed: the account needs verification, the key was blocked or
    /// leaked, or the region is not served (403).
    Permission,
    /// The model does not exist or this key cannot use it (404).
    Model,
    /// Quota or rate limit (429).
    Quota,
    /// Billing is needed for this request (402).
    Payment,
    /// Google is having trouble (5xx).
    Service,
    /// Google refused the request itself (400): a bug on Hub's side.
    BadRequest,
    /// The answer could not be read.
    BadResponse,
    /// The owner stopped it.
    Cancelled,
}

impl FailKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FailKind::Offline => "offline",
            FailKind::Timeout => "timeout",
            FailKind::Key => "key",
            FailKind::Permission => "permission",
            FailKind::Model => "model",
            FailKind::Quota => "quota",
            FailKind::Payment => "payment",
            FailKind::Service => "service",
            FailKind::BadRequest => "bad_request",
            FailKind::BadResponse => "bad_response",
            FailKind::Cancelled => "cancelled",
        }
    }

    /// Whether trying again later may work by itself (408, 429, 5xx, a lost connection).
    pub fn retryable(self) -> bool {
        matches!(self, FailKind::Offline | FailKind::Timeout | FailKind::Quota | FailKind::Service)
    }
}

/// A failed call to Gemini.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("gemini {}: {message}", kind.as_str())]
pub struct GeminiError {
    pub kind: FailKind,
    /// The HTTP status, when Google answered.
    pub status: Option<u16>,
    /// Google's error code (`quota_exceeded`, `RESOURCE_EXHAUSTED`, ...).
    pub code: Option<String>,
    /// Google's message, or what failed on the way, shortened.
    pub message: String,
    /// Seconds Google asked to wait (`Retry-After`), for quota.
    pub retry_after: Option<u32>,
}

/// Longest message kept from Google.
const MESSAGE_MAX: usize = 300;

/// Control characters out, length capped: the message goes into a log line and the UI.
pub(crate) fn plain(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.chars().count() > MESSAGE_MAX {
        let mut short: String = cleaned.chars().take(MESSAGE_MAX).collect();
        short.push('…');
        short
    } else {
        cleaned
    }
}

impl GeminiError {
    /// What the log keeps of a failure: its kind and Google's code and, when Google refused the
    /// request or the answer could not be read, the short message that says why (it names the
    /// field or limit at fault). Without it a refused PRD part could not be diagnosed (real run,
    /// 2026-11-09). Other kinds keep no message.
    pub fn log_line(&self) -> String {
        let code = self.code.as_deref().unwrap_or("-");
        match self.kind {
            FailKind::BadRequest | FailKind::BadResponse => format!("{} ({code}): {}", self.kind.as_str(), self.message),
            _ => format!("{} ({code})", self.kind.as_str()),
        }
    }

    pub(crate) fn new(kind: FailKind, message: impl AsRef<str>) -> GeminiError {
        GeminiError { kind, status: None, code: None, message: plain(message.as_ref()), retry_after: None }
    }

    /// An HTTP answer that was not a success, sorted by Google's code, then by its status.
    pub(crate) fn from_answer(status: u16, body: &str, retry_after: Option<u32>) -> GeminiError {
        let error = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|value| value.get("error").cloned());
        let text_code = error.as_ref().and_then(|e| e.get("code")).and_then(|c| c.as_str()).map(str::to_string);
        let rpc_status = error.as_ref().and_then(|e| e.get("status")).and_then(|s| s.as_str()).map(str::to_string);
        let reason = error
            .as_ref()
            .and_then(|e| e.get("details"))
            .and_then(|d| d.as_array())
            .and_then(|details| details.iter().find_map(|d| d.get("reason").and_then(|r| r.as_str())))
            .map(str::to_string);
        let message = error
            .as_ref()
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        let code = text_code.clone().or(reason.clone()).or(rpc_status.clone());

        let by_code = code.as_deref().and_then(|code| match code {
            "authentication" | "unauthenticated" | "UNAUTHENTICATED" | "API_KEY_INVALID" | "API_KEY_EXPIRED" => {
                Some(FailKind::Key)
            }
            "permission_denied" | "PERMISSION_DENIED" | "FREE_TIER_VERIFICATION_REQUIRED" => Some(FailKind::Permission),
            "model_not_found" | "not_found" | "NOT_FOUND" => Some(FailKind::Model),
            "rate_limit_exceeded" | "quota_exceeded" | "too_many_requests" | "RESOURCE_EXHAUSTED" => {
                Some(FailKind::Quota)
            }
            "payment_required" => Some(FailKind::Payment),
            "api_error" | "internal" | "INTERNAL" | "service_unavailable" | "UNAVAILABLE" | "deadline_exceeded"
            | "DEADLINE_EXCEEDED" | "gateway_timeout" => Some(FailKind::Service),
            "cancelled" | "CANCELLED" => Some(FailKind::Cancelled),
            _ => None,
        });
        // An invalid key used to come back as 400 INVALID_ARGUMENT with the reason API_KEY_INVALID;
        // the reason is read above before the status decides.
        let kind = by_code.unwrap_or(match status {
            401 => FailKind::Key,
            402 => FailKind::Payment,
            403 => FailKind::Permission,
            404 => FailKind::Model,
            408 => FailKind::Timeout,
            429 => FailKind::Quota,
            400..=499 => FailKind::BadRequest,
            _ => FailKind::Service,
        });
        GeminiError { kind, status: Some(status), code, message: plain(&message), retry_after }
    }

    /// A transport failure from reqwest. Its text may name the URL (never the key: the key travels
    /// in a header), so only the kind of failure is kept.
    pub(crate) fn from_transport(error: &reqwest::Error) -> GeminiError {
        if error.is_timeout() {
            GeminiError::new(FailKind::Timeout, "no answer in time")
        } else if error.is_connect() {
            GeminiError::new(FailKind::Offline, "could not connect")
        } else if error.is_decode() || error.is_body() {
            GeminiError::new(FailKind::BadResponse, "the answer could not be read")
        } else {
            GeminiError::new(FailKind::Offline, "the connection failed")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interactions_codes_decide_first() {
        let error = GeminiError::from_answer(429, r#"{"error":{"code":"quota_exceeded","message":"Daily limit"}}"#, Some(30));
        assert_eq!(error.kind, FailKind::Quota);
        assert_eq!(error.code.as_deref(), Some("quota_exceeded"));
        assert_eq!(error.message, "Daily limit");
        assert_eq!(error.retry_after, Some(30));
        assert_eq!(GeminiError::from_answer(401, r#"{"error":{"code":"authentication","message":"x"}}"#, None).kind, FailKind::Key);
        assert_eq!(GeminiError::from_answer(404, r#"{"error":{"code":"model_not_found","message":"x"}}"#, None).kind, FailKind::Model);
        assert_eq!(GeminiError::from_answer(403, r#"{"error":{"code":"permission_denied","message":"x"}}"#, None).kind, FailKind::Permission);
        assert_eq!(GeminiError::from_answer(503, r#"{"error":{"code":"service_unavailable","message":"x"}}"#, None).kind, FailKind::Service);
    }

    #[test]
    fn the_log_keeps_googles_reason_only_for_a_refused_or_unreadable_answer() {
        let refused = GeminiError::from_answer(400, r#"{"error":{"code":"invalid_request","message":"max_output_tokens too large"}}"#, None);
        assert_eq!(refused.log_line(), "bad_request (invalid_request): max_output_tokens too large");
        let quota = GeminiError::from_answer(429, r#"{"error":{"code":"quota_exceeded","message":"Daily limit for project 123"}}"#, None);
        assert_eq!(quota.log_line(), "quota (quota_exceeded)");
        assert_eq!(GeminiError::new(FailKind::BadResponse, "the answer is not the JSON asked for").log_line(), "bad_response (-): the answer is not the JSON asked for");
    }

    #[test]
    fn the_older_rpc_shape_is_read_too() {
        let body = r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT",
            "details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"API_KEY_INVALID"}]}}"#;
        let error = GeminiError::from_answer(400, body, None);
        assert_eq!(error.kind, FailKind::Key, "the reason wins over the 400");
        assert_eq!(error.code.as_deref(), Some("API_KEY_INVALID"));
        assert_eq!(GeminiError::from_answer(429, r#"{"error":{"code":429,"status":"RESOURCE_EXHAUSTED","message":"x"}}"#, None).kind, FailKind::Quota);
    }

    #[test]
    fn unknown_or_missing_bodies_fall_back_on_the_status() {
        assert_eq!(GeminiError::from_answer(502, "<html>Bad gateway</html>", None).kind, FailKind::Service);
        assert_eq!(GeminiError::from_answer(400, "", None).kind, FailKind::BadRequest);
        assert_eq!(GeminiError::from_answer(408, "{}", None).kind, FailKind::Timeout);
        assert_eq!(GeminiError::from_answer(404, r#"{"error":{"code":"something_new"}}"#, None).kind, FailKind::Model);
    }

    #[test]
    fn messages_are_plain_and_short() {
        let long = format!("line one\nline\ttwo {}", "x".repeat(400));
        let message = plain(&long);
        assert!(message.starts_with("line one line two"));
        assert!(message.chars().count() <= MESSAGE_MAX + 1);
        assert!(!message.contains('\n'));
    }

    #[test]
    fn only_passing_troubles_are_retried() {
        assert!(FailKind::Quota.retryable());
        assert!(FailKind::Service.retryable());
        assert!(!FailKind::Key.retryable());
        assert!(!FailKind::BadRequest.retryable());
    }
}
