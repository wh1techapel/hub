-- CP4: the file library, Belajar's running timer, and the search index.
-- Forward only; never edit after release (see 0001_init.sql).

CREATE INDEX files_added ON files (added_at) WHERE deleted_at IS NULL;
CREATE INDEX file_links_file ON file_links (file_id);
CREATE INDEX file_collections_collection ON file_collections (collection_id);
CREATE INDEX learning_sessions_item ON learning_sessions (item_id);

-- The one session being timed, if any. A row here is a timer that keeps running while Hub is
-- closed; stopping it writes a learning_sessions row and deletes this one.
CREATE TABLE learning_timer (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  item_id TEXT NOT NULL REFERENCES learning_items (id) ON DELETE CASCADE,
  started_at TEXT NOT NULL
) STRICT;

-- Search: one row per searchable thing, kept in step by the triggers below. Trigram tokens find
-- "nmap" inside "THM: Nmap Live Host Discovery"; queries under three characters use LIKE on the
-- title instead (search.rs).
CREATE VIRTUAL TABLE search USING fts5(
  kind UNINDEXED,
  ref_id UNINDEXED,
  title,
  body,
  tokenize = 'trigram remove_diacritics 1'
);

CREATE TRIGGER search_tasks_insert AFTER INSERT ON tasks WHEN new.deleted_at IS NULL BEGIN
  INSERT INTO search (kind, ref_id, title, body) VALUES ('task', new.id, new.title, new.notes);
END;
CREATE TRIGGER search_tasks_update AFTER UPDATE OF title, notes, deleted_at ON tasks BEGIN
  DELETE FROM search WHERE kind = 'task' AND ref_id = old.id;
  INSERT INTO search (kind, ref_id, title, body)
    SELECT 'task', new.id, new.title, new.notes WHERE new.deleted_at IS NULL;
END;
CREATE TRIGGER search_tasks_delete AFTER DELETE ON tasks BEGIN
  DELETE FROM search WHERE kind = 'task' AND ref_id = old.id;
END;

CREATE TRIGGER search_projects_insert AFTER INSERT ON projects WHEN new.deleted_at IS NULL BEGIN
  INSERT INTO search (kind, ref_id, title, body) VALUES ('project', new.id, new.name, new.description);
END;
CREATE TRIGGER search_projects_update AFTER UPDATE OF name, description, deleted_at ON projects BEGIN
  DELETE FROM search WHERE kind = 'project' AND ref_id = old.id;
  INSERT INTO search (kind, ref_id, title, body)
    SELECT 'project', new.id, new.name, new.description WHERE new.deleted_at IS NULL;
END;
CREATE TRIGGER search_projects_delete AFTER DELETE ON projects BEGIN
  DELETE FROM search WHERE kind = 'project' AND ref_id = old.id;
END;

CREATE TRIGGER search_files_insert AFTER INSERT ON files WHEN new.deleted_at IS NULL BEGIN
  INSERT INTO search (kind, ref_id, title, body) VALUES ('file', new.id, new.display_name, new.note);
END;
CREATE TRIGGER search_files_update AFTER UPDATE OF display_name, note, deleted_at ON files BEGIN
  DELETE FROM search WHERE kind = 'file' AND ref_id = old.id;
  INSERT INTO search (kind, ref_id, title, body)
    SELECT 'file', new.id, new.display_name, new.note WHERE new.deleted_at IS NULL;
END;
CREATE TRIGGER search_files_delete AFTER DELETE ON files BEGIN
  DELETE FROM search WHERE kind = 'file' AND ref_id = old.id;
END;

CREATE TRIGGER search_collections_insert AFTER INSERT ON collections WHEN new.deleted_at IS NULL BEGIN
  INSERT INTO search (kind, ref_id, title, body) VALUES ('collection', new.id, new.name, '');
