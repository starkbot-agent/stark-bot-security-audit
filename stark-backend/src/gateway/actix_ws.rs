//! Actix-Web WebSocket handler for the Gateway
//! This allows WebSocket connections on the same port as the HTTP server,
//! which is required for platforms like DigitalOcean App Platform that only expose one port.

use crate::channels::ChannelManager;
use crate::db::Database;
use crate::gateway::events::EventBroadcaster;
use crate::gateway::methods;
use crate::gateway::protocol::{ChannelIdParams, RpcError, RpcRequest, RpcResponse};
use crate::tx_queue::TxQueueManager;
use actix_web::{web, HttpRequest, HttpResponse};
use actix_ws::AggregatedMessage;
use futures_util::StreamExt;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Authentication timeout - client must authenticate within this time
const AUTH_TIMEOUT_SECS: u64 = 30;

/// Parameters for the auth RPC method
#[derive(Debug, Deserialize)]
struct AuthParams {
    token: String,
}

/// WebSocket handler for Actix-Web
pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    db: web::Data<Arc<Database>>,
    channel_manager: web::Data<Arc<ChannelManager>>,
    broadcaster: web::Data<Arc<EventBroadcaster>>,
    tx_queue: web::Data<Arc<TxQueueManager>>,
) -> Result<HttpResponse, actix_web::Error> {
    let (response, session, msg_stream) = actix_ws::handle(&req, stream)?;

    // Spawn the WebSocket handler task
    let db = db.get_ref().clone();
    let channel_manager = channel_manager.get_ref().clone();
    let broadcaster = broadcaster.get_ref().clone();
    let tx_queue = tx_queue.get_ref().clone();

    actix_web::rt::spawn(handle_ws_connection(
        session,
        msg_stream,
        db,
        channel_manager,
        broadcaster,
        tx_queue,
    ));

    Ok(response)
}

async fn handle_ws_connection(
    mut session: actix_ws::Session,
    msg_stream: actix_ws::MessageStream,
    db: Arc<Database>,
    channel_manager: Arc<ChannelManager>,
    broadcaster: Arc<EventBroadcaster>,
    tx_queue: Arc<TxQueueManager>,
) {
    log::info!("New Actix WebSocket connection");

    // Aggregate messages for easier handling
    let mut msg_stream = msg_stream
        .aggregate_continuations()
        .max_continuation_size(64 * 1024);

    // Phase 1: Authentication required before full access
    let authenticated = match tokio::time::timeout(
        Duration::from_secs(AUTH_TIMEOUT_SECS),
        wait_for_auth(&mut session, &mut msg_stream, &db),
    )
    .await
    {
        Ok(Ok(true)) => true,
        Ok(Ok(false)) => {
            log::warn!("Gateway client failed authentication");
            let _ = session.close(None).await;
            return;
        }
        Ok(Err(e)) => {
            log::error!("Gateway auth error: {}", e);
            let _ = session.close(None).await;
            return;
        }
        Err(_) => {
            log::warn!("Gateway client auth timeout after {}s", AUTH_TIMEOUT_SECS);
            let timeout_response = RpcResponse::error(
                "".to_string(),
                RpcError::new(-32000, "Authentication timeout".to_string()),
            );
            if let Ok(json) = serde_json::to_string(&timeout_response) {
                let _ = session.text(json).await;
            }
            let _ = session.close(None).await;
            return;
        }
    };

    if !authenticated {
        let _ = session.close(None).await;
        return;
    }

    log::info!("Gateway client authenticated successfully");

    // Phase 2: Full access after authentication
    // Subscribe to events
    let (client_id, mut event_rx) = broadcaster.subscribe();
    log::info!(
        "Gateway client {} subscribed to events (total: {} clients)",
        client_id,
        broadcaster.client_count()
    );

    // Create a channel for sending messages to the WebSocket
    let (tx, mut rx) = mpsc::channel::<String>(100);

    // Clone session for the send task
    let mut send_session = session.clone();
    let client_id_clone = client_id.clone();

    // Task to forward messages to WebSocket
    let send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                // Forward RPC responses
                Some(msg) = rx.recv() => {
                    log::debug!("[DATAGRAM] >>> TO AGENT (RPC response):\n{}", msg);
                    if send_session.text(msg).await.is_err() {
                        break;
                    }
                }
                // Forward events
                Some(event) = event_rx.recv() => {
                    let event_name = event.event.clone();
                    if let Ok(json) = serde_json::to_string(&event) {
                        if event_name == "agent.tool_call" || event_name == "tool.result" {
                            log::info!("[WEBSOCKET] Sending '{}' event to client {}", event_name, client_id_clone);
                        }
                        log::debug!("[DATAGRAM] >>> TO AGENT (event: {}):\n{}", event_name, json);
                        if send_session.text(json).await.is_err() {
                            log::warn!("[WEBSOCKET] Failed to send event to client {}", client_id_clone);
                            break;
                        }
                    }
                }
                else => break,
            }
        }
    });

    // Process incoming messages
    while let Some(msg_result) = msg_stream.next().await {
        match msg_result {
            Ok(AggregatedMessage::Text(text)) => {
                log::debug!("[DATAGRAM] <<< FROM AGENT (RPC request):\n{}", text);
                let response = process_request(&text, &db, &channel_manager, &broadcaster, &tx_queue).await;
                if let Ok(json) = serde_json::to_string(&response) {
                    let _ = tx.send(json).await;
                }
            }
            Ok(AggregatedMessage::Ping(data)) => {
                if session.pong(&data).await.is_err() {
                    break;
                }
            }
            Ok(AggregatedMessage::Close(_)) => {
                break;
            }
            Err(e) => {
                log::error!("WebSocket error: {:?}", e);
                break;
            }
            _ => {}
        }
    }

    // Cleanup
    broadcaster.unsubscribe(&client_id);
    send_task.abort();
    let _ = session.close(None).await;
    log::info!("Gateway client {} disconnected", client_id);
}

