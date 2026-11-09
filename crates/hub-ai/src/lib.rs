//! Hub Studio's side of Google Gemini (docs/studio.md, "Gemini integration").
//!
//! - [`key`]: the API key, kept in Windows Credential Manager and never shown, logged, or stored
//!   anywhere else;
//! - [`client`]: the REST client (`generativelanguage.googleapis.com`, `x-goog-api-key` header,
//!   the Interactions API with `store: false`), timeouts, and Google's errors sorted into
//!   [`FailKind`]s the UI can explain;
//! - [`check`]: "Uji koneksi", which tells the owner which of the usual problems it is, without
//!   changing anything.
//!
//! There is no official Rust SDK; Google documents the REST API for every language.

pub mod calls;
pub mod check;
pub mod client;
pub mod error;
pub mod key;
pub mod prd;
pub mod prompt;
pub mod roadmap;
pub mod stack;
pub mod stream;

pub use check::{check_connection, CheckOutcome, CheckReport};
pub use client::{Gemini, ModelInfo, DEFAULT_BASE_URL};
pub use error::{FailKind, GeminiError};
pub use key::{ApiKey, KeyStore};
