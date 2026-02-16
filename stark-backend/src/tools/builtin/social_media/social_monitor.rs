//! Social monitoring tools — account management, tweet queries, forensics, and control
//!
//! These tools are only registered when the social_monitor module is installed.
//! All operations go through the social-monitor-service via RPC.

use crate::integrations::social_monitor_client::SocialMonitorClient;
use crate::tools::registry::Tool;
use crate::tools::types::{
    PropertySchema, ToolContext, ToolDefinition, ToolGroup, ToolInputSchema, ToolResult,
    ToolSafetyLevel,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

// =====================================================
// SocialMonitorWatchlistTool
// =====================================================

pub struct SocialMonitorWatchlistTool {
    definition: ToolDefinition,
    client: Arc<SocialMonitorClient>,
}

impl SocialMonitorWatchlistTool {
    pub fn new(client: Arc<SocialMonitorClient>) -> Self {
        let mut properties = HashMap::new();

        properties.insert(
            "action".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Action: 'add_account', 'remove_account', 'list_accounts', 'update_account', 'add_keyword', 'remove_keyword', 'list_keywords'".to_string(),
                default: None,
                items: None,
                enum_values: Some(vec![
                    "add_account".to_string(),
                    "remove_account".to_string(),
                    "list_accounts".to_string(),
                    "update_account".to_string(),
                    "add_keyword".to_string(),
                    "remove_keyword".to_string(),
                    "list_keywords".to_string(),
                ]),
            },
        );

        properties.insert(
            "username".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Twitter/X username (with or without @). Required for 'add_account'.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "id".to_string(),
            PropertySchema {
                schema_type: "integer".to_string(),
                description: "Entry ID. Required for 'remove_account', 'update_account', 'remove_keyword'.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "notes".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Notes about this account".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "monitor_enabled".to_string(),
            PropertySchema {
                schema_type: "boolean".to_string(),
                description: "Enable/disable monitoring for this account".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "custom_keywords".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Comma-separated custom keywords to track for this account".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "keyword".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Keyword to track. Required for 'add_keyword'.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "category".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Keyword category: 'nft_collection', 'protocol', 'person', 'token'".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "aliases".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Comma-separated aliases for the keyword".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        SocialMonitorWatchlistTool {
            definition: ToolDefinition {
                name: "social_monitor_watchlist".to_string(),
                description: "Manage the social monitor watchlist. Add/remove Twitter/X accounts to monitor, manage tracked keywords.".to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec!["action".to_string()],
                },
                group: ToolGroup::Social,
                hidden: false,
            },
            client,
        }
    }
}

#[derive(Debug, Deserialize)]
struct WatchlistParams {
    action: String,
    username: Option<String>,
    id: Option<i64>,
    notes: Option<String>,
    monitor_enabled: Option<bool>,
    custom_keywords: Option<String>,
    keyword: Option<String>,
    category: Option<String>,
    aliases: Option<String>,
}

#[async_trait]
impl Tool for SocialMonitorWatchlistTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, _context: &ToolContext) -> ToolResult {
        let params: WatchlistParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        match params.action.as_str() {
            "add_account" => {
                let username = match params.username {
                    Some(ref u) => u,
                    None => return ToolResult::error("'username' is required for 'add_account'"),
                };
                match self
                    .client
                    .add_account(
                        username,
                        params.notes.as_deref(),
                        params.custom_keywords.as_deref(),
                    )
                    .await
                {
                    Ok(entry) => ToolResult::success(
                        json!({
                            "status": "added",
                            "id": entry.id,
                            "username": entry.username,
                            "display_name": entry.display_name,
                            "twitter_user_id": entry.twitter_user_id,
                        })
                        .to_string(),
                    ),
                    Err(e) => ToolResult::error(format!("Failed to add account: {}", e)),
                }
            }

            "remove_account" => {
                let id = match params.id {
                    Some(id) => id,
                    None => return ToolResult::error("'id' is required for 'remove_account'"),
                };
                match self.client.remove_account(id).await {
                    Ok(_) => ToolResult::success(format!("Removed account #{}", id)),
                    Err(e) => ToolResult::error(e),
                }
            }

            "list_accounts" => match self.client.list_accounts().await {
                Ok(entries) => {
                    if entries.is_empty() {
                        return ToolResult::success(
                            "No accounts being monitored. Use action='add_account' to start.",
                        );
                    }
                    let mut output = format!(
                        "**Monitored Accounts** ({} entries)\n\n",
                        entries.len()
                    );
                    for e in &entries {
                        let status = if e.monitor_enabled {
                            "active"
                        } else {
                            "paused"
                        };
                        let last = e
                            .last_checked_at
                            .as_deref()
                            .unwrap_or("not yet checked");
                        output.push_str(&format!(
                            "#{} | @{} ({}) | {} tweets | {} | last: {}\n",
                            e.id,
                            e.username,
                            e.display_name.as_deref().unwrap_or("-"),
                            e.total_tweets_captured,
                            status,
                            last
                        ));
                    }
                    ToolResult::success(output)
                }
                Err(e) => ToolResult::error(format!("Failed to list accounts: {}", e)),
            },

            "update_account" => {
                let id = match params.id {
                    Some(id) => id,
                    None => return ToolResult::error("'id' is required for 'update_account'"),
                };
                match self
                    .client
                    .update_account(
                        id,
                        params.monitor_enabled,
                        params.custom_keywords.as_deref(),
                        params.notes.as_deref(),
                    )
                    .await
                {
                    Ok(_) => ToolResult::success(format!("Updated account #{}", id)),
                    Err(e) => ToolResult::error(e),
                }
            }

            "add_keyword" => {
                let keyword = match params.keyword {
                    Some(ref k) => k,
                    None => return ToolResult::error("'keyword' is required for 'add_keyword'"),
                };
                let aliases: Option<Vec<String>> = params.aliases.as_ref().map(|a| {
                    a.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                });
                match self
                    .client
                    .add_keyword(keyword, params.category.as_deref(), aliases)
                    .await
                {
                    Ok(entry) => ToolResult::success(
                        json!({
                            "status": "added",
                            "id": entry.id,
                            "keyword": entry.keyword,
                            "category": entry.category,
                        })
                        .to_string(),
                    ),
                    Err(e) => ToolResult::error(format!("Failed to add keyword: {}", e)),
                }
            }

            "remove_keyword" => {
                let id = match params.id {
                    Some(id) => id,
                    None => return ToolResult::error("'id' is required for 'remove_keyword'"),
                };
                match self.client.remove_keyword(id).await {
                    Ok(_) => ToolResult::success(format!("Removed keyword #{}", id)),
                    Err(e) => ToolResult::error(e),
                }
            }

            "list_keywords" => match self.client.list_keywords().await {
                Ok(entries) => {
                    if entries.is_empty() {
                        return ToolResult::success(
                            "No tracked keywords. Use action='add_keyword' to add one.",
                        );
                    }
                    let mut output =
                        format!("**Tracked Keywords** ({} entries)\n\n", entries.len());
                    for e in &entries {
                        let cat = e.category.as_deref().unwrap_or("-");
                        output.push_str(&format!(
                            "#{} | {} | category: {}\n",
                            e.id, e.keyword, cat
                        ));
                    }
                    ToolResult::success(output)
                }
                Err(e) => ToolResult::error(format!("Failed to list keywords: {}", e)),
            },

            _ => ToolResult::error(format!(
                "Unknown action: '{}'. Use 'add_account', 'remove_account', 'list_accounts', 'update_account', 'add_keyword', 'remove_keyword', or 'list_keywords'.",
                params.action
            )),
        }
    }

    fn safety_level(&self) -> ToolSafetyLevel {
        ToolSafetyLevel::Standard
    }
}

