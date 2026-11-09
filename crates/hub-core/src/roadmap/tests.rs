use super::*;
use crate::db::open_in_memory;
use crate::prd::tests::sample as sample_prd;
use crate::studio::{create_project, get_project, StudioProjectInput};

fn now() -> Timestamp {
    "2026-11-06T05:00:00Z".parse().unwrap()
}

fn test(name: &str) -> RequiredTest {
    RequiredTest { name: name.into(), kind: TestKind::Unit, how: "cargo test".into() }
}

fn cp(title: &str, features: &[&str], deps: &[&str]) -> PlanCheckpoint {
    PlanCheckpoint {
        code: String::new(),
        title: title.into(),
        goal: format!("Tujuan {title}"),
        description: String::new(),
        features: features.iter().map(|f| f.to_string()).collect(),
        tasks: vec![
            PlanTask { code: String::new(), title: format!("Buat {title}"), detail: String::new(), feature: features.first().map(|f| f.to_string()).unwrap_or_default(), subtasks: vec!["Langkah kecil".into()] },
            PlanTask { title: format!("Uji {title}"), ..Default::default() },
        ],
        depends_on: deps.iter().map(|d| d.to_string()).collect(),
        difficulty: CheckpointDifficulty::Medium,
        risks: vec![],
        acceptance: vec![format!("{title} bisa didemonstrasikan")],
        required_tests: vec![test("Unit test")],
        deliverables: vec![],
        done_when: vec![],
    }
}

/// Three checkpoints covering the sample PRD: FEAT-001 (must), FEAT-002 (should), FEAT-003 (could).
fn plan() -> RoadmapPlan {
    RoadmapPlan {
        checkpoints: vec![
            PlanCheckpoint { code: "CP-01".into(), ..cp("Fondasi", &["FEAT-001"], &[]) },
            PlanCheckpoint { code: "CP-02".into(), ..cp("Izin", &["FEAT-002"], &["CP-01"]) },
            PlanCheckpoint { code: "CP-03".into(), ..cp("Rekap", &["FEAT-003"], &["CP-01", "CP-02"]) },
        ],
    }
}

/// A Studio project with the sample PRD approved as version 1. Returns (project id, PRD version id).
fn setup(conn: &mut Connection) -> (String, String) {
    let (id, _) = create_project(conn, StudioProjectInput { name: "Absensi".into(), idea: String::new() }, Actor::Owner, now()).unwrap();
    crate::prd::save_generated(conn, &id, &sample_prd(), now()).unwrap();
    crate::prd::approve(conn, &id, now()).unwrap();
    let prd = crate::prd::view(conn, &id).unwrap().approved.unwrap().id;
    (id, prd)
}

fn folder() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn approval(dir: &tempfile::TempDir, link: HubLink, access: ClaudeAccess) -> ApproveRoadmap {
    ApproveRoadmap { link, repo_path: dir.path().to_string_lossy().into_owned(), claude_access: access }
}

/// The project with its roadmap generated and approved (new Hub Proyek, report access).
fn approved(conn: &mut Connection, dir: &tempfile::TempDir) -> (String, Vec<String>) {
    let (id, prd) = setup(conn);
    save_generated(conn, &id, &prd, &plan(), now()).unwrap();
    approve(conn, &id, &approval(dir, HubLink::New, ClaudeAccess::Report), now()).unwrap();
    let ids = view(conn, &id).unwrap().approved.unwrap().checkpoints.into_iter().map(|c| c.id).collect();
    (id, ids)
}

fn status(conn: &Connection, id: &str) -> CheckpointStatus {
    checkpoint(conn, id).unwrap().status
}

fn refused<T: std::fmt::Debug>(result: Result<T>, field: &str) {
    match result {
        Err(Error::Invalid { field: f, .. }) if f == field => {}
        other => panic!("expected refusal on {field}, got {other:?}"),
    }
}

fn report(tests: &[(&str, TestResult)]) -> ReportInput {
    ReportInput {
        summary: "Selesai.".into(),
        changes: vec![ReportChange { path: "src/main.rs".into(), what: "fondasi".into() }],
        commit: "a1b2c3d".into(),
        tests: tests
            .iter()
            .map(|(name, result)| ReportTest { name: name.to_string(), command: "cargo test".into(), result: *result, exit_code: Some(0), duration_ms: Some(1200), output_tail: "ok".into() })
            .collect(),
        evidence: vec![],
        not_verified: vec![],
        open_issues: vec![],
        next: String::new(),
    }
}

// ---------- the plan and its rules ----------

#[test]
fn a_good_plan_is_normalized_and_has_no_problems() {
    let normalized = normalize(&plan());
    assert_eq!(check(&normalized, &sample_prd()), vec![]);
    let first = &normalized.checkpoints[0];
    assert_eq!(first.code, "CP-01");
    assert_eq!(first.tasks[1].code, "CP-01.T02");
    assert!(first.done_when.contains(&DOD_TESTS.to_string()) && first.done_when.contains(&DOD_REPORT.to_string()));
    // Normalizing twice changes nothing.
    assert_eq!(normalize(&normalized), normalized);
}

