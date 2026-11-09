//! Row ids: UUIDv7 text, ordered by creation time and safe to merge from another Hub or an import.

use crate::error::{invalid, Reason, Result};

pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// An id from outside (the UI, later the API): parsed, so nothing else ever reaches SQL as an id.
pub fn parse(field: &'static str, text: &str) -> Result<String> {
    uuid::Uuid::try_parse(text)
        .map(|id| id.to_string())
        .or_else(|_| invalid(field, Reason::Format))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_sort_by_creation() {
        let first = new_id();
        let second = new_id();
        assert!(first < second);
    }

    #[test]
    fn only_uuids_are_ids() {
        let id = new_id();
        assert_eq!(parse("id", &id).unwrap(), id);
        assert!(parse("id", "1; DROP TABLE tasks").is_err());
        assert!(parse("id", "").is_err());
    }
}
