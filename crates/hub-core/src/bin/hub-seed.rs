//! Fill an empty Hub with a demo week, to look at Hub without touching the owner's data.
//!
//!     cargo run -p hub-core --bin hub-seed -- <data folder>
//!
//! Then start Hub with `HUB_DATA_DIR` set to the same folder. It refuses a Hub that already has
//! tasks, so it never mixes demo rows into the owner's real data. Every row goes through the same
//! core functions the app uses, so the demo also has its Jejak.

use std::path::PathBuf;
use std::process::ExitCode;

use hub_core::areas;
use hub_core::events::{self, EventInput};
use hub_core::files::{self, CollectionInput, LinkTarget};
use hub_core::identity;
use hub_core::learning::{self, CtfInput, LearningInput, SessionInput};
use hub_core::model::{Actor, Difficulty, Domain, LearningKind, LearningStatus, LinkKind, Pigment, RoutineKind};
use hub_core::projects::{self, ProjectInput};
use hub_core::recurrence::{Day as WeekDay, Rule};
use hub_core::reminders::{self, ReminderInput};
use hub_core::routines::{self, RoutineInput};
use hub_core::tasks::{self, TaskInput};
use hub_core::time::{date_text, Day};
use hub_core::{settings, Hub, Paths, Result};
use jiff::{Span, Timestamp};

