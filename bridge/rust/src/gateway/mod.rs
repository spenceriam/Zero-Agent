pub mod telegram;

use telegram::TelegramAdapter;

pub struct GatewayConfig {
    pub token: String,
    pub allowed_users: String,
    pub api_key: String,
    pub model: String,
    pub provider: String,
    pub session_dir: String,
}

pub async fn run_gateway(config: GatewayConfig) -> Result<(), String> {
    let adapter = TelegramAdapter::new(config.token.clone(), config.allowed_users.clone());
    let mut offset: i64 = 0;

    eprintln!("gateway: starting Telegram polling loop");

    loop {
        match adapter.poll(offset).await {
            Ok(updates) => {
                for update in &updates {
                    if update.update_id >= offset {
                        offset = update.update_id + 1;
                    }

                    if let Some((chat_id, message_id, text)) = adapter.process_update(update) {
                        if text.starts_with('/') {
                            handle_command(&adapter, chat_id, message_id, &text).await;
                        } else {
                            handle_message(&adapter, chat_id, message_id, &text, &config).await;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("gateway: poll error: {e}");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}

async fn handle_command(adapter: &TelegramAdapter, chat_id: i64, message_id: i64, text: &str) {
    let cmd = text.split_whitespace().next().unwrap_or("");
    let response = match cmd {
        "/start" => "Welcome to Zero-Agent! Send me a message to get started.",
        "/help" => {
            "Available commands:\n\
             /start - Start session\n\
             /help - Show help\n\
             /status - Show status\n\
             /provider - Show current provider"
        }
        "/status" => "Zero-Agent is running.",
        "/provider" => "Provider: openrouter\nModel: anthropic/claude-sonnet-4",
        _ => "Unknown command. Type /help for available commands.",
    };

    if let Err(e) = adapter.send_message(chat_id, response, Some(message_id)).await {
        eprintln!("gateway: failed to send command response: {e}");
    }
}

async fn handle_message(
    adapter: &TelegramAdapter,
    chat_id: i64,
    message_id: i64,
    text: &str,
    config: &GatewayConfig,
) {
    // Send typing indicator
    let _ = adapter.send_typing(chat_id).await;

    // Build agent request
    let session_path = format!("{}/{}.jsonl", config.session_dir, chat_id);
    let agent_request = serde_json::json!({
        "id": "gateway",
        "op": "agent.run",
        "provider": config.provider,
        "model": config.model,
        "api_key": config.api_key,
        "prompt": text,
        "session_path": session_path
    });

    // Call agent.run via bridge
    let agent_response = crate::agent_run("gateway", &agent_request.to_string());
    let response_text = extract_agent_response_text(&agent_response);

    if response_text.is_empty() {
        let _ = adapter
            .send_message(chat_id, "I couldn't generate a response.", Some(message_id))
            .await;
        return;
    }

    // Chunk and send response
    let chunks = telegram::chunk_message(&response_text, 4096);
    for chunk in chunks {
        if let Err(e) = adapter.send_message(chat_id, &chunk, Some(message_id)).await {
            eprintln!("gateway: failed to send response: {e}");
        }
    }
}

fn extract_agent_response_text(response: &str) -> String {
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(response) else {
        return String::new();
    };

    let Some(events) = parsed.get("events").and_then(|e| e.as_array()) else {
        return String::new();
    };

    let mut text = String::new();
    for event in events {
        if let Some(kind) = event.get("kind").and_then(|k| k.as_str()) {
            if kind == "text" {
                if let Some(t) = event.get("text").and_then(|t| t.as_str()) {
                    text.push_str(t);
                }
            }
        }
    }

    text
}
