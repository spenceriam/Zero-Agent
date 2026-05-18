use crate::config::ApiFormat;
use crate::provider::{Message, StreamEvent, StreamEventType, ToolDefinition};
use super::ProviderBackend;
use futures::StreamExt;

pub struct AnthropicProvider {
    base_url: String,
    default_model: String,
}

impl AnthropicProvider {
    pub fn new() -> Self {
        Self {
            base_url: "https://api.anthropic.com".to_string(),
            default_model: "claude-sonnet-4-20250514".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl ProviderBackend for AnthropicProvider {
    fn id(&self) -> &str {
        "anthropic"
    }

    fn name(&self) -> &str {
        "Anthropic"
    }

    fn api_format(&self) -> ApiFormat {
        ApiFormat::Anthropic
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn default_model(&self) -> &str {
        &self.default_model
    }

    fn requires_api_key(&self) -> bool {
        true
    }

    async fn stream_chat(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        api_key: &str,
        model: &str,
    ) -> Result<Vec<StreamEvent>, String> {
        let url = format!("{}/v1/messages", self.base_url);

        // Split system message from conversation messages
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
            "model": model,
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

        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("x-api-key", api_key)
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
                                current_tool_id = cb.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                                current_tool_name = cb.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                                current_tool_args.clear();
                            }
                        }
                    }
                    "content_block_delta" => {
                        if let Some(delta) = json.get("delta") {
                            if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                events.push(StreamEvent {
                                    event_type: StreamEventType::Text(text.to_string()),
                                });
                            }
                            if let Some(partial) = delta.get("partial_json").and_then(|p| p.as_str()) {
                                current_tool_args.push_str(partial);
                            }
                        }
                    }
                    "content_block_stop" => {
                        if !current_tool_name.is_empty() {
                            events.push(StreamEvent {
                                event_type: StreamEventType::ToolCall {
                                    id: current_tool_id.clone(),
                                    name: current_tool_name.clone(),
                                    arguments: current_tool_args.clone(),
                                },
                            });
                            current_tool_id.clear();
                            current_tool_name.clear();
                            current_tool_args.clear();
                        }
                    }
                    "message_stop" => {
                        events.push(StreamEvent {
                            event_type: StreamEventType::Done,
                        });
                    }
                    _ => {}
                }
            }
        }

        if !events.iter().any(|e| matches!(e.event_type, StreamEventType::Done)) {
            events.push(StreamEvent {
                event_type: StreamEventType::Done,
            });
        }

        Ok(events)
    }
}
