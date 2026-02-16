//! Gmail integration types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Gmail integration configuration stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailConfig {
    pub id: i64,
    /// Email address being watched
    pub email: String,
    /// OAuth2 access token (encrypted in DB)
    pub access_token: String,
    /// OAuth2 refresh token (encrypted in DB)
    pub refresh_token: String,
    /// Token expiration time
    pub token_expires_at: Option<DateTime<Utc>>,
    /// Gmail labels to watch (comma-separated, e.g., "INBOX,IMPORTANT")
    pub watch_labels: String,
    /// Google Cloud Project ID for Pub/Sub
    pub project_id: String,
    /// Pub/Sub topic name
    pub topic_name: String,
    /// Watch expiration (Gmail watches expire after 7 days)
    pub watch_expires_at: Option<DateTime<Utc>>,
    /// History ID from last processed notification
    pub history_id: Option<String>,
    /// Whether this integration is enabled
    pub enabled: bool,
    /// Channel ID to route responses to (optional)
    pub response_channel_id: Option<i64>,
    /// Whether to auto-reply to emails
    pub auto_reply: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Pub/Sub push notification from Gmail
#[derive(Debug, Deserialize)]
pub struct PubSubPushNotification {
    pub message: PubSubMessage,
    pub subscription: String,
}

/// Pub/Sub message wrapper
#[derive(Debug, Deserialize)]
pub struct PubSubMessage {
    /// Base64-encoded message data
    pub data: String,
    /// Message ID from Pub/Sub
    #[serde(rename = "messageId")]
    pub message_id: String,
    /// Publication timestamp
    #[serde(rename = "publishTime")]
    pub publish_time: Option<String>,
    /// Optional attributes
    pub attributes: Option<std::collections::HashMap<String, String>>,
}

/// Decoded Gmail notification data (from Pub/Sub message.data)
#[derive(Debug, Deserialize)]
pub struct GmailNotificationData {
    /// Email address that received the notification
    #[serde(rename = "emailAddress")]
    pub email_address: String,
    /// History ID to fetch changes from
    #[serde(rename = "historyId")]
    pub history_id: String,
}

/// Gmail message from API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailMessage {
    pub id: String,
    pub thread_id: String,
    pub label_ids: Option<Vec<String>>,
    pub snippet: Option<String>,
    pub payload: Option<GmailMessagePayload>,
    pub internal_date: Option<String>,
}

/// Gmail message payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailMessagePayload {
    pub headers: Option<Vec<GmailHeader>>,
    pub parts: Option<Vec<GmailMessagePart>>,
    pub body: Option<GmailMessageBody>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

/// Gmail header (From, To, Subject, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailHeader {
    pub name: String,
    pub value: String,
}

/// Gmail message part (for multipart messages)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailMessagePart {
    #[serde(rename = "partId")]
    pub part_id: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    pub body: Option<GmailMessageBody>,
    pub parts: Option<Vec<GmailMessagePart>>,
}

/// Gmail message body
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailMessageBody {
    /// Base64url-encoded body data
    pub data: Option<String>,
    pub size: Option<i64>,
}

/// History list response from Gmail API
#[derive(Debug, Deserialize)]
pub struct GmailHistoryResponse {
    pub history: Option<Vec<GmailHistoryRecord>>,
    #[serde(rename = "historyId")]
    pub history_id: Option<String>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

/// Single history record
#[derive(Debug, Deserialize)]
pub struct GmailHistoryRecord {
    pub id: String,
    #[serde(rename = "messagesAdded")]
    pub messages_added: Option<Vec<GmailMessageAdded>>,
}

/// Message added in history
#[derive(Debug, Deserialize)]
pub struct GmailMessageAdded {
    pub message: GmailMessageRef,
}

/// Reference to a Gmail message
#[derive(Debug, Deserialize)]
pub struct GmailMessageRef {
    pub id: String,
    #[serde(rename = "threadId")]
    pub thread_id: String,
}

/// Parsed email for processing
#[derive(Debug, Clone, Serialize)]
pub struct ParsedEmail {
    pub message_id: String,
    pub thread_id: String,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub snippet: String,
    pub body: String,
    pub date: Option<String>,
    pub labels: Vec<String>,
}

impl GmailMessage {
    /// Extract a header value by name
    pub fn get_header(&self, name: &str) -> Option<String> {
        self.payload
            .as_ref()?
            .headers
            .as_ref()?
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
            .map(|h| h.value.clone())
    }

