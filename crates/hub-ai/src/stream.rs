//! Streaming an interaction (server-sent events) and a whole chat reply.
//!
//! An interaction streams as steps: `step.start` (the step's type; a function call brings its id
//! and name), `step.delta` (text, a thought signature, or a piece of a function call's arguments),
//! `step.stop`, then `interaction.completed` with the usage, or an `error` event
//! (aistudio.google.com/docs/streaming). The steps are rebuilt as they arrive, because a reply that
//! calls a function continues in a second request that must carry them back exactly.

use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::sync::watch;

use crate::client::Gemini;
use crate::error::{FailKind, GeminiError};
use crate::key::ApiKey;
use crate::prompt::{chat_request, ChatContext};

/// A function Gemini asked Hub to run.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// How one streamed interaction ended.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Streamed {
    pub text: String,
    pub calls: Vec<FunctionCall>,
    /// The model's steps, rebuilt, to send back when the reply continues.
    pub steps: Vec<Value>,
    /// `completed`, `requires_action`, `incomplete`, ...
    pub status: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Default)]
struct StepBuilder {
    kind: String,
    text: String,
    signature: Option<String>,
    id: String,
    name: String,
    arguments: String,
    start_arguments: Option<Value>,
}

impl StepBuilder {
    fn finish(self) -> (Value, Option<FunctionCall>) {
        match self.kind.as_str() {
            "model_output" => (json!({ "type": "model_output", "content": [{ "type": "text", "text": self.text }] }), None),
            "thought" => {
                let mut step = json!({ "type": "thought" });
                if let Some(signature) = self.signature {
                    step["signature"] = json!(signature);
                }
                (step, None)
            }
            "function_call" => {
                let arguments = if self.arguments.trim().is_empty() {
                    self.start_arguments.unwrap_or_else(|| json!({}))
                } else {
                    serde_json::from_str(&self.arguments).unwrap_or(Value::Null)
                };
                let step = json!({ "type": "function_call", "id": self.id, "name": self.name, "arguments": arguments });
                let call = FunctionCall { id: self.id, name: self.name, arguments };
                (step, Some(call))
            }
            other => (json!({ "type": other }), None),
        }
    }
}

/// Reads server-sent events into [`Streamed`], telling `on_text` each piece of text as it comes.
#[derive(Default)]
pub(crate) struct EventReader {
    buffer: String,
    open: Vec<(i64, StepBuilder)>,
    pub(crate) out: Streamed,
    pub(crate) done: bool,
}

fn usage_of(value: &Value, out: &mut Streamed) {
    let read = |key: &str| value.get(key).and_then(Value::as_u64).map(|n| n.min(u32::MAX as u64) as u32);
    if let Some(n) = read("total_input_tokens") {
        out.input_tokens = n;
    }
    if let Some(n) = read("total_output_tokens") {
        out.output_tokens = n;
    }
}

impl EventReader {
    /// Feed raw bytes as they arrive; complete events are handled at once.
    pub(crate) fn feed(&mut self, chunk: &str, on_text: &mut dyn FnMut(&str)) -> Result<(), GeminiError> {
        self.buffer.push_str(&chunk.replace("\r\n", "\n"));
        while let Some(end) = self.buffer.find("\n\n") {
            let block: String = self.buffer.drain(..end + 2).collect();
            let data: String = block
                .lines()
                .filter_map(|line| line.strip_prefix("data:"))
                .map(str::trim_start)
                .collect::<Vec<_>>()
                .join("\n");
            if data.is_empty() {
                continue;
            }
            if data.trim() == "[DONE]" {
                self.done = true;
                continue;
            }
            let Ok(event) = serde_json::from_str::<Value>(&data) else { continue };
            self.handle(&event, on_text)?;
        }
        Ok(())
    }

    fn step(&mut self, index: i64) -> Option<&mut StepBuilder> {
        self.open.iter_mut().find(|(i, _)| *i == index).map(|(_, step)| step)
    }

