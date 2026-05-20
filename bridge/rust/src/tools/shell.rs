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
        RiskLevel::Safe
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

fn max_risk(a: RiskLevel, b: RiskLevel) -> RiskLevel {
    use RiskLevel::*;
    match (a, b) {
        (Blocked, _) | (_, Blocked) => Blocked,
        (Destructive, _) | (_, Destructive) => Destructive,
        (Mutating, _) | (_, Mutating) => Mutating,
        _ => Safe,
    }
}

fn split_pipeline(command: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut rest = command;
    while !rest.is_empty() {
        let mut split_at = rest.len();
        for (i, _) in rest.match_indices("&&") {
            split_at = split_at.min(i);
        }
        for (i, _) in rest.match_indices("||") {
            split_at = split_at.min(i);
        }
        for (i, c) in rest.char_indices() {
            if c == '|' || c == ';' {
                split_at = split_at.min(i);
            }
        }
        segments.push(rest[..split_at].trim());
        if split_at >= rest.len() {
            break;
        }
        if rest[split_at..].starts_with("&&") {
            rest = rest[split_at + 2..].trim_start();
        } else if rest[split_at..].starts_with("||") {
            rest = rest[split_at + 2..].trim_start();
        } else {
            rest = rest[split_at + 1..].trim_start();
        }
    }
    segments.retain(|s| !s.is_empty());
    if segments.is_empty() {
        segments.push(command.trim());
    }
    segments
}

fn strip_env_prefix(segment: &str) -> &str {
    let mut rest = segment.trim();
    loop {
        let Some(eq) = rest.find('=') else {
            return rest;
        };
        let prefix = &rest[..eq];
        if prefix.is_empty()
            || !prefix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return rest;
        }
        let after = rest[eq + 1..].trim_start();
        let Some(next_space) = after.find(char::is_whitespace) else {
            return "";
        };
        rest = after[next_space..].trim_start();
    }
}

fn words(segment: &str) -> Vec<String> {
    strip_env_prefix(segment)
        .split_whitespace()
        .map(|w| w.to_string())
        .collect()
}

fn classify_git(args: &[String]) -> RiskLevel {
    if args.is_empty() {
        return RiskLevel::Safe;
    }
    if args.len() >= 2 && args[1] == "push" {
        return RiskLevel::Destructive;
    }
    if args.len() >= 2 && args[1] == "clean" {
        return RiskLevel::Destructive;
    }
    if args.len() >= 3 && args[1] == "reset" && args[2] == "--hard" {
        return RiskLevel::Destructive;
    }
    if args.len() >= 2 && (args[1] == "add" || args[1] == "commit") {
        return RiskLevel::Mutating;
    }
    if args.len() >= 2
        && matches!(
            args[1].as_str(),
            "status" | "log" | "diff" | "show" | "branch" | "rev-parse"
        )
    {
        return RiskLevel::Safe;
    }
    RiskLevel::Mutating
}

fn classify_gh(args: &[String]) -> RiskLevel {
    if args.len() < 2 {
        return RiskLevel::Safe;
    }
    match args[1].as_str() {
        "pr" if args.len() >= 3 => match args[2].as_str() {
            "merge" | "close" => RiskLevel::Destructive,
            "create" => RiskLevel::Mutating,
            "list" | "view" | "status" | "checks" => RiskLevel::Safe,
            _ => RiskLevel::Mutating,
        },
        "repo" if args.len() >= 3 => match args[2].as_str() {
            "delete" => RiskLevel::Destructive,
            "view" | "list" => RiskLevel::Safe,
            "clone" | "fork" | "create" => RiskLevel::Mutating,
            _ => RiskLevel::Mutating,
        },
        "release" if args.len() >= 3 && args[2] == "delete" => RiskLevel::Destructive,
        "workflow" if args.len() >= 3 && args[2] == "run" => RiskLevel::Destructive,
        "api" => {
            let joined = args.join(" ").to_ascii_lowercase();
            if joined.contains("-x delete") || joined.contains("--method delete") {
                RiskLevel::Destructive
            } else {
                RiskLevel::Mutating
            }
        }
        "issue" if args.len() >= 3 => match args[2].as_str() {
            "create" => RiskLevel::Mutating,
            "list" | "view" => RiskLevel::Safe,
            _ => RiskLevel::Mutating,
        },
        "run" if args.len() >= 3 && matches!(args[2].as_str(), "list" | "view") => {
            RiskLevel::Safe
        }
        _ => RiskLevel::Safe,
    }
}

fn classify_segment(segment: &str) -> RiskLevel {
    let lower = segment.to_ascii_lowercase();
    if lower.contains(":(){") || lower.contains(">/dev/") || lower.contains("> /dev/") {
        return RiskLevel::Destructive;
    }

    let args = words(segment);
    if args.is_empty() {
        return RiskLevel::Safe;
    }

    let bin = args[0].as_str();
    match bin {
        "rm" | "rmdir" | "dd" | "truncate" | "kill" | "shutdown" | "reboot" | "halt" => {
            RiskLevel::Destructive
        }
        "git" => classify_git(&args),
        "gh" => classify_gh(&args),
        "ls" | "pwd" | "cat" | "head" | "tail" | "grep" | "rg" | "find" | "wc" | "file"
        | "stat" | "tree" | "du" | "df" | "echo" | "which" | "type" | "env" | "printenv"
        | "date" | "uname" | "whoami" | "id" | "less" | "more" | "sort" | "uniq" | "cut"
        | "awk" | "sed" | "printf" | "test" | "true" | "false" => RiskLevel::Safe,
        "mkdir" | "touch" | "cp" | "mv" | "chmod" | "chown" | "npm" | "cargo" | "make"
        | "python" | "python3" | "node" | "rustc" | "go" => RiskLevel::Mutating,
        _ => RiskLevel::Mutating,
    }
}

/// Classify a shell command string by its highest-risk pipeline segment.
pub fn classify_shell_command(command: &str) -> RiskLevel {
    if command.trim().is_empty() {
        return RiskLevel::Safe;
    }
    split_pipeline(command)
        .into_iter()
        .map(classify_segment)
        .fold(RiskLevel::Safe, max_risk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_shell_ls_is_safe() {
        assert_eq!(classify_shell_command("ls -la"), RiskLevel::Safe);
    }

    #[test]
    fn classify_shell_rm_is_destructive() {
        assert_eq!(classify_shell_command("rm -rf dist"), RiskLevel::Destructive);
    }

    #[test]
    fn classify_shell_pipeline_takes_max_risk() {
        assert_eq!(
            classify_shell_command("git add . && git push"),
            RiskLevel::Destructive
        );
    }

    #[test]
    fn classify_git_push_is_destructive() {
        assert_eq!(classify_shell_command("git push origin main"), RiskLevel::Destructive);
    }

    #[test]
    fn classify_git_add_commit_are_mutating() {
        assert_eq!(classify_shell_command("git add ."), RiskLevel::Mutating);
        assert_eq!(
            classify_shell_command("git commit -m test"),
            RiskLevel::Mutating
        );
    }

    #[test]
    fn classify_gh_pr_merge_is_destructive() {
        assert_eq!(
            classify_shell_command("gh pr merge 123"),
            RiskLevel::Destructive
        );
    }

    #[test]
    fn classify_gh_pr_view_is_safe() {
        assert_eq!(classify_shell_command("gh pr view 123"), RiskLevel::Safe);
    }
}
