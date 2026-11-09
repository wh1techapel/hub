//! The functions Gemini may call in a chat reply, checked before anything is applied.
//!
//! Gemini's arguments are read into typed values; what does not fit (an unknown kind, a missing
//! field) is refused with a reason Gemini receives as the function's result. What is merely too
//! much (a ninth question, a sixth option, a second recommendation) is trimmed, and an unknown
//! part of the idea is left unplaced, since the owner still benefits from the rest. Lengths are
//! limited here because Gemini ignores length limits in schemas.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::stream::FunctionCall;

const NOTES_MAX: usize = 10;
/// One set of essential questions, grouped by part (R2): enough to cover an idea in one round.
const QUESTIONS_MAX: usize = 8;
const OPTIONS_MAX: usize = 5;
const OPEN_MAX: usize = 10;
/// The parts of an idea Studio's first stage makes clear (`studio_ledger.area`).
pub const AREAS: &[&str] = &["problem", "users", "features", "constraints", "success"];

/// A known part, or `None` (Gemini's "other", a typo, or nothing).
fn area(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|a| AREAS.contains(a)).map(str::to_string)
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Note {
    pub kind: String,
    pub topic: String,
    pub statement: String,
    #[serde(default)]
    pub rationale: String,
    /// The part of the idea it settles (one of `AREAS`), if any.
    #[serde(default)]
    pub area: Option<String>,
    /// The code of the item it replaces ("C3").
    #[serde(default)]
    pub replaces: Option<String>,
}

/// One answer the owner can pick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Choice {
    pub label: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub recommended: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Question {
    pub text: String,
    #[serde(default)]
    pub why: String,
    #[serde(default)]
    pub options: Vec<Choice>,
    /// The part of the idea it asks about (one of `AREAS`), empty when none: the owner sees the
    /// set grouped by it. Questions stored before R2 have none.
    #[serde(default)]
    pub area: String,
    /// More than one option may be chosen (which features, which devices).
    #[serde(default)]
    pub multiple: bool,
}

/// Gemini thinks the conversation is ready for the next stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StageProposal {
    pub next: String,

    pub ready_because: String,
    #[serde(default)]
    pub still_open: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    Notes(Vec<Note>),
    Questions(Vec<Question>),
    Stage(StageProposal),
}

#[derive(Deserialize)]
struct NotesArgs {
    items: Vec<Note>,
}

#[derive(Deserialize)]
struct QuestionsArgs {
    questions: Vec<Question>,
}

#[derive(Deserialize)]
struct StageArgs {
    next: String,
    ready_because: String,
    #[serde(default)]
    still_open: Vec<String>,
}

fn clip(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max { text.to_string() } else { text.chars().take(max).collect() }
}

fn required(text: &str, max: usize, what: &str) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err(format!("{what} kosong"));
    }
    if text.chars().count() > max {
        return Err(format!("{what} lebih dari {max} karakter"));
    }
    Ok(text.to_string())
}

