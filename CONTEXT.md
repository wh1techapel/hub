# Hub: context for the next session

Read this first, then `docs/checkpoints.md` for where the work stands.

Hub is the owner's local-first personal workspace on Windows 11: schedules, routines, reminders,
tasks, projects, cybersecurity learning progress, files, and the history of all of it. It is a
separate application from ULTRON (`../Ultron`), and in CP5 it becomes the place where ULTRON reads
and writes the owner's personal data through a local API.

## Owner decisions (2026-10-22, answers to explicit questions: don't re-ask)

| Topic | Decision |
|---|---|
| Location | New repository `PROJEK/Hub`. VIO-DASHBOARD (`../VIO-DASHBOARD`) is left untouched and is only a source of ideas. |
| Source of truth | Hub owns tasks, reminders, notes, and files. In CP5 ULTRON reads and writes through Hub's API, and ULTRON's existing tasks, reminders, notes, and kept files are migrated into Hub. Timers and Pomodoro stay in ULTRON (updated 2026-11-10: Hub also has its own Sesi Fokus for students without ULTRON, kept apart from ULTRON's Pomodoro; see "Lentera on the desktop"). |
| Visual direction | **Daylight Studio**: the light follows the owner's day (morning, day, dusk, night), warm paper and ink, serif display type, the Day Arc as the signature element, an accent ink the owner picks. ULTRON's crimson appears only where ULTRON acted. |
| Files | Kept as a reference to the original file (no copy), followed when moved. Any file can also get a copy in Hub's Vault. |
| UI language | Indonesian. Technical names (Dashboard, CTF, Nmap, room names) stay as they are. Strings live in one place so English can be added later. |
| Cyber learning | Track platforms (TryHackMe, Hack The Box, and similar, entered by hand: no official public API), CTFs and competitions, and labs (school TJKT practicals and the homelab). Certification roadmaps are not wanted. |
| Rhythm | The owner opens Hub mostly while studying or working (midday), in the evening to review the day, and at any moment to capture something quickly. There is no morning planning ritual: tomorrow's focus is chosen in the evening review. |
| Calendar | Local only. No Google Calendar, no account, no internet. `.ics` import may come later. |
| Working style | One checkpoint at a time, at most five in total, a short report and a Git commit at the end of each, and the owner's explicit approval before the next one. Quiet work: no progress narration. |
| Start with Windows (CP2) | Yes, quietly into the tray (`--tray`), so reminders and ULTRON can use Hub. A switch in Pengaturan turns it off. Turned on once, on the first run of a release build. |
| Closing the window (CP2) | Hides Hub to the tray. Quitting is "Keluar dari Hub" in the tray menu. |
| Day hours (CP2) | Wake 05.00, sleep 22.30 (editable in Pengaturan). |
| Data folder (CP2) | `%LOCALAPPDATA%\Hub` (database, backups, Vault, WebView2 profile, `hub.log`). |
| Quick capture hotkey (CP3) | `Ctrl+Alt+H`, global. |
| Calendar (CP3) | Week view by default, Monday to Sunday; Day and Month one key away (D, W, M). |
| Reminder notifications (CP3) | Windows' reminder sound, buttons "Tunda 10 m" and "Selesai". The buttons work through a `hubapp://` link Hub registers for the owner's account. Until CP5, ULTRON's own reminders stay separate. |
| Tutup Hari nudge (CP3) | An invitation on Hari Ini from 18.30, and one gentle notification at 21.00 if the day is not closed yet (time and switch in Pengaturan). |
| School timetable (CP3) | The owner enters it as routines in Kalender. |
| Quick capture language (CP3) | Indonesian plus basic English ("besok jam 7 malam", "tomorrow 7pm"). The reading is always shown before saving. |
| Missed do dates (CP3) | An unfinished task whose do date passed comes back on Hari Ini until done or moved; Tutup Hari offers to move it. |
| Ruang (CP3) | Four to start with: Sekolah, Belajar Cyber, Pribadi, Proyek Pribadi. Renamable and removable. |
| Dropping files (CP4) | Files dropped on Library are saved at once (kind detected), with a toast offering Urungkan and Beri catatan. Dropped on a project or task pool, they are saved and attached there. A folder is kept as one reference; its contents are not indexed. |
| Vault copies (CP4) | Made when the owner chooses "Simpan salinan", and automatically for files dropped from a removable drive (a flash disk will be unplugged). |
| Belajar goal (CP4) | Counted in active days per week (default 4 of 7), forgiving: nothing resets. Hours are shown as information. |
| Search (CP4) | Ctrl+K searches file names, notes, and Koleksi, plus every task, project, learning item, event, and note in Hub. File contents are not indexed. |
| CP5 rhythm (CP5) | Built in three parts (5a API, 5b ULTRON on Hub, 5c polish and packaging) without stopping between them, one commit per part, one report at the end. |
| Fired reminders (CP5) | Hub always shows the Windows notification. ULTRON, when connected, says it aloud and shows its island alert, without a second notification. |
| Hub not running (CP5) | ULTRON starts Hub quietly into the tray (`--tray`) and carries on; it speaks only when that fails. |
| Dates said to ULTRON (CP5) | "Tomorrow" / "besok" in a task command is the do date, as in Catat Cepat; only "due / deadline / tenggat ..." sets the deadline. |

## Hub Studio decisions (2026-11-06, answers to explicit questions: don't re-ask)

Design: `docs/studio.md`.

| Topic | Decision |
|---|---|
| Data leaving the computer | An exception for Studio only: Studio's conversation and documents go to Google's Gemini API, and only when the owner sends a message or presses a generate button. Every other part of Hub stays offline, and no other Hub data is sent to an AI without the owner choosing it. |
| Gemini tier | Start on the free tier, with a plain warning in Settings and under the chat that Google may use and review what is sent. |
| Studio and Hub projects | A Studio project links to one Hub Proyek (new or existing) once its roadmap is approved. Checkpoints and Claude's tasks live in Studio's own tables and do not appear in Tugas or Hari Ini. |
| Development phase | Studio is a new phase of six checkpoints (S1 to S6). The working rules stay: one at a time, a short report, a commit, and the owner's approval before the next. |
| Claude Code | The owner uses both the desktop app and the CLI; both must work. |
| Installing into Claude Code | A "Pasang ke Claude Code" button in Hub, after the owner confirms what will change: `claude mcp add --scope user` and the skill in `~/.claude/skills/hub-connect`. |
| MCP transport | A stdio bridge `hub-mcp.exe`, installed beside Hub, calling Hub's local API with its own token. It starts Hub into the tray when Hub is not running. |
| Claude and the PRD | Claude only reads the PRD. Conflicts become questions for the owner in Hub. Only the owner approves a checkpoint. |
| Language | PRDs, roadmaps, and Gemini's replies in Indonesian; technical terms and ids (FEAT-001, CP-01) stay as they are. |
| Deadline | None given: the order S1 to S6 stands. |
| Roadmap after a new PRD (S4, 2026-11-06) | A new roadmap may replace the approved one only while none of its checkpoints has started (anything past Ready, or a report). After that the roadmap is locked; changes come as new checkpoints added at its end. |
| Building without Claude (S4) | The owner may run a checkpoint alone: start it, write a short report (summary and a result for every required test), and approve it. Same rules and statuses as Claude's reports. |
| Repository folder (S4) | The folder must exist. Hub only checks whether `.git` is there (never reads the repository) and warns, without refusing, when it is not. |
| API switches (S5, 2026-11-06) | One switch per client in Pengaturan: ULTRON and Claude Code each on or off, each with its own token and file. The local API runs while either is on; switching one off does not cut the other. |
| Real Claude Code run (S5) | Allowed once: a short `claude -p` against a test Hub with a test MCP configuration (connect, read the checkpoint, report), using the owner's Claude usage. |
| Notifications from Claude (S5) | A Windows toast when Claude sends a report, asks a question, or is blocked, also while Hub is in the tray; it opens Pantau. |
| New Hub Proyek from Studio (S4) | Made automatically: the Studio project's name, the PRD's summary as description, no area, the default colour. Changed later in Proyek. |
| Claude Code app (S6, 2026-11-08) | Studio works from Claude Code in the desktop app's Code tab as well as from the CLI. Claude's desktop chat (no project folder) is not supported. |

## Studio revision and navigation decisions (2026-11-08, answers to explicit questions: don't re-ask)

Design: `docs/studio.md`, "Revision: three stages". Plan: `docs/checkpoints.md`, "Studio revision".

| Topic | Decision |
|---|---|
| Studio's stages | Exactly three: **Mau buat apa?** (brainstorm with Gemini), **Tech stack** (picked by the owner or recommended by Gemini, then approved), **PRD dan bangun** (PRD, the checkpoint plan, the implementation prompt, Claude Code over MCP, and Pantau). Pantau is part of the third stage, not a fourth. |
| Order of the work | Studio first (R1 to R5), then Hub's navigation (R6), then validation (R7). One checkpoint at a time, a short report, a commit, and the owner's approval before the next, as before. |
| Tech stack input | A short developer profile in Pengaturan, filled once (what the owner knows, cost preference, where things are deployed), read by every recommendation. Gemini does not ask it again per project. |
| Implementation prompt | Two versions: "Dengan Hub" (short; Claude reads the PRD and the checkpoint over MCP) and "Mandiri" (self-contained: PRD, stack, checkpoints, and rules, for use without MCP). |
| Claude's status | Three separate signals, each from real data: the bridge (installed, registered, skill), the session (the bridge reports while Claude Code has it open and says goodbye when stdin closes), and activity (the last tool call). "Status belum diketahui" when Hub has no evidence yet. |
| Navigation | Main: Hari Ini, Kalender, Proyek & Tugas (one place: Kotak Masuk, lists, and projects by Ruang), Studio, Belajar. Secondary, under a divider: File, Jejak. Pengaturan at the bottom. Tugas's "Hari Ini" list is dropped (the dashboard covers it); Pengingat moves to Kalender. Old addresses stay as aliases. |
| Belajar | Stays a main page (its own data: timer, sessions, weekly goal). Overlap with projects is solved by linking, not merging. |
| Data safety | No data is moved or deleted by the revision. `studio_projects` is not rebuilt (its children use `ON DELETE CASCADE`); new state goes into new tables or columns, with a backup and an upgrade test. |

## Studio like Gemini decisions (2026-11-09, answers to explicit questions: don't re-ask)

Plan: `docs/checkpoints.md`, "Studio like Gemini (U1 to U6)".

| Topic | Decision |
|---|---|
| What is taken from Gemini | The layout only: a clean page, the greeting in the middle, one pill-shaped input, the history on the left. Hub's colours and Daylight Studio stay (malam is already dark). |
| Sidebar in Studio | Opening Studio slides Hub's sidebar over to Studio's history (the Pengaturan pattern from P1): back to Hub on top, Ide baru, search, the projects (waiting for the owner first, then recent). It folds into a narrow rail. |
| A project's workspace | One conversation from the idea to the build, with a canvas on the right for the documents (Ringkasan, tech stack, PRD, checkpoint plan, Bangun). Big results arrive as cards in the conversation with Buka and Setujui. The stage, document, and panel tabs become one small stepper. |
| Faster flow | Chained steps only: approving the tech stack writes the PRD, approving the PRD writes the checkpoint plan. The owner still approves the stack, the PRD, the plan, and every checkpoint. No Cepat/Teliti mode, no separate next-step bar. |
| The PRD | Deeper features and acceptance criteria (failure and edge cases), technical parts specific to the approved stack, the user flow drawn as a diagram, as creative and detailed as possible while leaving Claude room to decide how to build. |
| Order | U1 (installing `/hub-connect`) after the Gemini prompt research, then one checkpoint at a time with a report, a commit, and the owner's approval. |

## Lentera decisions (2026-11-09, answers to explicit questions: don't re-ask)

Plan: `docs/checkpoints.md`, "Lentera (L1 to L4)".

| Topic | Decision |
|---|---|
| The agent's name | **Lentera**. Studio's UI never names Gemini as the one talking: the conversation, the question set, the stack, the PRD, the plan, Ringkasan, and Jejak say Lentera. Gemini stays named only where the provider matters: Pengaturan > Integrasi AI (the key comes from Google) and the privacy notes, which say the conversation is processed by Google. |
| Replies that ask | Every reply that brings a question set shows only the questions, no text before or after them. The prompt asks Gemini for no text with `tanya` (feedback goes into a question's `why` or an option's `detail`), and the UI hides any text such a reply still carries. An answered set leaves one line ("Mengajukan 3 pertanyaan."). |
| Choosing the model | The model chip in Studio's inputs opens a menu of the text models the key can use, for every project (the same setting as Pengaturan). When the quota runs out, "Ganti model" stands beside "Coba lagi": Google counts the free quota per model. |
| Background glow | Studio's home gets a glow that follows the light of the day (pagi, siang, senja, malam) mixed with the owner's accent, moving slowly (about 12 s) and still when Windows asks for less motion. |
| Writing the PRD | Its loading is smooth: the three parts as steps (done, running, waiting), elapsed time, and the document fading in when it is done. |
| Order | L1 (Lentera, replies that ask), L2 (model picker), L3 (glow and the PRD's loading), L4 (validation). One at a time, a report, a commit, and the owner's approval before the next. |

## Lentera on the desktop decisions (2026-11-10, answers to explicit questions: don't re-ask)

Plan: `docs/checkpoints.md`, "Lentera on the desktop (CP01 to CP06)". Design: `docs/design.md`,
"Lentera on the desktop". Windows: `docs/architecture.md`, "Lentera on the desktop (CP01)".

| Topic | Decision |
|---|---|
| What it is | Lentera also becomes a light desktop assistant for students, like ULTRON but light: no GPU, text only (no voice), AI through Gemini with the existing `hub-ai` client and the same key in Credential Manager, and every change proposed as a confirmation card before it is written (from CP03). |
| Where | The capsule and the bar sit at the bottom right of the primary screen's work area, above the taskbar beside the clock, so they never meet ULTRON's island at the top centre. |
| When it shows | Hidden by default. The capsule shows only when Lentera is called or, from CP02, when something needs the owner; never permanently. |
| Shortcut | `Ctrl+Alt+L`, global, kept although IntelliJ-family IDEs use it for Reformat Code (the owner does not mind). Inside Hub also `Ctrl+L`, the sidebar's Tanya Lentera, and the palette's "Tanya Lentera"; the tray has "Tanya Lentera". |
| Inside Hub | The same desktop bar opens at the bottom right; there is no second Lentera inside Hub's window. Esc gives the keyboard back to Hub. |
| The bar before CP03 | A line goes through Catat Cepat's reader (offline, the same chips) and Enter saves it, with a plain note that Lentera's answers come later. No fake answers. |
| After a save, or a click elsewhere | After a save the bar folds into a capsule ("Tugas tersimpan: …") for about 2 s and leaves, and the keyboard goes back at once. A click elsewhere hides an empty bar; with an unsaved line the bar folds into a capsule that keeps it (click to go on, or Buang draf). |
| Catat Cepat | `Ctrl+Alt+H` and its window stay as they are. |
| Sesi Fokus and ULTRON (CP02) | Hub's Sesi Fokus stands on its own, for students without ULTRON. ULTRON's Pomodoro stays as it is; nothing is synced and Hub's API stream leaves Sesi Fokus out. Syncing the two can be a checkpoint of its own later. |
| The capsule during a session (CP02) | Visible for the whole session, at the bottom right, click-through around it. One action tucks it away (Selipkan) until the next phase change. It steps aside while a fullscreen application is in front, like ULTRON's island. |
| Belajar (CP02) | A finished focus block on a Belajar item is that item's learning session (it counts for the week's active days). Belajar's own timer and a session never run together: starting one stops the other into its session. |
| Phase changes (CP02) | A focus block ends in its break by itself (systematic breaks, Biwer et al. 2023); after the break the session waits for the owner ("Lanjut fokus"). A soft sound and the capsule, no Windows notification. |
| Lengths (CP02) | 25 minutes of focus, 5 of break, 15 for the long break after every 4 blocks, editable in Pengaturan > Hari (5 to 90, 1 to 30, 5 to 60, 2 to 8). Biwer et al. tested 24/6; the classic 25/5 was kept. |
| Time not seen (CP02) | Time Hub did not see (the computer asleep, Hub closed) never counts: a block pauses where Hub last saw it, a break counts as over. A session left paused or waiting for 30 minutes ends by itself. |
| What Lentera may do (CP03) | Propose new things only: tasks (do date, deadline), reminders, events, and notes, each as a card. It reads nothing of Hub, so no Hub data goes with it; changing or completing existing tasks waits for CP06 (Tanya Lentera). |
| What is sent to Google (CP03) | The owner's line (or the cleaned group message), the last few turns of this bar's conversation, today's date and the time, and the names of the active projects and the Ruang. Never tasks, notes, the calendar, or files. The bar says so under its line ("Apa yang dikirim?"); Pengaturan > Integrasi AI says it too. This extends Studio's exception (Hub Studio decisions): data leaves only when the owner presses Enter or Kirim. |
| Dari Pesan (CP03) | A pasted group message is cleaned on the computer first (senders' names, times of sending, phone numbers, mentions, and emails taken out; the date it was sent kept, so "besok" means the day after it was sent) and shown; only after the owner presses Kirim does it go. Names written inside a message may remain, and the panel says so. |
| Enter and no Gemini (CP03) | Enter sends the line to Lentera. Without a key the bar is Catat Cepat's, as in CP01, and says so; after a failure (offline, quota, Google's trouble) the owner may hand the line to Catat Cepat ("Catat Cepat saja"), which sends nothing. |
| Who wrote it (CP03) | A card saved is the owner's (they confirmed it): Jejak says "kamu". Tables outside Studio refuse Gemini as a writer, and that stays. |
| Order | CP01 the capsule and the bar; CP02 Sesi Fokus (Pomodoro offline, linked to tasks and Belajar, in Jejak, the countdown in the capsule); CP03 Lentera's brain (Gemini function calling over Hub's own functions, confirmation cards, minimal data sent, a privacy notice, and "Dari Pesan"); CP04 Pecah Tugas and Susun Minggu; CP05 Kuis Kilat (questions and flashcards, review after 1, 3, and 7 days); CP06 Tanya Lentera about the schedule and concepts (without doing homework), full validation, and the PSAJ report and pitch. One at a time, a report, and the owner's approval before the next. |

