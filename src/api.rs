use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;

const API_BASE_URL: &str = "https://jules.googleapis.com/v1alpha";

#[derive(Debug, Serialize, Deserialize)]
pub struct Source {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ListSourcesResponse {
    sources: Vec<Source>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SourceContext {
    pub source: String,
    #[serde(rename = "githubRepoContext")]
    pub github_repo_context: Option<GithubRepoContext>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubRepoContext {
    pub starting_branch: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    pub name: String,
    pub id: String,
    pub state: String,
    pub title: String,
    #[serde(rename = "sourceContext")]
    pub source_context: Option<SourceContext>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ListSessionsResponse {
    sessions: Vec<Session>,
}

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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlanApproved {}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressUpdated {
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserMessaged {
    pub user_message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessaged {
    pub agent_message: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListActivitiesResponse {
    activities: Vec<Activity>,
    next_page_token: Option<String>,
}

#[derive(Debug)]
pub enum JulesError {
    ApiKeyMissing,
    ReqwestError(reqwest::Error),
    ApiError(String),
}

impl From<reqwest::Error> for JulesError {
    fn from(err: reqwest::Error) -> JulesError {
        JulesError::ReqwestError(err)
    }
}

pub struct JulesClient {
    api_key: String,
    client: reqwest::Client,
}

impl JulesClient {
    pub fn new(api_key: Option<String>) -> Result<Self, JulesError> {
        let api_key = api_key.ok_or(JulesError::ApiKeyMissing)?;
        Ok(Self {
            api_key,
            client: reqwest::Client::new(),
        })
    }

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

    pub async fn create_session(&self, source: &str, auto_pr: bool) -> Result<Session, JulesError> {
        let url = format!("{}/sessions", API_BASE_URL);
        let mut json_body = serde_json::json!({ "source": source });
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

    pub async fn approve_plan(&self, id: &str) -> Result<(), JulesError> {
        let url = format!("{}/sessions/{}:approvePlan", API_BASE_URL, id);
        let response = self
            .client
            .post(&url)
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

    pub fn list_cached_activities(&self, session_id: &str) -> Result<Vec<Activity>, JulesError> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| JulesError::ApiError("Could not determine cache directory".to_string()))?
            .join("julezz")
            .join(session_id);

        let messages_path = cache_dir.join("messages.json");
        let last_page_path = cache_dir.join("last_page.json");

        let mut activities: Vec<Activity> = if messages_path.exists() {
            let data = fs::read_to_string(&messages_path).map_err(|e| JulesError::ApiError(format!("Could not read messages file: {}", e)))?;
            serde_json::from_str(&data).map_err(|e| JulesError::ApiError(format!("Could not parse messages file: {}", e)))?
        } else {
            Vec::new()
        };

        if last_page_path.exists() {
            let data = fs::read_to_string(&last_page_path).map_err(|e| JulesError::ApiError(format!("Could not read last page file: {}", e)))?;
            let last_page_activities: Vec<Activity> = serde_json::from_str(&data).map_err(|e| JulesError::ApiError(format!("Could not parse last page file: {}", e)))?;
            activities.extend(last_page_activities);
        }

        Ok(activities)
    }

    pub async fn fetch_activities(
        &self,
        session_id: &str,
    ) -> Result<Vec<Activity>, JulesError> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| JulesError::ApiError("Could not determine cache directory".to_string()))?
            .join("julezz")
            .join(session_id);
        fs::create_dir_all(&cache_dir).map_err(|e| JulesError::ApiError(format!("Could not create cache directory: {}", e)))?;

        let messages_path = cache_dir.join("messages.json");
        let last_page_path = cache_dir.join("last_page.json");
        let page_token_path = cache_dir.join("page_token.json");

        let mut stable_activities: Vec<Activity> = if messages_path.exists() {
            let data = fs::read_to_string(&messages_path).map_err(|e| JulesError::ApiError(format!("Could not read messages file: {}", e)))?;
            serde_json::from_str(&data).map_err(|e| JulesError::ApiError(format!("Could not parse messages file: {}", e)))?
        } else {
            Vec::new()
        };

        let mut page_token: Option<String> = if page_token_path.exists() {
            fs::read_to_string(&page_token_path).map_err(|e| JulesError::ApiError(format!("Could not read page token file: {}", e))).ok()
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

        stable_activities.extend(new_activities_to_make_stable.clone());
        fs::write(&messages_path, serde_json::to_string(&stable_activities).map_err(|e| JulesError::ApiError(format!("Could not serialize messages: {}", e)))?).map_err(|e| JulesError::ApiError(format!("Could not write messages file: {}", e)))?;

        fs::write(&last_page_path, serde_json::to_string(&last_page_activities).map_err(|e| JulesError::ApiError(format!("Could not serialize last page activities: {}", e)))?).map_err(|e| JulesError::ApiError(format!("Could not write last page file: {}", e)))?;

        if let Some(token) = last_page_token {
            fs::write(&page_token_path, token).map_err(|e| JulesError::ApiError(format!("Could not write page token file: {}", e)))?;
        } else {
            if page_token_path.exists() {
                 fs::remove_file(&page_token_path).map_err(|e| JulesError::ApiError(format!("Could not remove page token file: {}", e)))?;
            }
        }

        stable_activities.extend(last_page_activities);
        Ok(stable_activities)
    }

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
}

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