    fn handle(&mut self, event: &Value, on_text: &mut dyn FnMut(&str)) -> Result<(), GeminiError> {
        let index = event.get("index").and_then(Value::as_i64).unwrap_or(0);
        match event.get("event_type").and_then(Value::as_str).unwrap_or("") {
            "step.start" => {
                let step = event.get("step").cloned().unwrap_or(Value::Null);
                let builder = StepBuilder {
                    kind: step.get("type").and_then(Value::as_str).unwrap_or("").to_string(),
                    id: step.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
                    name: step.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                    signature: step.get("signature").and_then(Value::as_str).map(str::to_string),
                    start_arguments: step.get("arguments").filter(|a| a.as_object().is_some_and(|o| !o.is_empty())).cloned(),
                    ..Default::default()
                };
                self.open.retain(|(i, _)| *i != index);
                self.open.push((index, builder));
            }
            "step.delta" => {
                let delta = event.get("delta").cloned().unwrap_or(Value::Null);
                let kind = delta.get("type").and_then(Value::as_str).unwrap_or("");
                // A delta for a step that never started is kept as model output.
                if self.step(index).is_none() {
                    self.open.push((index, StepBuilder { kind: "model_output".into(), ..Default::default() }));
                }
                let step = self.step(index).expect("just made");
                match kind {
                    "text" => {
                        let text = delta.get("text").and_then(Value::as_str).unwrap_or("");
                        if step.kind == "model_output" {
                            step.text.push_str(text);
                            self.out.text.push_str(text);
                            on_text(text);
                        }
                    }
                    "thought_signature" => step.signature = delta.get("signature").and_then(Value::as_str).map(str::to_string),
                    "arguments_delta" => step.arguments.push_str(delta.get("arguments").and_then(Value::as_str).unwrap_or("")),
                    _ => {}
                }
            }
            "step.stop" => {
                if let Some(position) = self.open.iter().position(|(i, _)| *i == index) {
                    let (_, builder) = self.open.remove(position);
                    let (step, call) = builder.finish();
                    self.out.steps.push(step);
                    if let Some(call) = call {
                        self.out.calls.push(call);
                    }
                }
                if let Some(usage) = event.get("usage") {
                    usage_of(usage, &mut self.out);
                }
            }
            "interaction.created" | "interaction.status_update" => {
                if let Some(status) = event.get("status").or_else(|| event.pointer("/interaction/status")).and_then(Value::as_str) {
                    self.out.status = status.to_string();
                }
            }
            "interaction.completed" => {
                if let Some(interaction) = event.get("interaction") {
                    if let Some(status) = interaction.get("status").and_then(Value::as_str) {
                        self.out.status = status.to_string();
                    }
                    if let Some(usage) = interaction.get("usage") {
                        usage_of(usage, &mut self.out);
                    }
                }
                self.done = true;
            }
            "error" => {
                let error = event.get("error").cloned().unwrap_or(Value::Null);
                let body = json!({ "error": error }).to_string();
                let failure = GeminiError::from_answer(0, &body, None);
                // Unknown in-stream codes (`gateway_timeout`) count as Google's trouble.
                let kind = if failure.kind == FailKind::Service || failure.code.is_none() { FailKind::Service } else { failure.kind };
                return Err(GeminiError { kind, ..failure });
            }
            _ => {}
        }
        Ok(())
    }

    /// Steps still open when the stream ended are closed as they are.
    pub(crate) fn close(mut self) -> Streamed {
        for (_, builder) in std::mem::take(&mut self.open) {
            let (step, call) = builder.finish();
            self.out.steps.push(step);
            if let Some(call) = call {
                self.out.calls.push(call);
            }
        }
        self.out
    }
}

/// `true` once the owner pressed Hentikan.
pub type Cancel = watch::Receiver<bool>;

fn cancelled() -> GeminiError {
    GeminiError::new(FailKind::Cancelled, "stopped by the owner")
}

