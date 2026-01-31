use crate::channels::dispatcher::MessageDispatcher;
use crate::channels::types::{ChannelType, NormalizedMessage};
use crate::gateway::events::EventBroadcaster;
use crate::gateway::protocol::GatewayEvent;
use crate::models::Channel;
use serenity::all::{
    Client, Context, EventHandler, GatewayIntents, Message, Ready,
};
use std::sync::Arc;
use tokio::sync::oneshot;

/// Format a tool call event for Discord display
fn format_tool_call_for_discord(tool_name: &str, parameters: &serde_json::Value) -> String {
    let params_str = serde_json::to_string_pretty(parameters)
        .unwrap_or_else(|_| parameters.to_string());
    // Truncate params if too long for Discord
    let params_display = if params_str.len() > 800 {
        format!("{}...", &params_str[..800])
    } else {
        params_str
    };
    format!("🔧 **Tool Call:** `{}`\n```json\n{}\n```", tool_name, params_display)
}

/// Format a tool result event for Discord display
fn format_tool_result_for_discord(tool_name: &str, success: bool, duration_ms: i64, content: &str) -> String {
    let status = if success { "✅" } else { "❌" };
    // Truncate content if too long
    let content_display = if content.len() > 1200 {
        format!("{}...", &content[..1200])
    } else {
        content.to_string()
    };
    format!(
        "{} **Tool Result:** `{}` ({} ms)\n```\n{}\n```",
        status, tool_name, duration_ms, content_display
    )
}

/// Format an agent mode change for Discord display
fn format_mode_change_for_discord(mode: &str, label: &str, reason: Option<&str>) -> String {
    let emoji = match mode {
        "explore" => "🔍",
        "plan" => "📋",
        "perform" => "⚡",
        _ => "🔄",
    };
    match reason {
        Some(r) => format!("{} **Mode:** {} - {}", emoji, label, r),
        None => format!("{} **Mode:** {}", emoji, label),
    }
}

struct DiscordHandler {
    channel_id: i64,
    dispatcher: Arc<MessageDispatcher>,
    broadcaster: Arc<EventBroadcaster>,
}

#[serenity::async_trait]
impl EventHandler for DiscordHandler {
    async fn message(&self, ctx: Context, msg: Message) {
        // Ignore messages from bots (including ourselves)
        if msg.author.bot {
            return;
        }

        let text = msg.content.clone();
        if text.is_empty() {
            return;
        }

        let user_id = msg.author.id.to_string();
        // Discord moved away from discriminators, so just use the username
        // If discriminator exists and is non-zero, include it for backwards compatibility
        let user_name = match msg.author.discriminator {
            Some(disc) => format!("{}#{}", msg.author.name, disc),
            None => msg.author.name.clone(),
        };

        log::info!(
            "Discord: Message from {} ({}): {}",
            user_name,
            user_id,
            if text.len() > 50 { &text[..50] } else { &text }
        );

        let normalized = NormalizedMessage {
            channel_id: self.channel_id,
            channel_type: ChannelType::Discord.to_string(),
            chat_id: msg.channel_id.to_string(),
            user_id,
            user_name: user_name.clone(),
            text,
            message_id: Some(msg.id.to_string()),
            session_mode: None,
        };

        // Subscribe to events for real-time tool call forwarding
        let (client_id, mut event_rx) = self.broadcaster.subscribe();
        log::info!("Discord: Subscribed to events as client {}", client_id);

        // Clone context and channel info for the event forwarder task
        let http = ctx.http.clone();
        let discord_channel_id = msg.channel_id;
        let channel_id_for_events = self.channel_id;

        // Spawn task to forward events to Discord in real-time
        let event_task = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                // Only forward events for this channel
                if let Some(event_channel_id) = event.data.get("channel_id").and_then(|v| v.as_i64()) {
                    if event_channel_id != channel_id_for_events {
                        continue;
                    }
                }

                let message_text = match event.event.as_str() {
                    "agent.tool_call" => {
                        let tool_name = event.data.get("tool_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let params = event.data.get("parameters")
                            .cloned()
                            .unwrap_or(serde_json::json!({}));
                        Some(format_tool_call_for_discord(tool_name, &params))
                    }
                    "tool.result" => {
                        let tool_name = event.data.get("tool_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let success = event.data.get("success")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let duration_ms = event.data.get("duration_ms")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        let content = event.data.get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        Some(format_tool_result_for_discord(tool_name, success, duration_ms, content))
                    }
                    "agent.mode_change" => {
                        let mode = event.data.get("mode")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let label = event.data.get("label")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown");
                        let reason = event.data.get("reason")
                            .and_then(|v| v.as_str());
                        Some(format_mode_change_for_discord(mode, label, reason))
                    }
                    "execution.task_started" => {
                        let task_type = event.data.get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("task");
                        let name = event.data.get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown task");
                        Some(format!("▶️ **{}:** {}", task_type, name))
                    }
                    "execution.task_completed" => {
                        let status = event.data.get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("completed");
                        let emoji = if status == "completed" { "✅" } else { "❌" };
                        Some(format!("{} Task {}", emoji, status))
                    }
                    _ => None,
                };

                if let Some(text) = message_text {
                    // Split message if too long for Discord
                    let chunks = split_message(&text, 2000);
                    for chunk in chunks {
                        if let Err(e) = discord_channel_id.say(&http, &chunk).await {
                            log::error!("Discord: Failed to send event message: {}", e);
                        }
                    }
                }
            }
        });