## PSAJ finalization decisions (2026-11-09, answers to explicit questions: don't re-ask)

Audit and plan: `docs/finalisasi/` (PRODUCT_AUDIT, FEATURE_STATUS, FINALIZATION_PLAN, F1 to F5).
Finalization documents are in Indonesian (they feed the PSAJ report). The group's members, their
contributions, the teachers, the costs, and the report's details are in
`docs/finalisasi/PSAJ_CONTEXT.md`, which stays out of the public repository with the rest of the
PSAJ material (see "Open source release" below).

| Topic | Decision |
|---|---|
| Purpose | Hub is the object of the group's PSAJ Bahasa Indonesia report (jobsheet in `..\BINDO`). Stability first, essential features second, polish third; no new features, no architecture changes. |
| Deadline | As the jobsheet says: chapters III to V and the layout in November 2026; submission and pitching on the PSAJ schedule. |
| Product name | **Hub**, in the app, the report, and the slides. "Project Hub" is not used. |
| Origin of the project | The Python subject (mata pelajaran Python). |
| Real runs in F1 | Allowed: Studio with the owner's real Gemini key and quota (a sample idea goes to Google), and one real Claude Code session (about USD 0.5 of the owner's usage). |
| Demo | Not decided yet (real data or a demo folder; internet at the venue unknown): F2 prepares both and a fallback. |
| The report | Written by Claude under the owner's direction and accepted as finished; the group signs it. The AI-use statement says so honestly, as the jobsheet requires. |
| Report checkpoint (F4) | Asked for by the owner on 2026-11-09: a checkpoint where Claude completes the report with what only the builder can get (real screenshots, diagrams from the code, test data, versions), which another writer such as GPT cannot. |