impl Gemini {
    /// Stream one interaction, telling `on_text` each piece of text. Stops at once when `cancel`
    /// turns true (the connection is dropped, which Google counts as a closed request).
    pub async fn stream(&self, key: &ApiKey, body: &Value, cancel: &mut Cancel, on_text: &mut (dyn FnMut(&str) + Send)) -> Result<Streamed, GeminiError> {
        if *cancel.borrow() {
            return Err(cancelled());
        }
        let request = self.post_interaction(body, key)?;
        let answer = tokio::select! {
            answer = request.send() => answer.map_err(|error| GeminiError::from_transport(&error))?,
            _ = cancel.wait_for(|stop| *stop) => return Err(cancelled()),
        };
        let status = answer.status().as_u16();
        if !(200..300).contains(&status) {
            let retry_after = answer
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u32>().ok());
            let body = answer.text().await.unwrap_or_default();
            return Err(GeminiError::from_answer(status, &body, retry_after));
        }
        let mut reader = EventReader::default();
        let mut bytes = answer.bytes_stream();
        let mut pending: Vec<u8> = Vec::new();
        loop {
            let next = tokio::select! {
                next = bytes.next() => next,
                _ = cancel.wait_for(|stop| *stop) => return Err(cancelled()),
            };
            let Some(chunk) = next else { break };
            let chunk = chunk.map_err(|error| GeminiError::from_transport(&error))?;
            pending.extend_from_slice(&chunk);
            // Only whole UTF-8 characters are handed on; a split one waits for the next chunk.
            let valid = match std::str::from_utf8(&pending) {
                Ok(text) => text.len(),
                Err(error) => error.valid_up_to(),
            };
            let text: String = String::from_utf8_lossy(&pending[..valid]).into_owned();
            pending.drain(..valid);
            reader.feed(&text, on_text)?;
            if reader.done {
                break;
            }
        }
        reader.feed("\n\n", on_text)?;
        Ok(reader.close())
    }
}

/// The most rounds one reply may take (each function call round is one more request).
pub const ROUNDS_MAX: usize = 3;

/// A whole reply: text streamed to `on_text`, each function call handed to `on_call` (which
/// applies it and returns its result), and further rounds while Gemini waits for results.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Reply {
    pub text: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    /// The request was sent again without functions after Google refused them.
    pub without_tools: bool,
}

pub async fn reply(
    gemini: &Gemini,
    key: &ApiKey,
    model: &str,
    context: &ChatContext,
    cancel: &mut Cancel,
    on_text: &mut (dyn FnMut(&str) + Send),
    on_call: &mut (dyn FnMut(&FunctionCall) -> Value + Send),
) -> Result<Reply, GeminiError> {
    let mut build = |continuation: &[Value], with_tools: bool| chat_request(model, context, continuation, with_tools);
    // Only a reply that waits for the results goes on; one that already finished is done. A round
    // that asked the owner questions ends the turn too: the answers are the next message. Going on
    // let Gemini answer its own questions (the real run of 2026-11-09: six questions, then five
    // "decisions" from its own recommendations and a proposal to write the PRD, all before the
    // owner had answered anything).
    let go_on = |streamed: &Streamed, _outcomes: &[Value]| {
        streamed.status == "requires_action" && !streamed.calls.iter().any(|call| crate::calls::ends_turn(&call.name))
    };
    rounds(gemini, key, &mut build, &go_on, cancel, on_text, on_call).await
}

