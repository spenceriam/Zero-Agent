use super::{RiskLevel, Tool};
use serde_json::{json, Value};

pub struct ShellTool;

impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Run a shell command and return stdout, stderr, and exit code"
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Mutating
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute"
                }
            },
            "required": ["command"]
        })
    }

    fn execute(&self, args: &Value) -> String {
        let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
        let shell = if cfg!(target_os = "windows") {
            ("cmd", "/C")
        } else {
            ("sh", "-c")
        };
        match std::process::Command::new(shell.0)
            .arg(shell.1)
            .arg(command)
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let code = output.status.code().unwrap_or(-1);
                let mut result = String::new();
                if !stdout.is_empty() {
                    result.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !result.is_empty() {
                        result.push('\n');
                    }
                    result.push_str(&format!("stderr: {stderr}"));
                }
                if code != 0 {
                    result.push_str(&format!("\nexit code: {code}"));
                }
                if result.is_empty() {
                    "(no output)".to_string()
                } else {
                    result
                }
            }
            Err(e) => format!("Error running command: {e}"),
        }
    }
}