## Open source release decisions (2026-11-09, answers to explicit questions: don't re-ask)

| Topic | Decision |
|---|---|
| Price | Hub is not sold. It is released on GitHub as open source, free to use (Rp0). |
| License | MIT (`LICENSE`), declared once in the Cargo workspace and in `package.json`. |
| Public repository | `github.com/wh1techapel/hub`, a new repository that starts from one commit of the cleaned tree. The full history stays in this private repository. Public commits use the GitHub noreply address, never the owner's own email. |
| What stays private | The PSAJ material: marked `export-ignore` in `.gitattributes`, so `git archive` of this repository is the public tree. Names of other people, the school, and personal costs never go into a file that is exported. |
| CI | GitHub Actions on Windows (`.github/workflows/ci.yml`): typecheck, lint, Vitest, contrast, clippy, Rust tests, and a check that the generated files are current. Vitest and the Rust build run one after the other: Studio's tests time out when they share the CPU with a compile. Actions pinned to commit SHAs, kept current by Dependabot. |

## Glossary (UI words in Indonesian, code words in English)

| UI (Indonesian) | Code | Meaning |
|---|---|---|
| Hari Ini | `today` | The dashboard. Changes emphasis with the time of day: "Sekarang" during the day, "Tutup Hari" in the evening. |
| Busur Hari | `DayArc` | The day drawn as an arc from wake to sleep time: events on it, the sun (or moon) at the current time. The signature element. |
| Sekarang | `now` | What is happening now or next: the current event, the active task, the learning item in progress. |
| Fokus | `focus` | At most three tasks chosen for a day. Chosen in the evening review for tomorrow, or accepted from Hub's suggestion. |
| Tutup Hari | `dayClose` | The evening review: what got done, what slipped (tomorrow, a date, or let go), one optional sentence, tomorrow's focus. |
| Tugas | `task` | Something to do. Has a do date (when to work on it) separate from a deadline (when it is due). |
| Jadwal | `event` | Something at a time on the calendar. A task placed on the calendar becomes a time block linked to the task. |
| Rutinitas | `routine` | A recurring block, such as the school timetable or "belajar cyber" three evenings a week. |
| Pengingat | `reminder` | A one-time (or repeating) alert at a time. Never a task by itself. |
| Blok waktu | `block` | An event linked to a task: time set aside on the calendar to work on it. |
| Kotak Masuk | `inbox` | Tasks with no project, Ruang, or date yet: caught, not yet sorted. |
| Catat Cepat | `capture` | One line typed anywhere (Ctrl+Alt+H, the palette, the add rows), read by the core into a task, reminder, event, or note. |
| Lentera | `lentera` | Studio's agent, and since 2026-11-10 also the desktop assistant of Ctrl+Alt+L. |
| Kapsul | `capsule` | Lentera's small pill at the bottom right: what was saved, a kept draft, and from CP02 the focus countdown. |
| Bar Lentera | `bar` | The capsule grown into a line and a card area for Lentera's replies. |
| Kartu usulan | `proposal` | Something Lentera proposes in the bar (CP03): a task, reminder, event, or note, saved only by its Simpan; Ubah changes it in place, Lewati sets it aside. |
| Dari Pesan | `group_message` | A pasted group message (a WhatsApp class group), cleaned on the computer and shown before Lentera reads it for tasks and deadlines (CP03). |
| Sesi Fokus | `focus_session` | Hub's Pomodoro (CP02): focus blocks and breaks for a task, a Belajar item, or nothing in particular, in Lentera's capsule. Not the day's Fokus (three tasks). |
| Blok fokus | `focus block` | One stretch of focus in a session (25 minutes by default), kept in `focus_blocks` and Jejak. |
| Selipkan | `tuck` | Fold the session's capsule away until the next phase. |
| Proyek | `project` | An outcome with tasks, milestones, files, and history. Progress is derived from its tasks (done over all), not typed in. |
| Tonggak | `milestone` | A dated group of a project's tasks. Reached once all its tasks are done; one without tasks is ticked by hand. It never changes how progress is counted. |
| Ruang | `area` | A life area that groups projects: Sekolah, Cyber, Pribadi, and so on. |
| Belajar | `learning` | The cybersecurity learning page: tracks, items, sessions. |
| Jalur | `track` | A learning source: TryHackMe, Hack The Box, a CTF event, Lab Sekolah, Homelab. |
| Materi | `learningItem` | One room, machine, challenge, or practical. Has a domain (network, web, ...) and a status. |
| Sesi | `session` | Time spent on a learning item, started and stopped by the owner. |
| Minggu aktif | `activeWeek` | The forgiving streak: days with learning in a week against the owner's weekly goal. A missed day never resets anything. |
| File / Library | `file` | A file Hub knows about: where it is, what kind it is, what it is attached to. |
| Vault | `vault` | Hub's own folder for files the owner chose to keep a copy of. |
| Koleksi | `collection` | A category the owner makes for files, next to the automatic kinds. |
| Catatan | `journal` | A dated note or done activity ("aku sudah membersihkan kamar"). The evening sentence is one too. |
| Jejak | `activity` | The append-only history of what changed, by whom (the owner, ULTRON, or Hub itself), and the insights drawn from it. |
| Tinta | `ink` | The accent colour the owner picks (Biru Tinta, Hijau Botol, Sepia, Ungu Senja, Grafit). |
| Cahaya | `phase` | The light of the moment: `pagi`, `siang`, `senja`, `malam`. |