#[test]
fn codes_gemini_skipped_are_renumbered_with_their_dependencies() {
    let mut p = plan();
    p.checkpoints[0].code = "CP-1".into();
    p.checkpoints[1].code = "CP-05".into();
    p.checkpoints[1].depends_on = vec!["CP-1".into()];
    p.checkpoints[2].depends_on = vec!["CP-05".into()];
    let n = normalize(&p);
    assert_eq!(n.checkpoints[1].depends_on, ["CP-01"]);
    assert_eq!(n.checkpoints[2].depends_on, ["CP-02"]);
    assert_eq!(check(&n, &sample_prd()), vec![]);
}

#[test]
fn the_rules_find_what_breaks_a_roadmap() {
    let mut p = normalize(&plan());
    p.checkpoints[0].features = vec!["FEAT-002".into(), "FEAT-009".into()]; // FEAT-001 (must) now nowhere, FEAT-002 twice
    p.checkpoints[1].depends_on = vec!["CP-03".into()];
    p.checkpoints[2].tasks.clear();
    p.checkpoints[2].acceptance.clear();
    p.checkpoints[2].required_tests = vec![test("Unit test"), test("unit test ")];
    p.checkpoints[2].done_when = vec!["Selesai".into()];
    let problems = check(&p, &sample_prd());
    let has = |at: &str, needle: &str| problems.iter().any(|x| x.at == at && x.message.contains(needle));
    assert!(has("FEAT-001", "belum masuk checkpoint"), "{problems:?}");
    assert!(has("FEAT-002", "lebih dari satu checkpoint (CP-01, CP-02)"));
    assert!(has("CP-01.features", "FEAT-009 tidak ada"));
    assert!(has("CP-02.dependsOn", "bukan checkpoint sebelumnya"));
    assert!(has("CP-03.tasks", "satu tugas"));
    assert!(has("CP-03.acceptance", "kriteria penerimaan"));
    assert!(has("CP-03.requiredTests[1].name", "dipakai dua kali"));
    assert!(has("CP-03.doneWhen", "Definition of Done"));
}

#[test]
fn wont_features_counts_and_codes_are_checked() {
    let mut prd = sample_prd();
    prd.features[2].priority = Priority::Wont;
    let p = normalize(&plan());
    assert!(check(&p, &prd).iter().any(|x| x.message.contains("won't")));
    assert!(check(&RoadmapPlan::default(), &sample_prd()).iter().any(|x| x.message.contains("belum ada checkpoint")));
    let many = RoadmapPlan { checkpoints: (0..21).map(|_| cp("X", &[], &[])).collect() };
    assert!(check(&normalize(&many), &sample_prd()).iter().any(|x| x.message.contains("lebih dari 20")));
    let mut wrong = normalize(&plan());
    wrong.checkpoints[1].code = "CP-07".into();
    assert!(check(&wrong, &sample_prd()).iter().any(|x| x.at == "CP-07" && x.message.contains("CP-02")));
}

#[test]
fn a_plan_is_read_strictly_and_a_broken_checkpoint_is_named() {
    let value = serde_json::to_value(plan()).unwrap();
    assert_eq!(read_plan(&value).unwrap(), plan());
    let mut broken = value.clone();
    broken["checkpoints"][1]["surprise"] = serde_json::json!(1);
    let problems = read_plan(&broken).unwrap_err();
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].at, "CP-02");
    assert!(read_plan(&serde_json::json!({})).is_err());
    assert_eq!(check_value(&value, &sample_prd()), vec![]);
}

// ---------- the draft ----------

#[test]
fn a_generated_roadmap_needs_the_approved_prd_and_becomes_the_draft() {
    let mut conn = open_in_memory().unwrap();
    let (id, _) = create_project(&mut conn, StudioProjectInput { name: "Absensi".into(), idea: String::new() }, Actor::Owner, now()).unwrap();
    refused(save_generated(&mut conn, &id, &crate::ids::new_id(), &plan(), now()), "prd");
    crate::prd::save_generated(&mut conn, &id, &sample_prd(), now()).unwrap();
    crate::prd::approve(&mut conn, &id, now()).unwrap();
    let prd = crate::prd::view(&conn, &id).unwrap().approved.unwrap().id;
    refused(save_generated(&mut conn, &id, &crate::ids::new_id(), &plan(), now()), "prdVersion");

    let (problems, _) = save_generated(&mut conn, &id, &prd, &plan(), now()).unwrap();
    assert!(problems.is_empty());
    let v = view(&conn, &id).unwrap();
    let draft = v.draft.unwrap();
    assert_eq!(draft.meta.made_by, StatedBy::Gemini);
    assert_eq!(draft.meta.prd_number, Some(1));
    assert_eq!(draft.checkpoints.len(), 3);
    assert_eq!(draft.checkpoints[0].tasks.len(), 3, "two tasks and one subtask");
    assert_eq!(draft.checkpoints[0].tasks[1].code, "CP-01.T01.1");
    assert_eq!(draft.checkpoints[0].tasks[1].parent_id.as_deref(), Some(draft.checkpoints[0].tasks[0].id.as_str()));
    assert!(v.problems.is_empty() && !v.locked && v.approved.is_none());
    assert_eq!(get_project(&conn, &id).unwrap().stage, StudioStage::Roadmap);

    // A second generation replaces the draft.
    let mut shorter = plan();
    shorter.checkpoints.truncate(1);
    shorter.checkpoints[0].features = vec!["FEAT-001".into(), "FEAT-002".into(), "FEAT-003".into()];
    save_generated(&mut conn, &id, &prd, &shorter, now()).unwrap();
    assert_eq!(view(&conn, &id).unwrap().draft.unwrap().checkpoints.len(), 1);
    let roadmaps: i64 = conn.query_row("SELECT count(*) FROM studio_roadmaps", [], |r| r.get(0)).unwrap();
    assert_eq!(roadmaps, 1);
}

