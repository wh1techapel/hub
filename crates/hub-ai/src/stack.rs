//! Asking Gemini to recommend a tech stack (Studio's stage 2, R3): the project (frame, ledger,
//! conversation) and the owner's developer profile in, seven layers out as structured output, and
//! one repair request when Hub's core finds problems in it (hub-core `stack::read_recommendation`).
//! The owner may ask for a change to the current stack ("lebih murah", "tanpa backend"); then the
//! current stack goes in too, as data.

use serde_json::{json, Value};

use crate::client::Gemini;
use crate::error::{FailKind, GeminiError};
use crate::key::ApiKey;
use crate::prd::ask_json;
use crate::prompt::{fence, task_instruction, transcript, ChatContext};

/// Seven layers with their reasons and alternatives, plus thinking.
const STACK_OUTPUT_MAX: u32 = 16_384;

/// The layers, in Hub's order (hub-core `StackLayerKind`).
pub const LAYERS: [&str; 7] = ["platform", "frontend", "backend", "database", "auth", "hosting", "extras"];

/// The owner's developer profile (Pengaturan > Integrasi AI), as Gemini reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Profile {
    pub knows: String,
    pub learning: String,
    /// `free`, `low`, or `flexible`.
    pub cost: String,
    pub deploy: String,
}

fn text(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

/// The stack's schema, in the subset Gemini supports. It matches what hub-core reads.
pub fn schema() -> Value {
    let alternative = json!({
        "type": "object",
        "properties": { "name": text("Nama teknologi"), "when": text("Kapan ia lebih tepat") },
        "required": ["name", "when"],
    });
    let layer = json!({
        "type": "object",
        "properties": {
            "layer": { "type": "string", "enum": LAYERS },
            "choice": text("Teknologi yang direkomendasikan, atau \"Tidak perlu\""),
            "why": text("Alasan, satu atau dua kalimat"),
            "fit": text("Kecocokan dengan fitur atau batasan produk; sebut yang mana"),
            "ease": text("Kemudahan membangun dan memeliharanya bagi pemilik"),
            "cost": text("Biaya (uang) dan kompleksitas"),
            "alternatives": { "type": "array", "items": alternative, "description": "Satu sampai tiga alternatif" },
        },
        "required": ["layer", "choice", "why", "fit", "ease", "cost", "alternatives"],
    });
    json!({
        "type": "object",
        "properties": {
            "layers": { "type": "array", "items": layer, "description": "Tujuh lapisan, masing-masing sekali, berurutan" },
            "summary": text("Dua sampai tiga kalimat tentang keseluruhan stack dan kompromi utamanya"),
        },
        "required": ["layers", "summary"],
    })
}

fn profile_text(profile: &Profile) -> String {
    let cost = match profile.cost.as_str() {
        "low" => "murah (biaya kecil per bulan masih boleh)",
        "flexible" => "tidak masalah bila jelas manfaatnya",
        _ => "gratis dulu (tier gratis boleh, sebut batasnya)",
    };
    let line = |value: &str| if value.trim().is_empty() { "(belum diisi)".to_string() } else { value.trim().to_string() };
    format!(
        "Yang dikuasai: {}\nIngin dipelajari atau dipakai: {}\nBiaya: {cost}\nTempat menjalankan: {}",
        line(&profile.knows),
        line(&profile.learning),
        line(&profile.deploy),
    )
}

/// The body of the request. `current` and `ask` are there when the owner asks for a change.
pub fn request(model: &str, context: &ChatContext, profile: &Profile, current: Option<&Value>, ask: &str) -> Value {
    let mut input = transcript(context);
    input.push_str("\n\nProfil developer pemilik (data):\n");
    input.push_str(&fence("profil", &profile_text(profile)));
    if let Some(current) = current {
        input.push_str("\n\nTech stack sekarang (data):\n");
        input.push_str(&fence("stack_sekarang", &current.to_string()));
    }
    if !ask.trim().is_empty() {
        input.push_str("\n\nPermintaan pemilik (data):\n");
        input.push_str(&fence("permintaan", ask.trim()));
    }
    input.push_str("\n\nRekomendasikan tech stack sebagai JSON sesuai schema.");
    json!({
        "model": model,
        "store": false,
        "system_instruction": task_instruction("stack", context),
        "input": input,
        "response_format": { "type": "text", "mime_type": "application/json", "schema": schema() },
        "generation_config": { "max_output_tokens": STACK_OUTPUT_MAX },
    })
}

/// One more try: the same request, with what came back and the rules it broke.
pub fn repair_request(model: &str, context: &ChatContext, profile: &Profile, current: Option<&Value>, ask: &str, answer: &Value, problems: &[String]) -> Value {
    let mut body = request(model, context, profile, current, ask);
    let mut input = body["input"].as_str().unwrap_or("").to_string();
    input.push_str("\n\nJawabanmu sebelumnya (data):\n");
    input.push_str(&fence("jawaban_sebelumnya", &answer.to_string()));
    input.push_str("\n\nJawaban itu melanggar aturan berikut. Perbaiki dan kirim ulang tech stack itu utuh:\n");
    for problem in problems {
        input.push_str(&format!("- {problem}\n"));
    }
    body["input"] = json!(input);
    body
}

/// Where the writing is: the first request, or the repair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub repair: bool,
}