## Map

- `docs/design.md`: visual identity, UX direction, information architecture, motion, references.
- `docs/architecture.md`: stack, process model, data model, local API for ULTRON, security, resources.
- `docs/checkpoints.md`: CP1 to CP5, scope, definition of done, status.
- `docs/studio.md`: Hub Studio (Gemini, PRD, roadmap, Claude Code over MCP): design and S1 to S6.
- `design/tokens.css`: the design tokens. The app imports this file; it is the source of truth.
- `design/check-contrast.mjs`: checks every text colour against every paper (4.5:1), all phases and inks.
- `design/prototype/`: the CP1 visual prototype (static HTML, served by `python -m http.server`
  from `design/`, see README). Not app code.
- `crates/hub-core/`: the domain core and the only code that touches SQLite (`hub-seed` lives here).
- `src-tauri/`: the desktop app (window, tray, commands). `src/`: the React UI.
- `scripts/`: `smoke-native.mjs` (the real window over DevTools), `measure.ps1` (start, memory, CPU).

## Notes for sessions run by Claude

- Inside the Claude sandbox `%LOCALAPPDATA%` is virtualized (MSIX): a Hub started from there writes
  where the owner's Hub never looks. Use `HUB_DATA_DIR` for smokes and give the owner the command
  to start their own Hub. The writes land in
  `%LOCALAPPDATA%\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Local`; the processes have no package
  identity, so `GetCurrentPackageFullName` cannot tell (`hub-mcp` checks where a probe file lands).
