//! Versioned, runtime-swappable resources (prompts, configs).
//!
//! System prompts, model configs, and tool configs become versioned resources
//! stored in SQLite. Each rollout records which `resources_id` it used.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// A single versioned resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// Unique name for this resource (e.g., "system_prompt.assistant")
    pub name: String,
    /// The resource type
    pub resource_type: ResourceType,
    /// The content of the resource
    pub content: String,
    /// Optional structured metadata
    pub metadata: Value,
}

/// The type of a versioned resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    /// A prompt template (e.g., system prompt, planner prompt)
    PromptTemplate,
    /// Model configuration (archetype, parameters)
    ModelConfig,
    /// Tool configuration (allow/deny lists, groups)
    ToolConfig,
}

impl ResourceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResourceType::PromptTemplate => "prompt_template",
            ResourceType::ModelConfig => "model_config",
            ResourceType::ToolConfig => "tool_config",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "prompt_template" => Some(ResourceType::PromptTemplate),
            "model_config" => Some(ResourceType::ModelConfig),
            "tool_config" => Some(ResourceType::ToolConfig),
            _ => None,
        }
    }
}

/// A bundle of resources at a specific version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceBundle {
    /// Unique version identifier
    pub version_id: String,
    /// Human-readable version label (e.g., "v1.2", "prompt-update-2024-01")
    pub label: String,
    /// All resources in this bundle
    pub resources: Vec<Resource>,
    /// Whether this is the active version
    pub is_active: bool,
    /// When this version was created
    pub created_at: DateTime<Utc>,
    /// Optional description of changes
    pub description: Option<String>,
}

impl ResourceBundle {
    /// Create a new resource bundle.
    pub fn new(label: String, resources: Vec<Resource>) -> Self {
        Self {
            version_id: uuid::Uuid::new_v4().to_string(),
            label,
            resources,
            is_active: false,
            created_at: Utc::now(),
            description: None,
        }
    }

    /// Get a resource by name.
    pub fn get(&self, name: &str) -> Option<&Resource> {
        self.resources.iter().find(|r| r.name == name)
    }

    /// Get a prompt template by name, returning its content.
    pub fn get_prompt(&self, name: &str) -> Option<&str> {
        self.resources.iter()
            .find(|r| r.name == name && r.resource_type == ResourceType::PromptTemplate)
            .map(|r| r.content.as_str())
    }
}

/// Manages versioned resources with creation, activation, and rollback.
pub struct ResourceManager {
    db: Arc<crate::db::Database>,
    /// Cache of the current active bundle
    active_cache: parking_lot::RwLock<Option<ResourceBundle>>,
}

impl ResourceManager {
    pub fn new(db: Arc<crate::db::Database>) -> Self {
        Self {
            db,
            active_cache: parking_lot::RwLock::new(None),
        }
    }

    /// Initialize with default resources from compiled-in prompts.
    /// Only creates a version if none exists.
    pub fn seed_defaults(&self) {
        match self.db.get_active_resource_bundle() {
            Ok(Some(_)) => {
                log::info!("[RESOURCES] Active resource bundle already exists, skipping seed");
            }
            Ok(None) => {
                log::info!("[RESOURCES] No active resource bundle found, seeding defaults");
                let resources = vec![
                    Resource {
                        name: "system_prompt.assistant".to_string(),
                        resource_type: ResourceType::PromptTemplate,
                        content: include_str!("../ai/multi_agent/prompts/assistant.md").to_string(),
                        metadata: Value::Null,
                    },
                    Resource {
                        name: "system_prompt.task_planner".to_string(),
                        resource_type: ResourceType::PromptTemplate,
                        content: include_str!("../ai/multi_agent/prompts/task_planner.md").to_string(),
                        metadata: Value::Null,
                    },
                ];

                let mut bundle = ResourceBundle::new("v1.0-default".to_string(), resources);
                bundle.description = Some("Initial seed from compiled-in prompts".to_string());
                bundle.is_active = true;

                if let Err(e) = self.db.create_resource_bundle(&bundle) {
                    log::error!("[RESOURCES] Failed to seed default resource bundle: {}", e);
                }
            }
            Err(e) => {
                log::error!("[RESOURCES] Failed to check existing resource bundles: {}", e);
            }
        }
    }

    /// Create a new version of the resource bundle.
    pub fn create_version(
        &self,
        label: String,
        resources: Vec<Resource>,
        description: Option<String>,
    ) -> Result<ResourceBundle, String> {
        let mut bundle = ResourceBundle::new(label, resources);
        bundle.description = description;

        self.db.create_resource_bundle(&bundle)
            .map_err(|e| format!("Failed to create resource bundle: {}", e))?;

        Ok(bundle)
    }

    /// Activate a specific version, deactivating all others.
    pub fn activate_version(&self, version_id: &str) -> Result<(), String> {
        self.db.activate_resource_bundle(version_id)
            .map_err(|e| format!("Failed to activate resource bundle: {}", e))?;

        // Invalidate cache
        *self.active_cache.write() = None;
        Ok(())
    }

    /// Get the currently active resource bundle.
    pub fn get_active(&self) -> Option<ResourceBundle> {
        // Check cache first
        if let Some(ref cached) = *self.active_cache.read() {
            return Some(cached.clone());
        }

        // Load from database
        match self.db.get_active_resource_bundle() {
            Ok(Some(bundle)) => {
                *self.active_cache.write() = Some(bundle.clone());
                Some(bundle)
            }
            Ok(None) => None,
            Err(e) => {
                log::error!("[RESOURCES] Failed to load active resource bundle: {}", e);
                None
            }
        }
    }

    /// Get the latest version (active or not).
    pub fn get_latest(&self) -> Option<ResourceBundle> {
        match self.db.get_latest_resource_bundle() {
            Ok(bundle) => bundle,
            Err(e) => {
                log::error!("[RESOURCES] Failed to get latest resource bundle: {}", e);
                None
            }
        }
    }

    /// Rollback to a previous version by its version_id.
    pub fn rollback(&self, version_id: &str) -> Result<(), String> {
        self.activate_version(version_id)
    }

    /// List all resource bundle versions.
    pub fn list_versions(&self) -> Vec<ResourceBundle> {
        match self.db.list_resource_bundles() {
            Ok(bundles) => bundles,
            Err(e) => {
                log::error!("[RESOURCES] Failed to list resource bundles: {}", e);
                Vec::new()
            }
        }
    }

    /// Resolve a prompt by name, falling back to compile-time default.
    pub fn resolve_prompt(&self, name: &str) -> String {
        if let Some(bundle) = self.get_active() {
            if let Some(content) = bundle.get_prompt(name) {
                return content.to_string();
            }
        }

        // Fallback to compiled-in defaults
        match name {
            "system_prompt.assistant" => {
                include_str!("../ai/multi_agent/prompts/assistant.md").to_string()
            }
            "system_prompt.task_planner" => {
                include_str!("../ai/multi_agent/prompts/task_planner.md").to_string()
            }
            _ => {
                log::warn!("[RESOURCES] Unknown prompt '{}', returning empty", name);
                String::new()
            }
        }
    }

    /// Get the version ID of the currently active bundle (for rollout tracking).
    pub fn active_version_id(&self) -> Option<String> {
        self.get_active().map(|b| b.version_id)
    }
}
