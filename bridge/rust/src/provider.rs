use crate::config::{ApiFormat, ProviderConfig};
use crate::debug;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone)]
pub struct StreamEvent {
    pub event_type: StreamEventType,
}

#[derive(Debug, Clone)]
pub enum StreamEventType {
    Text(String),
    Reasoning(String),
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    Done,
}

fn stream_has_content(events: &[StreamEvent]) -> bool {
    events.iter().any(|e| {
        matches!(
            e.event_type,
            StreamEventType::Text(_)
                | StreamEventType::Reasoning(_)
                | StreamEventType::ToolCall { .. }
        )
    })
}

/// Backfill `reasoning_content` on assistant+tool_calls messages when replaying tool history.
/// Required by OpenAI-compatible thinking-mode providers (DeepSeek, MiMo, etc.).
pub(crate) fn normalize_thinking_replay(messages: &[Message]) -> Vec<Message> {
    let in_tool_replay_chain = messages
        .iter()
        .any(|m| m.role == "assistant" && m.tool_calls.is_some());
    if !in_tool_replay_chain {
        return messages.to_vec();
    }
    let mut out = messages.to_vec();
    for m in &mut out {
        if m.role == "assistant" && m.tool_calls.is_some() && m.reasoning_content.is_none() {
            m.reasoning_content = Some(String::new());
        }
    }
    out
}

fn openai_sse_error_message(data: &str) -> Option<String> {
    if data == "[DONE]" {
        return None;
    }
    let json: serde_json::Value = serde_json::from_str(data).ok()?;
    let msg = json
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())?;
    Some(format!("API error: {msg}"))
}

fn log_empty_openai_stream(messages_len: usize, raw_sse: &str, finish_reason: Option<&str>) {
    if !debug::is_enabled() {
        return;
    }
    debug::log("provider", &format!("empty stream messages={messages_len}"));
    if let Some(reason) = finish_reason {
        debug::log("provider", &format!("finish_reason={reason}"));
    }
    let snippet: String = raw_sse.chars().take(500).collect();
    let redacted = snippet.replace("Bearer ", "Bearer [REDACTED]");
    debug::log("provider", &format!("sse_snippet={redacted}"));
}

pub struct Provider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl Provider {
    pub fn new(config: ProviderConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { config, client }
    }

    pub fn id(&self) -> &str {
        &self.config.id
    }

    pub fn model(&self) -> &str {
        &self.config.default_model
    }

    pub fn set_model(&mut self, model: String) {
        self.config.default_model = model;
    }

