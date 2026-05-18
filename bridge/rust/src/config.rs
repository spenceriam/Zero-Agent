use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub data_dir: String,
    pub default_provider: String,
    pub providers: Vec<ProviderConfig>,
    pub tool_policy: ToolPolicy,
    #[serde(default)]
    pub telegram: TelegramConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub api_format: ApiFormat,
    pub base_url: String,
    pub api_key: String,
    pub default_model: String,
    #[serde(default)]
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ApiFormat {
    OpenAI,
    Anthropic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPolicy {
    pub allow_safe_without_prompt: bool,
    pub ask_before_mutating: bool,
    pub ask_before_destructive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TelegramConfig {
    #[serde(default)]
    pub bot_token: String,
    #[serde(default)]
    pub allowed_users: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: ".zero-agent".to_string(),
            default_provider: "ollama-cloud".to_string(),
            providers: vec![
                ProviderConfig {
                    id: "ollama-cloud".to_string(),
                    name: "Ollama Cloud".to_string(),
                    api_format: ApiFormat::OpenAI,
                    base_url: "https://api.ollama.com/v1".to_string(),
                    api_key: String::new(),
                    default_model: "deepseek-v4-pro".to_string(),
                    models: vec!["deepseek-v4-pro".to_string()],
                },
                ProviderConfig {
                    id: "openrouter".to_string(),
                    name: "OpenRouter".to_string(),
                    api_format: ApiFormat::OpenAI,
                    base_url: "https://openrouter.ai/api/v1".to_string(),
                    api_key: String::new(),
                    default_model: String::new(),
                    models: vec![],
                },
                ProviderConfig {
                    id: "openai".to_string(),
                    name: "OpenAI".to_string(),
                    api_format: ApiFormat::OpenAI,
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: String::new(),
                    default_model: "gpt-4o".to_string(),
                    models: vec![],
                },
                ProviderConfig {
                    id: "anthropic".to_string(),
                    name: "Anthropic".to_string(),
                    api_format: ApiFormat::Anthropic,
                    base_url: "https://api.anthropic.com".to_string(),
                    api_key: String::new(),
                    default_model: "claude-sonnet-4-20250514".to_string(),
                    models: vec![],
                },
                ProviderConfig {
                    id: "ollama-local".to_string(),
                    name: "Ollama (Local)".to_string(),
                    api_format: ApiFormat::OpenAI,
                    base_url: "http://localhost:11434/v1".to_string(),
                    api_key: String::new(),
                    default_model: "llama3".to_string(),
                    models: vec![],
                },
            ],
            tool_policy: ToolPolicy {
                allow_safe_without_prompt: true,
                ask_before_mutating: true,
                ask_before_destructive: true,
            },
            telegram: TelegramConfig::default(),
        }
    }
}

impl Config {
    pub fn config_path(&self) -> PathBuf {
        PathBuf::from(&self.data_dir).join("config.json")
    }

    pub fn load() -> Self {
        let default = Self::default();
        let path = default.config_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(contents) => serde_json::from_str(&contents).unwrap_or(default),
                Err(_) => default,
            }
        } else {
            // Auto-create default config on first run
            let _ = default.save();
            default
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = self.config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create config dir: {e}"))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("failed to serialize config: {e}"))?;
        std::fs::write(&path, json)
            .map_err(|e| format!("failed to write config: {e}"))?;
        Ok(())
    }

    pub fn get_provider(&self, id: &str) -> Option<&ProviderConfig> {
        self.providers.iter().find(|p| p.id == id)
    }

    pub fn default_provider(&self) -> &ProviderConfig {
        self.get_provider(&self.default_provider)
            .or_else(|| self.providers.first())
            .expect("no providers configured")
    }
}
