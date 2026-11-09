//! Ruang: life areas that group projects (Sekolah, Belajar Cyber, ...).

use jiff::Timestamp;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::activity::{self, Record};
use crate::error::{invalid, Error, Reason, Result};
use crate::ids::{self, new_id};
use crate::model::{Actor, Change, Entity, Pigment, Verb};
use crate::order;
use crate::settings;
use crate::time::instant;

const NAME_MAX: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Area {
    pub id: String,
    pub name: String,
    pub pigment: Pigment,
    pub open_projects: u32,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AreaInput {
    pub name: String,
    pub pigment: Pigment,
}

fn checked_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > NAME_MAX {
        return invalid("name", Reason::Length);
    }
    Ok(name.to_string())
}

pub fn list(conn: &Connection) -> Result<Vec<Area>> {
    let mut statement = conn.prepare(
        "SELECT a.id, a.name, a.pigment,
                (SELECT count(*) FROM projects p WHERE p.area_id = a.id AND p.deleted_at IS NULL
                   AND p.status IN ('active', 'paused'))
         FROM areas a WHERE a.deleted_at IS NULL ORDER BY a.order_key",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(Area {
            id: row.get(0)?,
            name: row.get(1)?,
            pigment: row.get(2)?,
            open_projects: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn create(conn: &mut Connection, input: AreaInput, actor: Actor, now: Timestamp) -> Result<(String, Change)> {
    let name = checked_name(&input.name)?;
    let tx = conn.transaction()?;
    let last: Option<String> = tx.query_row("SELECT max(order_key) FROM areas", [], |row| row.get(0))?;
    let id = new_id();
    let stamp = instant(now);
    tx.execute(
        "INSERT INTO areas (id, name, pigment, order_key, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![id, name, input.pigment, order::after(last.as_deref()), stamp],
    )?;
    activity::record(
        &tx,
        now,
        Record { actor, verb: Verb::Created, entity: Entity::Area, entity_id: Some(&id), summary: &name, details: serde_json::json!({}) },
    )?;
    tx.commit()?;
    Ok((id.clone(), Change::one(Entity::Area, id)))
}

pub fn update(conn: &mut Connection, id: &str, input: AreaInput, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let name = checked_name(&input.name)?;
    let tx = conn.transaction()?;
    let changed = tx.execute(
        "UPDATE areas SET name = ?2, pigment = ?3, updated_at = ?4 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, name, input.pigment, instant(now)],
    )?;
    if changed == 0 {
        return Err(Error::NotFound("area"));
    }
    activity::record(
        &tx,
        now,
        Record { actor, verb: Verb::Updated, entity: Entity::Area, entity_id: Some(&id), summary: &name, details: serde_json::json!({}) },
    )?;
    tx.commit()?;
    Ok(Change::one(Entity::Area, id))
}

/// Remove an area. Its projects stay, without an area.
pub fn delete(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let name: Option<String> = tx
        .query_row("SELECT name FROM areas WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| row.get(0))
        .optional()?;
    let name = name.ok_or(Error::NotFound("area"))?;
    let stamp = instant(now);
    tx.execute("UPDATE areas SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1", params![id, stamp])?;
    tx.execute("UPDATE projects SET area_id = NULL, updated_at = ?2 WHERE area_id = ?1", params![id, stamp])?;
    tx.execute("UPDATE tasks SET area_id = NULL, updated_at = ?2 WHERE area_id = ?1", params![id, stamp])?;
    activity::record(
        &tx,
        now,
        Record { actor, verb: Verb::Deleted, entity: Entity::Area, entity_id: Some(&id), summary: &name, details: serde_json::json!({}) },
    )?;
    tx.commit()?;
    Ok(Change::all(Entity::Area))
}

/// The four areas the owner chose to start with (CONTEXT.md), made once.
pub fn ensure_defaults(conn: &mut Connection, now: Timestamp) -> Result<bool> {
    if settings::is_stored(conn, "areas_seeded")? {
        return Ok(false);
    }
    let has_areas: i64 = conn.query_row("SELECT count(*) FROM areas", [], |row| row.get(0))?;
    if has_areas == 0 {
        for (name, pigment) in [
            ("Sekolah", Pigment::Lazuardi),
            ("Belajar Cyber", Pigment::Lavender),
            ("Pribadi", Pigment::Lumut),
            ("Proyek Pribadi", Pigment::Kunyit),
        ] {
            create(conn, AreaInput { name: name.into(), pigment }, Actor::Hub, now)?;
        }
    }
    settings::store_initial(conn, "areas_seeded", serde_json::json!(true), now)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    fn now() -> Timestamp {
        "2026-10-23T03:00:00Z".parse().unwrap()
    }

    #[test]
    fn four_areas_once() {
        let mut conn = open_in_memory().unwrap();
        assert!(ensure_defaults(&mut conn, now()).unwrap());
        assert!(!ensure_defaults(&mut conn, now()).unwrap());
        let names: Vec<String> = list(&conn).unwrap().into_iter().map(|a| a.name).collect();
        assert_eq!(names, ["Sekolah", "Belajar Cyber", "Pribadi", "Proyek Pribadi"]);
    }

    #[test]
    fn removing_an_area_keeps_its_projects() {
        let mut conn = open_in_memory().unwrap();
        let (area, _) = create(&mut conn, AreaInput { name: "Sekolah".into(), pigment: Pigment::Lazuardi }, Actor::Owner, now()).unwrap();
        let (project, _) = crate::projects::create(
            &mut conn,
            crate::projects::ProjectInput { name: "Laporan Lab".into(), area_id: Some(area.clone()), ..Default::default() },
            Actor::Owner,
            now(),
        )
        .unwrap();
        delete(&mut conn, &area, Actor::Owner, now()).unwrap();
        assert!(list(&conn).unwrap().is_empty());
        let area_id: Option<String> =
            conn.query_row("SELECT area_id FROM projects WHERE id = ?1", [&project], |row| row.get(0)).unwrap();
        assert_eq!(area_id, None);
        assert!(update(&mut conn, &area, AreaInput { name: "X".into(), pigment: Pigment::Batu }, Actor::Owner, now()).is_err());
    }
}
