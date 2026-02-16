//! Streaming response types and utilities
//!
//! This module provides types for streaming AI responses in real-time,
//! allowing incremental updates of both content and tool calls.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

/// Events emitted during streaming response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Content is being generated incrementally
    ContentDelta {
        /// The new content chunk
        content: String,
        /// Index in the response (for multi-choice)
        index: usize,
    },
    /// A tool call is starting
    ToolCallStart {
        /// Unique ID for this tool call
        id: String,
        /// Tool name
        name: String,
        /// Index in the tool call list
        index: usize,
    },
    /// Arguments for a tool call are being streamed
    ToolCallDelta {
        /// Tool call ID
        id: String,
        /// Partial arguments (JSON string chunk)
        arguments_delta: String,
        /// Index in the tool call list
        index: usize,
    },
    /// A tool call is complete
    ToolCallComplete {
        /// Tool call ID
        id: String,
        /// Tool name
        name: String,
        /// Complete parsed arguments
        arguments: Value,
        /// Index in the tool call list
        index: usize,
    },
    /// Thinking/reasoning content (for Claude extended thinking)
    ThinkingDelta {
        /// The thinking content chunk
        content: String,
    },
    /// Stream has completed
    Done {
        /// Stop reason (e.g., "end_turn", "tool_use", "max_tokens")
        stop_reason: Option<String>,
        /// Usage statistics
        usage: Option<StreamUsage>,
    },
    /// An error occurred during streaming
    Error {
        /// Error message
        message: String,
        /// Optional error code
        code: Option<String>,
    },
}

/// Usage statistics for streaming response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamUsage {
    /// Input tokens used
    pub input_tokens: u32,
    /// Output tokens generated
    pub output_tokens: u32,
    /// Cache creation tokens (Claude)
    pub cache_creation_input_tokens: Option<u32>,
    /// Cache read tokens (Claude)
    pub cache_read_input_tokens: Option<u32>,
}

/// Sender for stream events
pub type StreamSender = mpsc::Sender<StreamEvent>;

/// Receiver for stream events
pub type StreamReceiver = mpsc::Receiver<StreamEvent>;

/// Create a new stream channel with specified buffer size
pub fn create_stream_channel(buffer_size: usize) -> (StreamSender, StreamReceiver) {
    mpsc::channel(buffer_size)
}

/// Create a stream channel with default buffer size (32)
pub fn create_default_stream_channel() -> (StreamSender, StreamReceiver) {
    create_stream_channel(32)
}

/// Accumulator for building complete response from stream events
#[derive(Debug, Clone, Default)]
pub struct StreamAccumulator {
    /// Accumulated content
    pub content: String,
    /// Accumulated thinking content
    pub thinking: String,
    /// Partial tool calls being built
    pub tool_calls: Vec<PartialToolCall>,
    /// Stop reason from Done event
    pub stop_reason: Option<String>,
    /// Usage from Done event
    pub usage: Option<StreamUsage>,
    /// Any error that occurred
    pub error: Option<String>,
}

/// A tool call being built from stream events
#[derive(Debug, Clone)]
pub struct PartialToolCall {
    pub id: String,
    pub name: String,
    pub arguments_json: String,
    pub complete: bool,
}

impl StreamAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a stream event and update accumulator state
    pub fn process_event(&mut self, event: StreamEvent) {
        match event {
            StreamEvent::ContentDelta { content, .. } => {
                self.content.push_str(&content);
            }
            StreamEvent::ThinkingDelta { content } => {
                self.thinking.push_str(&content);
            }
            StreamEvent::ToolCallStart { id, name, index } => {
                // Ensure we have enough slots
                while self.tool_calls.len() <= index {
                    self.tool_calls.push(PartialToolCall {
                        id: String::new(),
                        name: String::new(),
                        arguments_json: String::new(),
                        complete: false,
                    });
                }
                self.tool_calls[index] = PartialToolCall {
                    id,
                    name,
                    arguments_json: String::new(),
                    complete: false,
                };
            }
            StreamEvent::ToolCallDelta { arguments_delta, index, .. } => {
                if let Some(tc) = self.tool_calls.get_mut(index) {
                    tc.arguments_json.push_str(&arguments_delta);
                }
            }
            StreamEvent::ToolCallComplete { id, name, arguments, index } => {
                if let Some(tc) = self.tool_calls.get_mut(index) {
                    tc.id = id;
                    tc.name = name;
                    tc.arguments_json = arguments.to_string();
                    tc.complete = true;
                }
            }
            StreamEvent::Done { stop_reason, usage } => {
                self.stop_reason = stop_reason;
                self.usage = usage;
            }
            StreamEvent::Error { message, .. } => {
                self.error = Some(message);
            }
        }
    }

    /// Check if the stream is complete
    pub fn is_complete(&self) -> bool {
        self.stop_reason.is_some() || self.error.is_some()
    }

    /// Check if there was an error
    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }

    /// Get completed tool calls
    pub fn completed_tool_calls(&self) -> Vec<&PartialToolCall> {
        self.tool_calls.iter().filter(|tc| tc.complete).collect()
    }
}

/// Configuration for streaming behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    /// Whether streaming is enabled
    pub enabled: bool,
    /// Buffer size for the channel
    pub buffer_size: usize,
    /// Timeout for individual events in milliseconds
    pub event_timeout_ms: u64,
    /// Whether to emit thinking deltas (Claude)
    pub emit_thinking: bool,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            buffer_size: 32,
            event_timeout_ms: 30000,
            emit_thinking: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_accumulator() {
        let mut acc = StreamAccumulator::new();

        // Process content deltas
        acc.process_event(StreamEvent::ContentDelta {
            content: "Hello".to_string(),
            index: 0,
        });
        acc.process_event(StreamEvent::ContentDelta {
            content: " World".to_string(),
            index: 0,
        });

        assert_eq!(acc.content, "Hello World");
        assert!(!acc.is_complete());

        // Process done
        acc.process_event(StreamEvent::Done {
            stop_reason: Some("end_turn".to_string()),
            usage: None,
        });

        assert!(acc.is_complete());
        assert_eq!(acc.stop_reason, Some("end_turn".to_string()));
    }

    #[test]
    fn test_tool_call_accumulation() {
        let mut acc = StreamAccumulator::new();

        acc.process_event(StreamEvent::ToolCallStart {
            id: "call_1".to_string(),
            name: "get_weather".to_string(),
            index: 0,
        });

        acc.process_event(StreamEvent::ToolCallDelta {
            id: "call_1".to_string(),
            arguments_delta: r#"{"city":"#.to_string(),
            index: 0,
        });

        acc.process_event(StreamEvent::ToolCallDelta {
            id: "call_1".to_string(),
            arguments_delta: r#""New York"}"#.to_string(),
            index: 0,
        });

        acc.process_event(StreamEvent::ToolCallComplete {
            id: "call_1".to_string(),
            name: "get_weather".to_string(),
            arguments: serde_json::json!({"city": "New York"}),
            index: 0,
        });

        assert_eq!(acc.tool_calls.len(), 1);
        assert!(acc.tool_calls[0].complete);
        assert_eq!(acc.tool_calls[0].name, "get_weather");
    }
}
