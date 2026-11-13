//! Opening the database and moving its schema forward.

use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

use crate::error::{Error, Result};

/// Every migration, in order. Version N of the schema means the first N have run. Append only:
/// a released migration is never edited (see migrations/0001_init.sql).
pub const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_productivity.sql"),
    include_str!("../migrations/0003_library_learning.sql"),
    include_str!("../migrations/0004_api.sql"),
    include_str!("../migrations/0005_studio.sql"),
    include_str!("../migrations/0006_ledger_area.sql"),
    include_str!("../migrations/0007_studio_stack.sql"),
    include_str!("../migrations/0008_claude_sessions.sql"),
    include_str!("../migrations/0009_milestone_tasks.sql"),
];

pub fn latest_version() -> i64 {
    MIGRATIONS.len() as i64
}

/// Open (or create) the database at `path` and bring its schema up to date.
pub fn open(path: &Path) -> Result<Connection> {
    let mut conn = Connection::open(path)?;
    configure(&conn)?;
    migrate(&mut conn)?;
    Ok(conn)
}

/// [`open`], but a database about to be migrated is first copied whole into `backups`
/// (`hub-before-schema-N.db`, N the version it is moved to), so an upgrade can always be undone by
/// hand. A new, empty database needs no copy. The daily backups ignore these files.
pub fn open_with_backup(path: &Path, backups: &Path) -> Result<Connection> {
    let mut conn = Connection::open(path)?;
    configure(&conn)?;
    let found = schema_version(&conn)?;
    let known = latest_version();
    if found > 0 && found < known {
        std::fs::create_dir_all(backups)?;
        let target = backups.join(format!("hub-before-schema-{known}.db"));
        if !target.exists() {
            let partial = backups.join(format!("hub-before-schema-{known}.db.partial"));
            if partial.exists() {
                std::fs::remove_file(&partial)?;
            }
            conn.execute("VACUUM INTO ?1", [partial.to_string_lossy()])?;
            std::fs::rename(&partial, &target)?;
        }
    }
    migrate(&mut conn)?;
    Ok(conn)
}

