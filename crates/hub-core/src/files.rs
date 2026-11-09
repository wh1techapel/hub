//! The Library: files Hub knows by reference (their path and their identity on the volume), what
//! they are attached to, their Koleksi, Vault copies, and whether they are still where they were.
//!
//! Hub never moves, renames, or deletes the owner's files. "Hapus" here forgets the reference.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use jiff::Timestamp;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::activity::{self, Record};
use crate::error::{invalid, Error, Reason, Result};
use crate::hub::Paths;
use crate::identity::{Finder, Identity, Probe};
use crate::ids::{self, new_id};
use crate::model::{Actor, Change, Entity, FileKind, FileState, LinkKind, Pigment, Source, Verb};
use crate::order;
use crate::time::instant;
use crate::vault;

const NOTE_MAX: usize = 4000;
const NAME_MAX: usize = 260;

/// Extensions by kind. Anything else is "other".
const KINDS: &[(FileKind, &[&str])] = &[
    (FileKind::Document, &["pdf", "doc", "docx", "odt", "rtf", "txt", "md", "ppt", "pptx", "odp", "xls", "xlsx", "ods", "csv", "epub"]),
    (FileKind::Image, &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "heic", "ico", "tif", "tiff"]),
    (
        FileKind::Code,
        &[
            "rs", "py", "js", "ts", "tsx", "jsx", "html", "css", "json", "yaml", "yml", "toml", "c", "cpp", "h", "hpp", "cs", "java",
            "go", "sh", "ps1", "sql", "php", "rb", "ipynb", "xml", "conf", "ini", "cfg", "lua", "pcap", "pcapng", "ovpn",
        ],
    ),
    (FileKind::Archive, &["zip", "rar", "7z", "tar", "gz", "tgz", "xz", "bz2", "iso", "ova", "ovf", "vmdk", "vdi", "qcow2", "img"]),
    (FileKind::Media, &["mp3", "wav", "flac", "m4a", "ogg", "aac", "mp4", "mkv", "avi", "mov", "webm", "wmv"]),
];

/// Files Hub opens with Explorer selection instead of starting them: programs, scripts, links.
const PROGRAMS: &[&str] = &[
    "exe", "msi", "msix", "bat", "cmd", "com", "scr", "pif", "cpl", "ps1", "psm1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "hta",
    "reg", "lnk", "url", "appref-ms", "jar", "msc", "application", "gadget", "inf", "sys", "dll",
];

pub fn kind_of(extension: &str, is_dir: bool) -> FileKind {
    if is_dir {
        return FileKind::Folder;
    }
    KINDS
        .iter()
        .find(|(_, extensions)| extensions.contains(&extension))
        .map(|(kind, _)| *kind)
        .unwrap_or(FileKind::Other)
}

/// Whether opening this would run something.
pub fn is_program(extension: &str) -> bool {
    PROGRAMS.contains(&extension)
}

fn extension_of(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Something a file is attached to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct LinkTarget {
    pub kind: LinkKind,
    pub id: String,
}

/// An attachment as the UI shows it: with the thing's name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LinkRef {
    pub kind: LinkKind,
    pub id: String,
    pub title: String,
    pub pigment: Option<Pigment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CollectionRef {
    pub id: String,
    pub name: String,
    pub pigment: Pigment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileCard {
    pub id: String,
    pub name: String,
    pub kind: FileKind,
    pub extension: String,
    #[ts(type = "number | null")]
    pub size_bytes: Option<u64>,
    pub path: String,
    pub state: FileState,
    /// A copy is in the Vault.
    pub vaulted: bool,
    /// Opening it would run a program: Hub shows it in Explorer instead.
    pub program: bool,
    pub note: String,
    pub added_at: String,
    pub source: Source,
    pub links: Vec<LinkRef>,
    pub collections: Vec<CollectionRef>,
}

/// Which files to list. Empty means all.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct FileQuery {
    #[ts(optional)]
    pub kind: Option<FileKind>,
    #[ts(optional)]
    pub collection_id: Option<String>,
    /// Files attached to the project or to one of its tasks.
    #[ts(optional)]
    pub project_id: Option<String>,
    #[ts(optional)]
    pub target: Option<LinkTarget>,
    #[ts(optional)]
    pub state: Option<FileState>,
    #[ts(optional)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct FileEdit {
    pub name: String,
    pub note: String,
}

/// What adding one path did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Added {
    pub id: String,
    pub name: String,
    /// It was already in the Library (found by its identity or path).
    pub existed: bool,
    /// It came from a removable drive and should get a Vault copy.
    pub wants_copy: bool,
}

const CARD_COLUMNS: &str =
    "f.id, f.display_name, f.kind, f.extension, f.size_bytes, f.path, f.state, f.vault_path IS NOT NULL, f.note, f.added_at, f.source";

fn card_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<FileCard> {
    let extension: String = row.get(3)?;
    let kind: String = row.get(2)?;
    Ok(FileCard {
        id: row.get(0)?,
        name: row.get(1)?,
        kind: FileKind::parse(&kind).unwrap_or(FileKind::Other),
        program: is_program(&extension),
        extension,
        size_bytes: row.get::<_, Option<i64>>(4)?.map(|n| n.max(0) as u64),
        path: row.get(5)?,
        state: row.get(6)?,
        vaulted: row.get(7)?,
        note: row.get(8)?,
        added_at: row.get(9)?,
        source: row.get(10)?,
        links: Vec::new(),
        collections: Vec::new(),
    })
}

