-- Hub schema, version 1.
--
-- Forward only: once released this file is never edited; a change is a new numbered file.
-- Instants are UTC text 'YYYY-MM-DDTHH:MM:SSZ' (always that exact shape, so text order is time
-- order). Local dates are 'YYYY-MM-DD', local times 'HH:MM', in the owner's time zone. Ids are
-- UUIDv7 text. Deletion is soft (deleted_at). Enumerations that will grow (activity verbs,
-- learning domains, file kinds) are checked in Rust, not here, so they can grow without
-- rebuilding a table. The search index (FTS5) arrives with search in CP4.

CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL CHECK (json_valid(value)),
  updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE areas (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
  pigment TEXT NOT NULL DEFAULT 'batu'
    CHECK (pigment IN ('lazuardi', 'toska', 'lumut', 'kunyit', 'terakota', 'mawar', 'lavender', 'batu')),
  order_key TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
) STRICT;

CREATE TABLE projects (
  id TEXT PRIMARY KEY,
  area_id TEXT REFERENCES areas (id) ON DELETE SET NULL,
  name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
  description TEXT NOT NULL DEFAULT '',
  pigment TEXT NOT NULL DEFAULT 'batu'
    CHECK (pigment IN ('lazuardi', 'toska', 'lumut', 'kunyit', 'terakota', 'mawar', 'lavender', 'batu')),
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'paused', 'done', 'archived')),
  start_date TEXT,
  target_date TEXT,
  progress_mode TEXT NOT NULL DEFAULT 'tasks' CHECK (progress_mode IN ('milestones', 'tasks')),
  order_key TEXT NOT NULL,
  source TEXT NOT NULL DEFAULT 'owner' CHECK (source IN ('owner', 'ultron', 'hub', 'import')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
) STRICT;

CREATE TABLE milestones (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
  title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
  due_date TEXT,
  done_at TEXT,
  order_key TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
) STRICT;
CREATE INDEX milestones_project ON milestones (project_id) WHERE deleted_at IS NULL;

CREATE TABLE tasks (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 300),
  notes TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'todo' CHECK (status IN ('todo', 'doing', 'done', 'cancelled')),
  priority INTEGER NOT NULL DEFAULT 0 CHECK (priority BETWEEN 0 AND 3),
  -- When to work on it, separate from when it is due.
  do_date TEXT,
  due_date TEXT,
  due_time TEXT,
  estimate_min INTEGER CHECK (estimate_min IS NULL OR estimate_min > 0),
  project_id TEXT REFERENCES projects (id) ON DELETE SET NULL,
  area_id TEXT REFERENCES areas (id) ON DELETE SET NULL,
  parent_id TEXT REFERENCES tasks (id) ON DELETE CASCADE,
  order_key TEXT NOT NULL,
  completed_at TEXT,
  source TEXT NOT NULL DEFAULT 'owner' CHECK (source IN ('owner', 'ultron', 'hub', 'import')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  CHECK (due_time IS NULL OR due_date IS NOT NULL),
  CHECK ((status = 'done') = (completed_at IS NOT NULL))
) STRICT;
CREATE INDEX tasks_do_date ON tasks (do_date) WHERE deleted_at IS NULL;
CREATE INDEX tasks_due_date ON tasks (due_date) WHERE deleted_at IS NULL;
CREATE INDEX tasks_project ON tasks (project_id) WHERE deleted_at IS NULL;
CREATE INDEX tasks_parent ON tasks (parent_id) WHERE deleted_at IS NULL;

-- Fokus: at most three tasks for a day.
CREATE TABLE focus (
  date TEXT NOT NULL,
  slot INTEGER NOT NULL CHECK (slot BETWEEN 1 AND 3),
  task_id TEXT NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
  PRIMARY KEY (date, slot),
  UNIQUE (date, task_id)
) STRICT;

CREATE TABLE routines (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
  kind TEXT NOT NULL DEFAULT 'other' CHECK (kind IN ('lesson', 'habit', 'study', 'other')),
  rrule TEXT NOT NULL,
  start_time TEXT NOT NULL,
  duration_min INTEGER NOT NULL CHECK (duration_min BETWEEN 1 AND 1440),
  starts_on TEXT NOT NULL,
  ends_on TEXT,
  location TEXT NOT NULL DEFAULT '',
  pigment TEXT NOT NULL DEFAULT 'batu'
    CHECK (pigment IN ('lazuardi', 'toska', 'lumut', 'kunyit', 'terakota', 'mawar', 'lavender', 'batu')),
  project_id TEXT REFERENCES projects (id) ON DELETE SET NULL,
  active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
  source TEXT NOT NULL DEFAULT 'owner' CHECK (source IN ('owner', 'ultron', 'hub', 'import')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
) STRICT;

CREATE TABLE routine_exceptions (
  routine_id TEXT NOT NULL REFERENCES routines (id) ON DELETE CASCADE,
  date TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('skipped', 'moved')),
  moved_start TEXT,
  moved_end TEXT,
  PRIMARY KEY (routine_id, date),
  CHECK (kind = 'skipped' OR (moved_start IS NOT NULL AND moved_end IS NOT NULL))
) STRICT;

