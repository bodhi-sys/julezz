// src/resolve.rs

use crate::cache::Cache;

pub fn resolve_session_identifier(identifier: &str) -> Result<String, String> {
    resolve_session_identifier_and_index(identifier).map(|(id, _index)| id)
}

pub fn resolve_session_identifier_and_index(identifier: &str) -> Result<(String, usize), String> {
    let cache = Cache::new()?;
    let sessions = cache.read_sessions()?;

    if sessions.is_empty() {
        return Err("No sessions found in cache. Run `sessions list` to refresh.".to_string());
    }

    if identifier.starts_with('@') {
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
                    "Session ID '{}' for alias '{}' not found in cache. Run `sessions list`.",
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
                .ok_or_else(|| {
                    "Session index out of bounds. Run `sessions list` to see available sessions."
                        .to_string()
                });
        }

        // If it's not an alias and not a valid index, assume it's a session ID
        sessions
            .iter()
            .position(|s| s.id == identifier)
            .map(|index| (identifier.to_string(), index + 1))
            .ok_or_else(|| {
                format!(
                    "Session ID '{}' not found in cache. Run `sessions list`.",
                    identifier
                )
            })
    }
}
