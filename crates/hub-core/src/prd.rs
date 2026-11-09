//! Studio's PRD (docs/studio.md, "PRD"): the document's shape, the rules Hub checks it against
//! (Gemini ignores some schema limits and cannot express others), its versions, and Markdown.
//!
//! One working draft per project changes as the owner edits it; numbered versions never change
//! again except their status (`saved`, `approved`, `superseded`). Gemini's generation and the
//! owner's "Simpan versi" and "Setujui" each freeze the draft into a new number. A draft may hold
//! problems while it is being edited; approval needs none.

use std::collections::{BTreeMap, BTreeSet};

use jiff::Timestamp;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{invalid, Error, Reason, Result};
use crate::ids::{self, new_id};
use crate::activity::{self, Record};
use crate::model::{text_enum, Actor, Change, Entity, Verb};
use crate::studio::StatedBy;
use crate::time::instant;

const NAME_MAX: usize = 120;
const TEXT_MAX: usize = 2000;
const ITEM_MAX: usize = 500;
const LIST_MAX: usize = 40;
const FEATURES_MAX: usize = 60;
const NOTE_MAX: usize = 1000;

text_enum!(
    /// MoSCoW.
    Priority {
        Must => "must",
        Should => "should",
        Could => "could",
        Wont => "wont",
    }
);

text_enum!(
    NfrCategory {
        Performance => "performance",
        Security => "security",
        Privacy => "privacy",
        Reliability => "reliability",
        Scalability => "scalability",
        Compatibility => "compatibility",
        Accessibility => "accessibility",
        Storage => "storage",
        Failure => "failure",
        Offline => "offline",
    }
);

text_enum!(
    /// What a security requirement protects (R4).
    SecurityArea {
        Auth => "auth",
        Access => "access",
        Data => "data",
        Input => "input",
        Secrets => "secrets",
        Transport => "transport",
        Logging => "logging",
        Dependencies => "dependencies",
        Other => "other",
    }
);

text_enum!(
    /// Where a piece of evidence comes from: the owner said it, or nobody did yet.
    EvidenceSource {
        Owner => "owner",
        Unknown => "none",
    }
);

text_enum!(
    /// What a step of the user flow diagram is (U5b).
    FlowNodeKind {
        Start => "start",
        Screen => "screen",
        Action => "action",
        Decision => "decision",
        End => "end",
    }
);

text_enum!(
    PrdStatus {
        Draft => "draft",
        Saved => "saved",
        Approved => "approved",
        Superseded => "superseded",
    }
);

text_enum!(
    /// The parts of a PRD that are edited, revised, and generated one at a time.
    PrdSection {
        Identity => "identity",
        Background => "background",
        Goals => "goals",
        Experience => "experience",
        Flow => "flow",
        Features => "features",
        Nfr => "nfr",
        Technical => "technical",
        Security => "security",
        Delivery => "delivery",
    }
);