#[test]
fn the_draft_is_edited_added_to_removed_from_and_reordered() {
    let mut conn = open_in_memory().unwrap();
    let (id, prd) = setup(&mut conn);
    save_generated(&mut conn, &id, &prd, &plan(), now()).unwrap();
    let ids: Vec<String> = view(&conn, &id).unwrap().draft.unwrap().checkpoints.iter().map(|c| c.id.clone()).collect();

    // Edit: the code stays with the position whatever the input says; tasks get fresh codes.
    let mut edited = plan_of(&checkpoint(&conn, &ids[1]).unwrap());
    edited.code = "CP-99".into();
    edited.title = "  Izin dan sakit ".into();
    edited.tasks.push(PlanTask { title: "Tambah alasan sakit".into(), ..Default::default() });
    edited.done_when.clear();
    update_checkpoint(&mut conn, &ids[1], &edited, now()).unwrap();
    let c = checkpoint(&conn, &ids[1]).unwrap();
    assert_eq!((c.code.as_str(), c.title.as_str()), ("CP-02", "Izin dan sakit"));
    assert_eq!(c.tasks.last().unwrap().code, "CP-02.T03");
    assert!(c.done_when.contains(&DOD_REPORT.to_string()), "the DoD lines come back");
    refused(update_checkpoint(&mut conn, &ids[1], &PlanCheckpoint { title: " ".into(), ..Default::default() }, now()), "title");

    // Add after CP-01: the new one is CP-02, the rest move down with their dependencies.
    let (added, _) = add_checkpoint(&mut conn, &id, Some(&ids[0]), now()).unwrap();
    let d = view(&conn, &id).unwrap().draft.unwrap();
    let codes: Vec<(&str, &str)> = d.checkpoints.iter().map(|c| (c.code.as_str(), c.title.as_str())).collect();
    assert_eq!(codes, [("CP-01", "Fondasi"), ("CP-02", "Checkpoint baru"), ("CP-03", "Izin dan sakit"), ("CP-04", "Rekap")]);
    assert_eq!(d.checkpoints[3].depends_on, ["CP-01", "CP-03"]);
    assert_eq!(d.checkpoints[2].tasks[0].code, "CP-03.T01", "task codes follow");
    assert_eq!(d.checkpoints[2].tasks[1].code, "CP-03.T01.1");
    let problems = view(&conn, &id).unwrap().problems;
    assert!(problems.iter().any(|p| p.at == "CP-02.tasks"), "the blank one shows its problems: {problems:?}");

    // Move Rekap (CP-04) to the top: its dependencies now point forward, a problem.
    move_checkpoint(&mut conn, &ids[2], 1, now()).unwrap();
    let d = view(&conn, &id).unwrap().draft.unwrap();
    assert_eq!(d.checkpoints[0].title, "Rekap");
    assert_eq!(d.checkpoints[0].depends_on, ["CP-02", "CP-04"]);
    assert!(view(&conn, &id).unwrap().problems.iter().any(|p| p.at == "CP-01.dependsOn"));
    refused(move_checkpoint(&mut conn, &ids[2], 9, now()), "to");

    // Remove Fondasi: dependencies on it disappear.
    move_checkpoint(&mut conn, &ids[2], 4, now()).unwrap();
    remove_checkpoint(&mut conn, &added, now()).unwrap();
    remove_checkpoint(&mut conn, &ids[0], now()).unwrap();
    let d = view(&conn, &id).unwrap().draft.unwrap();
    let codes: Vec<(&str, &str)> = d.checkpoints.iter().map(|c| (c.code.as_str(), c.title.as_str())).collect();
    assert_eq!(codes, [("CP-01", "Izin dan sakit"), ("CP-02", "Rekap")]);
    assert_eq!(d.checkpoints[0].depends_on, Vec::<String>::new());
    assert_eq!(d.checkpoints[1].depends_on, ["CP-01"]);

    discard_draft(&mut conn, &id, now()).unwrap();
    assert!(view(&conn, &id).unwrap().draft.is_none());
}

// ---------- approval ----------

