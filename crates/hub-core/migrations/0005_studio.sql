-- Hub Studio (docs/studio.md, "Data"): ideas talked through with Gemini, PRDs, roadmaps of
-- checkpoints, and Claude Code's reports. Hub makes a copy of the database before this runs
-- (backups/hub-before-schema-5.db).

-- Jejak gains two actors: Claude Code (over MCP) and Gemini (what it wrote in Studio). SQLite
-- cannot change a CHECK constraint, so the table is rebuilt with every row kept, then given back
-- its indexes and its append-only rule.
DROP TRIGGER activity_append_only_update;
DROP TRIGGER activity_append_only_delete;

CREATE TABLE activity_new (
  id TEXT PRIMARY KEY,
  at TEXT NOT NULL,
  actor TEXT NOT NULL CHECK (actor IN ('owner', 'ultron', 'hub', 'claude', 'gemini')),
  verb TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  entity_id TEXT,
  summary TEXT NOT NULL,
  details TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(details))
) STRICT;

INSERT INTO activity_new (id, at, actor, verb, entity_type, entity_id, summary, details)
  SELECT id, at, actor, verb, entity_type, entity_id, summary, details FROM activity;

DROP TABLE activity;
ALTER TABLE activity_new RENAME TO activity;

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

-- A Studio project: an idea on its way to being built. It becomes linked to a Hub project once
-- its roadmap is approved; until then it may stay an idea.
CREATE TABLE studio_projects (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
  idea TEXT NOT NULL DEFAULT '' CHECK (length(idea) <= 2000),
  stage TEXT NOT NULL DEFAULT 'explore'
    CHECK (stage IN ('explore', 'clarify', 'prd', 'roadmap', 'build')),
  language TEXT NOT NULL DEFAULT 'id' CHECK (language IN ('id')),
  project_id TEXT REFERENCES projects (id) ON DELETE SET NULL,
  -- The repository folder the owner confirmed; Claude Code may connect only from there.
  repo_path TEXT,
  claude_access TEXT NOT NULL DEFAULT 'none' CHECK (claude_access IN ('none', 'read', 'report')),
  -- Gemini's running summary of the conversation before `summary_until` (a message id).
  summary TEXT NOT NULL DEFAULT '' CHECK (length(summary) <= 20000),
  summary_until TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
) STRICT;
CREATE UNIQUE INDEX studio_projects_project ON studio_projects (project_id)
  WHERE project_id IS NOT NULL AND deleted_at IS NULL;

-- The conversation. A reply is written as `streaming` when it starts and finished once; the
-- conversation is its own record, so messages leave no row in `activity`.
CREATE TABLE studio_messages (
  id TEXT PRIMARY KEY,
  studio_project_id TEXT NOT NULL REFERENCES studio_projects (id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('owner', 'model')),
  stage TEXT NOT NULL CHECK (stage IN ('explore', 'clarify', 'prd', 'roadmap', 'build')),
  text TEXT NOT NULL CHECK (length(text) <= 100000),
  -- Structured parts of a reply: questions with options, a proposed stage.
  parts TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(parts)),
  state TEXT NOT NULL DEFAULT 'complete'
    CHECK (state IN ('streaming', 'complete', 'stopped', 'failed')),
  error_kind TEXT,
  model TEXT,
  input_tokens INTEGER CHECK (input_tokens IS NULL OR input_tokens >= 0),
  output_tokens INTEGER CHECK (output_tokens IS NULL OR output_tokens >= 0),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (role = 'model' OR (state = 'complete' AND length(text) BETWEEN 1 AND 20000))
) STRICT;
CREATE INDEX studio_messages_project ON studio_messages (studio_project_id, id);