CREATE TABLE events (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
  notes TEXT NOT NULL DEFAULT '',
  location TEXT NOT NULL DEFAULT '',
  all_day INTEGER NOT NULL DEFAULT 0 CHECK (all_day IN (0, 1)),
  -- Timed events: instants. All-day events: local dates, end inclusive.
  start_at TEXT,
  end_at TEXT,
  start_date TEXT,
  end_date TEXT,
  pigment TEXT NOT NULL DEFAULT 'batu'
    CHECK (pigment IN ('lazuardi', 'toska', 'lumut', 'kunyit', 'terakota', 'mawar', 'lavender', 'batu')),
  project_id TEXT REFERENCES projects (id) ON DELETE SET NULL,
  -- A task placed on the calendar: the event is its time block.
  task_id TEXT REFERENCES tasks (id) ON DELETE SET NULL,
  source TEXT NOT NULL DEFAULT 'owner' CHECK (source IN ('owner', 'ultron', 'hub', 'import')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  CHECK (
    (all_day = 0 AND start_at IS NOT NULL AND end_at IS NOT NULL AND end_at > start_at
      AND start_date IS NULL AND end_date IS NULL)
    OR (all_day = 1 AND start_date IS NOT NULL AND end_date IS NOT NULL AND end_date >= start_date
      AND start_at IS NULL AND end_at IS NULL)
  )
) STRICT;
CREATE INDEX events_start_at ON events (start_at) WHERE deleted_at IS NULL;
CREATE INDEX events_dates ON events (start_date, end_date) WHERE deleted_at IS NULL;

CREATE TABLE reminders (
  id TEXT PRIMARY KEY,
  text TEXT NOT NULL CHECK (length(text) BETWEEN 1 AND 300),
  due_at TEXT NOT NULL,
  repeat_rule TEXT,
  state TEXT NOT NULL DEFAULT 'scheduled'
    CHECK (state IN ('scheduled', 'fired', 'snoozed', 'done', 'cancelled')),
  task_id TEXT REFERENCES tasks (id) ON DELETE SET NULL,
  event_id TEXT REFERENCES events (id) ON DELETE SET NULL,
  fired_at TEXT,
  source TEXT NOT NULL DEFAULT 'owner' CHECK (source IN ('owner', 'ultron', 'hub', 'import')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
) STRICT;
CREATE INDEX reminders_due_at ON reminders (due_at) WHERE deleted_at IS NULL;

CREATE TABLE files (
  id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL CHECK (length(display_name) BETWEEN 1 AND 260),
  kind TEXT NOT NULL,
  extension TEXT NOT NULL DEFAULT '',
  size_bytes INTEGER,
  path TEXT NOT NULL,
  -- The file's identity on its volume, to find it again after a move on the same drive.
  volume_serial INTEGER,
  file_index TEXT,
  state TEXT NOT NULL DEFAULT 'ok' CHECK (state IN ('ok', 'moved', 'missing')),
  vault_path TEXT,
  sha256 TEXT,
  note TEXT NOT NULL DEFAULT '',
  added_at TEXT NOT NULL,
  last_seen_at TEXT,
  source TEXT NOT NULL DEFAULT 'owner' CHECK (source IN ('owner', 'ultron', 'hub', 'import')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
) STRICT;
CREATE INDEX files_identity ON files (volume_serial, file_index) WHERE deleted_at IS NULL;

CREATE TABLE file_links (
  file_id TEXT NOT NULL REFERENCES files (id) ON DELETE CASCADE,
  target_type TEXT NOT NULL
    CHECK (target_type IN ('project', 'task', 'learning_item', 'event', 'journal')),
  target_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (file_id, target_type, target_id)
) STRICT;
CREATE INDEX file_links_target ON file_links (target_type, target_id);

CREATE TABLE collections (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
  pigment TEXT NOT NULL DEFAULT 'batu'
    CHECK (pigment IN ('lazuardi', 'toska', 'lumut', 'kunyit', 'terakota', 'mawar', 'lavender', 'batu')),
  order_key TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
) STRICT;

CREATE TABLE file_collections (
  file_id TEXT NOT NULL REFERENCES files (id) ON DELETE CASCADE,
  collection_id TEXT NOT NULL REFERENCES collections (id) ON DELETE CASCADE,
  PRIMARY KEY (file_id, collection_id)
) STRICT;

-- Jalur: TryHackMe, Hack The Box, a CTF, Lab Sekolah, Homelab.
CREATE TABLE tracks (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
  kind TEXT NOT NULL CHECK (kind IN ('platform', 'ctf', 'lab')),
  url TEXT,
  pigment TEXT NOT NULL DEFAULT 'batu'
    CHECK (pigment IN ('lazuardi', 'toska', 'lumut', 'kunyit', 'terakota', 'mawar', 'lavender', 'batu')),
  order_key TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
) STRICT;

CREATE TABLE ctf_events (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
  starts_on TEXT,
  ends_on TEXT,
  team TEXT,
  rank INTEGER CHECK (rank IS NULL OR rank > 0),
  participants INTEGER CHECK (participants IS NULL OR participants > 0),
  url TEXT,
  notes TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
) STRICT;

CREATE TABLE learning_items (
  id TEXT PRIMARY KEY,
  track_id TEXT NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
  ctf_event_id TEXT REFERENCES ctf_events (id) ON DELETE SET NULL,
  title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
  kind TEXT NOT NULL CHECK (kind IN ('room', 'path', 'machine', 'challenge', 'practical')),
  domain TEXT NOT NULL,
  difficulty TEXT CHECK (difficulty IS NULL OR difficulty IN ('easy', 'medium', 'hard', 'insane')),
  status TEXT NOT NULL DEFAULT 'todo' CHECK (status IN ('todo', 'doing', 'done', 'dropped')),
  url TEXT,
  points INTEGER,
  progress INTEGER CHECK (progress IS NULL OR progress BETWEEN 0 AND 100),
  started_at TEXT,
  completed_at TEXT,
  notes TEXT NOT NULL DEFAULT '',
  order_key TEXT NOT NULL,
  source TEXT NOT NULL DEFAULT 'owner' CHECK (source IN ('owner', 'ultron', 'hub', 'import')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
) STRICT;
CREATE INDEX learning_items_track ON learning_items (track_id) WHERE deleted_at IS NULL;

CREATE TABLE learning_sessions (
  id TEXT PRIMARY KEY,
  item_id TEXT NOT NULL REFERENCES learning_items (id) ON DELETE CASCADE,
  started_at TEXT NOT NULL,
  duration_min INTEGER NOT NULL CHECK (duration_min >= 0),
  note TEXT NOT NULL DEFAULT '',
  source TEXT NOT NULL DEFAULT 'owner' CHECK (source IN ('owner', 'ultron', 'hub', 'import')),
  created_at TEXT NOT NULL
) STRICT;
CREATE INDEX learning_sessions_started ON learning_sessions (started_at);

-- Catatan: notes, done activities, and the evening sentence.
CREATE TABLE journal (
  id TEXT PRIMARY KEY,
  date TEXT NOT NULL,
  text TEXT NOT NULL CHECK (length(text) BETWEEN 1 AND 4000),
  kind TEXT NOT NULL DEFAULT 'note' CHECK (kind IN ('note', 'done', 'reflection')),
  source TEXT NOT NULL DEFAULT 'owner' CHECK (source IN ('owner', 'ultron', 'hub', 'import')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
) STRICT;
CREATE INDEX journal_date ON journal (date) WHERE deleted_at IS NULL;

CREATE TABLE tags (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE CHECK (length(name) BETWEEN 1 AND 40),
  created_at TEXT NOT NULL
) STRICT;

CREATE TABLE taggings (
  tag_id TEXT NOT NULL REFERENCES tags (id) ON DELETE CASCADE,
  target_type TEXT NOT NULL CHECK (target_type IN ('task', 'file', 'learning_item', 'project')),
  target_id TEXT NOT NULL,
  PRIMARY KEY (tag_id, target_type, target_id)
) STRICT;

-- Jejak: what changed, when, and by whom. Append only.
CREATE TABLE activity (
  id TEXT PRIMARY KEY,
  at TEXT NOT NULL,
  actor TEXT NOT NULL CHECK (actor IN ('owner', 'ultron', 'hub')),
  verb TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  entity_id TEXT,
  summary TEXT NOT NULL,
  details TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(details))
) STRICT;
CREATE INDEX activity_at ON activity (at);
CREATE INDEX activity_entity ON activity (entity_type, entity_id);

CREATE TRIGGER activity_append_only_update BEFORE UPDATE ON activity
BEGIN
  SELECT RAISE(ABORT, 'activity is append-only');
END;

CREATE TRIGGER activity_append_only_delete BEFORE DELETE ON activity
BEGIN
  SELECT RAISE(ABORT, 'activity is append-only');
END;

-- Clients of the local API (CP5). Only a hash of each token is kept.
CREATE TABLE api_clients (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  token_hash TEXT NOT NULL UNIQUE,
  scopes TEXT NOT NULL CHECK (json_valid(scopes)),
  created_at TEXT NOT NULL,
  last_used_at TEXT,
  revoked_at TEXT
) STRICT;