/// Check a call's arguments. `Err` carries the reason Gemini is told.
pub fn read(call: &FunctionCall) -> Result<Call, String> {
    let args = call.arguments.clone();
    match call.name.as_str() {
        "catat" => {
            let args: NotesArgs = serde_json::from_value(args).map_err(|e| format!("argumen tidak sesuai: {e}"))?;
            if args.items.is_empty() {
                return Err("tidak ada catatan".into());
            }
            if args.items.len() > NOTES_MAX {
                return Err(format!("paling banyak {NOTES_MAX} catatan sekali panggil"));
            }
            let mut notes = Vec::new();
            for item in args.items {
                if !["decision", "assumption", "question", "constraint"].contains(&item.kind.as_str()) {
                    return Err(format!("jenis catatan tidak dikenal: {}", item.kind));
                }
                notes.push(Note {
                    topic: required(&item.topic, 120, "topic")?,
                    statement: required(&item.statement, 2000, "statement")?,
                    rationale: clip(&item.rationale, 2000),
                    area: area(item.area.as_deref()),
                    replaces: item.replaces.map(|code| code.trim().to_string()).filter(|code| !code.is_empty()),
                    kind: item.kind,
                });
            }
            Ok(Call::Notes(notes))
        }
        "tanya" => {
            let args: QuestionsArgs = serde_json::from_value(args).map_err(|e| format!("argumen tidak sesuai: {e}"))?;
            let mut questions = Vec::new();
            for question in args.questions.into_iter().take(QUESTIONS_MAX) {
                let mut options: Vec<Choice> = question
                    .options
                    .into_iter()
                    .filter(|o| !o.label.trim().is_empty())
                    .take(OPTIONS_MAX)
                    .map(|o| Choice { label: clip(&o.label, 120), detail: clip(&o.detail, 300), recommended: o.recommended })
                    .collect();
                // One option is no choice; one recommendation at most.
                if options.len() < 2 {
                    options.clear();
                }
                let mut recommended = false;
                for option in &mut options {
                    option.recommended = option.recommended && !recommended;
                    recommended |= option.recommended;
                }
                questions.push(Question {
                    text: required(&question.text, 500, "text")?,
                    why: clip(&question.why, 300),
                    multiple: question.multiple && options.len() >= 2,
                    area: area(Some(&question.area)).unwrap_or_default(),
                    options,
                });
            }
            if questions.is_empty() {
                return Err("tidak ada pertanyaan".into());
            }
            Ok(Call::Questions(questions))
        }
        "usul_tahap" => {
            let args: StageArgs = serde_json::from_value(args).map_err(|e| format!("argumen tidak sesuai: {e}"))?;
            // Stage 1 is one conversation since R2: the only step Gemini proposes is out of it.
            if args.next != "prd" {
                return Err(format!("tahap tidak dikenal: {}", args.next));
            }
            Ok(Call::Stage(StageProposal {
                next: args.next,
                ready_because: required(&args.ready_because, 1000, "ready_because")?,
                still_open: args.still_open.iter().map(|s| clip(s, 200)).filter(|s| !s.is_empty()).take(OPEN_MAX).collect(),
            }))
        }
        other => Err(format!("fungsi tidak dikenal: {other}")),
    }
}

/// Whether a call ends Gemini's turn: after `tanya` the owner answers first, so the reply does
/// not continue (stream::reply).
pub fn ends_turn(name: &str) -> bool {
    name == "tanya"
}

/// A reply's questions, into its parts. A stage proposal in the same reply is dropped: the owner
/// cannot be asked to move on while questions wait for answers.
pub fn put_questions(parts: &mut Value, questions: Vec<Question>) -> String {
    parts["questions"] = serde_json::to_value(questions).unwrap_or(Value::Null);
    if let Some(parts) = parts.as_object_mut() {
        parts.remove("stage");
    }
    "pertanyaan ditampilkan kepada pemilik; giliranmu selesai sampai pemilik menjawab".into()
}

/// A stage proposal, into a reply's parts, unless the reply asked questions.
pub fn put_stage(parts: &mut Value, proposal: StageProposal) -> Result<String, String> {
    if parts.get("questions").is_some_and(|q| !q.is_null()) {
        return Err("jangan usul tahap selagi pertanyaanmu belum dijawab pemilik".into());
    }
    parts["stage"] = serde_json::to_value(proposal).unwrap_or(Value::Null);
    Ok("usulan tahap ditampilkan; pemilik yang memutuskan".into())
}