/// The name and colour of what a link points at, or None when it is gone.
fn link_ref(conn: &Connection, kind: LinkKind, id: &str) -> Result<Option<LinkRef>> {
    let sql = match kind {
        LinkKind::Project => "SELECT name, pigment FROM projects WHERE id = ?1 AND deleted_at IS NULL",
        LinkKind::Task => {
            "SELECT t.title, p.pigment FROM tasks t LEFT JOIN projects p ON p.id = t.project_id
             WHERE t.id = ?1 AND t.deleted_at IS NULL"
        }
        LinkKind::LearningItem => {
            "SELECT i.title, tr.pigment FROM learning_items i JOIN tracks tr ON tr.id = i.track_id
             WHERE i.id = ?1 AND i.deleted_at IS NULL"
        }
        LinkKind::Event => "SELECT title, pigment FROM events WHERE id = ?1 AND deleted_at IS NULL",
        LinkKind::Journal => "SELECT text, NULL FROM journal WHERE id = ?1 AND deleted_at IS NULL",
    };
    Ok(conn
        .query_row(sql, [id], |row| {
            Ok(LinkRef { kind, id: id.to_string(), title: row.get(0)?, pigment: row.get(1)? })
        })
        .optional()?)
}

/// Fill in links and Koleksi for a page of cards.
fn decorate(conn: &Connection, cards: &mut [FileCard]) -> Result<()> {
    if cards.is_empty() {
        return Ok(());
    }
    let index: HashMap<String, usize> = cards.iter().enumerate().map(|(i, c)| (c.id.clone(), i)).collect();
    let ids = serde_json::to_string(&cards.iter().map(|c| &c.id).collect::<Vec<_>>())?;
    let mut links = conn.prepare(
        "SELECT file_id, target_type, target_id FROM file_links
         WHERE file_id IN (SELECT value FROM json_each(?1)) ORDER BY created_at",
    )?;
    let rows = links.query_map([&ids], |row| Ok((row.get::<_, String>(0)?, row.get::<_, LinkKind>(1)?, row.get::<_, String>(2)?)))?;
    for row in rows {
        let (file, kind, target) = row?;
        if let (Some(&i), Some(link)) = (index.get(&file), link_ref(conn, kind, &target)?) {
            cards[i].links.push(link);
        }
    }
    let mut collections = conn.prepare(
        "SELECT fc.file_id, c.id, c.name, c.pigment FROM file_collections fc
         JOIN collections c ON c.id = fc.collection_id AND c.deleted_at IS NULL
         WHERE fc.file_id IN (SELECT value FROM json_each(?1)) ORDER BY c.order_key",
    )?;
    let rows = collections.query_map([&ids], |row| {
        Ok((row.get::<_, String>(0)?, CollectionRef { id: row.get(1)?, name: row.get(2)?, pigment: row.get(3)? }))
    })?;
    for row in rows {
        let (file, collection) = row?;
        if let Some(&i) = index.get(&file) {
            cards[i].collections.push(collection);
        }
    }
    Ok(())
}

pub fn get(conn: &Connection, id: &str) -> Result<FileCard> {
    let id = ids::parse("id", id)?;
    let sql = format!("SELECT {CARD_COLUMNS} FROM files f WHERE f.id = ?1 AND f.deleted_at IS NULL");
    let card = conn.query_row(&sql, [&id], card_from).optional()?.ok_or(Error::NotFound("file"))?;
    let mut cards = [card];
    decorate(conn, &mut cards)?;
    let [card] = cards;
    Ok(card)
}

