//! Social Monitor module — tracks Twitter/X account activity, topics, and sentiment
//!
//! Delegates to the standalone social-monitor-service via RPC.
//! The service must be running separately on SOCIAL_MONITOR_URL (default: http://127.0.0.1:9102).

use async_trait::async_trait;
use crate::db::Database;
use crate::integrations::social_monitor_client::SocialMonitorClient;
use crate::tools::builtin::social_media::social_monitor::{
    SocialMonitorControlTool, SocialMonitorForensicsTool, SocialMonitorTweetsTool,
    SocialMonitorWatchlistTool,
};
use crate::tools::registry::Tool;
use serde_json::{json, Value};
use std::sync::Arc;

pub struct SocialMonitorModule;

impl SocialMonitorModule {
    fn make_client() -> Arc<SocialMonitorClient> {
        let url = Self::url_from_env();
        Arc::new(SocialMonitorClient::new(&url))
    }

    fn url_from_env() -> String {
        std::env::var("SOCIAL_MONITOR_URL")
            .unwrap_or_else(|_| {
                let port = std::env::var("SOCIAL_MONITOR_PORT")
                    .unwrap_or_else(|_| "9102".to_string());
                format!("http://127.0.0.1:{}", port)
            })
    }
}

#[async_trait]
impl super::Module for SocialMonitorModule {
    fn name(&self) -> &'static str {
        "social_monitor"
    }

    fn description(&self) -> &'static str {
        "Monitor Twitter/X accounts for tweet activity, topic trends, and sentiment analysis"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn default_port(&self) -> u16 {
        9102
    }

    fn service_url(&self) -> String {
        Self::url_from_env()
    }

    fn has_tools(&self) -> bool {
        true
    }

    fn has_dashboard(&self) -> bool {
        true
    }

    fn create_tools(&self) -> Vec<Arc<dyn Tool>> {
        let client = Self::make_client();
        vec![
            Arc::new(SocialMonitorWatchlistTool::new(client.clone())),
            Arc::new(SocialMonitorTweetsTool::new(client.clone())),
            Arc::new(SocialMonitorForensicsTool::new(client.clone())),
            Arc::new(SocialMonitorControlTool::new(client)),
        ]
    }

    fn skill_content(&self) -> Option<&'static str> {
        Some(include_str!("social_monitor.md"))
    }

    async fn dashboard_data(&self, _db: &Database) -> Option<Value> {
        let client = Self::make_client();
        let accounts = client.list_accounts().await.ok()?;
        let stats = client.get_tweet_stats().await.ok()?;
        let filter = social_monitor_types::TweetFilter {
            limit: Some(10),
            ..Default::default()
        };
        let recent = client.query_tweets(&filter).await.ok()?;

        let accounts_json: Vec<Value> = accounts
            .iter()
            .map(|a| {
                json!({
                    "id": a.id,
                    "username": a.username,
                    "display_name": a.display_name,
                    "monitor_enabled": a.monitor_enabled,
                    "total_tweets_captured": a.total_tweets_captured,
                    "last_checked_at": a.last_checked_at,
                })
            })
            .collect();

        let recent_tweets_json: Vec<Value> = recent
            .iter()
            .map(|t| {
                json!({
                    "tweet_id": t.tweet_id,
                    "text": t.text,
                    "tweet_type": t.tweet_type,
                    "tweeted_at": t.tweeted_at,
                    "like_count": t.like_count,
                    "retweet_count": t.retweet_count,
                })
            })
            .collect();

        Some(json!({
            "monitored_accounts": stats.monitored_accounts,
            "active_accounts": stats.active_accounts,
            "total_tweets": stats.total_tweets,
            "tweets_today": stats.tweets_today,
            "tweets_7d": stats.tweets_7d,
            "unique_topics": stats.unique_topics,
            "accounts": accounts_json,
            "recent_tweets": recent_tweets_json,
        }))
    }

    async fn backup_data(&self, _db: &Database) -> Option<Value> {
        let client = Self::make_client();
        let data = client.backup_export().await.ok()?;
        if data.accounts.is_empty() && data.keywords.is_empty() {
            return None;
        }
        Some(serde_json::to_value(&data).ok()?)
    }

    async fn restore_data(&self, _db: &Database, data: &Value) -> Result<(), String> {
        let backup_data: social_monitor_types::BackupData =
            serde_json::from_value(data.clone())
                .map_err(|e| format!("Invalid social_monitor backup data: {}", e))?;

        if backup_data.accounts.is_empty() && backup_data.keywords.is_empty() {
            return Ok(());
        }

        let client = Self::make_client();
        let restored = client.backup_restore(backup_data).await?;

        log::info!(
            "[social_monitor] Restored {} entries from backup",
            restored
        );
        Ok(())
    }
}
