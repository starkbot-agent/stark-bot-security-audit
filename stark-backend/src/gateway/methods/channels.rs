use crate::channels::ChannelManager;
use crate::db::Database;
use crate::gateway::protocol::{ChannelIdParams, RpcError};
use crate::models::ChannelResponse;
use serde_json::Value;
use std::sync::Arc;

pub async fn handle_channels_status(
    db: Arc<Database>,
    channel_manager: Arc<ChannelManager>,
) -> Result<Value, RpcError> {
    let channels = db
        .list_channels()
        .map_err(|e| RpcError::internal_error(format!("Database error: {}", e)))?;

    let responses: Vec<ChannelResponse> = channels
        .into_iter()
        .map(|ch| {
            let running = channel_manager.is_running(ch.id);
            ChannelResponse::from(ch).with_running(running)
        })
        .collect();

    serde_json::to_value(responses).map_err(|e| RpcError::internal_error(e.to_string()))
}

pub async fn handle_channels_start(
    params: ChannelIdParams,
    db: Arc<Database>,
    channel_manager: Arc<ChannelManager>,
) -> Result<Value, RpcError> {
    // Get the channel from database
    let channel = db
        .get_channel(params.id)
        .map_err(|e| RpcError::internal_error(format!("Database error: {}", e)))?
        .ok_or_else(|| RpcError::invalid_params(format!("Channel {} not found", params.id)))?;

    // Start the channel
    channel_manager
        .start_channel(channel)
        .await
        .map_err(|e| RpcError::internal_error(e))?;

    // Update enabled status in database
    db.set_channel_enabled(params.id, true)
        .map_err(|e| RpcError::internal_error(format!("Database error: {}", e)))?;

    Ok(serde_json::json!({
        "success": true,
        "channel_id": params.id
    }))
}

pub async fn handle_channels_stop(
    params: ChannelIdParams,
    channel_manager: Arc<ChannelManager>,
    db: Arc<Database>,
) -> Result<Value, RpcError> {
    // Stop the channel
    channel_manager
        .stop_channel(params.id)
        .await
        .map_err(|e| RpcError::internal_error(e))?;

    // Update enabled status in database
    db.set_channel_enabled(params.id, false)
        .map_err(|e| RpcError::internal_error(format!("Database error: {}", e)))?;

    Ok(serde_json::json!({
        "success": true,
        "channel_id": params.id
    }))
}

pub async fn handle_channels_restart(
    params: ChannelIdParams,
    db: Arc<Database>,
    channel_manager: Arc<ChannelManager>,
) -> Result<Value, RpcError> {
    // Stop if running
    let _ = channel_manager.stop_channel(params.id).await;

    // Get the channel from database
    let channel = db
        .get_channel(params.id)
        .map_err(|e| RpcError::internal_error(format!("Database error: {}", e)))?
        .ok_or_else(|| RpcError::invalid_params(format!("Channel {} not found", params.id)))?;

    // Start the channel
    channel_manager
        .start_channel(channel)
        .await
        .map_err(|e| RpcError::internal_error(e))?;

    Ok(serde_json::json!({
        "success": true,
        "channel_id": params.id
    }))
}