pub fn list(conn: &Connection, query: &FileQuery) -> Result<Vec<FileCard>> {
    let collection = query.collection_id.as_deref().map(|id| ids::parse("collectionId", id)).transpose()?;
    let project = query.project_id.as_deref().map(|id| ids::parse("projectId", id)).transpose()?;
    let target = query.target.as_ref().map(|t| ids::parse("target", &t.id).map(|id| (t.kind, id))).transpose()?;
    let limit = query.limit.unwrap_or(500).min(2000);
    let sql = format!(
        "SELECT {CARD_COLUMNS} FROM files f
         WHERE f.deleted_at IS NULL
           AND (?1 IS NULL OR f.kind = ?1)
           AND (?2 IS NULL OR f.state = ?2)
           AND (?3 IS NULL OR EXISTS (SELECT 1 FROM file_collections fc WHERE fc.file_id = f.id AND fc.collection_id = ?3))
           AND (?4 IS NULL OR EXISTS (
                 SELECT 1 FROM file_links l WHERE l.file_id = f.id AND (
                   (l.target_type = 'project' AND l.target_id = ?4)
                   OR (l.target_type = 'task' AND l.target_id IN (SELECT id FROM tasks WHERE project_id = ?4 AND deleted_at IS NULL)))))
           AND (?5 IS NULL OR EXISTS (SELECT 1 FROM file_links l WHERE l.file_id = f.id AND l.target_type = ?5 AND l.target_id = ?6))
         ORDER BY f.added_at DESC, f.id DESC
         LIMIT ?7"
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map(
        params![
            query.kind,
            query.state,
            collection,
            project,
            target.as_ref().map(|(kind, _)| *kind),
            target.as_ref().map(|(_, id)| id.clone()),
            limit
        ],
        card_from,
    )?;
    let mut cards = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    decorate(conn, &mut cards)?;
    Ok(cards)
}

/// How many files of each kind, for the Library's side list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileCounts {
    pub all: u32,
    pub missing: u32,
    pub kinds: Vec<KindCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KindCount {
    pub kind: FileKind,
    pub count: u32,
}

pub fn counts(conn: &Connection) -> Result<FileCounts> {
    let mut counts = FileCounts::default();
    let mut statement = conn.prepare("SELECT kind, count(*) FROM files WHERE deleted_at IS NULL GROUP BY kind")?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?)))?;
    let mut by_kind: HashMap<FileKind, u32> = HashMap::new();
    for row in rows {
        let (kind, count) = row?;
        *by_kind.entry(FileKind::parse(&kind).unwrap_or(FileKind::Other)).or_default() += count;
        counts.all += count;
    }
    counts.kinds = FileKind::ALL
        .iter()
        .map(|kind| KindCount { kind: *kind, count: by_kind.get(kind).copied().unwrap_or(0) })
        .collect();
    counts.missing = conn.query_row("SELECT count(*) FROM files WHERE deleted_at IS NULL AND state = 'missing'", [], |row| row.get(0))?;
    Ok(counts)
}

fn check_target(conn: &Connection, target: &LinkTarget) -> Result<LinkTarget> {
    let id = ids::parse("target", &target.id)?;
    if link_ref(conn, target.kind, &id)?.is_none() {
        return invalid("target", Reason::Unknown);
    }
    Ok(LinkTarget { kind: target.kind, id })
}

fn link(tx: &Transaction<'_>, file_id: &str, target: &LinkTarget, now: Timestamp) -> Result<bool> {
    let inserted = tx.execute(
        "INSERT OR IGNORE INTO file_links (file_id, target_type, target_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![file_id, target.kind, target.id, instant(now)],
    )?;
    Ok(inserted > 0)
}

fn record(tx: &Transaction<'_>, now: Timestamp, actor: Actor, verb: Verb, id: &str, name: &str, details: serde_json::Value) -> Result<()> {
    activity::record(tx, now, Record { actor, verb, entity: Entity::File, entity_id: Some(id), summary: name, details })
}

/// Keep a path in the Library (and attach it, if a target is given). A file already kept, found by
/// its identity or its path, is reused: its path is brought up to date and a forgotten one comes
/// back.
pub fn add(conn: &mut Connection, probe: &Probe, target: Option<&LinkTarget>, actor: Actor, now: Timestamp) -> Result<(Added, Vec<Change>)> {
    add_as(conn, probe, target, actor, Source::from(actor), now)
}