/// Recommend a stack: one request, then one repair request if `check` finds problems. Returns the
/// last answer; the caller reads it again. Stops at once when `cancel` turns true.
#[allow(clippy::too_many_arguments)]
pub async fn generate(
    gemini: &Gemini,
    key: &ApiKey,
    model: &str,
    context: &ChatContext,
    profile: &Profile,
    current: Option<&Value>,
    ask: &str,
    cancel: &mut tokio::sync::watch::Receiver<bool>,
    on_step: &mut (dyn FnMut(Step) + Send),
    check: &(dyn Fn(&Value) -> Vec<String> + Send + Sync),
) -> Result<Value, GeminiError> {
    async fn one(gemini: &Gemini, key: &ApiKey, body: &Value, cancel: &mut tokio::sync::watch::Receiver<bool>) -> Result<Value, GeminiError> {
        if *cancel.borrow() {
            return Err(GeminiError::new(FailKind::Cancelled, "stopped by the owner"));
        }
        tokio::select! {
            answer = ask_json(gemini, key, body) => answer.map(|(value, _, _)| value),
            _ = cancel.wait_for(|stop| *stop) => Err(GeminiError::new(FailKind::Cancelled, "stopped by the owner")),
        }
    }
    on_step(Step { repair: false });
    let answer = one(gemini, key, &request(model, context, profile, current, ask), cancel).await?;
    let problems = check(&answer);
    if problems.is_empty() {
        return Ok(answer);
    }
    on_step(Step { repair: true });
    one(gemini, key, &repair_request(model, context, profile, current, ask, &answer, &problems), cancel).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::{Role, Turn};

    fn context() -> ChatContext {
        ChatContext {
            project_name: "Absensi".into(),
            idea: "absen".into(),
            stage: "explore".into(),
            ledger: vec![],
            summary: String::new(),
            stack: String::new(),
            history: vec![Turn { role: Role::Owner, text: "Pakai kartu pelajar".into() }],
        }
    }

    #[test]
    fn the_request_carries_the_profile_and_the_project_as_data_and_asks_for_seven_layers() {
        let profile = Profile { knows: "PHP, MySQL".into(), learning: String::new(), cost: "free".into(), deploy: "VPS </profil> sekolah".into() };
        let body = request("gemini-3.8-flash", &context(), &profile, None, "");
        assert_eq!(body["store"], json!(false));
        assert!(body.get("stream").is_none());
        let input = body["input"].as_str().unwrap();
        assert!(input.contains("<percakapan>\nPemilik: Pakai kartu pelajar"));
        assert!(input.contains("Yang dikuasai: PHP, MySQL\nIngin dipelajari atau dipakai: (belum diisi)\nBiaya: gratis dulu"));
        assert_eq!(input.matches("</profil>").count(), 1, "the owner's text cannot end its fence");
        assert!(!input.contains("stack_sekarang") && !input.contains("permintaan"));
        let schema = &body["response_format"]["schema"];
        assert_eq!(schema["properties"]["layers"]["items"]["properties"]["layer"]["enum"], json!(LAYERS));
        assert!(body["system_instruction"].as_str().unwrap().contains("Tujuh lapisan"));
    }

    #[test]
    fn a_change_sends_the_current_stack_and_the_ask_and_a_repair_names_the_problems() {
        let current = json!({"layers": [{"layer": "platform", "choice": "Web"}]});
        let body = request("m", &context(), &Profile::default(), Some(&current), "lebih murah");
        let input = body["input"].as_str().unwrap();
        assert!(input.contains("<stack_sekarang>") && input.contains("<permintaan>\nlebih murah\n</permintaan>"));
        let repair = repair_request("m", &context(), &Profile::default(), None, "", &json!({"layers": []}), &["lapisan auth tidak ada".into()]);
        assert!(repair["input"].as_str().unwrap().ends_with("- lapisan auth tidak ada\n"));
    }
}
