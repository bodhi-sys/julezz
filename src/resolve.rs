// src/resolve.rs

use crate::api::Session;
use crate::cache::Cache;

pub fn resolve_session_identifier(
    identifier: &str,
    sessions: &[Session],
) -> Result<String, String> {
    resolve_session_identifier_and_index(identifier, sessions).map(|(id, _index)| id)
}

pub fn resolve_session_identifier_and_index(
    identifier: &str,
    sessions: &[Session],
) -> Result<(String, usize), String> {
    if sessions.is_empty() {
        return Err("No sessions found.".to_string());
    }

    if identifier.starts_with('@') {
        let cache = Cache::new()?;
        let aliases = cache.read_aliases()?;
        let session_id = aliases
            .get(identifier)
            .ok_or_else(|| format!("Alias '{}' not found.", identifier))?;

        sessions
            .iter()
            .position(|s| s.id == *session_id)
            .map(|index| (session_id.clone(), index + 1))
            .ok_or_else(|| {
                format!(
                    "Session ID '{}' for alias '{}' not found.",
                    session_id, identifier
                )
            })
    } else {
        // Try parsing as an index first
        if let Ok(index) = identifier.parse::<usize>() {
            if index == 0 {
                return Err("Index must be greater than 0".to_string());
            }

            return sessions
                .get(index - 1)
                .map(|session| (session.id.clone(), index))
                .ok_or_else(|| "Session index out of bounds.".to_string());
        }

        // If it's not an alias and not a valid index, assume it's a session ID
        sessions
            .iter()
            .position(|s| s.id == identifier)
            .map(|index| (identifier.to_string(), index + 1))
            .ok_or_else(|| format!("Session ID '{}' not found.", identifier))
    }
}