#[test]
fn approval_needs_no_problems_and_an_existing_folder_and_links_a_new_proyek() {
    let mut conn = open_in_memory().unwrap();
    let dir = folder();
    let (id, prd) = setup(&mut conn);
    let mut broken = plan();
    broken.checkpoints[0].features.clear();
    save_generated(&mut conn, &id, &prd, &broken, now()).unwrap();
    refused(approve(&mut conn, &id, &approval(&dir, HubLink::New, ClaudeAccess::Report), now()), "roadmap");

    save_generated(&mut conn, &id, &prd, &plan(), now()).unwrap();
    let gone = ApproveRoadmap { repo_path: dir.path().join("missing").to_string_lossy().into_owned(), ..approval(&dir, HubLink::New, ClaudeAccess::Read) };
    refused(approve(&mut conn, &id, &gone, now()), "repoPath");
    let relative = ApproveRoadmap { repo_path: "repo\\src".into(), ..approval(&dir, HubLink::New, ClaudeAccess::Read) };
    refused(approve(&mut conn, &id, &relative, now()), "repoPath");

    let changes = approve(&mut conn, &id, &approval(&dir, HubLink::New, ClaudeAccess::Read), now()).unwrap();
    assert!(changes.iter().any(|c| c.entity == Entity::Project));
    let p = get_project(&conn, &id).unwrap();
    assert_eq!(p.stage, StudioStage::Build);
    assert_eq!(p.claude_access, ClaudeAccess::Read);
    // F3: the new Proyek is named after the PRD's product, not the Studio project ("Absensi").
    assert_eq!(p.project_name.as_deref(), Some("Absensi Lab"));
    assert_eq!(p.repo_path.as_deref(), Some(normalize_folder(&dir.path().to_string_lossy()).unwrap().as_str()));
    let description: String = conn.query_row("SELECT description FROM projects WHERE id = ?1", [p.project_id.unwrap()], |r| r.get(0)).unwrap();
    assert_eq!(description, sample_prd().identity.summary);
    let v = view(&conn, &id).unwrap();
    assert!(v.draft.is_none());
    let a = v.approved.unwrap();
    assert!(a.checkpoints.iter().all(|c| c.status == CheckpointStatus::Planned));
    assert_eq!(a.meta.approved_at.as_deref(), Some("2026-11-06T05:00:00Z"));
}

#[test]
fn the_new_proyek_takes_the_prds_product_name_and_the_view_says_so_before_approval() {
    let mut conn = open_in_memory().unwrap();
    let dir = folder();
    let (id, _) = create_project(
        &mut conn,
        StudioProjectInput { name: "Aku mau membuat aplikasi web peminjaman alat Lab TJKT di…".into(), idea: String::new() },
        Actor::Owner,
        now(),
    )
    .unwrap();
    let mut prd = sample_prd();
    prd.identity.name = "  Aplikasi Peminjaman Alat Lab TJKT  ".into();
    crate::prd::save_generated(&mut conn, &id, &prd, now()).unwrap();
    crate::prd::approve(&mut conn, &id, now()).unwrap();
    let source = crate::prd::view(&conn, &id).unwrap().approved.unwrap();
    // The approval dialog reads the name from the view, so it says what approval will make.
    assert_eq!(view(&conn, &id).unwrap().prd.unwrap().proyek_name, "Aplikasi Peminjaman Alat Lab TJKT");
    save_generated(&mut conn, &id, &source.id, &plan(), now()).unwrap();
    approve(&mut conn, &id, &approval(&dir, HubLink::New, ClaudeAccess::NoAccess), now()).unwrap();
    assert_eq!(get_project(&conn, &id).unwrap().project_name.as_deref(), Some("Aplikasi Peminjaman Alat Lab TJKT"));

    // A PRD without a product name keeps the Studio project's name.
    prd.identity.name = " ".into();
    assert_eq!(proyek_name(&prd, "Absensi"), "Absensi");
}

#[test]
fn an_existing_proyek_can_be_linked_but_not_one_another_studio_project_holds() {
    let mut conn = open_in_memory().unwrap();
    let dir = folder();
    let input = crate::projects::ProjectInput { name: "Absensi lama".into(), ..Default::default() };
    let (hub, _) = crate::projects::create(&mut conn, input, Actor::Owner, now()).unwrap();
    let (first, prd) = setup(&mut conn);
    save_generated(&mut conn, &first, &prd, &plan(), now()).unwrap();
    refused(approve(&mut conn, &first, &approval(&dir, HubLink::Existing { project_id: crate::ids::new_id() }, ClaudeAccess::NoAccess), now()), "projectId");
    approve(&mut conn, &first, &approval(&dir, HubLink::Existing { project_id: hub.clone() }, ClaudeAccess::NoAccess), now()).unwrap();
    assert_eq!(get_project(&conn, &first).unwrap().project_id.as_deref(), Some(hub.as_str()));

    let (second, prd2) = setup(&mut conn);
    save_generated(&mut conn, &second, &prd2, &plan(), now()).unwrap();
    refused(approve(&mut conn, &second, &approval(&dir, HubLink::Existing { project_id: hub.clone() }, ClaudeAccess::NoAccess), now()), "projectId");

    let link = link_of(&conn, &hub).unwrap().unwrap();
    assert_eq!((link.studio_project_id.as_str(), link.completed, link.total, link.active_code), (first.as_str(), 0, 3, None));
}

