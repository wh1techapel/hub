//! The local API through its real router: every defence of docs/architecture.md ("Local API"),
//! then what ULTRON does with it. A real socket is used for the event stream.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use hub_api::{token, ApiEvent, Events, Host};
use hub_core::api_clients::{self, Scope};
use hub_core::model::{Actor, Entity};
use hub_core::{Change, Hub, Paths};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::watch;
use tower::ServiceExt;

const PORT: u16 = 47731;

struct TestHost {
    hub: Hub,
    events: Events,
    changes: Mutex<Vec<Change>>,
    vault: Mutex<Vec<String>>,
}

impl Host for TestHost {
    fn hub(&self) -> &Hub {
        &self.hub
    }
    fn events(&self) -> &Events {
        &self.events
    }
    fn changed(&self, change: &Change) {
        self.changes.lock().unwrap().push(change.clone());
        self.events.publish(ApiEvent::from(change));
    }
    fn kept_from_removable(&self, file_id: &str) {
        self.vault.lock().unwrap().push(file_id.into());
    }
    fn version(&self) -> String {
        "0.1.0-test".into()
    }
    fn log(&self, message: &str) {
        eprintln!("[hub-test] {message}");
    }
}

struct Setup {
    host: Arc<TestHost>,
    token: String,
    router: Router,
    _stop: watch::Sender<bool>,
    dir: tempfile::TempDir,
}

fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let hub = Hub::open(Paths::under(dir.path().join("hub"))).unwrap();
    token::ensure(&hub, PORT, token::ULTRON).unwrap();
    let token = token::current(hub.paths(), token::ULTRON).unwrap().token;
    let host = Arc::new(TestHost { hub, events: Events::default(), changes: Mutex::default(), vault: Mutex::default() });
    let (stop, stopping) = watch::channel(false);
    let router = hub_api::router(host.clone(), PORT, stopping);
    Setup { host, token, router, _stop: stop, dir }
}

impl Setup {
    fn request(&self, method: Method, path: &str, body: Option<Value>) -> axum::http::request::Builder {
        let builder = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, format!("127.0.0.1:{PORT}"))
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token));
        if body.is_some() { builder.header(header::CONTENT_TYPE, "application/json") } else { builder }
    }

    async fn send(&self, request: Request<Body>) -> (StatusCode, Value) {
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value = if bytes.is_empty() { Value::Null } else { serde_json::from_slice(&bytes).unwrap_or(Value::Null) };
        (status, value)
    }

    async fn call(&self, method: Method, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let request = self.request(method, path, body.clone());
        let body = body.map(|b| Body::from(b.to_string())).unwrap_or_else(Body::empty);
        self.send(request.body(body).unwrap()).await
    }
}

fn reason(value: &Value) -> &str {
    value["error"]["reason"].as_str().unwrap_or_default()
}

// ---------- defences ----------

#[tokio::test]
async fn every_request_needs_the_token_even_from_loopback() {
    let s = setup();
    let request = Request::builder().uri("/v1/health").header(header::HOST, format!("127.0.0.1:{PORT}")).body(Body::empty()).unwrap();
    let response = s.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.headers()[header::WWW_AUTHENTICATE], "Bearer");

    let wrong = Request::builder()
        .uri("/v1/health")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {}", "0".repeat(64)))
        .body(Body::empty())
        .unwrap();
    assert_eq!(s.send(wrong).await.0, StatusCode::UNAUTHORIZED);

    // An unknown route says nothing before the token is checked.
    let unknown = Request::builder().uri("/v1/nothing").header(header::HOST, format!("127.0.0.1:{PORT}")).body(Body::empty()).unwrap();
    assert_eq!(s.send(unknown).await.0, StatusCode::UNAUTHORIZED);

    let (status, health) = s.call(Method::GET, "/v1/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(health["client"], "ULTRON");
    assert_eq!(health["apiVersion"], 1);
}

