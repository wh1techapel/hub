-- Hub schema, version 2: the productivity system (CP3).
--
-- Forward only. A day closed with Tutup Hari is remembered here (with its sentence, when one was
-- written), so the evening invitation and the 21.00 nudge know the day is done.

CREATE TABLE day_closures (
  date TEXT PRIMARY KEY,
  closed_at TEXT NOT NULL,
  reflection_id TEXT REFERENCES journal (id) ON DELETE SET NULL
) STRICT;

-- Lists the UI reads often.
CREATE INDEX tasks_open_order ON tasks (status, order_key) WHERE deleted_at IS NULL;
CREATE INDEX events_task ON events (task_id) WHERE deleted_at IS NULL AND task_id IS NOT NULL;
CREATE INDEX reminders_state_due ON reminders (state, due_at) WHERE deleted_at IS NULL;
CREATE INDEX routines_active ON routines (active) WHERE deleted_at IS NULL;
CREATE INDEX projects_area ON projects (area_id) WHERE deleted_at IS NULL;
