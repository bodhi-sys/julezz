use colored::Colorize;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize, Deserialize)]
pub struct Activity {
    pub name: String,
    pub id: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ListActivitiesResponse {
    activities: Vec<Activity>,
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

    pub async fn create_session(&self, source: &str) -> Result<Session, JulesError> {
        let url = format!("{}/sessions", API_BASE_URL);
        let response = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&serde_json::json!({ "source": source }))
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

    pub async fn send_message(&self, id: &str, message: &str) -> Result<(), JulesError> {
        let url = format!("{}/sessions/{}:sendMessage", API_BASE_URL, id);
        let response = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&serde_json::json!({ "message": message }))
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

    pub async fn list_activities(
        &self,
        session_id: &str,
    ) -> Result<Vec<Activity>, JulesError> {
        let url = format!(
            "{}/sessions/{}/activities",
            API_BASE_URL, session_id
        );
        let response = self
            .client
            .get(&url)
            .header("x-goog-api-key", &self.api_key)
            .send()
            .await?;
        let list_response = self
            .handle_response::<ListActivitiesResponse>(response)
            .await?;
        Ok(list_response.activities)
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