-- What the conversation settled: decisions, assumptions to validate, open questions, constraints.
-- Gemini reads it every turn and does not ask again.
CREATE TABLE studio_ledger (
  id TEXT PRIMARY KEY,
  studio_project_id TEXT NOT NULL REFERENCES studio_projects (id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('decision', 'assumption', 'question', 'constraint')),
  topic TEXT NOT NULL CHECK (length(topic) BETWEEN 1 AND 120),
  statement TEXT NOT NULL CHECK (length(statement) BETWEEN 1 AND 2000),
  rationale TEXT NOT NULL DEFAULT '' CHECK (length(rationale) <= 2000),
  status TEXT NOT NULL DEFAULT 'active'
    CHECK (status IN ('active', 'confirmed', 'retired', 'answered')),
  stated_by TEXT NOT NULL CHECK (stated_by IN ('owner', 'gemini')),
  message_id TEXT REFERENCES studio_messages (id) ON DELETE SET NULL,
  replaces_id TEXT REFERENCES studio_ledger (id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (status <> 'confirmed' OR kind = 'assumption'),
  CHECK (status <> 'answered' OR kind = 'question')
) STRICT;
CREATE INDEX studio_ledger_project ON studio_ledger (studio_project_id);

-- PRDs. The working draft has no number and changes in place; a numbered version never changes
-- again except its status (saved, approved, superseded).
CREATE TABLE studio_prd_versions (
  id TEXT PRIMARY KEY,
  studio_project_id TEXT NOT NULL REFERENCES studio_projects (id) ON DELETE CASCADE,
  number INTEGER CHECK (number IS NULL OR number >= 1),
  status TEXT NOT NULL CHECK (status IN ('draft', 'saved', 'approved', 'superseded')),
  body TEXT NOT NULL CHECK (json_valid(body)),
  change_note TEXT NOT NULL DEFAULT '' CHECK (length(change_note) <= 1000),
  made_by TEXT NOT NULL CHECK (made_by IN ('owner', 'gemini')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  approved_at TEXT,
  CHECK ((number IS NULL) = (status = 'draft')),
  UNIQUE (studio_project_id, number)
) STRICT;
CREATE UNIQUE INDEX studio_prd_working ON studio_prd_versions (studio_project_id) WHERE number IS NULL;

CREATE TRIGGER studio_prd_versions_frozen BEFORE UPDATE OF studio_project_id, number, body, made_by, created_at
  ON studio_prd_versions WHEN OLD.number IS NOT NULL
BEGIN
  SELECT RAISE(ABORT, 'a saved PRD version does not change');
END;

-- Roadmaps, each made from one PRD version.
CREATE TABLE studio_roadmaps (
  id TEXT PRIMARY KEY,
  studio_project_id TEXT NOT NULL REFERENCES studio_projects (id) ON DELETE CASCADE,
  prd_version_id TEXT NOT NULL REFERENCES studio_prd_versions (id),
  status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'approved', 'superseded')),
  made_by TEXT NOT NULL CHECK (made_by IN ('owner', 'gemini')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  approved_at TEXT
) STRICT;
CREATE INDEX studio_roadmaps_project ON studio_roadmaps (studio_project_id);

CREATE TABLE studio_checkpoints (
  id TEXT PRIMARY KEY,
  roadmap_id TEXT NOT NULL REFERENCES studio_roadmaps (id) ON DELETE CASCADE,
  code TEXT NOT NULL CHECK (code GLOB 'CP-[0-9][0-9]*'),
  position INTEGER NOT NULL CHECK (position >= 1),
  title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
  goal TEXT NOT NULL DEFAULT '' CHECK (length(goal) <= 1000),
  description TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 4000),
  difficulty TEXT NOT NULL DEFAULT 'medium' CHECK (difficulty IN ('low', 'medium', 'high')),
  -- Lists, as JSON arrays: feature ids, checkpoint codes, risks, acceptance criteria, required
  -- tests, deliverables, the Definition of Done.
  features TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(features)),
  depends_on TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(depends_on)),
  risks TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(risks)),
  acceptance TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(acceptance)),
  required_tests TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(required_tests)),
  deliverables TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(deliverables)),
  done_when TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(done_when)),
  status TEXT NOT NULL DEFAULT 'planned'
    CHECK (status IN ('planned', 'ready', 'in_progress', 'blocked', 'in_review', 'completed')),
  ready_at TEXT,
  started_at TEXT,
  submitted_at TEXT,
  completed_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (roadmap_id, code)
) STRICT;

CREATE TABLE studio_tasks (
  id TEXT PRIMARY KEY,
  checkpoint_id TEXT NOT NULL REFERENCES studio_checkpoints (id) ON DELETE CASCADE,
  parent_id TEXT REFERENCES studio_tasks (id) ON DELETE CASCADE,
  code TEXT NOT NULL CHECK (length(code) BETWEEN 1 AND 40),
  feature_id TEXT,
  title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 300),
  detail TEXT NOT NULL DEFAULT '' CHECK (length(detail) <= 4000),
  status TEXT NOT NULL DEFAULT 'todo' CHECK (status IN ('todo', 'doing', 'done', 'blocked')),
  note TEXT NOT NULL DEFAULT '' CHECK (length(note) <= 2000),
  changed_by TEXT NOT NULL CHECK (changed_by IN ('owner', 'claude', 'gemini')),
  order_key TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
) STRICT;
CREATE INDEX studio_tasks_checkpoint ON studio_tasks (checkpoint_id);

-- What Claude (or the owner) reported about a checkpoint. A report never changes.
CREATE TABLE studio_reports (
  id TEXT PRIMARY KEY,
  checkpoint_id TEXT NOT NULL REFERENCES studio_checkpoints (id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('progress', 'submission')),
  summary TEXT NOT NULL CHECK (length(summary) BETWEEN 1 AND 4000),
  body TEXT NOT NULL CHECK (json_valid(body)),
  made_by TEXT NOT NULL CHECK (made_by IN ('owner', 'claude')),
  created_at TEXT NOT NULL
) STRICT;
CREATE INDEX studio_reports_checkpoint ON studio_reports (checkpoint_id);

CREATE TRIGGER studio_reports_frozen BEFORE UPDATE ON studio_reports
BEGIN
  SELECT RAISE(ABORT, 'a report does not change');
END;

-- What waits for someone: Claude's questions and blockers, a report to review, the owner's
-- requested changes.
CREATE TABLE studio_requests (
  id TEXT PRIMARY KEY,
  studio_project_id TEXT NOT NULL REFERENCES studio_projects (id) ON DELETE CASCADE,
  checkpoint_id TEXT REFERENCES studio_checkpoints (id) ON DELETE SET NULL,
  kind TEXT NOT NULL CHECK (kind IN ('question', 'blocker', 'review', 'changes')),
  text TEXT NOT NULL CHECK (length(text) BETWEEN 1 AND 4000),
  options TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(options)),
  status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'answered', 'withdrawn')),
  answer TEXT CHECK (answer IS NULL OR length(answer) <= 4000),
  made_by TEXT NOT NULL CHECK (made_by IN ('owner', 'claude', 'gemini')),
  answered_by TEXT CHECK (answered_by IS NULL OR answered_by IN ('owner', 'claude')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  answered_at TEXT
) STRICT;
CREATE INDEX studio_requests_project ON studio_requests (studio_project_id, status);
