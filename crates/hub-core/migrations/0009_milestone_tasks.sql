-- Tonggak hold tasks (CONTEXT.md, "Proyek"). A project's progress always comes from its tasks; a
-- milestone is a dated group of them, done once all its tasks are done (or ticked by hand while it
-- has none). Before this migration milestones were a separate checklist, and the project's progress
-- switched to them as soon as there was one, which the owner found hard to follow.
--
-- A task points at one milestone of its own project, or none. Hub keeps the two in step (a task
-- moved to another project, or a milestone removed, leaves the task without one). Existing tasks
-- start without a milestone; existing milestones keep their ticks.
ALTER TABLE tasks ADD COLUMN milestone_id TEXT REFERENCES milestones (id) ON DELETE SET NULL;
CREATE INDEX tasks_milestone ON tasks (milestone_id) WHERE deleted_at IS NULL AND milestone_id IS NOT NULL;

-- The per-project choice between the two ways of counting was never used; there is one way now.
ALTER TABLE projects DROP COLUMN progress_mode;
