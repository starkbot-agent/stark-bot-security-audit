use crate::ai::{AiClient, Message, MessageRole, ToolCall, ToolHistoryEntry, ToolResponse};
use crate::channels::types::{DispatchResult, NormalizedMessage};
use crate::db::Database;
use crate::execution::ExecutionTracker;
use crate::gateway::events::EventBroadcaster;
use crate::gateway::protocol::GatewayEvent;
use crate::models::{MemoryType, SessionScope};
use crate::models::session_message::MessageRole as DbMessageRole;
use crate::tools::{ToolConfig, ToolContext, ToolExecution, ToolRegistry};
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// Maximum number of tool execution iterations
const MAX_TOOL_ITERATIONS: usize = 10;

/// JSON response format from the AI when using text-based tool calling
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentResponse {
    body: String,
    tool_call: Option<TextToolCall>,
}

/// Tool call extracted from text-based JSON response
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TextToolCall {
    tool_name: String,
    tool_params: Value,
}

/// Dispatcher routes messages to the AI and returns responses
pub struct MessageDispatcher {
    db: Arc<Database>,
    broadcaster: Arc<EventBroadcaster>,
    tool_registry: Arc<ToolRegistry>,
    execution_tracker: Arc<ExecutionTracker>,
    burner_wallet_private_key: Option<String>,
    // Regex patterns for memory markers
    daily_log_pattern: Regex,
    remember_pattern: Regex,
    remember_important_pattern: Regex,
}

impl MessageDispatcher {
    pub fn new(
        db: Arc<Database>,
        broadcaster: Arc<EventBroadcaster>,
        tool_registry: Arc<ToolRegistry>,
        execution_tracker: Arc<ExecutionTracker>,
    ) -> Self {
        Self::new_with_wallet(db, broadcaster, tool_registry, execution_tracker, None)
    }

    pub fn new_with_wallet(
        db: Arc<Database>,
        broadcaster: Arc<EventBroadcaster>,
        tool_registry: Arc<ToolRegistry>,
        execution_tracker: Arc<ExecutionTracker>,
        burner_wallet_private_key: Option<String>,
    ) -> Self {
        Self {
            db,
            broadcaster,
            tool_registry,
            execution_tracker,
            burner_wallet_private_key,
            daily_log_pattern: Regex::new(r"\[DAILY_LOG:\s*(.+?)\]").unwrap(),
            remember_pattern: Regex::new(r"\[REMEMBER:\s*(.+?)\]").unwrap(),
            remember_important_pattern: Regex::new(r"\[REMEMBER_IMPORTANT:\s*(.+?)\]").unwrap(),
        }
    }

    /// Create a dispatcher without tool support (for backwards compatibility)
    pub fn new_without_tools(db: Arc<Database>, broadcaster: Arc<EventBroadcaster>) -> Self {
        // Create a minimal execution tracker for legacy use
        let execution_tracker = Arc::new(ExecutionTracker::new(broadcaster.clone()));
        Self {
            db,
            broadcaster,
            tool_registry: Arc::new(ToolRegistry::new()),
            execution_tracker,
            burner_wallet_private_key: None,
            daily_log_pattern: Regex::new(r"\[DAILY_LOG:\s*(.+?)\]").unwrap(),
            remember_pattern: Regex::new(r"\[REMEMBER:\s*(.+?)\]").unwrap(),
            remember_important_pattern: Regex::new(r"\[REMEMBER_IMPORTANT:\s*(.+?)\]").unwrap(),
        }
    }

