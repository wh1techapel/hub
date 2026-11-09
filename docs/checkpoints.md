# Checkpoints

Five at most. One at a time; each ends with tests, a short report, a commit, and waits for the
owner's explicit approval before the next starts.

**Status: CP5 done (2026-10-24). All five checkpoints are built; what is left are the owner's
checks listed under each.**

## CP1: Research and design foundation (done)

- Research of references and practices: `docs/design.md` (references table), `docs/architecture.md`.
- Owner decisions: `CONTEXT.md`.
- Visual identity as tokens: `design/tokens.css` (light phases, inks, pigments, ULTRON crimson,
  type, space, motion), contrast checked by `design/check-contrast.mjs`.
- Visual prototype: `design/prototype/` (Hari Ini by day and by night, Day Arc, ink check, Tutup
  Hari, drop pools, ULTRON mark, the foundations sheet).

## CP2: Hub foundation (done)

**Goal**: a Hub that starts, stores, and shows today.

Result (2026-10-22):

- Built as planned. Additions found necessary on the way: the window is shown by the page once
  drawn in the right light (no white flash at night); the Windows title bar takes the page's
  colours; a window hidden for 10 minutes is destroyed and rebuilt on demand (measurement showed
  hiding frees nothing); `hub-seed` fills a separate data folder with a demo day.
- Kept out on purpose (later checkpoints): creating tasks, events, and focus in the UI (CP3), the
  Tutup Hari review (CP3: CP2 shows the evening tally only), the Motion library (CP3, with drag),
  the search index (CP4).
- Checks: 54 Rust tests (`hub-core`), 29 UI tests, typecheck, lint (strict), contrast 184 pairs,
  clippy clean, browser check of the UI with the mock backend, native smoke 13/13 in the release
  window (`scripts/smoke-native.mjs`), measurements in `docs/architecture.md` ("Resources").
- Owner checks still open: the first real start registering autostart and the tray icon after a
  Windows restart; "Keluar dari Hub" from the tray; the autostart switch in Pengaturan.

- Tauri 2 + React app in this repository; tray, single instance, close to tray, autostart switch.
- Tokens imported from `design/tokens.css`; the light engine (phases from the clock and settings,
  theme modes, ink choice); fonts self-hosted.
- App shell: sidebar and rail, routes for every page (empty states for the ones built later),
  command palette skeleton (navigation only).
- Core: SQLite with migrations, the full schema of `docs/architecture.md` (empty tables for later
  checkpoints are cheap and avoid churn), the activity log, the `changed` event, settings, daily
  backup, `HUB_DATA_DIR`.
- Hari Ini v1: greeting, Day Arc with real time and today's events, Fokus and Sekarang reading real
  data, the evening invitation (the full review is CP3).
- Measurements: memory with the window open, hidden, and destroyed; idle CPU; start time.

**Done when**: `cargo test`, `vitest`, typecheck, lint, and build pass; the app starts, survives a
restart with its data, follows the phases; browser and native smokes pass; measurements recorded.

## CP3: Productivity system (done)

**Goal**: plan and run the days in Hub.

Result (2026-10-23):