// =====================================================
// SocialMonitorTweetsTool
// =====================================================

pub struct SocialMonitorTweetsTool {
    definition: ToolDefinition,
    client: Arc<SocialMonitorClient>,
}

impl SocialMonitorTweetsTool {
    pub fn new(client: Arc<SocialMonitorClient>) -> Self {
        let mut properties = HashMap::new();

        properties.insert(
            "action".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Action: 'recent', 'search', 'by_account', 'stats'".to_string(),
                default: None,
                items: None,
                enum_values: Some(vec![
                    "recent".to_string(),
                    "search".to_string(),
                    "by_account".to_string(),
                    "stats".to_string(),
                ]),
            },
        );

        properties.insert(
            "username".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Filter by username (for 'by_account' action)".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "search_text".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Search text to filter tweets (for 'search' action)".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "tweet_type".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description:
                    "Filter by tweet type: 'original', 'reply', 'quote'".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "limit".to_string(),
            PropertySchema {
                schema_type: "integer".to_string(),
                description: "Max results to return (default 25, max 200)".to_string(),
                default: Some(json!(25)),
                items: None,
                enum_values: None,
            },
        );

        SocialMonitorTweetsTool {
            definition: ToolDefinition {
                name: "social_monitor_tweets".to_string(),
                description: "Query captured tweets from monitored Twitter/X accounts. View recent tweets, search by text, filter by account.".to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec!["action".to_string()],
                },
                group: ToolGroup::Social,
                hidden: false,
            },
            client,
        }
    }
}

