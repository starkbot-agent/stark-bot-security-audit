use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::controllers::api_keys::get_key_config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: i64,
    pub service_name: String,  // Stores key names like "GITHUB_TOKEN", "MOLTX_API_KEY"
    #[serde(skip_serializing)]
    pub api_key: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ApiKey {
    /// Convert to response with masked key
    pub fn to_response(&self) -> ApiKeyResponse {
        // Get key config to determine if it's secret
        let (key_preview, is_secret) = match get_key_config(&self.service_name) {
            Some((_, config)) => {
                if config.secret {
                    (mask_key(&self.api_key), true)
                } else {
                    (self.api_key.clone(), false)
                }
            }
            // Default to masking if config not found
            None => (mask_key(&self.api_key), true),
        };

        ApiKeyResponse {
            id: self.id,
            key_name: self.service_name.clone(),
            key_preview,
            is_secret,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// Mask a key value for display
fn mask_key(value: &str) -> String {
    if value.len() > 12 {
        let start = &value[..4];
        let end = &value[value.len() - 4..];
        format!("{}...{}", start, end)
    } else {
        "****".to_string()
    }
}

/// Response version with masked key
#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyResponse {
    pub id: i64,
    pub key_name: String,
    pub key_preview: String,
    pub is_secret: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
