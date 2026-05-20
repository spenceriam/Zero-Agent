pub mod fs;
pub mod shell;

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Resolve tool risk for a specific invocation (shell uses command classification).
pub fn effective_risk(tool_name: &str, args: &Value, registry: &ToolRegistry) -> RiskLevel {
    if tool_name == "shell" {
        if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
            return shell::classify_shell_command(cmd);
        }
        return RiskLevel::Mutating;
    }
    registry
        .get(tool_name)
        .map(|t| t.risk_level())
        .unwrap_or(RiskLevel::Safe)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn approval_only_for_destructive_risk() {
        let registry = ToolRegistry::default();
        let safe = effective_risk("shell", &json!({"command": "ls -la"}), &registry);
        assert_eq!(safe, RiskLevel::Safe);
        let destructive =
            effective_risk("shell", &json!({"command": "git push"}), &registry);
        assert_eq!(destructive, RiskLevel::Destructive);
        let write = effective_risk("write_file", &json!({"path": "x", "contents": "y"}), &registry);
        assert_eq!(write, RiskLevel::Mutating);
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
