use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
struct TelegramResponse {
    ok: bool,
    result: Vec<TelegramUpdate>,
}

#[derive(Debug, Deserialize)]
pub struct TelegramUpdate {
    pub update_id: i64,
    pub message: Option<TelegramMessage>,
}

#[derive(Debug, Deserialize)]
pub struct TelegramMessage {
    pub message_id: i64,
    pub from: Option<TelegramUser>,
    pub chat: TelegramChat,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TelegramUser {
    pub id: i64,
    pub first_name: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TelegramChat {
    pub id: i64,
    #[serde(rename = "type")]
    pub chat_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct SendMessageBody {
    chat_id: i64,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to_message_id: Option<i64>,
}

#[derive(Debug, Serialize)]
struct SendChatActionBody {
    chat_id: i64,
    action: String,
}

pub struct TelegramAdapter {
    token: String,
    allowed_users: HashSet<i64>,
    client: reqwest::Client,
}

impl TelegramAdapter {
    pub fn new(token: String, allowed_users: String) -> Self {
        let allowed: HashSet<i64> = if allowed_users.is_empty() {
            HashSet::new()
        } else {
            allowed_users
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect()
        };

        Self {
            token,
            allowed_users: allowed,
            client: reqwest::Client::new(),
        }
    }

    fn api_url(&self, method: &str) -> String {
        format!("https://api.telegram.org/bot{}/{}", self.token, method)
    }

    fn is_authorized(&self, user_id: i64) -> bool {
        self.allowed_users.is_empty() || self.allowed_users.contains(&user_id)
    }

    pub async fn poll(&self, offset: i64) -> Result<Vec<TelegramUpdate>, String> {
        let url = self.api_url("getUpdates");
        let resp = self
            .client
            .get(&url)
            .query(&[("offset", offset.to_string()), ("timeout", "30".to_string())])
            .timeout(std::time::Duration::from_secs(35))
            .send()
            .await
            .map_err(|e| format!("poll request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("poll error {status}: {text}"));
        }

        let body: TelegramResponse = resp
            .json()
            .await
            .map_err(|e| format!("poll parse failed: {e}"))?;

        if !body.ok {
            return Err("Telegram API returned ok=false".to_string());
        }

        Ok(body.result)
    }

    pub async fn send_message(
        &self,
        chat_id: i64,
        text: &str,
        reply_to: Option<i64>,
    ) -> Result<(), String> {
        let url = self.api_url("sendMessage");
        let body = SendMessageBody {
            chat_id,
            text: text.to_string(),
            parse_mode: Some("Markdown".to_string()),
            reply_to_message_id: reply_to,
        };

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("send request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("send error {status}: {text}"));
        }

        Ok(())
    }

    pub async fn send_typing(&self, chat_id: i64) -> Result<(), String> {
        let url = self.api_url("sendChatAction");
        let body = SendChatActionBody {
            chat_id,
            action: "typing".to_string(),
        };

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("typing request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("typing error {status}: {text}"));
        }

        Ok(())
    }

    pub fn process_update(&self, update: &TelegramUpdate) -> Option<(i64, i64, String)> {
        let msg = update.message.as_ref()?;
        let user_id = msg.from.as_ref()?.id;
        let chat_id = msg.chat.id;
        let text = msg.text.as_ref()?;

        if !self.is_authorized(user_id) {
            eprintln!("telegram: unauthorized user {user_id}");
            return None;
        }

        Some((chat_id, msg.message_id, text.clone()))
    }
}

pub fn chunk_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= max_len {
            chunks.push(remaining.to_string());
            break;
        }

        // Find a good break point
        let break_at = remaining[..max_len]
            .rfind("\n\n")
            .or_else(|| remaining[..max_len].rfind('\n'))
            .or_else(|| remaining[..max_len].rfind(' '))
            .unwrap_or(max_len);

        chunks.push(remaining[..break_at].to_string());
        remaining = &remaining[break_at..].trim_start();
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_message_short() {
        let chunks = chunk_message("hello", 100);
        assert_eq!(chunks, vec!["hello"]);
    }

    #[test]
    fn test_chunk_message_long() {
        let text = "line1\n\nline2\n\nline3";
        let chunks = chunk_message(text, 10);
        assert!(chunks.len() > 1);
    }

    #[test]
    fn test_is_authorized_empty() {
        let adapter = TelegramAdapter::new("token".to_string(), String::new());
        assert!(adapter.is_authorized(123));
    }

    #[test]
    fn test_is_authorized_match() {
        let adapter = TelegramAdapter::new("token".to_string(), "123,456".to_string());
        assert!(adapter.is_authorized(123));
        assert!(adapter.is_authorized(456));
        assert!(!adapter.is_authorized(789));
    }
}