#[test]
fn a_roadmap_is_replaced_only_until_a_checkpoint_starts() {
    let mut conn = open_in_memory().unwrap();
    let dir = folder();
    let (id, cps) = approved(&mut conn, &dir);
    let prd = view(&conn, &id).unwrap().prd.unwrap().id;
    let hub = get_project(&conn, &id).unwrap().project_id.unwrap();

    // Ready is not started: "Ubah roadmap" copies the approved one, approving it supersedes.
    make_ready(&mut conn, &cps[0], Actor::Owner, now()).unwrap();
    draft_from_approved(&mut conn, &id, now()).unwrap();
    let draft = view(&conn, &id).unwrap().draft.unwrap();
    assert_eq!(draft.meta.made_by, StatedBy::Owner);
    assert_eq!(draft.checkpoints.len(), 3);
    assert!(draft.checkpoints.iter().all(|c| c.status == CheckpointStatus::Planned));
    approve(&mut conn, &id, &approval(&dir, HubLink::Existing { project_id: hub.clone() }, ClaudeAccess::Report), now()).unwrap();
    let v = view(&conn, &id).unwrap();
    assert_eq!(v.superseded.len(), 1);
    assert!(!v.locked);
    let cps: Vec<String> = v.approved.unwrap().checkpoints.into_iter().map(|c| c.id).collect();

    // Once a checkpoint is In Progress, the roadmap is locked.
    make_ready(&mut conn, &cps[0], Actor::Owner, now()).unwrap();
    start_checkpoint(&mut conn, &cps[0], Actor::Owner, now()).unwrap();
    assert!(view(&conn, &id).unwrap().locked);
    refused(save_generated(&mut conn, &id, &prd, &plan(), now()), "roadmap");
    refused(draft_from_approved(&mut conn, &id, now()), "roadmap");

    // It grows at its end instead.
    let (added, problems, _) = append_checkpoint(&mut conn, &id, &cp("Perbaikan", &["FEAT-001"], &["CP-01"]), now()).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
    let c = checkpoint(&conn, &added).unwrap();
    assert_eq!((c.code.as_str(), c.status), ("CP-04", CheckpointStatus::Planned));
    let (none, problems, _) = append_checkpoint(&mut conn, &id, &cp("Salah", &[], &["CP-09"]), now()).unwrap();
    assert!(none.is_empty() && problems.iter().any(|p| p.at == "CP-05.dependsOn"));
}

#[test]
fn claude_access_needs_a_folder_and_the_folder_must_exist() {
    let mut conn = open_in_memory().unwrap();
    let (id, _) = setup(&mut conn);
    refused(set_claude_access(&mut conn, &id, ClaudeAccess::Read, now()), "repoPath");
    set_claude_access(&mut conn, &id, ClaudeAccess::NoAccess, now()).unwrap();
    let dir = folder();
    refused(set_repo_path(&mut conn, &id, &dir.path().join("x").to_string_lossy(), now()), "repoPath");
    set_repo_path(&mut conn, &id, &dir.path().to_string_lossy(), now()).unwrap();
    set_claude_access(&mut conn, &id, ClaudeAccess::Report, now()).unwrap();
    assert_eq!(get_project(&conn, &id).unwrap().claude_access, ClaudeAccess::Report);
}

#[test]
fn folders_are_normalized_and_git_is_noticed() {
    let dir = folder();
    let base = dir.path().to_string_lossy().into_owned();
    let check = inspect_folder(&format!("  {base}\\ ")).unwrap();
    assert_eq!(check.path, base);
    assert!(check.exists && !check.git);
    std::fs::create_dir(dir.path().join(".git")).unwrap();
    assert!(inspect_folder(&base).unwrap().git);
    assert!(!inspect_folder(&format!("{base}\\nope")).unwrap().exists);
    assert_eq!(normalize_folder("C:\\").unwrap(), "C:\\");
    assert!(normalize_folder("C:").is_err());
    assert!(normalize_folder("relative\\path").is_err());
    assert!(normalize_folder("C:\\repo\\..\\other").is_err());
    assert!(normalize_folder("").is_err());
}

// ---------- statuses (docs/studio.md, "Checkpoint statuses") ----------

#[test]
fn ready_needs_the_owner_completed_dependencies_and_no_other_active_checkpoint() {
    let mut conn = open_in_memory().unwrap();
    let dir = folder();
    let (id, cps) = approved(&mut conn, &dir);
    refused(make_ready(&mut conn, &cps[0], Actor::Claude, now()), "actor");
    refused(make_ready(&mut conn, &cps[1], Actor::Owner, now()), "dependsOn");
    make_ready(&mut conn, &cps[0], Actor::Owner, now()).unwrap();
    assert_eq!(status(&conn, &cps[0]), CheckpointStatus::Ready);
    assert!(checkpoint(&conn, &cps[0]).unwrap().ready_at.is_some());
    refused(make_ready(&mut conn, &cps[0], Actor::Owner, now()), "status");

    // A draft's checkpoint is never Ready.
    let (other, prd) = setup(&mut conn);
    save_generated(&mut conn, &other, &prd, &plan(), now()).unwrap();
    let draft_cp = view(&conn, &other).unwrap().draft.unwrap().checkpoints[0].id.clone();
    refused(make_ready(&mut conn, &draft_cp, Actor::Owner, now()), "roadmap");

    // A checkpoint with no dependencies still waits while another is active.
    let (added, _, _) = append_checkpoint(&mut conn, &id, &cp("Lepas", &[], &[]), now()).unwrap();
    refused(make_ready(&mut conn, &added, Actor::Owner, now()), "active");
}