#[derive(Debug, Deserialize)]
struct TweetParams {
    action: String,
    username: Option<String>,
    search_text: Option<String>,
    tweet_type: Option<String>,
    limit: Option<usize>,
}

#[async_trait]
impl Tool for SocialMonitorTweetsTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, _context: &ToolContext) -> ToolResult {
        let params: TweetParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        match params.action.as_str() {
            "recent" => {
                let filter = social_monitor_types::TweetFilter {
                    tweet_type: params.tweet_type,
                    limit: Some(params.limit.unwrap_or(25)),
                    ..Default::default()
                };
                match self.client.query_tweets(&filter).await {
                    Ok(entries) => format_tweet_list(&entries, "Recent Tweets"),
                    Err(e) => ToolResult::error(format!("Query failed: {}", e)),
                }
            }

            "search" => {
                let filter = social_monitor_types::TweetFilter {
                    search_text: params.search_text,
                    username: params.username,
                    tweet_type: params.tweet_type,
                    limit: Some(params.limit.unwrap_or(50)),
                    ..Default::default()
                };
                match self.client.query_tweets(&filter).await {
                    Ok(entries) => format_tweet_list(&entries, "Search Results"),
                    Err(e) => ToolResult::error(format!("Query failed: {}", e)),
                }
            }

            "by_account" => {
                let username = match params.username {
                    Some(ref u) => u.clone(),
                    None => {
                        return ToolResult::error("'username' is required for 'by_account' action")
                    }
                };
                let filter = social_monitor_types::TweetFilter {
                    username: Some(username),
                    tweet_type: params.tweet_type,
                    limit: Some(params.limit.unwrap_or(25)),
                    ..Default::default()
                };
                match self.client.query_tweets(&filter).await {
                    Ok(entries) => format_tweet_list(&entries, "Account Tweets"),
                    Err(e) => ToolResult::error(format!("Query failed: {}", e)),
                }
            }

            "stats" => match self.client.get_tweet_stats().await {
                Ok(stats) => ToolResult::success(
                    json!({
                        "total_tweets": stats.total_tweets,
                        "monitored_accounts": stats.monitored_accounts,
                        "active_accounts": stats.active_accounts,
                        "tweets_today": stats.tweets_today,
                        "tweets_7d": stats.tweets_7d,
                        "unique_topics": stats.unique_topics,
                    })
                    .to_string(),
                ),
                Err(e) => ToolResult::error(format!("Stats query failed: {}", e)),
            },

            _ => ToolResult::error(format!(
                "Unknown action: '{}'. Use 'recent', 'search', 'by_account', or 'stats'.",
                params.action
            )),
        }
    }

    fn safety_level(&self) -> ToolSafetyLevel {
        ToolSafetyLevel::ReadOnly
    }
}

fn format_tweet_list(
    entries: &[social_monitor_types::CapturedTweet],
    title: &str,
) -> ToolResult {
    if entries.is_empty() {
        return ToolResult::success(format!("**{}**: No tweets found.", title));
    }

    let mut output = format!("**{}** ({} entries)\n\n", title, entries.len());
    for t in entries {
        let text_short = if t.text.len() > 120 {
            format!("{}...", &t.text[..120])
        } else {
            t.text.clone()
        };
        let engagement = t.like_count + t.retweet_count * 2 + t.reply_count;
        output.push_str(&format!(
            "[{}] {} | {} | eng:{} | {}\n",
            t.tweet_type, text_short, t.tweeted_at, engagement, t.tweet_id
        ));
    }
    ToolResult::success(output)
}

// =====================================================
// SocialMonitorForensicsTool
// =====================================================

pub struct SocialMonitorForensicsTool {
    definition: ToolDefinition,
    client: Arc<SocialMonitorClient>,
}