        // Dispatch to AI
        log::info!("Discord: Dispatching message to AI for user {}", user_name);
        let result = self.dispatcher.dispatch(normalized).await;
        log::info!("Discord: Dispatch complete, error={:?}", result.error);

        // Unsubscribe and stop event forwarding
        self.broadcaster.unsubscribe(&client_id);
        event_task.abort();
        log::info!("Discord: Unsubscribed from events, client {}", client_id);

        // Send final response
        if result.error.is_none() && !result.response.is_empty() {
            // Discord has a 2000 character limit per message
            let response = &result.response;
            let chunks = split_message(response, 2000);

            for chunk in chunks {
                if let Err(e) = msg.channel_id.say(&ctx.http, &chunk).await {
                    log::error!("Failed to send Discord message: {}", e);
                }
            }
        } else if let Some(error) = result.error {
            let error_msg = format!("Sorry, I encountered an error: {}", error);
            let _ = msg.channel_id.say(&ctx.http, &error_msg).await;
        }
    }

    async fn ready(&self, _ctx: Context, ready: Ready) {
        log::info!("Discord: Bot connected as {}", ready.user.name);
    }
}

/// Split a message into chunks respecting Discord's character limit
fn split_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for line in text.lines() {
        if current.len() + line.len() + 1 > max_len {
            if !current.is_empty() {
                chunks.push(current);
                current = String::new();
            }
            // If single line is too long, split it
            if line.len() > max_len {
                let mut remaining = line;
                while remaining.len() > max_len {
                    chunks.push(remaining[..max_len].to_string());
                    remaining = &remaining[max_len..];
                }
                if !remaining.is_empty() {
                    current = remaining.to_string();
                }
            } else {
                current = line.to_string();
            }
        } else {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

/// Start a Discord bot listener
pub async fn start_discord_listener(
    channel: Channel,
    dispatcher: Arc<MessageDispatcher>,
    broadcaster: Arc<EventBroadcaster>,
    mut shutdown_rx: oneshot::Receiver<()>,
) -> Result<(), String> {
    let channel_id = channel.id;
    let channel_name = channel.name.clone();
    let bot_token = channel.bot_token.clone();

    log::info!("Starting Discord listener for channel: {}", channel_name);
    log::info!("Discord: Token length = {}", bot_token.len());

    // Set up intents - we need message content to read messages
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let handler = DiscordHandler {
        channel_id,
        dispatcher,
        broadcaster: broadcaster.clone(),
    };

    // Create client
    let mut client = Client::builder(&bot_token, intents)
        .event_handler(handler)
        .await
        .map_err(|e| format!("Failed to create Discord client: {}", e))?;

    log::info!("Discord: Client created successfully");

    // Emit started event
    broadcaster.broadcast(GatewayEvent::channel_started(
        channel_id,
        ChannelType::Discord.as_str(),
        &channel_name,
    ));

    // Get shard manager for shutdown
    let shard_manager = client.shard_manager.clone();

    // Run with shutdown signal
    tokio::select! {
        _ = &mut shutdown_rx => {
            log::info!("Discord listener {} received shutdown signal", channel_name);
            shard_manager.shutdown_all().await;
        }
        result = client.start() => {
            match result {
                Ok(()) => log::info!("Discord listener {} stopped", channel_name),
                Err(e) => {
                    let error = format!("Discord client error: {}", e);
                    log::error!("{}", error);
                    broadcaster.broadcast(GatewayEvent::channel_stopped(
                        channel_id,
                        ChannelType::Discord.as_str(),
                        &channel_name,
                    ));
                    return Err(error);
                }
            }
        }
    }

    // Emit stopped event
    broadcaster.broadcast(GatewayEvent::channel_stopped(
        channel_id,
        ChannelType::Discord.as_str(),
        &channel_name,
    ));

    Ok(())
}