// ---------- the document ----------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Identity {
    pub name: String,
    pub summary: String,
    /// Penanggung jawab.
    #[serde(default)]
    pub owner: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Evidence {
    pub statement: String,
    pub source: EvidenceSource,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Assumption {
    /// ASM-01.
    pub id: String,
    pub statement: String,
    #[serde(default)]
    pub validate: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Background {
    pub problem: String,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub target_users: Vec<String>,
    #[serde(default)]
    pub unmet_need: String,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub assumptions: Vec<Assumption>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Metric {
    pub metric: String,
    pub target: String,
    #[serde(default)]
    pub how: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Goals {
    pub primary: String,
    #[serde(default)]
    pub benefits: Vec<String>,
    #[serde(default)]
    pub differentiator: String,
    #[serde(default)]
    pub metrics: Vec<Metric>,
    #[serde(default)]
    pub out_of_scope: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Persona {
    pub name: String,
    #[serde(default)]
    pub need: String,
    #[serde(default)]
    pub context: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Flow {
    pub name: String,
    #[serde(default)]
    pub steps: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Experience {
    #[serde(default)]
    pub personas: Vec<Persona>,
    /// A persona's way through the product, step by step.
    #[serde(default)]
    pub journeys: Vec<Flow>,
    #[serde(default)]
    pub main_flows: Vec<Flow>,
    #[serde(default)]
    pub alternate_flows: Vec<Flow>,
    #[serde(default)]
    pub empty_states: Vec<String>,
    #[serde(default)]
    pub error_states: Vec<String>,
    #[serde(default)]
    pub accessibility: Vec<String>,
}

/// One step of the user flow diagram (U5b): N1, N2, ...
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct FlowNode {
    pub id: String,
    pub kind: FlowNodeKind,
    pub label: String,
    /// The feature that handles this step (FEAT-001), or empty.
    #[serde(default)]
    pub feature: String,
}

/// An arrow of the user flow diagram; a decision's arrows carry their answer ("Ya", "Tidak").
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub label: String,
}

/// The product's main user flow as a diagram Hub draws (U5b), and writes as Mermaid in the
/// Markdown export. PRDs from before U5b have none (empty).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct FlowChart {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub nodes: Vec<FlowNode>,
    #[serde(default)]
    pub edges: Vec<FlowEdge>,
}

impl FlowChart {
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.edges.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Story {
    pub as_a: String,
    pub want: String,
    pub so_that: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Criterion {
    /// AC-001-1.
    pub id: String,
    pub given: String,
    pub when: String,
    pub then: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Feature {
    /// FEAT-001.
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub rationale: String,
    pub priority: Priority,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub story: Story,
    #[serde(default)]
    pub acceptance: Vec<Criterion>,
    #[serde(default)]
    pub tests: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Nfr {
    pub category: NfrCategory,
    pub requirement: String,
    #[serde(default)]
    pub measure: String,
}

/// Technical planning. Since R4 the technologies come from the approved tech stack (`from_stack`),
/// and the rest (architecture, components, data model, integrations) is a proposal built on it, as
/// the code is not read yet. Older PRDs have no stack: everything in them is a proposal.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Technical {
    #[serde(default)]
    pub applicable: bool,
    #[serde(default)]
    pub architecture: String,
    #[serde(default)]
    pub components: Vec<String>,
    #[serde(default)]
    pub data_model: String,
    #[serde(default)]
    pub integrations: Vec<String>,
    #[serde(default)]
    pub technologies: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    /// The technologies are the approved tech stack's, put there by Hub.
    #[serde(default)]
    pub from_stack: bool,
}

/// A security requirement (R4): what must hold, and how it is checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SecurityRequirement {
    /// SEC-01.
    pub id: String,
    pub area: SecurityArea,
    pub requirement: String,
    /// How it is checked (a test, a review, a setting).
    #[serde(default)]
    pub verify: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Risk {
    pub risk: String,
    #[serde(default)]
    pub impact: String,
    #[serde(default)]
    pub mitigation: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct OpenQuestion {
    /// Q-01.
    pub id: String,
    pub question: String,
    #[serde(default)]
    pub owner: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Delivery {
    /// Feature ids in the MVP.
    #[serde(default)]
    pub mvp: Vec<String>,
    /// Feature ids for later.
    #[serde(default)]
    pub later: Vec<String>,
    /// Feature ids in the order they are built.
    #[serde(default)]
    pub order: Vec<String>,
    #[serde(default)]
    pub testing: String,
    #[serde(default)]
    pub release: String,
    #[serde(default)]
    pub risks: Vec<Risk>,
    #[serde(default)]
    pub questions: Vec<OpenQuestion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Prd {
    pub identity: Identity,
    pub background: Background,
    pub goals: Goals,
    pub experience: Experience,
    /// The user flow as a diagram (U5b). PRDs from before U5b have none.
    #[serde(default)]
    pub flow: FlowChart,
    pub features: Vec<Feature>,
    #[serde(default)]
    pub nfr: Vec<Nfr>,
    #[serde(default)]
    pub technical: Technical,
    /// Persyaratan keamanan (R4). PRDs from before R4 have none and are asked for one.
    #[serde(default)]
    pub security: Vec<SecurityRequirement>,
    pub delivery: Delivery,
}

// ---------- the rules ----------

/// Something in a PRD that breaks a rule: where, and what (Indonesian, for the owner and for
/// Gemini's repair request).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Problem {
    /// `features[2].acceptance` style path, or an id (`FEAT-003`).
    pub at: String,
    pub message: String,
}

struct Rules {
    problems: Vec<Problem>,
}

impl Rules {
    fn add(&mut self, at: impl Into<String>, message: impl Into<String>) {
        self.problems.push(Problem { at: at.into(), message: message.into() });
    }

    fn text(&mut self, at: &str, value: &str, required: bool, max: usize) {
        let count = value.trim().chars().count();
        if required && count == 0 {
            self.add(at, "wajib diisi");
        } else if count > max {
            self.add(at, format!("lebih dari {max} karakter"));
        }
    }

    fn list(&mut self, at: &str, values: &[String]) {
        if values.len() > LIST_MAX {
            self.add(at, format!("lebih dari {LIST_MAX} butir"));
        }
        for (i, value) in values.iter().enumerate() {
            self.text(&format!("{at}[{i}]"), value, true, ITEM_MAX);
        }
    }

    fn flows(&mut self, at: &str, flows: &[Flow]) {
        if flows.len() > LIST_MAX {
            self.add(at, format!("lebih dari {LIST_MAX} butir"));
        }
        for (i, flow) in flows.iter().enumerate() {
            self.text(&format!("{at}[{i}].name"), &flow.name, true, NAME_MAX);
            self.list(&format!("{at}[{i}].steps"), &flow.steps);
        }
    }
}

fn shaped(id: &str, prefix: &str, digits: usize) -> bool {
    id.strip_prefix(prefix).is_some_and(|rest| rest.len() == digits && rest.chars().all(|c| c.is_ascii_digit()))
}

fn unique_ids<'a>(rules: &mut Rules, what: &str, ids: impl Iterator<Item = &'a str>, prefix: &str, digits: usize) {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !shaped(id, prefix, digits) {
            rules.add(id, format!("{what} harus berbentuk {prefix}{}", "0".repeat(digits)));
        } else if !seen.insert(id) {
            rules.add(id, format!("{what} {id} dipakai dua kali"));
        }
    }
}

/// The user flow diagram's rules (U5b), all placed under `flow`. No diagram at all is allowed (PRDs
/// from before U5b); one that is there must be drawable and make sense: steps N1, N2, ... once
/// each, a start and an end, arrows between steps that exist, a decision with two or more answered
/// ways out, every step reachable from a start, and none left without a way on except an end.
fn check_flow(r: &mut Rules, flow: &FlowChart, features: &BTreeSet<&str>) {
    if flow.is_empty() {
        return;
    }
    r.text("flow.title", &flow.title, false, NAME_MAX);
    if flow.nodes.len() > LIST_MAX {
        r.add("flow.nodes", format!("lebih dari {LIST_MAX} langkah"));
    }
    if flow.edges.len() > LIST_MAX * 2 {
        r.add("flow.edges", format!("lebih dari {} panah", LIST_MAX * 2));
    }
    let mut ids = BTreeSet::new();
    for node in &flow.nodes {
        let shaped = node.id.strip_prefix('N').is_some_and(|n| !n.is_empty() && n.len() <= 3 && n.chars().all(|c| c.is_ascii_digit()));
        if !shaped {
            r.add(format!("flow.{}", node.id), "langkah diagram harus berbentuk N1, N2, ...");
        } else if !ids.insert(node.id.as_str()) {
            r.add(format!("flow.{}", node.id), format!("langkah {} dipakai dua kali", node.id));
        }
        r.text(&format!("flow.{}.label", node.id), &node.label, true, NAME_MAX);
        if !node.feature.trim().is_empty() && !features.contains(node.feature.trim()) {
            r.add(format!("flow.{}", node.id), format!("menyebut {} yang tidak ada", node.feature.trim()));
        }
    }
    if !flow.nodes.iter().any(|n| n.kind == FlowNodeKind::Start) {
        r.add("flow", "diagram butuh langkah mulai (start)");
    }
    if !flow.nodes.iter().any(|n| n.kind == FlowNodeKind::End) {
        r.add("flow", "diagram butuh langkah akhir (end)");
    }
    let mut out: BTreeMap<&str, Vec<&FlowEdge>> = BTreeMap::new();
    for edge in &flow.edges {
        let at = format!("flow.{}-{}", edge.from, edge.to);
        if !ids.contains(edge.from.as_str()) || !ids.contains(edge.to.as_str()) {
            r.add(&at, "panah menyebut langkah yang tidak ada");
            continue;
        }
        if edge.from == edge.to {
            r.add(&at, "panah kembali ke langkah yang sama");
        }
        r.text(&format!("{at}.label"), &edge.label, false, NAME_MAX);
        out.entry(edge.from.as_str()).or_default().push(edge);
    }
    for node in &flow.nodes {
        let ways = out.get(node.id.as_str()).map(Vec::len).unwrap_or(0);
        match node.kind {
            FlowNodeKind::Decision if ways < 2 => r.add(format!("flow.{}", node.id), "keputusan butuh paling sedikit dua cabang"),
            FlowNodeKind::Decision if out[node.id.as_str()].iter().any(|e| e.label.trim().is_empty()) => {
                r.add(format!("flow.{}", node.id), "setiap cabang keputusan butuh label, misalnya Ya atau Tidak")
            }
            FlowNodeKind::End if ways > 0 => r.add(format!("flow.{}", node.id), "langkah akhir tidak punya langkah sesudahnya"),
            FlowNodeKind::End => {}
            _ if ways == 0 => r.add(format!("flow.{}", node.id), "belum punya langkah berikutnya atau akhir"),
            _ => {}
        }
    }
    // Every step can be reached from a start.
    let mut seen: BTreeSet<&str> = flow.nodes.iter().filter(|n| n.kind == FlowNodeKind::Start).map(|n| n.id.as_str()).collect();
    let mut queue: Vec<&str> = seen.iter().copied().collect();
    while let Some(at) = queue.pop() {
        for edge in out.get(at).into_iter().flatten() {
            if seen.insert(edge.to.as_str()) {
                queue.push(edge.to.as_str());
            }
        }
    }
    for node in &flow.nodes {
        if ids.contains(node.id.as_str()) && !seen.contains(node.id.as_str()) {
            r.add(format!("flow.{}", node.id), "tidak bisa dicapai dari langkah mulai");
        }
    }
}

/// The user flow diagram as Mermaid (`flowchart TD`), for the Markdown export and Claude Code.
/// Labels are quoted, their quotes written as Mermaid's `#quot;`.
pub fn mermaid(flow: &FlowChart) -> String {
    let quote = |text: &str| format!("\"{}\"", text.split_whitespace().collect::<Vec<_>>().join(" ").replace('"', "#quot;"));
    let mut out = String::from("flowchart TD\n");
    for node in &flow.nodes {
        let label = quote(&node.label);
        let shape = match node.kind {
            FlowNodeKind::Start | FlowNodeKind::End => format!("([{label}])"),
            FlowNodeKind::Screen => format!("[{label}]"),
            FlowNodeKind::Action => format!("({label})"),
            FlowNodeKind::Decision => format!("{{{label}}}"),
        };
        out.push_str(&format!("    {}{shape}\n", node.id));
    }
    for edge in &flow.edges {
        if edge.label.trim().is_empty() {
            out.push_str(&format!("    {} --> {}\n", edge.from, edge.to));
        } else {
            out.push_str(&format!("    {} -->|{}| {}\n", edge.from, quote(&edge.label), edge.to));
        }
    }
    out
}

/// Every rule a PRD must meet to be approved (docs/studio.md, "PRD"). Empty means none broken.
pub fn check(prd: &Prd) -> Vec<Problem> {
    let mut r = Rules { problems: Vec::new() };
    r.text("identity.name", &prd.identity.name, true, NAME_MAX);
    r.text("identity.summary", &prd.identity.summary, true, TEXT_MAX);
    r.text("identity.owner", &prd.identity.owner, false, NAME_MAX);

    let b = &prd.background;
    r.text("background.problem", &b.problem, true, TEXT_MAX);
    r.text("background.context", &b.context, false, TEXT_MAX);
    r.text("background.unmetNeed", &b.unmet_need, false, TEXT_MAX);
    r.list("background.targetUsers", &b.target_users);
    if b.target_users.is_empty() {
        r.add("background.targetUsers", "sebutkan paling sedikit satu pengguna sasaran");
    }
    for (i, e) in b.evidence.iter().enumerate() {
        r.text(&format!("background.evidence[{i}]"), &e.statement, true, ITEM_MAX);
    }
    unique_ids(&mut r, "Asumsi", b.assumptions.iter().map(|a| a.id.as_str()), "ASM-", 2);
    for a in &b.assumptions {
        r.text(&a.id, &a.statement, true, ITEM_MAX);
    }

    let g = &prd.goals;
    r.text("goals.primary", &g.primary, true, TEXT_MAX);
    r.text("goals.differentiator", &g.differentiator, false, TEXT_MAX);
    r.list("goals.benefits", &g.benefits);
    r.list("goals.outOfScope", &g.out_of_scope);
    for (i, m) in g.metrics.iter().enumerate() {
        r.text(&format!("goals.metrics[{i}].metric"), &m.metric, true, ITEM_MAX);
        r.text(&format!("goals.metrics[{i}].target"), &m.target, true, ITEM_MAX);
    }
    if g.metrics.is_empty() {
        r.add("goals.metrics", "sebutkan paling sedikit satu indikator keberhasilan dengan targetnya");
    }

    let x = &prd.experience;
    for (i, p) in x.personas.iter().enumerate() {
        r.text(&format!("experience.personas[{i}].name"), &p.name, true, NAME_MAX);
    }
    r.flows("experience.journeys", &x.journeys);
    r.flows("experience.mainFlows", &x.main_flows);
    r.flows("experience.alternateFlows", &x.alternate_flows);
    r.list("experience.emptyStates", &x.empty_states);
    r.list("experience.errorStates", &x.error_states);
    r.list("experience.accessibility", &x.accessibility);
    if x.main_flows.is_empty() {
        r.add("experience.mainFlows", "sebutkan paling sedikit satu alur utama");
    }

    if prd.features.is_empty() {
        r.add("features", "belum ada fitur");
    }
    if prd.features.len() > FEATURES_MAX {
        r.add("features", format!("lebih dari {FEATURES_MAX} fitur"));
    }
    unique_ids(&mut r, "Fitur", prd.features.iter().map(|f| f.id.as_str()), "FEAT-", 3);
    let known: BTreeSet<&str> = prd.features.iter().map(|f| f.id.as_str()).collect();
    let mut criteria = BTreeSet::new();
    for f in &prd.features {
        r.text(&format!("{}.name", f.id), &f.name, true, NAME_MAX);
        r.text(&format!("{}.description", f.id), &f.description, true, TEXT_MAX);
        r.text(&format!("{}.rationale", f.id), &f.rationale, false, TEXT_MAX);
        for d in &f.depends_on {
            if d == &f.id {
                r.add(&f.id, "bergantung pada dirinya sendiri");
            } else if !known.contains(d.as_str()) {
                r.add(&f.id, format!("bergantung pada {d} yang tidak ada"));
            }
        }
        let story = &f.story;
        for (part, value) in [("asA", &story.as_a), ("want", &story.want), ("soThat", &story.so_that)] {
            r.text(&format!("{}.story.{part}", f.id), value, f.priority != Priority::Wont, ITEM_MAX);
        }
        let needs_criteria = matches!(f.priority, Priority::Must | Priority::Should);
        if needs_criteria && f.acceptance.is_empty() {
            r.add(&f.id, "fitur must dan should butuh kriteria penerimaan");
        }
        if needs_criteria && f.tests.is_empty() {
            r.add(&f.id, "fitur must dan should butuh skenario pengujian");
        }
        for c in &f.acceptance {
            let number = f.id.trim_start_matches("FEAT-");
            let own = c.id.strip_prefix(&format!("AC-{number}-")).is_some_and(|n| !n.is_empty() && n.chars().all(|ch| ch.is_ascii_digit()));
            if !own {
                r.add(&c.id, format!("kriteria {} harus berbentuk AC-{number}-1", f.id));
            } else if !criteria.insert(c.id.clone()) {
                r.add(&c.id, "kriteria dipakai dua kali");
            }
            for (part, value) in [("given", &c.given), ("when", &c.when), ("then", &c.then)] {
                r.text(&format!("{}.{part}", c.id), value, true, ITEM_MAX);
            }
        }
        r.list(&format!("{}.tests", f.id), &f.tests);
    }
    check_flow(&mut r, &prd.flow, &known);
    // A cycle: follow each dependency chain; meeting the start again is one.
    let deps: BTreeMap<&str, Vec<&str>> = prd.features.iter().map(|f| (f.id.as_str(), f.depends_on.iter().map(String::as_str).collect())).collect();
    for start in deps.keys() {
        let mut stack: Vec<&str> = deps[start].clone();
        let mut visited = BTreeSet::new();
        while let Some(next) = stack.pop() {
            if next == *start {
                r.add(*start, "dependensinya melingkar");
                break;
            }
            if visited.insert(next) {
                stack.extend(deps.get(next).into_iter().flatten().copied());
            }
        }
    }

    for (i, n) in prd.nfr.iter().enumerate() {
        r.text(&format!("nfr[{i}].requirement"), &n.requirement, true, ITEM_MAX);
        r.text(&format!("nfr[{i}].measure"), &n.measure, false, ITEM_MAX);
    }
    let t = &prd.technical;
    r.text("technical.architecture", &t.architecture, false, TEXT_MAX);
    r.text("technical.dataModel", &t.data_model, false, TEXT_MAX);
    for (name, list) in [
        ("components", &t.components),
        ("integrations", &t.integrations),
        ("technologies", &t.technologies),
        ("dependencies", &t.dependencies),
        ("constraints", &t.constraints),
        ("risks", &t.risks),
    ] {
        r.list(&format!("technical.{name}"), list);
    }

    if prd.security.is_empty() {
        r.add("security", "tulis paling sedikit satu persyaratan keamanan");
    }
    if prd.security.len() > LIST_MAX {
        r.add("security", format!("lebih dari {LIST_MAX} butir"));
    }
    unique_ids(&mut r, "Persyaratan keamanan", prd.security.iter().map(|s| s.id.as_str()), "SEC-", 2);
    for s in &prd.security {
        r.text(&format!("{}.requirement", s.id), &s.requirement, true, ITEM_MAX);
        r.text(&format!("{}.verify", s.id), &s.verify, false, ITEM_MAX);
    }

    let d = &prd.delivery;
    for (name, list) in [("mvp", &d.mvp), ("later", &d.later), ("order", &d.order)] {
        for id in list {
            if !known.contains(id.as_str()) {
                r.add(format!("delivery.{name}"), format!("{id} tidak ada di daftar fitur"));
            }
        }
    }
    let must: Vec<&str> = prd.features.iter().filter(|f| f.priority == Priority::Must).map(|f| f.id.as_str()).collect();
    for id in &must {
        if !d.mvp.iter().any(|m| m == id) {
            r.add("delivery.mvp", format!("{id} (must) belum masuk MVP"));
        }
    }
    r.text("delivery.testing", &d.testing, false, TEXT_MAX);
    r.text("delivery.release", &d.release, false, TEXT_MAX);
    for (i, k) in d.risks.iter().enumerate() {
        r.text(&format!("delivery.risks[{i}].risk"), &k.risk, true, ITEM_MAX);
    }
    unique_ids(&mut r, "Pertanyaan", d.questions.iter().map(|q| q.id.as_str()), "Q-", 2);
    for q in &d.questions {
        r.text(&q.id, &q.question, true, ITEM_MAX);
    }
    r.problems
}

/// A PRD from the sections Gemini wrote (one JSON object holding them). Each section is read on
/// its own, so a broken one is named instead of the whole document failing as one.
pub fn assemble(value: &serde_json::Value) -> std::result::Result<Prd, Vec<Problem>> {
    fn read<T: serde::de::DeserializeOwned>(value: &serde_json::Value, key: &str, problems: &mut Vec<Problem>) -> Option<T> {
        match serde_json::from_value::<T>(value.get(key).cloned().unwrap_or(serde_json::Value::Null)) {
            Ok(section) => Some(section),
            Err(error) => {
                problems.push(Problem { at: key.to_string(), message: format!("bentuknya tidak sesuai: {error}") });
                None
            }
        }
    }
    let mut problems = Vec::new();
    let identity = read::<Identity>(value, "identity", &mut problems);
    let background = read::<Background>(value, "background", &mut problems);
    let goals = read::<Goals>(value, "goals", &mut problems);
    let experience = read::<Experience>(value, "experience", &mut problems);
    // U5b: missing is no diagram; a wrong shape is named.
    let flow = if value.get("flow").is_some() { read::<FlowChart>(value, "flow", &mut problems) } else { Some(FlowChart::default()) };
    let features = read::<Vec<Feature>>(value, "features", &mut problems);
    let nfr = read::<Vec<Nfr>>(value, "nfr", &mut problems);
    let technical = read::<Technical>(value, "technical", &mut problems);
    // Missing is empty (the rules then ask for one); a wrong shape is named.
    let security = if value.get("security").is_some() { read::<Vec<SecurityRequirement>>(value, "security", &mut problems) } else { Some(Vec::new()) };
    let delivery = read::<Delivery>(value, "delivery", &mut problems);
    match (identity, background, goals, experience, flow, features, nfr, technical, security, delivery) {
        (Some(identity), Some(background), Some(goals), Some(experience), Some(flow), Some(features), Some(nfr), Some(technical), Some(security), Some(delivery))
            if problems.is_empty() =>
        {
            Ok(Prd { identity, background, goals, experience, flow, features, nfr, technical, security, delivery })
        }
        _ => Err(problems),
    }
}

/// A section's JSON put into a PRD, read strictly (unknown fields refused).
pub fn with_section(prd: &Prd, section: PrdSection, value: serde_json::Value) -> Result<Prd> {
    let mut next = prd.clone();
    let bad = |_| Error::Invalid { field: "section", reason: Reason::Format };
    match section {
        PrdSection::Identity => next.identity = serde_json::from_value(value).map_err(bad)?,
        PrdSection::Background => next.background = serde_json::from_value(value).map_err(bad)?,
        PrdSection::Goals => next.goals = serde_json::from_value(value).map_err(bad)?,
        PrdSection::Experience => next.experience = serde_json::from_value(value).map_err(bad)?,
        PrdSection::Flow => next.flow = serde_json::from_value(value).map_err(bad)?,
        PrdSection::Features => next.features = serde_json::from_value(value).map_err(bad)?,
        PrdSection::Nfr => next.nfr = serde_json::from_value(value).map_err(bad)?,
        PrdSection::Technical => next.technical = serde_json::from_value(value).map_err(bad)?,
        PrdSection::Security => next.security = serde_json::from_value(value).map_err(bad)?,
        PrdSection::Delivery => next.delivery = serde_json::from_value(value).map_err(bad)?,
    }
    Ok(next)
}

/// One section of a PRD as JSON.
pub fn section_of(prd: &Prd, section: PrdSection) -> serde_json::Value {
    let value = serde_json::to_value(prd).unwrap_or_default();
    let key = match section {
        PrdSection::Identity => "identity",
        PrdSection::Background => "background",
        PrdSection::Goals => "goals",
        PrdSection::Experience => "experience",
        PrdSection::Flow => "flow",
        PrdSection::Features => "features",
        PrdSection::Nfr => "nfr",
        PrdSection::Technical => "technical",
        PrdSection::Security => "security",
        PrdSection::Delivery => "delivery",
    };
    value.get(key).cloned().unwrap_or_default()
}

// ---------- versions ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrdVersion {
    pub id: String,
    /// None for the working draft.
    pub number: Option<u32>,
    pub status: PrdStatus,
    pub change_note: String,
    pub made_by: StatedBy,
    pub created_at: String,
    pub updated_at: String,
    pub approved_at: Option<String>,
}

/// What the PRD tab shows: the draft (if any), its problems, whether it differs from the latest
/// numbered version, and the versions.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrdView {
    pub draft: Option<Prd>,
    pub draft_meta: Option<PrdVersion>,
    pub problems: Vec<Problem>,
    /// The draft holds changes no numbered version has.
    pub unsaved: bool,
    /// The approved version, if one is.
    pub approved: Option<PrdVersion>,
    pub versions: Vec<PrdVersion>,
    /// The approved tech stack as the technologies it gives the PRD (R4), so the page can say when
    /// the draft no longer matches it. `None` without an approved stack.
    pub stack: Option<Vec<String>>,
}

const VERSION_COLUMNS: &str = "id, number, status, change_note, made_by, created_at, updated_at, approved_at";

fn version_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PrdVersion> {
    Ok(PrdVersion {
        id: row.get(0)?,
        number: row.get(1)?,
        status: row.get(2)?,
        change_note: row.get(3)?,
        made_by: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        approved_at: row.get(7)?,
    })
}

fn live_project(conn: &Connection, id: &str) -> Result<String> {
    conn.query_row("SELECT name FROM studio_projects WHERE id = ?1 AND deleted_at IS NULL", [id], |row| row.get(0))
        .optional()?
        .ok_or(Error::NotFound("studioProject"))
}

fn record(tx: &Transaction<'_>, now: Timestamp, actor: Actor, verb: Verb, id: &str, summary: &str) -> Result<()> {
    activity::record(tx, now, Record { actor, verb, entity: Entity::StudioPrd, entity_id: Some(id), summary, details: serde_json::json!({}) })
}

fn draft_row(conn: &Connection, project: &str) -> Result<Option<(PrdVersion, String)>> {
    Ok(conn
        .query_row(
            &format!("SELECT {VERSION_COLUMNS}, body FROM studio_prd_versions WHERE studio_project_id = ?1 AND number IS NULL"),
            [project],
            |row| Ok((version_row(row)?, row.get::<_, String>(8)?)),
        )
        .optional()?)
}

fn latest_body(conn: &Connection, project: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT body FROM studio_prd_versions WHERE studio_project_id = ?1 AND number IS NOT NULL ORDER BY number DESC LIMIT 1",
            [project],
            |row| row.get(0),
        )
        .optional()?)
}

fn parse_body(body: &str) -> Result<Prd> {
    serde_json::from_str(body).map_err(|_| Error::Invalid { field: "prd", reason: Reason::Format })
}

/// The PRD tab's view of a project.
pub fn view(conn: &Connection, project_id: &str) -> Result<PrdView> {
    let project = ids::parse("projectId", project_id)?;
    live_project(conn, &project)?;
    let draft = draft_row(conn, &project)?;
    let mut statement = conn.prepare(&format!(
        "SELECT {VERSION_COLUMNS} FROM studio_prd_versions WHERE studio_project_id = ?1 AND number IS NOT NULL ORDER BY number DESC"
    ))?;
    let versions = statement.query_map([&project], version_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
    let approved = versions.iter().find(|v| v.status == PrdStatus::Approved).cloned();
    let latest = latest_body(conn, &project)?;
    let stack = approved_stack(conn, &project)?.map(|body| technologies(&body));
    Ok(match draft {
        Some((meta, body)) => {
            let prd = parse_body(&body)?;
            let unsaved = latest.as_deref().map(|l| serde_json::from_str::<serde_json::Value>(l).ok() != serde_json::from_str::<serde_json::Value>(&body).ok()).unwrap_or(true);
            PrdView { problems: check(&prd), draft: Some(prd), draft_meta: Some(meta), unsaved, approved, versions, stack }
        }
        None => PrdView { draft: None, draft_meta: None, problems: Vec::new(), unsaved: false, approved, versions, stack },
    })
}

// ---------- the tech stack in the PRD (R4) ----------

fn layer_label(layer: crate::stack::StackLayerKind) -> &'static str {
    use crate::stack::StackLayerKind as L;
    match layer {
        L::Platform => "Platform",
        L::Frontend => "Frontend",
        L::Backend => "Backend",
        L::Database => "Database",
        L::Auth => "Autentikasi",
        L::Hosting => "Hosting",
        L::Extras => "Layanan tambahan",
    }
}

/// The technologies an approved stack gives the PRD: one line per layer ("Backend: Laravel").
pub fn technologies(stack: &crate::stack::StackBody) -> Vec<String> {
    stack.layers.iter().map(|l| format!("{}: {}", layer_label(l.layer), l.choice)).collect()
}

fn approved_stack(conn: &Connection, project: &str) -> Result<Option<crate::stack::StackBody>> {
    Ok(crate::stack::get(conn, project)?.filter(|s| s.status == crate::stack::StackStatus::Approved).map(|s| s.body))
}

/// The PRD with its technologies taken from the approved stack: the technical part applies, and its
/// technologies are the stack's. The rest of the technical part stays as written.
pub fn with_stack(prd: &Prd, stack: &crate::stack::StackBody) -> Prd {
    let mut next = prd.clone();
    next.technical.applicable = true;
    next.technical.technologies = technologies(stack);
    next.technical.from_stack = true;
    next
}

/// "Perbarui dari tech stack": the draft's technologies follow the approved stack again (after
/// the stack changed, or for a PRD written before R4).
pub fn refresh_stack(conn: &mut Connection, project_id: &str, now: Timestamp) -> Result<Change> {
    let project = ids::parse("projectId", project_id)?;
    let stack = approved_stack(conn, &project)?.ok_or(Error::Invalid { field: "stack", reason: Reason::Order })?;
    let (_, body) = draft_row(conn, &project)?.ok_or(Error::NotFound("prd"))?;
    let next = with_stack(&parse_body(&body)?, &stack);
    update_section(conn, &project, PrdSection::Technical, section_of(&next, PrdSection::Technical), StatedBy::Owner, now)
}

/// One numbered version's document.
pub fn version_body(conn: &Connection, version_id: &str) -> Result<Prd> {
    let id = ids::parse("id", version_id)?;
    let body: String = conn
        .query_row("SELECT body FROM studio_prd_versions WHERE id = ?1 AND number IS NOT NULL", [id], |row| row.get(0))
        .optional()?
        .ok_or(Error::NotFound("prdVersion"))?;
    parse_body(&body)
}

fn freeze(tx: &Transaction<'_>, project: &str, body: &str, status: PrdStatus, note: &str, made_by: StatedBy, now: Timestamp) -> Result<(String, u32)> {
    let next: u32 = tx.query_row(
        "SELECT coalesce(max(number), 0) + 1 FROM studio_prd_versions WHERE studio_project_id = ?1",
        [project],
        |row| row.get(0),
    )?;
    let id = new_id();
    let stamp = instant(now);
    let approved_at = (status == PrdStatus::Approved).then(|| stamp.clone());
    tx.execute(
        "INSERT INTO studio_prd_versions (id, studio_project_id, number, status, body, change_note, made_by, created_at, updated_at, approved_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?9)",
        params![id, project, next, status, body, note, made_by, stamp, approved_at],
    )?;
    Ok((id, next))
}

fn write_draft(tx: &Transaction<'_>, project: &str, body: &str, made_by: StatedBy, now: Timestamp) -> Result<()> {
    let stamp = instant(now);
    let changed = tx.execute(
        "UPDATE studio_prd_versions SET body = ?2, made_by = ?3, updated_at = ?4 WHERE studio_project_id = ?1 AND number IS NULL",
        params![project, body, made_by, stamp],
    )?;
    if changed == 0 {
        tx.execute(
            "INSERT INTO studio_prd_versions (id, studio_project_id, number, status, body, made_by, created_at, updated_at)
             VALUES (?1, ?2, NULL, 'draft', ?3, ?4, ?5, ?5)",
            params![new_id(), project, body, made_by, stamp],
        )?;
    }
    tx.execute("UPDATE studio_projects SET updated_at = ?2 WHERE id = ?1", params![project, stamp])?;
    Ok(())
}

/// A PRD Gemini wrote from the conversation: the draft becomes it, and it is kept as a numbered
/// version at once (so nothing Gemini wrote is ever lost to later edits).
pub fn save_generated(conn: &mut Connection, project_id: &str, prd: &Prd, now: Timestamp) -> Result<(u32, Change)> {
    let project = ids::parse("projectId", project_id)?;
    let body = serde_json::to_string(prd)?;
    let tx = conn.transaction()?;
    let name = live_project(&tx, &project)?;
    write_draft(&tx, &project, &body, StatedBy::Gemini, now)?;
    let (id, number) = freeze(&tx, &project, &body, PrdStatus::Saved, "Disusun Lentera dari percakapan.", StatedBy::Gemini, now)?;
    record(&tx, now, Actor::Gemini, Verb::Created, &id, &name)?;
    tx.commit()?;
    Ok((number, Change::one(Entity::StudioPrd, project)))
}

/// The owner edits one section of the draft (or accepts Gemini's revision of it). The draft may
/// break rules while it is being edited; its problems are shown.
pub fn update_section(conn: &mut Connection, project_id: &str, section: PrdSection, value: serde_json::Value, made_by: StatedBy, now: Timestamp) -> Result<Change> {
    let project = ids::parse("projectId", project_id)?;
    let tx = conn.transaction()?;
    let name = live_project(&tx, &project)?;
    let (_, body) = draft_row(&tx, &project)?.ok_or(Error::NotFound("prd"))?;
    let next = with_section(&parse_body(&body)?, section, value)?;
    write_draft(&tx, &project, &serde_json::to_string(&next)?, made_by, now)?;
    let actor = if made_by == StatedBy::Gemini { Actor::Gemini } else { Actor::Owner };
    activity::record(
        &tx,
        now,
        Record { actor, verb: Verb::Updated, entity: Entity::StudioPrd, entity_id: Some(&project), summary: &name, details: serde_json::json!({ "section": section }) },
    )?;
    tx.commit()?;
    Ok(Change::one(Entity::StudioPrd, project))
}

/// "Simpan versi": the draft as a new numbered version, with a note.
pub fn save_version(conn: &mut Connection, project_id: &str, note: &str, now: Timestamp) -> Result<(u32, Change)> {
    let project = ids::parse("projectId", project_id)?;
    let note = note.trim();
    if note.chars().count() > NOTE_MAX {
        return invalid("note", Reason::Length);
    }
    let tx = conn.transaction()?;
    let name = live_project(&tx, &project)?;
    let (meta, body) = draft_row(&tx, &project)?.ok_or(Error::NotFound("prd"))?;
    let (id, number) = freeze(&tx, &project, &body, PrdStatus::Saved, note, meta.made_by, now)?;
    record(&tx, now, Actor::Owner, Verb::Created, &id, &name)?;
    tx.commit()?;
    Ok((number, Change::one(Entity::StudioPrd, project)))
}

/// "Setujui": the draft, with no problems, becomes the approved version (frozen as a new number
/// unless the latest number already holds the same document). An earlier approval is superseded.
pub fn approve(conn: &mut Connection, project_id: &str, now: Timestamp) -> Result<(u32, Change)> {
    let project = ids::parse("projectId", project_id)?;
    let tx = conn.transaction()?;
    let name = live_project(&tx, &project)?;
    let (meta, body) = draft_row(&tx, &project)?.ok_or(Error::NotFound("prd"))?;
    if !check(&parse_body(&body)?).is_empty() {
        return invalid("prd", Reason::Format);
    }
    let stamp = instant(now);
    tx.execute(
        "UPDATE studio_prd_versions SET status = 'superseded', updated_at = ?2 WHERE studio_project_id = ?1 AND status = 'approved'",
        params![project, stamp],
    )?;
    let latest: Option<(String, u32, String)> = tx
        .query_row(
            "SELECT id, number, body FROM studio_prd_versions WHERE studio_project_id = ?1 AND number IS NOT NULL ORDER BY number DESC LIMIT 1",
            [&project],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let same = |a: &str, b: &str| serde_json::from_str::<serde_json::Value>(a).ok() == serde_json::from_str::<serde_json::Value>(b).ok();
    let (id, number) = match latest {
        Some((id, number, latest)) if same(&latest, &body) => {
            tx.execute(
                "UPDATE studio_prd_versions SET status = 'approved', approved_at = ?2, updated_at = ?2 WHERE id = ?1",
                params![id, stamp],
            )?;
            (id, number)
        }
        _ => freeze(&tx, &project, &body, PrdStatus::Approved, "Disetujui.", meta.made_by, now)?,
    };
    record(&tx, now, Actor::Owner, Verb::Approved, &id, &name)?;
    tx.commit()?;
    Ok((number, Change::one(Entity::StudioPrd, project)))
}

// ---------- Markdown ----------

fn bullet(out: &mut String, items: &[String]) {
    for item in items {
        out.push_str(&format!("- {}\n", item.trim()));
    }
    if !items.is_empty() {
        out.push('\n');
    }
}

fn para(out: &mut String, label: &str, text: &str) {
    if !text.trim().is_empty() {
        out.push_str(&format!("**{label}:** {}\n\n", text.trim()));
    }
}

fn flows(out: &mut String, title: &str, flows: &[Flow]) {
    if flows.is_empty() {
        return;
    }
    out.push_str(&format!("### {title}\n\n"));
    for flow in flows {
        out.push_str(&format!("**{}**\n\n", flow.name.trim()));
        for (i, step) in flow.steps.iter().enumerate() {
            out.push_str(&format!("{}. {}\n", i + 1, step.trim()));
        }
        out.push('\n');
    }
}

fn priority_label(p: Priority) -> &'static str {
    match p {
        Priority::Must => "Must",
        Priority::Should => "Should",
        Priority::Could => "Could",
        Priority::Wont => "Won't",
    }
}

/// The PRD as a Markdown file: what the owner exports, readable anywhere.
fn security_label(area: SecurityArea) -> &'static str {
    match area {
        SecurityArea::Auth => "autentikasi",
        SecurityArea::Access => "hak akses",
        SecurityArea::Data => "data",
        SecurityArea::Input => "input",
        SecurityArea::Secrets => "rahasia",
        SecurityArea::Transport => "koneksi",
        SecurityArea::Logging => "log",
        SecurityArea::Dependencies => "dependensi",
        SecurityArea::Other => "lainnya",
    }
}

pub fn markdown(prd: &Prd, version: Option<&PrdVersion>) -> String {
    let mut out = format!("# {}\n\n{}\n\n", prd.identity.name.trim(), prd.identity.summary.trim());
    if let Some(v) = version {
        let status = match v.status {
            PrdStatus::Approved => "disetujui",
            PrdStatus::Superseded => "digantikan",
            PrdStatus::Saved => "tersimpan",
            PrdStatus::Draft => "draf",
        };
        out.push_str(&format!("Versi {} ({status}), {}.\n\n", v.number.map(|n| n.to_string()).unwrap_or_else(|| "draf".into()), &v.updated_at[..10.min(v.updated_at.len())]));
    }
    para(&mut out, "Penanggung jawab", &prd.identity.owner);

    let b = &prd.background;
    out.push_str("## Latar belakang\n\n");
    para(&mut out, "Masalah", &b.problem);
    para(&mut out, "Konteks", &b.context);
    para(&mut out, "Kebutuhan yang belum terpenuhi", &b.unmet_need);
    if !b.target_users.is_empty() {
        out.push_str("**Pengguna sasaran:**\n\n");
        bullet(&mut out, &b.target_users);
    }
    if !b.evidence.is_empty() {
        out.push_str("**Bukti:**\n\n");
        let lines: Vec<String> = b
            .evidence
            .iter()
            .map(|e| if e.source == EvidenceSource::Unknown { format!("{} (belum ada bukti)", e.statement) } else { e.statement.clone() })
            .collect();
        bullet(&mut out, &lines);
    }
    if !b.assumptions.is_empty() {
        out.push_str("**Asumsi yang perlu divalidasi:**\n\n");
        let lines: Vec<String> = b
            .assumptions
            .iter()
            .map(|a| if a.validate.trim().is_empty() { format!("{}: {}", a.id, a.statement) } else { format!("{}: {} (validasi: {})", a.id, a.statement, a.validate) })
            .collect();
        bullet(&mut out, &lines);
    }

    let g = &prd.goals;
    out.push_str("## Tujuan\n\n");
    para(&mut out, "Tujuan utama", &g.primary);
    para(&mut out, "Nilai pembeda", &g.differentiator);
    if !g.benefits.is_empty() {
        out.push_str("**Manfaat bagi pengguna:**\n\n");
        bullet(&mut out, &g.benefits);
    }
    if !g.metrics.is_empty() {
        out.push_str("**Indikator keberhasilan:**\n\n");
        let lines: Vec<String> = g.metrics.iter().map(|m| format!("{}: target {}{}", m.metric, m.target, if m.how.is_empty() { String::new() } else { format!(" ({})", m.how) })).collect();
        bullet(&mut out, &lines);
    }
    if !g.out_of_scope.is_empty() {
        out.push_str("**Tidak termasuk lingkup:**\n\n");
        bullet(&mut out, &g.out_of_scope);
    }

    let x = &prd.experience;
    out.push_str("## Pengalaman pengguna\n\n");
    if !x.personas.is_empty() {
        out.push_str("### Persona\n\n");
        let lines: Vec<String> = x.personas.iter().map(|p| format!("**{}**: {} {}", p.name, p.need, p.context).trim().to_string()).collect();
        bullet(&mut out, &lines);
    }
    flows(&mut out, "User journey", &x.journeys);
    flows(&mut out, "Alur utama", &x.main_flows);
    flows(&mut out, "Kondisi alternatif", &x.alternate_flows);
    for (title, list) in [("Empty state", &x.empty_states), ("Error state", &x.error_states), ("Aksesibilitas", &x.accessibility)] {
        if !list.is_empty() {
            out.push_str(&format!("### {title}\n\n"));
            bullet(&mut out, list);
        }
    }

    // U5b: the user flow as Mermaid, which GitHub and many Markdown readers draw.
    if !prd.flow.is_empty() {
        out.push_str("## Alur pengguna\n\n");
        if !prd.flow.title.trim().is_empty() {
            out.push_str(&format!("{}\n\n", prd.flow.title.trim()));
        }
        out.push_str(&format!("```mermaid\n{}```\n\n", mermaid(&prd.flow)));
    }

    out.push_str("## Kebutuhan fungsional\n\n");
    for f in &prd.features {
        out.push_str(&format!("### {} {}\n\n", f.id, f.name.trim()));
        out.push_str(&format!("**Prioritas:** {}\n\n", priority_label(f.priority)));
        out.push_str(&format!("{}\n\n", f.description.trim()));
        para(&mut out, "Alasan", &f.rationale);
        if !f.depends_on.is_empty() {
            out.push_str(&format!("**Bergantung pada:** {}\n\n", f.depends_on.join(", ")));
        }
        out.push_str(&format!("**User story:** Sebagai {}, aku ingin {}, agar {}.\n\n", f.story.as_a.trim(), f.story.want.trim(), f.story.so_that.trim()));
        if !f.acceptance.is_empty() {
            out.push_str("**Kriteria penerimaan:**\n\n");
            for c in &f.acceptance {
                out.push_str(&format!("- {}: Given {}, When {}, Then {}\n", c.id, c.given.trim(), c.when.trim(), c.then.trim()));
            }
            out.push('\n');
        }
        if !f.tests.is_empty() {
            out.push_str("**Skenario pengujian:**\n\n");
            bullet(&mut out, &f.tests);
        }
    }

    if !prd.nfr.is_empty() {
        out.push_str("## Kebutuhan nonfungsional\n\n");
        let lines: Vec<String> = prd
            .nfr
            .iter()
            .map(|n| format!("{}: {}{}", n.category.as_str(), n.requirement, if n.measure.is_empty() { String::new() } else { format!(" (ukuran: {})", n.measure) }))
            .collect();
        bullet(&mut out, &lines);
    }

    let t = &prd.technical;
    if t.applicable {
        if t.from_stack {
            out.push_str("## Perencanaan teknis\n\n");
            out.push_str("**Tech stack (disetujui pemilik):**\n\n");
            bullet(&mut out, &t.technologies);
            out.push_str("Bagian berikut adalah usulan di atas tech stack itu; kode proyek belum diperiksa.\n\n");
        } else {
            out.push_str("## Perencanaan teknis (usulan)\n\n");
        }
        para(&mut out, "Arsitektur", &t.architecture);
        para(&mut out, "Model data", &t.data_model);
        let technologies = if t.from_stack { &Vec::new() } else { &t.technologies };
        for (title, list) in [
            ("Komponen", &t.components),
            ("Integrasi", &t.integrations),
            ("Teknologi", technologies),
            ("Dependensi", &t.dependencies),
            ("Batasan teknis", &t.constraints),
            ("Risiko implementasi", &t.risks),
        ] {
            if !list.is_empty() {
                out.push_str(&format!("**{title}:**\n\n"));
                bullet(&mut out, list);
            }
        }
    }

    if !prd.security.is_empty() {
        out.push_str("## Persyaratan keamanan\n\n");
        let lines: Vec<String> = prd
            .security
            .iter()
            .map(|s| {
                let verify = if s.verify.trim().is_empty() { String::new() } else { format!(" Cara memeriksa: {}", s.verify.trim()) };
                format!("{} ({}): {}{verify}", s.id, security_label(s.area), s.requirement.trim())
            })
            .collect();
        bullet(&mut out, &lines);
    }

    let d = &prd.delivery;
    out.push_str("## Strategi pengembangan\n\n");
    if !d.mvp.is_empty() {
        out.push_str(&format!("**MVP:** {}\n\n", d.mvp.join(", ")));
    }
    if !d.later.is_empty() {
        out.push_str(&format!("**Pengembangan berikutnya:** {}\n\n", d.later.join(", ")));
    }
    if !d.order.is_empty() {
        out.push_str(&format!("**Urutan implementasi:** {}\n\n", d.order.join(", ")));
    }
    para(&mut out, "Strategi pengujian", &d.testing);
    para(&mut out, "Rencana rilis", &d.release);
    if !d.risks.is_empty() {
        out.push_str("**Risiko dan mitigasi:**\n\n");
        let lines: Vec<String> = d.risks.iter().map(|k| format!("{}{}{}", k.risk, if k.impact.is_empty() { String::new() } else { format!(" (dampak: {})", k.impact) }, if k.mitigation.is_empty() { String::new() } else { format!("; mitigasi: {}", k.mitigation) })).collect();
        bullet(&mut out, &lines);
    }
    if !d.questions.is_empty() {
        out.push_str("**Pertanyaan yang belum terjawab:**\n\n");
        let lines: Vec<String> = d.questions.iter().map(|q| format!("{}: {}{}", q.id, q.question, if q.owner.is_empty() { String::new() } else { format!(" ({})", q.owner) })).collect();
        bullet(&mut out, &lines);
    }
    out.trim_end().to_string() + "\n"
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::studio::{create_project, StudioProjectInput};

    /// A small user flow with a decision and a way back (U5b).
    pub fn sample_flow() -> FlowChart {
        let node = |id: &str, kind, label: &str, feature: &str| FlowNode { id: id.into(), kind, label: label.into(), feature: feature.into() };
        let edge = |from: &str, to: &str, label: &str| FlowEdge { from: from.into(), to: to.into(), label: label.into() };
        FlowChart {
            title: "Absen praktikum".into(),
            nodes: vec![
                node("N1", FlowNodeKind::Start, "Siswa masuk lab", ""),
                node("N2", FlowNodeKind::Screen, "Layar absen", "FEAT-001"),
                node("N3", FlowNodeKind::Decision, "Kartu terdaftar?", ""),
                node("N4", FlowNodeKind::Action, "Sistem mencatat hadir", "FEAT-001"),
                node("N5", FlowNodeKind::End, "Praktikum dimulai", ""),
                node("N6", FlowNodeKind::Screen, "Pesan \"kartu tidak dikenal\"", ""),
            ],
            edges: vec![edge("N1", "N2", ""), edge("N2", "N3", "tempel kartu"), edge("N3", "N4", "Ya"), edge("N4", "N5", ""), edge("N3", "N6", "Tidak"), edge("N6", "N2", "coba lagi")],
        }
    }

    // U5b: the diagram's rules, its Mermaid, and PRDs without one.
    #[test]
    fn the_flow_diagram_is_checked_drawn_as_mermaid_and_optional() {
        assert!(check(&sample()).is_empty(), "{:?}", check(&sample()));
        let at = |prd: &Prd| check(prd).into_iter().filter(|p| p.at.starts_with("flow")).map(|p| format!("{}: {}", p.at, p.message)).collect::<Vec<_>>();

        // No diagram at all is fine (PRDs from before U5b), and reads from JSON without the key.
        let mut none = sample();
        none.flow = FlowChart::default();
        assert!(at(&none).is_empty());
        let mut old = serde_json::to_value(sample()).unwrap();
        old.as_object_mut().unwrap().remove("flow");
        assert!(assemble(&old).unwrap().flow.is_empty());

        let broken = |change: &dyn Fn(&mut FlowChart)| {
            let mut prd = sample();
            change(&mut prd.flow);
            at(&prd)
        };
        assert!(broken(&|f| f.nodes[1].id = "X2".into()).iter().any(|p| p.contains("berbentuk N1")));
        assert!(broken(&|f| f.nodes[5].id = "N1".into()).iter().any(|p| p.contains("dipakai dua kali")));
        assert!(broken(&|f| f.nodes[1].feature = "FEAT-009".into()).iter().any(|p| p == "flow.N2: menyebut FEAT-009 yang tidak ada"));
        assert!(broken(&|f| f.nodes.retain(|n| n.kind != FlowNodeKind::Start)).iter().any(|p| p.contains("langkah mulai")));
        assert!(broken(&|f| f.edges.push(FlowEdge { from: "N2".into(), to: "N9".into(), label: String::new() })).iter().any(|p| p.contains("tidak ada")));
        // A decision needs two answered ways out.
        assert!(broken(&|f| f.edges.retain(|e| e.label != "Tidak")).iter().any(|p| p == "flow.N3: keputusan butuh paling sedikit dua cabang"));
        assert!(broken(&|f| f.edges[2].label.clear()).iter().any(|p| p.contains("butuh label")));
        // Nothing after an end; nothing left without a way on; every step reachable.
        assert!(broken(&|f| f.edges.push(FlowEdge { from: "N5".into(), to: "N2".into(), label: String::new() })).iter().any(|p| p.contains("akhir tidak punya")));
        assert!(broken(&|f| f.edges.retain(|e| !(e.from == "N4" && e.to == "N5"))).iter().any(|p| p == "flow.N4: belum punya langkah berikutnya atau akhir"));
        assert!(broken(&|f| f.edges.retain(|e| e.from != "N1")).iter().any(|p| p.contains("tidak bisa dicapai")));

        let text = mermaid(&sample_flow());
        assert!(text.starts_with("flowchart TD\n"));
        for part in [
            "    N1([\"Siswa masuk lab\"])",
            "    N2[\"Layar absen\"]",
            "    N3{\"Kartu terdaftar?\"}",
            "    N4(\"Sistem mencatat hadir\")",
            "    N6[\"Pesan #quot;kartu tidak dikenal#quot;\"]",
            "    N3 -->|\"Ya\"| N4",
            "    N1 --> N2",
        ] {
            assert!(text.contains(part), "{part}\n{text}");
        }
        let md = markdown(&sample(), None);
        assert!(md.contains("## Alur pengguna\n\nAbsen praktikum\n\n```mermaid\nflowchart TD\n"));
        assert!(!markdown(&none, None).contains("## Alur pengguna"));
    }

    pub fn sample() -> Prd {
        let feature = |n: u32, priority: Priority, deps: &[&str]| Feature {
            id: format!("FEAT-{n:03}"),
            name: format!("Fitur {n}"),
            description: "Siswa mencatat kehadiran dengan kartu.".into(),
            rationale: "Absen manual lambat.".into(),
            priority,
            depends_on: deps.iter().map(|d| d.to_string()).collect(),
            story: Story { as_a: "siswa".into(), want: "absen cepat".into(), so_that: "praktikum mulai tepat waktu".into() },
            acceptance: vec![Criterion {
                id: format!("AC-{n:03}-1"),
                given: "siswa terdaftar".into(),
                when: "menempelkan kartu".into(),
                then: "kehadiran tercatat dalam 2 detik".into(),
            }],
            tests: vec!["Kartu terdaftar tercatat".into()],
        };
        Prd {
            identity: Identity { name: "Absensi Lab".into(), summary: "Absensi praktikum dengan kartu.".into(), owner: "Glenvio".into() },
            background: Background {
                problem: "Absen manual memakan 10 menit.".into(),
                context: "Lab TJKT".into(),
                target_users: vec!["Siswa TJKT".into()],
                unmet_need: String::new(),
                evidence: vec![Evidence { statement: "Pengamatan pemilik".into(), source: EvidenceSource::Owner }],
                assumptions: vec![Assumption { id: "ASM-01".into(), statement: "Semua siswa punya kartu".into(), validate: "Tanya wali kelas".into() }],
            },
            goals: Goals {
                primary: "Absen selesai dalam 2 menit.".into(),
                benefits: vec!["Praktikum mulai tepat waktu".into()],
                differentiator: String::new(),
                metrics: vec![Metric { metric: "Waktu absen".into(), target: "di bawah 2 menit".into(), how: "stopwatch".into() }],
                out_of_scope: vec!["Penilaian".into()],
            },
            experience: Experience { main_flows: vec![Flow { name: "Absen".into(), steps: vec!["Tempel kartu".into()] }], ..Default::default() },
            flow: sample_flow(),
            features: vec![feature(1, Priority::Must, &[]), feature(2, Priority::Should, &["FEAT-001"]), feature(3, Priority::Could, &[])],
            nfr: vec![Nfr { category: NfrCategory::Performance, requirement: "Cepat".into(), measure: "2 detik".into() }],
            technical: Technical::default(),
            security: vec![SecurityRequirement {
                id: "SEC-01".into(),
                area: SecurityArea::Data,
                requirement: "Data kehadiran hanya bisa dilihat guru".into(),
                verify: "Uji akses sebagai siswa".into(),
            }],
            delivery: Delivery {
                mvp: vec!["FEAT-001".into(), "FEAT-002".into()],
                later: vec!["FEAT-003".into()],
                order: vec!["FEAT-001".into(), "FEAT-002".into()],
                testing: "Uji di lab".into(),
                release: String::new(),
                risks: vec![],
                questions: vec![OpenQuestion { id: "Q-01".into(), question: "Pakai kartu apa?".into(), owner: "Guru".into() }],
            },
        }
    }

    fn now() -> Timestamp {
        "2026-11-06T03:00:00Z".parse().unwrap()
    }

    fn project(conn: &mut Connection) -> String {
        create_project(conn, StudioProjectInput { name: "Absensi".into(), idea: String::new() }, Actor::Owner, now()).unwrap().0
    }

    #[test]
    fn a_good_prd_has_no_problems_and_round_trips() {
        let prd = sample();
        assert_eq!(check(&prd), vec![]);
        let json = serde_json::to_string(&prd).unwrap();
        assert!(json.contains("\"targetUsers\"") && json.contains("\"dependsOn\"") && json.contains("\"soThat\""));
        assert_eq!(serde_json::from_str::<Prd>(&json).unwrap(), prd);
    }

    #[test]
    fn the_rules_find_what_gemini_and_edits_get_wrong() {
        let mut prd = sample();
        prd.features[1].id = "FEAT-001".into();
        prd.features[0].depends_on = vec!["FEAT-009".into()];
        prd.features[0].acceptance[0].then = " ".into();
        prd.features[2].priority = Priority::Must;
        prd.features[2].tests.clear();
        prd.goals.metrics[0].target = String::new();
        prd.background.assumptions[0].id = "A1".into();
        prd.identity.name = "x".repeat(121);
        let problems = check(&prd);
        let has = |at: &str, needle: &str| problems.iter().any(|p| p.at == at && p.message.contains(needle));
        assert!(has("FEAT-001", "dipakai dua kali"), "{problems:?}");
        assert!(has("FEAT-001", "FEAT-009 yang tidak ada"));
        assert!(has("AC-001-1.then", "wajib"));
        assert!(has("FEAT-003", "skenario pengujian"));
        assert!(has("delivery.mvp", "FEAT-003 (must) belum masuk MVP"));
        assert!(has("goals.metrics[0].target", "wajib"));
        assert!(has("A1", "ASM-00"));
        assert!(has("identity.name", "120"));
    }

    #[test]
    fn a_dependency_cycle_is_found() {
        let mut prd = sample();
        prd.features[0].depends_on = vec!["FEAT-002".into()];
        let problems = check(&prd);
        assert!(problems.iter().any(|p| p.message.contains("melingkar")), "{problems:?}");
        prd.features[0].depends_on = vec!["FEAT-001".into()];
        assert!(check(&prd).iter().any(|p| p.message.contains("dirinya sendiri")));
    }

    #[test]
    fn a_wont_feature_needs_no_criteria_and_unknown_fields_are_refused() {
        let mut prd = sample();
        prd.features[2].priority = Priority::Wont;
        prd.features[2].acceptance.clear();
        prd.features[2].tests.clear();
        assert_eq!(check(&prd), vec![]);
        let mut value = serde_json::to_value(&prd).unwrap();
        value["features"][0]["extra"] = serde_json::json!("x");
        assert!(serde_json::from_value::<Prd>(value).is_err());
    }

    #[test]
    fn versions_freeze_edits_change_only_the_draft_and_approval_needs_no_problems() {
        let mut conn = open_in_memory().unwrap();
        let id = project(&mut conn);
        assert!(view(&conn, &id).unwrap().draft.is_none());
        let (n, _) = save_generated(&mut conn, &id, &sample(), now()).unwrap();
        assert_eq!(n, 1);
        let v = view(&conn, &id).unwrap();
        assert!(!v.unsaved && v.problems.is_empty() && v.versions.len() == 1);
        assert_eq!(v.versions[0].made_by, StatedBy::Gemini);

        // An edit that breaks a rule is kept in the draft, with its problem shown.
        let mut goals = section_of(&sample(), PrdSection::Goals);
        goals["metrics"] = serde_json::json!([]);
        update_section(&mut conn, &id, PrdSection::Goals, goals, StatedBy::Owner, now()).unwrap();
        let v = view(&conn, &id).unwrap();
        assert!(v.unsaved);
        assert_eq!(v.problems.len(), 1);
        assert!(matches!(approve(&mut conn, &id, now()), Err(Error::Invalid { field: "prd", .. })));
        // Version 1 is untouched.
        assert_eq!(version_body(&conn, &v.versions[0].id).unwrap(), sample());

        update_section(&mut conn, &id, PrdSection::Goals, section_of(&sample(), PrdSection::Goals), StatedBy::Owner, now()).unwrap();
        assert!(!view(&conn, &id).unwrap().unsaved, "back to what version 1 holds");
        let (approved, _) = approve(&mut conn, &id, now()).unwrap();
        assert_eq!(approved, 1, "the same document: version 1 itself is approved");

        let mut identity = section_of(&sample(), PrdSection::Identity);
        identity["summary"] = serde_json::json!("Absensi praktikum dan izin.");
        update_section(&mut conn, &id, PrdSection::Identity, identity, StatedBy::Owner, now()).unwrap();
        let (saved, _) = save_version(&mut conn, &id, "Tambah izin", now()).unwrap();
        assert_eq!(saved, 2);
        let (again, _) = approve(&mut conn, &id, now()).unwrap();
        assert_eq!(again, 2);
        let v = view(&conn, &id).unwrap();
        let statuses: Vec<(Option<u32>, PrdStatus)> = v.versions.iter().map(|x| (x.number, x.status)).collect();
        assert_eq!(statuses, [(Some(2), PrdStatus::Approved), (Some(1), PrdStatus::Superseded)]);
        assert_eq!(v.approved.unwrap().number, Some(2));
        let verbs: i64 = conn.query_row("SELECT count(*) FROM activity WHERE entity_type = 'studio_prd' AND verb = 'approved'", [], |r| r.get(0)).unwrap();
        assert_eq!(verbs, 2);
    }

    /// Hari Ini and Jejak read every activity row: one with a verb or entity they cannot read
    /// would break them (found by the native smoke in S3: an "approved" row emptied Hari Ini).
    #[test]
    fn every_row_studio_writes_is_readable_by_jejak_and_hari_ini() {
        use crate::studio::{add_ledger_item, advance_stage, LedgerInput, LedgerKind, StudioStage};
        let mut conn = open_in_memory().unwrap();
        let id = project(&mut conn);
        let note = LedgerInput { kind: LedgerKind::Decision, topic: "Platform".into(), statement: "Web".into(), rationale: String::new(), area: None, replaces_id: None };
        add_ledger_item(&mut conn, &id, note, StatedBy::Gemini, None, now()).unwrap();
        advance_stage(&mut conn, &id, StudioStage::Clarify, &[StudioStage::Clarify], Actor::Owner, now()).unwrap();
        save_generated(&mut conn, &id, &sample(), now()).unwrap();
        update_section(&mut conn, &id, PrdSection::Goals, section_of(&sample(), PrdSection::Goals), StatedBy::Owner, now()).unwrap();
        save_version(&mut conn, &id, "", now()).unwrap();
        approve(&mut conn, &id, now()).unwrap();
        let rows = crate::activity::recent(&conn, 100, true).unwrap();
        assert!(rows.iter().any(|r| r.verb == Verb::Approved && r.entity == Entity::StudioPrd));
        assert!(rows.iter().any(|r| r.actor == Actor::Gemini));
        crate::insights::trail(&conn, &crate::insights::TrailQuery::default()).unwrap();
        let ctx = crate::context::Ctx::load(&conn, now()).unwrap();
        crate::today::today(&conn, ctx.now, &ctx.tz, ctx.waking).unwrap();
    }

    #[test]
    fn a_section_must_fit_its_shape() {
        let mut conn = open_in_memory().unwrap();
        let id = project(&mut conn);
        assert!(matches!(update_section(&mut conn, &id, PrdSection::Goals, serde_json::json!({}), StatedBy::Owner, now()), Err(Error::NotFound("prd"))));
        save_generated(&mut conn, &id, &sample(), now()).unwrap();
        let bad = serde_json::json!({ "primary": "x", "surprise": true });
        assert!(matches!(update_section(&mut conn, &id, PrdSection::Goals, bad, StatedBy::Owner, now()), Err(Error::Invalid { field: "section", .. })));
    }

    #[test]
    fn a_prd_is_assembled_from_its_parts_and_a_broken_section_is_named() {
        let value = serde_json::to_value(sample()).unwrap();
        assert_eq!(assemble(&value).unwrap(), sample());
        let mut broken = value.clone();
        broken["goals"] = serde_json::json!({ "primary": 3 });
        broken.as_object_mut().unwrap().remove("delivery");
        let problems = assemble(&broken).unwrap_err();
        let places: Vec<&str> = problems.iter().map(|p| p.at.as_str()).collect();
        assert_eq!(places, ["goals", "delivery"]);
    }

    #[test]
    fn markdown_reads_like_a_document() {
        let text = markdown(&sample(), None);
        assert!(text.starts_with("# Absensi Lab\n\nAbsensi praktikum dengan kartu."));
        for part in [
            "## Latar belakang",
            "**Masalah:** Absen manual memakan 10 menit.",
            "ASM-01: Semua siswa punya kartu (validasi: Tanya wali kelas)",
            "## Kebutuhan fungsional",
            "### FEAT-002 Fitur 2",
            "**Bergantung pada:** FEAT-001",
            "- AC-001-1: Given siswa terdaftar, When menempelkan kartu, Then kehadiran tercatat dalam 2 detik",
            "**MVP:** FEAT-001, FEAT-002",
            "Q-01: Pakai kartu apa? (Guru)",
        ] {
            assert!(text.contains(part), "missing {part}\n{text}");
        }
        assert!(!text.contains("Perencanaan teknis"), "not applicable, not shown");
        assert!(text.contains("## Persyaratan keamanan\n\n- SEC-01 (data): Data kehadiran hanya bisa dilihat guru Cara memeriksa: Uji akses sebagai siswa"));
    }

    // R4: security is a section of its own, required, with SEC-NN ids.
    #[test]
    fn a_prd_needs_a_security_requirement_with_a_proper_id() {
        let mut prd = sample();
        prd.security.clear();
        assert!(check(&prd).iter().any(|p| p.at == "security"));
        prd.security = vec![
            SecurityRequirement { id: "SEC-01".into(), area: SecurityArea::Secrets, requirement: "API key tidak di repo".into(), verify: String::new() },
            SecurityRequirement { id: "SEC-01".into(), area: SecurityArea::Input, requirement: " ".into(), verify: String::new() },
            SecurityRequirement { id: "SEC-3".into(), area: SecurityArea::Other, requirement: "Lain".into(), verify: String::new() },
        ];
        let problems: Vec<String> = check(&prd).into_iter().map(|p| p.at).collect();
        assert!(problems.contains(&"SEC-01".to_string()), "a repeated id: {problems:?}");
        assert!(problems.contains(&"SEC-01.requirement".to_string()), "an empty requirement: {problems:?}");
        assert!(problems.contains(&"SEC-3".to_string()), "a badly shaped id: {problems:?}");
        // A PRD written before R4 reads, without security, and is asked for it.
        let mut old = serde_json::to_value(sample()).unwrap();
        old.as_object_mut().unwrap().remove("security");
        old["technical"].as_object_mut().unwrap().remove("fromStack");
        let read: Prd = serde_json::from_value(old).unwrap();
        assert!(read.security.is_empty() && !read.technical.from_stack);
    }

    // R4: the technologies come from the approved stack, and follow it again on request.
    #[test]
    fn the_technologies_follow_the_approved_stack() {
        use crate::stack::{self, StackLayer, StackLayerKind, StackMode};
        let mut conn = open_in_memory().unwrap();
        let id = project(&mut conn);
        let layers = |backend: &str| stack::StackBody {
            layers: StackLayerKind::ALL
                .iter()
                .map(|k| StackLayer {
                    layer: *k,
                    choice: if *k == StackLayerKind::Backend { backend.into() } else { "Tidak perlu".into() },
                    why: String::new(),
                    fit: String::new(),
                    ease: String::new(),
                    cost: String::new(),
                    alternatives: vec![],
                    by: StatedBy::Owner,
                })
                .collect(),
            summary: String::new(),
        };
        save_generated(&mut conn, &id, &sample(), now()).unwrap();
        assert_eq!(view(&conn, &id).unwrap().stack, None, "no approved stack yet");
        assert!(refresh_stack(&mut conn, &id, now()).is_err());

        stack::save(&mut conn, &id, layers("Laravel"), StackMode::Own, Actor::Owner, now()).unwrap();
        stack::approve(&mut conn, &id, now()).unwrap();
        let with = with_stack(&sample(), &layers("Laravel"));
        assert!(with.technical.applicable && with.technical.from_stack);
        assert_eq!(with.technical.technologies[2], "Backend: Laravel");
        let text = markdown(&with, None);
        assert!(text.contains("## Perencanaan teknis\n\n**Tech stack (disetujui pemilik):**\n\n- Platform: Tidak perlu"));
        assert!(!text.contains("**Teknologi:**"), "the stack is not listed twice");

        // The draft follows the stack on request, and the view says what the stack gives.
        refresh_stack(&mut conn, &id, now()).unwrap();
        let shown = view(&conn, &id).unwrap();
        assert_eq!(shown.draft.unwrap().technical.technologies, shown.stack.clone().unwrap());
        assert_eq!(shown.stack.unwrap()[2], "Backend: Laravel");
    }
}
