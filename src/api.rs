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

//! This module provides a client for the Jules API.
//!
//! It includes data structures for the API resources and a client for making
//! requests to the API.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::process::Command;

const API_BASE_URL: &str = "https://jules.googleapis.com/v1alpha";

/// Represents a source in the Jules API.
#[derive(Debug, Serialize, Deserialize)]
pub struct Source {
    pub name: String,
    pub id: String,
}

/// Represents the response from the `list_sources` endpoint.
#[derive(Debug, Serialize, Deserialize)]
struct ListSourcesResponse {
    sources: Vec<Source>,
}

/// Represents the source context for a session.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SourceContext {
    pub source: String,
    #[serde(rename = "githubRepoContext")]
    pub github_repo_context: Option<GithubRepoContext>,
}

/// Represents the GitHub repository context for a session.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GithubRepoContext {
    pub starting_branch: String,
}

/// Represents a session in the Jules API.
#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    pub name: String,
    pub id: String,
    pub state: Option<String>,
    pub title: String,
    #[serde(rename = "sourceContext")]
    pub source_context: Option<SourceContext>,
    #[serde(rename = "pullRequestUrl")]
    pub pull_request_url: Option<String>,
}

/// Represents the response from the `list_sessions` endpoint.
#[derive(Debug, Serialize, Deserialize)]
struct ListSessionsResponse {
    sessions: Vec<Session>,
}

/// Represents an activity in a session.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub name: String,
    pub id: String,
    pub title: Option<String>,
    pub create_time: String,
    pub originator: String,
    pub agent_messaged: Option<AgentMessaged>,
    pub user_messaged: Option<UserMessaged>,
    pub progress_updated: Option<ProgressUpdated>,
    pub plan_approved: Option<PlanApproved>,
    pub plan_generated: Option<PlanGenerated>,
    pub session_completed: Option<SessionCompleted>,
    pub artifacts: Option<Vec<Artifact>>,
}

/// Represents a plan approval activity.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlanApproved {
    pub plan_id: Option<String>,
}

/// Represents a progress update activity.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressUpdated {
    pub title: Option<String>,
    pub description: Option<String>,
}

/// Represents an artifact associated with an activity.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub bash_output: Option<BashOutput>,
    pub change_set: Option<ChangeSet>,
}

/// Represents the output of a bash command.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BashOutput {
    pub command: String,
    pub output: String,
}

/// Represents a change set.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSet {
    pub source: String,
    pub git_patch: GitPatch,
    pub suggested_commit_message: Option<String>,
}

/// Represents a git patch.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GitPatch {
    pub unidiff_patch: Option<String>,
    pub base_commit_id: String,
}

/// Represents a plan generation activity.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlanGenerated {
    pub plan: Plan,
}

/// Represents a plan.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    pub steps: Vec<Step>,
}

/// Represents a step in a plan.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
}

/// Represents a session completion activity.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionCompleted {}

/// Represents a user message activity.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserMessaged {
    pub user_message: String,
}

/// Represents an agent message activity.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessaged {
    pub agent_message: String,
}

/// Represents the response from the `list_activities` endpoint.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListActivitiesResponse {
    activities: Vec<Activity>,
    next_page_token: Option<String>,
}

/// Represents an error that can occur when using the Jules API client.
#[derive(Debug)]
pub enum JulesError {
    ApiKeyMissing,
    ReqwestError(reqwest::Error),
    ApiError(String),
}

impl fmt::Display for JulesError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            JulesError::ApiKeyMissing => write!(f, "API key is missing."),
            JulesError::ReqwestError(e) => write!(f, "Request error: {}", e),
            JulesError::ApiError(e) => write!(f, "API error: {}", e),
        }
    }
}

impl From<reqwest::Error> for JulesError {
    fn from(err: reqwest::Error) -> JulesError {
        JulesError::ReqwestError(err)
    }
}

/// A client for the Jules API.
pub struct JulesClient {
    api_key: String,
    client: reqwest::Client,
}

impl JulesClient {
    /// Creates a new `JulesClient`.
    pub fn new(api_key: Option<String>) -> Result<Self, JulesError> {
        let api_key = api_key.ok_or(JulesError::ApiKeyMissing)?;
        Ok(Self {
            api_key,
            client: reqwest::Client::new(),
        })
    }

    /// Handles the response from the Jules API.
    async fn handle_response<T: for<'de> Deserialize<'de>>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, JulesError> {
        if response.status().is_success() {
            Ok(response.json::<T>().await?)
        } else {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            Err(JulesError::ApiError(format!(
                "API Error: {} - {}",
                status, text
            )))
        }
    }

