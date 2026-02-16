//! In-memory cache layer for hot-path database queries.
//!
//! Uses moka::sync::Cache to avoid hitting SQLite on every request for
//! nearly-static data like bot settings, API keys, channels, etc.

use std::sync::Arc;
use std::time::Duration;

use moka::sync::Cache;

use crate::models::{AgentSettings, ApiKey, BotSettings, Channel, ChannelSetting};
use crate::tools::ToolConfig;

/// Short TTL for config data (bot settings, agent settings, API keys, tool configs)
const CONFIG_TTL: Duration = Duration::from_secs(300); // 5 min
/// Shorter TTL for channel data (changes more frequently)
const CHANNEL_TTL: Duration = Duration::from_secs(120); // 2 min

/// In-memory cache for frequently-read, rarely-written database data.
///
/// All caches use string keys for simplicity. Values are wrapped in Arc
/// to make cloning cheap for large collections.
pub struct DbCache {
    /// Singleton cache: key "bot_settings" → BotSettings
    bot_settings: Cache<&'static str, BotSettings>,

    /// Singleton cache: key "active" → Option<AgentSettings>
    agent_settings: Cache<&'static str, Option<AgentSettings>>,

    /// Per-service: key = service_name → Option<ApiKey>
    api_keys: Cache<String, Option<ApiKey>>,

    /// Tool configs: key = "global" or "channel:{id}" → Option<ToolConfig>
    tool_configs: Cache<String, Option<ToolConfig>>,

    /// Channel by ID: key = channel id → Option<Channel>
    channels: Cache<i64, Option<Channel>>,

    /// Enabled channels list: singleton key "enabled" → Vec<Channel>
    enabled_channels: Cache<&'static str, Arc<Vec<Channel>>>,

    /// Channel settings: key = channel id → Vec<ChannelSetting>
    channel_settings: Cache<i64, Arc<Vec<ChannelSetting>>>,

    /// Single channel setting: key = "channel_id:key" → Option<String>
    channel_setting_values: Cache<String, Option<String>>,
}

impl DbCache {
    /// Create a new cache with default TTLs and reasonable max capacities.
    pub fn new() -> Self {
        Self {
            bot_settings: Cache::builder()
                .time_to_live(CONFIG_TTL)
                .max_capacity(1)
                .build(),
            agent_settings: Cache::builder()
                .time_to_live(CONFIG_TTL)
                .max_capacity(1)
                .build(),
            api_keys: Cache::builder()
                .time_to_live(CONFIG_TTL)
                .max_capacity(64)
                .build(),
            tool_configs: Cache::builder()
                .time_to_live(CONFIG_TTL)
                .max_capacity(128)
                .build(),
            channels: Cache::builder()
                .time_to_live(CHANNEL_TTL)
                .max_capacity(128)
                .build(),
            enabled_channels: Cache::builder()
                .time_to_live(CHANNEL_TTL)
                .max_capacity(1)
                .build(),
            channel_settings: Cache::builder()
                .time_to_live(CHANNEL_TTL)
                .max_capacity(128)
                .build(),
            channel_setting_values: Cache::builder()
                .time_to_live(CHANNEL_TTL)
                .max_capacity(512)
                .build(),
        }
    }

    // ── Bot settings ────────────────────────────────────────

    pub fn get_bot_settings(&self) -> Option<BotSettings> {
        self.bot_settings.get(&"bot_settings")
    }

    pub fn set_bot_settings(&self, settings: BotSettings) {
        self.bot_settings.insert("bot_settings", settings);
    }

    pub fn invalidate_bot_settings(&self) {
        self.bot_settings.invalidate(&"bot_settings");
    }

    // ── Agent settings ──────────────────────────────────────

    pub fn get_active_agent_settings(&self) -> Option<Option<AgentSettings>> {
        self.agent_settings.get(&"active")
    }

    pub fn set_active_agent_settings(&self, settings: Option<AgentSettings>) {
        self.agent_settings.insert("active", settings);
    }

    pub fn invalidate_agent_settings(&self) {
        self.agent_settings.invalidate(&"active");
    }

    // ── API keys ────────────────────────────────────────────

    pub fn get_api_key(&self, service: &str) -> Option<Option<ApiKey>> {
        self.api_keys.get(&service.to_string())
    }

    pub fn set_api_key(&self, service: &str, key: Option<ApiKey>) {
        self.api_keys.insert(service.to_string(), key);
    }

    pub fn invalidate_api_key(&self, service: &str) {
        self.api_keys.invalidate(&service.to_string());
    }

    // ── Tool configs ────────────────────────────────────────

    pub fn get_global_tool_config(&self) -> Option<Option<ToolConfig>> {
        self.tool_configs.get(&"global".to_string())
    }

    pub fn set_global_tool_config(&self, config: Option<ToolConfig>) {
        self.tool_configs.insert("global".to_string(), config);
    }

    pub fn get_channel_tool_config(&self, channel_id: i64) -> Option<Option<ToolConfig>> {
        self.tool_configs.get(&format!("channel:{}", channel_id))
    }

    pub fn set_channel_tool_config(&self, channel_id: i64, config: Option<ToolConfig>) {
        self.tool_configs.insert(format!("channel:{}", channel_id), config);
    }

    pub fn invalidate_tool_configs(&self) {
        self.tool_configs.invalidate_all();
    }

    // ── Channels ────────────────────────────────────────────

    pub fn get_channel(&self, id: i64) -> Option<Option<Channel>> {
        self.channels.get(&id)
    }

    pub fn set_channel(&self, id: i64, channel: Option<Channel>) {
        self.channels.insert(id, channel);
    }

    pub fn get_enabled_channels(&self) -> Option<Arc<Vec<Channel>>> {
        self.enabled_channels.get(&"enabled")
    }

    pub fn set_enabled_channels(&self, channels: Vec<Channel>) {
        self.enabled_channels.insert("enabled", Arc::new(channels));
    }

    pub fn invalidate_channels(&self) {
        self.channels.invalidate_all();
        self.enabled_channels.invalidate_all();
    }

    // ── Channel settings ────────────────────────────────────

    pub fn get_channel_settings(&self, channel_id: i64) -> Option<Arc<Vec<ChannelSetting>>> {
        self.channel_settings.get(&channel_id)
    }

    pub fn set_channel_settings(&self, channel_id: i64, settings: Vec<ChannelSetting>) {
        self.channel_settings.insert(channel_id, Arc::new(settings));
    }

    pub fn get_channel_setting_value(&self, channel_id: i64, key: &str) -> Option<Option<String>> {
        self.channel_setting_values.get(&format!("{}:{}", channel_id, key))
    }

    pub fn set_channel_setting_value(&self, channel_id: i64, key: &str, value: Option<String>) {
        self.channel_setting_values.insert(format!("{}:{}", channel_id, key), value);
    }

    pub fn invalidate_channel_settings(&self, channel_id: i64) {
        self.channel_settings.invalidate(&channel_id);
        // Invalidate all individual setting values for this channel
        // Since we can't enumerate keys by prefix in moka, just invalidate all
        self.channel_setting_values.invalidate_all();
    }

    pub fn invalidate_all_channel_settings(&self) {
        self.channel_settings.invalidate_all();
        self.channel_setting_values.invalidate_all();
    }
}
