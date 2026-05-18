pub mod openai;
pub mod anthropic;

use crate::provider::{Message, StreamEvent, ToolDefinition};
use crate::config::ApiFormat;

#[async_trait::async_trait]
pub trait ProviderBackend: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn api_format(&self) -> ApiFormat;
    fn base_url(&self) -> &str;
    fn default_model(&self) -> &str;
    fn requires_api_key(&self) -> bool;

    async fn stream_chat(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        api_key: &str,
        model: &str,
    ) -> Result<Vec<StreamEvent>, String>;
}

pub struct ProviderRegistry {
    providers: Vec<Box<dyn ProviderBackend>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: Box<dyn ProviderBackend>) {
        self.providers.push(provider);
    }

    pub fn get(&self, id: &str) -> Option<&dyn ProviderBackend> {
        self.providers.iter().find(|p| p.id() == id).map(|p| &**p)
    }

    pub fn list(&self) -> Vec<(&str, &str)> {
        self.providers.iter().map(|p| (p.id(), p.name())).collect()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(openai::OpenAIProvider::new(
            "openrouter",
            "OpenRouter",
            "https://openrouter.ai/api/v1",
            "anthropic/claude-sonnet-4",
        )));
        registry.register(Box::new(openai::OpenAIProvider::new(
            "openai",
            "OpenAI",
            "https://api.openai.com/v1",
            "gpt-4o",
        )));
        registry.register(Box::new(openai::OpenAIProvider::new(
            "ollama",
            "Ollama (Local)",
            "http://localhost:11434/v1",
            "llama3",
        )));
        registry.register(Box::new(anthropic::AnthropicProvider::new()));
        registry
    }
}
