//! Identifiers. Wisp uses UUIDv7 (time-ordered) for all persisted rows.

use uuid::Uuid;

/// Create a new time-ordered UUID string suitable for primary keys.
pub fn new_id() -> String {
    Uuid::now_v7().to_string()
}

/// Returns true if `s` parses as a UUID. Used to validate ids received over IPC.
pub fn is_valid_id(s: &str) -> bool {
    Uuid::parse_str(s).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_valid_and_ordered() {
        let a = new_id();
        let b = new_id();
        assert!(is_valid_id(&a));
        assert!(a < b, "v7 ids are time-ordered");
        assert!(!is_valid_id("../etc/passwd"));
    }
}
