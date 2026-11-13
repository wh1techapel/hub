use std::fmt;

/// Why a request was refused or failed. `Invalid` carries a field and a reason code instead of a
/// sentence, so the UI says it in the owner's language.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid {field}: {reason}")]
    Invalid { field: &'static str, reason: Reason },
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("this database was written by a newer Hub (schema {found}, this Hub knows {known})")]
    NewerDatabase { found: i64, known: i64 },
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("file system: {0}")]
    Io(#[from] std::io::Error),
    #[error("time: {0}")]
    Time(#[from] jiff::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("the database lock was poisoned by an earlier panic")]
    Poisoned,
}

pub type Result<T> = std::result::Result<T, Error>;

/// What was wrong with a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Empty or too long.
    Length,
    /// Not in the expected shape (a date, a time, an id).
    Format,
    /// Values in the wrong order (sleep before wake, end before start).
    Order,
    /// Too many items (more than three focus tasks).
    TooMany,
    /// Points at something that does not exist.
    Unknown,
    /// Worked out by Hub, so it cannot be set (a milestone with tasks is done when they are).
    Derived,
}

impl Reason {
    pub fn as_str(self) -> &'static str {
        match self {
            Reason::Length => "length",
            Reason::Format => "format",
            Reason::Order => "order",
            Reason::TooMany => "too_many",
            Reason::Unknown => "unknown",
            Reason::Derived => "derived",
        }
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub(crate) fn invalid<T>(field: &'static str, reason: Reason) -> Result<T> {
    Err(Error::Invalid { field, reason })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ErrorKind {
    Invalid,
    NotFound,
    NewerDatabase,
    Internal,
}

/// An error as the UI receives it: a kind and, for invalid input, the field and reason code. The
/// message is for logs and developers, never shown to the owner as is.
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ErrorView {
    pub kind: ErrorKind,
    pub field: Option<String>,
    pub reason: Option<String>,
    pub message: String,
}

impl From<&Error> for ErrorView {
    fn from(error: &Error) -> ErrorView {
        let (kind, field, reason) = match error {
            Error::Invalid { field, reason } => {
                (ErrorKind::Invalid, Some(field.to_string()), Some(reason.as_str().to_string()))
            }
            Error::NotFound(what) => (ErrorKind::NotFound, Some(what.to_string()), None),
            Error::NewerDatabase { .. } => (ErrorKind::NewerDatabase, None, None),
            _ => (ErrorKind::Internal, None, None),
        };
        ErrorView {
            kind,
            field,
            reason,
            message: error.to_string(),
        }
    }
}

impl From<Error> for ErrorView {
    fn from(error: Error) -> ErrorView {
        ErrorView::from(&error)
    }
}
