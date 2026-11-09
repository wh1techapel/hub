-- Studio revision R5 (docs/studio.md, "Claude's status"): Claude Code's sessions as the bridge
-- reports them. The bridge says hello once Claude Code has it open, reports every minute and after
-- every tool call, and says goodbye when Claude Code closes its input. A new table only: nothing
-- else changes.
--
-- Not exported and not in Jejak: it says how Claude Code was connected, not what the owner's data
-- is, and Claude's changes to the data are in `activity` already. Hub keeps 30 days of it, and at
-- most 100 sessions.
CREATE TABLE studio_claude_sessions (
  id TEXT PRIMARY KEY,
  -- Where Claude Code runs, and the Studio project whose repository folder holds it (Hub decides).
  folder TEXT NOT NULL CHECK (length(folder) BETWEEN 1 AND 1000),
  studio_project_id TEXT REFERENCES studio_projects (id) ON DELETE SET NULL,
  -- The MCP client as it named itself (`claude-code`, its version), and the bridge's version.
  client TEXT NOT NULL DEFAULT '' CHECK (length(client) <= 60),
  client_version TEXT NOT NULL DEFAULT '' CHECK (length(client_version) <= 40),
  bridge_version TEXT NOT NULL DEFAULT '' CHECK (length(bridge_version) <= 40),
  started_at TEXT NOT NULL,
  -- The last time the bridge was heard from.
  seen_at TEXT NOT NULL,
  -- When the bridge said goodbye. Empty: still open, or gone without a word.
  ended_at TEXT,
  -- The last tool call: which, when, whether it worked, why not, and the project it was about.
  tool TEXT,
  tool_at TEXT,
  tool_ok INTEGER CHECK (tool_ok IS NULL OR tool_ok IN (0, 1)),
  tool_reason TEXT CHECK (tool_reason IS NULL OR length(tool_reason) BETWEEN 1 AND 40),
  tool_project_id TEXT REFERENCES studio_projects (id) ON DELETE SET NULL,
  tool_calls INTEGER NOT NULL DEFAULT 0 CHECK (tool_calls >= 0),
  CHECK ((tool IS NULL) = (tool_at IS NULL) AND (tool IS NULL) = (tool_ok IS NULL))
) STRICT;
CREATE INDEX studio_claude_sessions_seen ON studio_claude_sessions (seen_at);
CREATE INDEX studio_claude_sessions_project ON studio_claude_sessions (studio_project_id)
  WHERE studio_project_id IS NOT NULL;
CREATE INDEX studio_claude_sessions_tool ON studio_claude_sessions (tool_project_id, tool_at)
  WHERE tool_project_id IS NOT NULL;