- Built as planned: Tugas (Kotak Masuk, Hari Ini, Mendatang, Semua, Selesai, Pengingat; the panel
  beside the list saves each field as it changes; steps; time blocks; delete with Urungkan),
  Proyek and Ruang (grouped list, derived progress, next step, milestones, status, history),
  Kalender (day, week, month; drag to create, move, and resize; routines with skip, move once, and
  restore; tasks from the tray become time blocks; the over-full warning), reminders (scheduler,
  Windows notifications with Tunda 10 m and Selesai through `hubapp://`, the line above every page),
  Catat Cepat (Ctrl+Alt+H window, palette, add rows; the core reads Indonesian and basic English),
  Fokus suggestions with "Pakai saran ini", and Tutup Hari (slips to tomorrow, a date, let go, or
  kept; one sentence; tomorrow's focus; the arc fills with ink and the moon rises).
- Changed on the way: no Motion library and no `rrule` crate (pointer events and a small RRULE
  subset were enough, see `docs/architecture.md`); Fokus can also be set from a task's panel; a
  line starting with "ingatkan" switches Catat Cepat to Pengingat; `useAct` returns an outcome,
  because commands that return nothing made success look like failure (sheets did not close).
- Checks: 137 Rust tests (`hub-core`, including the capture reader, routines, calendar, reminders
  fired late, Tutup Hari) and 3 in the app crate (plus one that shows a real notification, run by
  hand), 56 UI tests, typecheck, lint, clippy, contrast 184 pairs; browser checks with the mock
  (every page, the review sheet, Catat Cepat window, mouse drag to move and create on the calendar
  and to reorder tasks); native smoke 21/21 in the release window (a reminder fired by the real
  scheduler and snoozed, Catat Cepat to Tugas, the week against the core, Tutup Hari surviving a
  restart); measurements in `docs/architecture.md` ("Resources").
- Owner checks still open (the sandbox cannot reach the owner's registry or session): a real
  notification's Tunda 10 m and Selesai buttons; Ctrl+Alt+H from another app; the 21.00 Tutup Hari
  notification; dragging a task from the Kalender tray onto the grid with the mouse; entering the
  real timetable as routines.

- Tasks: quick add in Indonesian, do date and deadline, priority, subtasks, drag order, Inbox,
  Hari Ini, Mendatang.
- Projects and Ruang: milestones, derived progress, next step.
- Calendar: day, week, month; drag to create; tasks dragged in become time blocks; routines with
  RRULE and exceptions (school timetable); over-full day warning.
- Reminders: scheduler, Windows notifications, snooze and done, missed while away.
- Fokus suggestions and the full Tutup Hari review with its close.
- Global quick capture window (Ctrl+Alt+H) and the command palette's create actions.

**Done when**: parser and scheduler tests (fake clock, sleep, clock change) pass; a reminder fires
in the real app; keyboard paths exist for every drag; smokes pass.

## CP4: File library and progress (done)

**Goal**: files and learning in one place, and a visible history.

Result (2026-10-23):

- Built as planned: the Library (drop pools on every page, kinds, Koleksi, Proyek filter, grid and
  list, the panel with note, Koleksi, attachments, Vault copy, Explorer, and "Cari file" for a
  missing file; Windows thumbnails), files on projects, tasks, and learning items, move tracking
  by file id, Belajar (tracks as tabs, items by status, the timer, sessions by hand, CTF events,
  the week's dots against the goal, the 20-week heatmap, domain bars), Jejak (filters by who and
  what, the week's five measures against the 4-week average with sparklines), and search in the
  palette (FTS5 over every kind of item). Hari Ini gains Belajar and the latest files.
- Owner decisions (`CONTEXT.md`): a drop is saved at once with Urungkan and "Beri catatan"; a
  folder stays one reference; Vault copies on request and automatically from removable drives; the
  goal counts active days (4 of 7); search covers names, notes, and Koleksi, not file contents.
- Changed on the way: no new plugin for dialogs (Hub's own `IFileOpenDialog` on its COM thread);
  file identity uses the 128-bit `FILE_ID_INFO`, which ReFS needs too; the hourly maintenance pass
  checks 100 files; `checkFile` is called when opening or revealing fails rather than on every
  view. Two CSS faults found in the real window and fixed for every page: module keyframes were
  renamed by CSS modules and never ran, and an entrance transform kept by `both` turned the page
  into the containing block of fixed panels (`docs/architecture.md`, Stack).
- Checks: 198 Rust tests (`hub-core`, file identity on the real NTFS drive, the library through a
  fake finder, Vault, search, learning, the week's insights against hand-computed numbers) and 5
  in the app crate (two new: a Windows thumbnail made once and cached, and its alpha), 72 UI tests, typecheck, lint, clippy, contrast 184 pairs;
  browser checks with the mock (every new page, the narrow panel, drop pools with the pointer, the
  palette's search); native smoke 29/29 in the release window (the Library against the core, a
  real Windows thumbnail, a Vault copy on disk, a file renamed on disk found again by its id, a
  deleted one marked missing, the learning timer writing a session, Jejak's measures, search across
  kinds); measurements in `docs/architecture.md` ("Resources").
- Owner checks still open (the sandbox does not drag from Explorer into another window): a real
  drag from Explorer onto the Library and onto an open project, task, and learning item; the
  "Tambah file" dialog; a file moved to another drive (missing, then "Cari file"); a drop from a
  flash disk making its Vault copy; Explorer and opened files coming to the front; thumbnails of
  real PDF and Office files (depend on the installed apps).

- File drop anywhere (real paths), drop pools, kinds, Koleksi, attach to projects, tasks, learning
  items; move tracking by file id; Vault copies; Windows thumbnails; missing and locate.
- Belajar: tracks (TryHackMe, Hack The Box, CTF, Lab Sekolah, Homelab), items, sessions with a
  timer, domain bars, heatmap, weekly goal.
- Jejak: the activity timeline with filters, weekly insights against the 4-week average.
- Search: FTS5 across everything, in the command palette.

**Done when**: a real drag from Explorer works in the real window; a file moved on the same drive
is found again; insights match hand-computed numbers on seed data; smokes pass.

## CP5: ULTRON integration and polish (done)

**Goal**: ULTRON as the owner's secretary over Hub, and a finished feel.

Result (2026-10-24), built in three parts without stopping between them (owner's choice):

- **5a, the local API** (`crates/hub-api`, 176e6be): axum on Tauri's tokio runtime, 127.0.0.1
  only. Every request passes the checks before any database read: a loopback peer, no browser
  headers (`Origin`, `Sec-Fetch-Site`; no CORS), an exact `Host` with Hub's port, a bearer token
  even from loopback (only its SHA-256 stored; `api.json` in the data folder holds it for ULTRON),
  a rate limit, and a scope per route; bodies small, JSON only, strict. Endpoints for the day, the
  schedule, tasks, reminders, projects, files (search through what they are attached to),
  notes, learning, search, and `/v1/events` (server-sent events: changes and fired reminders).
  Writes are ULTRON's (crimson mark); `importRef` makes an import run once (migration 0004, which
  also lets a note explain a task). Pengaturan > ULTRON: on/off, port, a new token.
- **5b, ULTRON on Hub** (ULTRON b08d8e5, Hub ea521fe): ULTRON's tasks, reminders, notes, and kept
  files live in Hub; its timers and Pomodoro stay. ULTRON starts Hub quietly when it is not
  running (owner's choice), says fired reminders aloud without a second notification (owner's
  choice), reads a plain day as the do date and "due ..." or a time as the deadline (owner's
  choice), answers "what's my schedule tomorrow?" / "apa jadwalku besok?", and finds files through
  Hub's search. `scripts/migrate_to_hub.py` (dry run, then `--apply`): the owner's database held
  one note on 2026-10-23.
- **5c, polish and packaging**: "Ekspor semua data" (Pengaturan > Sistem: one JSON file of every
  table of the owner's data, written whole; no token hashes), the NSIS installer (per user, no
  admin, program in `%LOCALAPPDATA%\Programs\Hub` beside the data, not inside it:
  `src-tauri/windows/hooks.nsh`), the demo backend loaded only in the browser, an accessibility
  pass (every page: named controls, labelled fields, one h1, unique ids; contrast 184 pairs), and
  measurements (`docs/architecture.md`, "Resources").
- Changed on the way: removing a task or note over the API exists after all (soft, to the trash),
  because ULTRON's "delete the networking task" needed it; the token file is `api.json` (address
  and token together, so a new port needs nothing on ULTRON's side). No Motion library: every
  animation stays CSS and honours reduced motion.
- Checks: 207 Rust tests in `hub-core` (export, journal links, imports, API clients), 22 in
  `hub-api` (every defence over the real router, then what ULTRON does; the event stream over a
  real socket through a stop and a token rotation), 5 in the app crate; 74 UI tests; typecheck,
  lint, clippy, contrast 184 pairs; accessibility pass on every page; native smoke 35/35 in the
  release window (ULTRON adds a task through the API on a test port and it shows at once with
  its mark; a web page's request is refused); the installer installed silently, put the program
  in `%LOCALAPPDATA%\Programs\Hub` by default, and uninstalled cleanly; measurements meet every
  target (`docs/architecture.md`). In ULTRON: 4215 tests, `scripts/smoke_hub.py` 13/13 and the
  productivity voice smoke 16/16 against a real test `hub.exe`.
- Owner checks still open (the sandbox cannot reach the owner's Hub, registry, or session):
  installing with the real installer and Hub starting with Windows after a restart; ULTRON
  started normally, starting Hub by itself and saying a reminder aloud; the migration
  `uv run --no-sync python scripts/migrate_to_hub.py --apply` in `..\Ultron` once Hub runs; one
  real "Ekspor semua data".

**Done when** (as planned): API tests pass; ULTRON's productivity voice smoke passes against Hub;
measurements meet the targets or the gaps are reported; the release build installs and autostarts.

# Hub Studio (S1 to S6)

A new phase after CP5 (owner's decision, 2026-11-06, CONTEXT.md): Hub Studio, designed in
`docs/studio.md`. Six checkpoints, the same rules: one at a time, tests, a short report, a commit,
and the owner's approval before the next.

**Status: S6 done (2026-11-08). Studio is built; what is left are the owner's checks listed under
S6.**

## S1: Foundation and Gemini (done)

Result (2026-11-06):

- Migration 0005: every Studio table (projects, messages, ledger, PRD versions, roadmaps,
  checkpoints, their tasks, reports, requests) and `activity` rebuilt to accept the actors `claude`
  and `gemini`, every row kept; a saved PRD version and a report cannot change (triggers). Hub copies
  a database whole to `backups\hub-before-schema-N.db` before migrating it.
- `hub-core::studio`: projects (create, list, rename, remove), the conversation (owner messages,
  a reply that streams, finishes once, can be stopped, and is marked failed when Hub stopped
  mid-answer), the ledger (decisions, assumptions, questions, constraints; a replaced decision is
  retired in the same write; confirmed only for assumptions, answered only for questions).
- `crates/hub-ai`: the Gemini REST client (Windows TLS, `x-goog-api-key` header, model ids checked
  before they reach a URL, the Interactions API with `store: false`), Google's errors sorted into
  kinds (the Interactions shape and the older google.rpc shape), Uji koneksi in two steps (read
  the model, then a very short generation), and the key in Windows Credential Manager.
- App: commands for the key (save, remove, whether one is saved; never the key itself), Uji
  koneksi, the model list, Studio's projects. Pengaturan > Integrasi AI > Google Gemini (hidden
  field with show and hide, save, change, remove with one question, model from Google's list, the
  outcome in words with Google's own words and wait, the data note). Studio in the sidebar
  (`/studio`, loaded on first visit): the greeting, the way to Pengaturan without a key, Gemini's
  state, projects grouped by stage, the data note. Jejak says "Kamu menyimpan API key Gemini" and
  knows Studio's words.
- Changed on the way: a test Hub (`HUB_DATA_DIR`) no longer joins the single-instance rule, so a
  smoke runs beside the owner's running Hub instead of handing itself to it and quitting; a link
  set as a button no longer shows underlined. The planned separate probe command was not needed:
  Uji koneksi with the owner's key exercises the same live calls (model read, then a generation),
  and an unexpected answer shape shows as "Jawaban Gemini tidak dikenali".
- Checks: 231 Rust tests in `hub-core` (the 0005 upgrade keeps every activity row and the
  append-only rule; the copy before an upgrade; Studio's rules), 26 in `hub-ai` (each Uji koneksi
  outcome against a fake Gemini on loopback: connected, invalid key in both shapes, permission,
  model, quota with its wait, payment, service, refused, unknown host, timeout; the real
  Credential Manager under a test name; the key never in a report), 22 in `hub-api` unchanged, 5
  in the app crate; 87 UI tests (`--maxWorkers=2`; a fully parallel run timed out under the
  machine's load that day, the same tests pass alone); typecheck, lint, clippy, contrast 184 pairs;
  browser check with the mock (Studio, the key field and its error); native smoke 42/42 in the
  release window (35 before, plus Studio asking for a key, the key saved, Uji koneksi against a fake
  Gemini, the key in no file of Hub's, offline when Gemini is gone, Hub working on, the test key
  removed). hub.exe 15.4 MB (13.8 MB in CP5: the HTTPS client).
- Owner checks still open (need the owner's key and account): save the real key, press Uji
  koneksi (expect "Terhubung"), and see the model list filled from Google.

## S2: Brainstorming (done)

Result (2026-11-06):

- `crates/hub-ai`: the instructions in `prompts/studio.toml` (base rules, one block per stage, the
  summary, the three functions), embedded at build time and checked by tests (every stage and
  rule present, the ledger in every request, retired items left out, owner text and summaries
  fenced so a closing tag inside them cannot end the fence, nothing but the project in a request);
  the streaming reader for the Interactions API (text, thought signatures, function calls rebuilt
  from their argument pieces, usage, errors in the stream, split UTF-8 and split events); a whole
  reply that hands each function call to Hub and continues while Gemini waits for results, sending
  the model's steps back exactly; one retry without functions if Google refuses the request
  itself; Hentikan through a watch channel that drops the connection at once; the checked
  arguments of `catat`, `tanya`, `usul_tahap` (refused with a reason Gemini hears, or trimmed to
  three questions, four options, one recommendation).
- `hub-core::studio`: stages move forward one at a time, only when the owner says so and only into
  stages this Hub can serve (Klarifikasi for now; PRD comes with S3); a name from the idea; the
  conversation summary kept with the message it covers.
- App: start a project from an idea, the conversation over a Tauri channel (started, text, parts,
  finished), Coba lagi, Hentikan, the summary of messages past the 40 Gemini reads in full
  (made when 10 more wait), Gemini's notes written to the ledger as Gemini's with the reply they
  came from, its questions and stage proposal kept with the reply; the ledger for the owner (add,
  edit, confirm, answer, retire); rename and remove a project.
- UI: Studio's greeting takes the idea and opens the project with it sent; the project page with
  its name, the stage line, the conversation (messages on paper, Gemini always labelled, a small
  safe Markdown renderer that never renders HTML, the caret while streaming, Hentikan and Esc,
  stopped and failed replies said plainly with Coba lagi, questions as choices with the
  recommendation and "Belum tahu" that fill the input, the proposal to move on, the data note with
  "Apa yang dikirim?"), Ringkasan beside it (four lists, the retired ones folded, "Lihat di
  percakapan"), one pane at a time with a switch and a dot under 820 px of content. Lazy route.
- `cargo run -p hub-ai --bin studio-eval`: five ideas against the real Gemini with the owner's key
  (from Credential Manager or `GEMINI_API_KEY`), each reply checked: text, no PRD yet, questions
  asked through `tanya`, at most three, at most one recommendation each, valid arguments, no em
  dash.
- Changed on the way: questions go through the function `tanya` (the reply's text stays prose);
  the instructions say so and that a function call still needs words for the owner. Moving into
  PRD waits for S3: the proposal says so instead of a button that would lead nowhere.
- Checks: 234 Rust tests in `hub-core`, 46 in `hub-ai` (the streaming reader in pieces of 1 to
  10,000 characters, a function call round against a fake streaming Gemini with the second request
  checked step by step, the retry without functions, Hentikan ending a stalled stream in under 5 s,
  the prompt and argument rules), 22 in `hub-api` unchanged, 5 in the app crate; clippy clean; 96
  UI tests (`--maxWorkers=2`), typecheck, lint; browser check with the mock at 1280 and 960 px wide
  (two panes, then the switch with its dot); native smoke 48/48 in the release window (adds: an
  idea starts a project and the reply streams in from a fake streaming Gemini with a function call
  round, the question as choices, Gemini's note in Ringkasan as Gemini's, the two requests stateless
  and holding nothing but the project, a reply failing plainly with Coba lagi when Gemini is gone).
- Owner checks still open: a real conversation with the owner's key, and
  `cargo run -p hub-ai --bin studio-eval` (uses five replies of the key's quota).

## S3: PRD (done)

Result (2026-11-06):

- `hub-core::prd`: the PRD's shape (identity, background with evidence and assumptions, goals with
  measured indicators, experience with personas and flows, features with MoSCoW priority, user
  story, Given/When/Then criteria and test scenarios, NFRs by category, technical planning marked
  as proposals and only when applicable, delivery with MVP, order, risks, open questions), read
  strictly (unknown fields refused) and section by section so a broken section is named; the rules
  Gemini's schema cannot express (ids shaped and unique, dependencies that exist and do not loop,
  criteria and tests for every Must and Should feature, every Must feature in the MVP, measured
  targets, lengths); versions (one working draft; Gemini's PRD frozen as a numbered version at
  once; "Simpan versi"; approval only with no problems, of the latest number when it holds the
  same document, superseding an earlier approval); Markdown export.
- `hub-ai::prd`: three structured requests (`response_format` with a JSON Schema per part, each
  part seeing the ones before), one repair request for each part the core still finds problems
  in, Hentikan between and during requests, and the revision of one section returned as
  `value`, `changeNote`, `impacts`; the instructions for both in `prompts/studio.toml`.
- App: Susun PRD (moves the project to the PRD stage; progress per part; the result kept even
  with problems, which the owner then sees), edit a section, ask for a revision (a proposal,
  saved only when accepted, recorded as Gemini's), save a version, approve, open earlier versions,
  export Markdown through Windows' Save dialog.
- UI: the documents beside the conversation now have tabs (Ringkasan, PRD); the PRD set for
  reading (display titles, contents, ids in mono, assumptions and missing evidence marked,
  criteria as Given/When/Then); per section Ubah (a generic editor: fields, one item per line,
  choices, nested groups, lists with add, remove, reorder, and ids Hub assigns) and Minta revisi
  (the proposal with what changed, its impacts, and the problems it would leave); the problems
  linked to their section; approval blocked while any remain, and asked once when open questions
  remain; history with read-only versions; Gemini's proposal to move on now offers Susun PRD; the
  conversation stays in view while a long document scrolls.
- Found on the way (native smoke): approving a PRD wrote an activity row with the verb `approved`
  that Hari Ini and Jejak could not read, so Hari Ini failed to load after a restart. Fixed by
  adding the verb, by writing every Studio activity row through the typed `activity::record`, and
  by a test that reads Hari Ini, Jejak, and recent activity after every kind of Studio write.
  Approving is disabled while the approved version is already the draft.
- Checks: 270 Rust tests in `hub-core` (the rules case by case, a cycle, assembly naming broken
  sections, versions through edit, save, approve, re-approve, Markdown, the Jejak regression),
  52 in `hub-ai` (three parts then one repair against a fake Gemini, the second request checked;
  stopping; schemas; repair and revision fencing), 22 in `hub-api` unchanged, 5 in the app crate;
  clippy clean; 102 UI tests (`--maxWorkers=2`), typecheck, lint; browser check with the mock at
  1440 px (the document, the features editor); native smoke 52/52 in the release window (adds:
  Susun PRD writing three structured parts into version 1 with no problem left, each part asking
  for JSON against its own schema, approval as version 1, and Hari Ini and Jejak reading after
  Studio wrote; the restart check now reads the close from Jejak, since Hari Ini lists only the
  latest few rows). The last UI change (approval disabled when nothing changed) came after the
  release build and is covered by the UI tests only.
- Owner checks still open: Susun PRD and a revision with the owner's key on a real conversation;
  Ekspor Markdown through the Save dialog.

## S4: Roadmap (done)

S4 is built in three parts, each with its own commit: S4a the core (rules, statuses, Gemini's
request), S4b the Roadmap tab, S4c Pantau, the prompt, and the Proyek line. Owner decisions for
S4 are in `CONTEXT.md` (roadmap lock, building without Claude, the folder, the new Proyek).

### S4a: Roadmap core (done)

Result (2026-11-06):

- `hub-core::roadmap`: the plan's shape (checkpoints with goal, features, tasks and subtasks,
  dependencies, difficulty, risks, acceptance, required tests with kind and command, deliverables,
  DoD), read strictly and per checkpoint so a broken one is named; Hub assigns the codes (`CP-01`
  upward, task codes `CP-01.T01`, subtasks `CP-01.T01.1`), renames dependencies with them, and
  keeps two DoD lines in every checkpoint (required tests pass, report sent); the rules (1 to 20
  checkpoints, every Must feature in exactly one, Should and Could at most once, Won't never,
  features and task features that exist in the approved PRD, dependencies only on earlier
  checkpoints, at least one task, criterion, and required test, test names unique, lengths).
- The draft: one per project, from Gemini against the approved PRD version (refused when that is
  no longer the approved one); edit a checkpoint, add after one, remove, move (codes, dependencies,
  and task codes follow; a dependency that now points forward shows as a problem); "Ubah roadmap"
  copies the approved one; discard.
- Approval (no problems against the approved PRD): links a new Proyek (name, PRD summary, no area)
  or an existing one no other Studio project holds, confirms an existing absolute repository
  folder, sets Akses Claude, supersedes the earlier roadmap, and moves the project to Bangun, in
  one write. Replacing is refused once a checkpoint of the approved roadmap has started; the
  locked roadmap grows by checkpoints appended at its end (Planned). Folder check: exists, and
  whether `.git` is there (a warning for the UI, never read further).
- Statuses, every transition of the design's table: Mulai (owner only, dependencies Completed, one
  active at a time), start, block with a blocker request, resume (Claude only after the owner
  answered; the owner may answer while resuming), submit a report (every required test reported,
  a failed or skipped one explained in "not verified"; paths relative and inside the repository,
  links http(s), commit hash, at most 64 KB), approve and request changes (owner only), task
  status and note while In Progress or Blocked. Reports never change (the 0005 trigger). The owner
  can do all of it alone with a short report.
- "Buat prompt implementasi": the design's template filled by Hub from Ready until Completed
  (names on one line, no AI). `link_of` gives the Proyek page its line (active checkpoint, done
  of total).
- `hub-ai::roadmap`: one structured request (the approved PRD fenced as data, a schema matching
  the core's plan) and one repair request with the problems; Hentikan before and during a
  request. The instructions in `prompts/studio.toml` (`[task.roadmap]`).
- `projects::insert` split out of `projects::create` so the Proyek is made inside approval's write.
- Checks: 319 Rust tests in `hub-core` (49 new: the rules case by case, renumbering, the draft
  edits, approval with each link and folder case, the lock, every transition and every refused
  one, the owner building alone, the report rules, the prompt, Jejak and Hari Ini reading every row
  the roadmap writes), 56 in `hub-ai` (4 new: fencing, the schema, one request then one repair
  against a fake Gemini, a good answer needing none, stopping), `hub-api` and the app crate
  unchanged; clippy clean; typecheck and lint with the new generated types. No UI yet, so no UI
  test, browser check, or native smoke in this part.
- Not yet proven: a real Gemini answer read by the core's plan type (the schema and the type are
  written to match; S4b's command test checks a recorded answer through both, and the owner's
  real run with their key).

### S4b: The Roadmap tab (done)

Result (2026-11-06):

- App (`src-tauri/src/roadmap.rs`): Susun roadmap over a Tauri channel (the request, the repair,
  the end with the problems left), stopped by the conversation's Hentikan, refused while the
  roadmap is locked; the draft's edits (one checkpoint, add after, remove, move, Ubah roadmap,
  discard); a checkpoint appended to an approved roadmap (refused with its problems, nothing
  saved); Windows' folder dialog (`IFileOpenDialog` with `FOS_PICKFOLDERS`, no new dependency) and
  a typed folder checked by Hub; approval; the folder and Akses Claude changed later.
- UI: a Roadmap tab beside Ringkasan and PRD (followed by the stage, in the narrow switch too).
  Checkpoints as rows on paper: code in mono, title, goal, status as a word with a glyph,
  difficulty, features; Rincian as a disclosure (tasks with codes and steps, dependencies,
  criteria, required tests with their command, risks, deliverables, DoD). The draft is edited in
  place (fields, PRD features and earlier checkpoints as checkboxes, tasks, tests; the two DoD lines
  Hub keeps are left out of the editor and named in a hint), reordered with Naikkan and Turunkan
  (announced) or by dragging the grip, added to after a row, and removed with one question; its
  problems link to their checkpoint and keep Setujui roadmap disabled. Approval is one dialog: a
  new Proyek (named, PRD summary) or an existing one, the folder (dialog or typed; missing
  refuses, no `.git` warns), and Akses Claude with what each level allows. After approval: done
  of total, Ubah roadmap and Susun ulang while not started, Tambah checkpoint di akhir, and the
  project's Proyek, folder, and access.
- Checks: 3 new app crate tests (an answer in the schema read by the core's plan type with no
  problems; the schema asks for exactly the fields the core reads, task codes left to Hub, its
  enum words accepted; a broken checkpoint named, not saved), Rust suites as in S4a, clippy clean;
  108 UI tests (6 new: waiting for an approved PRD, Susun roadmap and the stage, edit, add, reorder
  and the problems blocking approval, approval with a new Proyek, the folder and access then Bangun
  and Ubah roadmap, typed folders, a failed writing), typecheck, lint; browser check with the mock
  at 1440 and 960 px wide (the list, Rincian, the dialog, the approved view; no horizontal scroll,
  no console error). The native smoke is not extended in this part: S4c adds the roadmap to it
  together with Pantau.
- Not yet proven: a real Gemini roadmap (owner's key) and the real folder dialog (native).

### S4c: Pantau, the prompt, and the Proyek line (done)

Result (2026-11-06):

- App: the owner's side of a checkpoint at work, every rule in `hub-core::roadmap`: Mulai (Ready),
  Kerjakan sendiri (In Progress), tasks ticked, Tandai terhambat and Lanjutkan with the answer,
  the owner's report (checked first by the same rules as Claude's, then sent), Setujui checkpoint,
  Minta perubahan with a note, answering a question or blocker; the requests and reports of a
  project; "Buat prompt implementasi"; the Studio link of a Hub Proyek.
- UI: a Pantau tab (followed by the Bangun stage) with Sekarang (the active checkpoint or the next
  one to start, its status in a word and glyph, "Tugas: 1 dari 2 ditandai selesai", Claude Code's
  connection and the project's access, and the one action the status needs), the prompt in a
  read-only box with Salin (a warning when Claude has no access), Menunggu kamu (reports to
  review, questions, blockers, each with its action in place), the checkpoints as a compact
  list, and the selected one's reports (tests that did not pass first, output folded). Progress
  as counts. The Proyek page says "Dibangun di Studio: CP-01 sedang ditinjau" and opens Studio.
- Left for S5 by the design: Claude's connection state (shown as "belum pernah terhubung" until
  the bridge exists), Pantau's Riwayat (Jejak's Studio group), and the waiting count on the
  sidebar (only Claude's requests count, and Claude cannot make one before S5).
- Checks: 319 Rust tests in `hub-core`, 56 in `hub-ai`, 22 in `hub-api` unchanged, 8 in the app
  crate; clippy clean; 112 UI tests (4 new: Mulai and the prompt with Salin; building alone with
  tasks, a report refused then sent, Minta perubahan, a second report, approval, the next
  checkpoint, both reports listed; blocked and resumed; the Proyek line opening Studio),
  typecheck, lint; browser check with the mock at 1440 px (Pantau with a report in review, the
  Proyek line; no horizontal scroll, no console error); native smoke 58/58 in the release window
  (52 before, plus: Susun roadmap asking once against the roadmap schema with the PRD fenced,
  Hub's codes and DoD lines in the draft, a folder checked without being read, approval linking a
  new Proyek with the folder and access, the prompt naming the project, folder, and CP-01, and the
  Proyek knowing CP-01 is ready). hub.exe 17.3 MB (15.4 MB in S1).
- Owner checks still open: Susun roadmap with the owner's key on a real PRD; the real folder
  dialog (the smoke passes the folder through the command, the dialog is native); Salin into the
  real clipboard and the prompt pasted into Claude Code (it names `/hub-connect`, which S5 adds).

## S5: MCP and Claude Code (done)

Three parts, each with its own commit: S5a the API for Claude, S5b `hub-mcp.exe`, S5c installing
it and showing Claude's work in Hub. Owner decisions for S5 are in `CONTEXT.md` (a switch per
client, one real Claude Code run, toasts).

### S5a: The API for Claude (done)

Result (2026-11-06):

- `hub-core::claude`: what Claude may see (projects with access, a confirmed folder, and an
  approved roadmap) and do; the folder check (the repository folder or inside it, after
  normalising case, slashes, the `\\?\` prefix, and the trailing separator; `..` refused); the frame (the
  project, its checkpoints, the owner's open notes); the PRD in parts (outline, sections, one
  feature); a checkpoint by code, looked up only inside the project; start (Ready, or Blocked once
  answered), a task by code, a question or blocker with up to four options
  (`roadmap::ask_owner`), the report (its problems returned, nothing kept). Access is checked on
  every call; reading needs `read` or `report`, every change `report`.
- `hub-api`: scopes `studio:read` and `studio:report`; a second client, Claude Code, with its own
  token and `claude-code.json` (`token::ULTRON`, `token::CLAUDE`; ULTRON's scopes no longer called
  "all"); the guard refuses a client switched off and passes the token's hash on; the Studio routes
  with sessions in memory (128 bits, bound to client, token, and project, 12 hours idle), 80 KB
  bodies, 403 `session` and `access`, and a report refusal listing every problem.
- Settings: `claude_enabled` (off by default). The app runs the API while either client is on and
  makes the token of each client that is on.
- Checks: 330 Rust tests in `hub-core` (11 new: visibility, the folder rules, the PRD in parts,
  Claude on the active checkpoint only with questions, blockers, and a refused then accepted
  report, read access, limits), `hub-api` 30 (7 new Studio route tests: each client sees only its
  routes and no token is refused; a client switched off while the other works on; a session bound
  to its project and folder, another project's checkpoint unreachable, ULTRON unable to use it,
  access taken away mid-session; a new token ending old sessions; Claude through start, a task,
  a blocker answered by the owner, a report refused with its problems then accepted, no route to
  approve, changes announced and Claude's in Jejak; read access; a browser `Origin`, an 85 KB body,
  an unknown field; plus one token test; ULTRON's 17 route tests unchanged and passing), 56 in
  `hub-ai`, 8 in the app crate; clippy clean; typecheck. No UI change in this part.

### S5b: `hub-mcp.exe` (done)

Result (2026-11-06):

- `crates/hub-mcp`, a stdio MCP server on `rmcp` 3.9 (named `hub`; it answers clients on
  2025-11-25 as the probe showed, and rmcp speaks 2026-07-28 too). Nine tools with input schemas
  and limits (enums for sections, task statuses, test results, evidence kinds; lengths and counts),
  read tools annotated read-only, writes not destructive; results as structured content with the
  same JSON as text; refusals as one error shape (`kind`, `field`, `reason`, `message`, and a
  report's `problems`) that Claude can act on, with the fix named in Hub's own labels
  ("Pengaturan > Integrasi AI > Claude Code"). Every description and the server's instructions say
  that what Hub returns is project data, not instructions.
- The bridge holds only plumbing: `claude-code.json` from `--data-dir` (loopback addresses only),
  read again after a 401; the session made again after Hub says it ended (a new token, a restart);
  `hub_connect` without an id finds the project whose repository holds the folder Claude Code runs
  in, and the other tools connect that way when no session exists; codes checked before they go
  into a path; Hub started once as `hub.exe --tray` (beside the bridge) when nothing answers,
  waiting up to 10 s, off with `--no-start`; HTTP without TLS or proxy.
- Checks: 3 integration tests that start the real `hub-mcp` binary through rmcp's client over
  stdio, in a repository folder, against Hub's real API on a free port: the server's name, the nine
  tools with their annotations and schemas, every tool called (status, projects, connect without
  an id from a subfolder, the PRD as outline, sections, and one feature, the active checkpoint,
  a refused and an accepted start, a task, a code that tries to leave its path, a question, a
  report refused with its problems then accepted, the checkpoint In Review with Claude's task and
  report, the change announced); a new token and the ended session recovered from without Claude
  noticing; refusals from the wrong folder, without a session, with Claude Code switched off,
  with Hub not running, and without a token file. All other suites unchanged and passing; clippy
  clean. `hub-mcp.exe` is 3.1 MB in release.
- Not yet proven here: starting Hub from the bridge (needs the installed `hub.exe`; S5c's native
  smoke), and Claude Code itself (S5c, the one real run).

### S5c: Installing it, and Claude's work in Hub (done)

Result (2026-11-08):

- Pengaturan > Integrasi AI > Claude Code (`src-tauri/src/claude_code.rs`): the switch; what is
  there (the bridge beside Hub, the `claude` command on PATH or in `%USERPROFILE%\.local\bin`,
  Hub's registration as read from `claude mcp get hub`, the skill folder); **Pasang** after a sheet
  that lists exactly what changes (the one `claude mcp add --scope user hub -- ...` line with the
  real data folder, the skill folder, the switch), refusing to touch an MCP server named `hub` or
  a `hub-connect` skill that is not Hub's; the command to copy when `claude` is not found;
  **Lepas** (asked once) removing only Hub's entry and folder; **Uji jembatan** starting the bridge
  once and talking MCP to it (bridge, Hub, token, each said in words); a new token for Claude.
  `claude` runs without a console window and with a 30 s limit.
- The skill `/hub-connect` (`src-tauri/skills/hub-connect/SKILL.md`, the design's draft with the
  bridge's real behaviour: connect by folder, fix a refused report and send again), marked as
  Hub's; only the read tools are pre-approved.
- The installer carries `hub-mcp.exe` beside `hub.exe` (an NSIS hook; `beforeBuildCommand` builds
  it), and removes it on uninstall. Installer 5.7 MB.
- Claude's work reaches the owner: the API tells the app when Claude connects, is refused (and
  why), asks, or reports (`Host::studio_event`); a question, a blocker, or a report becomes a
  Windows toast that opens the project (`hubapp://open/studio/<id>`, checked like the other
  links); Pantau says "Claude Code terhubung", "terakhir terlihat 14.02", or never, and a refused
  connection with its reason in words; the Studio entry in the sidebar counts what waits for the
  owner (also in the narrow rail); Jejak has a Studio group.
- Found on the way: switching Claude Code on did not start the API or make its token (only
  ULTRON's switch and the port did). Fixed; the native smoke covers it.
- Checks: Rust 331 `hub-core` (the Studio group in Jejak), `hub-api` 30 (Studio events: connected,
  refused with its reason, asked, reported once), 13 in the app crate (the skill pre-approves only
  reading; the registration line; only Hub's own entry and folder count as Hub's; `claude` found on
  PATH or in `.local\bin`; the `hubapp://open/studio` link), `hub-mcp` 3 (helpers now shared with
  the real-run test), `hub-ai` 56; clippy clean; 116 UI tests (4 new: the Claude Code section
  through the sheet, Pasang, Uji jembatan, and Lepas; the command to copy without `claude`; the
  sidebar count; Jejak's Studio group), typecheck, lint; browser check with the mock at 1280 px
  (the section and the sheet, no console error); native smoke 69/69 in the release window (adds:
  switching Claude Code on writes its token file; the real bridge reaching Hub over stdio;
  `hub_connect` by folder; Pantau saying Claude is connected, showing CP-01 In Progress, and the
  report waiting, all without a reload; the sidebar count; a bridge outside the repository refused
  and Pantau saying why; and, with Hub stopped, the bridge starting it into the tray and answering).
  One of three smoke runs failed once at a later, unrelated step (typing into the Studio chat)
  with "Illegal invocation"; the next two runs passed whole.
- The real Claude Code run (`cargo test -p hub-mcp --test real_claude -- --ignored --nocapture`,
  owner's decision; it never touches the owner's Claude Code configuration: `--strict-mcp-config`,
  a test MCP configuration, a test Hub). First attempts stopped before any turn: an expired CLI
  login, then a Claude Code auto-update interrupted on the owner's machine that left the npm
  `claude.cmd` shim pointing at a missing `claude.exe`. Hub now looks for a real `claude.exe`
  (PATH, then `%USERPROFILE%\.local\bin`) before an npm shim (`find_claude`, tested). After the
  owner reinstalled Claude Code, the run passed (2026-11-08): 14 turns, 35.6 s, USD 0.47. Claude
  Code connected by folder, read the checkpoint and the PRD, started CP-01, marked both tasks done
  as a test run, ran `cargo test` for real and reported it failed (exit 101, no `Cargo.toml` in
  the empty test repository) with the reason in `notVerified`; Hub took the report and CP-01 is
  In Review with both tasks changed by Claude.
- Owner checks still open: Pasang ke Claude Code with the real
  `claude` (it changes `%USERPROFILE%\.claude.json` and `.claude\skills`); `/hub-connect` in the
  Claude Code desktop app and the CLI (the desktop app may start MCP servers in another folder; the
  connection is bound to the folder); a toast arriving while Hub is in the tray.


## S6: Validation and polish (done)

Result (2026-11-08):

- **Claude Code in the desktop app** (owner's requirement: not only the CLI). Checked on the
  owner's machine from inside the desktop app's Code tab: it reads the same user-scope MCP servers
  and skills Pasang writes, and its stdio servers start in the session's folder. Two changes in
  the bridge (`crates/hub-mcp/src/place.rs`): the folder is `CLAUDE_PROJECT_DIR` when Claude Code
  names one (documented for every stdio server it starts), the working directory otherwise; and
  the bridge no longer starts Hub when its writes into the data folder are redirected. Processes
  the desktop app starts have `%LOCALAPPDATA%` writes moved into the Claude package's folder
  (without package identity, so the check asks Windows where a probe file really landed); a Hub
  started there would have kept its data where the owner's Hub never looks. Details in
  `docs/studio.md`, "Shape". Claude's desktop chat (outside the Code tab) is not supported: it has
  no project folder.
- **The smoke's "Illegal invocation"** (S5c, once in three runs): the setter was called on a missing
  `#studio-input`, and the input is always rendered inside the project page, so the whole page had
  gone. Cause found in the page: it showed its error view whenever the project query was in error,
  even with the project's data still there, so a refresh that failed in the background (two tries)
  replaced the page, the owner's draft included. Proven with a UI test (it fails before the fix) and
  fixed: the error view only when there is nothing to show. The failing run itself could not be
  replayed: four more runs with a watcher on the input never saw it leave. The smoke now waits for a
  field and says what was on screen when one is missing, and the fake Gemini closes its open
  connection when stopped (it used to hold the run for three minutes at that step).
- **End-to-end native smoke**: from an idea to an approved checkpoint against the fake Gemini and the
  real bridge (an MCP client over stdio): conversation, PRD, roadmap, CP-01 started and reported by
  "Claude", the owner's approval in Pantau, Claude reading CP-01 as completed, the Proyek line, the
  sidebar count gone; the bridge connecting by `CLAUDE_PROJECT_DIR` from another folder, answering
  with Gemini stopped (flow 12), starting a stopped Hub; the conversation after a restart (flow 3);
  memory with Studio open and hidden.
- Found on the way and fixed: approving the last checkpoint said nothing (Pantau switched to "Semua
  checkpoint selesai." and dropped its status line, so "CP-01 disetujui." was neither shown nor
  announced; UI test added); the Roadmap's "Rincian" pointed `aria-controls` at a panel that was not
  there while closed.
- **Accessibility and contrast**: `src/test/a11y.ts` runs the CP5 pass (one h1, unique ids, every
  `aria-*` and `for` pointing at an element that exists, every control named) in the UI tests, on
  each state of the Studio pages: the list, a conversation, a roadmap draft with details, the
  editor, and the approval sheet, the approved roadmap, Pantau with the prompt, a refused and an
  accepted report, Minta perubahan, and Pengaturan > Claude Code with the Pasang sheet. The contrast
  check now also lays the soft tints (`--warn-soft`, `--accent-soft` of each ink, their alpha read
  from the tokens) over each paper: ink and ink-2 meet 4.5:1 on all of them (328 pairs).
- Checks: Rust `hub-core` 331, `hub-api` 30, `hub-ai` 56, app 13, `hub-mcp` 8 (4 new unit tests: the
  folder and redirected writes; 4 integration, one new: the folder Claude Code names, from anywhere,
  and a named folder outside the repository refused; plus two run by hand: the redirection probe and
  the real Claude Code run); clippy clean;
  122 UI tests (6 new: the failed refresh, the last approval, four accessibility passes; the
  Pantau and accessibility journeys allowed 15 s, as they pass 5 s when every file runs at once);
  typecheck, lint (the three empty `catch` blocks lint found in the smoke fixed); contrast 328 pairs;
  native smoke 77/77 in the release window (69 in S5c; adds the approval, Claude reading
  it, the folder Claude Code names, the bridge without Gemini, the conversation after a restart,
  memory). The S6 runs before it stopped at what they found: the silent last approval, a check
  placed after Pantau stops showing Claude's line, and memory read after every page had been
  opened; each fixed. Before the changes, the 69 checks passed in four runs with a watcher on the
  chat input; ULTRON's `scripts/smoke_hub.py` 13/13 against the new
  `hub.exe`; measurements in `docs/architecture.md`, "Resources". Every target met: start 0.61 to 0.77 s, window
  open 121 MB, let go 7 MB, tray 3 MB, main bundle 180 KB gzipped; Studio open 125 MB (about 13 MB
  more than Hari Ini in the same window). No target was set for file sizes: hub.exe 18.8 MB,
  hub-mcp.exe 3.1 MB, installer 5.7 MB.
- The real Claude Code run (S5, 2026-11-08) was not repeated (CONTEXT.md allows one, and the
  bridge's tools did not change); the new folder rule is covered by the integration test
  and the smoke.

### Flows of `docs/studio.md` section 9 and what proves them

| # | Flow | Passing checks | Owner's check |
|---|---|---|---|
| 1 | The key is stored safely | `hub-ai` and app key tests; the Credential Manager test (S1); smoke: the key in no file of Hub's, removed at the end | The entry in the owner's Credential Manager after saving the real key |
| 2 | Uji koneksi | Fake-server tests for every outcome (`hub-ai`); smoke: connected, then offline | Done 2026-11-08: connected with the real key and `gemini-3.8-flash` (after the fix for an answer without `steps`, 904a490) |
| 3 | Brainstorming starts and continues | Core and UI tests; smoke: a conversation streamed in, and back after a restart | |
| 4 | Structured PRD | Validation and repair tests; generation against recorded answers; smoke: three parts, version 1 | Susun PRD with the real Gemini |
| 5 | Revise and approve the PRD | Core tests (versions, approval); UI tests (proposals); smoke: approval | Revision, approval, and Markdown export with the real Gemini |
| 6 | Roadmap from the PRD | Validation tests; generation against recorded answers; smoke: roadmap, approval with the folder | Susun roadmap with the real Gemini; the native folder dialog |
| 7 | Claude Code connects over MCP | `hub-mcp` integration tests; smoke: the real bridge, by folder and by `CLAUDE_PROJECT_DIR`; the real Claude Code run (S5) | Pasang ke Claude Code and Uji jembatan in the owner's configuration; `/hub-connect` in the CLI and in the desktop app's Code tab |
| 8 | Claude reads the right project | API tests (session, other project, other folder); smoke: a folder outside refused | |
| 9 | Progress is stored | API and core tests; smoke: start, report, and approval in Pantau without a reload | |
| 10 | Review and approval | Core transition tests; UI tests of Setujui and Minta perubahan; smoke: the owner's approval | |
| 11 | Unauthorised operations refused | API tests (every case in section 9) | |
| 12 | Hub works when Gemini fails | Smoke with the fake server stopped: Hub works on, Studio says why, Uji koneksi says offline, the bridge answers | Hub with no network at all |

Owner checks still open (a session run by Claude cannot reach these): the rows above; a toast
while Hub is in the tray; `/hub-connect` from the desktop app with Hub closed (the bridge should
answer "ask the owner to open Hub" instead of starting it).

After S6 (2026-11-08), from the owner's first use of the installed build: Uji koneksi with
`gemini-3.5-flash` said the answer was not recognised ("the answer has no steps"). The check gave
the model 64 output tokens; a thinking model can spend them all on thinking, and Google's JSON then
leaves the empty `steps` list out. Hub now reads a missing `steps` as an answer without text, gives
the check 512 tokens, and logs the reason for a failed check (status and field names, never
content). `gemini-3.5-flash` itself is deprecated (Google routes it to `gemini-3.6-flash`); the
owner chose `gemini-3.8-flash` and Uji koneksi connected.

# Studio revision (R1 to R7)

Approved by the owner on 2026-11-08 after an audit of the S6 build (`CONTEXT.md`, "Studio
revision"; design in `docs/studio.md`, "Revision: three stages"). Same rules as before: one
checkpoint at a time, tests, a short report, a commit, and the owner's approval before the next.

| # | Checkpoint | Done when |
|---|---|---|
| R1 | The Studio workspace: the window-high layout, the conversation's own scroll area and the input at the bottom, the side panel (a sheet in narrow windows), the three stages over the stored ones | UI tests updated and passing; no page but Studio's project changes; checked at a wide and a narrow window |
| R2 | Stage 1: merged brainstorming instructions, grouped question sets answered as one message, Yang sudah jelas, the check-answers step | Prompt tests; UI tests of the set and the summary; the eval script updated for the owner |
| R3 | Stage 2: the stack record, the developer profile, Pilih sendiri, Rekomendasi Gemini (structured, checked by Hub), approval | Core, `hub-ai`, and UI tests; upgrade test with no row lost |
| R4 | Stage 3: the PRD's technical part from the stack and a security section, the checkpoint plan inside the stage, the two prompts | Validation and UI tests; export matches |
| R5 | Claude's status: the bridge's session reports and goodbye, sessions and activity stored, the three signals in Bangun and Pengaturan | API, `hub-mcp`, and UI tests; native smoke with the real bridge |
| R6 | Hub's navigation: Proyek & Tugas as one place, the grouped sidebar, Pengingat in Kalender, aliases for old addresses | UI tests; ULTRON's `smoke_hub.py` against the build |
| R7 | Validation: native smoke, accessibility, contrast, narrow and wide windows, every suite, screenshots before and after, docs | Every check passing or its gap reported |

## R1: The Studio workspace (done)

- A Studio project is a workspace: the Shell gives `/studio/<id>` the window's height under the top
  bar (`main[data-workspace]`, up to `--workspace-max` 1,920 px), and only the conversation and the
  documents scroll. Before: a 440 px column in a 1,180 px page that scrolled as a whole, so on the
  owner's 2,560 px screen the header and Ringkasan scrolled away and most of the window was empty.
- Header: the name, the three stages as tabs (`src/pages/studio/steps.ts` maps the stored stages:
  `explore` and `clarify` are stage 1, `prd`, `roadmap`, and `build` are stage 3), and the panel's
  button. Stage 2 says it is being prepared (R3); a project past it shows it as skipped, not done.
  Stage 3 is open from the start, as the PRD tab was, so the PRD can still be written early.
- Body: a main area and a side panel (`--panel-width`, 320 to 420 px). Stage 1: the conversation,
  its text kept to `--reading-max` (760 px) and centred, with Ringkasan beside it. Stage 3: PRD,
  Rencana checkpoint, and Bangun (Pantau) as the main area, with Percakapan and Ringkasan beside it.
  The conversation stays mounted in one place and the grid moves it, so a streaming reply or a
  draft survives a change of stage or side (UI test).
- Conversation: its own scroll area that follows new text only while the owner is at the end
  ("Ke pesan terbaru" otherwise, and a change of size keeps the end in view); the input sits at the
  bottom and grows to about eight lines.
- Under 860 px of workspace the panel is a sheet over the right side: closed by default, Esc or its
  close button closes it, focus goes into it and back to the button. A dot on the button says
  Ringkasan grew while it was out of sight.
- Gemini's proposals use the stage words the owner now sees ("Lanjut bahas lingkup", "Susun PRD").
- Checks: 126 UI tests (2 new: a draft through a change of stage and side; the panel and stage 2),
  the Studio tests moved to the new tabs; typecheck, lint, contrast 328 pairs; native smoke 77/77
  in the release window (the smoke opens stage 3's sections through the stage tab now); memory with
  Studio open 131 MB (125 MB in S6), under the 180 MB check. Checked by hand in the browser at 2,560,
  1,600, 1,280, and 960 px wide: wide panel, sheet, stage 3 with the conversation in the panel.
  No Rust changed, so the Rust suites were not run again.

## R2: Stage 1, Mau buat apa? (done)

- Migration 0006 adds `studio_ledger.area` (`problem`, `users`, `features`, `constraints`,
  `success`), a column added in place: no table rebuilt, every row kept (upgrade test from schema
  5). The core reads, writes, and edits it; the owner can set it when adding or editing an item.
- Gemini's instructions: one stage-1 text for exploring and clarifying, built around the five parts;
  rule 4 asks for one set of questions grouped by part instead of three at a time; `catat` and
  `tanya` name the part of every note and question, `tanya` allows eight questions, five options,
  and `multiple`; `usul_tahap` proposes only `prd`. The project block tells Gemini which parts are
  clear, partly clear, or not yet. An unknown part is left unplaced, never refused.
- The question set: grouped by part, chips for one or several answers, "Belum tahu", "Jawaban
  lain", a count of what is answered, and "Kirim jawaban", which sends one message of answers and
  what was left for later without touching the input's draft. Older stored questions show ungrouped.
- Ringkasan opens with Yang sudah jelas (each part's state as a glyph and a word, and what settles
  it), then its items grouped by part, assumptions and open questions tagged as such.
- Cek ringkasan sebelum lanjut: opened from Gemini's proposal or from Ringkasan; each part with its
  items and state, Ubah per part (Ringkasan opens and focuses that part), what is not clear said
  plainly, and Susun PRD (Lanjut ke tech stack once R3 exists).
- `studio-eval` (for the owner, with their key) now checks one set of at most eight questions, each
  naming its part, covering more than one part.
- Checks: Rust `hub-core` 333 (the area kept and edited; the upgrade from schema 5), `hub-ai` 57
  (sets trimmed to eight, parts kept or left out, the proposal only to `prd`, the clarity rule, the
  tool schemas), the rest of the workspace unchanged and passing; clippy clean. 130 UI tests (the
  set sent as one message with the draft kept, the check-answers sheet with Ubah and Susun PRD, the
  clarity rule in TypeScript with the same cases as Rust, an item placed by part); typecheck, lint,
  contrast. Checked by hand in the browser at 1,600 px: the set, Yang sudah jelas, the sheet.
- Not run: the native smoke (its two new checks are written: the note keeps its part, Yang sudah
  jelas counts it) and `studio-eval` against the real Gemini (the owner's key and quota). With a
  game running on the machine the UI suite timed out at random when every file ran at once; it
  passes in full with two workers (`npx vitest run --maxWorkers=2`).

## R3: Stage 2, Tech stack (done)

- Migration 0007 adds `studio_stacks` (a new table only; upgrade test from schema 6 with every row
  kept). hub-core `stack`: the seven layers once each in order, lengths, a save is always a draft,
  approval only with every layer chosen, Gemini's answer read with its mistakes named, and
  `require_for_prd`. `StudioProject` carries `stackStatus`, so a project with a stack is in stage 2.
- The developer profile in settings (`dev_knows`, `dev_learning`, `dev_cost`, `dev_deploy`) and in
  Pengaturan > Integrasi AI > Profil developer.
- `hub_ai::stack`: the structured request with the profile and the project as data, the current
  stack and the owner's words when asking for a change, one repair; `[task.stack]` in `studio.toml`.
  Every later request carries the stack as data (`ChatContext.stack`).
- The app: `studio_stack`, `save_studio_stack`, `approve_studio_stack`, `recommend_studio_stack`
  (stoppable with Hentikan), allowed in the capability; `generate_prd` refuses before the stack is
  approved.
- The UI: stage 2 with the two ways in, the seven layers (typed or an alternative taken, the earlier
  choice kept), "Minta Gemini mengubah", the approval sheet with "Setujui dan susun PRD"; stage 3
  locked until the stack is approved; the check-answers sheet goes on to stage 2; the data note
  names the profile.
- Checks: Rust `hub-core` 347 (the stack's rules, the profile, the upgrade from schema 6), `hub-ai`
  59 (the request, the change, the repair, the stack in the context), the rest of the workspace
  passing; clippy clean. 134 UI tests (stage 2 picked by hand and by Gemini, an alternative taken,
  a change asked, a failure, stage 3 locked, the profile saved; the earlier Studio tests go through
  stage 2 now); typecheck, lint, contrast. Native smoke 82/82 in the release window (new: the note
  keeps its part, Yang sudah jelas counts it, stage 3 waits for the stack, the recommendation
  carries the profile, the approved stack is kept); memory with Studio open 134 MB. Checked by hand
  in the browser at 1,600 px: stage 2 before and after a recommendation, the profile in Pengaturan.
- Not run: `studio-eval` and a stack recommendation against the real Gemini (the owner's key).

## R4: Stage 3, the PRD from the stack and the two prompts (done)

- The PRD gains `security` (Persyaratan keamanan: `SEC-01` ids, an area, the requirement, how it
  is checked; at least one, unique ids) and `technical.fromStack`. Both default when absent, so every
  stored PRD still reads; one from before R4 is asked for a security requirement before approval.
  No migration: the PRD is JSON.
- `prd::with_stack` puts the approved stack's technologies into a PRD (one line per layer);
  `generate_prd` applies it to what Gemini writes, and `apply_stack_to_prd` (Perbarui dari tech
  stack) brings a draft back in line after the stack changed. `PrdView.stack` lets the page say when
  the draft differs. Markdown lists the stack as approved and the rest as proposals on it, and adds
  Persyaratan keamanan.
- Gemini writes security in the third part (`[task.prd]` says how, and that the stack is fixed);
  Claude can read the `security` section over MCP (the section limit follows the sections there are).
- `roadmap::standalone_prompt` ("Mandiri"): the working rules, the checkpoint with tasks, tests,
  and DoD, the plan around it, the approved stack, and the PRD as Markdown, nothing that needs the
  bridge. Bangun offers "Dengan Hub" and "Mandiri" side by side, each with what it is for.
- The PRD tab shows the technologies "dari tech stack yang disetujui" and the security section, and
  edits and revises it like the others.
- Checks: Rust `hub-core` 352 (the security rules, PRDs from before R4, the stack in the PRD and in
  Markdown, the standalone prompt), `hub-ai` 59 (the third part asks for security), `hub-api` and
  `hub-mcp` suites passing with security in their PRDs; clippy clean. 135 UI tests (the stack and
  security in the PRD, a stack changed after the PRD and followed on request, the Mandiri prompt);
  typecheck, lint, contrast. Native smoke 84/84 in the release window (new: the PRD's technologies
  and security, the Mandiri prompt); memory with Studio open 126 MB. Checked by hand in the browser
  at 1,600 px: the technical and security sections.
- Not run: a PRD with security written by the real Gemini (the owner's key).

## R5: Claude's status (done)

- Migration 0008 adds `studio_claude_sessions` (a new table only; upgrade test from schema 7 with
  every row kept). hub-core `claude_sessions`: a session from the bridge's hello, with the
  Studio project whose repository folder holds its folder (the innermost); beats; the last tool
  call with its outcome, its reason, and the project it named (an unknown id falls back to the
  session's); the goodbye; open while heard from in the last 150 s, otherwise closed or quiet;
  30 days and 100 rows kept. Not in Jejak and not in the export (the export test lists it as left
  out on purpose). `Entity::ClaudeSession` tells the windows; ULTRON's stream does not carry it.
- The API: `POST, DELETE /v1/studio/bridge` and `POST /v1/studio/bridge/beat`, named by the
  `Hub-Bridge` header, `studio:read` only, strict bodies (only the bridge's nine tools). The
  in-memory presence in the app and the connect events are gone; refusals now come from the
  bridge's report of the call.
- The bridge: hello five seconds after the client initialised or at the first tool call (a
  health check like `claude mcp get` is no session), a beat every 60 s and after every call (in
  the background, never starting Hub), a goodbye when stdin closes. `--no-reports` for Uji
  jembatan.
- The UI: `ClaudeStatus` (three rows: Jembatan, Sesi, Aktivitas; glyph, line, detail; "Status
  belum diketahui" without evidence) in Bangun, with the access line and how to connect while
  nothing is known, and in Pengaturan > Claude Code for every session (it replaces "Terakhir
  dipakai"). The signals are asked again every 30 s and on every session notice, which refreshes
  nothing else; the `claude` command's answer is kept five minutes.
- Found by the native smoke and fixed: a call made just before Claude Code closed the bridge could
  lose its report (the report runs in the background and the process ended first). The goodbye
  now waits for the reports still on their way (within its 2 s) before it is sent; integration
  test added.
- Checks: Rust `hub-core` 367 (the sessions' rules, the upgrade from schema 7), `hub-api` 31 (the
  bridge's routes: scope, bodies, 404 after a goodbye, no Jejak row), `hub-mcp` 8 integration (4
  new: a session with its calls and goodbye through the real binary, a refusal counted for the
  project it named, a call just before closing still reported, a health check and `--no-reports`
  leave nothing) and 4 unit, app 13; clippy clean. 140 UI tests (5 new: the three signals'
  wording, Pantau following the bridge's reports with only the signals asked again, Pengaturan's
  sessions and last call); typecheck, lint, contrast 328 pairs. Native smoke 86/86 in the release
  window with the real bridge (new: Pantau's signals from the bridge's hello and calls, the
  refusal with its reason, the goodbye stored with the calls, Pengaturan's last call); memory with
  Studio open 127 MB. Checked by hand in the browser at 1,600 px and 375 px: Pengaturan's three
  signals, no overflow.
- Not run: the real Claude Code session against the new bridge (its hello, beats, and goodbye
  from the CLI and the desktop app's Code tab); the release window of the smoke was built before
  the last CSS change (the glyph set on the first line), checked in the browser instead.

## R6: Hub's navigation (done)

- The sidebar in two groups (`src/app/nav.ts`, `PageGroup`): Hari Ini, Kalender, Proyek & Tugas,
  Studio, Belajar; a quiet line; File, Jejak; Pengaturan at the bottom.
- **Proyek & Tugas** at `/proyek` (`src/pages/tasks/WorkPage.tsx`): it opens on the projects by
  Ruang (the former Proyek page, now `ProjectsOverview`), and its place list holds the task lists
  under "Tugas": Kotak Masuk, Mendatang, Semua, Selesai, with their counts. A task opens in the
  panel beside its list (`?view=...&task=...`). Tugas's Hari Ini list is gone: a task opened on
  Hari Ini opens here among every open one. A project's own page stays `/proyek/<id>`, and the
  sidebar marks Proyek & Tugas there too.
- **Pengingat in Kalender**: a fourth view beside Hari, Minggu, Bulan (`/kalender?view=pengingat`,
  key P), without the day stepping and its keys; leaving it returns to the week. The line above
  every page links there.
- **Old addresses as aliases** (`router.tsx`, `tugasTarget`): `/tugas?view=inbox|upcoming|anytime|
  done` to that list, `/tugas` to Semua, `/tugas?view=today` to Hari Ini (with a task, to that
  task among all open ones), `/tugas?view=reminders` to Pengingat. `/proyek` was already the
  projects and still is. Notifications and the tray open `/`, the review, and Studio, unchanged;
  ULTRON uses the API only, so nothing on its side changes.
- No data moved: the core's task views (the dashboard's `today` included) are as they were.
- Found by the native smoke and fixed (R5's status): one run in three showed another session's
  refusal as the project's last call. Calls were timed to the second, and two sessions had called
  within the same one, so the order between them was left to their ids. A call's time is now kept
  to the millisecond (`claude_sessions`, the column's text keeps its order); core test added. The
  failing run's own detail was cut short, so the cause is the one found in its data, not proven
  by a replay; the next two runs passed.
- Checks: 146 UI tests (new: the place opening on the projects with the lists and their counts,
  and its accessibility pass; a task from Hari Ini opening here; four old addresses; Pengingat in
  Kalender with its accessibility pass and the way back to the week; the sidebar's groups and
  the project page marking Proyek & Tugas; the palette finding tasks and reminders by those
  words); the earlier Tugas and Proyek tests moved to the new place. Typecheck, lint; Rust
  `hub-core` 368 and every other suite passing, clippy clean. ULTRON's `smoke_hub.py` 13/13 against
  the R6 build (before the millisecond fix, which ULTRON does not touch). Native smoke 87/87 in the
  release window (new: Proyek & Tugas lists the captured task, the sidebar's groups, `#/tugas`
  still finding ULTRON's task); memory with Studio open 158 MB (Hari Ini 133 MB in the same run,
  both higher than the 127 and 112 MB of the run before; under the 180 MB check). Checked by hand
  in the browser: the place, the sidebar rail with its line, Pengingat reached by its old address.
- Not checked: the narrow layout of Proyek & Tugas below 800 px (the place list keeps its column,
  as Tugas did); a visual pass at the owner's 2,560 px screen is left for R7.

## R7: Validation (done)

Every check of the revision on the build that closes it (2026-11-09).

- **Accessibility**: the S6 pass (`src/test/a11y.ts`: one h1, unique ids, every `aria-*` and `for`
  pointing at an element that exists, every control named) now also runs on what R1 to R6 added:
  stage 1's question set and Yang sudah jelas, the check-answers sheet, stage 2 before and after
  a recommendation with a layer's reasons open, its approval sheet, the PRD from the stack with
  its security, Bangun with Claude Code's signals and the Mandiri prompt, Pengaturan with the
  signals and the developer profile; and on every page of the sidebar on its own (Hari Ini,
  Kalender, Pengingat, Proyek & Tugas with the projects and two lists, Studio, Belajar, File,
  Jejak, Pengaturan). No problem found.
- **Windows**: Hub's window is at least 960 x 640 (`window.rs`), so the narrow check is 960 px; 375
  and 800 px cannot happen in the app (the R6 gap below 800 px is therefore none). Measured in
  the browser at 960, 1,600, and 2,560 px on 15 views (every stage and section of a Studio
  project built to Bangun, every page, Pengingat, two task lists): no page wider than the window,
  no element past its edge, no sideways scrolling. Found at 2,560 px and fixed: Rencana
  checkpoint and Bangun spread to about 1,500 px, so their hints ran to lines of 200 characters
  and a checkpoint's status sat far from its name. Stage 3's documents now keep a document's
  width (1,040 px, as the tech stack keeps 880 px); the PRD's paragraphs were already held at
  72ch.
- **Contrast**: 328 pairs, lowest 4.57:1 (senja: warn on paper-2); the signals' glyphs repeat in
  shape what their colour says.
- **Every suite**: Rust `hub-core` 368, `hub-api` 31 (6 + 17 + 8), `hub-ai` 59, `hub-mcp` 8
  integration and 4 unit, app 13 (the real Claude Code run and the Credential Manager test are
  run by hand, as before); clippy clean. 160 UI tests (14 new: the accessibility passes above);
  typecheck, lint.
- **Smokes**: native 87/87 in the release window (the real bridge, the fake Gemini); ULTRON's
  `smoke_hub.py` 13/13 against the same build.
- **Resources** (`scripts/measure.ps1`, `docs/architecture.md`, "Resources", column R7): start
  0.53 to 0.58 s; window open 116 to 126 MB; let go 5.6 to 5.7 MB; tray 3 MB; main bundle 181 KB
  gzipped; Studio open 127 MB beside 111 MB for Hari Ini. Idle CPU with the window open read 2.5
  to 4.6 % in the measure (its sample starts 8 s after the window shows; S6 read about 1 %), and
  0.47 % when sampled after a minute open, so the higher reading is the window settling, not
  something that keeps running. One run read 5.3 % in the 10 s after hiding the window (others
  0.16 to 0.31 %; CP3 saw about 3 % there): a moment after hiding, not at rest.
- **Screenshots before and after**: the S6 smoke's window (before R1) beside this one's, 16
  pairs, in `revisi-studio-sebelum-sesudah.html` at the repository's root (not committed: 3 MB of
  pictures; made again from any two smoke folders).
- Owner checks still open (a session run by Claude cannot reach them): the revision with the real
  Gemini (a question set, a stack recommendation, a PRD with security); Claude Code's real session
  against the new bridge, from the CLI and the desktop app's Code tab (Bangun should say "Terbuka
  sekarang", then "Ditutup" when it closes); the rows of "Flows of `docs/studio.md` section 9"
  above.

## P1: Pengaturan in parts (done)

Asked by the owner on 2026-11-09 after R7: Pengaturan split by purpose, as Claude's own settings
are, the sidebar giving way to the parts while it is open and coming back smoothly. Their choices:
five parts (Claude Code inside Integrasi AI), Kembali to the page before, the parts as icons in the
rail, and the clearer Claude Code text at the same time. Design in `docs/design.md`, "Pengaturan
in parts".

- Five parts, each its own page and address (`src/pages/settings/parts.ts`): Tampilan, Hari &
  kebiasaan, Integrasi AI, ULTRON, Sistem & data, at `/pengaturan/<part>`. Each page says
  "Pengaturan" above its own name (the h1); a part with several sections labels each (Gemini,
  Profil developer, Claude Code as section labels now, not subheads). `/pengaturan` alone and a
  part that does not exist open Tampilan; Studio's links to the key and the profile open
  Integrasi AI. No setting changed, only where it is.
- The sidebar (`Shell.tsx`) holds two sides, the pages and the parts. Opening Pengaturan slides
  the pages out to the left and the parts in from the right (240 ms, the standard ease; only a
  cross-fade when Windows asks for less motion); the side not shown is inert and hidden from
  screen readers. Kembali returns to the last page outside Pengaturan, or Hari Ini.
- Focus follows to the side that came in, to the item it marks as current. Found by the UI test:
  the move ran while the address was still `/pengaturan`, before it was sent on to its first
  part, so nothing was marked current and focus went to Kembali; and it also took focus into the
  sidebar when the app first opened. It now waits for the route to settle, and moves only when
  the sides change with focus in the sidebar (or lost with it).
- Claude Code's text says it is for the CLI and the desktop app's Code tab alike: the intro, the
  Pasang sheet (both ways to start, and that Claude's plain chat cannot use Hub), the notice after
  Pasang, and Bangun's hint.
- Checks: 167 UI tests (new: Pengaturan opening on its first part with the sidebar's parts,
  focus following both ways and none on opening the app, Kembali to the page before and to Hari
  Ini, old and unknown addresses, every part reachable with its controls, the Pasang sheet's
  text, Studio's link to Integrasi AI, the accessibility pass on each of the five parts; the
  earlier Pengaturan tests moved to their parts). Typecheck, lint, contrast 328 pairs. No Rust
  changed. Native smoke 90/90 in the release window (new: `#/pengaturan` opening Tampilan with
  the parts in the sidebar and the pages' side inert, the ink in Tampilan, the API in ULTRON, the
  data folder in Sistem & data, Claude Code's last call in Integrasi AI, Kembali to Proyek &
  Tugas; the R6 sidebar check now reads the pages' side only); memory with Studio open 131 MB.
  Browser at 960, 1,600, and 2,560 px: every part, nothing past the window's edge; in the rail
  the parts and Kembali are 44 px icons with their names hidden and given as tooltips.
- Not watched: the slide itself. The Browser pane was hidden, so its transitions stayed at their
  start; the end positions were checked with transitions off (pages' side at -100 %, hidden and
  inert; the parts at 0, shown), and the timing read from the computed style. The release window
  of the smoke ran it, but a smoke cannot judge smoothness: that is the owner's look.

## P2a: Sound (done)

- Settings `sound_enabled` (on) and `sound_volume` (30, 0 to 100, refused above) in hub-core; no
  migration (settings are rows of key and value).
- `src/sound/`: the palette (the place from the address, a voice and a pentatonic root per place,
  the light's shift and brightness, each cue's notes, spacing, gain and ring, Belajar's quiet), the
  Web Audio engine (one context made on the first sound, a low-pass for the light, Hub's volume
  on a curve, 80 ms between sounds, silent while hidden or where there is no Web Audio), and the
  provider (switches, tabs, radios, and checkboxes sound by themselves; any control may name its
  cue in `data-sound`).
- Moments wired: a task checked or reopened (the last open focus task of the day is the hero
  sound), Catat Cepat saved (paper, in the main window and its own), dialogs, Pantau's task checks,
  the approval of a checkpoint, the prompt and the Claude Code command copied, Tutup Hari closed,
  Belajar's session start and stop and its weekly goal reached, blocks set down in Kalender.
- Pengaturan > Tampilan: Cahaya & tinta, and Suara (Efek suara, Volume saved 350 ms after the last
  move, Coba suara for five places).
- Checks: `hub-core` 369 (sound settings kept, refused above 100), clippy clean; 182 UI tests (15
  new: the palette's rules, the engine's context, gap, hidden window and quiet places, and the
  sounds where they happen, off, previewed, and with the volume saved once); typecheck, lint.
  Checked in the browser: the Suara section, an AudioContext made and running on Coba, no console
  errors.
- Not checked: how the sounds sound (a session run by Claude cannot hear); the owner's ear decides,
  with Coba suara, and the palette is the one place to tune them.

## P2b: Micro animation (done)

- `src/motion/motion.ts`: `useGlide` (a group's pill or underline follows its chosen item by
  clip-path), `useFlip` (rows keyed by `data-flip` slide to their new place; new rows fade in, not
  on the first paint), and `leave` (a dialog or toast leaves as a still, unreachable copy without
  ids; the real one is removed at once). All idle where there is no Web Animations API and still
  under Windows' reduced motion.
- Applied: every `Segmented`, Pengaturan's Cahaya and Belajar goal, Pantau's prompt versions,
  Studio's stage, section, and side tabs (the selected item no longer draws its own background or
  underline), task lists, every dialog, every toast. Already there and kept: the check's ink, the
  switch's spring, the pressed button.
- Checks: 186 UI tests (4 new: the clip from a chosen item and a line, the rows' moves, the copy
  that leaves and its cleanup, nothing under reduced motion); typecheck, lint. In the browser: the
  pill's clip follows Kalender's views, a closed dialog leaves two copies (backdrop, sheet) with no
  ids and inert that are gone after 450 ms, no console errors.
- Not seen moving: the Browser pane was hidden, so transitions there did not run frames; the
  values were checked with transitions off. The motion itself is for the owner's eye in the app.

## P2c: Validation of sound and motion (done)

- Found and fixed: once a sound had played, its AudioContext stayed running (an audio thread at
  rest). The engine now suspends the context 1.5 s after the last sound has rung out and wakes it
  for the next (engine test).
- The native smoke checks both in the real window: Coba suara makes Studio's notes with the context
  running, and the context is suspended once they have rung out; Kalender's view pill is caught
  mid-glide (its clip neither where it was nor where it goes) and lands on the new choice.
- Every suite: Rust `hub-core` 369, `hub-api` 31, `hub-ai` 59, `hub-mcp` 8 and 4, app 13; clippy
  clean. 187 UI tests (the accessibility pass on every page and part included), typecheck, lint,
  contrast 328 pairs. Native smoke 93/93 (new: the sound and its rest, the gliding pill);
  ULTRON's `smoke_hub.py` 13/13.
- Resources (`docs/architecture.md`, "Resources", column P2): the first run after the build read
  150 MB and 1.4 % with the window open; warm runs 116 to 128 MB and 0.9 to 2.2 % in the measure,
  and about 0.5 % after 90 s (renderer 0 %), as in R7. The page does no work at rest (no running
  animation; no layout, style, or script in 20 s, by DevTools' metrics). Main bundle 188 KB
  gzipped (181 KB in R7).
- Not checked: how it all feels and sounds to the owner (a session run by Claude cannot hear, and
  watched the motion only through measurements). Pengaturan > Tampilan > Suara has the switch,
  the volume, and Coba suara; `src/sound/palette.ts` is the one place to tune the sounds.


# Studio like Gemini (U1 to U6)

Decisions in `CONTEXT.md`, "Studio like Gemini" (2026-11-09). One checkpoint at a time, a short
report, a commit, and the owner's approval before the next.

| Checkpoint | Scope |
|---|---|
| U1 | Installing `/hub-connect`: the skill is written without the `claude` command, and a bridge registered without it is named. |
| U2 | Studio's sidebar: Hub's sidebar slides over to Studio's history (back to Hub, Ide baru, search, waiting and recent projects), folding into a rail. |
| U3 | Studio's home like Gemini: the greeting in the middle by the light of the day, the pill input, the project list moved to the sidebar. |
| U4 | The workspace: one conversation with a canvas on the right, result cards with Buka and Setujui, one small stepper, the chained steps (stack to PRD, PRD to plan). May be split in U4a (layout) and U4b (cards and chaining). |
| U5 | A deeper PRD: Gemini's PRD instructions rewritten from the research, the schema sent in its own key order, the user flow as data Hub draws as a diagram (and writes as Mermaid in the Markdown export and for Claude). Older PRDs stay readable. |
| U6 | Validation: every suite, the native smoke, wide and narrow windows, malam, accessibility, regressions, docs. |

## U1: Installing `/hub-connect` (done)

- Cause (2026-11-09): the owner registered the bridge by hand with the sheet's `claude mcp add`
  line. Claude Code then had Hub's tools, but `~/.claude/skills/hub-connect` was never written:
  only Pasang wrote it, and only after the `claude` command was found and the registration
  worked. So `/hub-connect` did not appear in a new session.
- Pasang now writes the skill first, needing no `claude` command, and registers after it. Without
  the command the sheet keeps the line to run by hand and offers **Tulis skill** (the skill and the
  switch), then **Periksa lagi**. A bridge registered without the skill is named in Pengaturan
  ("Server hub sudah terdaftar ..., tapi skill-nya belum ada") and the button reads **Lengkapi
  pemasangan**.
- Tests: Rust `hub` 14 (new: the skill written into a folder that does not exist yet, replacing
  Hub's older copy), clippy clean; UI 188 (new: the skill written without the command; the hand
  registration named and completed), typecheck, lint, build. In the browser preview the hand
  registration showed the line and **Lengkapi pemasangan**, and Pasang completed it.
- Not checked: the real Pasang on the owner's Hub (a session run by Claude does not install into
  the owner's Claude Code). The owner presses Lengkapi pemasangan once and opens a new session.

## U2: Studio's history in the sidebar (done)

- Opening Studio slides the sidebar to Studio's side (`src/app/StudioSide.tsx`, the pattern of
  Pengaturan's parts): Kembali to the Hub page left, the fold button, Ide baru, a search over names
  and ideas, the projects (Menunggu kamu with counts, then Terbaru), Pengaturan at the foot. Design
  in `docs/design.md`, "Studio's history".
- Folding: a rail of icons, kept on this computer (`hub:studio-folded`); in a narrow window the
  history opens over the page and closes with Esc, a click outside, or a new page.
- The rail now follows the sidebar's own width (a container query) instead of the window, so a
  narrow window and a folded history share one set of rules.
- Found and fixed on the way: the sidebar's buttons (Pengaturan's Kembali since P1) kept the
  browser's own border and grey background; the sides' column could grow wider than the sidebar
  with a long name or a field; and focus moving into a side while it slid in scrolled the sides
  out of place (`overflow: clip` now, not `hidden`).
- Changed on purpose: inside Studio, Hub's pages are behind Kembali (or Ctrl+K). Two tests followed
  that (Pantau's way to the linked Proyek, the waiting count looked up among Hub's pages).
- Tests: UI 193 (new: the order and search of the history, the empty state, Kembali, folding and
  its memory, the rail's search), typecheck, lint, build, contrast 328 pairs. In the browser
  preview: the slide, the fold and rail at 1,440 px, the history over the page at 1,000 px with
  Esc, outside click, and a project closing it, Kembali through Pengaturan and back.
- Not checked: the native smoke (U6), the night light by eye.

## U3: Studio's home like Gemini (done)

- `/studio` is a clean page in the window's middle: the greeting by the light of the day with the
  owner's name, the question (h1), one line on what Studio does, and one pill-shaped field
  (`--radius-xl`, new in `design/tokens.css`) with the cursor in it, growing to about eight lines,
  the model's name as a link to Integrasi AI, and a round send button. The privacy line under it.
  Design in `docs/studio.md`, "Studio's home".
- The project list and its groups left the page (Studio's history in the sidebar since U2); their
  strings went with them. Studio's home is a workspace like a project, so it centres in the window
  and scrolls only when the window is too short.
- Kept for the native smoke and the tests: the field `#studio-idea` labelled "Ceritakan idenya" in a
  form whose submit button is "Mulai".
- Tests: UI 193 (new: the greeting by the hour, the focus in the field, Shift+Enter, the model's
  link, the projects only in the sidebar; three tests now reach a project through the history),
  typecheck, lint, build, contrast. In the browser preview at 1,280 and 960 px: the column centred
  in the main area, a two-line idea growing the field, Enter opening the project with the idea
  sent, and the new project on top of the history.
- Not checked: the native smoke (U6); the preview's screenshots were cropped at this display's
  scale, so the layout was checked by measurement, not only by eye.

## U4a: The workspace, the conversation and a canvas (done)

- `StudioProjectPage.tsx`: the conversation keeps its column in every stage; the canvas beside it
  shows the chosen stage's document (Ringkasan narrow; the tech stack, the PRD, the plan, and
  Bangun wide, with stage 3's three sections as tabs). Eight tabs became the stepper and, only in
  stage 3, its three sections: the side panel's Percakapan and Ringkasan tabs are gone.
- One header row: the name, the stepper, Hapus proyek as an icon, the canvas's button. The link
  back to the list is gone (the sidebar's history and Ide baru replace it).
- A wide document folds Studio's history into the rail and the owner can open it again
  (`src/app/studioCanvas.ts`, `Shell.tsx`); nothing is kept.
- The conversation's input is the home's pill, with round send and stop buttons ("Kirim",
  "Hentikan" as their names).
- Measured in the browser preview at 1,440 px: stage 1 conversation 776 px and Ringkasan 320 px
  with the history open; stage 2 the history folded, conversation 531 px and canvas 733 px; the
  history opened again, 460 and 636 px; the canvas closed, the conversation alone. At 800 px the
  canvas is a sheet.
- Tests: UI 194 (rewritten: the conversation staying while the canvas changes, the canvas's
  button and the stepper opening it; new: the history folding beside a wide document), typecheck,
  lint, build, contrast.
- Not checked: the native smoke (U6). Next: U4b, the result cards and PRD to plan in one step.

## U4b: Result cards and the chained plan (done)

- The stack, the PRD, the plan, and the build are cards in the conversation (`results.ts`,
  `ResultCard.tsx`), placed where each was first made and showing where it stands now, read from
  what Hub stores (nothing new saved). Design in `docs/studio.md`, "Result cards and the chained
  plan".
- Buka opens the document; Setujui (only where the document's own button is enabled) opens it and
  starts its own approval with its own confirmation, through an imperative handle the document
  gives the page (no effect reacting to a prop: React's own advice for events).
- Approving the PRD writes the plan at once when there is none; an existing plan is never replaced
  by itself.
- Tests: UI 197 (new: the order of cards among messages; Buka and Setujui from Ringkasan opening
  the stack's sheet without approving; the PRD card's Setujui asking about open questions first; no
  Setujui while a layer is unchosen; approving the PRD writing the plan and the card saying
  "Disetujui"). The journeys that used Susun roadmap after the approval now wait for the plan the
  approval writes; the document's Setujui is looked up in the canvas, since a card has one too.
  Typecheck, lint, build, contrast. In the browser preview: the stack's card after Gemini's
  recommendation, with the accent edge, its technologies, Buka and Setujui.
- Not checked: the native smoke (U6).

## U5a: The PRD's instructions, the schema's order, and deeper thinking (done)

- `studio.toml`: `[task.prd]` rewritten and `[task.prd_standard]` added (the standard of each
  part, an example feature, a check before sending), read by the PRD and its revisions; the
  technical part separates what is fixed (`constraints`) from proposals Claude Code may decide.
  Both implementation prompts and the `/hub-connect` skill say the same (`roadmap::FREEDOM`).
  Design in `docs/studio.md`, "The PRD's instructions since U5a"; research in
  `docs/research/prd-prompt-research.md`.
- Found and fixed: the PRD schemas reached Gemini sorted alphabetically (`serde_json` without
  `preserve_order`), so Gemini wrote a feature's criteria before its name and description and Then
  before When. `preserve_order` is on for the workspace; every Rust suite passes with it.
- `hub-ai prd.rs`: schema descriptions that say what a good value holds, `minItems` for Hub's
  minimums and `maxItems` 60 for the features, `thinking_level` high for the parts and the repair
  (medium for a revision), output limits 32,768 and 65,536 (the thinking counts), a 600 s timeout,
  the ask last pointing at the standard, and an answer cut at the limit named in the log.
- Tests: Rust `hub-ai` 62 (new: the keys in written order, the thinking and limits, the minimums,
  the standard in writing and revising, the cut answer), `hub-core` 369 (both prompts carry the
  room for Claude Code), `hub` 14, the other crates; clippy clean.
- Not checked: a real PRD from Gemini with the new instructions (it uses the owner's quota and
  sends the project to Google); `studio-eval` has no PRD mode yet.

## U5b: The user flow diagram (done)

- hub-core `prd.rs`: the `flow` section (`FlowChart`, `FlowNode`, `FlowEdge`, `FlowNodeKind`), its
  rules (`check_flow`), and `mermaid()`, used by the Markdown export. PRDs without a diagram (from
  before U5b) read and approve as before.
- hub-ai: the `flow` schema (3 to 30 steps, `minItems` on the arrows too), written in the third part
  after the features; its standard in `[task.prd_standard]`.
- UI: the diagram drawn by Hub's own layered layout (`flowLayout.ts`) as React SVG elements (labels
  are text, never markup), a figure labelled by its caption, the flow as an ordered list under
  "Lihat sebagai teks", a sideways scroll for a wide diagram inside the canvas, and the editor's
  rows for steps and arrows.
- Found and fixed: Claude Code could not read the security requirements over MCP. Hub's PRD has
  had them since R4, but `hub_get_prd`'s list of sections did not name `security`. It names
  `security` and `flow` now.
- Tests: Rust `hub-core` 374 (new: the diagram's rules, Mermaid, a PRD without one), `hub-ai` 62,
  `hub-mcp` 8 (security and flow read over the bridge), the rest; clippy clean. UI 202 (new: rows,
  a decision's ways side by side, the return on the right, a missing step left out and named,
  wrapping, the labelled figure and its text, no markup from labels, an older PRD without one).
  Typecheck, lint, build, contrast. In the browser preview: the demo PRD's diagram with its
  start, screens with their feature, the decision's Ya and Tidak, and the dashed "coba lagi".
- Not checked: the native smoke (U6); a real diagram from Gemini (uses the owner's quota).

## U6: Validation of Studio like Gemini (done)

- Every suite on one tree (2026-11-09): Rust `hub-core` 374, `hub-ai` 62, `hub-api` 17 and 6,
  `hub-mcp` 8 and 4, `hub` 14; clippy clean. UI 202 tests (the accessibility pass on the Studio
  home, the history, the canvas, the result cards, and the PRD with its diagram included),
  typecheck, lint, build, contrast 328 pairs.
- Native smoke 97/97 against the release build (`npx tauri build --no-bundle`), with new checks:
  the sidebar slid to Studio's history with the project current; the PRD in the canvas beside the
  conversation with the history folded; the user flow drawn (6 steps, 6 arrows, its text list);
  the PRD's card in the conversation; the document's own Setujui approving and the plan written at
  once, asked for once. The fake Gemini's PRD now carries a diagram. Memory with Studio open 135 MB
  (under 180), hidden 132 MB.
- Looked at in the real window's screenshots: the result cards (Tech stack Disetujui; Rencana
  checkpoint Draf with Buka and Setujui), the plan in the canvas, the history as a rail. A card's
  Setujui looked paler in one screenshot: it was the card's entrance caught mid-way (the buttons'
  colours are the same, measured).
- In the browser preview: the night light (malam, dark) on the Studio project; the history, the
  canvas, and the sheet at 1,440, 1,280, 1,000, and 960 to 800 px (U2 to U4).
- Not checked: a real PRD from Gemini with the U5 instructions (it needs the owner's key and quota
  and sends the project to Google; the owner decides); the installed Hub, which still runs the
  build from before U1 until the owner installs this one.

# Lentera (L1 to L4)

Decisions in `CONTEXT.md`, "Lentera" (2026-11-09). One checkpoint at a time, a short report, a
commit, and the owner's approval before the next.

| Checkpoint | Scope |
|---|---|
| L1 | Studio's agent is Lentera in the UI and in its own instructions; a reply that asks shows only its questions. |
| L2 | The model chip opens a menu of text models (the global setting); "Ganti model" beside "Coba lagi" when the quota runs out. |
| L3 | The glow on Studio's home by the light of the day; the PRD's loading as three smooth steps. |
| L4 | Validation: every suite, the browser preview in malam and siang, narrow windows, reduced motion, docs. |

## L1: Lentera, and replies that ask (done)

- Every Studio string that named Gemini as the one talking says Lentera (`src/strings/id.ts`;
  the keys became `agent`, `agentHint`, `fromAgent`, `byAgent`). Kept on purpose: Pengaturan >
  Integrasi AI ("Google Gemini", "API key Gemini") and the privacy notes, now "Percakapan Studio
  diproses Google". The PRD's version note written from now on is "Disusun Lentera dari
  percakapan."; older versions keep their note.
- `crates/hub-ai/prompts/studio.toml`: "Kamu adalah Lentera"; a reply that calls `tanya` writes no
  text (rule 4, the explore stage, the tool's description); rule 13 now asks for text only with
  `catat` or `usul_tahap` alone. The transcripts Gemini reads name its turns "Lentera".
- `Conversation.tsx`: a reply with questions shows no text. While it is written, "Menyiapkan
  pertanyaan…" breathes (still under reduced motion); the set being answered stands alone under
  "Pertanyaan dari Lentera" and rises in; an answered set leaves "Mengajukan N pertanyaan.".
- Tests: Rust `hub-ai` 62 (the required rules now include Lentera and "no text with `tanya`"),
  `hub-core` 374; UI 202 (the mock still writes words around its questions, and the tests check
  they are hidden; the answered line), typecheck, lint, contrast 328 pairs. Browser preview: the
  first reply is only the question set; after answering, "Mengajukan 3 pertanyaan." and the next
  reply's text.
- Not checked: a real reply from Gemini with the new instructions (it needs the owner's key and
  quota).

## L2: The model picker (done)

- `src/pages/studio/models.ts`: the text models from Google's list (`gemini-<version>-pro|flash|
  flash-lite`, with a channel and a date at most; speech, image, live, embedding, and alias models
  are left out), newest first (Flash, Pro, Flash-Lite; a stable model before its previews), named
  without "Gemini" ("3.5 Flash"), the saved model kept when the list lacks it. Pengaturan's list
  uses the same filter.
- `ModelPicker.tsx`: the chip under the idea on Studio's home and beside the send button in the
  conversation; a menu button by the WAI-ARIA pattern (arrows, Home, End, Enter, Escape back to the
  chip, Tab or a press outside closes). Google's list is read the first time a menu opens. The
  choice saves `geminiModel`, Hub's one setting, so every project and the next retry use it. The
  menu turns to the other side only when it fits there whole, measured within the window and every
  scrolling area around it, and scrolls itself into view (in the conversation it scrolls down to
  it rather than being cut off by the top).
- A reply that failed for its model (quota, unavailable, billing) shows **Ganti model** beside
  **Coba lagi**; the texts say "Ganti model atau coba lagi nanti". An offline failure shows only
  Coba lagi.
- Tests: UI 209 (new: the model list rules; the chip by keyboard, saved and named; Ganti model and
  the retry with the new model; an offline failure without it; the open menu in the accessibility
  pass), typecheck, lint, build. Browser preview at 800 x 600: the home menu turned upward (no room
  under the chip), the conversation's Ganti model menu opened downward and the log scrolled to show
  it whole, the retry answered with 3.5 Flash-Lite, no console errors.
- Not checked: Google's real model list for the owner's key (the mock lists five models).

## L3: The glow and the PRD's loading (done)

- The glow on Studio's home: three lights per phase and the accent's (`design/tokens.css`,
  registered so they cross-fade with the phase), two soft layers under the input that drift by
  transform only (11 s and 14 s), faded out before the page's edges, still under reduced motion.
  The privacy line over it is ink-2 now. `design/check-contrast.mjs` checks ink and ink-2 over all
  four lights at their brightest on one spot for every phase and ink: 368 pairs pass.
- The PRD's writing (`PrdProgress.tsx`): a card with the three parts as steps (done with a check,
  the current one with a spinner and "Sedang disusun" or "Sedang diperbaiki", the rest waiting),
  joined by a line inked as they finish; "Berjalan 01:05" since it began (not read aloud; the step
  line is the live region); a word that each part takes minutes; an outline of the document with a
  passing light. The document fades in once when the writing ends. `useElapsed` and `clockOf`
  moved from Belajar to `src/time/elapsed.ts` so both clocks are one.
- Tests: UI 212 (new: the part states, the progress with its clock, the repair, the glow hidden
  from assistive technology), typecheck, lint, build, contrast 368 pairs. Browser preview at
  800 x 600: the glow at siang, senja, and malam; the PRD written with the mock slowed to 5 s a
  part (part 1 checked, part 2 spinning, 00:08 on the clock), then v2 faded in; the reduced-motion
  rules present; no console errors.
- Not checked: a real PRD from Gemini (minutes per part with the owner's key); the glow in the
  installed Hub (WebView2) beyond the browser preview.

## L4: Validation of Lentera (done)

- Every suite on one tree (2026-11-09): Rust `hub-core` 374, `hub-ai` 62, `hub-api` 6, 17, and 8,
  `hub-mcp` 4 and 8, `hub` 14; clippy clean with warnings as errors. UI 212 tests (the
  accessibility pass with the model menu open), typecheck, lint, build, contrast 368 pairs.
- Native smoke 103/103 against the release build (`npx tauri build --no-bundle`, the bridge and
  the seed rebuilt too), with new checks: the glow on Studio's home (two lights per layer, moving,
  hidden from assistive technology); the model chip named "3.8 Flash"; the menu from the fake
  Google list without its speech model; the choice saved and changed back; Lentera's first reply
  as its question alone (the fake Gemini's words around it hidden); the PRD's steps seen changing
  (running, then done); the written PRD fading in. Memory with Studio open 138 MB (under 180),
  hidden 134 MB.
- Found and fixed here: the first native screenshot showed faint oval lines in the glow. The
  lights' boxes were rounded, and the round clip cut a faint part of the gradient; the clip is
  gone (each light fades to nothing inside its box). The smoke now waits for the page to settle
  before the glow's picture; the second run's picture shows a smooth light.
- In the browser preview: 960 x 640 (Hub's smallest window), 1,280 x 820, and 1,920 x 1,080 with
  no sideways scroll; the home menu opens upward when there is no room below; the conversation's
  Ganti model menu opens downward inside the conversation and the chip's menu upward, both whole
  on screen, Escape giving the focus back; malam, senja, and siang looked at by eye.
- Not checked: Gemini itself (a real reply with the new instructions, Google's model list for the
  owner's key, a real PRD of several minutes); reduced motion switched on in Windows (its rules are
  in the built CSS, but the preview cannot turn the setting on); the installed Hub, which runs
  the build from before L1 until the owner installs this one.

## L4 follow-up: the model menu tidied, and the same trap closed elsewhere (done)

- Seen by the owner in the installed Hub with a real key (13 text models): the menu's rows ran
  into each other. Cause: the menu is a flex column with a height limit, so its rows shrank to
  their 40 px minimum and their two lines overflowed into the next row.
- Fixed: the menu's parts never shrink (the list scrolls inside instead); the models are grouped
  by family (Flash, Pro, Flash-Lite, then any the owner saved that Hub does not recognise), each
  family's hint said once in its head (the group's accessible description), one line per model.
  When neither side of the button has room for the whole menu, it opens on the roomier side as
  tall as that room, so opening it never scrolls the page away from the input.
- The same trap, audited in every scrolling flex column: Studio's history in the sidebar, File's
  places, and a dialog's body now keep their parts' height too. The other scroll areas (the
  palette, the side panels, the conversation, the canvas, the calendar) are block containers, where
  nothing shrinks.
- Tests: UI 213 (new: the families with their hints said once; the keyboard through the groups),
  typecheck, lint, build, contrast. The mock lists seven models now (two Flash, two Pro, one
  Flash-Lite, a speech and an image model). Browser preview at 800 x 600 in malam: the grouped
  menu opened upward at 314 px over the home's input, the page unscrolled; the conversation's
  Ganti model menu fitted inside the conversation.

# PSAJ finalization (F1 to F5)

Decisions in `CONTEXT.md`, "PSAJ finalization". Audit and plan (in Indonesian, they feed the PSAJ
report): `docs/finalisasi/`. The PSAJ documents named below stay in the private repository; the
public one leaves them out (`export-ignore` in `.gitattributes`). One checkpoint at a time, a short report, a commit, and the owner's
approval before the next.

## F1: Real runs of the main flows (done)

Run on 2026-11-09 against the owner's own Hub (their data folder, their Gemini key, the owner's
leave for the quota and one Claude Code session), driven through the window over a DevTools port
that was open only for the run (Hub restarted normally afterwards).

- **Found and fixed (F1-B1): a reply that asked went on answering itself.** The first real reply
  asked six questions with `tanya`; Hub returned the result, Gemini continued the same turn, wrote
  five "decisions" from its own recommended options, a summary, and a proposal to write the PRD,
  before the owner had answered. Those decisions then shaped the stack and the PRD (a WhatsApp
  reminder the owner never chose). The fake Gemini of the tests and the smoke asked for exactly
  that second round, so nothing caught it. Now `tanya` ends the turn (`stream::reply` stops after
  its round), a stage proposal is refused while the reply asks and dropped when it came first
  (`calls::put_questions`, `put_stage`), and the instructions say that `tanya` ends the turn and
  that answers not given are not recorded. Proven on the real Gemini (3.5 Flash and 3.5 Flash-Lite):
  questions only, one request, no proposal, notes only from the owner's own words.
- **Found and fixed (F1-B2): Susun PRD failed on the real Gemini.** Google refused the features
  part: "The specified schema produces a constraint that has too many states for serving" (400),
  once after 5.5 minutes (3.8 Flash), at once on 3.5 and 3.6 Flash. Cause: `maxItems: 60` on the
  nested features list (U5a); the fake Gemini accepts any schema. Removed; hub-core keeps the limit
  of 60 and the repair asks again. `cargo run -p hub-ai --bin schema-probe` (new, run by hand with
  the owner's key) sends every structured schema to the real Gemini: the features part refused
  before the fix and accepted after (3.5 Flash-Lite); the other parts, the stack, and the roadmap
  accepted. Then on the real Gemini: PRD version 1 in 64 s with no problem (6 features, 3 security
  requirements, a 10-step flow), and the plan written by the approval in 11 s (6 checkpoints, no
  problem).
- **Found and fixed (F1-B3):** a failure kept nothing of Google's reason in `hub.log`, so F1-B2
  could not be read there. `GeminiError::log_line` keeps Google's short message for a refused or
  unreadable answer only.
- **Found and fixed (F1-B4):** a model in high demand (503, Google names the model) and a refused
  request offered no Ganti model, and a 400 read "Jawaban tidak selesai."; both offer it now, with
  texts that say why (UI test).
- **Real Claude Code session** (the CLI, a test MCP configuration with `--strict-mcp-config`, the
  real bridge, the owner's Hub): connected by folder, read the checkpoint and the PRD, started
  CP-01, noted one task as a test run, reported both required tests not run (the run's narrow
  permissions; the demo repository has no code), CP-01 In Review; 16 turns, 29 s, USD 0.51; no file
  in the repository changed. Bangun showed "Terbuka sekarang", each call as it came, then
  "Ditutup 16.40" (R5's signals, first seen with a real session).
- **Learned from Google, not Hub's to fix:** the free tier allows few requests (gemini-3.5-flash is
  served as 3.6 Flash: 5 a minute, 20 a day, used up that afternoon), and models answer 503 "high
  demand" at busy times (3.7 and 3.8 Flash). One Studio flow takes about 10 to 14 requests. F2's demo
  plan works around it.
- Found in the owner's data: a Studio project taken from an idea to an approved plan on the morning
  of 2026-11-09 (before U5) in 6 minutes, with the real Gemini.
- Left in the owner's Hub by the run: three Studio projects (the full flow, "uji ulang", "uji ulang
  2") and the Hub Proyek linked to the first; the demo repository and a seeded demo data folder in
  `..\Hub-demo` (for F2); the owner's model setting put back to gemini-3.5-flash.
- Checks: Rust `hub-core` 374, `hub-ai` 65 (3 new: the turn ends after `tanya`, the proposal
  refused beside questions, the log keeps Google's reason), `hub-api` 6, 17, and 8, `hub-mcp` 4 and
  8, `hub` 14; clippy clean with warnings as errors; UI 214 (1 new: a busy model offers Ganti model),
  typecheck, lint, contrast 368 pairs; native smoke 104/104 (the chat check now expects one request
  and no second round). Memory with Studio open 132.8 MB.
- Not checked here (the owner's): the rows of `docs/finalisasi/MANUAL_TESTS.md`. The installed Hub
  runs the build from before F1 until F2's installer.

## F2: Release 1.0.0 and the demo (done)

- **1.0.0**: the version in `package.json`, the Cargo workspace (every crate), and
  `tauri.conf.json`; `CHANGELOG.md` (in Indonesian: what Hub holds, the F1 fixes, what is known,
  the stages by date); README's status. The audit had said the version was not shown in the app: it
  was (Pengaturan > Sistem & data, with the schema); corrected in the audit.
- **Found and fixed (F2-B1): an update left the old bridge in place.** Installing 1.0.0 over 0.1.0
  replaced `hub.exe` but not `hub-mcp.exe`, because Claude Code (this session) had it running; the
  silent installer said nothing. The NSIS hooks now stop every `hub-mcp.exe` before installing and
  before uninstalling (Claude Code starts the bridge again when it needs Hub). Proven by installing
  again with Hub and the bridge running: the bridge's hash matches the build, Hub was closed by the
  installer and started again.
- **Install test**: silent install into a test folder (2.9 s; `hub.exe` 1.0.0 and `hub-mcp.exe`
  beside it), silent uninstall (the program's files gone, Hub's data folder untouched); then the
  owner's Hub updated from 0.1.0 to 1.0.0 (their data unchanged) and running.
- **`scripts/demo.ps1`**: a demo Hub on its own data folder (`..\Hub-demo\data`, a demo week from
  hub-seed) and port (47741), so a pitch shows no personal data; asks to close the owner's Hub first
  (the hotkey and the tray), `-Reset` for a fresh week (asks first; Studio's demo projects go with
  it), and prints the two lines that point ULTRON at it (`ULTRON_HUB_API_FILE`, `ULTRON_HUB_START`).
  Run for real: the demo Hub listened on 47741 and ULTRON's own client reached it (ping, 6 tasks).
  Plain ASCII for Windows PowerShell 5.1.
- **Documents** (`docs/finalisasi/`): `DEMO_PLAN.md` (data A or B, the day before and the morning,
  a 90-second demo in the jobsheet's order, fallbacks, roles), `BUILD.md` (tools and versions, the
  commands, the release's hashes), `MANUAL_TESTS.md` section F (the demo); the plan's F4 is now the
  owner's report checkpoint (Claude completes the report with what only the builder can get).
- Checks: Rust `hub-core` 374, `hub-ai` 65, `hub-api` 6, 17, and 8, `hub-mcp` 4 and 8, `hub` 14;
  clippy clean with warnings as errors; UI 214, typecheck, lint, contrast 368 pairs; native smoke
  104/104 on the 1.0.0 build; installer `Hub_1.0.0_x64-setup.exe` 6,217,153 bytes, SHA-256
  `1dc26acd...1c82d8`.
- Not checked here: the demo run by the owner with their key and ULTRON's voice
  (`MANUAL_TESTS.md` M31 to M34); another laptop (F3).

## F3: Usability and compatibility tests, prepared; release 1.0.1 (instruments done, the tests are the group's)

- **Instruments** (`docs/finalisasi/`, in Indonesian, for chapter IV of the report):
  `UJI_PENGGUNA.md` (aim, at least five respondents (Nielsen, 2000), the session, seven tasks U1 to
  U7 on the demo Hub with their pass rule, the ten SUS statements (Brooke, 1996; Indonesian
  adaptation, Sharfina and Santoso, 2016), the scoring, Sauro's 68 and Bangor et al.'s acceptability
  bands, APA references); `Lembar-Uji-Pengguna-Hub.docx` (two printed pages per respondent, checked
  by rendering through Word); `Rekap-Uji-Pengguna-Hub.xlsx` (the group fills the yellow cells:
  respondents, B / BB / G and seconds per task, the ten answers; every figure is a formula: success
  rates per task and overall, mean time of successes, SUS per respondent, mean, min, max, standard
  deviation, acceptability). The workbook's 17 formulas were checked against hand-computed answers
  with a formula engine (LibreOffice is not installed): one was wrong at first (an average time that
  read only the first row) and was fixed; no formula errors when empty. `UJI_KOMPATIBILITAS.md`
  (twelve checks K01 to K12 for a second laptop, the recap table with the developer's laptop filled
  in) and `scripts/compat-info.ps1` (reads Windows, processor, memory, graphics, screen and scale,
  WebView2, and Hub's version into `compat-<computer>.txt`; run on the developer's laptop: 2560 x
  1440 at 150 %). The generators are in `docs/finalisasi/alat/`.
- **P2 fixed (release 1.0.1):** the Hub Proyek made by a roadmap's approval takes the PRD's product
  name (`roadmap::proyek_name`, also in `PrdRef` so the dialog says the same name), not the start of
  the idea (core test); a Catatan hit in Ctrl+K opens Jejak on its Catatan filter. P2-01 (an
  unanswered question set still shown in stage 3) judged intended: the owner may answer at any time.
- **1.0.1**: the version everywhere, CHANGELOG, BUILD's hashes; installed on the owner's laptop over
  1.0.0 (bridge replaced, Hub started again). Lint now leaves `docs` alone (its tools run on their
  own).
- Checks: Rust `hub-core` 375 (1 new), `hub-ai` 65, `hub-api` 6, 17, and 8, `hub-mcp` 4 and 8,
  `hub` 14; clippy clean; UI 214, typecheck, lint, contrast 368 pairs; native smoke 104/104 on the
  1.0.1 build; installer `Hub_1.0.1_x64-setup.exe` SHA-256 `8adf243a...226557`.
- Left to the group: the sessions with at least five respondents and the second laptop; the data
  goes into the workbook and feeds chapter IV in F4.

## F4: The report completed with the builder's evidence (done; the group's data still to come)

- **Diagrams from the code** (`docs/finalisasi/diagram/`): the architecture, the use cases (drawn by
  hand as UML with the actors and the Hub boundary: Mermaid cannot lay it out for A4), two
  simplified ERDs from the migrations' foreign keys (productivity data and Studio data), Hub Studio's
  three stages (by hand), and ULTRON's request path as a sequence. Mermaid is rendered offline in
  headless Edge; sources (`.mmd`, `.svg`, `gambar-svg.cjs`) beside the PNGs.
- **Screenshots of the real window** (1440 x 900, `..\BINDO\gambar-laporan\`, outside the repo): the
  productivity pages, Catat Cepat, Tutup Hari, the palette, Pengaturan, and the night light from the
  demo Hub with sample data (a task added through ULTRON's own client shows its mark); Hub Studio
  from a clean run on the owner's Hub with the real Gemini (3.5 Flash-Lite: questions in 7 s with no
  text and no proposal, stack 7 s, PRD 44 s with no problem, plan about 10 s; the new Hub Proyek
  named "Aplikasi Peminjaman Alat Lab TJKT"), and F1's real Claude Code report. The owner's theme,
  model, and sidebar were put back and Hub restarted without the debug port.
- **Documents** (`docs/finalisasi/`): `TEST_REPORT.md`, `KNOWN_LIMITATIONS.md`,
  `PSAJ_REPORT_EVIDENCE.md` (by chapter, with the cost table as the owner stated it, a
  free-for-students pricing proposal, and a break-even simulation for the group to decide),
  `FINAL_RELEASE_CHECKLIST.md`.
- **The report, version 2** (in `..\BINDO\`, outside the repo; its file name is in
  `docs/finalisasi/PSAJ_CONTEXT.md`), 45 pages; the GPT draft left as it was), generated by `docs/finalisasi/alat/buat_laporan.cjs` in
  the jobsheet's format: A4, margins 4/3/3/3 cm, Times New Roman 12, 1.5 lines, justified; roman
  page numbers for the front matter and arabic from BAB I; the contents, figures, and tables as Word
  fields (updated through Word, which also made the review PDF); 6 diagrams and 25 screenshots with
  numbered captions under figures and above tables; chapters I to V, APA 7 references, appendices.
  Hub Studio is now an implemented result; the AI-use statement says Claude wrote the report under
  the group's direction. What only the group can give is marked in yellow: the school logo,
  the members' contributions, the teacher's name, the usability, compatibility, and validation
  results, the price decision, the reflection, and the pitch.
- No product code changed, so the suites were not run again; lint passes (`docs` is outside it).