END;
CREATE TRIGGER search_collections_update AFTER UPDATE OF name, deleted_at ON collections BEGIN
  DELETE FROM search WHERE kind = 'collection' AND ref_id = old.id;
  INSERT INTO search (kind, ref_id, title, body)
    SELECT 'collection', new.id, new.name, '' WHERE new.deleted_at IS NULL;
END;
CREATE TRIGGER search_collections_delete AFTER DELETE ON collections BEGIN
  DELETE FROM search WHERE kind = 'collection' AND ref_id = old.id;
END;

CREATE TRIGGER search_learning_insert AFTER INSERT ON learning_items WHEN new.deleted_at IS NULL BEGIN
  INSERT INTO search (kind, ref_id, title, body) VALUES ('learning_item', new.id, new.title, new.notes);
END;
CREATE TRIGGER search_learning_update AFTER UPDATE OF title, notes, deleted_at ON learning_items BEGIN
  DELETE FROM search WHERE kind = 'learning_item' AND ref_id = old.id;
  INSERT INTO search (kind, ref_id, title, body)
    SELECT 'learning_item', new.id, new.title, new.notes WHERE new.deleted_at IS NULL;
END;
CREATE TRIGGER search_learning_delete AFTER DELETE ON learning_items BEGIN
  DELETE FROM search WHERE kind = 'learning_item' AND ref_id = old.id;
END;

CREATE TRIGGER search_events_insert AFTER INSERT ON events WHEN new.deleted_at IS NULL BEGIN
  INSERT INTO search (kind, ref_id, title, body) VALUES ('event', new.id, new.title, new.location || ' ' || new.notes);
END;
CREATE TRIGGER search_events_update AFTER UPDATE OF title, location, notes, deleted_at ON events BEGIN
  DELETE FROM search WHERE kind = 'event' AND ref_id = old.id;
  INSERT INTO search (kind, ref_id, title, body)
    SELECT 'event', new.id, new.title, new.location || ' ' || new.notes WHERE new.deleted_at IS NULL;
END;
CREATE TRIGGER search_events_delete AFTER DELETE ON events BEGIN
  DELETE FROM search WHERE kind = 'event' AND ref_id = old.id;
END;

CREATE TRIGGER search_journal_insert AFTER INSERT ON journal WHEN new.deleted_at IS NULL BEGIN
  INSERT INTO search (kind, ref_id, title, body) VALUES ('journal', new.id, new.text, '');
END;
CREATE TRIGGER search_journal_update AFTER UPDATE OF text, deleted_at ON journal BEGIN
  DELETE FROM search WHERE kind = 'journal' AND ref_id = old.id;
  INSERT INTO search (kind, ref_id, title, body)
    SELECT 'journal', new.id, new.text, '' WHERE new.deleted_at IS NULL;
END;
CREATE TRIGGER search_journal_delete AFTER DELETE ON journal BEGIN
  DELETE FROM search WHERE kind = 'journal' AND ref_id = old.id;
END;

-- What existed before this migration.
INSERT INTO search (kind, ref_id, title, body) SELECT 'task', id, title, notes FROM tasks WHERE deleted_at IS NULL;
INSERT INTO search (kind, ref_id, title, body) SELECT 'project', id, name, description FROM projects WHERE deleted_at IS NULL;
INSERT INTO search (kind, ref_id, title, body) SELECT 'file', id, display_name, note FROM files WHERE deleted_at IS NULL;
INSERT INTO search (kind, ref_id, title, body) SELECT 'collection', id, name, '' FROM collections WHERE deleted_at IS NULL;
INSERT INTO search (kind, ref_id, title, body) SELECT 'learning_item', id, title, notes FROM learning_items WHERE deleted_at IS NULL;
INSERT INTO search (kind, ref_id, title, body) SELECT 'event', id, title, location || ' ' || notes FROM events WHERE deleted_at IS NULL;
INSERT INTO search (kind, ref_id, title, body) SELECT 'journal', id, text, '' FROM journal WHERE deleted_at IS NULL;