#[tokio::test]
async fn requests_from_web_pages_are_refused() {
    let s = setup();
    for (name, value) in [("origin", "https://evil.example"), ("origin", "null"), ("sec-fetch-site", "cross-site")] {
        let request = s.request(Method::GET, "/v1/tasks", None).header(name, value).body(Body::empty()).unwrap();
        let (status, body) = s.send(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{name}: {value}");
        assert_eq!(reason(&body), "origin");
    }
    // A CORS preflight gets no CORS headers, only the refusal.
    let preflight = Request::builder()
        .method(Method::OPTIONS)
        .uri("/v1/tasks")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header("origin", "https://evil.example")
        .header("access-control-request-method", "POST")
        .body(Body::empty())
        .unwrap();
    let response = s.router.clone().oneshot(preflight).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(response.headers().get("access-control-allow-origin").is_none());
}

#[tokio::test]
async fn only_the_exact_loopback_host_passes() {
    let s = setup();
    for host in ["evil.example:47731", "127.0.0.1", "localhost:80", "127.0.0.2:47731", "[::1]:47731"] {
        let request = Request::builder()
            .uri("/v1/health")
            .header(header::HOST, host)
            .header(header::AUTHORIZATION, format!("Bearer {}", s.token))
            .body(Body::empty())
            .unwrap();
        let (status, body) = s.send(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{host}");
        assert_eq!(reason(&body), "host");
    }
    let absolute = Request::builder()
        .uri("http://evil.example:47731/v1/health")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {}", s.token))
        .body(Body::empty())
        .unwrap();
    assert_eq!(s.send(absolute).await.0, StatusCode::FORBIDDEN);

    let localhost = Request::builder()
        .uri("/v1/health")
        .header(header::HOST, format!("localhost:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {}", s.token))
        .body(Body::empty())
        .unwrap();
    assert_eq!(s.send(localhost).await.0, StatusCode::OK);
}

#[tokio::test]
async fn another_computer_is_refused() {
    let s = setup();
    let router = s.router.clone().layer(MockConnectInfo(SocketAddr::from(([192, 168, 1, 5], 50000))));
    let request = s.request(Method::GET, "/v1/health", None).body(Body::empty()).unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let local = s.router.clone().layer(MockConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50000))));
    let request = s.request(Method::GET, "/v1/health", None).body(Body::empty()).unwrap();
    assert_eq!(local.oneshot(request).await.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn each_write_needs_its_scope() {
    let s = setup();
    let reader = "1".repeat(64);
    s.host
        .hub
        .write(|conn| api_clients::set_token(conn, "Reader", &api_clients::hash_token(&reader), &[Scope::Read], Actor::Owner, jiff::Timestamp::now()))
        .unwrap();
    let ask = |method: Method, path: &str, body: Option<Value>| {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, format!("127.0.0.1:{PORT}"))
            .header(header::AUTHORIZATION, format!("Bearer {reader}"));
        if body.is_some() {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
        }
        builder.body(body.map(|b| Body::from(b.to_string())).unwrap_or_else(Body::empty)).unwrap()
    };
    assert_eq!(s.send(ask(Method::GET, "/v1/tasks", None)).await.0, StatusCode::OK);
    for (method, path, body) in [
        (Method::POST, "/v1/tasks", json!({ "title": "x" })),
        (Method::POST, "/v1/reminders", json!({ "text": "x", "dueAt": "2030-01-01T00:00:00Z" })),
        (Method::POST, "/v1/journal", json!({ "text": "x" })),
        (Method::POST, "/v1/files", json!({ "path": "C:\\Windows\\win.ini" })),
    ] {
        let (status, refused) = s.send(ask(method, path, Some(body))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        assert_eq!(reason(&refused), "scope");
    }
    assert!(s.host.changes.lock().unwrap().is_empty(), "nothing was written");
}

#[tokio::test]
async fn bodies_are_small_strict_json() {
    let s = setup();
    let (status, refused) = s.call(Method::POST, "/v1/tasks", Some(json!({ "title": "x", "owner": "evil" }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(refused["error"]["field"], "body");

    let big = json!({ "title": "x", "notes": "a".repeat(70 * 1024) });
    assert_eq!(s.call(Method::POST, "/v1/tasks", Some(big)).await.0, StatusCode::PAYLOAD_TOO_LARGE);

    let text = s.request(Method::POST, "/v1/tasks", None).header(header::CONTENT_TYPE, "text/plain").body(Body::from("{\"title\":\"x\"}")).unwrap();
    assert_eq!(s.send(text).await.0, StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let (status, refused) = s.call(Method::POST, "/v1/tasks", Some(json!({ "title": "   " }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(refused["error"]["field"], "title");

    let (status, _) = s.call(Method::GET, "/v1/tasks/not-an-id", None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = s.call(Method::GET, "/v1/tasks?view=everything", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_client_is_rate_limited() {
    let s = setup();
    let mut limited = false;
    for _ in 0..200 {
        let (status, body) = s.call(Method::GET, "/v1/health", None).await;
        if status == StatusCode::TOO_MANY_REQUESTS {
            assert_eq!(reason(&body), "rate");
            limited = true;
            break;
        }
    }
    assert!(limited);
}

#[tokio::test]
async fn a_rotated_token_replaces_the_old_one_at_once() {
    let mut s = setup();
    let old = s.token.clone();
    token::rotate(&s.host.hub, PORT, token::ULTRON).unwrap();
    assert_eq!(s.call(Method::GET, "/v1/health", None).await.0, StatusCode::UNAUTHORIZED);
    s.token = token::current(s.host.hub.paths(), token::ULTRON).unwrap().token;
    assert_ne!(s.token, old);
    assert_eq!(s.call(Method::GET, "/v1/health", None).await.0, StatusCode::OK);
}

#[tokio::test]
async fn network_and_relative_paths_are_refused() {
    let s = setup();
    for path in ["\\\\server\\share\\x.pdf", "//server/share/x.pdf", "relative\\x.pdf", "\\\\?\\C:\\x.pdf", ""] {
        let (status, refused) = s.call(Method::POST, "/v1/files", Some(json!({ "path": path }))).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{path}");
        assert_eq!(refused["error"]["field"], "path");
    }
    let missing = s.dir.path().join("not-there.pdf");
    let (status, refused) = s.call(Method::POST, "/v1/files", Some(json!({ "path": missing }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reason(&refused), "unknown");
}

// ---------- what ULTRON does ----------

#[tokio::test]
async fn tasks_made_by_ultron_carry_its_mark() {
    let s = setup();
    let (status, made) = s.call(Method::POST, "/v1/tasks", Some(json!({ "title": "Belajar subnetting", "doDate": "2026-10-24", "priority": 2 }))).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = made["id"].as_str().unwrap().to_string();
    assert_eq!(made["item"]["source"], "ultron");
    assert_eq!(made["item"]["doDate"], "2026-10-24");
    let actor: String = s.host.hub.read(|conn| Ok(conn.query_row("SELECT actor FROM activity WHERE entity_id = ?1", [&id], |row| row.get(0))?)).unwrap();
    assert_eq!(actor, "ultron");
    assert!(s.host.changes.lock().unwrap().iter().any(|c| c.entity == Entity::Task && c.ids == vec![id.clone()]));

    let (_, patched) = s.call(Method::PATCH, &format!("/v1/tasks/{id}"), Some(json!({ "dueDate": "2026-10-25", "dueTime": "21:00" }))).await;
    assert_eq!((patched["dueDate"].as_str(), patched["dueTime"].as_str()), (Some("2026-10-25"), Some("21:00")));
    let (_, cleared) = s.call(Method::PATCH, &format!("/v1/tasks/{id}"), Some(json!({ "dueDate": null, "title": "Belajar VLSM" }))).await;
    assert_eq!(cleared["dueDate"], Value::Null);
    assert_eq!(cleared["dueTime"], Value::Null, "the time goes with the deadline");
    assert_eq!(cleared["doDate"], "2026-10-24", "fields not named stay");
    assert_eq!(cleared["priority"], 2);

    let (_, done) = s.call(Method::POST, &format!("/v1/tasks/{id}/complete"), None).await;
    assert_eq!(done["changed"], true);
    let (_, again) = s.call(Method::POST, &format!("/v1/tasks/{id}/complete"), None).await;
    assert_eq!(again["changed"], false);
    let (_, listed) = s.call(Method::GET, "/v1/tasks?view=done", None).await;
    assert_eq!(listed.as_array().unwrap().len(), 1);

    let (status, _) = s.call(Method::DELETE, &format!("/v1/tasks/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s.call(Method::GET, &format!("/v1/tasks/{id}"), None).await.0, StatusCode::NOT_FOUND);
    let kept: i64 = s.host.hub.read(|conn| Ok(conn.query_row("SELECT count(*) FROM tasks WHERE id = ?1 AND deleted_at IS NOT NULL", [&id], |row| row.get(0))?)).unwrap();
    assert_eq!(kept, 1, "removal is soft: Hub can restore it");
}

#[tokio::test]
async fn an_import_runs_once() {
    let s = setup();
    let body = json!({ "title": "Kerjakan laporan", "importRef": "ultron:task:7" });
    let (first, made) = s.call(Method::POST, "/v1/tasks", Some(body.clone())).await;
    assert_eq!(first, StatusCode::CREATED);
    assert_eq!(made["item"]["source"], "import");
    let (second, again) = s.call(Method::POST, "/v1/tasks", Some(body)).await;
    assert_eq!(second, StatusCode::OK);
    assert_eq!(again["existed"], true);
    assert_eq!(again["id"], made["id"]);
    let (_, all) = s.call(Method::GET, "/v1/tasks", None).await;
    assert_eq!(all.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn reminders_are_set_moved_snoozed_and_done() {
    let s = setup();
    let (status, made) = s.call(Method::POST, "/v1/reminders", Some(json!({ "text": "Kumpulkan tugas", "dueAt": "2030-10-24T20:00:00+07:00" }))).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = made["id"].as_str().unwrap().to_string();
    assert_eq!(made["item"]["dueAt"], "2030-10-24T13:00:00Z");
    assert_eq!(made["item"]["source"], "ultron");

    let (_, moved) = s.call(Method::PATCH, &format!("/v1/reminders/{id}"), Some(json!({ "dueAt": "2030-10-24T21:00:00+07:00" }))).await;
    assert_eq!(moved["dueAt"], "2030-10-24T14:00:00Z");
    assert_eq!(moved["text"], "Kumpulkan tugas");

    let (_, snoozed) = s.call(Method::POST, &format!("/v1/reminders/{id}/snooze"), Some(json!({ "minutes": 10 }))).await;
    assert_eq!(snoozed["state"], "snoozed");
    let (status, _) = s.call(Method::POST, &format!("/v1/reminders/{id}/snooze"), Some(json!({ "minutes": 0 }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (_, done) = s.call(Method::POST, &format!("/v1/reminders/{id}/done"), None).await;
    assert_eq!(done["changed"], true);
    let (status, refused) = s.call(Method::PATCH, &format!("/v1/reminders/{id}"), Some(json!({ "text": "lagi" }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "a handled reminder stays handled");
    assert_eq!(refused["error"]["field"], "state");

    let (status, _) = s.call(Method::POST, "/v1/reminders", Some(json!({ "text": "x", "dueAt": "besok jam 8" }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn notes_keep_files_and_explain_tasks() {
    let s = setup();
    let file = s.dir.path().join("Tugas Jaringan.pdf");
    std::fs::write(&file, b"%PDF-1.4").unwrap();
    let (_, task) = s.call(Method::POST, "/v1/tasks", Some(json!({ "title": "Tugas jaringan", "dueDate": "2030-10-25" }))).await;
    let task_id = task["id"].as_str().unwrap().to_string();
    let (status, note) = s.call(Method::POST, "/v1/journal", Some(json!({ "text": "Ini tugas jaringan dari Pak Budi", "taskId": task_id }))).await;
    assert_eq!(status, StatusCode::CREATED);
    let note_id = note["id"].as_str().unwrap().to_string();
    assert_eq!(note["item"]["kind"], "note");

    let (status, kept) = s
        .call(Method::POST, "/v1/files", Some(json!({ "path": file, "attach": { "kind": "journal", "id": note_id }, "note": "dari Pak Budi" })))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let file_id = kept["id"].as_str().unwrap().to_string();
    assert_eq!(kept["item"]["note"], "dari Pak Budi");
    assert_eq!(kept["item"]["source"], "ultron");
    let (_, attached) = s.call(Method::POST, &format!("/v1/files/{file_id}/attach"), Some(json!({ "kind": "task", "id": task_id }))).await;
    assert_eq!(attached["links"].as_array().unwrap().len(), 2);

    let (_, found) = s.call(Method::GET, "/v1/journal?q=Budi", None).await;
    assert_eq!(found[0]["id"], note_id);
    assert_eq!(found[0]["files"][0]["id"], file_id);
    let (_, for_task) = s.call(Method::GET, &format!("/v1/journal?taskId={task_id}"), None).await;
    assert_eq!(for_task.as_array().unwrap().len(), 1);

    let (_, hits) = s.call(Method::GET, "/v1/files/search?q=jaringan", None).await;
    assert_eq!(hits[0]["file"]["id"], file_id);
    assert_eq!(hits[0]["via"], Value::Null, "its own name matched");

    let (_, unlinked) = s.call(Method::PATCH, &format!("/v1/journal/{note_id}"), Some(json!({ "taskId": null }))).await;
    assert_eq!(unlinked["taskId"], Value::Null);
    let (status, _) = s.call(Method::DELETE, &format!("/v1/journal/{note_id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(s.host.vault.lock().unwrap().is_empty(), "a file on a fixed drive gets no Vault copy");
}

#[tokio::test]
async fn files_are_found_through_their_project() {
    let s = setup();
    let file = s.dir.path().join("topologi.pkt");
    std::fs::write(&file, b"pkt").unwrap();
    let project = s
        .host
        .hub
        .write(|conn| {
            let input: hub_core::projects::ProjectInput =
                serde_json::from_value(json!({ "name": "Proyek Jaringan Sekolah", "pigment": "toska" })).unwrap();
            hub_core::projects::create(conn, input, Actor::Owner, jiff::Timestamp::now())
        })
        .unwrap()
        .0;
    let (status, _) = s.call(Method::POST, "/v1/files", Some(json!({ "path": file, "attach": { "kind": "project", "id": project } }))).await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, hits) = s.call(Method::GET, "/v1/files/search?q=jaringan%20sekolah", None).await;
    assert_eq!(hits[0]["file"]["name"], "topologi.pkt");
    assert_eq!(hits[0]["via"]["kind"], "project");
    assert_eq!(hits[0]["via"]["title"], "Proyek Jaringan Sekolah");
    let (_, project_detail) = s.call(Method::GET, &format!("/v1/projects/{project}"), None).await;
    assert_eq!(project_detail["files"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn the_day_and_the_schedule_are_read() {
    let s = setup();
    assert_eq!(s.call(Method::GET, "/v1/today", None).await.0, StatusCode::OK);
    let (status, days) = s.call(Method::GET, "/v1/schedule?from=2026-10-26&to=2026-11-01", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(days.as_array().unwrap().len(), 7);
    let (status, _) = s.call(Method::GET, "/v1/schedule?from=2026-10-26&to=2026-12-31", None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = s.call(Method::GET, "/v1/schedule?from=2026-10-26&to=2026-10-25", None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(s.call(Method::GET, "/v1/learning/summary", None).await.0, StatusCode::OK);
    assert_eq!(s.call(Method::GET, "/v1/projects", None).await.0, StatusCode::OK);
    assert_eq!(s.call(Method::GET, "/v1/search?q=apa", None).await.0, StatusCode::OK);
}

// ---------- the event stream, over a real socket ----------

async fn read_event(lines: &mut tokio::io::Lines<BufReader<tokio::net::tcp::OwnedReadHalf>>, name: &str) -> Value {
    let wanted = format!("event: {name}");
    loop {
        let line = lines.next_line().await.unwrap().expect("the stream ended early");
        if line == wanted {
            let data = lines.next_line().await.unwrap().unwrap();
            return serde_json::from_str(data.trim_start_matches("data: ")).unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_event_stream_tells_changes_and_reminders_and_ends_with_the_server() {
    let s = setup();
    let server = hub_api::start(s.host.clone(), 0).await.unwrap();
    let stream = tokio::net::TcpStream::connect(("127.0.0.1", server.port)).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let request = format!(
        "GET /v1/events HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nAccept: text/event-stream\r\n\r\n",
        server.port, s.token
    );
    writer.write_all(request.as_bytes()).await.unwrap();
    let mut lines = BufReader::new(reader).lines();
    let status = lines.next_line().await.unwrap().unwrap();
    assert!(status.starts_with("HTTP/1.1 200"), "{status}");
    assert_eq!(read_event(&mut lines, "hello").await["apiVersion"], 1);

    s.host.changed(&Change::one(Entity::Task, "abc"));
    let changed = read_event(&mut lines, "changed").await;
    assert_eq!((changed["entity"].as_str(), changed["ids"][0].as_str()), (Some("task"), Some("abc")));

    s.host.events.publish(ApiEvent::Reminder { id: "r1".into(), text: "Minum obat".into(), due_at: "2026-10-23T13:00:00Z".into(), late: false });
    let reminder = read_event(&mut lines, "reminder").await;
    assert_eq!((reminder["text"].as_str(), reminder["late"].as_bool()), (Some("Minum obat"), Some(false)));

    server.stop();
    let ended = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Ok(Some(_)) = lines.next_line().await {}
    })
    .await;
    assert!(ended.is_ok(), "the stream ends when the server stops");
    assert!(tokio::net::TcpStream::connect(("127.0.0.1", server.port)).await.is_err(), "the port is closed");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rotated_token_ends_open_streams() {
    let s = setup();
    let server = hub_api::start(s.host.clone(), 0).await.unwrap();
    let stream = tokio::net::TcpStream::connect(("127.0.0.1", server.port)).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let request = format!("GET /v1/events HTTP/1.1\r\nHost: localhost:{}\r\nAuthorization: Bearer {}\r\n\r\n", server.port, s.token);
    writer.write_all(request.as_bytes()).await.unwrap();
    let mut lines = BufReader::new(reader).lines();
    read_event(&mut lines, "hello").await;
    s.host.events.publish(ApiEvent::TokenRotated);
    let ended = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Ok(Some(line)) = lines.next_line().await {
            assert!(!line.contains("rotated"), "the rotation itself is not sent");
            if line == "0" {
                break;
            }
        }
    })
    .await;
    assert!(ended.is_ok());
}