#[test]
fn every_transition_and_every_refused_one() {
    let mut conn = open_in_memory().unwrap();
    let dir = folder();
    let (id, cps) = approved(&mut conn, &dir);
    let first = &cps[0];
    make_ready(&mut conn, first, Actor::Owner, now()).unwrap();

    // Ready: only start.
    refused(block_checkpoint(&mut conn, first, "x", Actor::Claude, now()), "status");
    refused(submit_report(&mut conn, first, &report(&[("Unit test", TestResult::Passed)]), Actor::Claude, now()), "status");
    refused(start_checkpoint(&mut conn, first, Actor::Gemini, now()), "actor");
    start_checkpoint(&mut conn, first, Actor::Claude, now()).unwrap();
    assert_eq!(status(&conn, first), CheckpointStatus::InProgress);

    // Tasks move while In Progress.
    let task = checkpoint(&conn, first).unwrap().tasks[0].id.clone();
    set_task_status(&mut conn, &task, StudioTaskStatus::Done, Some("selesai"), Actor::Claude, now()).unwrap();
    let t = checkpoint(&conn, first).unwrap().tasks[0].clone();
    assert_eq!((t.status, t.note.as_str(), t.changed_by), (StudioTaskStatus::Done, "selesai", Maker::Claude));
    let later_task = checkpoint(&conn, &cps[1]).unwrap().tasks[0].id.clone();
    refused(set_task_status(&mut conn, &later_task, StudioTaskStatus::Doing, None, Actor::Claude, now()), "status");

    // Blocked, with the blocker open; Claude cannot resume until the owner answers.
    refused(block_checkpoint(&mut conn, first, " ", Actor::Claude, now()), "reason");
    block_checkpoint(&mut conn, first, "Pakai SQLite atau Postgres?", Actor::Claude, now()).unwrap();
    assert_eq!(status(&conn, first), CheckpointStatus::Blocked);
    let open = requests(&conn, &id, true).unwrap();
    assert_eq!((open.len(), open[0].kind, open[0].made_by), (1, RequestKind::Blocker, Maker::Claude));
    assert_eq!(get_project(&conn, &id).unwrap().waiting, 1);
    refused(resume_checkpoint(&mut conn, first, None, Actor::Claude, now()), "blocker");
    refused(resume_checkpoint(&mut conn, first, Some("SQLite"), Actor::Claude, now()), "actor");
    answer_request(&mut conn, &open[0].id, "SQLite", now()).unwrap();
    refused(answer_request(&mut conn, &open[0].id, "lagi", now()), "request");
    resume_checkpoint(&mut conn, first, None, Actor::Claude, now()).unwrap();
    assert_eq!(status(&conn, first), CheckpointStatus::InProgress);

    // The owner may answer while resuming.
    block_checkpoint(&mut conn, first, "Butuh akses", Actor::Owner, now()).unwrap();
    resume_checkpoint(&mut conn, first, Some("Sudah diberi"), Actor::Owner, now()).unwrap();
    assert!(requests(&conn, &id, true).unwrap().is_empty());

    // In Review needs a report that reports every required test.
    refused(submit_report(&mut conn, first, &report(&[]), Actor::Claude, now()), "report");
    let (report_id, _) = submit_report(&mut conn, first, &report(&[("Unit test", TestResult::Passed)]), Actor::Claude, now()).unwrap();
    assert_eq!(status(&conn, first), CheckpointStatus::InReview);
    assert!(conn.execute("UPDATE studio_reports SET summary = 'x' WHERE id = ?1", [&report_id]).is_err(), "a report never changes");
    let review = requests(&conn, &id, true).unwrap();
    assert_eq!((review.len(), review[0].kind), (1, RequestKind::Review));

    // Only the owner approves or asks for changes.
    refused(approve_checkpoint(&mut conn, first, Actor::Claude, now()), "actor");
    refused(request_changes(&mut conn, first, "x", Actor::Claude, now()), "actor");
    refused(request_changes(&mut conn, first, "  ", Actor::Owner, now()), "note");
    request_changes(&mut conn, first, "Tambahkan tes login gagal.", Actor::Owner, now()).unwrap();
    assert_eq!(status(&conn, first), CheckpointStatus::InProgress);
    let open = requests(&conn, &id, true).unwrap();
    assert_eq!((open.len(), open[0].kind, open[0].made_by), (1, RequestKind::Changes, Maker::Owner));
    assert_eq!(get_project(&conn, &id).unwrap().waiting, 0, "the owner's own request waits for Claude, not the owner");

    // The next report answers the changes; approval completes it, for good.
    submit_report(&mut conn, first, &report(&[("Unit test", TestResult::Passed)]), Actor::Claude, now()).unwrap();
    assert!(requests(&conn, &id, true).unwrap().iter().all(|r| r.kind == RequestKind::Review));
    approve_checkpoint(&mut conn, first, Actor::Owner, now()).unwrap();
    let done = checkpoint(&conn, first).unwrap();
    assert_eq!(done.status, CheckpointStatus::Completed);
    assert!(done.completed_at.is_some() && done.started_at.is_some() && done.submitted_at.is_some());
    assert!(requests(&conn, &id, true).unwrap().is_empty());
    refused(start_checkpoint(&mut conn, first, Actor::Owner, now()), "status");
    refused(request_changes(&mut conn, first, "x", Actor::Owner, now()), "status");
    assert_eq!(reports(&conn, first).unwrap().len(), 2);

    // The next one can be made Ready now.
    make_ready(&mut conn, &cps[1], Actor::Owner, now()).unwrap();
    let link = link_of(&conn, &get_project(&conn, &id).unwrap().project_id.unwrap()).unwrap().unwrap();
    assert_eq!((link.completed, link.total, link.active_code.as_deref(), link.active_status), (1, 3, Some("CP-02"), Some(CheckpointStatus::Ready)));
}