/// What Gemini is told after a call: done, or why not.
pub fn result(outcome: &Result<String, String>) -> Value {
    match outcome {
        Ok(note) => serde_json::json!({ "ok": true, "note": note }),
        Err(reason) => serde_json::json!({ "ok": false, "error": reason }),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn call(name: &str, arguments: Value) -> FunctionCall {
        FunctionCall { id: "c".into(), name: name.into(), arguments }
    }

    #[test]
    fn notes_are_checked_and_kinds_must_be_known() {
        let ok = read(&call("catat", json!({"items": [{"kind": "decision", "topic": " Platform ", "statement": "Web", "replaces": "C2"}]}))).unwrap();
        assert_eq!(
            ok,
            Call::Notes(vec![Note { kind: "decision".into(), topic: "Platform".into(), statement: "Web".into(), rationale: String::new(), area: None, replaces: Some("C2".into()) }])
        );
        // A known part is kept; an unknown one leaves the note unplaced rather than refused.
        let Call::Notes(placed) = read(&call("catat", json!({"items": [
            {"kind": "constraint", "topic": "Waktu", "statement": "Dua minggu", "area": "constraints"},
            {"kind": "decision", "topic": "Warna", "statement": "Biru", "area": "other"}
        ]}))).unwrap() else { panic!() };
        assert_eq!(placed[0].area.as_deref(), Some("constraints"));
        assert_eq!(placed[1].area, None);
        assert!(read(&call("catat", json!({"items": [{"kind": "wish", "topic": "x", "statement": "y"}]}))).is_err());
        assert!(read(&call("catat", json!({"items": [{"kind": "decision", "topic": "", "statement": "y"}]}))).is_err());
        assert!(read(&call("catat", json!({"items": []}))).is_err());
        assert!(read(&call("catat", json!({"notes": []}))).is_err());
        let many: Vec<Value> = (0..11).map(|i| json!({"kind": "question", "topic": format!("t{i}"), "statement": "s"})).collect();
        assert!(read(&call("catat", json!({ "items": many }))).is_err());
    }

    #[test]
    fn a_question_set_is_trimmed_to_eight_grouped_by_part_with_one_recommendation() {
        let options = json!([
            {"label": "A", "recommended": true}, {"label": "B", "recommended": true},
            {"label": "C"}, {"label": "D"}, {"label": "E"}, {"label": "F"}
        ]);
        let mut list = vec![
            json!({"text": "Satu?", "why": "penting", "options": options, "area": "users"}),
            json!({"text": "Dua?", "options": [{"label": "Hanya satu"}], "area": "features", "multiple": true}),
            json!({"text": "Tiga?", "area": "mood", "multiple": true, "options": [{"label": "X"}, {"label": "Y"}]}),
        ];
        list.extend((4..=10).map(|n| json!({"text": format!("Ke-{n}?")})));
        let Call::Questions(questions) = read(&call("tanya", json!({ "questions": list }))).unwrap() else { panic!() };
        assert_eq!(questions.len(), 8);
        assert_eq!(questions[0].options.len(), 5);
        assert_eq!(questions[0].options.iter().filter(|o| o.recommended).count(), 1);
        assert!(questions[0].options[0].recommended);
        assert_eq!(questions[0].area, "users");
        assert!(questions[1].options.is_empty(), "one option is no choice");
        assert!(!questions[1].multiple, "nothing to choose several of");
        assert_eq!(questions[2].area, "", "an unknown part is left out");
        assert!(questions[2].multiple);
        // Stored before R2: no part, one answer.
        let old: Question = serde_json::from_value(json!({"text": "Lama?", "why": "", "options": []})).unwrap();
        assert_eq!((old.area.as_str(), old.multiple), ("", false));
    }

    #[test]
    fn a_stage_proposal_names_a_known_next_stage() {
        let Call::Stage(stage) = read(&call("usul_tahap", json!({"next": "prd", "ready_because": "Cukup", "still_open": ["Hosting", " "]}))).unwrap() else { panic!() };
        assert_eq!(stage.still_open, ["Hosting"]);
        assert!(read(&call("usul_tahap", json!({"next": "build", "ready_because": "x"}))).is_err());
        assert!(read(&call("usul_tahap", json!({"next": "clarify", "ready_because": "x"}))).is_err(), "stage 1 is one conversation");
        assert!(read(&call("hapus_semua", json!({}))).is_err());
    }

    #[test]
    fn questions_end_the_turn_and_no_stage_is_proposed_beside_them() {
        assert!(ends_turn("tanya"));
        assert!(!ends_turn("catat") && !ends_turn("usul_tahap"));
        let question = || Question { text: "Siapa penggunanya?".into(), why: String::new(), options: vec![], area: "users".into(), multiple: false };
        let proposal = || StageProposal { next: "prd".into(), ready_because: "Cukup".into(), still_open: vec![] };

        // Asked first: the proposal that follows is refused, with a reason Gemini hears.
        let mut parts = json!({});
        put_questions(&mut parts, vec![question()]);
        assert!(put_stage(&mut parts, proposal()).is_err());
        assert!(parts.get("stage").is_none());

        // Proposed first, asked after in the same reply: the proposal is dropped.
        let mut parts = json!({});
        put_stage(&mut parts, proposal()).unwrap();
        put_questions(&mut parts, vec![question()]);
        assert!(parts.get("stage").is_none());
        assert_eq!(parts["questions"][0]["text"], json!("Siapa penggunanya?"));

        // Without questions a proposal stands.
        let mut parts = json!({});
        put_stage(&mut parts, proposal()).unwrap();
        assert_eq!(parts["stage"]["next"], json!("prd"));
    }

    #[test]
    fn gemini_hears_why_a_call_was_refused() {
        assert_eq!(result(&Err("jenis tidak dikenal".into())), json!({"ok": false, "error": "jenis tidak dikenal"}));
        assert_eq!(result(&Ok("2 dicatat".into()))["ok"], json!(true));
    }
}
