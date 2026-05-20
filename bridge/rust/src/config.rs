use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub data_dir: String,
    pub default_provider: String,
    pub providers: Vec<ProviderConfig>,
    pub tool_policy: ToolPolicy,
    #[serde(default)]
    pub telegram: TelegramConfig,
    /// Absolute path to the loaded config.json (not serialized).
    #[serde(skip)]
    pub config_path: Option<PathBuf>,
    /// Resolved absolute data directory (not serialized).
    #[serde(skip)]
    pub resolved_data_dir: Option<PathBuf>,
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
    /// When false, empty api_key is allowed. Defaults from provider id when omitted.
    #[serde(default)]
    pub requires_api_key: Option<bool>,
}

impl ProviderConfig {
    pub fn requires_api_key(&self) -> bool {
        if let Some(v) = self.requires_api_key {
            return v;
        }
        match self.id.as_str() {
            "ollama-local" => false,
            _ => !self.base_url.contains("localhost") && !self.base_url.contains("127.0.0.1"),
        }
    }
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
    #[serde(default)]
    pub globally_approved_tools: Vec<String>,
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
            default_provider: "openrouter".to_string(),
            providers: vec![
                ProviderConfig {
                    id: "openrouter".to_string(),
                    name: "OpenRouter".to_string(),
                    api_format: ApiFormat::OpenAI,
                    base_url: "https://openrouter.ai/api/v1".to_string(),
                    api_key: String::new(),
                    default_model: "anthropic/claude-sonnet-4".to_string(),
                    models: vec![],
                    requires_api_key: None,
                },
                ProviderConfig {
                    id: "ollama-cloud".to_string(),
                    name: "Ollama Cloud".to_string(),
                    api_format: ApiFormat::OpenAI,
                    base_url: "https://api.ollama.com/v1".to_string(),
                    api_key: String::new(),
                    default_model: "deepseek-v4-pro".to_string(),
                    models: vec!["deepseek-v4-pro".to_string()],
                    requires_api_key: None,
                },
                ProviderConfig {
                    id: "openai".to_string(),
                    name: "OpenAI".to_string(),
                    api_format: ApiFormat::OpenAI,
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: String::new(),
                    default_model: "gpt-4o".to_string(),
                    models: vec![],
                    requires_api_key: None,
                },
                ProviderConfig {
                    id: "anthropic".to_string(),
                    name: "Anthropic".to_string(),
                    api_format: ApiFormat::Anthropic,
                    base_url: "https://api.anthropic.com".to_string(),
                    api_key: String::new(),
                    default_model: "claude-sonnet-4-20250514".to_string(),
                    models: vec![],
                    requires_api_key: None,
                },
                ProviderConfig {
                    id: "ollama-local".to_string(),
                    name: "Ollama (Local)".to_string(),
                    api_format: ApiFormat::OpenAI,
                    base_url: "http://localhost:11434/v1".to_string(),
                    api_key: String::new(),
                    default_model: "llama3".to_string(),
                    models: vec![],
                    requires_api_key: None,
                },
            ],
            tool_policy: ToolPolicy {
                allow_safe_without_prompt: true,
                ask_before_mutating: false,
                ask_before_destructive: true,
                globally_approved_tools: Vec::new(),
            },
            telegram: TelegramConfig::default(),
            config_path: None,
            resolved_data_dir: None,
        }
    }
}

impl Config {
    pub fn config_path(&self) -> PathBuf {
        self.config_path
            .clone()
            .unwrap_or_else(|| self.data_dir_path().join("config.json"))
    }