#[test]
fn the_owner_can_build_a_checkpoint_alone_with_a_short_report() {
    let mut conn = open_in_memory().unwrap();
    let dir = folder();
    let (_, cps) = approved(&mut conn, &dir);
    make_ready(&mut conn, &cps[0], Actor::Owner, now()).unwrap();
    start_checkpoint(&mut conn, &cps[0], Actor::Owner, now()).unwrap();
    let short = ReportInput {
        summary: "Aku kerjakan sendiri.".into(),
        changes: vec![],
        commit: String::new(),
        tests: vec![ReportTest { name: "Unit test".into(), command: String::new(), result: TestResult::NotRun, exit_code: None, duration_ms: None, output_tail: String::new() }],
        evidence: vec![],
        not_verified: vec!["Unit test belum ada, dicek manual.".into()],
        open_issues: vec![],
        next: String::new(),
    };
    submit_report(&mut conn, &cps[0], &short, Actor::Owner, now()).unwrap();
    approve_checkpoint(&mut conn, &cps[0], Actor::Owner, now()).unwrap();
    let r = &reports(&conn, &cps[0]).unwrap()[0];
    assert_eq!((r.made_by, r.kind), (Maker::Owner, ReportKind::Submission));
    assert_eq!(r.body, short);
}

// ---------- reports ----------

#[test]
fn a_report_must_be_checkable() {
    let required = [test("Unit test"), test("Build")];
    let good = report(&[("Unit test", TestResult::Passed), ("build", TestResult::Passed)]);
    assert_eq!(check_report(&good, &required), vec![]);

    let mut bad = report(&[("Unit test", TestResult::Failed)]);
    bad.summary = String::new();
    bad.commit = "xyz".into();
    bad.changes = vec![
        ReportChange { path: "../secret.txt".into(), what: String::new() },
        ReportChange { path: "C:\\Windows\\a".into(), what: String::new() },
        ReportChange { path: "/etc/passwd".into(), what: String::new() },
    ];
    bad.evidence = vec![ReportEvidence { kind: EvidenceKind::Link, value: "javascript:alert(1)".into() }];
    bad.tests[0].output_tail = "x".repeat(4001);
    let problems = check_report(&bad, &required);
    let has = |at: &str, needle: &str| problems.iter().any(|p| p.at == at && p.message.contains(needle));
    assert!(has("summary", "wajib"), "{problems:?}");
    assert!(has("commit", "heksadesimal"));
    for i in 0..3 {
        assert!(has(&format!("changes[{i}].path"), "relatif"));
    }
    assert!(has("evidence[0].value", "http"));
    assert!(has("tests", "\"Build\" belum dilaporkan"));
    assert!(has("notVerified", "Unit test"));
    assert!(has("tests[0].outputTail", "4000"));

    let mut huge = good.clone();
    huge.open_issues = vec!["x".repeat(400); 40];
    huge.not_verified = vec!["y".repeat(400); 40];
    huge.tests[0].output_tail = "z".repeat(4000);
    huge.changes = (0..200).map(|i| ReportChange { path: format!("src/file{i}.rs"), what: "w".repeat(200) }).collect();
    assert!(check_report(&huge, &required).iter().any(|p| p.message.contains("64 KB")));
}

// ---------- the prompt ----------

#[test]
fn the_prompt_is_filled_from_ready_until_completed() {
    let mut conn = open_in_memory().unwrap();
    let dir = folder();
    let (id, cps) = approved(&mut conn, &dir);
    refused(implementation_prompt(&conn, &cps[0]), "status");
    conn.execute("UPDATE studio_projects SET name = 'Absensi\nLab' WHERE id = ?1", [&id]).unwrap();
    make_ready(&mut conn, &cps[0], Actor::Owner, now()).unwrap();
    let text = implementation_prompt(&conn, &cps[0]).unwrap();
    let repo = get_project(&conn, &id).unwrap().repo_path.unwrap();
    assert!(text.starts_with(&format!("Kerjakan proyek Hub Studio \"Absensi Lab\" (id {id}) di folder {repo}.")), "{text}");
    for part in [
        format!("1. Jalankan /hub-connect {id} CP-01."),
        "2. Baca PRD versi 1 yang sudah disetujui dan checkpoint CP-01 (Fondasi) lewat Hub.".to_string(),
        "3. Kerjakan hanya CP-01.".to_string(),
        // U5a: what is fixed and what is Claude's to decide.
        FREEDOM.to_string(),
        "hub_submit_report".to_string(),
        "6. Setelah laporan terkirim, berhenti.".to_string(),
    ] {
        assert!(text.contains(&part), "missing {part}\n{text}");
    }
    assert!(!text.contains('\u{2014}'), "no em dash");
}