/// The rounds of one reply, whatever it is for (Studio's chat, Lentera's bar): `build` makes each
/// request from the steps and results so far (and whether functions may be offered), every
/// function call goes to `on_call`, and `go_on` decides after a round with calls whether Gemini
/// hears the results and writes on.
pub async fn rounds(
    gemini: &Gemini,
    key: &ApiKey,
    build: &mut (dyn FnMut(&[Value], bool) -> Value + Send),
    go_on: &(dyn Fn(&Streamed, &[Value]) -> bool + Send + Sync),
    cancel: &mut Cancel,
    on_text: &mut (dyn FnMut(&str) + Send),
    on_call: &mut (dyn FnMut(&FunctionCall) -> Value + Send),
) -> Result<Reply, GeminiError> {
    let mut out = Reply::default();
    let mut continuation: Vec<Value> = Vec::new();
    let mut with_tools = true;
    let mut round = 0;
    while round < ROUNDS_MAX {
        let body = build(&continuation, with_tools);
        // Paragraphs of later rounds start on their own line.
        let mut first_text = !out.text.is_empty();
        let mut forward = |piece: &str| {
            if first_text {
                first_text = false;
                out_text_push(piece, on_text, true);
            } else {
                out_text_push(piece, on_text, false);
            }
        };
        let streamed = match gemini.stream(key, &body, cancel, &mut forward).await {
            Ok(streamed) => streamed,
            // Google refused the request itself before anything was said: once more without
            // functions, so the owner still gets an answer.
            Err(error) if error.kind == FailKind::BadRequest && with_tools && round == 0 && out.text.is_empty() => {
                with_tools = false;
                out.without_tools = true;
                continue;
            }
            Err(error) => return Err(error),
        };
        if !out.text.is_empty() && !streamed.text.is_empty() {
            out.text.push_str("\n\n");
        }
        out.text.push_str(&streamed.text);
        out.input_tokens = out.input_tokens.saturating_add(streamed.input_tokens);
        out.output_tokens = out.output_tokens.saturating_add(streamed.output_tokens);
        if streamed.calls.is_empty() {
            break;
        }
        // Exactly one result for each call, matched by its id and name (Google refuses others).
        let outcomes: Vec<Value> = streamed.calls.iter().map(&mut *on_call).collect();
        let results: Vec<Value> = streamed
            .calls
            .iter()
            .zip(&outcomes)
            .map(|(call, outcome)| json!({ "type": "function_result", "name": call.name, "call_id": call.id, "result": [{ "type": "text", "text": outcome.to_string() }] }))
            .collect();
        if !go_on(&streamed, &outcomes) {
            break;
        }
        continuation.extend(streamed.steps);
        continuation.extend(results);
        round += 1;
    }
    Ok(out)
}

