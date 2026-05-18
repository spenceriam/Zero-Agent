use crate::provider::ToolDefinition;
use serde_json::json;

pub fn builtin_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Read the contents of a file at the given path".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative path to the file"
                    }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "write_file".to_string(),
            description: "Write content to a file, creating directories as needed".to_string(),
            parameters: json!({
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
            }),
        },
        ToolDefinition {
            name: "edit_file".to_string(),
            description: "Edit a file by replacing an exact string match with new content".to_string(),
            parameters: json!({
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
            }),
        },
        ToolDefinition {
            name: "shell".to_string(),
            description: "Run a shell command and return stdout, stderr, and exit code".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute"
                    }
                },
                "required": ["command"]
            }),
        },
        ToolDefinition {
            name: "glob".to_string(),
            description: "Find files matching a glob pattern".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern (e.g. '**/*.rs', 'src/*.toml')"
                    }
                },
                "required": ["pattern"]
            }),
        },
    ]
}

pub fn execute_tool(name: &str, args: &serde_json::Value) -> String {
    match name {
        "read_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match std::fs::read_to_string(path) {
                Ok(contents) => contents,
                Err(e) => format!("Error reading file: {e}"),
            }
        }
        "write_file" => {
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
        "edit_file" => {
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
        "shell" => {
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
        "glob" => {
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
        _ => format!("Unknown tool: {name}"),
    }
}
