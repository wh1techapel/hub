-- Studio revision R3 (docs/studio.md, "Revision: three stages"): stage 2, the tech stack. One per
-- project: the owner picks it, or Gemini recommends it and the owner changes what they like, then
-- approves it. Any later change makes it a draft again. A new table only: nothing else changes.
CREATE TABLE studio_stacks (
  studio_project_id TEXT PRIMARY KEY REFERENCES studio_projects (id) ON DELETE CASCADE,
  -- How the stack was made last: picked by the owner, or recommended by Gemini.
  mode TEXT NOT NULL CHECK (mode IN ('own', 'gemini')),
  -- {layers: [{layer, choice, why, fit, ease, cost, alternatives[], by}], summary}, checked by
  -- hub-core's `stack` module.
  body TEXT NOT NULL CHECK (json_valid(body)),
  status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'approved')),
  approved_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK ((status = 'approved') = (approved_at IS NOT NULL))
) STRICT;