/// [`add`], with where a new row came from (`Source::Import` for ULTRON's old file references).
pub fn add_as(
    conn: &mut Connection,
    probe: &Probe,
    target: Option<&LinkTarget>,
    actor: Actor,
    source: Source,
    now: Timestamp,
) -> Result<(Added, Vec<Change>)> {
    let path = probe.path.to_string_lossy().into_owned();
    if path.is_empty() || path.chars().count() > 32_000 {
        return invalid("path", Reason::Length);
    }
    let target = target.map(|t| check_target(conn, t)).transpose()?;
    let name: String = name_of(&probe.path).chars().take(NAME_MAX).collect();
    let extension = if probe.is_dir { String::new() } else { extension_of(&probe.path) };
    let kind = kind_of(&extension, probe.is_dir);
    let stamp = instant(now);

    let tx = conn.transaction()?;
    let existing: Option<(String, bool)> = match &probe.identity {
        Some(identity) => tx
            .query_row(
                "SELECT id, deleted_at IS NOT NULL FROM files WHERE volume_serial = ?1 AND file_index = ?2
                 ORDER BY deleted_at IS NOT NULL, added_at DESC LIMIT 1",
                params![identity.volume as i64, identity.file],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?,
        None => None,
    };
    let existing = match existing {
        Some(found) => Some(found),
        None => tx
            .query_row(
                "SELECT id, deleted_at IS NOT NULL FROM files WHERE path = ?1 COLLATE NOCASE
                 ORDER BY deleted_at IS NOT NULL, added_at DESC LIMIT 1",
                [&path],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?,
    };

    let (id, existed) = match existing {
        Some((id, was_forgotten)) => {
            tx.execute(
                "UPDATE files SET path = ?2, size_bytes = ?3, state = 'ok', last_seen_at = ?4, updated_at = ?4, deleted_at = NULL,
                        volume_serial = coalesce(?5, volume_serial), file_index = coalesce(?6, file_index)
                 WHERE id = ?1",
                params![
                    id,
                    path,
                    probe.size.map(|s| s as i64),
                    stamp,
                    probe.identity.as_ref().map(|i| i.volume as i64),
                    probe.identity.as_ref().map(|i| i.file.clone())
                ],
            )?;
            if was_forgotten {
                record(&tx, now, actor, Verb::Restored, &id, &name, serde_json::json!({}))?;
            }
            (id, !was_forgotten)
        }
        None => {
            let id = new_id();
            tx.execute(
                "INSERT INTO files (id, display_name, kind, extension, size_bytes, path, volume_serial, file_index, state,
                                    added_at, last_seen_at, source, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'ok', ?9, ?9, ?10, ?9, ?9)",
                params![
                    id,
                    name,
                    kind,
                    extension,
                    probe.size.map(|s| s as i64),
                    path,
                    probe.identity.as_ref().map(|i| i.volume as i64),
                    probe.identity.as_ref().map(|i| i.file.clone()),
                    stamp,
                    source
                ],
            )?;
            record(&tx, now, actor, Verb::Created, &id, &name, serde_json::json!({ "kind": kind }))?;
            (id, false)
        }
    };
    let mut changes = vec![Change::one(Entity::File, id.clone())];
    if let Some(target) = &target
        && link(&tx, &id, target, now)?
    {
        record(&tx, now, actor, Verb::Attached, &id, &name, serde_json::json!({ "kind": target.kind, "id": target.id }))?;
        changes.push(Change::one(target_entity(target.kind), target.id.clone()));
    }
    let vaulted: bool = tx.query_row("SELECT vault_path IS NOT NULL FROM files WHERE id = ?1", [&id], |row| row.get(0))?;
    tx.commit()?;
    Ok((Added { id, name, existed, wants_copy: probe.removable && !probe.is_dir && !vaulted }, changes))
}

fn target_entity(kind: LinkKind) -> Entity {
    match kind {
        LinkKind::Project => Entity::Project,
        LinkKind::Task => Entity::Task,
        LinkKind::LearningItem => Entity::LearningItem,
        LinkKind::Event => Entity::Event,
        LinkKind::Journal => Entity::Journal,
    }
}

fn name_for(tx: &Connection, id: &str) -> Result<String> {
    tx.query_row("SELECT display_name FROM files WHERE id = ?1 AND deleted_at IS NULL", [id], |row| row.get(0))
        .optional()?
        .ok_or(Error::NotFound("file"))
}

pub fn attach(conn: &mut Connection, id: &str, target: &LinkTarget, actor: Actor, now: Timestamp) -> Result<Vec<Change>> {
    let id = ids::parse("id", id)?;
    let target = check_target(conn, target)?;
    let tx = conn.transaction()?;
    let name = name_for(&tx, &id)?;
    if !link(&tx, &id, &target, now)? {
        return Ok(Vec::new());
    }
    record(&tx, now, actor, Verb::Attached, &id, &name, serde_json::json!({ "kind": target.kind, "id": target.id }))?;
    tx.commit()?;
    Ok(vec![Change::one(Entity::File, id), Change::one(target_entity(target.kind), target.id)])
}

pub fn detach(conn: &mut Connection, id: &str, target: &LinkTarget, actor: Actor, now: Timestamp) -> Result<Vec<Change>> {
    let id = ids::parse("id", id)?;
    let target_id = ids::parse("target", &target.id)?;
    let tx = conn.transaction()?;
    let name = name_for(&tx, &id)?;
    let removed = tx.execute(
        "DELETE FROM file_links WHERE file_id = ?1 AND target_type = ?2 AND target_id = ?3",
        params![id, target.kind, target_id],
    )?;
    if removed == 0 {
        return Ok(Vec::new());
    }
    record(&tx, now, actor, Verb::Detached, &id, &name, serde_json::json!({ "kind": target.kind, "id": target_id }))?;
    tx.commit()?;
    Ok(vec![Change::one(Entity::File, id), Change::one(target_entity(target.kind), target_id)])
}

/// Rename in Hub (the file on disk keeps its name) and change the note.
pub fn update(conn: &mut Connection, id: &str, edit: FileEdit, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let name = edit.name.trim();
    if name.is_empty() || name.chars().count() > NAME_MAX {
        return invalid("name", Reason::Length);
    }
    let note = edit.note.trim();
    if note.chars().count() > NOTE_MAX {
        return invalid("note", Reason::Length);
    }
    let tx = conn.transaction()?;
    let changed = tx.execute(
        "UPDATE files SET display_name = ?2, note = ?3, updated_at = ?4 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, name, note, instant(now)],
    )?;
    if changed == 0 {
        return Err(Error::NotFound("file"));
    }
    record(&tx, now, actor, Verb::Updated, &id, name, serde_json::json!({}))?;
    tx.commit()?;
    Ok(Change::one(Entity::File, id))
}

/// Forget the reference. The file on disk is untouched; a Vault copy stays until the owner removes
/// it, so Urungkan brings everything back.
pub fn delete(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    set_deleted(conn, id, true, actor, now)
}

pub fn restore(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    set_deleted(conn, id, false, actor, now)
}

fn set_deleted(conn: &mut Connection, id: &str, deleted: bool, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let name: String = tx
        .query_row("SELECT display_name FROM files WHERE id = ?1", [&id], |row| row.get(0))
        .optional()?
        .ok_or(Error::NotFound("file"))?;
    let stamp = instant(now);
    let changed = if deleted {
        tx.execute("UPDATE files SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NULL", params![id, stamp])?
    } else {
        tx.execute("UPDATE files SET deleted_at = NULL, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NOT NULL", params![id, stamp])?
    };
    if changed > 0 {
        let verb = if deleted { Verb::Deleted } else { Verb::Restored };
        record(&tx, now, actor, verb, &id, &name, serde_json::json!({}))?;
    }
    tx.commit()?;
    Ok(Change::one(Entity::File, id))
}

/// Record a Vault copy made by `vault::copy_in`.
pub fn set_vault(conn: &mut Connection, id: &str, copy: &vault::Copy, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let name = name_for(&tx, &id)?;
    tx.execute(
        "UPDATE files SET vault_path = ?2, sha256 = ?3, updated_at = ?4 WHERE id = ?1",
        params![id, copy.relative, copy.sha256, instant(now)],
    )?;
    record(&tx, now, actor, Verb::Updated, &id, &name, serde_json::json!({ "vault": true }))?;
    tx.commit()?;
    Ok(Change::one(Entity::File, id))
}

/// Remove the Vault copy (from disk too) at the owner's request.
pub fn remove_vault(conn: &mut Connection, paths: &Paths, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let name = name_for(&tx, &id)?;
    let relative: Option<String> = tx.query_row("SELECT vault_path FROM files WHERE id = ?1", [&id], |row| row.get(0))?;
    let Some(relative) = relative else {
        return Ok(Change::one(Entity::File, id));
    };
    vault::remove(paths, &relative)?;
    tx.execute("UPDATE files SET vault_path = NULL, sha256 = NULL, updated_at = ?2 WHERE id = ?1", params![id, instant(now)])?;
    record(&tx, now, actor, Verb::Updated, &id, &name, serde_json::json!({ "vault": false }))?;
    tx.commit()?;
    Ok(Change::one(Entity::File, id))
}

/// Where to open a file from: the original when it is there, else its Vault copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Openable {
    pub path: PathBuf,
    pub from_vault: bool,
    pub program: bool,
    pub name: String,
}

