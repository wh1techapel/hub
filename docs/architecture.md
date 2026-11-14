# Architecture

## Stack

| Layer | Choice | Why |
|---|---|---|
| Desktop | **Tauri 2** (Rust) | Same stack as ULTRON's shell, so the knowledge carries over. Uses Windows' shared WebView2, no bundled browser. Gives real file paths on drop, a tray, a global hotkey, single instance, autostart, and native Windows calls (file identity, thumbnails, notifications). |
| Data | **SQLite** through `rusqlite` (bundled SQLite, WAL, FTS5) in Rust | The data must be reachable from two doors, the UI and ULTRON's local API, so it lives in the Rust core, not in the webview. `tauri-plugin-sql` was considered and left: it is built for the frontend talking to the database. |
| UI | **React 19 + TypeScript + Vite** | Same as ULTRON's shell. |
| UI data | **TanStack Query** over typed Tauri commands | Caching, loading and error states, and invalidation when the core says something changed (including changes made by ULTRON). |
| Routing | **TanStack Router** | Typed routes, and every page has an address ULTRON can open. |
| Styling | Plain CSS with the tokens in `design/tokens.css` (CSS modules per component) | Distinctive look without a utility framework's defaults; one source of truth for tokens. Keyframes live in `global.css`, so a module names them `global(rise)` (a bare name is renamed by the module and silently never runs), and entrance animations fill `backwards`, not `both`: a kept `transform` would make the page the containing block of its `position: fixed` panels. |
| Motion | CSS only (transitions, keyframes, ULTRON's spring as CSS `linear()`). Drag (task order, the calendar, the tray) is plain pointer events with pointer capture | CP3 was where **Motion** would come in with drag; it was not needed: every drag follows the pointer directly and settles with CSS. CP5 kept it so: nothing needed more, and every animation honours reduced motion. |
| Types across the boundary | **ts-rs** generates TypeScript from the Rust types into `src/data/generated` (`npm run test:core`) | The UI cannot drift from the core. |
| Dates | `jiff` (Rust, time zones and spans), Hub's own RRULE subset for routines (`recurrence.rs`: `FREQ=DAILY/WEEKLY/MONTHLY`, `INTERVAL`, `BYDAY`), the browser's `Intl` with `id-ID` (UI formatting) | Correct local time handling for WIB; no date library in the UI. A school timetable needs only that subset, so the `rrule` crate was not added. |
| Icons | `lucide-react` | One consistent stroke set, tree-shaken. |
| Fonts | `@fontsource-variable/fraunces`, `instrument-sans`, `jetbrains-mono` | Self-hosted, no network at run time. |
| AI (Studio) | Google Gemini over REST from Rust (`crates/hub-ai`: `reqwest` with Windows' own TLS through `native-tls`, HTTP/2, the system proxy), the Interactions API with `store: false`; the key in Windows Credential Manager | No official Rust SDK; REST is Google's documented path. SChannel adds no second TLS stack or build tools. The page never holds the key or reaches Google (the CSP allows `ipc:` only). See `docs/studio.md`. |
| Claude Code (Studio) | `hub-mcp.exe` (`crates/hub-mcp`): a stdio MCP server on `rmcp` 3.9 (the official Rust SDK, schemas through `schemars`), calling Hub's local API over plain HTTP on 127.0.0.1 with Claude Code's own token | The MCP specification advises stdio for local servers and Claude Code starts and stops it; the API stays plain REST with every ULTRON defence. No rule lives in the bridge. 3.1 MB in release. Works under the CLI and the desktop app's Code tab (`docs/studio.md`, "Shape"). |

Rust: edition 2024, `rust-version` 1.90 (Tauri 2.12 needs a newer compiler than 1.85; the workspace
resolver picks crate versions that fit the declared version).

Rejected: a web app in the browser (VIO's route: cannot see a dropped file's path, no tray or
global hotkey); a Python sidecar (a second runtime to ship and start, for no gain); Electron
(ships its own Chromium, several times the memory).

## Process model

```text
Hub.exe (Tauri, Rust): starts with Windows into the tray
├── core          domain services: tasks, events, routines, reminders, projects, files,
│                 learning, journal, activity. The only code that touches SQLite.
├── scheduler     one thread: fires due reminders, nudges Tutup Hari once in the evening.
│                 Wakes at the next due time or after 30 s at most, so sleep, resume, and
│                 clock changes need no special handling. The daily backup is in maintenance.
├── files         file identity, move tracking, Vault copies, Windows Shell thumbnails.
├── commands      Tauri commands for the UI: validated input, typed output (ts-rs).
├── api           local HTTP API on 127.0.0.1 for ULTRON (crates/hub-api), the same core services.
├── studio        Hub Studio: Gemini over HTTPS (crates/hub-ai), the key in Credential Manager.
│                 The only outbound traffic Hub makes, and only when the owner acts in Studio.
├── tray, single instance, autostart, global hotkeys (quick capture, Lentera)
└── windows       main (created when opened), Catat Cepat (small, created on first use),
                  Lentera's capsule and bar (transparent, created on first use)
        │
        └── WebView2: React UI. Renders state, sends intents. Holds no truth.

ULTRON (Python, ../Ultron) ── httpx ──> Hub local API (CP5) ── core services
```

The core emits a `changed` event (entity type and ids) after every committed write, whoever made
it. The UI invalidates the matching queries, so a task ULTRON adds appears without a reload.

Closing the main window hides it to the tray (see "Resources" for what that costs, measured).

The window is built hidden and shown by the page itself (`window_ready`) once the first frame is
drawn in the right light with Hub's fonts, so it never flashes white at night; a 2.5 s fallback
shows it anyway if the page fails. The page also paints the Windows title bar in its own paper and
ink (`set_caption`, DWM caption colour, Windows 11). WebView2's profile lives in the data folder
(`webview\`), so `HUB_DATA_DIR` isolates it too.

## Repository layout

```text
Hub/
├── CONTEXT.md, README.md
├── docs/                    design.md, architecture.md, checkpoints.md
├── design/                  tokens.css (imported by the app), check-contrast.mjs, icon.svg, prototype/
├── crates/hub-core/         the domain core: no Tauri, no UI
├── crates/hub-api/          the local API for ULTRON (axum): guard, routes, token file, event stream
├── crates/hub-mcp/          Studio's bridge for Claude Code: hub (the API, the token file, the
│                            session, starting Hub), lib (the nine tools), place (the folder
│                            Claude Code works in, redirected writes), main (stdio)
├── crates/hub-ai/           Studio's Gemini: REST client, error kinds, Uji koneksi, the key store,
│                            prompts/studio.toml (the instructions), prompt (requests), stream (SSE
│                            and whole replies with function calls), calls (checked arguments),
│                            prd (schemas per part, generation with one repair, revision),
│                            roadmap (its schema, one request and one repair),
│                            bin/studio-eval.rs (the owner's check against the real API)
│   ├── migrations/          numbered SQL files, forward only
│   └── src/                 db, settings, activity, tasks, cards (task rows), projects, areas,
│                            events, routines, recurrence (RRULE subset), calendar (days, load),
│                            reminders, capture (the Catat Cepat reader), dayclose (Tutup Hari),
│                            journal, today, context, backup, order (sort keys), time, model,
│                            files, identity (file ids, move tracking), vault, learning, insights
│                            (Jejak), search (FTS5), studio (Studio projects, the conversation, the
│                            ledger), prd (the PRD: shape, rules, versions, Markdown), roadmap
│                            (checkpoints, rules, approval with the Proyek and folder, statuses,
│                            reports, requests, the implementation prompt), bin/hub-seed.rs
├── src-tauri/               the app: lib.rs (startup), commands.rs, window.rs, tray.rs,
│                            lentera.rs (Lentera's window: its place, click-through, keyboard),
│                            scheduler.rs, notify.rs (toasts), protocol.rs and registry.rs
│                            (hubapp:// links), maintenance.rs (daily backup, hourly file check),
│                            library.rs (drops and file commands), shell.rs (open, reveal, the
│                            file dialog, on a COM thread), thumbs.rs, belajar.rs (Belajar,
│                            Jejak, search commands), studio.rs (Gemini key, Uji koneksi, Studio),
│                            roadmap.rs (the roadmap, the folder dialog, checkpoints at work),
│                            claude_code.rs (Pasang, Lepas, Uji jembatan), skills/ (the
│                            skill /hub-connect Pasang writes),
│                            log.rs; capabilities/main.json, capture.json, and lentera.json
├── src/                     React UI
│   ├── app/                 App, router, Shell (sidebar, top line), CommandPalette (with search),
│   │                        DropPools, nav
│   ├── pages/               today/ (Hari Ini, Day Arc, Tutup Hari), tasks/, projects/,
│   │                        calendar/, files/ (Library), learning/ (Belajar), trail/ (Jejak),
│   │                        settings/ (with Integrasi AI), studio/ (Studio, loaded on first visit)
│   ├── components/          shared UI: TaskList (ink check, keyboard, drag), CaptureForm,
│   │                        Dialog, Fields, Toast, FileThumb, FileRows
│   ├── capture/             the Catat Cepat window (capture.html)
│   ├── lentera/             Lentera's capsule and bar (lentera.html): states, window, styles
│   ├── data/                backend interface, Tauri and mock backends, queries, generated/
│   ├── light/               phase rules and the light engine
│   ├── time/                the shared minute clock, Indonesian formats
│   ├── strings/id.ts        every UI string (English can be added beside it)
│   └── styles/global.css    base styles over design/tokens.css
└── scripts/                 smoke-native.mjs (real window over DevTools), measure.ps1
```

The webview may call exactly the commands listed in `src-tauri/build.rs` (190 since Lentera CP02),
allowed one by one per window. The main window (`capabilities/main.json`) gets all of them except
`hide_capture` and Lentera's own three (`set_lentera_shape`, `hide_lentera`,
`return_lentera_focus`, `take_lentera_open`, `focus_lentera`); it may `open_lentera` and run a Sesi Fokus (`get_focus`, `start_focus`, `pause_focus`, `resume_focus`, `next_focus`, `stop_focus`, `tuck_focus`). The Catat Cepat window (`capabilities/capture.json`)
gets only `get_settings`, `get_shell`, `window_ready`, `read_capture`, `save_capture`, and
`hide_capture`. Lentera's window (`capabilities/lentera.json`) gets `get_settings`, `get_shell`,
`window_ready`, `read_capture`, `save_capture`, its own five, and the Sesi Fokus commands. No plugin API (file system,
shell, autostart, window) is exposed to any of them; autostart is switched by `update_settings` in
Rust. Write commands return no new state to trust: the UI refetches after the `changed` event.

## Data

### Location

`%LOCALAPPDATA%\Hub\`: `hub.db` (plus WAL), `vault\`, `backups\`, `cache\thumbs\`. `HUB_DATA_DIR`
overrides it for tests and smokes. Note: inside the Claude sandbox `%LOCALAPPDATA%` is virtualized
(MSIX), so a Hub started from the sandbox writes somewhere the owner's Hub never looks. Smokes use
`HUB_DATA_DIR`.

### Rules

- **Migrations**: numbered SQL files applied in order, tracked with `PRAGMA user_version`, each in
  its own transaction, forward only, never edited after release. A database from a newer Hub is
  refused with a clear message (the pattern ULTRON's memory uses). Before migrating a database that
  has data, Hub copies it whole to `backups\hub-before-schema-N.db` (Studio, 0005, rebuilds
  `activity` to accept two new actors).
- **IDs**: UUIDv7 as text (time ordered, safe to import and merge).
- **Time**: instants as UTC ISO 8601 text; all-day things as local dates (`YYYY-MM-DD`); the owner's
  IANA time zone in settings. Routines store an RRULE plus a local start time.
- **Every write records itself**: a row in `activity` in the same transaction, with the actor
  (`owner`, `ultron`, `hub`, and for Studio `claude` and `gemini`). Jejak and insights are read
  from it. One exception: Studio's conversation is its own record (a message leaves no activity
  row, or a chat would bury Jejak).
- **Deletion is soft** (`deleted_at`), purged from Trash after 30 days. Files on disk are never
  deleted by Hub, except Vault copies when the owner removes them.
- **Order** within lists uses fractional order keys, so a drag rewrites one row.
- `STRICT` tables, foreign keys on, `CHECK` constraints on enums.

### Tables

| Table | Holds |
|---|---|
| `areas` | Ruang: name, pigment, order. |
| `projects` | name, description, area, pigment, status (`active`, `paused`, `done`, `archived`), start and target dates. |
| `milestones` | project, title, due date, done time, order. |
| `tasks` | title, notes, status (`todo`, `doing`, `done`, `cancelled`), priority 0 to 3, do date, deadline, estimate, project, area, milestone (one of its project's, migration 0009), parent (subtasks), order key, completed time, source. |
| `focus` | date, task, slot 1 to 3 (Fokus for a day). |
| `events` | title, start, end, all day, location, notes, project, pigment, linked task (a time block), routine occurrence override, source. |
| `routines` | title, kind (`lesson`, `habit`, `study`, `other`), RRULE, local start time, duration, pigment, active. |
| `routine_exceptions` | routine, date, skipped or moved to. |
| `reminders` | text, due time, optional repeat, state (`scheduled`, `fired`, `snoozed`, `done`, `cancelled`), optional task or event, fired time, source. |
| `files` | display name, kind, extension, size, original path, volume serial and 128-bit file id (identity), state (`ok`, `moved`, `missing`), Vault path, SHA-256 (Vault copies only), note, added and last seen times, source. |
| `file_links` | file, target type (`project`, `task`, `learning_item`, `event`, `journal`), target id. |
| `collections`, `file_collections` | Koleksi and membership. |
| `tracks` | Jalur: name, kind (`platform`, `ctf`, `lab`), URL. |
| `learning_items` | track, title, kind (`room`, `path`, `machine`, `challenge`, `practical`), domain, difficulty, status, URL, points, started and completed times, notes. CTF challenges also point at their event. |
| `ctf_events` | name, dates, team, rank, participants, URL. |
| `learning_sessions` | item, start, duration, note. |
| `learning_timer` | the one running timer (item, start), so it survives a restart. |
| `focus_run`, `focus_blocks` | Sesi Fokus (migration 0010, Lentera CP02): the one running session (task or item, phase, when the phase began as if never paused, its length, a pause, when Hub last looked, the lengths kept from its start, tucked) and every focus block kept (start, seconds, planned, finished or stopped, task, item, the learning session it became). |
| `journal` | date, text, kind (`note`, `done`, `reflection`), the task a note explains, source. Notes and done activities from ULTRON land here. |
| `tags`, `taggings` | free tags on tasks, files, learning items. |
| `activity` | time, actor, verb, entity type and id, a short text snapshot, JSON details. Append only. |
| `settings` | key, JSON value (wake and sleep time, phase hours, theme mode, ink, weekly learning goal). |
| `api_clients` | name, token hash (SHA-256), scopes, created and last used times. |
| `imports` | references of rows imported from ULTRON (`ultron:task:12`) and the row each became, so an import runs once. |
| `studio_*` | Hub Studio (migration 0005, `docs/studio.md` "Data"): projects, messages, ledger, PRD versions, roadmaps, checkpoints, their tasks, reports, requests. A saved PRD version and a report never change (triggers). Migration 0006 adds the ledger's `area` (the part of an idea an item settles, R2); 0007 adds `studio_stacks` (stage 2's tech stack, R3); 0008 adds `studio_claude_sessions` (Claude Code's sessions and last calls as the bridge reports them, R5; not exported, not in Jejak, kept 30 days and at most 100). |
| `search` | FTS5 index over titles and notes of tasks, projects, files, Koleksi, learning items, events, journal; kept in step by triggers. |

Progress is derived: a project's progress is done tasks over tasks (top-level, not cancelled), the
same way whether it has milestones or not. A milestone groups some of the project's tasks and is
reached once all of them are done; a milestone without tasks keeps the owner's own tick
(migration 0009, which also dropped the unused `progress_mode`). Insights are SQL over `activity`, `learning_sessions`, and `tasks`. There is
no hand-typed percentage anywhere.

### Backups

A daily `VACUUM INTO backups\hub-YYYY-MM-DD.db` while idle, keeping 7 daily and 4 weekly copies.
JSON export of everything (CP5). Restore is a documented manual step at first.

## Files

- **Reference by default.** Hub stores the path plus the file's identity on its volume (`identity.rs`:
  the 64-bit volume serial and 128-bit file id from `GetFileInformationByHandleEx(FileIdInfo)`).
  If the path stops working, Hub finds the drive with that serial, opens the file by its id
  (`OpenFileById`) and asks Windows for its current path (`GetFinalPathNameByHandleW`), so a file
  moved or renamed on the same NTFS drive is found again (`moved`, with the new path stored and an
  activity row by `hub`). Across drives that is impossible: the file is marked `missing`, and the
  owner points to it with the Windows file dialog ("Cari file"). A file added twice (same identity,
  or the same path) is the same row.
- **Drops never hand a path to the page.** Tauri's `DragDrop` window event gives Rust the paths;
  `library.rs` keeps them under a drop id and tells the page only where the pointer is and how many
  files (`hub://drag`). The page hit-tests its drop pools and answers `keep_drop(dropId, target)`.
  "Tambah file" (`IFileOpenDialog`, owned by Hub's window) is the keyboard and mouse alternative.
  A folder is kept as one reference.
- **Vault copy** on request, and automatically for files dropped from a removable drive
  (`GetDriveTypeW`): copied into `vault\<yyyy>\<uuid>-<name>` and hashed with SHA-256 on the way,
  so a missing original still opens its copy. Only plain path components are accepted when a Vault
  path is resolved.
- **Thumbnails** from the Windows Shell (`IShellItemImageFactory`, 176 px, as ULTRON's
  `previews.rs`), cached as PNGs in `cache\thumbs\<id>-<mtime>-<size>-<t|i>.png` (a changed file
  gets a new picture; `i` marks a plain icon when Windows has no thumbnail) and handed to the page as data URLs. Asked for only once a tile is near the screen.
- **Opening** goes through the Shell (`ShellExecuteW`) for files Hub knows only, by id, never a
  path from the page. Programs and scripts (`exe`, `bat`, `ps1`, `lnk`, and the like) are shown in
  Explorer instead of started. "Tampilkan di Explorer" selects the file
  (`SHOpenFolderAndSelectItems`). Shell calls run on one COM (STA) thread (`shell.rs`).
  Links of tracks and learning items open only when they are `http` or `https`, read from the
  database by id.
- Checks run lazily (when opening or showing a file fails) and in a slow background pass (the 100
  files seen longest ago, hourly), never a full disk scan.

## Belajar, Jejak, and search

- **Belajar**: tracks (the five defaults are created once, settings key `tracks_seeded`), items
  with kind, domain, difficulty, and status, CTF events with their challenges, and sessions. One
  timer at a time (`learning_timer`): starting another stops the first into its session; a stopped
  timer rounds up to whole minutes and is capped at 8 hours. The weekly goal counts active days (4
  of 7 by default). The overview gives this week, a 140-day heatmap, and minutes by domain.
- **Jejak** reads `activity` in pages (cursor `at|id`), filtered by actor and by group (tasks,
  projects, calendar, files, learning, notes). The week's measures (tasks done, learning minutes,
  active days, files added, projects moved) come with eight weeks of history and the average of the
  four weeks before this one.
- **Search** is one FTS5 table (`trigram` tokenizer, diacritics removed) filled by triggers, so it
  can never drift from the data. Queries of three or more characters use it (each word quoted);
  shorter ones fall back to `LIKE`. Title matches come first, then bm25. File contents are not
  indexed (owner decision), only names, notes, and Koleksi.

## Reminders

`scheduler.rs` runs one thread. Each look at the clock fires every reminder whose time has come
(`reminders::fire_due`, which marks one as late when it came due well before, for example while
Hub was not running or the laptop slept), then waits until the next due time, the Tutup Hari
nudge, or 30 seconds, whichever is first. A write that touches reminders or settings wakes it at
once. On fire:

- a Windows notification with Hub's own AppUserModelID (`dev.hub.desktop`, registered under HKCU
  on start), `scenario="reminder"` so it stays until handled, with the buttons **Tunda 10 m** and
  **Selesai**. The buttons are `hubapp://reminder/<id>/snooze|done` links: `protocol.rs` accepts
  only those and `hubapp://open/today|review`, with real ids; a second Hub started by the link
  hands it to the running one (single instance);
- a line above every page in Hub (`Alerts.tsx`) until it is snoozed or done, and the list in
  Kalender, Pengingat (Tugas, Pengingat before R6);
- a `reminder` event on the API's stream, so ULTRON, when listening, says it aloud (owner's
  choice: Hub's notification always shows, ULTRON adds no second one).

The Tutup Hari nudge is one notification at the chosen time (21.00 by default) when the day is not
closed, at most once a day (settings key `close_nudged_on`). With `HUB_DATA_DIR` set (tests,
smokes) Hub registers neither the notification identity nor the link handler, so a test run never
touches the owner's registry entries.

## Catat Cepat

Ctrl+Alt+H (global, `tauri-plugin-global-shortcut`) shows a small frameless window, always on top,
that hides when it loses focus and is let go after 10 minutes hidden like the main window. If the
hotkey is taken, Pengaturan says so. The same line is in the command palette and at the top of the
task lists. The core reads the line (`capture.rs`, Indonesian and basic English): dates (besok,
lusa, jumat, 25/10, next friday), times (19.00, jam 7 malam, 7pm), deadlines (tenggat, deadline,
sebelum), priority (!, !!, !!!), estimates, #project, and "di <tempat>" for events. The UI shows
the reading as chips while typing and never guesses on its own; Enter saves what the chips show. A
line that starts with "ingatkan" or "remind me" switches the kind to Pengingat.

## Lentera on the desktop (CP01)

Lentera's capsule and bar (design: `docs/design.md`, "Lentera on the desktop") live in a third
window, `lentera` (`src-tauri/src/lentera.rs`, page `lentera.html` and `src/lentera/`). It opens
from Ctrl+Alt+L (global, registered beside Ctrl+Alt+H; Pengaturan says if another application holds
it), the tray's "Tanya Lentera", and inside Hub from the sidebar, Ctrl+L, and the palette
(`open_lentera`). Pressed again while the bar has the keyboard, Ctrl+Alt+L closes it.

- **The window** is built like ULTRON's island: frameless, transparent, always on top, off the
  taskbar, no native shadow (an undecorated window with one gets a 1 px frame), fixed at 480 x 640
  logical px (400 in CP01; CP02's session card made the bar taller, 460; CP03's conversation and
  cards, 640, scrolling inside the bar beyond that). The capsule and the bar are drawn inside and change shape with one spring; the
  window never resizes, because Windows cannot resize a window in step with what its WebView draws
  (ULTRON measured the jump). Tauri issue #8308 (a transparent window drawn opaque) was a Tauri 2
  alpha bug, fixed long ago; ULTRON's island has used a transparent Tauri 2 window on this laptop
  since 2026-10, and the native smoke compares the real screen with Lentera shown and hidden to
  prove the area around the bar shows what is behind it.
- **Its place**: the window's bottom-right corner on the primary monitor's work area (above the
  taskbar, beside the clock, wherever the taskbar is), in that monitor's physical pixels, so the
  display scale is respected. It is placed every time it opens and again when its scale changes.
  ULTRON's island is top centre, so they never meet. `placement()` is a pure function with tests
  for 100, 125, and 150 %, taskbars on any side, a primary display left of another, and a work
  area smaller than the window.
- **Only the shape takes the mouse.** The page reports its shape (`set_lentera_shape`: the bar at
  once when it grows, the capsule once the spring has settled into it), checked and kept inside the
  window. While Lentera is shown, a thread reads the cursor every 25 ms and makes the window
  click-through (WS_EX_TRANSPARENT and WS_EX_LAYERED, as Tauri's `set_ignore_cursor_events` does, set
  directly since CP02) wherever the shape is not; it stops when the window
  hides. When the cursor cannot be read, the window takes clicks.
- **The keyboard.** Opening Lentera remembers the window that had the keyboard
  (`GetForegroundWindow`). Esc, the close button, and a save give it back (`SetForegroundWindow`,
  while Lentera still has the foreground, so Windows allows it); a click elsewhere does not, and
  Lentera forgets the window as soon as it loses the keyboard, so a later Esc never pulls the owner
  back somewhere old. The taskbar is never remembered (Lentera opened from the tray menu): Windows
  then picks the next window itself.
- **What the page decides**: the states (`src/lentera/state.ts`, one pure step function: hidden,
  capsule, bar) and when to hide (`hide_lentera`). Hidden, the window is let go after 10 minutes
  like the others; an unsaved line kept in it lives as long as the window does.
- Until Lentera can answer (CP03) the bar saves lines through the same `read_capture` and
  `save_capture` as Catat Cepat. No data leaves the computer in CP01. Since CP03 it does so only
  without a Gemini key, or when the owner hands a line Lentera could not answer to Catat Cepat.

### Sesi Fokus (CP02)

- **The rules** are hub-core's `focus.rs`: one session at a time, its phase read from stored times
  (no ticking), moved on by `settle`. A block ends in its break, a break ends in waiting, every
  few blocks the break is long. Time Hub did not see pauses a block where Hub last looked
  (`seen_at`, written at every look) and ends a break; a session paused or waiting for 30 minutes
  ends by itself. A finished block is kept in `focus_blocks` and Jejak; on a Belajar item it is
  that item's learning session instead (written once), and Belajar's timer and a session stop each
  other. The rules are tested with a made-up clock (`focus::tests`: breaks, the long break, pauses,
  a lid closed mid-block, a break slept through, the idle end, stopping early, Belajar).
- **The clock** is the scheduler (`scheduler.rs`): every look settles the session, and the next
  wake is the earliest of a reminder, the nudge, and the session's next phase end. A change is
  announced like any other, except that ULTRON's stream leaves it out (owner's decision).
- **The capsule follows the session** (`lentera::sync_session`, on every Sesi Fokus change): while
  a session runs and is not tucked away, Lentera's window shows without taking the keyboard
  (`window::show_quietly`: shown by Windows directly without activation and with WS_EX_NOACTIVATE,
  so a click on the capsule's button never takes the keyboard or closes the owner's open menu;
  Tauri cannot show a window without activating it). Tauri then still thinks the window hidden
  and would hide it again at any change of its own flags, so the click-through is set directly
  too (WS_EX_TRANSPARENT and WS_EX_LAYERED, re-checked every 25 ms), and opening the bar
  (`focus_lentera`) shows it through Tauri again. It is built quietly if it was let go (the
  page then shows the capsule instead of opening the bar: `take_lentera_open`). Once a second a
  presence check hides it while a fullscreen application is in front of the primary screen
  (Windows reports busy, Direct3D fullscreen, or presentation mode, and the window in front covers
  the whole screen without being maximised: ULTRON's rule) and shows it again after. The page
  hides the window itself when it has nothing left to show.
- **Lengths** come from Pengaturan; `HUB_FOCUS_SECONDS_PER_MINUTE` (honoured only with
  `HUB_DATA_DIR`) makes a minute a second for the native smoke.

### Lentera's brain (CP03)

The bar's line goes to Gemini through the same client, key, model, and Interactions API as
Studio (`store: false`, streamed, the temperature left at Google's default as Google asks for
Gemini 3, thinking `low` so the bar answers in seconds). Decisions in `CONTEXT.md`, "Lentera on
the desktop".

- **Four functions, all proposals** (`crates/hub-ai/src/lentera.rs`, words in
  `prompts/lentera.toml`): `usul_tugas`, `usul_pengingat`, `usul_jadwal`, `usul_catatan`, each with
  a list of items. Google advises 10 to 20 tools at most and checking every call before acting; Hub
  declares four and acts on none. Every item is read into a typed `Proposal` (dates
  `YYYY-MM-DD` that exist and are not years away, times `HH:MM`, lengths, a time on a task as its
  deadline as in Catat Cepat, a reminder only ahead of now, project and Ruang names matched to the
  ones sent, unknown names dropped with a note). What fails is refused item by item with a reason;
  the reply goes on to a second round only then, so Gemini can mend it (one request when all pass:
  the free quota counts requests). At most 12 cards a reply, 10 a call; three rounds.
- **What is sent** (`DeskContext`): the system instruction with "Saat ini" (today, the time, the
  next 14 days by name, so "Jumat" needs no arithmetic) and the names of the active projects and
  the Ruang, fenced as data; the last six turns of the bar's conversation (kept by the page in
  memory, each clipped to 2000 characters; a turn from Lentera is its text and a line per proposal);
  the owner's line (1000 characters at most) or the cleaned message. Ids stay home. The
  `ask_lentera` command reads the key, builds this, streams the answer to the page over a Tauri
  channel (`Started` with what was sent, `Delta`, `Proposals`, `Finished`), and keeps one answer at
  a time (`stop_lentera` is Hentikan). Logs carry the failure's kind, never the text.
- **Dari Pesan** (`crates/hub-ai/src/group_message.rs`): WhatsApp's export on Android and iPhone,
  copied messages from the desktop and web, and twelve-hour clocks are read; senders' names, times,
  phone numbers (nine digits or more starting with `+`, `0`, or `62`), mentions, and emails go; the
  date sent stays as "(dikirim Selasa, 2026-11-10)"; media and deleted messages are dropped.
  Cleaning is pure and idempotent: the page shows the cleaned text (`clean_lentera_message`), and
  `ask_lentera` cleans what it is handed again before sending, so nothing leaves that the owner did
  not see. 8000 characters at most after cleaning.
- **Simpan** (`save_lentera_proposal`): the proposal as the page hands it back (maybe changed on
  its card) is checked again against the current projects and Ruang, then written through
  hub-core (`tasks::create`, `reminders::create_at`, `events::create`, `journal::create`) as the
  owner's, in one write each, announced like any change. Simpan semua is the same, card by card.
- **The page** (`src/lentera/talk.ts`, one pure step function for the conversation and its cards;
  `Conversation.tsx` draws it; `LenteraWindow.tsx` calls the core): the conversation lives in the
  window's memory only and is dropped when the bar opens again with nothing waiting. A click
  elsewhere keeps an answer coming or cards waiting in the capsule, as it keeps a draft.
- **Tests**: hub-ai checks the requests, the reading of every function, the mend round against a
  fake Gemini, and the cleaning of each format; src-tauri writes every kind into a test database
  and checks Jejak; Vitest drives the bar against the demo backend's pretend Gemini
  (`src/data/mockLentera.ts`); the native smoke drives the release build against a fake Gemini.

## ULTRON integration (CP5)

### Local API

`crates/hub-api` (axum on the tokio runtime Tauri already runs; no Tauri inside, so it is tested
on its own), started by `src-tauri/src/api.rs` when Pengaturan has it on. HTTP and JSON on
`127.0.0.1` only, port 47731 unless the owner picks another (`HUB_API_PORT` overrides it for
smokes). Defences, layered, because loopback alone is not a boundary: a web page in the owner's
browser can send requests to loopback, and through DNS rebinding can even read the answers. Real
local-API vulnerabilities came from exactly this mix (a token skipped for loopback clients, no
`Host` check, permissive CORS). In the order `guard.rs` runs them, before any database read:

1. A peer that is not loopback is refused (the listener binds 127.0.0.1; this holds even if that
   ever changed).
2. A request a browser made is refused: any `Origin` or `Sec-Fetch-Site` header. There is no CORS,
   so a preflight gets a refusal and no `Access-Control-*` header.
3. `Host` must be exactly `127.0.0.1:<port>` or `localhost:<port>`, once; an absolute request
   target must agree (stops DNS rebinding).
4. A bearer token is **always** required, even from loopback: 32 random bytes as 64 hex
   characters. Hub writes it with the address to `api.json` in the data folder
   (`%LOCALAPPDATA%\Hub`, which only the owner's account can read) and keeps only its SHA-256 in
   `api_clients`. ULTRON reads the file before connecting and again after a 401, so a rotated
   token or a new port needs nothing on ULTRON's side. "Buat token baru" in Pengaturan replaces it
   at once and ends the event streams opened with the old one.
5. A rate limit per client (bursts of 60, 20 a second).
6. Scopes per route: `read`, `write:tasks`, `write:reminders`, `write:events`, `write:files`,
   `write:journal` for ULTRON (`write:events` has no route yet); `studio:read` and
   `studio:report` for Claude Code (Studio S5). Hub sets up two clients, each with its own token,
   file (`api.json`, `claude-code.json`), and scopes: neither has any of the other's, so each
   reaches only its own routes. Each is switched on or off in Pengaturan (`api_enabled`,
   `claude_enabled`); the server runs while either is on, and the guard refuses a client that is
   off (403 `client`).
7. Bodies at most 64 KB, JSON only, unknown fields refused; values are checked by the core like
   the UI's. Paths for `POST /v1/files` must be local and absolute: network shares and device paths
   are refused, since looking at one would make Windows reach out (and offer the owner's
   credentials) on ULTRON's say-so.

Every API write is made by the actor `ultron`: Jejak and the UI show ULTRON's crimson mark. A write
with an `importRef` ("ultron:task:12") is recorded with the source `import` and made at most once
(`imports` table), so ULTRON's one-time migration can run again safely. Removing a task or a note
over the API is soft, like every removal in Hub (CP1's draft had no delete at all; ULTRON's "delete
the networking task" needed one, and Hub can restore it).

Errors have one shape: `{"error": {"kind", "field", "reason", "message"}}` with 401 (token), 403
(`peer`, `origin`, `host`, `scope`), 404, 413, 415, 422 (invalid input: field and reason code, as
the UI's errors), 429 (`Retry-After`).

A named pipe was considered: it gives Windows' own caller identity and no port, but adds work on
both sides and the token file already limits access to the owner's account. HTTP keeps ULTRON's
`httpx` and is easy to test with `curl`.

### Endpoints

| | |
|---|---|
| `GET /v1/health` | name, version, API and schema version, today, time zone, the client |
| `GET /v1/today` | Hari Ini: schedule, reminders, Fokus, other tasks for today, alerts, learning |
| `GET /v1/schedule?from=&to=` | days (at most 31) with events, routine occurrences, time blocks, reminders |
| `GET /v1/tasks?view=`, `POST /v1/tasks` | lists (`inbox`, `today`, `upcoming`, `anytime`, `done`); a new task |
| `GET, PATCH, DELETE /v1/tasks/{id}`, `POST .../complete`, `.../reopen` | one task; a partial edit (`null` clears) |
| `GET, POST /v1/reminders`, `GET, PATCH /v1/reminders/{id}` | reminders (instants with offset, RFC 3339) |
| `POST /v1/reminders/{id}/snooze` (`minutes`), `.../done`, `.../cancel` | acting on one |
| `GET /v1/projects`, `GET /v1/projects/{id}` | projects with progress, tasks, and files |
| `GET /v1/files?kind=&id=` or `?projectId=`, `GET /v1/files/{id}` | files attached to something; one file, looked at on disk first |
| `GET /v1/files/search?q=` | files named for the question first, then files attached to matching projects, tasks, learning items, events, notes, Koleksi (`via` says which) |
| `POST /v1/files`, `POST /v1/files/{id}/attach` | keep a path (attach it, give it a note); attach a kept file |
| `GET /v1/journal?date=` / `?q=` / `?taskId=` / `?kind=`, `POST /v1/journal` | notes and done activities, with their files; a note may explain a task |
| `GET, PATCH, DELETE /v1/journal/{id}` | one note; point it at a task |
| `GET /v1/learning/summary` | this week against the goal, the item in progress, the timer |
| `GET /v1/search?q=` | Hub's search across everything |
| `GET /v1/events` | server-sent events: `hello`, then `changed` (entity, ids) and `reminder` (id, text, due, late); `resync` when the reader fell behind; keep-alive every 15 s |

### Studio routes for Claude Code (S5)

`crates/hub-api/src/studio.rs`, rules in `hub-core::claude` (docs/studio.md, "Access"). A
connection is a session: `POST /v1/studio/sessions` (`projectId`, `folder`) checks that the owner
gave the project to Claude (access, confirmed folder, approved roadmap) and that `folder` is its
repository folder or inside it, and answers with a 128-bit handle and the project's frame. Every
later call sends it in the `Hub-Session` header; it is bound to the client, the token it was made
with (a new token ends it), and the project, kept in memory, and ends after 12 hours unused (the
bridge connects again, also after Hub restarts). The project's access is checked again on every
call, so taking it away stops Claude at once. 403 `session` means "connect again", 403 `access`
means the owner allowed reading only.

| | |
|---|---|
| `GET /v1/studio/status` | Hub, version, the client, how many projects it may see |
| `GET /v1/studio/projects` | the projects given to Claude: id, name, stage, folder, access, PRD version, active checkpoint |
| `POST, DELETE /v1/studio/sessions` | connect (session and frame); end the session |
| `GET /v1/studio/frame` | the project, its checkpoints in one line each, the owner's open notes |
| `GET /v1/studio/prd?sections=&feature=` | the approved PRD: outline, named sections, or one feature |
| `GET /v1/studio/checkpoints/{code}` | one checkpoint with its tasks, reports, and requests |
| `POST /v1/studio/checkpoints/{code}/start` | Ready (or Blocked, once answered) to In Progress |
| `POST /v1/studio/tasks/{code}` | a task's status and note |
| `POST /v1/studio/questions` | a question with options, or a blocker (`blocking`) |
| `POST /v1/studio/checkpoints/{code}/report` | the report: 201, or 422 with every problem (`at`, `message`) and nothing kept |
| `POST, DELETE /v1/studio/bridge` | R5: the bridge's hello (`folder`, `client`, `clientVersion`, `bridgeVersion`; 201 with `id` and `beatSeconds`) and its goodbye |
| `POST /v1/studio/bridge/beat` | R5: still there, with the tool call just made if any (`call`: `tool`, `ok`, `reason`, `projectId`); 404 means "say hello again" |

The bridge's own routes name its session in the `Hub-Bridge` header, need only `studio:read`, and
change no project data: they feed what Pantau and Pengaturan say about Claude Code (hub-core
`claude_sessions`). Their changes reach Hub's windows but not ULTRON's event stream.

Bodies up to 80 KB here (a report is at most 64 KB). There is no route to approve, edit the PRD
or the roadmap, or reach another project: those are the owner's, in Hub.

### On ULTRON's side

ULTRON's `docs/hub.md` has the detail; in short:

- `backend/ultron/hub/`: a client that reads `api.json`, sends every request with the token to
  127.0.0.1 (proxies ignored), reads the file again after a 401, and starts Hub into the tray
  (`hub.exe --tray`, found through Windows' autostart entry) when nothing answers: at ULTRON's
  start and on the owner's requests, never for a snapshot nobody asked for. A second start with
  `--tray` shows no window (`lib.rs`). One thread follows `/v1/events`.
- ULTRON's tasks, reminders, notes, and kept files are Hub's: its `TaskBook`, `ReminderBook`, and
  `Notebook` keep their methods and call Hub, still behind GUARD and the plain rules for clear
  commands. Ids are Hub's UUIDs, also on the island. Timers and the Pomodoro stay in ULTRON.
- Owner's choices (CONTEXT.md, CP5): Hub shows a fired reminder's notification and ULTRON says it
  aloud with its island alert, no second notification; a plain day said with a task is its do
  date, "due ..." or a time of day its deadline.
- New voice queries: "what's my schedule tomorrow?" / "apa jadwalku besok?" (`/v1/schedule`), and
  "where's the file for my networking project?" also asks `/v1/files/search`.
- `scripts/migrate_to_hub.py`: a dry run, then `--apply`, with `importRef`s so it runs once.
  ULTRON's old tables stay and are no longer read.
- Checked by `scripts/smoke_hub.py` against a real test `hub.exe` (its own `HUB_DATA_DIR` and
  `HUB_API_PORT`), and by ULTRON's unit tests against a Hub in memory with the same routes.

## Security

- Tauri capabilities: only what each window uses. No generic filesystem or shell plugin in the UI;
  file actions are Hub's own commands, which only accept ids of files Hub knows.
- Strict CSP, no remote content, no `eval`. User text is rendered as text, never as HTML.
- Input validated in Rust (lengths, enums, dates) before it reaches SQL; all SQL parameterised.
- No secrets in the database: the Gemini key is in Windows Credential Manager (`Hub/gemini`; tests and
  smokes use `Hub-test/<data folder>/gemini`), never in SQLite, the export, backups, the log, or the
  page. Nothing leaves the machine except Studio's conversation and documents, sent to Google's
  Gemini API only when the owner sends a message or presses a generate button, and what the owner
  sends in Lentera's bar (CP03: the line or the cleaned group message, the last few turns, the date
  and time, the names of projects and Ruang) when they press Enter or Kirim (owner's decisions,
  CONTEXT.md). Text from Gemini is rendered as text; its function calls only propose, and only the
  owner's Simpan writes. A fake Gemini (`HUB_GEMINI_BASE_URL`) is honoured only with `HUB_DATA_DIR` and only on
  loopback.

## Resources

Measured on the owner's laptop with the release build and the demo week (`scripts/measure.ps1`):
CP2 on 2026-10-22, CP3 and CP4 on 2026-10-23, CP5 on 2026-10-24 (two runs; the API on a test port), S6 on 2026-11-08 (two runs), R7 on 2026-11-09 (two runs, and one sampled after the window had been open a minute), P2 on 2026-11-09 (sound and motion; three runs, the first just after a build, and a probe per process after 90 s). Memory is the private working set (Task Manager's Memory
column) of hub.exe plus its WebView2 processes.

| | Target | CP2 | CP3 | CP4 | CP5 | S6 | R7 | P2 |
|---|---|---|---|---|---|---|---|---|
| Start to visible window | under 1.5 s | 0.8 to 1.0 s | 0.55 s | 0.62 s | 0.57 to 0.62 s | 0.61 to 0.77 s | 0.53 to 0.58 s | 0.53 to 0.65 s |
| Window open, idle | under 180 MB; CPU about 0 % | 107 to 130 MB; renderer and GPU process 0 %, WebView2's browser process in short bursts of 0.3 to 2 % (one run caught a 13 % burst in its first seconds) | 106 MB and 0.5 % CPU; the first run after a build read 147 MB and 12 % (WebView2 filling its caches). Per process over 20 s: hub.exe 0 %, renderer 0 to 0.1 %, WebView2's browser process 0.2 to 1 % | 117 to 123 MB; 1.3 % in the measure (its sample starts soon after the start). Per process from the first minute on, over 30 s: hub.exe, renderer, and GPU process 0 %, WebView2's browser process 0.16 % | 114 to 118 MB; 0.4 to 1.2 % in the measure (the API server idles on a socket: no thread wakes) | 121 to 122 MB; 0.9 to 1.1 % in the measure | 116 to 126 MB; 2.5 to 4.6 % in the measure (its 20 s sample starts 8 s after the window shows); 0.47 % over 30 s after a minute open | 116 to 128 MB once WebView2's caches are warm (150 MB in the first run after a build, as CP3 saw); 0.9 to 2.2 % in the measure; after 90 s about 0.5 % (WebView2's browser process 0.3 %, GPU process 0.2 %, renderer 0 %). At rest the page does no work: no running animation, and no layout, style, or script in 20 s |
| Closed, first 10 minutes | | unchanged (about 127 MB): hiding alone gives nothing back | about 110 MB, about 3 % CPU in the 10 s after hiding | about 120 MB | about 119 MB | 120 to 127 MB | 115 to 120 MB | 115 to 149 MB; one 10 s sample just after hiding read 9 %, the others 0.16 to 0.31 % |
| Closed for 10 minutes: window let go | under 40 MB | 4.5 MB, 0 % CPU; opening again takes about 0.45 s | 5 MB, 0 % CPU (the scheduler thread wakes at most every 30 s) | 6 MB, 0 to 0.2 % CPU | 6.4 to 6.6 MB, 0 % CPU (the API keeps listening) | 6.9 to 7.2 MB, 0 % CPU | 5.6 to 5.7 MB, 0 % CPU | 5.6 MB, 0 % CPU |
| Started in the tray (autostart) | under 40 MB | 1.8 MB, 0 % CPU | 2 MB, 0 % CPU | 2 MB, 0 % CPU | 2.7 MB, 0 % CPU (with the API listening: 0.7 MB more) | 3 MB, 0 % CPU | 3 MB, 0 % CPU | 3.1 MB, 0 % CPU |
| JavaScript bundle | under 400 KB gzipped | 120 KB gzipped (376 KB raw), CSS 10 KB gzipped, fonts about 0.8 MB on disk | main window 152 KB gzipped (488 KB raw: 55 KB its own, 97 KB shared with Catat Cepat), CSS 17 KB gzipped; Catat Cepat adds 1 KB | main window 170 KB gzipped (566 KB raw: 67 KB its own, 103 KB shared with Catat Cepat), CSS 20 KB gzipped; Catat Cepat adds 1 KB | main window 164 KB gzipped (70 KB its own, 94 KB shared with Catat Cepat), CSS 20 KB gzipped; the demo backend (14 KB) is loaded only in the browser | main window 180 KB gzipped (74 KB its own, 105 KB shared with Catat Cepat), CSS 21 KB gzipped; Studio a lazy route: 21 KB gzipped JS and 4 KB CSS for a project, 1.5 KB for the list | main window 181 KB gzipped (54 KB its own, 127 KB in shared chunks), CSS 21 KB gzipped; Studio a lazy route: 27 KB gzipped JS and 6 KB CSS for a project, 1.5 KB for the list | main window 188 KB gzipped (57 KB its own with the sound and motion code, 131 KB in shared chunks), CSS 22 KB gzipped; Studio a lazy route: 27 KB gzipped JS and 6 KB CSS |
| hub.exe | | 9.3 MB (UI embedded) | 10.9 MB | 11.7 MB | 13.8 MB (the API: axum and hyper); installer 3.9 MB | 18.8 MB (15.4 MB after S1: the HTTPS client for Gemini; 17.3 MB after S4; the Studio routes and Claude Code since); hub-mcp.exe 3.1 MB; installer 5.7 MB (both programs) | 19.3 MB; hub-mcp.exe 3.2 MB; installer 5.8 MB | 19.3 MB; hub-mcp.exe 3.2 MB; installer 5.9 MB |
| GPU | none at rest; no blur or WebGL | no animation at rest | no animation at rest; the Tutup Hari ink and moon play once | no animation at rest; thumbnails are still images | unchanged | unchanged; Studio streams text, no animation at rest | unchanged | none at rest: the gliding pill, sliding rows, and leaving dialogs run only after an action, 160 to 240 ms, then rest; the audio context is suspended 1.5 s after the last sound, so no audio thread runs at rest |
| Disk | | the Rust `target` folder takes a few GB during development | same | same, plus `cache\thumbs` (one small PNG per file shown) | plus `api.json` and `claude-code.json`; an export is one JSON file where the owner saves it | plus `claude-code.json`; Studio's tables in hub.db | plus `studio_claude_sessions` (at most 100 rows, 30 days) | unchanged: the sounds are synthesised, no files |
| Studio open, a whole project (native smoke, debug port on) | under 180 MB | | | | | 125 MB beside 112 MB for Hari Ini in the same fresh window (Studio adds about 13 MB); hidden on Studio 122 MB, let go like any page after 10 minutes | 127 MB beside 111 MB for Hari Ini; hidden on Studio 125 MB (one R6 run read 158 MB beside 133 MB for Hari Ini in the same window, both raised alike; the runs before and after it read 127 to 128 MB) | 141 MB beside 122.5 MB for Hari Ini (the first run after a build: both higher, the difference as before) |

The demo backend (`src/data/mock.ts`, for the browser and tests) is loaded with a dynamic import
only when Hub runs in a browser (CP5), so the release window never carries it.

"Commit" (private bytes) reads about 310 MB with the window open, almost all of it in WebView2's
GPU process: driver reservations, not RAM in use, which is why the private working set is the
number that matters. Hence the window lifecycle in `window.rs`: hidden on close for a quick return,
destroyed after 10 minutes hidden, rebuilt in under half a second when opened again.

## Testing

- Rust: unit tests per domain service and for migrations (a fresh database and an upgrade from
  each released version); reminders, routines, the calendar, the capture reader, and Tutup Hari at
  chosen instants (the core takes "now" as an argument, so no test waits on the wall clock); file
  identity on the real NTFS drive (a temp file renamed, then found by its id); the library through
  a fake `Finder`. The scheduler thread itself is checked in the native smoke.
- UI: Vitest and Testing Library for components and the quick add parser's rendering; a browser
  mode with a mocked command layer for visual checks in the Browser pane.
- Native smoke in the real window through WebView2's remote debugging port (as in ULTRON).
- API: `crates/hub-api/tests/api.rs` drives the real router for every defence above (no token, a
  wrong token, `Origin` and `Sec-Fetch-Site`, wrong and doubled `Host`, an absolute target, another
  computer, each missing scope, oversized and unknown-field bodies, the rate limit, a rotated token)
  and for what ULTRON does; the event stream is read over a real socket, through a server stop and
  a token rotation.
