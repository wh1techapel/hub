//! Studio's behaviour against the real Gemini, run by the owner with their own key:
//!
//!   cargo run -p hub-ai --bin studio-eval
//!
//! The key is read from Windows Credential Manager (`Hub/gemini`, where Hub keeps it after
//! "Simpan" in Pengaturan), or from the environment variable `GEMINI_API_KEY` if set. The model is
//! `gemini-3.8-flash` unless given as the first argument. Five short ideas are sent as a first
//! message, the way Studio sends them, and each reply is checked for what the instructions promise
//! (docs/studio.md, section 6, and "Revision: three stages": one set of at most eight questions,
//! each naming its part of the idea). It uses some of the key's quota (five replies). Nothing is stored,
//! here or at Google (`store: false`).

use std::time::Duration;

use hub_ai::calls::{self, Call};
use hub_ai::prompt::{ChatContext, Role, Turn};
use hub_ai::stream::{self, FunctionCall};
use hub_ai::{ApiKey, Gemini, KeyStore};
use serde_json::{json, Value};
use tokio::sync::watch;

const IDEAS: &[&str] = &[
    "Aplikasi absensi lab",
    "Aku mau bikin marketplace untuk jual beli barang bekas antar siswa di sekolah",
    "Bot Discord yang mengingatkan tugas kelas",
    "Website portofolio untuk lulusan TJKT",
    "Sistem monitoring jaringan sederhana untuk lab sekolah pakai Raspberry Pi",
];

fn key() -> Option<ApiKey> {
    if let Ok(text) = std::env::var("GEMINI_API_KEY") {
        return ApiKey::parse(&text).ok();
    }
    #[cfg(windows)]
    {
        hub_ai::key::CredentialStore::new("Hub/gemini").get().ok().flatten()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

struct Seen {
    text: String,
    raw_calls: Vec<FunctionCall>,
}

fn check(seen: &Seen) -> Vec<(&'static str, bool, String)> {
    let questions: Vec<&FunctionCall> = seen.raw_calls.iter().filter(|c| c.name == "tanya").collect();
    let raw_count: usize = questions
        .iter()
        .map(|c| c.arguments.get("questions").and_then(Value::as_array).map_or(0, Vec::len))
        .sum();
    let recommendations_ok = questions.iter().all(|c| {
        c.arguments
            .get("questions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .all(|q| q.get("options").and_then(Value::as_array).map_or(0, |o| o.iter().filter(|x| x["recommended"] == json!(true)).count()) <= 1)
    });
    // R2: one set, each question naming its part of the idea, so Hub can show it grouped.
    let raw_questions: Vec<&Value> = questions
        .iter()
        .flat_map(|c| c.arguments.get("questions").and_then(Value::as_array).into_iter().flatten())
        .collect();
    let with_area = raw_questions.iter().filter(|q| q.get("area").and_then(Value::as_str).is_some_and(|a| calls::AREAS.contains(&a))).count();
    let mut parts: Vec<&str> = raw_questions.iter().filter_map(|q| q.get("area").and_then(Value::as_str)).filter(|a| calls::AREAS.contains(a)).collect();
    parts.sort_unstable();
    parts.dedup();
    let valid = seen.raw_calls.iter().all(|c| calls::read(c).is_ok());
    let lower = seen.text.to_lowercase();
    vec![
        ("ada jawaban teks", !seen.text.trim().is_empty(), format!("{} karakter", seen.text.chars().count())),
        (
            "belum menyusun PRD",
            !seen.text.contains("FEAT-") && !lower.contains("kriteria penerimaan") && !lower.contains("acceptance criteria"),
            String::new(),
        ),
        ("bertanya lewat fungsi tanya, dalam satu set", questions.len() == 1, format!("{} panggilan", questions.len())),
        ("paling banyak delapan pertanyaan", raw_count <= 8, format!("{raw_count} pertanyaan")),
        ("setiap pertanyaan menyebut bagian idenya", with_area == raw_count && raw_count > 0, format!("{with_area} dari {raw_count}")),
        ("set mencakup lebih dari satu bagian", parts.len() >= 2, parts.join(", ")),
        ("paling banyak satu rekomendasi per pertanyaan", recommendations_ok, String::new()),
        ("argumen fungsi valid", valid, String::new()),
        ("tanpa em dash", !seen.text.contains('\u{2014}'), String::new()),
    ]
}

fn main() {
    let Some(key) = key() else {
        eprintln!("Tidak ada key: simpan dulu di Hub (Pengaturan > Integrasi AI) atau set GEMINI_API_KEY.");
        std::process::exit(2);
    };
    let model = std::env::args().nth(1).unwrap_or_else(|| "gemini-3.8-flash".into());
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
    let gemini = Gemini::google().expect("client");
    let mut failed = 0;
    for idea in IDEAS {
        println!("\n== {idea}");
        let context = ChatContext {
            project_name: idea.chars().take(60).collect(),
            idea: (*idea).into(),
            stage: "explore".into(),
            ledger: vec![],
            summary: String::new(),
            stack: String::new(),
            history: vec![Turn { role: Role::Owner, text: (*idea).into() }],
        };
        let mut seen = Seen { text: String::new(), raw_calls: Vec::new() };
        let (_stop, mut cancel) = watch::channel(false);
        let outcome = runtime.block_on(async {
            let mut on_text = |_: &str| {};
            let mut calls_seen: Vec<FunctionCall> = Vec::new();
            let mut on_call = |call: &FunctionCall| {
                calls_seen.push(call.clone());
                let result = match calls::read(call) {
                    Ok(Call::Notes(n)) => Ok(format!("{} catatan tersimpan", n.len())),
                    Ok(_) => Ok("ditampilkan kepada pemilik".to_string()),
                    Err(reason) => Err(reason),
                };
                calls::result(&result)
            };
            let reply = tokio::time::timeout(
                Duration::from_secs(180),
                stream::reply(&gemini, &key, &model, &context, &mut cancel, &mut on_text, &mut on_call),
            )
            .await;
            (reply, calls_seen)
        });
        match outcome {
            (Ok(Ok(reply)), calls_seen) => {
                seen.text = reply.text;
                seen.raw_calls = calls_seen;
                if reply.without_tools {
                    println!("  catatan: Google menolak fungsi; dijawab tanpa fungsi");
                }
                for (name, ok, detail) in check(&seen) {
                    if !ok {
                        failed += 1;
                    }
                    println!("  {} {name}{}", if ok { "LULUS" } else { "GAGAL" }, if detail.is_empty() { String::new() } else { format!(" ({detail})") });
                }
                println!("  token: {} masuk, {} keluar", reply.input_tokens, reply.output_tokens);
                let preview: String = seen.text.chars().take(240).collect();
                println!("  awal jawaban: {}", preview.replace('\n', " "));
            }
            (Ok(Err(error)), _) => {
                failed += 1;
                println!("  GAGAL memanggil Gemini: {} ({})", error.kind.as_str(), error.message);
            }
            (Err(_), _) => {
                failed += 1;
                println!("  GAGAL: tidak selesai dalam 3 menit");
            }
        }
    }
    println!("\n{}", if failed == 0 { "Semua pemeriksaan lulus.".to_string() } else { format!("{failed} pemeriksaan gagal.") });
    std::process::exit(if failed == 0 { 0 } else { 1 });
}
