pub mod fs;
pub mod shell;

use serde_json::Value;

#[derive(Debug, Clone)]
pub enum RiskLevel {
    Safe,
    Mutating,
    Destructive,
    Blocked,
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn risk_level(&self) -> RiskLevel;
    fn input_schema(&self) -> Value;
    fn execute(&self, args: &Value) -> String;
}

pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.iter().find(|t| t.name() == name).map(|t| &**t)
    }

    pub fn list(&self) -> Vec<(&str, &str, RiskLevel)> {
        self.tools
            .iter()
            .map(|t| (t.name(), t.description(), t.risk_level()))
            .collect()
    }

    pub fn definitions(&self) -> Vec<crate::provider::ToolDefinition> {
        self.tools
            .iter()
            .map(|t| crate::provider::ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                parameters: t.input_schema(),
            })
            .collect()
    }

    pub fn execute(&self, name: &str, args: &Value) -> String {
        match self.get(name) {
            Some(tool) => tool.execute(args),
            None => format!("Unknown tool: {name}"),
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(fs::ReadFileTool));
        registry.register(Box::new(fs::WriteFileTool));
        registry.register(Box::new(fs::EditFileTool));
        registry.register(Box::new(fs::GlobTool));
        registry.register(Box::new(shell::ShellTool));
        registry
    }
}