/// Wait for authentication from the client
async fn wait_for_auth(
    session: &mut actix_ws::Session,
    msg_stream: &mut (impl StreamExt<Item = Result<AggregatedMessage, actix_ws::ProtocolError>> + Unpin),
    db: &Arc<Database>,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    while let Some(msg_result) = msg_stream.next().await {
        match msg_result {
            Ok(AggregatedMessage::Text(text)) => {
                log::debug!("[DATAGRAM] <<< FROM AGENT (auth phase):\n{}", text);

                // Try to parse as RPC request
                let request: RpcRequest = match serde_json::from_str(&text) {
                    Ok(req) => req,
                    Err(_) => {
                        let response = RpcResponse::error("".to_string(), RpcError::parse_error());
                        if let Ok(json) = serde_json::to_string(&response) {
                            let _ = session.text(json).await;
                        }
                        continue;
                    }
                };

                // Only allow "auth" and "ping" methods before authentication
                match request.method.as_str() {
                    "auth" => {
                        let params: AuthParams = match serde_json::from_value(request.params.clone())
                        {
                            Ok(p) => p,
                            Err(e) => {
                                let response = RpcResponse::error(
                                    request.id.clone(),
                                    RpcError::invalid_params(format!(
                                        "Missing or invalid token: {}",
                                        e
                                    )),
                                );
                                if let Ok(json) = serde_json::to_string(&response) {
                                    let _ = session.text(json).await;
                                }
                                continue;
                            }
                        };

                        // Validate token against database
                        match db.validate_session(&params.token) {
                            Ok(Some(_session)) => {
                                let response = RpcResponse::success(
                                    request.id,
                                    serde_json::json!({"authenticated": true}),
                                );
                                if let Ok(json) = serde_json::to_string(&response) {
                                    let _ = session.text(json).await;
                                }
                                return Ok(true);
                            }
                            Ok(None) => {
                                let response = RpcResponse::error(
                                    request.id,
                                    RpcError::new(-32001, "Invalid or expired token".to_string()),
                                );
                                if let Ok(json) = serde_json::to_string(&response) {
                                    let _ = session.text(json).await;
                                }
                                return Ok(false);
                            }
                            Err(e) => {
                                log::error!("Database error validating token: {}", e);
                                let response = RpcResponse::error(
                                    request.id,
                                    RpcError::internal_error(format!("Database error: {}", e)),
                                );
                                if let Ok(json) = serde_json::to_string(&response) {
                                    let _ = session.text(json).await;
                                }
                                return Ok(false);
                            }
                        }
                    }
                    "ping" => {
                        let response =
                            RpcResponse::success(request.id, serde_json::json!("pong"));
                        if let Ok(json) = serde_json::to_string(&response) {
                            let _ = session.text(json).await;
                        }
                    }
                    _ => {
                        let response = RpcResponse::error(
                            request.id,
                            RpcError::new(
                                -32002,
                                "Authentication required. Call 'auth' method first.".to_string(),
                            ),
                        );
                        if let Ok(json) = serde_json::to_string(&response) {
                            let _ = session.text(json).await;
                        }
                    }
                }
            }
            Ok(AggregatedMessage::Ping(data)) => {
                let _ = session.pong(&data).await;
            }
            Ok(AggregatedMessage::Close(_)) => {
                return Ok(false);
            }
            Err(e) => {
                log::error!("WebSocket error during auth: {:?}", e);
                return Err(format!("WebSocket error: {:?}", e).into());
            }
            _ => {}
        }
    }

    Ok(false)
}