    pub async fn discover_models(&self) -> Vec<String> {
        if !self.config.models.is_empty() {
            return self.config.models.clone();
        }

        let url = format!("{}/models", self.config.base_url.trim_end_matches('/'));
        let mut req = self.client.get(&url);
        if !self.config.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.config.api_key));
        }

        let Ok(resp) = req.send().await else {
            return fallback_models(&self.config);
        };
        if !resp.status().is_success() {
            return fallback_models(&self.config);
        }

        let Ok(json) = resp.json::<serde_json::Value>().await else {
            return fallback_models(&self.config);
        };

        let mut models = Vec::new();
        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
            for item in data {
                if let Some(id) = item.get("id").and_then(|id| id.as_str()) {
                    models.push(id.to_string());
                }
            }
        }

        if models.is_empty() {
            fallback_models(&self.config)
        } else {
            models.sort();
            models
        }
    }

    pub async fn stream_chat(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        interrupt: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<Vec<StreamEvent>, String> {
        self.stream_chat_with(messages, tools, interrupt, Option::<fn(StreamEvent)>::None)
            .await
    }

    pub async fn stream_chat_with<F>(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        interrupt: Option<&std::sync::atomic::AtomicBool>,
        mut on_event: Option<F>,
    ) -> Result<Vec<StreamEvent>, String>
    where
        F: FnMut(StreamEvent),
    {
        match self.config.api_format {
            ApiFormat::OpenAI => {
                self.stream_openai(messages, tools, interrupt, on_event.as_mut())
                    .await
            }
            ApiFormat::Anthropic => {
                self.stream_anthropic(messages, tools, interrupt, on_event.as_mut())
                    .await
            }
        }
    }

    fn push_event<F>(
        events: &mut Vec<StreamEvent>,
        cb: &mut Option<&mut F>,
        event: StreamEvent,
    ) where
        F: FnMut(StreamEvent),
    {
        if let Some(f) = cb.as_mut() {
            f(event.clone());
        }
        events.push(event);
    }

    async fn stream_openai<F>(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        interrupt: Option<&std::sync::atomic::AtomicBool>,
        mut on_event: Option<&mut F>,
    ) -> Result<Vec<StreamEvent>, String>
    where
        F: FnMut(StreamEvent),
    {
        let url = format!("{}/chat/completions", self.config.base_url);
        let api_messages = normalize_thinking_replay(messages);

        let mut body = serde_json::json!({
            "model": self.config.default_model,
            "messages": api_messages,
            "stream": true,
        });

        if !tools.is_empty() {
            let tool_defs: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(tool_defs);
        }

        if debug::is_enabled() {
            debug::log(
                "provider",
                &format!(
                    "openai request messages={} tools={}",
                    messages.len(),
                    tools.len()
                ),
            );
            for (i, msg) in api_messages.iter().enumerate() {
                if msg.role == "assistant" && msg.tool_calls.is_some() {
                    if let Ok(serialized) = serde_json::to_string(msg) {
                        debug::log(
                            "provider",
                            &format!("assistant_tool_msg[{i}]={serialized}"),
                        );
                    }
                }
            }
        }

        let mut req = self
            .client
            .post(&url)
            .header("Content-Type", "application/json");

        if !self.config.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.config.api_key));
        }

        let resp = req
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("API error {status}: {text}"));
        }

        let mut events = Vec::new();
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut raw_sse = String::new();
        let mut last_finish_reason: Option<String> = None;
        // Accumulate tool call arguments across chunks
        // (id, name, accumulated_arguments)
        let mut tool_call_acc: std::collections::HashMap<String, (String, String, String)> =
            std::collections::HashMap::new();

        while let Some(chunk) = stream.next().await {
            if interrupt.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed)) {
                Self::push_event(
                    &mut events,
                    &mut on_event,
                    StreamEvent {
                        event_type: StreamEventType::Done,
                    },
                );
                return Ok(events);
            }

            let chunk = chunk.map_err(|e| format!("stream error: {e}"))?;
            let chunk_str = String::from_utf8_lossy(&chunk);
            if raw_sse.len() < 2000 {
                raw_sse.push_str(&chunk_str);
                raw_sse.truncate(2000);
            }
            buffer.push_str(&chunk_str);

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim().to_string();
                buffer = buffer[line_end + 1..].to_string();

                if line.is_empty() || !line.starts_with("data: ") {
                    continue;
                }

                let data = &line[6..];
                if data == "[DONE]" {
                    // Flush accumulated tool calls
                    for (_index, (id, name, args)) in tool_call_acc.drain() {
                        Self::push_event(
                            &mut events,
                            &mut on_event,
                            StreamEvent {
                                event_type: StreamEventType::ToolCall {
                                    id,
                                    name,
                                    arguments: args,
                                },
                            },
                        );
                    }
                    Self::push_event(
                        &mut events,
                        &mut on_event,
                        StreamEvent {
                            event_type: StreamEventType::Done,
                        },
                    );
                    continue;
                }

                if let Some(err_msg) = openai_sse_error_message(data) {
                    if debug::is_enabled() {
                        debug::log("provider", &format!("sse_error={err_msg}"));
                    }
                    return Err(err_msg);
                }

                let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
                    continue;
                };

                if let Some(reason) = json
                    .get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("finish_reason"))
                    .and_then(|r| r.as_str())
                {
                    if !reason.is_empty() {
                        last_finish_reason = Some(reason.to_string());
                    }
                }

                // Extract text content
                if let Some(delta) = json
                    .get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("delta"))
                {
                    if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                        if !content.is_empty() {
                            Self::push_event(
                                &mut events,
                                &mut on_event,
                                StreamEvent {
                                    event_type: StreamEventType::Text(content.to_string()),
                                },
                            );
                        }
                    }
                    if let Some(reasoning) = delta.get("reasoning_content").and_then(|c| c.as_str())
                    {
                        if !reasoning.is_empty() {
                            Self::push_event(
                                &mut events,
                                &mut on_event,
                                StreamEvent {
                                    event_type: StreamEventType::Reasoning(reasoning.to_string()),
                                },
                            );
                        }
                    }

                    // Handle tool calls
                    if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                        for tc in tool_calls {
                            let index = tc
                                .get("index")
                                .and_then(|i| i.as_u64())
                                .unwrap_or(0)
                                .to_string();
                            let id = tc
                                .get("id")
                                .and_then(|i| i.as_str())
                                .unwrap_or("")
                                .to_string();
                            let name = tc
                                .get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(|n| n.as_str())
                                .unwrap_or("")
                                .to_string();
                            let args = tc
                                .get("function")
                                .and_then(|f| f.get("arguments"))
                                .and_then(|a| a.as_str())
                                .unwrap_or("")
                                .to_string();

                            let entry = tool_call_acc.entry(index).or_default();
                            if !id.is_empty() {
                                entry.0 = id;
                            }
                            if !name.is_empty() {
                                entry.1 = name;
                            }
                            entry.2.push_str(&args);
                        }
                    }
                }
            }
        }

        // Flush any remaining tool calls
        for (_index, (id, name, args)) in tool_call_acc.drain() {
            Self::push_event(
                &mut events,
                &mut on_event,
                StreamEvent {
                    event_type: StreamEventType::ToolCall {
                        id,
                        name,
                        arguments: args,
                    },
                },
            );
        }

        if !events.iter().any(|e| matches!(e.event_type, StreamEventType::Done)) {
            Self::push_event(
                &mut events,
                &mut on_event,
                StreamEvent {
                    event_type: StreamEventType::Done,
                },
            );
        }

        if !stream_has_content(&events) {
            log_empty_openai_stream(
                messages.len(),
                &raw_sse,
                last_finish_reason.as_deref(),
            );
        }

        Ok(events)
    }

    async fn stream_anthropic<F>(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        interrupt: Option<&std::sync::atomic::AtomicBool>,
        mut on_event: Option<&mut F>,
    ) -> Result<Vec<StreamEvent>, String>
    where
        F: FnMut(StreamEvent),
    {
        let url = format!("{}/v1/messages", self.config.base_url);

        // Convert messages to Anthropic format
        let system_msg;
        let api_messages = if let Some(first) = messages.first() {
            if first.role == "system" {
                system_msg = first.content.clone().unwrap_or_default();
                &messages[1..]
            } else {
                system_msg = String::new();
                messages
            }
        } else {
            system_msg = String::new();
            messages
        };

        let mut body = serde_json::json!({
            "model": self.config.default_model,
            "max_tokens": 8192,
            "messages": api_messages,
            "stream": true,
        });

        if !system_msg.is_empty() {
            body["system"] = serde_json::json!(system_msg);
        }

        if !tools.is_empty() {
            let tool_defs: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters,
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(tool_defs);
        }

        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("API error {status}: {text}"));
        }

        let mut events = Vec::new();
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut current_tool_id = String::new();
        let mut current_tool_name = String::new();
        let mut current_tool_args = String::new();

        while let Some(chunk) = stream.next().await {
            if interrupt.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed)) {
                Self::push_event(
                    &mut events,
                    &mut on_event,
                    StreamEvent {
                        event_type: StreamEventType::Done,
                    },
                );
                return Ok(events);
            }

            let chunk = chunk.map_err(|e| format!("stream error: {e}"))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim().to_string();
                buffer = buffer[line_end + 1..].to_string();

                if line.is_empty() || !line.starts_with("data: ") {
                    continue;
                }

                let data = &line[6..];
                let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
                    continue;
                };

                let event_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");

                match event_type {
                    "content_block_start" => {
                        if let Some(cb) = json.get("content_block") {
                            if cb.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                current_tool_id = cb
                                    .get("id")
                                    .and_then(|i| i.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                current_tool_name = cb
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                current_tool_args.clear();
                            }
                        }
                    }
                    "content_block_delta" => {
                        if let Some(delta) = json.get("delta") {
                            if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                Self::push_event(
                                    &mut events,
                                    &mut on_event,
                                    StreamEvent {
                                        event_type: StreamEventType::Text(text.to_string()),
                                    },
                                );
                            }
                            if let Some(partial) =
                                delta.get("partial_json").and_then(|p| p.as_str())
                            {
                                current_tool_args.push_str(partial);
                            }
                        }
                    }
                    "content_block_stop" => {
                        if !current_tool_name.is_empty() {
                            Self::push_event(
                                &mut events,
                                &mut on_event,
                                StreamEvent {
                                    event_type: StreamEventType::ToolCall {
                                        id: current_tool_id.clone(),
                                        name: current_tool_name.clone(),
                                        arguments: current_tool_args.clone(),
                                    },
                                },
                            );
                            current_tool_id.clear();
                            current_tool_name.clear();
                            current_tool_args.clear();
                        }
                    }
                    "message_stop" => {
                        Self::push_event(
                            &mut events,
                            &mut on_event,
                            StreamEvent {
                                event_type: StreamEventType::Done,
                            },
                        );
                    }
                    _ => {}
                }
            }
        }

        if !events.iter().any(|e| matches!(e.event_type, StreamEventType::Done)) {
            Self::push_event(
                &mut events,
                &mut on_event,
                StreamEvent {
                    event_type: StreamEventType::Done,
                },
            );
        }

        Ok(events)
    }
}