pub fn openable(conn: &Connection, paths: &Paths, id: &str) -> Result<Openable> {
    let id = ids::parse("id", id)?;
    let (path, vault_path, extension, name): (String, Option<String>, String, String) = conn
        .query_row(
            "SELECT path, vault_path, extension, display_name FROM files WHERE id = ?1 AND deleted_at IS NULL",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?
        .ok_or(Error::NotFound("file"))?;
    let original = PathBuf::from(&path);
    if original.exists() {
        return Ok(Openable { path: original, from_vault: false, program: is_program(&extension), name });
    }
    match vault_path.as_deref().and_then(|relative| vault::resolve(paths, relative)) {
        Some(copy) if copy.exists() => Ok(Openable { path: copy, from_vault: true, program: is_program(&extension), name }),
        _ => Err(Error::NotFound("path")),
    }
}

/// The original's path, for a Vault copy to be made from.
pub fn source_path(conn: &Connection, id: &str) -> Result<(PathBuf, String)> {
    let id = ids::parse("id", id)?;
    let (path, name): (String, String) = conn
        .query_row("SELECT path, display_name FROM files WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?
        .ok_or(Error::NotFound("file"))?;
    Ok((PathBuf::from(path), name))
}

/// path, volume serial, file id, state, display name.
type StoredFile = (String, Option<i64>, Option<String>, FileState, String);

/// Look at one file again: still there, found somewhere else on its drive, or missing. Returns
/// the change when its state or path changed.
pub fn check(conn: &mut Connection, id: &str, finder: &dyn Finder, now: Timestamp) -> Result<Option<Change>> {
    let id = ids::parse("id", id)?;
    let row: Option<StoredFile> = conn
        .query_row(
            "SELECT path, volume_serial, file_index, state, display_name FROM files WHERE id = ?1 AND deleted_at IS NULL",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((path, volume, file_index, state, name)) = row else {
        return Err(Error::NotFound("file"));
    };
    let stored = match (volume, file_index) {
        (Some(volume), Some(file)) => Some(Identity { volume: volume as u64, file }),
        _ => None,
    };
    let here = Path::new(&path);
    // Still there when the path exists and, if Hub knows the file's id, it is the same file.
    let still_there = here.exists() && stored.as_ref().is_none_or(|identity| finder.identify(here).as_ref() == Some(identity));
    let stamp = instant(now);
    let tx = conn.transaction()?;
    if still_there {
        tx.execute("UPDATE files SET last_seen_at = ?2, state = 'ok' WHERE id = ?1", params![id, stamp])?;
        tx.commit()?;
        // "moved" becomes "ok" quietly: the owner has had the move shown once.
        return Ok((state == FileState::Missing).then(|| Change::one(Entity::File, id)));
    }
    if let Some(found) = stored.as_ref().and_then(|identity| finder.locate(identity)) {
        let found = found.to_string_lossy().into_owned();
        tx.execute(
            "UPDATE files SET path = ?2, state = 'moved', last_seen_at = ?3, updated_at = ?3 WHERE id = ?1",
            params![id, found, stamp],
        )?;
        record(&tx, now, Actor::Hub, Verb::Moved, &id, &name, serde_json::json!({ "from": path, "to": found }))?;
        tx.commit()?;
        return Ok(Some(Change::one(Entity::File, id)));
    }
    if state == FileState::Missing {
        return Ok(None);
    }
    tx.execute("UPDATE files SET state = 'missing', updated_at = ?2 WHERE id = ?1", params![id, stamp])?;
    record(&tx, now, Actor::Hub, Verb::Updated, &id, &name, serde_json::json!({ "state": "missing" }))?;
    tx.commit()?;
    Ok(Some(Change::one(Entity::File, id)))
}

/// The slow pass: the files seen longest ago first, a few at a time.
pub fn check_some(conn: &mut Connection, finder: &dyn Finder, limit: u32, now: Timestamp) -> Result<Vec<Change>> {
    let ids: Vec<String> = {
        let mut statement =
            conn.prepare("SELECT id FROM files WHERE deleted_at IS NULL ORDER BY coalesce(last_seen_at, '') LIMIT ?1")?;
        statement.query_map([limit], |row| row.get(0))?.collect::<rusqlite::Result<_>>()?
    };
    let mut changes = Vec::new();
    for id in ids {
        if let Some(change) = check(conn, &id, finder, now)? {
            changes.push(change);
        }
    }
    Ok(changes)
}

/// The owner points at where a missing file is now.
pub fn relocate(conn: &mut Connection, id: &str, probe: &Probe, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let name = name_for(&tx, &id)?;
    let path = probe.path.to_string_lossy().into_owned();
    tx.execute(
        "UPDATE files SET path = ?2, size_bytes = ?3, volume_serial = ?4, file_index = ?5, state = 'ok', last_seen_at = ?6, updated_at = ?6
         WHERE id = ?1",
        params![
            id,
            path,
            probe.size.map(|s| s as i64),
            probe.identity.as_ref().map(|i| i.volume as i64),
            probe.identity.as_ref().map(|i| i.file.clone()),
            instant(now)
        ],
    )?;
    record(&tx, now, actor, Verb::Moved, &id, &name, serde_json::json!({ "to": path }))?;
    tx.commit()?;
    Ok(Change::one(Entity::File, id))
}

// ---------- Koleksi ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub pigment: Pigment,
    pub files: u32,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CollectionInput {
    pub name: String,
    pub pigment: Pigment,
}

fn collection_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return invalid("name", Reason::Length);
    }
    Ok(name.to_string())
}

pub fn collections(conn: &Connection) -> Result<Vec<Collection>> {
    let mut statement = conn.prepare(
        "SELECT c.id, c.name, c.pigment,
                (SELECT count(*) FROM file_collections fc JOIN files f ON f.id = fc.file_id AND f.deleted_at IS NULL
                 WHERE fc.collection_id = c.id)
         FROM collections c WHERE c.deleted_at IS NULL ORDER BY c.order_key",
    )?;
    let rows = statement.query_map([], |row| Ok(Collection { id: row.get(0)?, name: row.get(1)?, pigment: row.get(2)?, files: row.get(3)? }))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn record_collection(tx: &Transaction<'_>, now: Timestamp, actor: Actor, verb: Verb, id: &str, name: &str) -> Result<()> {
    activity::record(tx, now, Record { actor, verb, entity: Entity::Collection, entity_id: Some(id), summary: name, details: serde_json::json!({}) })
}

pub fn create_collection(conn: &mut Connection, input: CollectionInput, actor: Actor, now: Timestamp) -> Result<(String, Change)> {
    let name = collection_name(&input.name)?;
    let tx = conn.transaction()?;
    let last: Option<String> = tx.query_row("SELECT max(order_key) FROM collections", [], |row| row.get(0))?;
    let id = new_id();
    tx.execute(
        "INSERT INTO collections (id, name, pigment, order_key, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![id, name, input.pigment, order::after(last.as_deref()), instant(now)],
    )?;
    record_collection(&tx, now, actor, Verb::Created, &id, &name)?;
    tx.commit()?;
    Ok((id.clone(), Change::one(Entity::Collection, id)))
}

pub fn update_collection(conn: &mut Connection, id: &str, input: CollectionInput, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let name = collection_name(&input.name)?;
    let tx = conn.transaction()?;
    let changed = tx.execute(
        "UPDATE collections SET name = ?2, pigment = ?3, updated_at = ?4 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, name, input.pigment, instant(now)],
    )?;
    if changed == 0 {
        return Err(Error::NotFound("collection"));
    }
    record_collection(&tx, now, actor, Verb::Updated, &id, &name)?;
    tx.commit()?;
    Ok(Change::one(Entity::Collection, id))
}

/// Remove a Koleksi. Its files stay in the Library.
pub fn delete_collection(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let name: String = tx
        .query_row("SELECT name FROM collections WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| row.get(0))
        .optional()?
        .ok_or(Error::NotFound("collection"))?;
    let stamp = instant(now);
    tx.execute("UPDATE collections SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1", params![id, stamp])?;
    tx.execute("DELETE FROM file_collections WHERE collection_id = ?1", [&id])?;
    record_collection(&tx, now, actor, Verb::Deleted, &id, &name)?;
    tx.commit()?;
    Ok(Change::all(Entity::Collection))
}

/// Put a file in exactly these Koleksi.
pub fn set_collections(conn: &mut Connection, id: &str, collection_ids: &[String], actor: Actor, now: Timestamp) -> Result<Vec<Change>> {
    let id = ids::parse("id", id)?;
    let wanted = collection_ids.iter().map(|c| ids::parse("collectionIds", c)).collect::<Result<Vec<_>>>()?;
    if wanted.len() > 50 {
        return invalid("collectionIds", Reason::TooMany);
    }
    let tx = conn.transaction()?;
    let name = name_for(&tx, &id)?;
    for collection in &wanted {
        let known: bool = tx.query_row(
            "SELECT count(*) > 0 FROM collections WHERE id = ?1 AND deleted_at IS NULL",
            [collection],
            |row| row.get(0),
        )?;
        if !known {
            return invalid("collectionIds", Reason::Unknown);
        }
    }
    tx.execute("DELETE FROM file_collections WHERE file_id = ?1", [&id])?;
    for collection in &wanted {
        tx.execute("INSERT OR IGNORE INTO file_collections (file_id, collection_id) VALUES (?1, ?2)", params![id, collection])?;
    }
    record(&tx, now, actor, Verb::Updated, &id, &name, serde_json::json!({ "collections": wanted }))?;
    tx.commit()?;
    Ok(vec![Change::one(Entity::File, id), Change::all(Entity::Collection)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::projects::{self, ProjectInput};
    use std::cell::RefCell;

    fn now() -> Timestamp {
        "2026-10-23T05:00:00Z".parse().unwrap()
    }

    /// A finder that knows where files "are" without touching the disk.
    #[derive(Default)]
    struct Fake {
        ids: RefCell<HashMap<PathBuf, Identity>>,
        moved_to: RefCell<Option<PathBuf>>,
    }

    impl Finder for Fake {
        fn identify(&self, path: &Path) -> Option<Identity> {
            self.ids.borrow().get(path).cloned()
        }
        fn locate(&self, _identity: &Identity) -> Option<PathBuf> {
            self.moved_to.borrow().clone()
        }
    }

    fn probe(path: &Path, file: &str) -> Probe {
        Probe {
            path: path.to_path_buf(),
            is_dir: false,
            size: Some(3),
            identity: Some(Identity { volume: 7, file: file.repeat(32) }),
            removable: false,
        }
    }

    #[test]
    fn kinds_come_from_the_extension() {
        assert_eq!(kind_of("pdf", false), FileKind::Document);
        assert_eq!(kind_of("pcapng", false), FileKind::Code);
        assert_eq!(kind_of("ova", false), FileKind::Archive);
        assert_eq!(kind_of("", true), FileKind::Folder);
        assert_eq!(kind_of("xyz", false), FileKind::Other);
        assert!(is_program("exe") && is_program("lnk") && !is_program("pdf"));
    }

    #[test]
    fn a_dropped_file_is_kept_once_and_attached() {
        let mut conn = open_in_memory().unwrap();
        let (project, _) = projects::create(
            &mut conn,
            ProjectInput { name: "Laporan Lab".into(), description: String::new(), area_id: None, pigment: Pigment::Toska, target_date: None },
            Actor::Owner,
            now(),
        )
        .unwrap();
        let path = PathBuf::from(r"C:\Users\x\Documents\Laporan VLAN.PDF");
        let target = LinkTarget { kind: LinkKind::Project, id: project.clone() };
        let (added, changes) = add(&mut conn, &probe(&path, "a"), Some(&target), Actor::Owner, now()).unwrap();
        assert!(!added.existed);
        assert_eq!(added.name, "Laporan VLAN.PDF");
        assert_eq!(changes.len(), 2);

        // The same file again (renamed meanwhile) is the same row.
        let renamed = PathBuf::from(r"C:\Users\x\Documents\VLAN final.pdf");
        let (again, _) = add(&mut conn, &probe(&renamed, "a"), None, Actor::Owner, now()).unwrap();
        assert!(again.existed);
        assert_eq!(again.id, added.id);

        let card = get(&conn, &added.id).unwrap();
        assert_eq!(card.kind, FileKind::Document);
        assert_eq!(card.extension, "pdf");
        assert_eq!(card.path, renamed.to_string_lossy());
        assert_eq!(card.links.len(), 1);
        assert_eq!(card.links[0].title, "Laporan Lab");
        assert_eq!(list(&conn, &FileQuery { project_id: Some(project), ..Default::default() }).unwrap().len(), 1);
        assert_eq!(counts(&conn).unwrap().all, 1);
    }

    #[test]
    fn unknown_targets_are_refused() {
        let mut conn = open_in_memory().unwrap();
        let target = LinkTarget { kind: LinkKind::Task, id: new_id() };
        let result = add(&mut conn, &probe(Path::new(r"C:\a.txt"), "b"), Some(&target), Actor::Owner, now());
        assert!(matches!(result, Err(Error::Invalid { field: "target", reason: Reason::Unknown })));
        assert_eq!(counts(&conn).unwrap().all, 0);
    }

    #[test]
    fn a_moved_file_is_followed_and_a_lost_one_marked_missing() {
        let mut conn = open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("writeup.md");
        std::fs::write(&old, b"x").unwrap();
        let (added, _) = add(&mut conn, &probe(&old, "c"), None, Actor::Owner, now()).unwrap();
        let fake = Fake::default();
        fake.ids.borrow_mut().insert(old.clone(), Identity { volume: 7, file: "c".repeat(32) });
        assert_eq!(check(&mut conn, &added.id, &fake, now()).unwrap(), None);

        // Moved on the same drive: found by id, path updated, recorded by Hub.
        let new = dir.path().join("writeup-thm.md");
        std::fs::rename(&old, &new).unwrap();
        *fake.moved_to.borrow_mut() = Some(new.clone());
        assert!(check(&mut conn, &added.id, &fake, now()).unwrap().is_some());
        let card = get(&conn, &added.id).unwrap();
        assert_eq!((card.state, card.path.as_str()), (FileState::Moved, new.to_string_lossy().as_ref()));
        assert_eq!(activity::recent(&conn, 1, false).unwrap()[0].verb, Verb::Moved);

        // Gone for good: missing, once.
        std::fs::remove_file(&new).unwrap();
        *fake.moved_to.borrow_mut() = None;
        assert!(check(&mut conn, &added.id, &fake, now()).unwrap().is_some());
        assert_eq!(check(&mut conn, &added.id, &fake, now()).unwrap(), None);
        assert_eq!(get(&conn, &added.id).unwrap().state, FileState::Missing);
        assert_eq!(counts(&conn).unwrap().missing, 1);

        // The owner points at it again.
        let back = dir.path().join("found.md");
        std::fs::write(&back, b"x").unwrap();
        relocate(&mut conn, &added.id, &probe(&back, "d"), Actor::Owner, now()).unwrap();
        assert_eq!(get(&conn, &added.id).unwrap().state, FileState::Ok);
    }

    #[test]
    fn forgetting_keeps_the_file_and_undo_brings_it_back() {
        let mut conn = open_in_memory().unwrap();
        let (added, _) = add(&mut conn, &probe(Path::new(r"C:\nmap.txt"), "e"), None, Actor::Owner, now()).unwrap();
        delete(&mut conn, &added.id, Actor::Owner, now()).unwrap();
        assert!(list(&conn, &FileQuery::default()).unwrap().is_empty());
        restore(&mut conn, &added.id, Actor::Owner, now()).unwrap();
        assert_eq!(list(&conn, &FileQuery::default()).unwrap().len(), 1);
        // Dropping a forgotten file again brings the old row back.
        delete(&mut conn, &added.id, Actor::Owner, now()).unwrap();
        let (again, _) = add(&mut conn, &probe(Path::new(r"C:\nmap.txt"), "e"), None, Actor::Owner, now()).unwrap();
        assert_eq!(again.id, added.id);
        assert!(!again.existed);
    }

    #[test]
    fn koleksi_hold_files_and_removing_one_keeps_them() {
        let mut conn = open_in_memory().unwrap();
        let (added, _) = add(&mut conn, &probe(Path::new(r"C:\cheatsheet.pdf"), "f"), None, Actor::Owner, now()).unwrap();
        let (writeups, _) = create_collection(&mut conn, CollectionInput { name: "Write-up".into(), pigment: Pigment::Lavender }, Actor::Owner, now()).unwrap();
        set_collections(&mut conn, &added.id, std::slice::from_ref(&writeups), Actor::Owner, now()).unwrap();
        assert_eq!(collections(&conn).unwrap()[0].files, 1);
        assert_eq!(get(&conn, &added.id).unwrap().collections[0].name, "Write-up");
        assert_eq!(list(&conn, &FileQuery { collection_id: Some(writeups.clone()), ..Default::default() }).unwrap().len(), 1);
        delete_collection(&mut conn, &writeups, Actor::Owner, now()).unwrap();
        assert!(get(&conn, &added.id).unwrap().collections.is_empty());
        assert_eq!(counts(&conn).unwrap().all, 1);
        assert!(set_collections(&mut conn, &added.id, &[writeups], Actor::Owner, now()).is_err());
    }

    #[test]
    fn a_vault_copy_opens_when_the_original_is_gone() {
        let mut conn = open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path().join("hub"));
        let source = dir.path().join("soal.pdf");
        std::fs::write(&source, b"pdf").unwrap();
        let mut from_usb = probe(&source, "9");
        from_usb.removable = true;
        let (added, _) = add(&mut conn, &from_usb, None, Actor::Owner, now()).unwrap();
        assert!(added.wants_copy);
        let copy = vault::copy_in(&paths, &source, &added.name, now()).unwrap();
        set_vault(&mut conn, &added.id, &copy, Actor::Owner, now()).unwrap();
        assert!(get(&conn, &added.id).unwrap().vaulted);
        std::fs::remove_file(&source).unwrap();
        let open = openable(&conn, &paths, &added.id).unwrap();
        assert!(open.from_vault);
        remove_vault(&mut conn, &paths, &added.id, Actor::Owner, now()).unwrap();
        assert!(openable(&conn, &paths, &added.id).is_err());
    }
}
