use crate::api;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

// Copyright 2024 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

/// This module handles the local file-based cache for sessions and aliases.
///
/// The cache is responsible for storing and retrieving session information and
/// alias mappings to and from the user's configuration directory. This allows
/// for persistent state between application runs.

/// Represents a session that is stored in the local cache.
///
/// This struct holds the essential information about a session that needs to
/// be persisted locally for quick access and for resolving session identifiers.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CachedSession {
    /// The unique identifier of the session.
    pub id: String,
    /// The title of the session.
    pub title: String,
    /// The source context of the session.
    #[serde(rename = "sourceContext")]
    pub source_context: Option<api::SourceContext>,
}

/// A type alias for a map of aliases to their corresponding session IDs.
///
/// Aliases are user-defined shortcuts for session IDs, allowing for easier
/// reference to sessions in the command-line interface.
pub type Aliases = HashMap<String, String>;

/// Manages the local cache for sessions and aliases.
///
/// This struct provides a centralized way to interact with the local cache,
/// handling file I/O and serialization/deserialization of session and alias
/// data.
pub struct Cache {
    /// The path to the sessions cache file.
    sessions_file: PathBuf,
    /// The path to the aliases cache file.
    aliases_file: PathBuf,
    /// The path to the chat ID cache file.
    chat_id_file: PathBuf,
    /// The path to the current session cache file.
    current_session_file: PathBuf,
}

impl Cache {
    /// Creates a new `Cache` instance.
    ///
    /// This function initializes the cache by determining the paths to the
    /// session and alias cache files within the user's configuration directory.
    /// It also ensures that the cache directory exists, creating it if necessary.
    ///
    /// # Returns
    ///
    /// A `Result` containing the new `Cache` instance, or an error string if
    /// the configuration directory cannot be found or created.
    pub fn new() -> Result<Self, String> {
        let config_dir = dirs::config_dir().ok_or("Could not find config directory")?;
        let julezz_dir = config_dir.join("julezz");
        fs::create_dir_all(&julezz_dir)
            .map_err(|e| format!("Could not create config directory: {}", e))?;

        Ok(Self {
            sessions_file: julezz_dir.join("sessions.json"),
            aliases_file: julezz_dir.join("aliases.json"),
            chat_id_file: julezz_dir.join("chat_id.txt"),
            current_session_file: julezz_dir.join("current_session.txt"),
        })
    }

    /// Reads the cached sessions from disk.
    ///
    /// # Returns
    ///
    /// A `Result` containing a vector of `CachedSession`s, or an error string
    /// if the file cannot be read or parsed.
    pub fn read_sessions(&self) -> Result<Vec<CachedSession>, String> {
        if !self.sessions_file.exists() {
            return Ok(Vec::new());
        }
        let data = fs::read_to_string(&self.sessions_file)
            .map_err(|e| format!("Could not read sessions file: {}", e))?;
        serde_json::from_str(&data).map_err(|e| format!("Could not parse sessions file: {}", e))
    }

    /// Writes the given sessions to the cache file.
    ///
    /// # Arguments
    ///
    /// * `sessions` - A slice of `CachedSession`s to write to the cache.
    pub fn write_sessions(&self, sessions: &[CachedSession]) -> Result<(), String> {
        let json = serde_json::to_string(sessions)
            .map_err(|e| format!("Could not serialize sessions: {}", e))?;
        fs::write(&self.sessions_file, json)
            .map_err(|e| format!("Could not write sessions file: {}", e))
    }

    /// Reads the aliases from the cache file.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `Aliases` map, or an error string if the file
    /// cannot be read or parsed.
    pub fn read_aliases(&self) -> Result<Aliases, String> {
        if !self.aliases_file.exists() {
            return Ok(Aliases::new());
        }
        let data = fs::read_to_string(&self.aliases_file)
            .map_err(|e| format!("Could not read aliases file: {}", e))?;

        serde_json::from_str(&data).map_err(|e| {
            if e.is_data() {
                "Your aliases file is in an old format. Please delete it and re-create your aliases.".to_string()
            } else {
                format!("Could not parse aliases file: {}", e)
            }
        })
    }

    /// Writes the given aliases to the cache file.
    ///
    /// # Arguments
    ///
    /// * `aliases` - A reference to the `Aliases` map to write to the cache.
    pub fn write_aliases(&self, aliases: &Aliases) -> Result<(), String> {
        let json = serde_json::to_string(aliases)
            .map_err(|e| format!("Could not serialize aliases: {}", e))?;
        fs::write(&self.aliases_file, json)
            .map_err(|e| format!("Could not write aliases file: {}", e))
    }

    /// Reads the chat ID from the cache file.
    ///
    /// # Returns
    ///
    /// A `Result` containing the chat ID as a string, or `None` if the file
    /// does not exist.
    pub fn read_chat_id(&self) -> Result<Option<String>, String> {
        if !self.chat_id_file.exists() {
            return Ok(None);
        }
        fs::read_to_string(&self.chat_id_file)
            .map(Some)
            .map_err(|e| format!("Could not read chat ID file: {}", e))
    }

    /// Writes the given chat ID to the cache file.
    ///
    /// # Arguments
    ///
    /// * `chat_id` - The chat ID to write to the cache.
    pub fn write_chat_id(&self, chat_id: &str) -> Result<(), String> {
        fs::write(&self.chat_id_file, chat_id)
            .map_err(|e| format!("Could not write chat ID file: {}", e))
    }

    /// Reads the current session ID from the cache file.
    ///
    /// # Returns
    ///
    /// A `Result` containing the current session ID as a string, or `None` if
    /// the file does not exist.
    pub fn read_current_session(&self) -> Result<Option<String>, String> {
        if !self.current_session_file.exists() {
            return Ok(None);
        }
        fs::read_to_string(&self.current_session_file)
            .map(Some)
            .map_err(|e| format!("Could not read current session file: {}", e))
    }

    /// Writes the given session ID to the current session cache file.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID to write to the cache.
    pub fn write_current_session(&self, session_id: &str) -> Result<(), String> {
        fs::write(&self.current_session_file, session_id)
            .map_err(|e| format!("Could not write current session file: {}", e))
    }
}
