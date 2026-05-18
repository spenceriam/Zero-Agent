use crate::config::{ApiFormat, ProviderConfig};
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

#[derive(Debug)]
pub struct StreamEvent {
    pub event_type: StreamEventType,
}

#[derive(Debug)]
pub enum StreamEventType {
    Text(String),
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    Done,
}

pub struct Provider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl Provider {
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    pub fn id(&self) -> &str {
        &self.config.id
    }

    pub fn model(&self) -> &str {
        &self.config.default_model
    }

    pub async fn stream_chat(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<Vec<StreamEvent>, String> {
        match self.config.api_format {
            ApiFormat::OpenAI => self.stream_openai(messages, tools).await,
            ApiFormat::Anthropic => self.stream_anthropic(messages, tools).await,
        }
    }

    async fn stream_openai(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<Vec<StreamEvent>, String> {
        let url = format!("{}/chat/completions", self.config.base_url);

        let mut body = serde_json::json!({
            "model": self.config.default_model,
            "messages": messages,
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
        // Accumulate tool call arguments across chunks
        // (id, name, accumulated_arguments)
        let mut tool_call_acc: std::collections::HashMap<String, (String, String, String)> =
            std::collections::HashMap::new();

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
                if data == "[DONE]" {
                    // Flush accumulated tool calls
                    for (_index, (id, name, args)) in tool_call_acc.drain() {
                        events.push(StreamEvent {
                            event_type: StreamEventType::ToolCall {
                                id,
                                name,
                                arguments: args,
                            },
                        });
                    }
                    events.push(StreamEvent {
                        event_type: StreamEventType::Done,
                    });
                    continue;
                }

                let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
                    continue;
                };

                // Extract text content
                if let Some(delta) = json
                    .get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("delta"))
                {
                    if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                        if !content.is_empty() {
                            events.push(StreamEvent {
                                event_type: StreamEventType::Text(content.to_string()),
                            });
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
            events.push(StreamEvent {
                event_type: StreamEventType::ToolCall {
                    id,
                    name,
                    arguments: args,
                },
            });
        }

        if !events.iter().any(|e| matches!(e.event_type, StreamEventType::Done)) {
            events.push(StreamEvent {
                event_type: StreamEventType::Done,
            });
        }

        Ok(events)
    }

    async fn stream_anthropic(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<Vec<StreamEvent>, String> {
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
                                events.push(StreamEvent {
                                    event_type: StreamEventType::Text(text.to_string()),
                                });
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

