//! Gmail API client

use super::types::*;
use reqwest::Client;
use serde_json::json;

const GMAIL_API_BASE: &str = "https://gmail.googleapis.com/gmail/v1";
const TOKEN_REFRESH_URL: &str = "https://oauth2.googleapis.com/token";

/// Gmail API client
pub struct GmailClient {
    http: Client,
    access_token: String,
    refresh_token: String,
    client_id: Option<String>,
    client_secret: Option<String>,
}

impl GmailClient {
    /// Create a new Gmail client with tokens
    pub fn new(access_token: String, refresh_token: String) -> Self {
        Self {
            http: crate::http::shared_client().clone(),
            access_token,
            refresh_token,
            client_id: None,
            client_secret: None,
        }
    }

    /// Set OAuth client credentials for token refresh
    pub fn with_client_credentials(mut self, client_id: String, client_secret: String) -> Self {
        self.client_id = Some(client_id);
        self.client_secret = Some(client_secret);
        self
    }

    /// Get the current access token
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    /// Refresh the access token using the refresh token
    pub async fn refresh_access_token(&mut self) -> Result<String, String> {
        let client_id = self.client_id.as_ref()
            .ok_or("Client ID not set for token refresh")?;
        let client_secret = self.client_secret.as_ref()
            .ok_or("Client secret not set for token refresh")?;

        let response = self.http
            .post(TOKEN_REFRESH_URL)
            .form(&[
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("refresh_token", &self.refresh_token),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .map_err(|e| format!("Failed to refresh token: {}", e))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(format!("Token refresh failed: {}", error));
        }

        #[derive(serde::Deserialize)]
        struct TokenResponse {
            access_token: String,
        }

        let token_response: TokenResponse = response.json().await
            .map_err(|e| format!("Failed to parse token response: {}", e))?;

        self.access_token = token_response.access_token.clone();
        Ok(token_response.access_token)
    }

    /// Get message history since a specific history ID
    pub async fn get_history(
        &self,
        user_id: &str,
        start_history_id: &str,
        label_ids: Option<&[&str]>,
    ) -> Result<GmailHistoryResponse, String> {
        let mut url = format!(
            "{}/users/{}/history?startHistoryId={}",
            GMAIL_API_BASE, user_id, start_history_id
        );

        if let Some(labels) = label_ids {
            for label in labels {
                url.push_str(&format!("&labelId={}", label));
            }
        }

        let response = self.http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await
            .map_err(|e| format!("Failed to get history: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            return Err(format!("Gmail API error ({}): {}", status, error));
        }

        response.json().await
            .map_err(|e| format!("Failed to parse history response: {}", e))
    }

    /// Get a specific message by ID
    pub async fn get_message(
        &self,
        user_id: &str,
        message_id: &str,
        format: Option<&str>,
    ) -> Result<GmailMessage, String> {
        let format = format.unwrap_or("full");
        let url = format!(
            "{}/users/{}/messages/{}?format={}",
            GMAIL_API_BASE, user_id, message_id, format
        );

        let response = self.http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await
            .map_err(|e| format!("Failed to get message: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            return Err(format!("Gmail API error ({}): {}", status, error));
        }

        response.json().await
            .map_err(|e| format!("Failed to parse message response: {}", e))
    }

    /// Set up a watch on the mailbox
    pub async fn setup_watch(
        &self,
        user_id: &str,
        topic_name: &str,
        label_ids: &[&str],
    ) -> Result<WatchResponse, String> {
        let url = format!("{}/users/{}/watch", GMAIL_API_BASE, user_id);

        let body = json!({
            "topicName": topic_name,
            "labelIds": label_ids,
            "labelFilterBehavior": "INCLUDE"
        });

        let response = self.http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| format!("Failed to setup watch: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            return Err(format!("Gmail API error ({}): {}", status, error));
        }

        response.json().await
            .map_err(|e| format!("Failed to parse watch response: {}", e))
    }

    /// Stop watching the mailbox
    pub async fn stop_watch(&self, user_id: &str) -> Result<(), String> {
        let url = format!("{}/users/{}/stop", GMAIL_API_BASE, user_id);

        let response = self.http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await
            .map_err(|e| format!("Failed to stop watch: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            return Err(format!("Gmail API error ({}): {}", status, error));
        }

        Ok(())
    }

    /// Send a reply to an email
    pub async fn send_reply(
        &self,
        user_id: &str,
        thread_id: &str,
        to: &str,
        subject: &str,
        body: &str,
        in_reply_to: Option<&str>,
    ) -> Result<GmailMessage, String> {
        // Build RFC 2822 message
        let mut message = format!(
            "To: {}\r\nSubject: Re: {}\r\nContent-Type: text/plain; charset=utf-8\r\n",
            to, subject
        );

        if let Some(msg_id) = in_reply_to {
            message.push_str(&format!("In-Reply-To: {}\r\nReferences: {}\r\n", msg_id, msg_id));
        }

        message.push_str(&format!("\r\n{}", body));

        // Base64url encode the message
        use base64::Engine;
        let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let encoded = engine.encode(message.as_bytes());

        let url = format!("{}/users/{}/messages/send", GMAIL_API_BASE, user_id);

        let request_body = json!({
            "raw": encoded,
            "threadId": thread_id
        });

        let response = self.http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Content-Type", "application/json")
            .body(request_body.to_string())
            .send()
            .await
            .map_err(|e| format!("Failed to send reply: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            return Err(format!("Gmail API error ({}): {}", status, error));
        }

        response.json().await
            .map_err(|e| format!("Failed to parse send response: {}", e))
    }

    /// Get user profile (email address)
    pub async fn get_profile(&self, user_id: &str) -> Result<UserProfile, String> {
        let url = format!("{}/users/{}/profile", GMAIL_API_BASE, user_id);

        let response = self.http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await
            .map_err(|e| format!("Failed to get profile: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            return Err(format!("Gmail API error ({}): {}", status, error));
        }

        response.json().await
            .map_err(|e| format!("Failed to parse profile response: {}", e))
    }
}

/// Watch response from Gmail API
#[derive(Debug, serde::Deserialize)]
pub struct WatchResponse {
    #[serde(rename = "historyId")]
    pub history_id: String,
    pub expiration: String,
}

/// User profile from Gmail API
#[derive(Debug, serde::Deserialize)]
pub struct UserProfile {
    #[serde(rename = "emailAddress")]
    pub email_address: String,
    #[serde(rename = "messagesTotal")]
    pub messages_total: Option<i64>,
    #[serde(rename = "threadsTotal")]
    pub threads_total: Option<i64>,
    #[serde(rename = "historyId")]
    pub history_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = GmailClient::new("access".to_string(), "refresh".to_string());
        assert_eq!(client.access_token(), "access");
    }
}
