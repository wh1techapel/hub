//! The clients' tokens and where each client finds its own.
//!
//! Hub writes one file per client in its data folder (`%LOCALAPPDATA%\Hub`, which only the
//! owner's account can read): `api.json` for ULTRON, `claude-code.json` for Claude Code's bridge
//! (docs/studio.md, "MCP"). Each holds the address and that client's token; the database keeps
//! only the token's hash. A client reads its file before connecting and again after a 401, so a
//! rotated token or a new port is picked up without touching the client's settings.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use hub_core::api_clients::{self, Scope};
use hub_core::model::Actor;
use hub_core::{Hub, Paths};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::guard::TOKEN_CHARS;

/// A client Hub sets up itself: its name, its file, and its scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Client {
    pub name: &'static str,
    file: &'static str,
    scopes: &'static [Scope],
}

pub const ULTRON: Client = Client { name: "ULTRON", file: "api.json", scopes: Scope::ULTRON };
pub const CLAUDE: Client = Client { name: "Claude Code", file: "claude-code.json", scopes: Scope::CLAUDE };
/// ULTRON's name, as before Studio.
pub const CLIENT: &str = ULTRON.name;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiFile {
    pub version: u32,
    pub url: String,
    pub port: u16,
    pub client: String,
    pub token: String,
}

pub fn file_path(paths: &Paths, client: Client) -> PathBuf {
    paths.root.join(client.file)
}

fn read(paths: &Paths, client: Client) -> Option<ApiFile> {
    let text = fs::read_to_string(file_path(paths, client)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Written beside, then moved over the old file, so a client never reads half a file.
fn write(paths: &Paths, client: Client, port: u16, token: &str) -> std::io::Result<()> {
    let file = ApiFile {
        version: crate::API_VERSION,
        url: format!("http://127.0.0.1:{port}"),
        port,
        client: client.name.into(),
        token: token.into(),
    };
    let target = file_path(paths, client);
    let partial = target.with_extension("json.partial");
    {
        let mut out = fs::File::create(&partial)?;
        out.write_all(serde_json::to_string_pretty(&file).map_err(std::io::Error::other)?.as_bytes())?;
        out.sync_all()?;
    }
    fs::rename(&partial, &target)
}

fn new_token() -> Result<String, hub_core::Error> {
    let mut bytes = [0u8; TOKEN_CHARS / 2];
    getrandom::fill(&mut bytes).map_err(|error| hub_core::Error::Io(std::io::Error::other(error.to_string())))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Make sure `client` has a working token in its file for `port`: the one there is kept when the
/// database knows it for that client, otherwise a new one is made (first start, a deleted file, a
/// restored database). Returns true when a new token was made.
pub fn ensure(hub: &Hub, port: u16, client: Client) -> hub_core::Result<bool> {
    if let Some(file) = read(hub.paths(), client) {
        let known = hub.read(|conn| api_clients::find_by_hash(conn, &api_clients::hash_token(&file.token)))?;
        if known.is_some_and(|found| found.name == client.name) {
            if file.port != port {
                write(hub.paths(), client, port, &file.token)?;
            }
            return Ok(false);
        }
    }
    replace(hub, port, client, Actor::Hub)?;
    Ok(true)
}

/// A new token (Pengaturan's "Buat token baru"): the old one stops working at once.
pub fn rotate(hub: &Hub, port: u16, client: Client) -> hub_core::Result<()> {
    replace(hub, port, client, Actor::Owner)
}

fn replace(hub: &Hub, port: u16, client: Client, actor: Actor) -> hub_core::Result<()> {
    let token = new_token()?;
    let hash = api_clients::hash_token(&token);
    // The file first: if the database write then failed, the old token would still work and the
    // next start would simply make another.
    write(hub.paths(), client, port, &token)?;
    hub.write(|conn| api_clients::set_token(conn, client.name, &hash, client.scopes, actor, Timestamp::now()))?;
    Ok(())
}

/// The token in a client's file, for the app's own checks and the smokes.
pub fn current(paths: &Paths, client: Client) -> Option<ApiFile> {
    read(paths, client)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_made_once_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let hub = Hub::open(Paths::under(dir.path())).unwrap();
        assert!(ensure(&hub, 47731, ULTRON).unwrap());
        let first = current(hub.paths(), ULTRON).unwrap();
        assert_eq!(first.token.len(), TOKEN_CHARS);
        assert_eq!(first.url, "http://127.0.0.1:47731");
        assert!(!ensure(&hub, 47731, ULTRON).unwrap(), "a known token is kept");

        assert!(!ensure(&hub, 48000, ULTRON).unwrap(), "a new port keeps the token");
        let moved = current(hub.paths(), ULTRON).unwrap();
        assert_eq!((moved.port, moved.token.as_str()), (48000, first.token.as_str()));

        rotate(&hub, 48000, ULTRON).unwrap();
        let rotated = current(hub.paths(), ULTRON).unwrap();
        assert_ne!(rotated.token, first.token);
        let old = hub.read(|conn| api_clients::find_by_hash(conn, &api_clients::hash_token(&first.token))).unwrap();
        assert!(old.is_none(), "the old token stops working");
    }

    #[test]
    fn an_unknown_token_in_the_file_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let hub = Hub::open(Paths::under(dir.path())).unwrap();
        write(hub.paths(), ULTRON, 47731, &"0".repeat(64)).unwrap();
        assert!(ensure(&hub, 47731, ULTRON).unwrap());
        assert_ne!(current(hub.paths(), ULTRON).unwrap().token, "0".repeat(64));
    }

    #[test]
    fn each_client_has_its_own_file_token_and_scopes() {
        let dir = tempfile::tempdir().unwrap();
        let hub = Hub::open(Paths::under(dir.path())).unwrap();
        ensure(&hub, 47731, ULTRON).unwrap();
        ensure(&hub, 47731, CLAUDE).unwrap();
        let ultron = current(hub.paths(), ULTRON).unwrap();
        let claude = current(hub.paths(), CLAUDE).unwrap();
        assert_ne!(ultron.token, claude.token);
        assert_eq!(claude.client, "Claude Code");
        assert!(file_path(hub.paths(), CLAUDE).ends_with("claude-code.json"));
        let found = hub.read(|conn| api_clients::find_by_hash(conn, &api_clients::hash_token(&claude.token))).unwrap().unwrap();
        assert_eq!(found.scopes, Scope::CLAUDE);
        assert!(!found.may(Scope::Read), "Claude reads Studio only");
        // A token in Claude's file that belongs to ULTRON is not Claude's: replaced.
        write(hub.paths(), CLAUDE, 47731, &ultron.token).unwrap();
        assert!(ensure(&hub, 47731, CLAUDE).unwrap());
        // Rotating Claude's leaves ULTRON's working.
        rotate(&hub, 47731, CLAUDE).unwrap();
        assert!(hub.read(|conn| api_clients::find_by_hash(conn, &api_clients::hash_token(&ultron.token))).unwrap().is_some());
    }
}
