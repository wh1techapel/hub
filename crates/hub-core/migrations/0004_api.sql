-- CP5: what the local API for ULTRON needs (docs/architecture.md, "ULTRON integration").

-- A note can explain a task: a file ULTRON was told about ("my networking assignment, due
-- Friday") becomes a task, and the owner's words stay beside it as a note pointing at it.
ALTER TABLE journal ADD COLUMN task_id TEXT REFERENCES tasks (id);
CREATE INDEX journal_task ON journal (task_id) WHERE task_id IS NOT NULL;

-- Rows brought in from elsewhere (ULTRON's old database), by the reference they had there
-- ("ultron:task:12"), so an import that runs twice keeps one copy.
CREATE TABLE imports (
  ref TEXT PRIMARY KEY CHECK (length(ref) BETWEEN 1 AND 100),
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  at TEXT NOT NULL
) STRICT;
