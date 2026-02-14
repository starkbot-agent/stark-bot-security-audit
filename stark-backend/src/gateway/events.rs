use crate::gateway::protocol::GatewayEvent;
use dashmap::DashMap;
use std::collections::VecDeque;
use std::sync::Mutex;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Max number of recent events to keep in the ring buffer for replay on connect
const EVENT_BUFFER_SIZE: usize = 200;

/// Broadcasts events to all connected WebSocket clients
pub struct EventBroadcaster {
    clients: DashMap<String, mpsc::Sender<GatewayEvent>>,
    /// Ring buffer of recent events so new clients can see what happened before they connected
    recent_events: Mutex<VecDeque<GatewayEvent>>,
}

impl EventBroadcaster {
    pub fn new() -> Self {
        Self {
            clients: DashMap::new(),
            recent_events: Mutex::new(VecDeque::with_capacity(EVENT_BUFFER_SIZE)),
        }
    }

    /// Subscribe a new client and return (client_id, receiver)
    pub fn subscribe(&self) -> (String, mpsc::Receiver<GatewayEvent>) {
        let client_id = Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::channel(1000);
        self.clients.insert(client_id.clone(), tx);
        log::debug!("Client {} subscribed to events", client_id);
        (client_id, rx)
    }

    /// Get a snapshot of recent events for replaying to newly connected clients
    pub fn get_recent_events(&self) -> Vec<GatewayEvent> {
        let buffer = self.recent_events.lock().unwrap();
        buffer.iter().cloned().collect()
    }

    /// Unsubscribe a client
    pub fn unsubscribe(&self, client_id: &str) {
        self.clients.remove(client_id);
        log::debug!("Client {} unsubscribed from events", client_id);
    }

    /// Broadcast an event to all connected clients
    pub fn broadcast(&self, event: GatewayEvent) {
        // Store in ring buffer for replay to future clients
        if let Ok(mut buffer) = self.recent_events.lock() {
            if buffer.len() >= EVENT_BUFFER_SIZE {
                buffer.pop_front();
            }
            buffer.push_back(event.clone());
        }

        let event_name = event.event.clone();
        let mut failed_clients = Vec::new();

        // Log tool call and result events at info level for visibility
        if event_name == "agent.tool_call" || event_name == "tool.result" {
            log::info!(
                "[BROADCAST] '{}' to {} client(s)",
                event_name,
                self.clients.len()
            );
        }

        // Log the full event payload for debugging
        if let Ok(json) = serde_json::to_string_pretty(&event) {
            log::debug!(
                "[DATAGRAM] BROADCAST event '{}' to {} clients:\n{}",
                event_name,
                self.clients.len(),
                json
            );
        }

        for entry in self.clients.iter() {
            let client_id = entry.key().clone();
            let sender = entry.value();

            match sender.try_send(event.clone()) {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Full(_)) => {
                    // Channel full — drop this event but keep the subscriber alive.
                    // Removing the subscriber here would silently kill all future
                    // events (including say_to_user delivery to Telegram/Discord).
                    log::warn!(
                        "[BROADCAST] Channel full for client {}, dropping '{}' event",
                        client_id, event_name
                    );
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    // Receiver dropped — subscriber is gone, clean up
                    failed_clients.push(client_id);
                }
            }
        }

        // Clean up actually-disconnected clients (not just slow ones)
        for client_id in failed_clients {
            self.clients.remove(&client_id);
            log::debug!("Removed disconnected client {}", client_id);
        }
    }

    /// Get the number of connected clients
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}
