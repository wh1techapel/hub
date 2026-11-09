# Design direction: Daylight Studio

Hub is a desk the owner returns to several times a day. The idea is a quiet studio whose light
follows the real day: warm paper in the morning, clear paper at midday, a dusk tint in the
afternoon, ink-dark at night. Content is set like good print (a serif voice for the moments that
matter, a precise sans for work, a mono for technical things), and colour is used the way ink is
used on paper: rarely, and on purpose.

The tokens are in [`design/tokens.css`](../design/tokens.css). The prototype in
[`design/prototype/`](../design/prototype/index.html) shows all of this working.

## Principles

1. **Paper first.** Structure comes from spacing, type, and hairlines, not from boxes. One raised
   surface per region at most. No card grid.
2. **Time is the material.** The light, the greeting, the dashboard's emphasis, and the Day Arc all
   come from the clock and the owner's schedule, so the app feels alive without being busy.
3. **One accent, the owner's.** The accent ink is chosen by the owner. Everything else is neutral.
   ULTRON's crimson is reserved for ULTRON.
4. **Motion explains, then rests.** Every animation shows where something came from or went, or
   confirms a done thing. At rest nothing moves, and nothing costs CPU.
5. **Kind, never guilty.** Missed days, overdue tasks, and slow weeks are stated plainly and paired
   with a next step. No red walls, no broken streaks, no shaming numbers.
6. **Quick in, calm inside.** Capture takes one hotkey and one line. Reviewing takes two minutes.

## References: what we take, what we leave

