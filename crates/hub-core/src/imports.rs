//! Rows brought in from elsewhere (ULTRON's old database), remembered by the reference they had
//! there ("ultron:task:12"). Asking again for a reference already imported returns the row made
//! the first time, so an import that stops halfway can simply run again.

use jiff::Timestamp;
use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{invalid, Reason, Result};
use crate::model::Entity;
use crate::time::instant;

const REF_MAX: usize = 100;

/// Check the shape of a reference: short, printable, no spaces.
pub fn check(reference: &str) -> Result<()> {
    if reference.is_empty() || reference.chars().count() > REF_MAX || !reference.chars().all(|c| c.is_ascii_graphic()) {
        return invalid("importRef", Reason::Format);
    }
    Ok(())
}

/// The id of the row made for `reference` earlier, if it still points at that kind of thing.
pub fn find(conn: &Connection, reference: &str, entity: Entity) -> Result<Option<String>> {
    check(reference)?;
    Ok(conn
        .query_row("SELECT entity_id FROM imports WHERE ref = ?1 AND entity_type = ?2", params![reference, entity], |row| row.get(0))
        .optional()?)
}

/// Remember that `reference` became `id`.
pub fn remember(conn: &Connection, reference: &str, entity: Entity, id: &str, now: Timestamp) -> Result<()> {
    check(reference)?;
    conn.execute(
        "INSERT INTO imports (ref, entity_type, entity_id, at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (ref) DO UPDATE SET entity_type = excluded.entity_type, entity_id = excluded.entity_id, at = excluded.at",
        params![reference, entity, id, instant(now)],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn a_reference_is_remembered_once() {
        let conn = open_in_memory().unwrap();
        let now: Timestamp = "2026-10-23T14:00:00Z".parse().unwrap();
        assert_eq!(find(&conn, "ultron:task:1", Entity::Task).unwrap(), None);
        remember(&conn, "ultron:task:1", Entity::Task, "abc", now).unwrap();
        assert_eq!(find(&conn, "ultron:task:1", Entity::Task).unwrap().as_deref(), Some("abc"));
        assert_eq!(find(&conn, "ultron:task:1", Entity::Reminder).unwrap(), None);
        assert!(find(&conn, "has space", Entity::Task).is_err());
        assert!(find(&conn, "", Entity::Task).is_err());
    }
}
