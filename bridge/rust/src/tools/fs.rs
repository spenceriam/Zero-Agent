use super::{RiskLevel, Tool};
use serde_json::{json, Value};

pub struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file at the given path"
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute or relative path to the file"
                }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, args: &Value) -> String {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        match std::fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(e) => format!("Error reading file: {e}"),
        }
    }
}

pub struct WriteFileTool;

impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file, creating directories as needed"
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Mutating
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute or relative path to the file"
                },
                "contents": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["path", "contents"]
        })
    }

    fn execute(&self, args: &Value) -> String {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let contents = args.get("contents").and_then(|v| v.as_str()).unwrap_or("");
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return format!("Error creating directory: {e}");
                }
            }
        }
        match std::fs::write(path, contents) {
            Ok(()) => format!("File written: {path}"),
            Err(e) => format!("Error writing file: {e}"),
        }
    }
}

pub struct EditFileTool;

impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing an exact string match with new content"
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Mutating
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "Exact string to find and replace (must be unique in file)"
                },
                "new_string": {
                    "type": "string",
                    "description": "Replacement string"
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    fn execute(&self, args: &Value) -> String {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let old = args.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
        let new = args.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
        match std::fs::read_to_string(path) {
            Ok(contents) => {
                let count = contents.matches(old).count();
                if count == 0 {
                    "Error: old_string not found in file".to_string()
                } else if count > 1 {
                    "Error: old_string matches multiple times; provide more context".to_string()
                } else {
                    let new_contents = contents.replacen(old, new, 1);
                    match std::fs::write(path, &new_contents) {
                        Ok(()) => format!("File edited: {path}"),
                        Err(e) => format!("Error writing file: {e}"),
                    }
                }
            }
            Err(e) => format!("Error reading file: {e}"),
        }
    }
}

pub struct GlobTool;

impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern"
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Safe
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern (e.g. '**/*.rs', 'src/*.toml')"
                }
            },
            "required": ["pattern"]
        })
    }

    fn execute(&self, args: &Value) -> String {
        let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
        match glob::glob(pattern) {
            Ok(paths) => {
                let matches: Vec<String> = paths
                    .filter_map(|p| p.ok())
                    .filter_map(|p| p.to_str().map(|s| s.to_string()))
                    .collect();
                if matches.is_empty() {
                    "No files found".to_string()
                } else {
                    matches.join("\n")
                }
            }
            Err(e) => format!("Invalid glob pattern: {e}"),
        }
    }
}