impl SocialMonitorForensicsTool {
    pub fn new(client: Arc<SocialMonitorClient>) -> Self {
        let mut properties = HashMap::new();

        properties.insert(
            "action".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Action: 'topics', 'sentiment', 'report', 'signals'".to_string(),
                default: None,
                items: None,
                enum_values: Some(vec![
                    "topics".to_string(),
                    "sentiment".to_string(),
                    "report".to_string(),
                    "signals".to_string(),
                ]),
            },
        );

        properties.insert(
            "username".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Filter by username".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "account_id".to_string(),
            PropertySchema {
                schema_type: "integer".to_string(),
                description: "Filter by account ID".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "topic".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Filter by topic name".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "trend".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Filter by trend: 'rising', 'falling', 'stable', 'new', 'dormant'"
                    .to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "limit".to_string(),
            PropertySchema {
                schema_type: "integer".to_string(),
                description: "Max results to return (default 25)".to_string(),
                default: Some(json!(25)),
                items: None,
                enum_values: None,
            },
        );

        SocialMonitorForensicsTool {
            definition: ToolDefinition {
                name: "social_monitor_forensics".to_string(),
                description: "Social media forensics and intelligence. Analyze topic trends, sentiment history, and get full forensics reports for monitored accounts.".to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec!["action".to_string()],
                },
                group: ToolGroup::Social,
                hidden: false,
            },
            client,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ForensicsParams {
    action: String,
    username: Option<String>,
    account_id: Option<i64>,
    topic: Option<String>,
    trend: Option<String>,
    limit: Option<usize>,
}

#[async_trait]
impl Tool for SocialMonitorForensicsTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, _context: &ToolContext) -> ToolResult {
        let params: ForensicsParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        match params.action.as_str() {
            "topics" => {
                let filter = social_monitor_types::TopicFilter {
                    account_id: params.account_id,
                    topic: params.topic,
                    trend: params.trend,
                    limit: Some(params.limit.unwrap_or(25)),
                    ..Default::default()
                };
                match self.client.query_topics(&filter).await {
                    Ok(entries) => {
                        if entries.is_empty() {
                            return ToolResult::success(
                                "**Topics**: No topic data available yet.",
                            );
                        }
                        let mut output = format!(
                            "**Topic Scores** ({} entries)\n\n",
                            entries.len()
                        );
                        for ts in &entries {
                            output.push_str(&format!(
                                "{} | 7d:{} 30d:{} total:{} | trend:{} | eng:{:.1}\n",
                                ts.topic,
                                ts.mention_count_7d,
                                ts.mention_count_30d,
                                ts.mention_count_total,
                                ts.trend,
                                ts.avg_engagement_score
                            ));
                        }
                        ToolResult::success(output)
                    }
                    Err(e) => ToolResult::error(format!("Query failed: {}", e)),
                }
            }

            "sentiment" => {
                let filter = social_monitor_types::SentimentFilter {
                    account_id: params.account_id,
                    limit: Some(params.limit.unwrap_or(24)),
                    ..Default::default()
                };
                match self.client.query_sentiment(&filter).await {
                    Ok(entries) => {
                        if entries.is_empty() {
                            return ToolResult::success(
                                "**Sentiment**: No sentiment data available yet.",
                            );
                        }
                        let mut output = format!(
                            "**Sentiment History** ({} snapshots)\n\n",
                            entries.len()
                        );
                        for s in &entries {
                            output.push_str(&format!(
                                "{} -> {} | score:{:.2} ({}) | {} tweets\n",
                                s.window_start,
                                s.window_end,
                                s.sentiment_score,
                                s.sentiment_label,
                                s.tweet_count
                            ));
                        }
                        ToolResult::success(output)
                    }
                    Err(e) => ToolResult::error(format!("Query failed: {}", e)),
                }
            }

            "report" => {
                if params.account_id.is_none() && params.username.is_none() {
                    return ToolResult::error(
                        "Either 'account_id' or 'username' is required for 'report'",
                    );
                }
                match self
                    .client
                    .forensics_report(params.account_id, params.username.as_deref())
                    .await
                {
                    Ok(report) => {
                        let mut output = format!(
                            "**Forensics Report: @{}**\n\n",
                            report.account.username
                        );

                        output.push_str(&format!(
                            "Total tweets: {} | ",
                            report.tweet_count
                        ));
                        if let Some((start, end)) = &report.date_range {
                            output.push_str(&format!("Date range: {} to {}\n\n", start, end));
                        } else {
                            output.push_str("No tweets captured yet\n\n");
                        }

                        if !report.top_topics.is_empty() {
                            output.push_str("**Top Topics:**\n");
                            for ts in &report.top_topics {
                                output.push_str(&format!(
                                    "  {} — 7d:{} 30d:{} total:{} trend:{}\n",
                                    ts.topic,
                                    ts.mention_count_7d,
                                    ts.mention_count_30d,
                                    ts.mention_count_total,
                                    ts.trend
                                ));
                            }
                            output.push('\n');
                        }

                        if !report.recent_sentiment.is_empty() {
                            let latest = &report.recent_sentiment[0];
                            output.push_str(&format!(
                                "**Latest Sentiment:** {:.2} ({}) over {} tweets\n\n",
                                latest.sentiment_score,
                                latest.sentiment_label,
                                latest.tweet_count
                            ));
                        }

                        if !report.signals.is_empty() {
                            output.push_str("**Signals:**\n");
                            for sig in &report.signals {
                                output.push_str(&format!(
                                    "  [{}] {} ({})\n",
                                    sig.severity, sig.description, sig.signal_type
                                ));
                            }
                        }

                        ToolResult::success(output)
                    }
                    Err(e) => ToolResult::error(format!("Report failed: {}", e)),
                }
            }

            "signals" => {
                // Get report for the signals section
                if params.account_id.is_none() && params.username.is_none() {
                    return ToolResult::error(
                        "Either 'account_id' or 'username' is required for 'signals'",
                    );
                }
                match self
                    .client
                    .forensics_report(params.account_id, params.username.as_deref())
                    .await
                {
                    Ok(report) => {
                        if report.signals.is_empty() {
                            return ToolResult::success(format!(
                                "No signals detected for @{}",
                                report.account.username
                            ));
                        }
                        let mut output = format!(
                            "**Signals for @{}** ({} detected)\n\n",
                            report.account.username,
                            report.signals.len()
                        );
                        for sig in &report.signals {
                            output.push_str(&format!(
                                "[{}] {} — {}\n",
                                sig.severity.to_uppercase(),
                                sig.signal_type,
                                sig.description
                            ));
                        }
                        ToolResult::success(output)
                    }
                    Err(e) => ToolResult::error(format!("Signals query failed: {}", e)),
                }
            }

            _ => ToolResult::error(format!(
                "Unknown action: '{}'. Use 'topics', 'sentiment', 'report', or 'signals'.",
                params.action
            )),
        }
    }