// R4: "Mandiri", the whole of it in one text for working without the bridge.
#[test]
fn the_standalone_prompt_carries_the_rules_the_checkpoint_the_plan_the_stack_and_the_prd() {
    use crate::stack::{self, StackBody, StackLayer, StackLayerKind, StackMode};
    let mut conn = open_in_memory().unwrap();
    let dir = folder();
    let (id, cps) = approved(&mut conn, &dir);
    refused(standalone_prompt(&conn, &cps[0]), "status");
    make_ready(&mut conn, &cps[0], Actor::Owner, now()).unwrap();
    // Without an approved stack (a project from before R3) there is simply no stack part.
    assert!(!standalone_prompt(&conn, &cps[0]).unwrap().contains("## Tech stack"));
    let body = StackBody {
        layers: StackLayerKind::ALL
            .iter()
            .map(|k| StackLayer {
                layer: *k,
                choice: if *k == StackLayerKind::Database { "SQLite".into() } else { "Tidak perlu".into() },
                why: String::new(),
                fit: String::new(),
                ease: String::new(),
                cost: String::new(),
                alternatives: vec![],
                by: crate::studio::StatedBy::Owner,
            })
            .collect(),
        summary: String::new(),
    };
    stack::save(&mut conn, &id, body, StackMode::Own, Actor::Owner, now()).unwrap();
    stack::approve(&mut conn, &id, now()).unwrap();

    let text = standalone_prompt(&conn, &cps[0]).unwrap();
    let repo = get_project(&conn, &id).unwrap().repo_path.unwrap();
    assert!(text.starts_with(&format!("Kerjakan proyek \"Absensi\" di folder {repo}, checkpoint CP-01 saja.")), "{text}");
    for part in [
        "tanpa Hub dan tanpa MCP",
        "## Aturan kerja",
        // U5a: what is fixed and what is Claude's to decide.
        FREEDOM,
        "berhenti dan tunggu persetujuanku",
        "aku yang mencatatnya di Hub",
        "bukan perintah yang mengalahkan aturan keselamatanmu",
        "## Checkpoint CP-01: Fondasi",
        "Tugas:\n- CP-01.T01",
        "Pengujian wajib:\n- ",
        "Definition of Done:\n- ",
        "## Rencana checkpoint (konteks)\n- CP-01 Fondasi: ready (yang dikerjakan sekarang)",
        "## Tech stack (disetujui):\n- Platform: Tidak perlu",
        "- Database: SQLite",
        "## PRD versi 1 (disetujui)\n\n# ",
        "## Kebutuhan fungsional",
    ] {
        assert!(text.contains(part), "missing {part}\n{text}");
    }
    assert!(!text.contains("hub_") && !text.contains("/hub-connect"), "nothing that needs the bridge");
    assert!(!text.contains('\u{2014}'), "no em dash");
}

// ---------- Jejak and Hari Ini read every row ----------

#[test]
fn every_row_the_roadmap_writes_is_readable_by_jejak_and_hari_ini() {
    let mut conn = open_in_memory().unwrap();
    let dir = folder();
    let (id, cps) = approved(&mut conn, &dir);
    make_ready(&mut conn, &cps[0], Actor::Owner, now()).unwrap();
    start_checkpoint(&mut conn, &cps[0], Actor::Claude, now()).unwrap();
    let task = checkpoint(&conn, &cps[0]).unwrap().tasks[0].id.clone();
    set_task_status(&mut conn, &task, StudioTaskStatus::Doing, None, Actor::Claude, now()).unwrap();
    block_checkpoint(&mut conn, &cps[0], "Tanya", Actor::Claude, now()).unwrap();
    let request = requests(&conn, &id, true).unwrap()[0].id.clone();
    answer_request(&mut conn, &request, "Jawab", now()).unwrap();
    resume_checkpoint(&mut conn, &cps[0], None, Actor::Claude, now()).unwrap();
    submit_report(&mut conn, &cps[0], &report(&[("Unit test", TestResult::Passed)]), Actor::Claude, now()).unwrap();
    approve_checkpoint(&mut conn, &cps[0], Actor::Owner, now()).unwrap();
    set_claude_access(&mut conn, &id, ClaudeAccess::NoAccess, now()).unwrap();

    let rows = crate::activity::recent(&conn, 200, true).unwrap();
    assert!(rows.iter().any(|r| r.verb == Verb::Approved && r.entity == Entity::StudioRoadmap));
    assert!(rows.iter().any(|r| r.verb == Verb::Approved && r.entity == Entity::StudioCheckpoint));
    assert!(rows.iter().any(|r| r.actor == Actor::Claude && r.entity == Entity::StudioTask));
    assert!(rows.iter().any(|r| r.entity == Entity::StudioReport));
    crate::insights::trail(&conn, &crate::insights::TrailQuery::default()).unwrap();
    let ctx = crate::context::Ctx::load(&conn, now()).unwrap();
    crate::today::today(&conn, ctx.now, &ctx.tz, ctx.waking).unwrap();
}