- The Browser pane may stop drawing frames while it is hidden: CSS transitions then stay at their
  start, and screenshots can show an old frame. Read state through the DOM, and take a second
  screenshot before trusting one.
- A window built inside a synchronous Tauri command never appears on Windows (the command holds
  the main thread the build waits for): a command that builds one is `#[tauri::command(async)]`
  (`open_lentera`).
- Build the release with `npm run tauri build` (or `npx tauri build --no-bundle`), never a bare
  `cargo build --release`: without Tauri's `custom-protocol` feature the window loads the dev
  server's address and stays empty.
- To capture the real screen in a script, call GDI's `BitBlt` with `CAPTUREBLT` (0x40CC0020): .NET's
  `CopyFromScreen` refuses that flag, fails, and leaves a blank image.
- Never show a Tauri window behind Tauri's back (a raw `ShowWindow`): tao still thinks it hidden
  and hides it again at its next style change (`set_ignore_cursor_events`, for one). To show one
  without taking the keyboard, make it unfocusable through Tauri first (`window::show_quietly`).
- A click scripted over DevTools is not input to Windows: a window it opens may not keep the
  keyboard (Lentera then folds away as after a click elsewhere). The native smoke opens Lentera
  with the real shortcut (`keybd_event`), which Windows trusts.
- A thread that asks a Tauri window anything (visible, position) waits on the main thread: never
  hold a lock the main thread's commands also take while doing so (Lentera's cursor watch did, and
  Hub stopped answering).
- Run the native smoke in the daytime: in the evening the test Hub's own toasts (the 21.00 Tutup
  Hari nudge, an overdue seeded reminder) sit in the bottom-right corner over Lentera, and Windows'
  notification panel (`ControlCenterWindow`) can hold the foreground, so CP01's transparency shot
  and CP02's keyboard checks fail for reasons outside Hub (seen on 2026-11-10 after 21.00).
- Windows' file system ignores case: `Conversation.tsx` and `conversation.ts` in one folder are one
  module to TypeScript (Lentera's pure conversation steps live in `src/lentera/talk.ts` for that).