    /// Parse the message into a simpler format
    pub fn parse(&self) -> ParsedEmail {
        let from = self.get_header("From").unwrap_or_default();
        let to = self.get_header("To").unwrap_or_default();
        let subject = self.get_header("Subject").unwrap_or_default();
        let date = self.get_header("Date");
        let snippet = self.snippet.clone().unwrap_or_default();
        let body = self.extract_body().unwrap_or_default();
        let labels = self.label_ids.clone().unwrap_or_default();

        ParsedEmail {
            message_id: self.id.clone(),
            thread_id: self.thread_id.clone(),
            from,
            to,
            subject,
            snippet,
            body,
            date,
            labels,
        }
    }

    /// Extract plain text body from message
    fn extract_body(&self) -> Option<String> {
        let payload = self.payload.as_ref()?;

        // Try direct body first
        if let Some(body) = &payload.body {
            if let Some(data) = &body.data {
                if let Ok(decoded) = base64_url_decode(data) {
                    return Some(decoded);
                }
            }
        }

        // Try parts (multipart message)
        if let Some(parts) = &payload.parts {
            return extract_text_from_parts(parts);
        }

        None
    }
}

/// Decode base64url-encoded string (Gmail uses URL-safe base64)
fn base64_url_decode(input: &str) -> Result<String, String> {
    use base64::Engine;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let bytes = engine.decode(input).map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

/// Recursively extract text/plain content from message parts
fn extract_text_from_parts(parts: &[GmailMessagePart]) -> Option<String> {
    for part in parts {
        // Check if this part is text/plain
        if part.mime_type.as_deref() == Some("text/plain") {
            if let Some(body) = &part.body {
                if let Some(data) = &body.data {
                    if let Ok(decoded) = base64_url_decode(data) {
                        return Some(decoded);
                    }
                }
            }
        }

        // Recursively check nested parts
        if let Some(nested_parts) = &part.parts {
            if let Some(text) = extract_text_from_parts(nested_parts) {
                return Some(text);
            }
        }
    }

    // Fallback: try text/html if no text/plain found
    for part in parts {
        if part.mime_type.as_deref() == Some("text/html") {
            if let Some(body) = &part.body {
                if let Some(data) = &body.data {
                    if let Ok(decoded) = base64_url_decode(data) {
                        // Strip HTML tags (basic)
                        let text = strip_html(&decoded);
                        return Some(text);
                    }
                }
            }
        }
    }

    None
}

/// Basic HTML tag stripping
fn strip_html(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;

    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }

    // Clean up whitespace
    result
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Request to set up Gmail integration
#[derive(Debug, Deserialize)]
pub struct SetupGmailRequest {
    pub email: String,
    pub access_token: String,
    pub refresh_token: String,
    pub project_id: String,
    pub topic_name: String,
    pub watch_labels: Option<String>,
    pub response_channel_id: Option<i64>,
    pub auto_reply: Option<bool>,
}

/// Request to update Gmail integration
#[derive(Debug, Deserialize)]
pub struct UpdateGmailRequest {
    pub watch_labels: Option<String>,
    pub response_channel_id: Option<i64>,
    pub auto_reply: Option<bool>,
    pub enabled: Option<bool>,
}

/// Response for Gmail config operations
#[derive(Debug, Serialize)]
pub struct GmailConfigResponse {
    pub success: bool,
    pub config: Option<GmailConfigSummary>,
    pub error: Option<String>,
}

/// Summary of Gmail config (without sensitive tokens)
#[derive(Debug, Serialize)]
pub struct GmailConfigSummary {
    pub id: i64,
    pub email: String,
    pub watch_labels: String,
    pub project_id: String,
    pub topic_name: String,
    pub watch_expires_at: Option<DateTime<Utc>>,
    pub enabled: bool,
    pub auto_reply: bool,
    pub response_channel_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<GmailConfig> for GmailConfigSummary {
    fn from(config: GmailConfig) -> Self {
        Self {
            id: config.id,
            email: config.email,
            watch_labels: config.watch_labels,
            project_id: config.project_id,
            topic_name: config.topic_name,
            watch_expires_at: config.watch_expires_at,
            enabled: config.enabled,
            auto_reply: config.auto_reply,
            response_channel_id: config.response_channel_id,
            created_at: config.created_at,
            updated_at: config.updated_at,
        }
    }
}