async fn process_request(
    text: &str,
    db: &Arc<Database>,
    channel_manager: &Arc<ChannelManager>,
    broadcaster: &Arc<EventBroadcaster>,
    tx_queue: &Arc<TxQueueManager>,
) -> RpcResponse {
    let request: RpcRequest = match serde_json::from_str(text) {
        Ok(req) => req,
        Err(_) => {
            return RpcResponse::error("".to_string(), RpcError::parse_error());
        }
    };

    let id = request.id.clone();

    let result = dispatch_method(&request, db, channel_manager, broadcaster, tx_queue).await;

    match result {
        Ok(value) => RpcResponse::success(id, value),
        Err(error) => RpcResponse::error(id, error),
    }
}

async fn dispatch_method(
    request: &RpcRequest,
    db: &Arc<Database>,
    channel_manager: &Arc<ChannelManager>,
    broadcaster: &Arc<EventBroadcaster>,
    tx_queue: &Arc<TxQueueManager>,
) -> Result<serde_json::Value, RpcError> {
    match request.method.as_str() {
        "ping" => methods::handle_ping().await,
        "status" => methods::handle_status(broadcaster.clone()).await,
        "channels.status" => {
            methods::handle_channels_status(db.clone(), channel_manager.clone()).await
        }
        "channels.start" => {
            let params: ChannelIdParams = serde_json::from_value(request.params.clone())
                .map_err(|e| RpcError::invalid_params(format!("Invalid params: {}", e)))?;
            methods::handle_channels_start(params, db.clone(), channel_manager.clone()).await
        }
        "channels.stop" => {
            let params: ChannelIdParams = serde_json::from_value(request.params.clone())
                .map_err(|e| RpcError::invalid_params(format!("Invalid params: {}", e)))?;
            methods::handle_channels_stop(params, channel_manager.clone(), db.clone()).await
        }
        "channels.restart" => {
            let params: ChannelIdParams = serde_json::from_value(request.params.clone())
                .map_err(|e| RpcError::invalid_params(format!("Invalid params: {}", e)))?;
            methods::handle_channels_restart(params, db.clone(), channel_manager.clone()).await
        }
        "tx_queue.confirm" => {
            let params: methods::TxQueueParams = serde_json::from_value(request.params.clone())
                .map_err(|e| RpcError::invalid_params(format!("Invalid params: {}", e)))?;
            methods::handle_tx_queue_confirm(params, tx_queue.clone(), broadcaster.clone()).await
        }
        "tx_queue.deny" => {
            let params: methods::TxQueueParams = serde_json::from_value(request.params.clone())
                .map_err(|e| RpcError::invalid_params(format!("Invalid params: {}", e)))?;
            methods::handle_tx_queue_deny(params, tx_queue.clone(), broadcaster.clone()).await
        }
        _ => Err(RpcError::method_not_found()),
    }
}
