use crate::gateway::protocol::GatewayEvent;
use dashmap::DashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Max number of recent events to keep in the ring buffer for replay on connect
const EVENT_BUFFER_SIZE: usize = 200;

/// Internal commands sent to the background broadcast task.
enum BroadcastCmd {
    /// Deliver an event to all current subscribers and buffer it for replay.
    Send(GatewayEvent),
    /// Register a new subscriber.
    Subscribe {
        client_id: String,
        sender: mpsc::Sender<GatewayEvent>,
    },
    /// Remove a subscriber.
    Unsubscribe(String),
}

/// Broadcasts events to all connected WebSocket clients.
///
/// Calling `broadcast()` is non-blocking: the event is sent to an internal
/// channel and a background tokio task handles mutex locking, cloning, and
/// per-client delivery so the caller (the agentic loop) is never stalled.
pub struct EventBroadcaster {
    /// Non-blocking command channel to the background task.
    cmd_tx: mpsc::UnboundedSender<BroadcastCmd>,
    /// Shared client map — used by `subscribe` / `unsubscribe` / `client_count`
    /// from any thread without going through the command channel.
    clients: Arc<DashMap<String, mpsc::Sender<GatewayEvent>>>,
    /// Ring buffer accessible for replay on new connections.
    recent_events: Arc<std::sync::Mutex<VecDeque<GatewayEvent>>>,
}

impl EventBroadcaster {
    pub fn new() -> Self {
        let clients: Arc<DashMap<String, mpsc::Sender<GatewayEvent>>> =
            Arc::new(DashMap::new());
        let recent_events =
            Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(EVENT_BUFFER_SIZE)));

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

        // Spawn the background broadcast loop
        tokio::spawn(Self::run_loop(
            cmd_rx,
            clients.clone(),
            recent_events.clone(),
        ));

        Self {
            cmd_tx,
            clients,
            recent_events,
        }
    }

    /// Subscribe a new client and return (client_id, receiver).
    pub fn subscribe(&self) -> (String, mpsc::Receiver<GatewayEvent>) {
        let client_id = Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::channel(1000);

        // Insert into shared map so client_count is immediately accurate
        self.clients.insert(client_id.clone(), tx.clone());

        // Also notify the background loop (it uses the shared map directly,
        // but the command keeps the door open for future per-subscribe logic).
        let _ = self.cmd_tx.send(BroadcastCmd::Subscribe {
            client_id: client_id.clone(),
            sender: tx,
        });

        log::debug!("Client {} subscribed to events", client_id);
        (client_id, rx)
    }

    /// Get a snapshot of recent events for replaying to newly connected clients.
    pub fn get_recent_events(&self) -> Vec<GatewayEvent> {
        let buffer = self.recent_events.lock().unwrap();
        buffer.iter().cloned().collect()
    }

    /// Unsubscribe a client.
    pub fn unsubscribe(&self, client_id: &str) {
        self.clients.remove(client_id);
        let _ = self.cmd_tx.send(BroadcastCmd::Unsubscribe(client_id.to_string()));
        log::debug!("Client {} unsubscribed from events", client_id);
    }

    /// Queue an event for broadcast. Returns immediately — the actual fan-out
    /// happens on a background task so the caller is never blocked by mutex
    /// contention, event cloning, or slow subscribers.
    pub fn broadcast(&self, event: GatewayEvent) {
        let _ = self.cmd_tx.send(BroadcastCmd::Send(event));
    }

    /// Get the number of connected clients.
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    // ── background task ──────────────────────────────────────────────

    async fn run_loop(
        mut cmd_rx: mpsc::UnboundedReceiver<BroadcastCmd>,
        clients: Arc<DashMap<String, mpsc::Sender<GatewayEvent>>>,
        recent_events: Arc<std::sync::Mutex<VecDeque<GatewayEvent>>>,
    ) {
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                BroadcastCmd::Send(event) => {
                    // Store in ring buffer for replay
                    if let Ok(mut buffer) = recent_events.lock() {
                        if buffer.len() >= EVENT_BUFFER_SIZE {
                            buffer.pop_front();
                        }
                        buffer.push_back(event.clone());
                    }

                    let event_name = event.event.clone();

                    // Log tool call and result events at info level for visibility
                    if event_name == "agent.tool_call" || event_name == "tool.result" {
                        log::info!(
                            "[BROADCAST] '{}' to {} client(s)",
                            event_name,
                            clients.len()
                        );
                    }

                    // Log the full event payload for debugging (gated to avoid
                    // expensive serialization when debug logging is disabled)
                    if log::log_enabled!(log::Level::Debug) {
                        if let Ok(json) = serde_json::to_string_pretty(&event) {
                            log::debug!(
                                "[DATAGRAM] BROADCAST event '{}' to {} clients:\n{}",
                                event_name,
                                clients.len(),
                                json
                            );
                        }
                    }

                    let mut failed_clients = Vec::new();

                    for entry in clients.iter() {
                        let client_id = entry.key().clone();
                        let sender = entry.value();

                        match sender.try_send(event.clone()) {
                            Ok(()) => {}
                            Err(mpsc::error::TrySendError::Full(_)) => {
                                log::warn!(
                                    "[BROADCAST] Channel full for client {}, dropping '{}' event",
                                    client_id, event_name
                                );
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                failed_clients.push(client_id);
                            }
                        }
                    }

                    // Clean up disconnected clients
                    for client_id in failed_clients {
                        clients.remove(&client_id);
                        log::debug!("Removed disconnected client {}", client_id);
                    }
                }
                BroadcastCmd::Subscribe { client_id, sender } => {
                    // Ensure the client is in the shared map (should already be
                    // inserted by `subscribe()`, but this is a safety net).
                    clients.insert(client_id, sender);
                }
                BroadcastCmd::Unsubscribe(client_id) => {
                    clients.remove(&client_id);
                }
            }
        }

        log::info!("[EVENT_BROADCASTER] Background broadcast loop shutting down");
    }
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}
