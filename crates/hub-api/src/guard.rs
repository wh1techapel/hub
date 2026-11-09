//! The checks in front of every route (see the crate docs for why each exists). A refused
//! request touches nothing: no database read happens before the browser and `Host` checks pass.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::extract::connect_info::MockConnectInfo;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use hub_core::api_clients::{self, ApiClient, Scope};
use hub_core::settings;
use jiff::Timestamp;

use crate::error::ApiError;
use crate::ApiState;

/// A client may burst this many requests, refilled at `RATE` per second.
const BURST: f64 = 60.0;
const RATE: f64 = 20.0;
/// A token is 32 random bytes as hex.
pub const TOKEN_CHARS: usize = 64;

/// The hash of the token a request came with, for what is bound to a token (Studio sessions).
#[derive(Debug, Clone)]
pub struct Presented(pub String);

/// Each client the owner can switch off in Pengaturan: its name and whether it is on.
fn switched_on(name: &str, stored: &settings::Settings) -> bool {
    match name {
        crate::token::CLIENT => stored.api_enabled,
        n if n == crate::token::CLAUDE.name => stored.claude_enabled,
        _ => true,
    }
}

/// Headers only a browser sends: their presence means a web page made this request.
const BROWSER_HEADERS: [&str; 2] = ["origin", "sec-fetch-site"];

pub(crate) async fn check(State(state): State<Arc<ApiState>>, mut request: Request, next: Next) -> Response {
    // The listener binds 127.0.0.1 only; this holds even if that ever changed. (Tests give the
    // peer through MockConnectInfo.)
    let extensions = request.extensions();
    let peer = extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0)
        .or_else(|| extensions.get::<MockConnectInfo<SocketAddr>>().map(|info| info.0));
    if peer.is_some_and(|peer| !peer.ip().is_loopback()) {
        return ApiError::forbidden("peer", "only this computer may connect").into_response();
    }
    if BROWSER_HEADERS.iter().any(|name| request.headers().contains_key(*name)) {
        return ApiError::forbidden("origin", "requests from web pages are refused").into_response();
    }
    if !host_ok(request.headers(), request.uri().authority().map(|a| a.as_str()), state.port) {
        return ApiError::forbidden("host", "the Host header must name 127.0.0.1 or localhost with Hub's port").into_response();
    }
    let Some(token) = bearer(request.headers()) else {
        return ApiError::unauthorized().into_response();
    };
    let hash = api_clients::hash_token(token);
    let host = state.host.clone();
    let presented = hash.clone();
    let found = tokio::task::spawn_blocking(move || {
        host.hub().read(|conn| {
            let client = api_clients::find_by_hash(conn, &hash)?;
            let on = match &client {
                Some(client) => switched_on(&client.name, &settings::load(conn)?),
                None => false,
            };
            if let Some(client) = client.as_ref().filter(|_| on) {
                api_clients::touch(conn, client, Timestamp::now())?;
            }
            Ok(client.map(|client| (client, on)))
        })
    })
    .await;
    let client = match found {
        Ok(Ok(Some((_, false)))) => return ApiError::forbidden("client", "the owner has switched this client off in Hub").into_response(),
        Ok(Ok(Some((client, true)))) => client,
        Ok(Ok(None)) => return ApiError::unauthorized().into_response(),
        Ok(Err(error)) => {
            state.host.log(&format!("api auth: {error}"));
            return ApiError::internal().into_response();
        }
        Err(_) => return ApiError::internal().into_response(),
    };
    if !state.limiter.allow(&client.id) {
        return ApiError::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "rate", "too many requests").into_response();
    }
    request.extensions_mut().insert(client);
    request.extensions_mut().insert(Presented(presented));
    next.run(request).await
}

/// Exactly `127.0.0.1:<port>` or `localhost:<port>`; an absolute request target must agree.
fn host_ok(headers: &HeaderMap, authority: Option<&str>, port: u16) -> bool {
    let allowed = |value: &str| {
        let value = value.to_ascii_lowercase();
        value == format!("127.0.0.1:{port}") || value == format!("localhost:{port}")
    };
    let Some(host) = headers.get(header::HOST).and_then(|value| value.to_str().ok()) else {
        return false;
    };
    if headers.get_all(header::HOST).iter().count() != 1 || !allowed(host) {
        return false;
    }
    authority.is_none_or(allowed)
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && token.len() == TOKEN_CHARS && token.bytes().all(|b| b.is_ascii_hexdigit()))
        .then_some(token)
}

/// Refuse a route to a client without `scope`.
pub fn need(client: &ApiClient, scope: Scope) -> Result<(), ApiError> {
    if client.may(scope) {
        Ok(())
    } else {
        Err(ApiError::forbidden("scope", "this client may not do that"))
    }
}

/// A token bucket per client.
#[derive(Default)]
pub struct Limiter {
    buckets: Mutex<HashMap<String, (f64, Instant)>>,
}

impl Limiter {
    pub fn allow(&self, client: &str) -> bool {
        let Ok(mut buckets) = self.buckets.lock() else { return false };
        let now = Instant::now();
        let (tokens, at) = buckets.entry(client.to_string()).or_insert((BURST, now));
        *tokens = (*tokens + now.duration_since(*at).as_secs_f64() * RATE).min(BURST);
        *at = now;
        if *tokens >= 1.0 {
            *tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.append(*name, HeaderValue::from_str(value).unwrap());
        }
        map
    }

    #[test]
    fn only_loopback_names_with_the_port_pass() {
        assert!(host_ok(&headers(&[("host", "127.0.0.1:47731")]), None, 47731));
        assert!(host_ok(&headers(&[("host", "LOCALHOST:47731")]), None, 47731));
        assert!(!host_ok(&headers(&[("host", "127.0.0.1")]), None, 47731));
        assert!(!host_ok(&headers(&[("host", "evil.example:47731")]), None, 47731));
        assert!(!host_ok(&headers(&[("host", "127.0.0.1:80")]), None, 47731));
        assert!(!host_ok(&headers(&[]), None, 47731));
        assert!(!host_ok(&headers(&[("host", "127.0.0.1:47731"), ("host", "evil:47731")]), None, 47731));
        assert!(!host_ok(&headers(&[("host", "127.0.0.1:47731")]), Some("evil.example:47731"), 47731));
    }

    #[test]
    fn a_bearer_token_has_one_shape() {
        let token = "a".repeat(64);
        assert_eq!(bearer(&headers(&[("authorization", &format!("Bearer {token}"))])), Some(token.as_str()));
        assert_eq!(bearer(&headers(&[("authorization", &format!("bearer {token}"))])), Some(token.as_str()));
        assert_eq!(bearer(&headers(&[("authorization", &format!("Basic {token}"))])), None);
        assert_eq!(bearer(&headers(&[("authorization", "Bearer short")])), None);
        assert_eq!(bearer(&headers(&[("authorization", &format!("Bearer {}", "z".repeat(64)))])), None);
    }

    #[test]
    fn the_bucket_empties_and_refills() {
        let limiter = Limiter::default();
        let allowed = (0..100).filter(|_| limiter.allow("ultron")).count();
        assert_eq!(allowed, BURST as usize);
        assert!(limiter.allow("someone-else"));
    }
}
