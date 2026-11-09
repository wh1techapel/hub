//! Does the real Gemini accept every schema Studio sends? Run by the owner (or with their leave),
//! with their own key:
//!
//!   cargo run -p hub-ai --bin schema-probe -- gemini-3.5-flash gemini-3.8-flash
//!
//! Google refuses a schema whose constraint "has too many states for serving" with a 400, and
//! only the real service says so: the fake Gemini of the tests accepts anything. A PRD written
//! with the U5 schemas failed that way on 2026-11-09. For each model given (default
//! `gemini-3.8-flash`) this sends each structured request's schema (the PRD's three parts, the
//! tech stack, the roadmap) with a one-line input, low thinking, and a small output limit, and
//! prints whether Google accepted it, with Google's reason when it did not. An answer cut short is
//! fine: only acceptance is checked. It uses a few hundred tokens of the key's quota per schema.
//! The key is read from Windows Credential Manager (`Hub/gemini`) or `GEMINI_API_KEY`; nothing is
//! stored, here or at Google (`store: false`).

use std::time::Duration;

use hub_ai::{ApiKey, FailKind, Gemini};
use serde_json::{json, Value};

fn key() -> Option<ApiKey> {
    if let Ok(text) = std::env::var("GEMINI_API_KEY") {
        return ApiKey::parse(&text).ok();
    }
    #[cfg(windows)]
    {
        use hub_ai::KeyStore;
        hub_ai::key::CredentialStore::new("Hub/gemini").get().ok().flatten()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn schemas() -> Vec<(String, Value)> {
    let mut all: Vec<(String, Value)> = hub_ai::prd::PARTS
        .iter()
        .enumerate()
        .map(|(index, sections)| {
            let properties: serde_json::Map<String, Value> = sections.iter().map(|s| (s.to_string(), hub_ai::prd::section_schema(s))).collect();
            (format!("prd part {}", index + 1), json!({ "type": "object", "properties": properties, "required": sections }))
        })
        .collect();
    all.push(("stack".into(), hub_ai::stack::schema()));
    all.push(("roadmap".into(), hub_ai::roadmap::schema()));
    all
}

fn main() {
    let Some(key) = key() else {
        eprintln!("No key: save one in Hub (Pengaturan > Integrasi AI) or set GEMINI_API_KEY.");
        std::process::exit(2);
    };
    let mut models: Vec<String> = std::env::args().skip(1).collect();
    if models.is_empty() {
        models.push("gemini-3.8-flash".into());
    }
    let gemini = Gemini::google().expect("the HTTPS client builds");
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime");
    let mut refused = 0;
    // `PROBE_ONLY=part 2` probes only the schemas whose name holds it (the free tier allows few
    // requests a minute and a day).
    let only = std::env::var("PROBE_ONLY").unwrap_or_default();
    for model in &models {
        for (name, schema) in schemas().into_iter().filter(|(name, _)| name.contains(only.as_str())) {
            let body = json!({
                "model": model,
                "store": false,
                "input": "Uji skema: isi setiap field dengan nilai sangat singkat.",
                "response_format": { "type": "text", "mime_type": "application/json", "schema": schema },
                "generation_config": { "max_output_tokens": 256, "thinking_level": "low" },
            });
            let outcome = runtime.block_on(gemini.generate_with(&key, &body, Duration::from_secs(120)));
            match outcome {
                Ok(_) => println!("ok       {model:<22} {name}"),
                Err(error) if error.kind == FailKind::BadRequest => {
                    refused += 1;
                    println!("REFUSED  {model:<22} {name}: {}", error.message);
                }
                Err(error) => println!("skipped  {model:<22} {name}: {} ({})", error.kind.as_str(), error.message),
            }
        }
    }
    std::process::exit(if refused == 0 { 0 } else { 1 });
}