| Reference | Take | Leave |
|---|---|---|
| [Things 3](https://www.macstories.net/reviews/things-3-beauty-and-delight-in-a-task-manager/) | White space with bold type and small splashes of colour; the satisfying check; areas that hold projects that hold tasks; events shown as quiet reference text beside tasks. | Apple-only conventions. |
| [Linear](https://linear.app/now/how-we-redesigned-the-linear-ui) | Themes generated in a perceptual colour space from a base, an accent, and a contrast (we use OKLCH); a dimmed navigation so content leads; keyboard-first; the command palette. | Density meant for issue trackers; grey-on-grey low contrast text. |
| [Amie](https://thereport.substack.com/p/a-joyful-calendar) | Joy in small details; to-dos dragged from a side column onto the week to become time blocks. | Social features. |
| [Structured](https://daveswift.com/structured/) | The day as one visual strip where length is duration and gaps are obvious; a satisfying finish animation. | Its limited scope (tasks only in a timeline). |
| [Sunsama](https://help.sunsama.com/docs/daily-shutdown) | The shutdown ritual: review what got done, decide what slipped, a short reflection, ready for tomorrow; warning about an over-full day. | A long guided morning ritual (the owner does not plan in the morning). |
| [Notion Calendar](https://blakecrosley.com/guides/design/notion-calendar) | Calendar operable without the mouse (today, new, arrows), drag to create on a 15-minute grid. | Account-based integrations. |
| Forgiving habit trackers ([overview](https://habitdoom.com/blog/streak-anxiety-habit-trackers), Kintsu) | Weekly goals instead of daily chains, rest days that do not count against you, rewarding the comeback. | Streak resets and loss-aversion pressure. |
| VIO-DASHBOARD (owner's earlier project) | Do date separate from deadline; at most three focus tasks; deterministic summaries. | Graphite-and-lichen look; a web app (it cannot see real file paths on drop). |
| ULTRON | Its crimson as ULTRON's mark; its spring rhythm (`MORPH` 200/24) so both apps move alike. | Its obsidian dark look: Hub is a different place. |

## Visual identity

### Light (Cahaya)

| Phase | Default hours | Paper | Character |
|---|---|---|---|
| `pagi` | 04:30 to 10:00 | warm cream | soft, slightly peach |
| `siang` | 10:00 to 15:00 | clear warm white | crisp, highest contrast, the main working light |
| `senja` | 15:00 to 18:30 | apricot tint | warmer, a little dimmer |
| `malam` | 18:30 to 04:30 | blue-black ink | warm off-white text, the evening review light |

The phase changes with a slow cross-fade (2.4 s) of registered properties on the root, never a
jump. Everything that differs between light and dark is registered (paper, ink, accent, status
colours, pigment lightness and chroma) or derived from a registered value; anything left out would
jump while the paper fades. While the light shifts, the root carries `data-shifting` and element
colour transitions are switched off, or each element would start a new transition every frame. The hours follow the owner's wake and sleep time from Settings. Theme modes:
**Otomatis** (by phase, the default), **Ikuti Windows**, **Terang** (always `siang`), **Gelap**
(always `malam`).

**The glow (L3, Studio's home only).** Each phase has three lights (`--glow-a` to `--glow-c`:
apricot, rose, and a pale sky at pagi; sky, mint, and sunlight at siang; orange, rose, and violet
at senja; indigo, violet, and deep blue at malam), and `--glow-accent` adds the owner's ink. They
rise from under Studio's input and drift slowly (11 s and 14 s), still under reduced motion. They
are registered, so they cross-fade with the phase. Text over them is ink or ink-2, never ink-3;
`design/check-contrast.mjs` checks it with all four lights at their brightest on one spot.

### Ink and paper

- Text is ink: a deep blue-black on paper, a warm off-white at night. Three levels only (`--ink`,
  `--ink-2`, `--ink-3`), all at least 4.5:1 on every paper they sit on (checked by script).
- Hairlines are ink at 10 to 20 % alpha, so they follow the phase by themselves.
- Paper has a very light grain (a static SVG noise tile at 3 % opacity). It is a background image,
  so it costs nothing after the first paint.

### Accent ink (Tinta)

The owner picks one. Each has a light and a dark value, tuned to the same perceived weight.

| Tinta | Light | Dark |
|---|---|---|
| Biru Tinta (default) | `oklch(0.50 0.16 262)` | `oklch(0.76 0.12 258)` |
| Hijau Botol | `oklch(0.49 0.10 165)` | `oklch(0.78 0.11 165)` |
| Sepia | `oklch(0.50 0.11 58)` | `oklch(0.80 0.10 70)` |
| Ungu Senja | `oklch(0.50 0.15 300)` | `oklch(0.77 0.12 300)` |
| Grafit | `oklch(0.32 0.02 265)` | `oklch(0.90 0.01 265)` |

The accent marks the primary action, the selected item, the current time on the Day Arc, focus
tasks, and completion. Nothing else.

### Pigments

Eight muted watercolour hues for projects, routines, and calendar blocks: Lazuardi (250),
Toska (195), Lumut (140), Kunyit (85), Terakota (45), Mawar (10), Lavender (300), Batu (265, low
chroma). Each pigment gives a fill, a line, and a text colour per scheme from one hue number.
Colour is never the only signal: blocks always carry their name.

### ULTRON's crimson

`#b0213a` on light paper, `#e5667a` on dark (ULTRON's own `crimson-500` and `crimson-300`). Used
only for: the small ULTRON mark on things ULTRON created or changed, the connection indicator, and
the brief crimson thread that traces an item when ULTRON changes it while the owner is looking.
Warnings use amber, destructive actions use brick (hue 40), so nothing else reads as ULTRON.

### Typography

| Role | Face | Use |
|---|---|---|
| Display | **Fraunces** (variable: optical size, weight, softness) | Greeting, page titles, big numbers, the evening close. Set soft (`SOFT` 100) and large optical size. |
| UI | **Instrument Sans** (variable) | Everything you read and click. 14 px base for the UI, 15 px for longer text. |
| Mono | **JetBrains Mono** (variable) | Technical content: commands, IPs, room and machine names, durations, file sizes. |

All three are self-hosted with `@fontsource-variable/*` (no network at run time). Times and counts
use tabular figures. Scale: 44 / 32 / 24 (display), 17 (title), 15 / 14 / 13 / 12 (text),
12.5 (mono). Nothing under 12 px.

### Space, shape, depth

- Spacing on a 4 px base: 4, 8, 12, 16, 20, 24, 32, 40, 56, 72.
- Radius: 4 (small chips), 6 (controls), 10 (panels), 16 (sheets, palette), full (pills).
- Depth: paper (0), hairline region (1), lifted popover or palette (2, one soft shadow), dragged
  item (3). No glow, no glass, no backdrop blur (blur costs GPU that ULTRON's models need).

### Icons

Lucide, 1.5 px stroke, 16 or 20 px, always with a text label except in the collapsed rail (then
with a tooltip and an accessible name). File kinds use small shaped glyphs in a pigment, not
coloured blobs. No emoji as icons.

## Signature elements

1. **Busur Hari (Day Arc).** At the top of Hari Ini, a wide shallow arc from wake time (left
   horizon) to sleep time (right horizon). Today's events and routine blocks sit on it as pigment
   segments; the passed part is inked, the rest is a faint line; the sun (accent) marks now and
   becomes a moon after sleep time. Under it, one line: time left in the day and what is next.
   It updates once a minute and never animates at rest.
2. **Ink check.** Completing a task: the ring fills with accent like a drop of ink spreading, the
   tick draws itself, the title gets a light hand-drawn strike, and after a short pause the row
   folds away. Undo stays available for a few seconds.
3. **Tutup Hari close.** At the end of the evening review the Day Arc fills with ink from left to
   right and the moon rises: the day is closed. Once per day, so it stays special.
4. **Drop pools.** Dragging files from Explorer anywhere over Hub dims the page and shows drop
   targets as soft pools (Library, the open project, the selected task). The pool under the
   pointer swells; on drop, each file chip travels to where it went.
5. **ULTRON thread.** When ULTRON changes something on screen, a thin crimson line traces the item
   once (600 ms) and leaves the small ULTRON mark behind.

## Information architecture

Sidebar (232 px, collapsible to a 64 px rail; dimmed so the page leads). Since R6 (2026-11-09,
owner's decision in `CONTEXT.md`, "Navigation") in two groups:

1. **Hari Ini**: the dashboard, which is also the list of today's tasks.
2. **Kalender**: day and week views with time blocking, month overview, routines, and
   **Pengingat** as a fourth view (`/kalender?view=pengingat`, key P).
3. **Proyek & Tugas** (`/proyek`): one place. It opens on the projects by Ruang (each with
   progress and next step; a project's own page has its milestones, files, and history), and holds
   the task lists beside them: Kotak Masuk, Mendatang, Semua, Selesai. A task opens in a panel
   beside its list.
4. **Studio**: ideas built with Gemini and Claude Code.
5. **Belajar**: TryHackMe, Hack The Box, CTF, Lab Sekolah, Homelab (its own data: timer, sessions,
   weekly goal).

Under a quiet line, the secondary pages: **File** (the Library) and **Jejak** (history and
insights). At the bottom: quick capture, Settings, and ULTRON's connection mark (CP5).

**Pengaturan in parts** (P1, 2026-11-09, the owner's choice): five parts, each its own page and
address, `/pengaturan/<part>`:

| Part | Holds |
|---|---|
| Tampilan (`tampilan`) | Cahaya & tinta: the light (Otomatis, Ikuti Windows, Terang, Gelap), the ink; Suara (P2): the switch, Hub's volume, a sample of each place |
| Hari & kebiasaan (`hari`) | the name in the greeting, wake and sleep time, the Tutup Hari nudge, the Belajar goal |
| Integrasi AI (`ai`) | Google Gemini, the developer profile, Claude Code (for the CLI and the desktop app's Code tab alike) |
| ULTRON (`ultron`) | the local API: switch, port, state, new token |
| Sistem & data (`sistem`) | starting with Windows, the capture hotkey, the export, the data folder, the last backup, the version |

Each page says "Pengaturan" above its own name (the page's h1). Opening Pengaturan slides the
sidebar over to the parts, with **Kembali** on top: the pages' side moves out to the left as the
parts come in from the right (240 ms, the standard ease; a cross-fade only when Windows asks for
less motion), and the side not shown is inert, out of reach of the keyboard and screen readers.
Focus follows to the item the new side marks as current. Kembali returns to the page the owner
left (Hari Ini when Pengaturan was opened from an address). In the rail the parts are icons with
their names as tooltips, like the pages. `/pengaturan` alone, or a part that does not exist, opens
Tampilan; Studio's links to the key and the profile open Integrasi AI.

**Studio's history** (U2, 2026-11-09, the owner's choice, after Gemini's layout): opening Studio
slides the sidebar over to Studio's own side the same way, between the pages and Pengaturan (the
pages go left, Pengaturan opened from it comes in from the right). From the top: **Kembali** (to
the Hub page left before Studio, Hari Ini when Studio was opened from an address) with the fold
button beside it, **Ide baru** (Studio's home, marked current there), a search over the projects'
names and ideas (Esc empties it), the projects as a history (**Menunggu kamu** with each count,
then **Terbaru**, newest first, one line each), and Pengaturan (Integrasi AI) at the foot.

- The fold button (`aria-expanded`, `aria-controls`) folds the history into the rail: back, Ide
  baru, search, and Pengaturan as icons, the fold button on top carrying the count of what waits.
  The choice is kept on this computer (`localStorage`, `hub:studio-folded`), not in Hub's data,
  and applies only in Studio: Hub's pages are never folded.
- In a window that already shows the rail (1,100 px or less) the button opens the history over the
  page, the rail's place kept; Esc (focus back on the button), a click outside, or opening a
  project closes it.
- The rail is decided by the sidebar's own width (a container query), so a narrow window and a
  folded history share one set of rules.
- Hub's pages are one Kembali away; Ctrl+K reaches any page directly.

Old addresses stay as aliases: `/tugas?view=inbox|upcoming|anytime|done` lead to that list in
Proyek & Tugas, `/tugas` alone to Semua, `/tugas?view=today` to Hari Ini (or, with a task, to that
task among all open ones), `/tugas?view=reminders` to Kalender's Pengingat.

Everywhere: **Ctrl+K** command palette (go to, search, create, act). **Ctrl+Alt+H** global quick
capture (a small floating window, works while Hub is hidden; the hotkey is checked against
ULTRON's `Ctrl+Alt+U`, `Ctrl+Alt+Space`, `Ctrl+Alt+Esc`). Single-key shortcuts inside views
(`T` today, `N` new, arrows) as in Notion Calendar. Every page has a route so ULTRON can open it
("buka proyek Hub").

## Pages

- **Hari Ini, day ("Sekarang").** Greeting in Fraunces ("Selamat siang, Glenvio.") with the date,
  the Day Arc, then two columns: left is *Sekarang* (current or next event, the task in progress,
  the learning item to resume) and *Fokus* (up to three tasks); right is today's schedule as a
  compact timeline, *Belajar* (this week's active days, the item in progress), and *File terakhir*
  for the active project. If no focus was chosen, Hub suggests three (overdue, nearest deadline,
  do date today) and one click accepts.
- **Hari Ini, evening ("Tutup Hari").** From dusk the top of the page invites the review
  ("3 dari 4 fokus selesai. Tutup hari?"). The review is one calm sheet: done (with ink checks),
  slipped (Besok, pilih tanggal, lepaskan), one optional sentence, tomorrow's focus, then the close.
- **Kalender.** Week view by default; tasks from a side tray dragged onto the grid become time
  blocks (15-minute snap). Routines (school timetable) are hatched pigment blocks, events are solid,
  reminders are small markers on the time line. An over-full day says so in the header.
- **Tugas.** Quick add understands Indonesian ("laporan lab besok 19:00 #jaringan !penting").
  Do date and deadline are separate. Subtasks as a checklist. Manual order by drag.
- **Proyek.** Header with derived progress (milestones or tasks), the next step, then milestones,
  tasks, files, and the project's own history.
- **Belajar.** This week: hours and active days against the weekly goal. Tracks as tabs; items with
  status, domain, difficulty, time spent, link, and an attached write-up. A domain view as
  horizontal bars (network, web, Linux, Windows/AD, crypto, forensics, OSINT, reversing, pwn), and
  a 20-week heatmap of sessions in accent ink. *Lanjutkan* starts a session timer.
- **File.** Kinds (Dokumen, Gambar, Kode, Arsip, Media, Lainnya), Koleksi, and Proyek on the left;
  grid or list with Windows' own thumbnails on the right. A moved file is found again; a missing
  one says so and offers to locate it. Per file: open, show in Explorer, attach, keep a copy.
- **Jejak.** A day-grouped timeline of what changed and who did it (you, ULTRON, Hub). Above it,
  this week against the owner's 4-week average: tasks done, learning hours, files added, projects
  that moved. Plain sentences, small sparklines, no league tables.

## Motion

Tokens (in `tokens.css`):

| Token | Value | For |
|---|---|---|
| `--dur-instant` | 90 ms | press feedback |
| `--dur-quick` | 160 ms | hover, toggles, small fades |
| `--dur-base` | 240 ms | panels, page content, list changes |
| `--dur-slow` | 420 ms | page transitions, the Day Arc, the ink check |
| `--dur-ambient` | 2400 ms | the light changing phase |
| `--ease-standard` | `cubic-bezier(0.2, 0, 0, 1)` | things arriving |
| `--ease-exit` | `cubic-bezier(0.4, 0, 1, 1)` | things leaving (exits are shorter than entrances) |
| `--ease-ink` | `cubic-bezier(0.65, 0, 0.35, 1)` | strokes drawing themselves |
| `--spring-gentle` | ULTRON's `MORPH` spring as CSS `linear()` | layout changes |

Direct manipulation (dragging a task, a block, a file) uses a JavaScript spring that keeps its
velocity when interrupted, ULTRON's `DIRECT` rhythm (800/54).

Rules: animate only `transform`, `opacity`, `clip-path`, and stroke offsets; never width, height,
or blur. Lists stagger by 30 ms, at most 8 items. Pages: content fades and rises 8 px; a project
card grows into the project header (shared element). With Windows' reduced motion on, movement
becomes a 120 ms fade and the Day Arc close becomes a fade. Nothing loops. When the window is
hidden, the minute ticker stops.

Micro animation (P2b, `src/motion/motion.ts`), each starting at once and resting when done:

- **A choice glides.** Segmented controls (Kalender's views, Cahaya, the Belajar goal, the prompt's
  two versions, every `Segmented`) draw their pill on the group, and Studio's stages, sections, and
  panel sides their underline; a new choice moves it by clipping (`clip-path`, 240 ms), never by
  width. The first place is drawn still.
- **Rows slide into place.** In task lists, when a row leaves, arrives, or is moved, the others
  slide from where they were (240 ms) and a new one fades in (200 ms).
- **Leaving, not vanishing.** A dialog settles away (its backdrop fades, the sheet drops 8 px and
  shrinks to 98 %) and a toast slides down, in 160 ms with the exit easing, as a still copy
  without ids that nothing can reach; the real one is gone at once, so nothing waits.
- Kept from before: the task check's ink, tick, and strike; switches on the gentle spring; a
  pressed button at 97 %.
- With Windows' reduced motion nothing slides or is copied; choices change in place.

## Sound

P2 (2026-11-09, the owner's choices: micro and hero sounds, a character per place and activity, on
and quiet from the start). Code: `src/sound/` (`palette.ts` holds every rule as data, `engine.ts`
plays it, `SoundProvider.tsx` hears the controls).

From the research behind it (Material's sound guidance, the Glasgow earcon guidelines, NN/g on
animation timing):

- **Silence is the default.** Typing, keyboard movement, hover, scrolling, and moving between pages
  make no sound. A sound always comes with a visible change and never carries meaning alone.
- **Two kinds.** Micro sounds (a switch on or off, a task checked or reopened, a tab or a choice, a
  line saved in Catat Cepat, a dialog opened or closed, a block set down in Kalender, a copy) last
  under 0.3 s. Hero sounds are rare and under 1 s: the day closed in Tutup Hari, a checkpoint
  approved, the week's Belajar goal reached, the last focus task of the day done. Belajar's
  session start and stop sit between (under 0.6 s).
- **Loudness is what annoys**, so every sound stays within about 10 dB of the others in every place
  and light, under one volume of Hub's own (0 to 100, default 30, on a curve) beneath Windows'.
  Two sounds never start within 80 ms; a repeated sound moves by a few cents so it does not grate.
  Nothing plays while the window is hidden. Reminders and Claude's toasts keep Windows' own sound.
- **Character by place and activity**, all synthesised in the app (no files, nothing to license):

  | Place | Doing | Voice |
  |---|---|---|
  | Hari Ini, Kalender | planning the day | warm wooden marimba |
  | Proyek & Tugas | getting tasks done | a short kalimba pluck |
  | Studio | building an idea | soft glass |
  | Belajar | focusing | near silence: only the session's start and stop and the goal, low and muted |
  | Catat Cepat | writing a line down | paper, then a soft note |
  | Pengaturan, File, Jejak | settling things | a quieter kalimba or marimba |

- **The light follows the day**, as everything in Hub: in the morning two semitones up and bright,
  at dusk two down and darker, at night four down, darker, and a little softer.
- **The owner's controls**: Pengaturan > Tampilan > Suara: Efek suara (on), Volume (saved when the
  owner lets go), and Coba suara, which plays a place's check and hero sound even with sounds off.

## Words

Indonesian, informal and warm, "kamu". Short sentences. Numbers with units ("2 j 15 m",
"3 dari 4"). Plain about problems, always with a way forward: "Laporan Lab lewat tenggat kemarin.
Kerjakan hari ini atau pilih tanggal baru?" Never "Kamu gagal", never exclamation-heavy cheering.

## Accessibility

- Text contrast at least 4.5:1 in every phase and ink, large display text at least 3:1.
- Everything reachable by keyboard, visible focus ring in the accent (2 px, offset 2 px).
- Drag always has a keyboard or menu alternative (move to date, attach to project).
- Hit targets at least 32 px in the dense desktop UI, 40 px for primary actions.
- Respect Windows' reduced motion and text scaling.

## Not this

A grid of identical cards; gradients and glow as decoration; glassmorphism; icons without labels;
confetti; streak counters that reset; red for anything that is not destructive; charts that need a
legend to be read; more than one accent on a screen.