    /// Lists the available sources.
    pub async fn list_sources(&self) -> Result<Vec<Source>, JulesError> {
        let url = format!("{}/sources", API_BASE_URL);
        let response = self
            .client
            .get(&url)
            .header("x-goog-api-key", &self.api_key)
            .send()
            .await?;
        let list_response = self.handle_response::<ListSourcesResponse>(response).await?;
        Ok(list_response.sources)
    }

    /// Gets a source by its ID.
    pub async fn get_source(&self, id: &str) -> Result<Source, JulesError> {
        let url = format!("{}/sources/{}", API_BASE_URL, id);
        let response = self
            .client
            .get(&url)
            .header("x-goog-api-key", &self.api_key)
            .send()
            .await?;
        self.handle_response(response).await
    }

    /// Deletes a session by its ID.
    pub async fn delete_session(&self, id: &str) -> Result<(), JulesError> {
        let url = format!("{}/sessions/{}", API_BASE_URL, id);
        let response = self
            .client
            .delete(&url)
            .header("x-goog-api-key", &self.api_key)
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(JulesError::ApiError(format!(
                "API Error: {} - {}",
                status, text
            )));
        }
        Ok(())
    }

    /// Lists the available sessions.
    pub async fn list_sessions(&self) -> Result<Vec<Session>, JulesError> {
        let url = format!("{}/sessions", API_BASE_URL);
        let response = self
            .client
            .get(&url)
            .header("x-goog-api-key", &self.api_key)
            .send()
            .await?;
        let list_response = self.handle_response::<ListSessionsResponse>(response).await?;
        Ok(list_response.sessions)
    }

    /// Creates a new session.
    pub async fn create_session(
        &self,
        source: &str,
        title: &str,
        auto_pr: bool,
        branch: &str,
    ) -> Result<Session, JulesError> {
        let url = format!("{}/sessions", API_BASE_URL);
        let mut json_body = serde_json::json!({
            "prompt": title,
            "sourceContext": {
                "source": source,
                "githubRepoContext": {
                    "startingBranch": branch
                }
            },
            "title": title
        });
        if auto_pr {
            json_body["automationMode"] = serde_json::json!("AUTO_CREATE_PR");
        }
        let response = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&json_body)
            .send()
            .await?;
        self.handle_response(response).await
    }

    /// Gets a session by its ID.
    pub async fn get_session(&self, id: &str) -> Result<Session, JulesError> {
        let url = format!("{}/sessions/{}", API_BASE_URL, id);
        let response = self
            .client
            .get(&url)
            .header("x-goog-api-key", &self.api_key)
            .send()
            .await?;
        self.handle_response(response).await
    }

    /// Approves the plan for a session.
    pub async fn approve_plan(&self, id: &str) -> Result<(), JulesError> {
        let url = format!("{}/sessions/{}:approvePlan", API_BASE_URL, id);
        let response = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&serde_json::json!({}))
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(JulesError::ApiError(format!(
                "API Error: {} - {}",
                status, text
            )));
        }
        Ok(())
    }

    /// Sends a message to a session.
    pub async fn send_message(&self, id: &str, prompt: &str) -> Result<(), JulesError> {
        let url = format!("{}/sessions/{}:sendMessage", API_BASE_URL, id);
        let response = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&serde_json::json!({ "prompt": prompt }))
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(JulesError::ApiError(format!(
                "API Error: {} - {}",
                status, text
            )));
        }
        Ok(())
    }

    /// Lists the cached activities for a session.
    pub fn list_cached_activities(&self, session_id: &str) -> Result<Vec<Activity>, JulesError> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| JulesError::ApiError("Could not determine cache directory".to_string()))?
            .join("julezz")
            .join(session_id);

        let messages_path = cache_dir.join("messages.json");
        let last_page_path = cache_dir.join("last_page.json");

        let mut activities: Vec<Activity> = if messages_path.exists() {
            let data = fs::read_to_string(&messages_path)
                .map_err(|e| JulesError::ApiError(format!("Could not read messages file: {}", e)))?;
            serde_json::from_str(&data)
                .map_err(|e| JulesError::ApiError(format!("Could not parse messages file: {}", e)))?
        } else {
            Vec::new()
        };

        if last_page_path.exists() {
            let data = fs::read_to_string(&last_page_path).map_err(|e| {
                JulesError::ApiError(format!("Could not read last page file: {}", e))
            })?;
            let last_page_activities: Vec<Activity> = serde_json::from_str(&data).map_err(
                |e| JulesError::ApiError(format!("Could not parse last page file: {}", e)),
            )?;
            activities.extend(last_page_activities);
        }

        Ok(activities)
    }

    /// Fetches the activities for a session from the API.
    pub async fn fetch_activities(
        &self,
        session_id: &str,
    ) -> Result<Vec<Activity>, JulesError> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| JulesError::ApiError("Could not determine cache directory".to_string()))?
            .join("julezz")
            .join(session_id);
        fs::create_dir_all(&cache_dir)
            .map_err(|e| JulesError::ApiError(format!("Could not create cache directory: {}", e)))?;

        let messages_path = cache_dir.join("messages.json");
        let last_page_path = cache_dir.join("last_page.json");
        let page_token_path = cache_dir.join("page_token.json");

        let mut stable_activities: Vec<Activity> = if messages_path.exists() {
            let data = fs::read_to_string(&messages_path)
                .map_err(|e| JulesError::ApiError(format!("Could not read messages file: {}", e)))?;
            serde_json::from_str(&data)
                .map_err(|e| JulesError::ApiError(format!("Could not parse messages file: {}", e)))?
        } else {
            Vec::new()
        };

        let mut page_token: Option<String> = if page_token_path.exists() {
            fs::read_to_string(&page_token_path)
                .map_err(|e| JulesError::ApiError(format!("Could not read page token file: {}", e)))
                .ok()
        } else {
            None
        };

        let mut new_activities_to_make_stable = Vec::new();
        let mut last_page_activities = Vec::new();
        let mut last_page_token: Option<String> = None;

        loop {
            let current_page_token_for_request = page_token.clone();
            let url = format!("{}/sessions/{}/activities", API_BASE_URL, session_id);
            let mut request_builder = self
                .client
                .get(&url)
                .header("x-goog-api-key", &self.api_key);

            if let Some(token) = &current_page_token_for_request {
                request_builder = request_builder.query(&[("page_token", token)]);
            }

            let response = request_builder.send().await?;
            let list_response = self
                .handle_response::<ListActivitiesResponse>(response)
                .await?;

            if list_response.next_page_token.is_some() {
                new_activities_to_make_stable.extend(list_response.activities);
            } else {
                last_page_activities = list_response.activities;
                last_page_token = current_page_token_for_request;
            }

            page_token = list_response.next_page_token;

            if page_token.is_none() {
                break;
            }
        }

        stable_activities.extend(new_activities_to_make_stable);
        fs::write(
            &messages_path,
            serde_json::to_string(&stable_activities)
                .map_err(|e| JulesError::ApiError(format!("Could not serialize messages: {}", e)))?,
        )
        .map_err(|e| JulesError::ApiError(format!("Could not write messages file: {}", e)))?;

        fs::write(
            &last_page_path,
            serde_json::to_string(&last_page_activities).map_err(|e| {
                JulesError::ApiError(format!("Could not serialize last page activities: {}", e))
            })?,
        )
        .map_err(|e| JulesError::ApiError(format!("Could not write last page file: {}", e)))?;

        if let Some(token) = last_page_token {
            fs::write(&page_token_path, token)
                .map_err(|e| JulesError::ApiError(format!("Could not write page token file: {}", e)))?;
        } else if page_token_path.exists() {
            fs::remove_file(&page_token_path).map_err(|e| {
                JulesError::ApiError(format!("Could not remove page token file: {}", e))
            })?;
        }

        stable_activities.extend(last_page_activities);
        Ok(stable_activities)
    }

    /// Gets an activity by its ID.
    pub async fn get_activity(&self, session_id: &str, id: &str) -> Result<Activity, JulesError> {
        let url = format!(
            "{}/sessions/{}/activities/{}",
            API_BASE_URL, session_id, id
        );
        let response = self
            .client
            .get(&url)
            .header("x-goog-api-key", &self.api_key)
            .send()
            .await?;
        self.handle_response(response).await
    }

    /// Merges the pull request for a session.
    pub fn merge_pull_request(&self, pull_request_url: &str) -> Result<(), JulesError> {
        let output = Command::new("gh")
            .arg("pr")
            .arg("merge")
            .arg(pull_request_url)
            .output()
            .map_err(|e| JulesError::ApiError(format!("Failed to execute gh command: {}", e)))?;

        if !output.status.success() {
            return Err(JulesError::ApiError(format!(
                "Failed to merge pull request: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }
}

/// Handles an error from the Jules API client.
pub fn handle_error(err: JulesError) {
    match err {
        JulesError::ApiKeyMissing => {
            eprintln!(
                "{} {}",
                "Error:".red(),
                "API key is missing. Please provide it using the --api-key flag or the JULES_API_KEY environment variable."
            );
        }
        JulesError::ReqwestError(e) => {
            eprintln!("{} {}", "Error:".red(), e);
        }
        JulesError::ApiError(e) => {
            eprintln!("{} {}", "Error:".red(), e);
        }
    }
}
