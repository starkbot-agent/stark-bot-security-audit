//! Transaction queue RPC methods for partner mode confirmation
//!
//! Handles user confirmation/denial of queued transactions via the frontend modal.

use crate::gateway::events::EventBroadcaster;
use crate::gateway::protocol::{GatewayEvent, RpcError};
use crate::tools::rpc_config::resolve_rpc_from_network;
use crate::tx_queue::{QueuedTxStatus, TxQueueManager};
use crate::wallet::WalletProvider;
use crate::x402::X402EvmRpc;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct TxQueueParams {
    pub uuid: String,
    pub channel_id: i64,
}

/// Handle tx_queue.confirm RPC method
/// Broadcasts the transaction and emits result events
pub async fn handle_tx_queue_confirm(
    params: TxQueueParams,
    tx_queue: Arc<TxQueueManager>,
    broadcaster: Arc<EventBroadcaster>,
    wallet_provider: Option<Arc<dyn WalletProvider>>,
) -> Result<Value, RpcError> {
    log::info!("[tx_queue.confirm] Confirming transaction {}", params.uuid);

    // Get transaction
    let tx = tx_queue.get(&params.uuid)
        .ok_or_else(|| RpcError::new(-32000, format!("Transaction {} not found", params.uuid)))?;

    // Validate pending status
    if tx.status != QueuedTxStatus::Pending {
        return Err(RpcError::new(-32000, format!("Transaction {} is not pending (status: {:?})", params.uuid, tx.status)));
    }

    // Mark broadcasting
    tx_queue.mark_broadcasting(&params.uuid);

    // Get wallet provider for x402 payments
    let wallet_provider = wallet_provider
        .ok_or_else(|| RpcError::new(-32000, "Wallet not configured".to_string()))?;

    // Resolve RPC configuration
    let rpc_config = resolve_rpc_from_network(&tx.network);

    // Initialize RPC client with WalletProvider (works in both Standard and Flash mode)
    let rpc = X402EvmRpc::new_with_wallet_provider(
        wallet_provider,
        &tx.network,
        Some(rpc_config.url.clone()),
        rpc_config.use_x402,
    ).map_err(|e| {
        tx_queue.mark_failed(&params.uuid, &e);
        RpcError::new(-32000, format!("RPC error: {}", e))
    })?;

    // Decode signed transaction from hex
    let signed_tx_bytes = hex::decode(tx.signed_tx_hex.trim_start_matches("0x"))
        .map_err(|e| {
            tx_queue.mark_failed(&params.uuid, &format!("Invalid tx hex: {}", e));
            RpcError::new(-32000, format!("Invalid tx hex: {}", e))
        })?;

    // Broadcast the transaction
    let tx_hash = rpc.send_raw_transaction(&signed_tx_bytes).await
        .map_err(|e| {
            tx_queue.mark_failed(&params.uuid, &e);
            RpcError::new(-32000, format!("Broadcast failed: {}", e))
        })?;

    let tx_hash_str = format!("{:?}", tx_hash);
    let explorer_url = format!("{}/{}", tx.get_explorer_base_url(), tx_hash_str);

    // Mark as broadcast (partner mode - user confirmed)
    tx_queue.mark_broadcast(&params.uuid, &tx_hash_str, &explorer_url, "partner");

    log::info!("[tx_queue.confirm] Transaction {} broadcast as {}", params.uuid, tx_hash_str);

    // Emit tx.pending event
    broadcaster.broadcast(GatewayEvent::tx_pending(
        params.channel_id, &tx_hash_str, &tx.network, &explorer_url
    ));

    // Clone values for the spawned task
    let uuid = params.uuid.clone();
    let channel_id = params.channel_id;
    let network = tx.network.clone();
    let tx_queue_clone = tx_queue.clone();
    let broadcaster_clone = broadcaster.clone();
    let tx_hash_clone = tx_hash_str.clone();

    // Wait for confirmation in a spawned task (don't block the RPC response)
    tokio::spawn(async move {
        match rpc.wait_for_receipt(tx_hash, Duration::from_secs(120)).await {
            Ok(receipt) => {
                let status = if receipt.status == Some(ethers::types::U64::from(1)) {
                    tx_queue_clone.mark_confirmed(&uuid);
                    "confirmed"
                } else {
                    tx_queue_clone.mark_failed(&uuid, "Reverted");
                    "reverted"
                };
                broadcaster_clone.broadcast(GatewayEvent::tx_confirmed(
                    channel_id, &tx_hash_clone, &network, status
                ));
                log::info!("[tx_queue.confirm] Transaction {} {}", uuid, status);
            }
            Err(e) => {
                log::warn!("[tx_queue.confirm] Receipt wait timeout for {}: {}", uuid, e);
                // Timeout - tx may still confirm, don't mark as failed
            }
        }
    });

    // Emit tx_queue.confirmed event
    broadcaster.broadcast(GatewayEvent::tx_queue_confirmed(
        params.channel_id, &params.uuid, &tx_hash_str
    ));

    Ok(json!({
        "success": true,
        "uuid": params.uuid,
        "tx_hash": tx_hash_str,
        "explorer_url": explorer_url
    }))
}

/// Handle tx_queue.deny RPC method
/// Removes the transaction from the queue without broadcasting
pub async fn handle_tx_queue_deny(
    params: TxQueueParams,
    tx_queue: Arc<TxQueueManager>,
    broadcaster: Arc<EventBroadcaster>,
) -> Result<Value, RpcError> {
    log::info!("[tx_queue.deny] Denying transaction {}", params.uuid);

    // Remove from queue
    let removed = tx_queue.remove(&params.uuid);

    if removed.is_none() {
        return Err(RpcError::new(-32000, format!("Transaction {} not found", params.uuid)));
    }

    // Emit denied event
    broadcaster.broadcast(GatewayEvent::tx_queue_denied(
        params.channel_id, &params.uuid
    ));

    log::info!("[tx_queue.deny] Transaction {} denied and deleted", params.uuid);

    Ok(json!({
        "success": true,
        "uuid": params.uuid,
        "action": "denied_and_deleted"
    }))
}
