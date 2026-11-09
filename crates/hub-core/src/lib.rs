//! Hub's domain core: the only code that touches the database.
//!
//! The desktop app (`src-tauri`) and, from CP5, the local API for ULTRON both call these
//! functions, so the rules live in one place: input is checked here, every write records itself in
//! `activity` in the same transaction, and every write returns a [`Change`] the app can announce.
//! Nothing in this crate knows about windows, Tauri, or the UI's language.

pub mod activity;
pub mod api_clients;
pub mod areas;
pub mod backup;
pub mod calendar;
pub mod claude;
pub mod claude_sessions;
pub mod capture;
pub mod cards;
pub mod context;
pub mod dayclose;
pub mod db;
mod error;
pub mod events;
pub mod export;
pub mod files;
mod hub;
pub mod identity;
pub mod ids;
pub mod imports;
pub mod info;
pub mod insights;
pub mod journal;
pub mod learning;
pub mod model;
pub mod order;
pub mod prd;
pub mod projects;
pub mod recurrence;
pub mod roadmap;
pub mod reminders;
pub mod routines;
pub mod search;
pub mod settings;
pub mod stack;
pub mod studio;
pub mod tasks;
pub mod time;
pub mod today;
pub mod vault;

pub use error::{Error, ErrorKind, ErrorView, Reason, Result};
pub use hub::{Hub, Paths};
pub use model::Change;