/// A fresh database in memory, for tests.
pub fn open_in_memory() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrate(&mut conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<()> {
    // WAL lets a backup or a reader run beside a write; in memory it stays "memory".
    conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(Duration::from_secs(5))?;
    Ok(())
}

pub fn schema_version(conn: &Connection) -> Result<i64> {
    Ok(conn.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

/// Run the migrations this database has not seen, each in its own transaction. A database from a
/// newer Hub is refused untouched.
fn migrate(conn: &mut Connection) -> Result<()> {
    migrate_until(conn, latest_version())
}

/// Run the migrations up to version `target` (tests build older databases with it).
fn migrate_until(conn: &mut Connection, target: i64) -> Result<()> {
    let found = schema_version(conn)?;
    let known = latest_version();
    if found > known {
        return Err(Error::NewerDatabase { found, known });
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().take(target as usize).skip(found as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", index as i64 + 1)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_database_gets_the_latest_schema() {
        let conn = open_in_memory().unwrap();
        assert_eq!(schema_version(&conn).unwrap(), latest_version());
        let tables: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'tasks'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables, 1);
    }

    #[test]
    fn reopening_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hub.db");
        drop(open(&path).unwrap());
        let conn = open(&path).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), latest_version());
        let mode: String = conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }

    #[test]
    fn a_newer_database_is_refused_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hub.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "user_version", latest_version() + 5)
                .unwrap();
        }
        match open(&path) {
            Err(Error::NewerDatabase { found, known }) => {
                assert_eq!(found, latest_version() + 5);
                assert_eq!(known, latest_version());
            }
            other => panic!("expected NewerDatabase, got {other:?}"),
        }
        let conn = Connection::open(&path).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), latest_version() + 5);
    }

    #[test]
    fn activity_is_append_only() {
        let conn = open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO activity (id, at, actor, verb, entity_type, summary) VALUES ('a', '2026-10-22T00:00:00Z', 'owner', 'created', 'task', 'x')",
            [],
        )
        .unwrap();
        assert!(conn.execute("UPDATE activity SET summary = 'y'", []).is_err());
        assert!(conn.execute("DELETE FROM activity", []).is_err());
    }

    /// A database file left at schema `version`, with some history in it.
    fn old_database(path: &Path, version: i64) {
        let mut conn = Connection::open(path).unwrap();
        configure(&conn).unwrap();
        migrate_until(&mut conn, version).unwrap();
        for (index, actor) in ["owner", "ultron", "hub"].iter().enumerate() {
            conn.execute(
                "INSERT INTO activity (id, at, actor, verb, entity_type, entity_id, summary, details)
                 VALUES (?1, ?2, ?3, 'created', 'task', 't', ?4, '{\"n\":1}')",
                rusqlite::params![format!("a{index}"), format!("2026-10-2{index}T08:00:00Z"), actor, format!("row {index}")],
            )
            .unwrap();
        }
    }

    type ActivityRow = (String, String, String, String, String, Option<String>, String, String);

    fn activity_rows(conn: &Connection) -> Vec<ActivityRow> {
        let mut statement = conn
            .prepare("SELECT id, at, actor, verb, entity_type, entity_id, summary, details FROM activity ORDER BY id")
            .unwrap();
        statement
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    }

    #[test]
    fn the_studio_upgrade_keeps_every_activity_row_and_the_append_only_rule() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hub.db");
        old_database(&path, 4);
        let before = activity_rows(&Connection::open(&path).unwrap());
        assert_eq!(before.len(), 3);

        let conn = open(&path).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), latest_version());
        assert_eq!(activity_rows(&conn), before, "every row, every column");
        // The new actors are accepted, others still refused, and the history still cannot change.
        for actor in ["claude", "gemini"] {
            conn.execute(
                "INSERT INTO activity (id, at, actor, verb, entity_type, summary) VALUES (?1, '2026-11-06T00:00:00Z', ?1, 'created', 'studio_project', 'x')",
                [actor],
            )
            .unwrap();
        }
        assert!(conn
            .execute("INSERT INTO activity (id, at, actor, verb, entity_type, summary) VALUES ('z', '2026-11-06T00:00:00Z', 'someone', 'created', 'task', 'x')", [])
            .is_err());
        assert!(conn.execute("UPDATE activity SET summary = 'y'", []).is_err());
        assert!(conn.execute("DELETE FROM activity WHERE id = 'a0'", []).is_err());
        let indexes: i64 = conn
            .query_row("SELECT count(*) FROM sqlite_master WHERE type = 'index' AND name IN ('activity_at', 'activity_entity')", [], |row| row.get(0))
            .unwrap();
        assert_eq!(indexes, 2);
    }

    #[test]
    fn the_ledger_area_upgrade_keeps_every_ledger_row() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hub.db");
        old_database(&path, 5);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "INSERT INTO studio_projects (id, name, created_at, updated_at) VALUES ('p', 'Ide', 't', 't');
                 INSERT INTO studio_ledger (id, studio_project_id, kind, topic, statement, stated_by, created_at, updated_at)
                   VALUES ('l1', 'p', 'decision', 'Platform', 'Web', 'owner', 't', 't'),
                          ('l2', 'p', 'assumption', 'Pengguna', 'Siswa', 'gemini', 't', 't');",
            )
            .unwrap();
        }
        let conn = open(&path).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), latest_version());
        let rows: Vec<(String, String, Option<String>)> = conn
            .prepare("SELECT id, statement, area FROM studio_ledger ORDER BY id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(rows, [("l1".into(), "Web".into(), None), ("l2".into(), "Siswa".into(), None)], "every row kept, no area yet");
        assert_eq!(activity_rows(&conn).len(), 3);
        conn.execute("UPDATE studio_ledger SET area = 'users' WHERE id = 'l2'", []).unwrap();
        assert!(conn.execute("UPDATE studio_ledger SET area = 'mood' WHERE id = 'l1'", []).is_err(), "only the five parts");
    }

    #[test]
    fn the_stack_upgrade_adds_its_table_and_loses_no_row() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hub.db");
        old_database(&path, 6);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "INSERT INTO studio_projects (id, name, created_at, updated_at) VALUES ('p', 'Ide', 't', 't');
                 INSERT INTO studio_ledger (id, studio_project_id, kind, topic, statement, stated_by, created_at, updated_at, area)
                   VALUES ('l1', 'p', 'decision', 'Platform', 'Web', 'owner', 't', 't', 'constraints');",
            )
            .unwrap();
        }
        let conn = open(&path).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), latest_version());
        let count = |table: &str| -> i64 { conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row.get(0)).unwrap() };
        assert_eq!((count("studio_projects"), count("studio_ledger"), count("activity")), (1, 1, 3));
        assert_eq!(count("studio_stacks"), 0);
        // Approved needs its time, and only the two states exist.
        assert!(conn
            .execute("INSERT INTO studio_stacks (studio_project_id, mode, body, status, created_at, updated_at) VALUES ('p', 'own', '{}', 'approved', 't', 't')", [])
            .is_err());
        conn.execute("INSERT INTO studio_stacks (studio_project_id, mode, body, created_at, updated_at) VALUES ('p', 'own', '{}', 't', 't')", []).unwrap();
    }

    #[test]
    fn the_claude_sessions_upgrade_adds_its_table_and_loses_no_row() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hub.db");
        old_database(&path, 7);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "INSERT INTO studio_projects (id, name, repo_path, created_at, updated_at) VALUES ('p', 'Ide', 'D:\repo', 't', 't');
                 INSERT INTO studio_stacks (studio_project_id, mode, body, created_at, updated_at) VALUES ('p', 'own', '{}', 't', 't');",
            )
            .unwrap();
        }
        let conn = open(&path).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), latest_version());
        let count = |table: &str| -> i64 { conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row.get(0)).unwrap() };
        assert_eq!((count("studio_projects"), count("studio_stacks"), count("activity")), (1, 1, 3));
        assert_eq!(count("studio_claude_sessions"), 0);
        // A tool needs its time and its outcome.
        assert!(conn
            .execute("INSERT INTO studio_claude_sessions (id, folder, started_at, seen_at, tool) VALUES ('s', 'D:\repo', 't', 't', 'hub_status')", [])
            .is_err());
        conn.execute(
            "INSERT INTO studio_claude_sessions (id, folder, studio_project_id, started_at, seen_at, tool, tool_at, tool_ok) VALUES ('s', 'D:\repo', 'p', 't', 't', 'hub_status', 't', 1)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn the_milestone_tasks_upgrade_keeps_projects_tasks_and_ticks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hub.db");
        old_database(&path, 8);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "INSERT INTO projects (id, name, progress_mode, order_key, created_at, updated_at) VALUES ('p', 'PSAJ', 'milestones', 'a', 't', 't');
                 INSERT INTO milestones (id, project_id, title, done_at, order_key, created_at, updated_at)
                   VALUES ('m', 'p', 'PPT pitching', 't', 'a', 't', 't');
                 INSERT INTO tasks (id, title, project_id, order_key, created_at, updated_at) VALUES ('t', 'Laporan', 'p', 'a', 't', 't');",
            )
            .unwrap();
        }
        let conn = open(&path).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), latest_version());
        let count = |table: &str| -> i64 { conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row.get(0)).unwrap() };
        assert_eq!((count("projects"), count("milestones"), count("tasks"), count("activity")), (1, 1, 1, 3));
        let task: (String, Option<String>) = conn.query_row("SELECT title, milestone_id FROM tasks", [], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
        assert_eq!(task, ("Laporan".into(), None), "existing tasks start without a milestone");
        let ticked: Option<String> = conn.query_row("SELECT done_at FROM milestones", [], |row| row.get(0)).unwrap();
        assert_eq!(ticked.as_deref(), Some("t"), "existing ticks stay");
        assert!(conn.query_row("SELECT progress_mode FROM projects", [], |row| row.get::<_, String>(0)).is_err(), "the unused column is gone");
        conn.execute("UPDATE tasks SET milestone_id = 'm'", []).unwrap();
        assert!(conn.execute("UPDATE tasks SET milestone_id = 'missing'", []).is_err(), "only a real milestone");
    }

    #[test]
    fn an_upgrade_is_preceded_by_a_whole_copy_and_a_new_database_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let backups = dir.path().join("backups");

        let fresh = dir.path().join("fresh.db");
        drop(open_with_backup(&fresh, &backups).unwrap());
        assert!(!backups.exists(), "nothing to keep from an empty database");

        let path = dir.path().join("hub.db");
        old_database(&path, 4);
        drop(open_with_backup(&path, &backups).unwrap());
        let copy = backups.join(format!("hub-before-schema-{}.db", latest_version()));
        let copy = Connection::open(copy).unwrap();
        assert_eq!(schema_version(&copy).unwrap(), 4, "the copy is the database as it was");
        assert_eq!(activity_rows(&copy).len(), 3);
        // Opening again migrates nothing and copies nothing new.
        drop(open_with_backup(&path, &backups).unwrap());
        assert_eq!(std::fs::read_dir(&backups).unwrap().count(), 1);
    }

    #[test]
    fn a_saved_prd_version_and_a_report_do_not_change() {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO studio_projects (id, name, created_at, updated_at) VALUES ('p', 'Ide', 't', 't');
             INSERT INTO studio_prd_versions (id, studio_project_id, number, status, body, made_by, created_at, updated_at)
               VALUES ('d', 'p', NULL, 'draft', '{}', 'gemini', 't', 't'),
                      ('v1', 'p', 1, 'saved', '{}', 'gemini', 't', 't');",
        )
        .unwrap();
        conn.execute("UPDATE studio_prd_versions SET body = '{\"a\":1}' WHERE id = 'd'", []).unwrap();
        assert!(conn.execute("UPDATE studio_prd_versions SET body = '{\"a\":1}' WHERE id = 'v1'", []).is_err());
        conn.execute("UPDATE studio_prd_versions SET status = 'approved', approved_at = 't' WHERE id = 'v1'", []).unwrap();
        assert!(conn
            .execute("INSERT INTO studio_prd_versions (id, studio_project_id, number, status, body, made_by, created_at, updated_at) VALUES ('d2', 'p', NULL, 'draft', '{}', 'owner', 't', 't')", [])
            .is_err(), "one working draft per project");
        conn.execute_batch(
            "INSERT INTO studio_roadmaps (id, studio_project_id, prd_version_id, made_by, created_at, updated_at) VALUES ('r', 'p', 'v1', 'gemini', 't', 't');
             INSERT INTO studio_checkpoints (id, roadmap_id, code, position, title, created_at, updated_at) VALUES ('c', 'r', 'CP-01', 1, 'Fondasi', 't', 't');
             INSERT INTO studio_reports (id, checkpoint_id, kind, summary, body, made_by, created_at) VALUES ('rep', 'c', 'submission', 'ok', '{}', 'claude', 't');",
        )
        .unwrap();
        assert!(conn.execute("UPDATE studio_reports SET summary = 'lain'", []).is_err());
        assert!(conn
            .execute("INSERT INTO studio_checkpoints (id, roadmap_id, code, position, title, created_at, updated_at) VALUES ('c2', 'r', 'X-1', 2, 'Salah', 't', 't')", [])
            .is_err(), "checkpoint codes look like CP-01");
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let conn = open_in_memory().unwrap();
        let result = conn.execute(
            "INSERT INTO focus (date, slot, task_id) VALUES ('2026-10-22', 1, 'missing')",
            [],
        );
        assert!(result.is_err());
    }
}
