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

    pub fn env_path(&self) -> PathBuf {
        PathBuf::from(&self.data_dir).join(".env")
    }

    pub fn soul_path(&self) -> PathBuf {
        PathBuf::from(&self.data_dir).join("SOUL.md")
    }

    pub fn sessions_dir(&self) -> PathBuf {
        PathBuf::from(&self.data_dir).join("sessions")
    }

    pub fn load() -> Self {
        let default = Self::default();
        let path = default.config_path();
        let mut config = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(contents) => serde_json::from_str(&contents).unwrap_or(default),
                Err(_) => default,
            }
        } else {
            let _ = default.save();
            default
        };

        // Load .env file for API keys
        config.load_env();

        // Ensure directories exist
        let _ = std::fs::create_dir_all(config.sessions_dir());

        config
    }

    fn load_env(&mut self) {
        let env_path = self.env_path();
        if !env_path.exists() {
            return;
        }

        let Ok(contents) = std::fs::read_to_string(&env_path) else {
            return;
        };

        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');

                // Apply env values to config
                match key {
                    "OPENROUTER_API_KEY" => self.set_provider_key("openrouter", value),
                    "OPENAI_API_KEY" => self.set_provider_key("openai", value),
                    "ANTHROPIC_API_KEY" => self.set_provider_key("anthropic", value),
                    "TELEGRAM_BOT_TOKEN" => self.telegram.bot_token = value.to_string(),
                    "TELEGRAM_ALLOWED_USERS" => self.telegram.allowed_users = value.to_string(),
                    _ => {}
                }
            }
        }
    }

    fn set_provider_key(&mut self, provider_id: &str, key: &str) {
        if let Some(provider) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            provider.api_key = key.to_string();
        }
    }

    pub fn load_soul(&self) -> Option<String> {
        let soul_path = self.soul_path();
        if soul_path.exists() {
            std::fs::read_to_string(&soul_path).ok()
        } else {
            None
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
