-- Lentera CP02: Sesi Fokus, a Pomodoro kept by Hub (CONTEXT.md, "Lentera on the desktop").
-- Forward only; never edit after release (see 0001_init.sql).

-- The one session running, if any. Like Belajar's timer it outlives Hub: the phase is read from
-- its times (focus.rs). `phase_started_at` is when the phase would have started had it never been
-- paused, so the time left is always its length minus (now, or paused_at, minus it). `seen_at` is
-- when Hub last looked at a running session: time Hub did not see (the computer asleep, Hub
-- closed) pauses the session instead of counting. Lengths are in seconds and kept from the start,
-- so a change in Pengaturan never moves a phase already running.
CREATE TABLE focus_run (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  task_id TEXT REFERENCES tasks (id) ON DELETE SET NULL,
  item_id TEXT REFERENCES learning_items (id) ON DELETE SET NULL,
  title TEXT NOT NULL DEFAULT '',
  phase TEXT NOT NULL CHECK (phase IN ('focus', 'short_break', 'long_break', 'waiting')),
  phase_started_at TEXT NOT NULL,
  phase_seconds INTEGER NOT NULL CHECK (phase_seconds >= 0),
  paused_at TEXT,
  seen_at TEXT NOT NULL,
  blocks_done INTEGER NOT NULL DEFAULT 0 CHECK (blocks_done >= 0),
  focused_seconds INTEGER NOT NULL DEFAULT 0 CHECK (focused_seconds >= 0),
  tucked INTEGER NOT NULL DEFAULT 0 CHECK (tucked IN (0, 1)),
  started_at TEXT NOT NULL,
  focus_seconds INTEGER NOT NULL CHECK (focus_seconds > 0),
  break_seconds INTEGER NOT NULL CHECK (break_seconds > 0),
  long_break_seconds INTEGER NOT NULL CHECK (long_break_seconds > 0),
  long_break_every INTEGER NOT NULL CHECK (long_break_every > 0),
  CHECK (task_id IS NULL OR item_id IS NULL)
) STRICT;

-- Every focus block, finished or stopped early (breaks are not kept). A block on a Belajar item
-- is also a learning session (`session_id`), so Belajar counts it once, as its own.
CREATE TABLE focus_blocks (
  id TEXT PRIMARY KEY,
  started_at TEXT NOT NULL,
  seconds INTEGER NOT NULL CHECK (seconds > 0),
  planned_seconds INTEGER NOT NULL CHECK (planned_seconds > 0),
  completed INTEGER NOT NULL CHECK (completed IN (0, 1)),
  task_id TEXT REFERENCES tasks (id) ON DELETE SET NULL,
  item_id TEXT REFERENCES learning_items (id) ON DELETE SET NULL,
  session_id TEXT REFERENCES learning_sessions (id) ON DELETE SET NULL,
  title TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL
) STRICT;
CREATE INDEX focus_blocks_started ON focus_blocks (started_at);
CREATE INDEX focus_blocks_task ON focus_blocks (task_id) WHERE task_id IS NOT NULL;