fn fallback_models(config: &ProviderConfig) -> Vec<String> {
    if !config.default_model.is_empty() {
        vec![config.default_model.clone()]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod stream_tests {
    use super::*;

    #[test]
    fn stream_has_content_detects_text_and_tools() {
        assert!(!stream_has_content(&[]));
        assert!(stream_has_content(&[StreamEvent {
            event_type: StreamEventType::Text("x".into()),
        }]));
        assert!(stream_has_content(&[StreamEvent {
            event_type: StreamEventType::ToolCall {
                id: "1".into(),
                name: "shell".into(),
                arguments: "{}".into(),
            },
        }]));
        assert!(!stream_has_content(&[StreamEvent {
            event_type: StreamEventType::Done,
        }]));
    }

    #[test]
    fn normalize_thinking_replay_backfills_assistant_tool_calls() {
        let messages = vec![
            Message {
                role: "user".to_string(),
                content: Some("hi".into()),
                reasoning_content: None,
                tool_calls: None,
                tool_call_id: None,
            },
            Message {
                role: "assistant".to_string(),
                content: Some("checking".into()),
                reasoning_content: None,
                tool_calls: Some(vec![ToolCall {
                    id: "c1".into(),
                    call_type: "function".into(),
                    function: FunctionCall {
                        name: "shell".into(),
                        arguments: "{}".into(),
                    },
                }]),
                tool_call_id: None,
            },
            Message {
                role: "tool".to_string(),
                content: Some("ok".into()),
                reasoning_content: None,
                tool_calls: None,
                tool_call_id: Some("c1".into()),
            },
        ];
        let normalized = normalize_thinking_replay(&messages);
        assert_eq!(
            normalized[1].reasoning_content,
            Some(String::new()),
            "expected empty reasoning_content backfill"
        );
    }

    #[test]
    fn normalize_thinking_replay_noop_without_tool_history() {
        let messages = vec![Message {
            role: "user".to_string(),
            content: Some("hi".into()),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
        }];
        let normalized = normalize_thinking_replay(&messages);
        assert_eq!(normalized.len(), 1);
        assert!(normalized[0].reasoning_content.is_none());
    }

    #[test]
    fn openai_sse_error_message_detects_upstream_error() {
        let data = r#"{"error":{"message":"Param Incorrect","type":"upstream_error","code":"400"}}"#;
        assert_eq!(
            openai_sse_error_message(data),
            Some("API error: Param Incorrect".into())
        );
        assert!(openai_sse_error_message("[DONE]").is_none());
    }

    #[test]
    fn message_reasoning_content_serde_roundtrip() {
        let msg = Message {
            role: "assistant".to_string(),
            content: Some("hi".into()),
            reasoning_content: Some("thinking".into()),
            tool_calls: None,
            tool_call_id: None,
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(json.contains("reasoning_content"));
        let back: Message = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.reasoning_content.as_deref(), Some("thinking"));
    }
}

#[cfg(test)]
mod smoke_tests {
    use super::*;
    use crate::config::{ApiFormat, ProviderConfig};

    #[tokio::test]
    async fn opengateway_mimo_stream_without_api_key() {
        let provider = Provider::new(ProviderConfig {
            id: "opengateway".to_string(),
            name: "OpenGateway (gitlawb)".to_string(),
            api_format: ApiFormat::OpenAI,
            base_url: "https://opengateway.gitlawb.com/v1".to_string(),
            api_key: String::new(),
            default_model: "mimo-v2.5-pro".to_string(),
            models: vec!["mimo-v2.5-pro".to_string()],
            requires_api_key: None,
        });

        let messages = vec![Message {
            role: "user".to_string(),
            content: Some("Reply with exactly: zero-agent-ok".to_string()),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
        }];

        let events = provider
            .stream_chat(&messages, &[], None)
            .await
            .expect("opengateway stream should succeed without api key");

        let text: String = events
            .iter()
            .filter_map(|e| match &e.event_type {
                StreamEventType::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();

        assert!(
            text.contains("zero-agent-ok"),
            "expected streamed reply, got: {text:?}"
        );
    }

    #[tokio::test]
    async fn opengateway_discover_models_without_api_key() {
        let provider = Provider::new(ProviderConfig {
            id: "opengateway".to_string(),
            name: "OpenGateway (gitlawb)".to_string(),
            api_format: ApiFormat::OpenAI,
            base_url: "https://opengateway.gitlawb.com/v1".to_string(),
            api_key: String::new(),
            default_model: "mimo-v2.5-pro".to_string(),
            models: vec![],
            requires_api_key: None,
        });

        let models = provider.discover_models().await;
        assert!(
            models.iter().any(|m| m.contains("mimo")),
            "expected mimo model in discovery list, got: {models:?}"
        );
    }
}

/// Live OpenGateway matrix: proves which assistant-history shapes allow post-tool continuation.
#[cfg(test)]
mod mimo_format_matrix_tests {
    use super::*;
    use crate::config::{ApiFormat, ProviderConfig};
    use futures::StreamExt;

    const BASE: &str = "https://opengateway.gitlawb.com/v1";
    const MODEL: &str = "mimo-v2.5-pro";
    const DEBUG_LOG: &str = "/Users/spencer/GitHub/zero-agent/.cursor/debug-1e2fa8.log";

    fn matrix_log(case: &str, hypothesis_id: &str, ok: bool, detail: &str) {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(DEBUG_LOG)
        {
            use std::io::Write;
            let entry = serde_json::json!({
                "sessionId": "1e2fa8",
                "runId": "matrix",
                "hypothesisId": hypothesis_id,
                "location": "provider.rs:mimo_format_matrix",
                "message": case,
                "data": { "ok": ok, "detail": detail },
                "timestamp": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0)
            });
            let _ = writeln!(f, "{entry}");
        }
        eprintln!("[matrix] {case}: ok={ok} — {detail}");
    }

    fn shell_tool_def() -> serde_json::Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "shell",
                "description": "Run shell command",
                "parameters": {
                    "type": "object",
                    "properties": { "command": { "type": "string" } },
                    "required": ["command"]
                }
            }
        })
    }

    fn base_history(assistant_msg: serde_json::Value) -> serde_json::Value {
        serde_json::json!([
            { "role": "user", "content": "Run pwd using the shell tool" },
            assistant_msg,
            {
                "role": "tool",
                "tool_call_id": "call_matrix_test",
                "content": "/tmp\n"
            }
        ])
    }

    fn assistant_tool_call(content: Option<&str>, reasoning: Option<&str>) -> serde_json::Value {
        let mut msg = serde_json::json!({
            "role": "assistant",
            "tool_calls": [{
                "id": "call_matrix_test",
                "type": "function",
                "function": {
                    "name": "shell",
                    "arguments": "{\"command\":\"pwd\"}"
                }
            }]
        });
        match content {
            Some(c) => msg["content"] = serde_json::json!(c),
            None => msg["content"] = serde_json::Value::Null,
        }
        if let Some(r) = reasoning {
            msg["reasoning_content"] = serde_json::json!(r);
        }
        msg
    }

    async fn post_continuation(messages: serde_json::Value) -> (bool, String) {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("client");
        let body = serde_json::json!({
            "model": MODEL,
            "stream": true,
            "messages": messages,
            "tools": [shell_tool_def()]
        });

        let resp = client
            .post(format!("{BASE}/chat/completions"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        let Ok(resp) = resp else {
            return (false, "request failed".into());
        };
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return (false, format!("HTTP {status}: {text}"));
        }

        let mut sse = String::new();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            if let Ok(bytes) = chunk {
                sse.push_str(&String::from_utf8_lossy(&bytes));
                if sse.contains("[DONE]") || sse.len() > 8000 {
                    break;
                }
            }
        }

        if sse.contains("Param Incorrect") || sse.contains("\"code\":\"400\"") {
            return (false, "SSE error: Param Incorrect".into());
        }
        if sse.contains("\"error\"") {
            let snippet: String = sse.chars().take(300).collect();
            return (false, format!("SSE error: {snippet}"));
        }
        if sse.contains("\"content\"") || sse.contains("finish_reason") {
            return (true, "SSE returned content or finish_reason".into());
        }
        (false, format!("empty/unrecognized SSE: {}", &sse.chars().take(200).collect::<String>()))
    }

    #[test]
    fn tool_call_message_includes_reasoning_after_normalize() {
        let msg = Message {
            role: "assistant".to_string(),
            content: Some("Let me check.".to_string()),
            reasoning_content: None,
            tool_calls: Some(vec![ToolCall {
                id: "call_x".into(),
                call_type: "function".into(),
                function: FunctionCall {
                    name: "shell".into(),
                    arguments: r#"{"command":"pwd"}"#.into(),
                },
            }]),
            tool_call_id: None,
        };
        let history = vec![
            Message {
                role: "user".to_string(),
                content: Some("Run pwd".into()),
                reasoning_content: None,
                tool_calls: None,
                tool_call_id: None,
            },
            msg,
            Message {
                role: "tool".to_string(),
                content: Some("/tmp\n".into()),
                reasoning_content: None,
                tool_calls: None,
                tool_call_id: Some("call_x".into()),
            },
        ];
        let normalized = normalize_thinking_replay(&history);
        let json = serde_json::to_string(&normalized[1]).expect("serialize");
        assert!(
            json.contains("reasoning_content"),
            "expected normalized assistant+tool_calls to include reasoning_content, got: {json}"
        );
    }

    #[tokio::test]
    async fn mimo_post_tool_continuation_format_matrix() {
        let cases: Vec<(&str, &str, serde_json::Value, bool)> = vec![
            (
                "content+tool_calls NO reasoning (zero-agent shape)",
                "B",
                base_history(assistant_tool_call(
                    Some("Let me check."),
                    None,
                )),
                false,
            ),
            (
                "content:null + reasoning_content text",
                "B",
                base_history(assistant_tool_call(None, Some("The user wants pwd."))),
                true,
            ),
            (
                "content + reasoning_content text",
                "B",
                base_history(assistant_tool_call(
                    Some("Let me check."),
                    Some("thinking here"),
                )),
                true,
            ),
            (
                "content:null + empty reasoning_content",
                "B",
                base_history(assistant_tool_call(None, Some(""))),
                true,
            ),
            (
                "content + empty reasoning_content",
                "B",
                base_history(assistant_tool_call(Some("Let me check."), Some(""))),
                true,
            ),
        ];

        let mut failures = Vec::new();
        for (name, hyp, messages, expect_ok) in cases {
            let (ok, detail) = post_continuation(messages).await;
            matrix_log(name, hyp, ok, &detail);
            if ok != expect_ok {
                failures.push(format!(
                    "{name}: expected ok={expect_ok}, got ok={ok} ({detail})"
                ));
            }
        }

        // Provider::stream_chat normalizes thinking replay before send
        let provider = Provider::new(ProviderConfig {
            id: "opengateway".to_string(),
            name: "OpenGateway".to_string(),
            api_format: ApiFormat::OpenAI,
            base_url: BASE.to_string(),
            api_key: String::new(),
            default_model: MODEL.to_string(),
            models: vec![MODEL.to_string()],
            requires_api_key: None,
        });
        let zero_agent_messages = vec![
            Message {
                role: "user".to_string(),
                content: Some("Run pwd using the shell tool".to_string()),
                reasoning_content: None,
                tool_calls: None,
                tool_call_id: None,
            },
            Message {
                role: "assistant".to_string(),
                content: Some("Let me check.".to_string()),
                reasoning_content: None,
                tool_calls: Some(vec![ToolCall {
                    id: "call_matrix_test".into(),
                    call_type: "function".into(),
                    function: FunctionCall {
                        name: "shell".into(),
                        arguments: r#"{"command":"pwd"}"#.into(),
                    },
                }]),
                tool_call_id: None,
            },
            Message {
                role: "tool".to_string(),
                content: Some("/tmp\n".to_string()),
                reasoning_content: None,
                tool_calls: None,
                tool_call_id: Some("call_matrix_test".into()),
            },
        ];
        let tools = vec![ToolDefinition {
            name: "shell".into(),
            description: "Run shell".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": { "command": { "type": "string" } },
                "required": ["command"]
            }),
        }];
        let events = provider
            .stream_chat(&zero_agent_messages, &tools, None)
            .await
            .expect("stream should return Ok even on empty/error SSE");
        let has_content = stream_has_content(&events);
        matrix_log(
            "Provider::stream_chat with normalized Message history",
            "C",
            has_content,
            &format!("events={}", events.len()),
        );
        if !has_content {
            failures.push(
                "Provider::stream_chat returned empty stream after normalization".into(),
            );
        }

        assert!(
            failures.is_empty(),
            "format matrix failures:\n{}",
            failures.join("\n")
        );
    }
}