    /// Dispatch a normalized message to the AI and return the response
    pub async fn dispatch(&self, message: NormalizedMessage) -> DispatchResult {
        // Emit message received event
        self.broadcaster.broadcast(GatewayEvent::channel_message(
            message.channel_id,
            &message.channel_type,
            &message.user_name,
            &message.text,
        ));

        // Check for reset commands
        let text_lower = message.text.trim().to_lowercase();
        if text_lower == "/new" || text_lower == "/reset" {
            return self.handle_reset_command(&message).await;
        }

        // Start execution tracking
        let execution_id = self.execution_tracker.start_execution(message.channel_id, "execute");

        // Get or create identity for the user
        let identity = match self.db.get_or_create_identity(
            &message.channel_type,
            &message.user_id,
            Some(&message.user_name),
        ) {
            Ok(id) => id,
            Err(e) => {
                log::error!("Failed to get/create identity: {}", e);
                self.execution_tracker.complete_execution(message.channel_id);
                return DispatchResult::error(format!("Identity error: {}", e));
            }
        };

        // Determine session scope (group if chat_id != user_id, otherwise dm)
        let scope = if message.chat_id != message.user_id {
            SessionScope::Group
        } else {
            SessionScope::Dm
        };

        // Get or create chat session
        let session = match self.db.get_or_create_chat_session(
            &message.channel_type,
            message.channel_id,
            &message.chat_id,
            scope,
            None,
        ) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to get/create session: {}", e);
                self.execution_tracker.complete_execution(message.channel_id);
                return DispatchResult::error(format!("Session error: {}", e));
            }
        };

        // Store user message in session
        if let Err(e) = self.db.add_session_message(
            session.id,
            DbMessageRole::User,
            &message.text,
            Some(&message.user_id),
            Some(&message.user_name),
            message.message_id.as_deref(),
            None,
        ) {
            log::error!("Failed to store user message: {}", e);
        }

        // Get active agent settings from database
        let settings = match self.db.get_active_agent_settings() {
            Ok(Some(settings)) => settings,
            Ok(None) => {
                let error = "No AI provider configured. Please configure agent settings.".to_string();
                log::error!("{}", error);
                self.execution_tracker.complete_execution(message.channel_id);
                return DispatchResult::error(error);
            }
            Err(e) => {
                let error = format!("Database error: {}", e);
                log::error!("{}", error);
                self.execution_tracker.complete_execution(message.channel_id);
                return DispatchResult::error(error);
            }
        };

        log::info!(
            "Using {} provider with model {} for message dispatch (api_key_len={})",
            settings.provider,
            settings.model,
            settings.api_key.len()
        );

        // Create AI client from settings with x402 wallet support
        let client = match AiClient::from_settings_with_wallet(
            &settings,
            self.burner_wallet_private_key.as_deref(),
        ) {
            Ok(c) => c,
            Err(e) => {
                let error = format!("Failed to create AI client: {}", e);
                log::error!("{}", error);
                self.execution_tracker.complete_execution(message.channel_id);
                return DispatchResult::error(error);
            }
        };

        // Add thinking event before AI generation
        self.execution_tracker.add_thinking(message.channel_id, "Processing request...");

        // Get tool configuration for this channel (needed for system prompt)
        let tool_config = self.db.get_effective_tool_config(Some(message.channel_id))
            .unwrap_or_default();

        // Debug: Log tool configuration
        log::info!(
            "[DISPATCH] Tool config - profile: {:?}, allowed_groups: {:?}",
            tool_config.profile,
            tool_config.allowed_groups
        );

        // Build context from memories, tools, skills, and session history
        let system_prompt = self.build_system_prompt(&message, &identity.identity_id, &tool_config);

        // Debug: Log full system prompt
        log::debug!("[DISPATCH] System prompt:\n{}", system_prompt);

        // Get recent session messages for conversation context
        let history = self.db.get_recent_session_messages(session.id, 20).unwrap_or_default();

        // Build messages for the AI
        let mut messages = vec![Message {
            role: MessageRole::System,
            content: system_prompt.clone(),
        }];

        // Add conversation history (skip the last one since it's the current message)
        for msg in history.iter().take(history.len().saturating_sub(1)) {
            let role = match msg.role {
                DbMessageRole::User => MessageRole::User,
                DbMessageRole::Assistant => MessageRole::Assistant,
                DbMessageRole::System => MessageRole::System,
            };
            messages.push(Message {
                role,
                content: msg.content.clone(),
            });
        }

        // Add current user message
        messages.push(Message {
            role: MessageRole::User,
            content: message.text.clone(),
        });

        // Debug: Log user message
        log::info!("[DISPATCH] User message: {}", message.text);

        // Check if the client supports tools and tools are configured
        let use_tools = client.supports_tools() && !self.tool_registry.is_empty();

        // Debug: Log tool availability
        log::info!(
            "[DISPATCH] Tool support - client_supports: {}, registry_count: {}, use_tools: {}",
            client.supports_tools(),
            self.tool_registry.len(),
            use_tools
        );

        // Build tool context with API keys from database
        let workspace_dir = std::env::var("STARK_WORKSPACE_DIR")
            .unwrap_or_else(|_| "./workspace".to_string());

        let mut tool_context = ToolContext::new()
            .with_channel(message.channel_id, message.channel_type.clone())
            .with_user(message.user_id.clone())
            .with_workspace(workspace_dir.clone());

        // Ensure workspace directory exists
        let _ = std::fs::create_dir_all(&workspace_dir);

        // Load API keys from database for tools that need them
        if let Ok(keys) = self.db.list_api_keys() {
            for key in keys {
                tool_context = tool_context.with_api_key(&key.service_name, key.api_key);
            }
        }

        // Generate response with optional tool execution loop
        let final_response = if use_tools {
            self.generate_with_tool_loop(
                &client,
                messages,
                &tool_config,
                &tool_context,
                &identity.identity_id,
                session.id,
                &message,
            ).await
        } else {
            // Simple generation without tools
            client.generate_text(messages).await
        };

        match final_response {
            Ok(response) => {
                // Parse and create memories from the response
                self.process_memory_markers(
                    &response,
                    &identity.identity_id,
                    session.id,
                    &message.channel_type,
                    message.message_id.as_deref(),
                );

                // Clean response by removing memory markers before storing/returning
                let clean_response = self.clean_response(&response);

                // Store AI response in session
                if let Err(e) = self.db.add_session_message(
                    session.id,
                    DbMessageRole::Assistant,
                    &clean_response,
                    None,
                    None,
                    None,
                    None,
                ) {
                    log::error!("Failed to store AI response: {}", e);
                }

                // Emit response event
                self.broadcaster.broadcast(GatewayEvent::agent_response(
                    message.channel_id,
                    &message.user_name,
                    &clean_response,
                ));

                log::info!(
                    "Generated response for {} on channel {} using {}",
                    message.user_name,
                    message.channel_id,
                    settings.provider
                );

                // Complete execution tracking
                self.execution_tracker.complete_execution(message.channel_id);

                DispatchResult::success(clean_response)
            }
            Err(e) => {
                let error = format!("AI generation error ({}): {}", settings.provider, e);
                log::error!("{}", error);

                // Complete execution tracking on error
                self.execution_tracker.complete_execution(message.channel_id);

                DispatchResult::error(error)
            }
        }
    }

    /// Generate a response with tool execution loop (supports both native and text-based tool calling)
    async fn generate_with_tool_loop(
        &self,
        client: &AiClient,
        messages: Vec<Message>,
        tool_config: &ToolConfig,
        tool_context: &ToolContext,
        _identity_id: &str,
        _session_id: i64,
        original_message: &NormalizedMessage,
    ) -> Result<String, String> {
        let tools = self.tool_registry.get_tool_definitions(tool_config);

        // Debug: Log available tools
        log::info!(
            "[TOOL_LOOP] Available tools ({}): {:?}",
            tools.len(),
            tools.iter().map(|t| &t.name).collect::<Vec<_>>()
        );

        if tools.is_empty() {
            log::warn!("[TOOL_LOOP] No tools available, falling back to text-only generation");
            return client.generate_text(messages).await;
        }

        let mut conversation = messages.clone();
        let mut final_response = String::new();
        let mut iterations = 0;

        loop {
            iterations += 1;
            log::info!("[TOOL_LOOP] Iteration {} starting", iterations);

            if iterations > MAX_TOOL_ITERATIONS {
                log::warn!("Tool execution loop exceeded max iterations ({})", MAX_TOOL_ITERATIONS);
                break;
            }

            // Generate response (text-only since we're doing JSON-based tool calling)
            let ai_content = client.generate_text(conversation.clone()).await?;

            log::info!("[TOOL_LOOP] Raw AI response: {}", ai_content);

            // Try to parse as JSON AgentResponse
            let parsed = self.parse_agent_response(&ai_content);

            match parsed {
                Some(agent_response) => {
                    log::info!(
                        "[TOOL_LOOP] Parsed response - body_len: {}, has_tool_call: {}",
                        agent_response.body.len(),
                        agent_response.tool_call.is_some()
                    );

                    // Check if there's a tool call
                    if let Some(tool_call) = agent_response.tool_call {
                        log::info!(
                            "[TOOL_LOOP] Text-based tool call: {} with params: {}",
                            tool_call.tool_name,
                            tool_call.tool_params
                        );

                        // Handle special "use_skill" tool
                        let tool_result = if tool_call.tool_name == "use_skill" {
                            self.execute_skill_tool(&tool_call.tool_params).await
                        } else {
                            // Execute regular tool
                            self.tool_registry.execute(
                                &tool_call.tool_name,
                                tool_call.tool_params.clone(),
                                tool_context,
                                Some(tool_config),
                            ).await
                        };

                        log::info!("[TOOL_LOOP] Tool result success: {}", tool_result.success);
                        log::debug!("[TOOL_LOOP] Tool result content: {}", tool_result.content);

                        // Broadcast tool execution event
                        let _ = self.broadcaster.broadcast(GatewayEvent::tool_result(
                            original_message.channel_id,
                            &tool_call.tool_name,
                            tool_result.success,
                            0, // duration_ms - not tracked for text-based tool calls
                        ));

                        // Add the assistant's response and tool result to conversation
                        conversation.push(Message {
                            role: MessageRole::Assistant,
                            content: ai_content.clone(),
                        });

                        // Add tool result as user message for next iteration
                        conversation.push(Message {
                            role: MessageRole::User,
                            content: format!(
                                "Tool '{}' returned:\n{}\n\nNow provide your final response to the user based on this result. Remember to respond in JSON format.",
                                tool_call.tool_name,
                                tool_result.content
                            ),
                        });

                        // Continue loop to get final response
                        continue;
                    } else {
                        // No tool call, this is the final response
                        final_response = agent_response.body;
                        break;
                    }
                }
                None => {
                    // Couldn't parse as JSON, return raw content
                    log::warn!("[TOOL_LOOP] Could not parse response as JSON, returning raw content");
                    final_response = ai_content;
                    break;
                }
            }
        }

        if final_response.is_empty() {
            return Err("AI returned empty response".to_string());
        }

        Ok(final_response)
    }

    /// Execute the special "use_skill" tool
    async fn execute_skill_tool(&self, params: &Value) -> crate::tools::ToolResult {
        let skill_name = params.get("skill_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let input = params.get("input")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        log::info!("[SKILL] Executing skill '{}' with input: {}", skill_name, input);

        // Look up the skill
        let skills = match self.db.list_enabled_skills() {
            Ok(s) => s,
            Err(e) => {
                return crate::tools::ToolResult::error(format!("Failed to load skills: {}", e));
            }
        };

        let skill = skills.iter().find(|s| s.name == skill_name && s.enabled);

        match skill {
            Some(skill) => {
                // Return the skill's instructions/body along with context
                let mut result = format!("## Skill: {}\n\n", skill.name);
                result.push_str(&format!("Description: {}\n\n", skill.description));

                if !skill.body.is_empty() {
                    result.push_str("### Instructions:\n");
                    result.push_str(&skill.body);
                    result.push_str("\n\n");
                }

                result.push_str(&format!("### User Query:\n{}\n\n", input));
                result.push_str("Use the appropriate tools (like `exec` for commands) to fulfill this skill request based on the instructions above.");

                crate::tools::ToolResult::success(&result)
            }
            None => {
                crate::tools::ToolResult::error(format!(
                    "Skill '{}' not found or not enabled. Available skills: {}",
                    skill_name,
                    skills.iter()
                        .filter(|s| s.enabled)
                        .map(|s| s.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            }
        }
    }

    /// Parse AI response as JSON AgentResponse, with fallback extraction
    fn parse_agent_response(&self, content: &str) -> Option<AgentResponse> {
        let content = content.trim();

        // Try direct JSON parse first
        if let Ok(response) = serde_json::from_str::<AgentResponse>(content) {
            return Some(response);
        }

        // Try to extract JSON from markdown code blocks
        let json_patterns = [
            // ```json ... ```
            regex::Regex::new(r"```(?:json)?\s*\n?([\s\S]*?)\n?```").ok()?,
        ];

        for pattern in &json_patterns {
            if let Some(captures) = pattern.captures(content) {
                if let Some(json_match) = captures.get(1) {
                    if let Ok(response) = serde_json::from_str::<AgentResponse>(json_match.as_str().trim()) {
                        return Some(response);
                    }
                }
            }
        }

        // Try to find JSON object anywhere in the content
        if let Some(start) = content.find('{') {
            // Find matching closing brace
            let mut depth = 0;
            let mut end = start;
            for (i, c) in content[start..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = start + i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if end > start {
                let json_str = &content[start..end];
                if let Ok(response) = serde_json::from_str::<AgentResponse>(json_str) {
                    return Some(response);
                }
            }
        }

        // If all parsing fails, treat the whole content as body with no tool call
        log::debug!("[PARSE] Could not extract JSON, treating as plain text response");
        Some(AgentResponse {
            body: content.to_string(),
            tool_call: None,
        })
    }

    /// Execute a list of tool calls and return responses (for native tool calling)
    #[allow(dead_code)]
    async fn execute_tool_calls(
        &self,
        tool_calls: &[ToolCall],
        tool_config: &ToolConfig,
        tool_context: &ToolContext,
        channel_id: i64,
    ) -> Vec<ToolResponse> {
        let mut responses = Vec::new();

        // Get the current execution ID for tracking
        let execution_id = self.execution_tracker.get_execution_id(channel_id);

        for call in tool_calls {
            let start = std::time::Instant::now();

            // Start tracking this tool execution
            let task_id = if let Some(ref exec_id) = execution_id {
                Some(self.execution_tracker.start_tool(channel_id, exec_id, &call.name))
            } else {
                None
            };

            // Emit tool execution event (legacy event for backwards compatibility)
            self.broadcaster.broadcast(GatewayEvent::tool_execution(
                channel_id,
                &call.name,
                &call.arguments,
            ));

            // Execute the tool
            let result = self
                .tool_registry
                .execute(&call.name, call.arguments.clone(), tool_context, Some(tool_config))
                .await;

            let duration_ms = start.elapsed().as_millis() as i64;

            // Complete the tool tracking
            if let Some(ref tid) = task_id {
                if result.success {
                    self.execution_tracker.complete_task(tid);
                } else {
                    self.execution_tracker.complete_task_with_error(tid, &result.content);
                }
            }

            // Emit tool result event (legacy event for backwards compatibility)
            self.broadcaster.broadcast(GatewayEvent::tool_result(
                channel_id,
                &call.name,
                result.success,
                duration_ms,
            ));

            // Log the execution
            if let Err(e) = self.db.log_tool_execution(&ToolExecution {
                id: None,
                channel_id,
                tool_name: call.name.clone(),
                parameters: call.arguments.clone(),
                success: result.success,
                result: Some(result.content.clone()),
                duration_ms: Some(duration_ms),
                executed_at: Utc::now().to_rfc3339(),
            }) {
                log::error!("Failed to log tool execution: {}", e);
            }

            log::info!(
                "Tool '{}' executed in {}ms, success: {}",
                call.name,
                duration_ms,
                result.success
            );

            // Create tool response
            responses.push(if result.success {
                ToolResponse::success(call.id.clone(), result.content)
            } else {
                ToolResponse::error(call.id.clone(), result.content)
            });
        }

        responses
    }

    /// Build the system prompt with context from memories, tools, and skills
    fn build_system_prompt(
        &self,
        message: &NormalizedMessage,
        identity_id: &str,
        tool_config: &ToolConfig,
    ) -> String {
        let mut prompt = String::from(
            "You are StarkBot, an AI agent who can respond to users and operate tools.\n\n"
        );

        // Add JSON response format instruction
        prompt.push_str("## RESPONSE FORMAT (CRITICAL)\n\n");
        prompt.push_str("You MUST respond in this JSON format:\n");
        prompt.push_str("```\n");
        prompt.push_str("{\"body\": \"your message\", \"tool_call\": null}\n");
        prompt.push_str("```\n\n");
        prompt.push_str("To call a tool:\n");
        prompt.push_str("```\n");
        prompt.push_str("{\"body\": \"brief status\", \"tool_call\": {\"tool_name\": \"name\", \"tool_params\": {...}}}\n");
        prompt.push_str("```\n\n");

        // Build tools array in OpenAI schema format
        let tools = self.tool_registry.get_tool_definitions(tool_config);
        let skills = self.db.list_enabled_skills().unwrap_or_default();
        let active_skills: Vec<_> = skills.iter().filter(|s| s.enabled).collect();

        if !tools.is_empty() || !active_skills.is_empty() {
            prompt.push_str("## AVAILABLE TOOLS\n\n");
            prompt.push_str("```json\n");
            prompt.push_str("[\n");

            let mut tool_entries: Vec<String> = Vec::new();

            // Add regular tools
            for tool in &tools {
                let tool_json = serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.input_schema
                    }
                });
                tool_entries.push(serde_json::to_string_pretty(&tool_json).unwrap_or_default());
            }

            // Add skills as a special tool with nested skill options
            if !active_skills.is_empty() {
                let skill_names: Vec<&str> = active_skills.iter().map(|s| s.name.as_str()).collect();
                let skill_descriptions: Vec<String> = active_skills.iter()
                    .map(|s| format!("{}: {}", s.name, s.description))
                    .collect();

                let skills_tool = serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": "use_skill",
                        "description": format!("Execute a skill. Available skills: {}", skill_descriptions.join("; ")),
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "skill_name": {
                                    "type": "string",
                                    "enum": skill_names,
                                    "description": "The skill to execute"
                                },
                                "input": {
                                    "type": "string",
                                    "description": "Input or query for the skill"
                                }
                            },
                            "required": ["skill_name", "input"]
                        }
                    }
                });
                tool_entries.push(serde_json::to_string_pretty(&skills_tool).unwrap_or_default());
            }

            prompt.push_str(&tool_entries.join(",\n"));
            prompt.push_str("\n]\n```\n\n");

            // Add usage examples
            prompt.push_str("## EXAMPLES\n\n");
            prompt.push_str("Weather query:\n");
            prompt.push_str("```\n{\"body\": \"Checking...\", \"tool_call\": {\"tool_name\": \"exec\", \"tool_params\": {\"command\": \"curl -s 'wttr.in/Ohio?format=3'\"}}}\n```\n\n");

            prompt.push_str("Web search:\n");
            prompt.push_str("```\n{\"body\": \"Searching...\", \"tool_call\": {\"tool_name\": \"web_search\", \"tool_params\": {\"query\": \"latest news\"}}}\n```\n\n");

            if !active_skills.is_empty() {
                prompt.push_str("Using a skill:\n");
                prompt.push_str(&format!(
                    "```\n{{\"body\": \"Using skill...\", \"tool_call\": {{\"tool_name\": \"use_skill\", \"tool_params\": {{\"skill_name\": \"{}\", \"input\": \"your query\"}}}}}}\n```\n\n",
                    active_skills.first().map(|s| s.name.as_str()).unwrap_or("weather")
                ));
            }

            prompt.push_str("**IMPORTANT**: For weather, news, or live data - USE TOOLS IMMEDIATELY. Do not say you cannot access real-time data.\n\n");
        }

        // Add skill details for context
        if !active_skills.is_empty() {
            prompt.push_str("## SKILL DETAILS\n\n");
            for skill in &active_skills {
                prompt.push_str(&format!("### {}\n", skill.name));
                prompt.push_str(&format!("{}\n", skill.description));
                if !skill.body.is_empty() {
                    prompt.push_str(&format!("Instructions: {}\n", skill.body.lines().take(3).collect::<Vec<_>>().join(" ")));
                }
                prompt.push_str("\n");
            }
        }

        // Add daily logs context
        if let Ok(daily_logs) = self.db.get_todays_daily_logs(Some(identity_id)) {
            if !daily_logs.is_empty() {
                prompt.push_str("## Today's Notes\n");
                for log in daily_logs {
                    prompt.push_str(&format!("- {}\n", log.content));
                }
                prompt.push('\n');
            }
        }

        // Add relevant long-term memories
        if let Ok(memories) = self.db.get_long_term_memories(Some(identity_id), Some(5), 10) {
            if !memories.is_empty() {
                prompt.push_str("## User Context\n");
                for mem in memories {
                    prompt.push_str(&format!("- {}\n", mem.content));
                }
                prompt.push('\n');
            }
        }

        // Add context
        prompt.push_str(&format!(
            "## Current Request\nUser: {} | Channel: {}\n",
            message.user_name, message.channel_type
        ));

        prompt
    }

    /// Process memory markers in the AI response
    fn process_memory_markers(
        &self,
        response: &str,
        identity_id: &str,
        session_id: i64,
        channel_type: &str,
        message_id: Option<&str>,
    ) {
        let today = Utc::now().date_naive();

        // Process daily logs
        for cap in self.daily_log_pattern.captures_iter(response) {
            if let Some(content) = cap.get(1) {
                let content_str = content.as_str().trim();
                if !content_str.is_empty() {
                    if let Err(e) = self.db.create_memory(
                        MemoryType::DailyLog,
                        content_str,
                        None,
                        None,
                        5,
                        Some(identity_id),
                        Some(session_id),
                        Some(channel_type),
                        message_id,
                        Some(today),
                        None,
                    ) {
                        log::error!("Failed to create daily log: {}", e);
                    } else {
                        log::info!("Created daily log: {}", content_str);
                    }
                }
            }
        }

        // Process regular remember markers (importance 7)
        for cap in self.remember_pattern.captures_iter(response) {
            if let Some(content) = cap.get(1) {
                let content_str = content.as_str().trim();
                if !content_str.is_empty() {
                    if let Err(e) = self.db.create_memory(
                        MemoryType::LongTerm,
                        content_str,
                        None,
                        None,
                        7,
                        Some(identity_id),
                        Some(session_id),
                        Some(channel_type),
                        message_id,
                        None,
                        None,
                    ) {
                        log::error!("Failed to create long-term memory: {}", e);
                    } else {
                        log::info!("Created long-term memory: {}", content_str);
                    }
                }
            }
        }

        // Process important remember markers (importance 9)
        for cap in self.remember_important_pattern.captures_iter(response) {
            if let Some(content) = cap.get(1) {
                let content_str = content.as_str().trim();
                if !content_str.is_empty() {
                    if let Err(e) = self.db.create_memory(
                        MemoryType::LongTerm,
                        content_str,
                        None,
                        None,
                        9,
                        Some(identity_id),
                        Some(session_id),
                        Some(channel_type),
                        message_id,
                        None,
                        None,
                    ) {
                        log::error!("Failed to create important memory: {}", e);
                    } else {
                        log::info!("Created important memory: {}", content_str);
                    }
                }
            }
        }
    }

    /// Remove memory markers from the response before returning to user
    fn clean_response(&self, response: &str) -> String {
        let mut clean = response.to_string();
        clean = self.daily_log_pattern.replace_all(&clean, "").to_string();
        clean = self.remember_pattern.replace_all(&clean, "").to_string();
        clean = self.remember_important_pattern.replace_all(&clean, "").to_string();
        // Clean up any double spaces or trailing whitespace
        clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
        clean.trim().to_string()
    }

    /// Handle /new or /reset commands
    async fn handle_reset_command(&self, message: &NormalizedMessage) -> DispatchResult {
        // Determine session scope
        let scope = if message.chat_id != message.user_id {
            SessionScope::Group
        } else {
            SessionScope::Dm
        };

        // Get the current session
        match self.db.get_or_create_chat_session(
            &message.channel_type,
            message.channel_id,
            &message.chat_id,
            scope,
            None,
        ) {
            Ok(session) => {
                // Reset the session
                match self.db.reset_chat_session(session.id) {
                    Ok(_) => {
                        let response = "Session reset. Let's start fresh!".to_string();
                        self.broadcaster.broadcast(GatewayEvent::agent_response(
                            message.channel_id,
                            &message.user_name,
                            &response,
                        ));
                        DispatchResult::success(response)
                    }
                    Err(e) => {
                        log::error!("Failed to reset session: {}", e);
                        DispatchResult::error(format!("Failed to reset session: {}", e))
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to get session for reset: {}", e);
                DispatchResult::error(format!("Session error: {}", e))
            }
        }
    }
}
