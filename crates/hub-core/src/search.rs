//! Search across Hub (the palette, Ctrl+K): tasks, projects, files and Koleksi, learning items,
//! events, and notes. The index is the `search` FTS5 table, kept in step by triggers
//! (migrations/0003). File contents are not indexed (CONTEXT.md, "Search").

use rusqlite::{params, Connection};
use serde::Serialize;
use ts_rs::TS;

use crate::error::Result;
use crate::model::Pigment;

/// What a hit is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SearchKind {
    Task,
    Project,
    File,
    Collection,
    LearningItem,
    Event,
    Journal,
}

impl SearchKind {
    fn parse(text: &str) -> Option<SearchKind> {
        Some(match text {
            "task" => SearchKind::Task,
            "project" => SearchKind::Project,
            "file" => SearchKind::File,
            "collection" => SearchKind::Collection,
            "learning_item" => SearchKind::LearningItem,
            "event" => SearchKind::Event,
            "journal" => SearchKind::Journal,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchHit {
    pub kind: SearchKind,
    pub id: String,
    pub title: String,
    /// One quiet line under the title: the project, the folder, the track, the date.
    pub detail: String,
    pub pigment: Option<Pigment>,
    /// For a task: its project, so the palette can say where it lives.
    pub project_id: Option<String>,
}

const QUERY_MAX: usize = 100;
/// Trigram tokens need three characters; shorter words are matched with LIKE.
const TRIGRAM: usize = 3;

fn like_pattern(word: &str) -> String {
    let escaped = word.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{escaped}%")
}

/// Hits for what the owner typed, best first. An empty query finds nothing.
pub fn search(conn: &Connection, query: &str, limit: u32) -> Result<Vec<SearchHit>> {
    let query: String = query.trim().chars().take(QUERY_MAX).collect();
    let words: Vec<&str> = query.split_whitespace().take(8).collect();
    if words.is_empty() {
        return Ok(Vec::new());
    }
    let limit = limit.clamp(1, 50);
    let long: Vec<&&str> = words.iter().filter(|w| w.chars().count() >= TRIGRAM).collect();
    let short: Vec<String> = words.iter().filter(|w| w.chars().count() < TRIGRAM).map(|w| like_pattern(w)).collect();

    // Rows from the index: MATCH on the long words (each a quoted phrase, so no FTS syntax gets
    // through), LIKE on the short ones. A title that contains the query ranks first.
    let shorts = serde_json::to_string(&short)?;
    let rows: Vec<(String, String, String)> = if long.is_empty() {
        let mut statement = conn.prepare(
            "SELECT kind, ref_id, title FROM search
             WHERE NOT EXISTS (SELECT 1 FROM json_each(?1) w WHERE (title || ' ' || body) NOT LIKE w.value ESCAPE '\\')
             ORDER BY instr(lower(title), lower(?2)) = 0, length(title) LIMIT ?3",
        )?;
        statement
            .query_map(params![shorts, query, limit * 2], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?
    } else {
        let phrase = long.iter().map(|w| format!("\"{}\"", w.replace('"', "\"\""))).collect::<Vec<_>>().join(" ");
        let mut statement = conn.prepare(
            "SELECT kind, ref_id, title FROM search
             WHERE search MATCH ?1
               AND NOT EXISTS (SELECT 1 FROM json_each(?2) w WHERE (title || ' ' || body) NOT LIKE w.value ESCAPE '\\')
             ORDER BY instr(lower(title), lower(?3)) = 0, bm25(search, 0, 0, 10.0, 1.0) LIMIT ?4",
        )?;
        statement
            .query_map(params![phrase, shorts, query, limit * 2], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?
    };

    let mut hits = Vec::new();
    for (kind, id, title) in rows {
        let Some(kind) = SearchKind::parse(&kind) else { continue };
        if let Some(hit) = describe(conn, kind, id, title)? {
            hits.push(hit);
        }
        if hits.len() as u32 >= limit {
            break;
        }
    }
    Ok(hits)
}

/// The detail line for a hit, read from the live row (a hit whose row is gone is skipped).
fn describe(conn: &Connection, kind: SearchKind, id: String, title: String) -> Result<Option<SearchHit>> {
    let found = match kind {
        SearchKind::Task => conn.query_row(
            "SELECT coalesce(p.name, ''), p.pigment, t.project_id FROM tasks t
             LEFT JOIN projects p ON p.id = t.project_id WHERE t.id = ?1 AND t.deleted_at IS NULL",
            [&id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<Pigment>>(1)?, row.get::<_, Option<String>>(2)?)),
        ),
        SearchKind::Project => conn.query_row(
            "SELECT coalesce(a.name, ''), p.pigment, NULL FROM projects p LEFT JOIN areas a ON a.id = p.area_id
             WHERE p.id = ?1 AND p.deleted_at IS NULL",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ),
        SearchKind::File => conn.query_row("SELECT path, NULL, NULL FROM files WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        }),
        SearchKind::Collection => conn.query_row(
            "SELECT '', pigment, NULL FROM collections WHERE id = ?1 AND deleted_at IS NULL",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ),
        SearchKind::LearningItem => conn.query_row(
            "SELECT t.name, t.pigment, NULL FROM learning_items i JOIN tracks t ON t.id = i.track_id
             WHERE i.id = ?1 AND i.deleted_at IS NULL",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ),
        SearchKind::Event => conn.query_row(
            "SELECT coalesce(start_date, substr(start_at, 1, 10)), pigment, NULL FROM events WHERE id = ?1 AND deleted_at IS NULL",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ),
        SearchKind::Journal => conn.query_row("SELECT date, NULL, NULL FROM journal WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        }),
    };
    match found {
        Ok((detail, pigment, project_id)) => Ok(Some(SearchHit { kind, id, title, detail, pigment, project_id })),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::model::Actor;
    use crate::tasks::{self, TaskInput};
    use jiff::Timestamp;

    fn now() -> Timestamp {
        "2026-10-23T05:00:00Z".parse().unwrap()
    }

    fn task(conn: &mut Connection, title: &str, notes: &str) -> String {
        tasks::create(conn, TaskInput { title: title.into(), notes: notes.into(), ..Default::default() }, Actor::Owner, now()).unwrap().0
    }

    #[test]
    fn finds_words_inside_titles_and_notes_without_case_or_accents() {
        let mut conn = open_in_memory().unwrap();
        let nmap = task(&mut conn, "THM: Nmap Live Host Discovery", "");
        task(&mut conn, "Laporan VLAN", "pakai trunk dan nmap");
        task(&mut conn, "Résumé café", "");
        let hits = search(&conn, "NMAP", 10).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, nmap, "a title match ranks first");
        assert_eq!(search(&conn, "resume", 10).unwrap().len(), 1);
        assert_eq!(search(&conn, "vlan trunk", 10).unwrap().len(), 1);
    }

    #[test]
    fn short_words_and_odd_characters_are_safe() {
        let mut conn = open_in_memory().unwrap();
        task(&mut conn, "Soal no 5", "");
        task(&mut conn, "100% selesai", "");
        assert_eq!(search(&conn, "no", 10).unwrap().len(), 1);
        assert_eq!(search(&conn, "5", 10).unwrap().len(), 1);
        assert_eq!(search(&conn, "%", 10).unwrap().len(), 1);
        assert!(search(&conn, "\"OR*( NEAR", 10).unwrap().is_empty());
        assert!(search(&conn, "   ", 10).unwrap().is_empty());
    }

    #[test]
    fn deleted_and_edited_rows_follow_the_index() {
        let mut conn = open_in_memory().unwrap();
        let id = task(&mut conn, "Ganti password router", "");
        tasks::delete(&mut conn, &id, Actor::Owner, now()).unwrap();
        assert!(search(&conn, "router", 10).unwrap().is_empty());
        tasks::restore(&mut conn, &id, Actor::Owner, now()).unwrap();
        assert_eq!(search(&conn, "router", 10).unwrap().len(), 1);
    }
}