fn main() -> ExitCode {
    let Some(root) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: hub-seed <data folder>");
        return ExitCode::from(2);
    };
    match seed(root) {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("hub-seed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn seed(root: PathBuf) -> Result<String> {
    let hub = Hub::open(Paths::under(&root))?;
    let has_tasks: i64 = hub.read(|conn| Ok(conn.query_row("SELECT count(*) FROM tasks", [], |row| row.get(0))?))?;
    if has_tasks > 0 {
        return Ok(format!("{} already has tasks: nothing seeded.", root.display()));
    }
    let tz = hub.read(|conn| Ok(hub_core::time::zone(settings::load(conn)?.time_zone.as_deref())))?;
    let now = Timestamp::now();
    let day = Day::containing(now, &tz)?;
    let date = date_text(day.date);
    let in_days = |n: i64| date_text(day.date.checked_add(Span::new().days(n)).expect("in range"));
    // The timetable started on the Monday of this week.
    let monday = date_text(
        day.date
            .checked_sub(Span::new().days(i64::from(day.date.weekday().to_monday_zero_offset())))
            .expect("in range"),
    );

    hub.write(|conn| {
        areas::ensure_defaults(conn, now)?;
        let area = |conn: &rusqlite::Connection, name: &str| -> Option<String> {
            areas::list(conn).ok()?.into_iter().find(|a| a.name == name).map(|a| a.id)
        };
        let school_area = area(conn, "Sekolah");
        let cyber_area = area(conn, "Belajar Cyber");
        let own_area = area(conn, "Proyek Pribadi");
        let project = |conn: &mut rusqlite::Connection, name: &str, pigment, area: &Option<String>, target: Option<String>| {
            projects::create(
                conn,
                ProjectInput { name: name.into(), pigment, area_id: area.clone(), target_date: target, ..Default::default() },
                Actor::Owner,
                now,
            )
            .map(|(id, _)| id)
        };
        let lab = project(conn, "Laporan Lab Jaringan", Pigment::Toska, &school_area, Some(in_days(1)))?;
        let hub_project = project(conn, "Hub", Pigment::Kunyit, &own_area, None)?;
        let learning = project(conn, "Jalur Jr Penetration Tester", Pigment::Lavender, &cyber_area, None)?;
        let school = project(conn, "Presentasi KIK", Pigment::Lazuardi, &school_area, Some(in_days(6)))?;
        projects::add_milestone(conn, &lab, "Topologi dan konfigurasi", None, Actor::Owner, now)?;
        projects::add_milestone(conn, &lab, "Laporan ditulis", Some(&in_days(1)), Actor::Owner, now)?;

        let lesson = |title: &str, days: &[WeekDay], start: &str, minutes: u32, location: &str, pigment| RoutineInput {
            title: title.into(),
            kind: RoutineKind::Lesson,
            rule: Rule::weekly(days),
            start_time: start.into(),
            duration_min: minutes,
            starts_on: monday.clone(),
            ends_on: None,
            location: location.into(),
            pigment,
            project_id: None,
        };
        use WeekDay::{Fr, Mo, Th, Tu, We};
        for input in [
            lesson("Matematika", &[Mo, Th], "07:00", 90, "Ruang 12", Pigment::Lazuardi),
            lesson("Bahasa Inggris", &[Mo, We, Th], "08:30", 90, "Ruang 12", Pigment::Lazuardi),
            lesson("Praktikum Jaringan", &[Tu, Th], "10:15", 135, "Lab TJKT 2", Pigment::Toska),
            lesson("Produktif TJKT", &[Mo, We, Th, Fr], "13:00", 90, "Lab TJKT 2", Pigment::Toska),
        ] {
            routines::create(conn, input, Actor::Owner, now)?;
        }
        routines::create(
            conn,
            RoutineInput {
                title: "Belajar Cyber".into(),
                kind: RoutineKind::Study,
                rule: Rule::weekly(&[Mo, We, Th, Fr]),
                start_time: "19:00".into(),
                duration_min: 90,
                starts_on: monday.clone(),
                ends_on: None,
                location: "TryHackMe".into(),
                pigment: Pigment::Lavender,
                project_id: Some(learning.clone()),
            },
            Actor::Owner,
            now,
        )?;

        let task = |conn: &mut rusqlite::Connection, title: &str, project: &str, input: TaskInput, actor| {
            tasks::create(conn, TaskInput { title: title.into(), project_id: Some(project.into()), ..input }, actor, now).map(|(id, _)| id)
        };
        let report = task(conn, "Laporan Praktikum 5: VLAN dan Trunking", &lab, TaskInput { due_date: Some(in_days(1)), due_time: Some("23:59".into()), priority: 2, estimate_min: Some(90), ..Default::default() }, Actor::Owner)?;
        let nmap = task(conn, "THM: Nmap Live Host Discovery", &learning, TaskInput { do_date: Some(date.clone()), priority: 1, estimate_min: Some(45), ..Default::default() }, Actor::Owner)?;
        let design = task(conn, "Hub: kunci arah desain", &hub_project, TaskInput { do_date: Some(date.clone()), ..Default::default() }, Actor::Owner)?;
        task(conn, "Revisi slide presentasi KIK", &school, TaskInput { do_date: Some(date.clone()), ..Default::default() }, Actor::Owner)?;
        task(conn, "Bawa kabel UTP ke lab besok", &school, TaskInput { do_date: Some(date.clone()), ..Default::default() }, Actor::Ultron)?;
        task(conn, "Latihan soal subnetting /27", &learning, TaskInput { do_date: Some(in_days(2)), ..Default::default() }, Actor::Owner)?;
        tasks::create(conn, TaskInput { title: "Ide: dashboard homelab".into(), ..Default::default() }, Actor::Owner, now)?;
        tasks::create(conn, TaskInput { title: "Topologi VLAN".into(), parent_id: Some(report.clone()), ..Default::default() }, Actor::Owner, now)?;
        tasks::create(conn, TaskInput { title: "Konfigurasi trunk".into(), parent_id: Some(report.clone()), ..Default::default() }, Actor::Owner, now)?;
        tasks::set_focus(conn, &date, &[&report, &nmap, &design], Actor::Owner, now)?;
        tasks::complete(conn, &design, Actor::Owner, now)?;

        events::create(
            conn,
            EventInput { title: "Kerjakan Laporan Lab".into(), date: date.clone(), start_minute: 16 * 60, end_minute: 17 * 60, pigment: Pigment::Kunyit, task_id: Some(report.clone()), ..Default::default() },
            &tz,
            Actor::Owner,
            now,
        )?;
        events::create(
            conn,
            EventInput { title: "Rapat OSIS".into(), location: "Aula".into(), date: in_days(1), start_minute: 15 * 60, end_minute: 16 * 60 + 30, pigment: Pigment::Terakota, ..Default::default() },
            &tz,
            Actor::Owner,
            now,
        )?;
        reminders::create_at(
            conn,
            ReminderInput { text: "Kumpulkan laporan di Classroom".into(), date: date.clone(), minute: 21 * 60, task_id: Some(report.clone()) },
            &tz,
            Actor::Owner,
            now,
        )?;

        // Belajar: the owner's tracks, a few items, six weeks of evenings, one past CTF.
        learning::ensure_default_tracks(conn, now)?;
        let tracks = learning::tracks(conn)?;
        let track = |name: &str| tracks.iter().find(|t| t.name == name).map(|t| t.id.clone()).expect("seeded track");
        let item = |conn: &mut rusqlite::Connection, track: &str, title: &str, kind, domain, status, ctf: Option<String>, points| {
            let input = LearningInput {
                track_id: track.into(),
                title: title.into(),
                kind: Some(kind),
                domain: Some(domain),
                difficulty: Some(Difficulty::Easy),
                status: Some(status),
                points,
                ctf_event_id: ctf,
                ..Default::default()
            };
            learning::create_item(conn, input, Actor::Owner, now).map(|(id, _)| id)
        };
        let nmap_room = item(conn, &track("TryHackMe"), "Nmap Live Host Discovery", LearningKind::Room, Domain::Network, LearningStatus::Doing, None, None)?;
        item(conn, &track("TryHackMe"), "Linux Fundamentals Part 1", LearningKind::Room, Domain::Linux, LearningStatus::Done, None, None)?;
        item(conn, &track("TryHackMe"), "OWASP Top 10 - 2021", LearningKind::Room, Domain::Web, LearningStatus::Todo, None, None)?;
        let lame = item(conn, &track("Hack The Box"), "Lame", LearningKind::Machine, Domain::Linux, LearningStatus::Done, None, None)?;
        let vlan = item(conn, &track("Lab Sekolah"), "Praktikum VLAN dan Trunking", LearningKind::Practical, Domain::Network, LearningStatus::Doing, None, None)?;
        let (lks, _) = learning::create_ctf(
            conn,
            CtfInput { name: "LKS Cyber Security Kabupaten".into(), starts_on: Some(in_days(-12)), ends_on: Some(in_days(-12)), rank: Some(3), participants: Some(24), ..Default::default() },
            Actor::Owner,
            now,
        )?;
        let ctf = track("CTF");
        item(conn, &ctf, "baby-sqli", LearningKind::Challenge, Domain::Web, LearningStatus::Done, Some(lks.clone()), Some(100))?;
        item(conn, &ctf, "pcap-hunt", LearningKind::Challenge, Domain::Forensics, LearningStatus::Done, Some(lks.clone()), Some(200))?;
        item(conn, &ctf, "rev-101", LearningKind::Challenge, Domain::Reversing, LearningStatus::Todo, Some(lks), Some(300))?;
        for back in 1..=42_i64 {
            // Most evenings, a few skipped, so the heatmap has texture.
            if back % 7 == 3 || back % 5 == 0 {
                continue;
            }
            let item_id = if back % 3 == 0 { &vlan } else if back > 20 { &lame } else { &nmap_room };
            let input = SessionInput { item_id: item_id.clone(), date: in_days(-back), start_minute: 19 * 60, minutes: 25 + (back as u32 * 7) % 70, note: String::new() };
            learning::add_session(conn, input, &tz, Actor::Owner, now)?;
        }

        // The Library: a few real files in the demo folder, attached and in Koleksi.
        let folder = root.join("demo-files");
        std::fs::create_dir_all(&folder)?;
        let demo = [
            ("Laporan Praktikum 5 - VLAN.docx", &b"demo"[..]),
            ("topologi-vlan.png", TOPOLOGY),
            ("nmap-cheatsheet.pdf", &b"%PDF-1.4 demo"[..]),
            ("writeup-lame.md", &b"# Lame\n\nSMB, distcc.\n"[..]),
        ];
        let mut kept = Vec::new();
        for (name, bytes) in demo {
            let path = folder.join(name);
            std::fs::write(&path, bytes)?;
            let (added, _) = files::add(conn, &identity::probe(&path)?, None, Actor::Owner, now)?;
            kept.push(added.id);
        }
        let attach = |conn: &mut rusqlite::Connection, file: &str, kind, id: &str| {
            files::attach(conn, file, &LinkTarget { kind, id: id.into() }, Actor::Owner, now).map(|_| ())
        };
        attach(conn, &kept[0], LinkKind::Task, &report)?;
        attach(conn, &kept[1], LinkKind::Project, &lab)?;
        attach(conn, &kept[2], LinkKind::LearningItem, &nmap_room)?;
        attach(conn, &kept[3], LinkKind::LearningItem, &lame)?;
        let (sheets, _) = files::create_collection(conn, CollectionInput { name: "Cheatsheet".into(), pigment: Pigment::Lumut }, Actor::Owner, now)?;
        let (writeups, _) = files::create_collection(conn, CollectionInput { name: "Write-up".into(), pigment: Pigment::Lavender }, Actor::Owner, now)?;
        files::set_collections(conn, &kept[2], &[sheets], Actor::Owner, now)?;
        files::set_collections(conn, &kept[3], &[writeups], Actor::Owner, now)?;
        Ok(())
    })?;
    Ok(format!("Seeded a demo week ({date}) into {}.", root.display()))
}

/// A small VLAN topology picture, so the demo image has a real thumbnail.
const TOPOLOGY: &[u8] = include_bytes!("../../demo/topologi-vlan.png");