fn out_text_push(piece: &str, on_text: &mut (dyn FnMut(&str) + Send), new_paragraph: bool) {
    if new_paragraph {
        on_text("\n\n");
    }
    on_text(piece);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(lines: &[Value]) -> String {
        lines.iter().map(|value| format!("event: x\ndata: {value}\n\n")).collect()
    }

    fn read(raw: &str, split: usize) -> (Streamed, String) {
        let mut reader = EventReader::default();
        let mut seen = String::new();
        let mut on_text = |t: &str| seen.push_str(t);
        // Fed in small pieces, as the network would.
        let chars: Vec<char> = raw.chars().collect();
        for piece in chars.chunks(split) {
            reader.feed(&piece.iter().collect::<String>(), &mut on_text).unwrap();
        }
        (reader.close(), seen)
    }

    #[test]
    fn text_streams_and_steps_are_rebuilt_in_pieces() {
        let raw = events(&[
            json!({"interaction": {"id": "v1", "status": "in_progress"}, "event_type": "interaction.created"}),
            json!({"index": 0, "step": {"type": "thought"}, "event_type": "step.start"}),
            json!({"index": 0, "delta": {"type": "thought_signature", "signature": "sig"}, "event_type": "step.delta"}),
            json!({"index": 0, "event_type": "step.stop"}),
            json!({"index": 1, "step": {"type": "model_output"}, "event_type": "step.start"}),
            json!({"index": 1, "delta": {"type": "text", "text": "Halo, "}, "event_type": "step.delta"}),
            json!({"index": 1, "delta": {"type": "text", "text": "aku Gemini. Ä"}, "event_type": "step.delta"}),
            json!({"index": 1, "event_type": "step.stop"}),
            json!({"interaction": {"id": "v1", "status": "completed", "usage": {"total_input_tokens": 11, "total_output_tokens": 90}}, "event_type": "interaction.completed"}),
        ]) + "event: done\ndata: [DONE]\n\n";
        for split in [1, 7, 64, 10_000] {
            let (out, seen) = read(&raw, split);
            assert_eq!(seen, "Halo, aku Gemini. Ä");
            assert_eq!(out.text, seen);
            assert_eq!(out.status, "completed");
            assert_eq!((out.input_tokens, out.output_tokens), (11, 90));
            assert_eq!(out.steps[0], json!({"type": "thought", "signature": "sig"}));
            assert_eq!(out.steps[1]["content"][0]["text"], json!("Halo, aku Gemini. Ä"));
        }
    }

    #[test]
    fn a_function_call_is_rebuilt_from_its_argument_pieces() {
        let raw = events(&[
            json!({"index": 0, "step": {"type": "function_call", "id": "c1", "name": "catat", "arguments": {}}, "event_type": "step.start"}),
            json!({"index": 0, "delta": {"type": "arguments_delta", "arguments": "{\"items\": [{\"kind\": \"decision\","}, "event_type": "step.delta"}),
            json!({"index": 0, "delta": {"type": "arguments_delta", "arguments": " \"topic\": \"Platform\", \"statement\": \"Web\"}]}"}, "event_type": "step.delta"}),
            json!({"index": 0, "event_type": "step.stop"}),
            json!({"interaction_id": "v1", "status": "requires_action", "event_type": "interaction.status_update"}),
        ]);
        let (out, _) = read(&raw, 13);
        assert_eq!(out.status, "requires_action");
        assert_eq!(out.calls.len(), 1);
        assert_eq!(out.calls[0].name, "catat");
        assert_eq!(out.calls[0].arguments["items"][0]["statement"], json!("Web"));
        assert_eq!(out.steps[0]["type"], json!("function_call"));
        assert_eq!(out.steps[0]["id"], json!("c1"));
    }

    #[test]
    fn an_error_in_the_stream_is_a_failure() {
        let raw = events(&[json!({"error": {"message": "Deadline expired", "code": "gateway_timeout"}, "event_type": "error"})]);
        let mut reader = EventReader::default();
        let error = reader.feed(&raw, &mut |_| {}).unwrap_err();
        assert_eq!(error.kind, FailKind::Service);
        let quota = events(&[json!({"error": {"message": "Quota", "code": "quota_exceeded"}, "event_type": "error"})]);
        assert_eq!(EventReader::default().feed(&quota, &mut |_| {}).unwrap_err().kind, FailKind::Quota);
    }

    mod replies {
        use std::sync::{Arc, Mutex};
        use std::time::Duration;

        use axum::http::StatusCode;
        use axum::response::IntoResponse;
        use axum::routing::post;
        use axum::{Json, Router};

        use super::*;
        use crate::client::tests::{key, serve};
        use crate::prompt::{Role, Turn};

        fn context() -> ChatContext {
            ChatContext {
                project_name: "Absensi".into(),
                idea: "absen".into(),
                stage: "explore".into(),
                ledger: vec![],
                summary: String::new(),
                stack: String::new(),
                history: vec![Turn { role: Role::Owner, text: "Halo".into() }],
            }
        }

        fn sse(lines: &[Value]) -> axum::response::Response {
            ([("content-type", "text/event-stream")], events(lines)).into_response()
        }

        fn text_stream(text: &str) -> Vec<Value> {
            vec![
                json!({"index": 0, "step": {"type": "model_output"}, "event_type": "step.start"}),
                json!({"index": 0, "delta": {"type": "text", "text": text}, "event_type": "step.delta"}),
                json!({"index": 0, "event_type": "step.stop"}),
                json!({"interaction": {"status": "completed", "usage": {"total_input_tokens": 5, "total_output_tokens": 3}}, "event_type": "interaction.completed"}),
            ]
        }

        #[tokio::test]
        async fn a_reply_with_a_function_call_continues_with_the_steps_and_the_result() {
            let seen: Arc<Mutex<Vec<Value>>> = Arc::default();
            let bodies = seen.clone();
            let router = Router::new().route(
                "/v1beta/interactions",
                post(move |Json(body): Json<Value>| {
                    let bodies = bodies.clone();
                    async move {
                        bodies.lock().unwrap().push(body.clone());
                        let input = body["input"].as_array().unwrap();
                        if input.iter().any(|s| s["type"] == "function_result") {
                            sse(&text_stream("Sudah aku catat."))
                        } else {
                            sse(&[
                                json!({"index": 0, "step": {"type": "thought"}, "event_type": "step.start"}),
                                json!({"index": 0, "delta": {"type": "thought_signature", "signature": "s1"}, "event_type": "step.delta"}),
                                json!({"index": 0, "event_type": "step.stop"}),
                                json!({"index": 1, "step": {"type": "model_output"}, "event_type": "step.start"}),
                                json!({"index": 1, "delta": {"type": "text", "text": "Baik."}, "event_type": "step.delta"}),
                                json!({"index": 1, "event_type": "step.stop"}),
                                json!({"index": 2, "step": {"type": "function_call", "id": "c1", "name": "catat", "arguments": {}}, "event_type": "step.start"}),
                                json!({"index": 2, "delta": {"type": "arguments_delta", "arguments": "{\"items\":[]}"}, "event_type": "step.delta"}),
                                json!({"index": 2, "event_type": "step.stop"}),
                                json!({"interaction": {"status": "requires_action", "usage": {"total_input_tokens": 10, "total_output_tokens": 4}}, "event_type": "interaction.completed"}),
                            ])
                        }
                    }
                }),
            );
            let base = serve(router).await;
            let gemini = Gemini::new(&base, Duration::from_secs(5)).unwrap();
            let (_stop, mut cancel) = watch::channel(false);
            let mut streamed = String::new();
            let mut calls = Vec::new();
            let reply = reply(&gemini, &key(), "m", &context(), &mut cancel, &mut |t| streamed.push_str(t), &mut |call| {
                calls.push(call.name.clone());
                json!({"ok": true})
            })
            .await
            .unwrap();
            assert_eq!(reply.text, "Baik.\n\nSudah aku catat.");
            assert_eq!(streamed, reply.text, "what streamed is what is kept");
            assert_eq!((reply.input_tokens, reply.output_tokens), (15, 7));
            assert_eq!(calls, ["catat"]);
            let bodies = seen.lock().unwrap();
            assert_eq!(bodies.len(), 2);
            let second: Vec<Value> = bodies[1]["input"].as_array().unwrap().clone();
            let types: Vec<&str> = second.iter().map(|s| s["type"].as_str().unwrap()).collect();
            assert_eq!(types, ["user_input", "thought", "model_output", "function_call", "function_result"]);
            assert_eq!(second[1]["signature"], json!("s1"), "the thought goes back exactly");
            assert_eq!(second[4]["call_id"], json!("c1"));
            assert_eq!(bodies[1]["store"], json!(false));
        }

        #[tokio::test]
        async fn a_reply_that_asks_ends_its_turn_and_waits_for_the_owner() {
            // Gemini asks, then waits for the result; continuing would let it answer itself.
            let seen: Arc<Mutex<usize>> = Arc::default();
            let requests = seen.clone();
            let router = Router::new().route(
                "/v1beta/interactions",
                post(move || {
                    let requests = requests.clone();
                    async move {
                        *requests.lock().unwrap() += 1;
                        sse(&[
                            json!({"index": 0, "step": {"type": "function_call", "id": "c1", "name": "catat", "arguments": {}}, "event_type": "step.start"}),
                            json!({"index": 0, "delta": {"type": "arguments_delta", "arguments": "{\"items\":[]}"}, "event_type": "step.delta"}),
                            json!({"index": 0, "event_type": "step.stop"}),
                            json!({"index": 1, "step": {"type": "function_call", "id": "c2", "name": "tanya", "arguments": {}}, "event_type": "step.start"}),
                            json!({"index": 1, "delta": {"type": "arguments_delta", "arguments": "{\"questions\":[]}"}, "event_type": "step.delta"}),
                            json!({"index": 1, "event_type": "step.stop"}),
                            json!({"interaction": {"status": "requires_action", "usage": {"total_input_tokens": 10, "total_output_tokens": 4}}, "event_type": "interaction.completed"}),
                        ])
                    }
                }),
            );
            let base = serve(router).await;
            let gemini = Gemini::new(&base, Duration::from_secs(5)).unwrap();
            let (_stop, mut cancel) = watch::channel(false);
            let mut calls = Vec::new();
            let reply = reply(&gemini, &key(), "m", &context(), &mut cancel, &mut |_| {}, &mut |call| {
                calls.push(call.name.clone());
                json!({"ok": true})
            })
            .await
            .unwrap();
            assert_eq!(calls, ["catat", "tanya"], "every call of the round is still applied");
            assert_eq!(*seen.lock().unwrap(), 1, "no second request after the questions");
            assert_eq!(reply.text, "");
        }

        #[tokio::test]
        async fn a_refused_request_is_tried_once_more_without_functions() {
            let router = Router::new().route(
                "/v1beta/interactions",
                post(|Json(body): Json<Value>| async move {
                    if body.get("tools").is_some() {
                        (StatusCode::BAD_REQUEST, Json(json!({"error": {"code": "invalid_request", "message": "tools"}}))).into_response()
                    } else {
                        sse(&text_stream("Tanpa fungsi."))
                    }
                }),
            );
            let base = serve(router).await;
            let gemini = Gemini::new(&base, Duration::from_secs(5)).unwrap();
            let (_stop, mut cancel) = watch::channel(false);
            let reply = reply(&gemini, &key(), "m", &context(), &mut cancel, &mut |_| {}, &mut |_| json!({})).await.unwrap();
            assert_eq!(reply.text, "Tanpa fungsi.");
            assert!(reply.without_tools);
        }

        #[tokio::test]
        async fn hentikan_ends_a_reply_at_once() {
            let router = Router::new().route(
                "/v1beta/interactions",
                post(|| async {
                    // Says one thing, then nothing for a long time.
                    let first = events(&[
                        json!({"index": 0, "step": {"type": "model_output"}, "event_type": "step.start"}),
                        json!({"index": 0, "delta": {"type": "text", "text": "Separuh"}, "event_type": "step.delta"}),
                    ]);
                    let stream = futures_util::stream::iter(vec![Ok::<_, std::io::Error>(first)])
                        .chain(futures_util::stream::once(async {
                            tokio::time::sleep(Duration::from_secs(30)).await;
                            Ok(String::new())
                        }));
                    ([("content-type", "text/event-stream")], axum::body::Body::from_stream(stream)).into_response()
                }),
            );
            let base = serve(router).await;
            let gemini = Gemini::new(&base, Duration::from_secs(60)).unwrap();
            let (stop, mut cancel) = watch::channel(false);
            let streamed = Arc::new(Mutex::new(String::new()));
            let sink = streamed.clone();
            let started = std::time::Instant::now();
            let task = tokio::spawn(async move {
                reply(&gemini, &key(), "m", &context(), &mut cancel, &mut |t| sink.lock().unwrap().push_str(t), &mut |_| json!({})).await
            });
            while streamed.lock().unwrap().is_empty() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            stop.send(true).unwrap();
            let error = task.await.unwrap().unwrap_err();
            assert_eq!(error.kind, FailKind::Cancelled);
            assert_eq!(*streamed.lock().unwrap(), "Separuh", "what arrived before stays with the caller");
            assert!(started.elapsed() < Duration::from_secs(5));
        }
    }

    #[test]
    fn comments_unknown_events_and_bad_lines_are_ignored() {
        let raw = ": keep-alive\n\nevent: x\ndata: not json\n\n".to_string()
            + &events(&[json!({"event_type": "something.new"}), json!({"index": 3, "delta": {"type": "text", "text": "tanpa awal"}, "event_type": "step.delta"})]);
        let (out, seen) = read(&raw, 5);
        assert_eq!(seen, "tanpa awal", "a delta without its start is still the owner's text");
        assert_eq!(out.steps.len(), 1);
    }
}