    fn safety_level(&self) -> ToolSafetyLevel {
        ToolSafetyLevel::ReadOnly
    }
}

// =====================================================
// SocialMonitorControlTool
// =====================================================

pub struct SocialMonitorControlTool {
    definition: ToolDefinition,
    client: Arc<SocialMonitorClient>,
}

impl SocialMonitorControlTool {
    pub fn new(client: Arc<SocialMonitorClient>) -> Self {
        let mut properties = HashMap::new();

        properties.insert(
            "action".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Action: 'status' to check service health".to_string(),
                default: None,
                items: None,
                enum_values: Some(vec!["status".to_string()]),
            },
        );

        SocialMonitorControlTool {
            definition: ToolDefinition {
                name: "social_monitor_control".to_string(),
                description: "Check the social monitor service health and status.".to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec!["action".to_string()],
                },
                group: ToolGroup::Social,
                hidden: false,
            },
            client,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ControlParams {
    action: String,
}

#[async_trait]
impl Tool for SocialMonitorControlTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, _context: &ToolContext) -> ToolResult {
        let params: ControlParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        match params.action.as_str() {
            "status" => match self.client.get_status().await {
                Ok(status) => ToolResult::success(
                    json!({
                        "running": status.running,
                        "uptime_secs": status.uptime_secs,
                        "monitored_accounts": status.monitored_accounts,
                        "active_accounts": status.active_accounts,
                        "total_tweets": status.total_tweets,
                        "unique_topics": status.unique_topics,
                        "last_tick_at": status.last_tick_at,
                        "poll_interval_secs": status.poll_interval_secs,
                    })
                    .to_string(),
                ),
                Err(e) => ToolResult::error(format!(
                    "Social monitor service unavailable: {}",
                    e
                )),
            },

            _ => ToolResult::error(format!(
                "Unknown action: '{}'. Use 'status'.",
                params.action
            )),
        }
    }

    fn safety_level(&self) -> ToolSafetyLevel {
        ToolSafetyLevel::Standard
    }
}
