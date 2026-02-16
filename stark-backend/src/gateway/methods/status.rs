use crate::gateway::events::EventBroadcaster;
use crate::gateway::protocol::RpcError;
use serde_json::Value;
use std::sync::Arc;

pub async fn handle_ping() -> Result<Value, RpcError> {
    Ok(serde_json::json!("pong"))
}

pub async fn handle_status(broadcaster: Arc<EventBroadcaster>) -> Result<Value, RpcError> {
    Ok(serde_json::json!({
        "status": "ok",
        "connected_clients": broadcaster.client_count(),
    }))
}