    pub fn data_dir_path(&self) -> PathBuf {
        self.resolved_data_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from(&self.data_dir))
    }

    pub fn env_path(&self) -> PathBuf {
        self.data_dir_path().join(".env")
    }

    pub fn soul_path(&self) -> PathBuf {
        self.data_dir_path().join("SOUL.md")
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.resolved_data_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from(&self.data_dir))
            .join("sessions")
    }

    pub fn load() -> Self {
        Self::load_from(None)
    }

    pub fn load_from(explicit_path: Option<&Path>) -> Self {
        let (path, mut config) = if let Some(p) = explicit_path {
            load_config_file(p)
        } else if let Some(found) = discover_config_path() {
            load_config_file(&found)
        } else {
            let default = Self::default();
            let path = default.config_path();
            if path.exists() {
                load_config_file(&path)
            } else {
                let _ = default.save();
                (path, default)
            }
        };

        config.config_path = Some(path.clone());
        config.resolved_data_dir = Some(resolve_data_dir(&config, &path));

        config.load_env();
        let _ = std::fs::create_dir_all(config.sessions_dir());
        config
    }

    pub fn config_display_path(&self) -> String {
        self.config_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| self.config_path().display().to_string())
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
        let path = self
            .config_path
            .clone()
            .unwrap_or_else(|| self.config_path());
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

    pub fn set_default_model(&mut self, model: &str) -> Result<(), String> {
        let provider_id = self.default_provider.clone();
        if let Some(provider) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            provider.default_model = model.to_string();
        }
        self.save()
    }

    pub fn set_default_provider(&mut self, provider_id: &str) -> Result<(), String> {
        if self.get_provider(provider_id).is_none() {
            return Err(format!("unknown provider: {provider_id}"));
        }
        self.default_provider = provider_id.to_string();
        self.save()
    }

    pub fn approve_tool_globally(&mut self, tool_name: &str) -> Result<(), String> {
        if !self
            .tool_policy
            .globally_approved_tools
            .iter()
            .any(|t| t == tool_name)
        {
            self.tool_policy.globally_approved_tools.push(tool_name.to_string());
        }
        self.save()
    }
}

fn load_config_file(path: &Path) -> (PathBuf, Config) {
    let path = path.to_path_buf();
    let default = Config::default();
    let config = if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or(default),
            Err(_) => default,
        }
    } else {
        default
    };
    (path, config)
}

fn discover_config_path() -> Option<PathBuf> {
    if let Ok(mut dir) = std::env::current_dir() {
        loop {
            let candidate = dir.join(".zero-agent").join("config.json");
            if candidate.exists() {
                return Some(candidate);
            }
            if !dir.pop() {
                break;
            }
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let candidate = PathBuf::from(home).join(".zero-agent").join("config.json");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    std::env::var("ZERO_HOME")
        .ok()
        .map(|h| PathBuf::from(h).join("config.json"))
        .filter(|p| p.exists())
}

fn resolve_data_dir(config: &Config, config_path: &Path) -> PathBuf {
    let data = PathBuf::from(&config.data_dir);
    if data.is_absolute() {
        return data;
    }
    config_path
        .parent()
        .map(|p| p.join(&config.data_dir))
        .unwrap_or(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn explicit_requires_api_key_false() {
        let p = ProviderConfig {
            id: "custom".into(),
            name: "Custom".into(),
            api_format: ApiFormat::OpenAI,
            base_url: "https://example.com/v1".into(),
            api_key: String::new(),
            default_model: String::new(),
            models: vec![],
            requires_api_key: Some(false),
        };
        assert!(!p.requires_api_key());
    }

    #[test]
    fn discovers_config_walking_up_from_subdir() {
        let tmp = std::env::temp_dir().join(format!("zero-agent-cfg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("sub/deep")).unwrap();
        fs::create_dir_all(tmp.join(".zero-agent")).unwrap();
        fs::write(
            tmp.join(".zero-agent/config.json"),
            r#"{"data_dir":".zero-agent","default_provider":"openrouter","providers":[],"tool_policy":{"allow_safe_without_prompt":true,"ask_before_mutating":true,"ask_before_destructive":true}}"#,
        )
        .unwrap();

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.join("sub/deep")).unwrap();
        let found = discover_config_path();
        std::env::set_current_dir(prev).unwrap();
        let _ = fs::remove_dir_all(&tmp);

        assert!(found.is_some());
        assert!(found.unwrap().ends_with(".zero-agent/config.json"));
    }
}
