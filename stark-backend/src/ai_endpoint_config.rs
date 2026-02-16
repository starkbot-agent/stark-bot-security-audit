use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

static AI_ENDPOINTS: OnceLock<HashMap<String, AiEndpointPreset>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiEndpointPreset {
    pub display_name: String,
    pub endpoint: String,
    pub model_archetype: String,
    #[serde(default)]
    pub x402_cost: Option<u64>,
}

pub fn load_ai_endpoints(config_dir: &Path) {
    let config_path = config_dir.join("ai_endpoints.ron");

    let endpoints = if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(content) => match ron::from_str::<HashMap<String, AiEndpointPreset>>(&content) {
                Ok(endpoints) => {
                    log::info!(
                        "Loaded {} AI endpoint presets from config: {:?}",
                        endpoints.len(),
                        endpoints.keys().collect::<Vec<_>>()
                    );
                    endpoints
                }
                Err(e) => {
                    log::error!("Failed to parse ai_endpoints.ron: {}", e);
                    default_endpoints()
                }
            },
            Err(e) => {
                log::error!("Failed to read ai_endpoints.ron: {}", e);
                default_endpoints()
            }
        }
    } else {
        log::info!("No ai_endpoints.ron found, using defaults");
        default_endpoints()
    };

    if AI_ENDPOINTS.set(endpoints).is_err() {
        log::warn!("AI endpoints already initialized");
    }
}

fn default_endpoints() -> HashMap<String, AiEndpointPreset> {
    let mut endpoints = HashMap::new();
    endpoints.insert(
        "kimi".to_string(),
        AiEndpointPreset {
            display_name: "kimi.defirelay.com".to_string(),
            endpoint: "https://kimi.defirelay.com/api/v1/chat/completions".to_string(),
            model_archetype: "kimi".to_string(),
            x402_cost: None,
        },
    );
    endpoints.insert(
        "llama".to_string(),
        AiEndpointPreset {
            display_name: "llama.defirelay.com".to_string(),
            endpoint: "https://llama.defirelay.com/api/v1/chat/completions".to_string(),
            model_archetype: "llama".to_string(),
            x402_cost: None,
        },
    );
    endpoints
}

pub fn list_ai_endpoints() -> Vec<(String, AiEndpointPreset)> {
    AI_ENDPOINTS
        .get()
        .map(|endpoints| {
            let mut list: Vec<_> = endpoints
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            list.sort_by(|a, b| a.0.cmp(&b.0));
            list
        })
        .unwrap_or_default()
}
