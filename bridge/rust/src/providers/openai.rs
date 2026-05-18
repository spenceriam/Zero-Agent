use crate::config::ApiFormat;
use crate::provider::{Message, StreamEvent, StreamEventType, ToolDefinition};
use super::ProviderBackend;
use futures::StreamExt;

pub struct OpenAIProvider {
    id: String,
    name: String,
    base_url: String,
    default_model: String,
}

impl OpenAIProvider {
    pub fn new(id: &str, name: &str, base_url: &str, default_model: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            base_url: base_url.to_string(),
            default_model: default_model.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl ProviderBackend for OpenAIProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn api_format(&self) -> ApiFormat {
        ApiFormat::OpenAI
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn default_model(&self) -> &str {
        &self.default_model
    }

    fn requires_api_key(&self) -> bool {
        !self.base_url.contains("localhost")
    }

    async fn stream_chat(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        api_key: &str,
        model: &str,
    ) -> Result<Vec<StreamEvent>, String> {
        let url = format!("{}/chat/completions", self.base_url);

        let mut body = serde_json::json!({
            "model": model,
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

        let client = reqwest::Client::new();
        let mut req = client
            .post(&url)
            .header("Content-Type", "application/json");

        if !api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {api_key}"));
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
                    for (_index, (id, name, args)) in tool_call_acc.drain() {
                        events.push(StreamEvent {
                            event_type: StreamEventType::ToolCall { id, name, arguments: args },
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

                    if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                        for tc in tool_calls {
                            let index = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0).to_string();
                            let id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                            let name = tc.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("").to_string();
                            let args = tc.get("function").and_then(|f| f.get("arguments")).and_then(|a| a.as_str()).unwrap_or("").to_string();

                            let entry = tool_call_acc.entry(index).or_default();
                            if !id.is_empty() { entry.0 = id; }
                            if !name.is_empty() { entry.1 = name; }
                            entry.2.push_str(&args);
                        }
                    }
                }
            }
        }

        for (_index, (id, name, args)) in tool_call_acc.drain() {
            events.push(StreamEvent {
                event_type: StreamEventType::ToolCall { id, name, arguments: args },
            });
        }

        if !events.iter().any(|e| matches!(e.event_type, StreamEventType::Done)) {
            events.push(StreamEvent {
                event_type: StreamEventType::Done,
            });
        }

        Ok(events)
    }
}
