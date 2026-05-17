#[cfg(feature = "tui")]
mod tui;

use std::char;
use std::io::{self, BufRead, Write};

fn main() -> io::Result<()> {
    // Check for TUI mode
    #[cfg(feature = "tui")]
    {
        let args: Vec<String> = std::env::args().collect();
        if args.contains(&"tui".to_string()) {
            return tui::run();
        }
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = handle_line(&line);
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }

    Ok(())
}

fn handle_line(line: &str) -> String {
    if !looks_like_json_object(line) {
        return error_response("unknown", "invalid bridge request");
    }

    let id = match json_field(line, "id") {
        Ok(Some(id)) => id,
        Ok(None) => "unknown".to_string(),
        Err(_) => return error_response("unknown", "invalid bridge request"),
    };

    let op = match json_field(line, "op") {
        Ok(Some(op)) => op,
        Ok(None) => String::new(),
        Err(_) => return error_response(&id, "invalid bridge request"),
    };

    match op.as_str() {
        "ping" => format!(
            "{{\"id\":{},\"ok\":true,\"event\":\"pong\",\"output\":{{\"ready\":true}}}}",
            json_string(&id)
        ),
        "config.default" => format!(
            "{{\"id\":{},\"ok\":true,\"event\":\"config\",\"output\":{}}}",
            json_string(&id),
            default_config_json()
        ),
        "config.validate" => validate_config(&id, line),
        "config.write" => write_config(&id, line),
        "config.read" => read_config(&id, line),
        "session.append" => session_append(&id, line),
        "session.read" => session_read(&id, line),
        "session.list" => session_list(&id, line),
        "fs.read" => fs_read(&id, line),
        "fs.write" => fs_write(&id, line),
        "fs.edit" => fs_edit(&id, line),
        "fs.glob" => fs_glob(&id, line),
        "shell.run" => shell_run(&id, line),
        "memory.save" => memory_save(&id, line),
        "memory.list" => memory_list(&id, line),
        "extension.list" => extension_list(&id, line),
        "extension.read" => extension_read(&id, line),
        "extension.write" => extension_write(&id, line),
        "models.discover" => models_discover(&id, line),
        "setup.run" => setup_run(&id),
        "setup.save" => setup_save(&id, line),
        "agent.run" => agent_run(&id, line),
        "agent.tool" => agent_tool(&id, line),
        "telegram.poll" => telegram_poll(&id, line),
        "telegram.send" => telegram_send(&id, line),
        "telegram.loop" => telegram_loop(&id, line),
        "http.request" => http_request(&id, line),
        "http.stream_sse" => http_stream_sse(&id, line),
        "process.spawn" => {
            format!(
                "{{\"id\":{},\"ok\":false,\"event\":\"not_implemented\",\"output\":{{\"op\":{}}},\"error\":\"process.spawn is not implemented yet\"}}",
                json_string(&id),
                json_string(&op)
            )
        }
        _ => error_response(&id, "unknown bridge op"),
    }
}

fn default_config_json() -> &'static str {
    "{\"data_dir\":\".zero-agent\",\"default_provider\":\"openrouter\",\"default_model\":\"\",\"api_key_env\":\"\",\"telegram\":{\"bot_token\":\"\",\"allowed_users\":\"\"},\"tool_policy\":{\"allow_safe_without_prompt\":true,\"ask_before_mutating\":true,\"ask_before_destructive\":true}}"
}

fn validate_config(id: &str, line: &str) -> String {
    let data_dir = match json_field(line, "data_dir") {
        Ok(Some(value)) => value,
        Ok(None) => return config_validation_response(id, false, "missing data_dir"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let default_provider = match json_field(line, "default_provider") {
        Ok(Some(value)) => value,
        Ok(None) => return config_validation_response(id, false, "missing default_provider"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    if data_dir.is_empty() {
        return config_validation_response(id, false, "data_dir must not be empty");
    }
    if default_provider.is_empty() {
        return config_validation_response(id, false, "default_provider must not be empty");
    }

    config_validation_response(id, true, "")
}

fn config_validation_response(id: &str, valid: bool, error: &str) -> String {
    format!(
        "{{\"id\":{},\"ok\":true,\"event\":\"config.validation\",\"output\":{{\"valid\":{},\"error\":{}}}}}",
        json_string(id),
        valid,
        json_string(error)
    )
}

fn write_config(id: &str, line: &str) -> String {
    let path = match json_field(line, "path") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing path"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let contents = match json_field(line, "contents") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing contents"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    if let Some(parent) = std::path::Path::new(&path).parent()
        && !parent.as_os_str().is_empty()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        return error_response(id, &format!("failed to create config directory: {error}"));
    }

    match std::fs::write(&path, contents) {
        Ok(()) => format!(
            "{{\"id\":{},\"ok\":true,\"event\":\"config.written\",\"output\":{{\"path\":{}}}}}",
            json_string(id),
            json_string(&path)
        ),
        Err(error) => error_response(id, &format!("failed to write config: {error}")),
    }
}

fn read_config(id: &str, line: &str) -> String {
    let path = match json_field(line, "path") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing path"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    match std::fs::read_to_string(&path) {
        Ok(contents) => format!(
            "{{\"id\":{},\"ok\":true,\"event\":\"config.read\",\"output\":{{\"path\":{},\"contents\":{}}}}}",
            json_string(id),
            json_string(&path),
            json_string(&contents)
        ),
        Err(error) => error_response(id, &format!("failed to read config: {error}")),
    }
}

fn session_append(id: &str, line: &str) -> String {
    let path = match json_field(line, "path") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing path"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let contents = match json_field(line, "contents") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing contents"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    if let Some(parent) = std::path::Path::new(&path).parent()
        && !parent.as_os_str().is_empty()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        return error_response(id, &format!("failed to create session directory: {error}"));
    }

    let mut file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) => return error_response(id, &format!("failed to open session file: {error}")),
    };

    use std::io::Write;
    match writeln!(file, "{contents}") {
        Ok(()) => format!(
            "{{\"id\":{},\"ok\":true,\"event\":\"session.appended\",\"output\":{{\"path\":{}}}}}",
            json_string(id),
            json_string(&path)
        ),
        Err(error) => error_response(id, &format!("failed to append to session: {error}")),
    }
}

fn session_read(id: &str, line: &str) -> String {
    let path = match json_field(line, "path") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing path"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            let lines: Vec<&str> = contents.lines().filter(|l| !l.trim().is_empty()).collect();
            let mut json_lines = String::from("[");
            for (i, l) in lines.iter().enumerate() {
                if i > 0 {
                    json_lines.push(',');
                }
                json_lines.push_str(json_string(l).as_str());
            }
            json_lines.push(']');
            format!(
                "{{\"id\":{},\"ok\":true,\"event\":\"session.read\",\"output\":{{\"path\":{},\"lines\":{}}}}}",
                json_string(id),
                json_string(&path),
                json_lines
            )
        }
        Err(error) => error_response(id, &format!("failed to read session: {error}")),
    }
}

fn session_list(id: &str, line: &str) -> String {
    let dir = match json_field(line, "dir") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing dir"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) => return error_response(id, &format!("failed to list sessions: {error}")),
    };

    let mut sessions = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "jsonl")
            && let Some(name) = path.file_stem().and_then(|n| n.to_str())
        {
            sessions.push(name.to_string());
        }
    }

    let mut json_list = String::from("[");
    for (i, s) in sessions.iter().enumerate() {
        if i > 0 {
            json_list.push(',');
        }
        json_list.push_str(json_string(s).as_str());
    }
    json_list.push(']');

    format!(
        "{{\"id\":{},\"ok\":true,\"event\":\"session.list\",\"output\":{{\"dir\":{},\"sessions\":{}}}}}",
        json_string(id),
        json_string(&dir),
        json_list
    )
}

fn fs_read(id: &str, line: &str) -> String {
    let path = match json_field(line, "path") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing path"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    match std::fs::read_to_string(&path) {
        Ok(contents) => format!(
            "{{\"id\":{},\"ok\":true,\"event\":\"fs.read\",\"output\":{{\"path\":{},\"contents\":{}}}}}",
            json_string(id),
            json_string(&path),
            json_string(&contents)
        ),
        Err(error) => error_response(id, &format!("failed to read file: {error}")),
    }
}

fn fs_write(id: &str, line: &str) -> String {
    let path = match json_field(line, "path") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing path"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let contents = match json_field(line, "contents") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing contents"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    if let Some(parent) = std::path::Path::new(&path).parent()
        && !parent.as_os_str().is_empty()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        return error_response(id, &format!("failed to create directory: {error}"));
    }

    match std::fs::write(&path, contents) {
        Ok(()) => format!(
            "{{\"id\":{},\"ok\":true,\"event\":\"fs.written\",\"output\":{{\"path\":{}}}}}",
            json_string(id),
            json_string(&path)
        ),
        Err(error) => error_response(id, &format!("failed to write file: {error}")),
    }
}

fn fs_edit(id: &str, line: &str) -> String {
    let path = match json_field(line, "path") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing path"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let old_string = match json_field(line, "old_string") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing old_string"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let new_string = match json_field(line, "new_string") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing new_string"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) => return error_response(id, &format!("failed to read file: {error}")),
    };

    let count = contents.matches(&old_string).count();
    if count == 0 {
        return error_response(id, "old_string not found in file");
    }
    if count > 1 {
        return error_response(id, "old_string matches multiple times; provide more context");
    }

    let new_contents = contents.replacen(&old_string, &new_string, 1);
    match std::fs::write(&path, &new_contents) {
        Ok(()) => format!(
            "{{\"id\":{},\"ok\":true,\"event\":\"fs.edited\",\"output\":{{\"path\":{}}}}}",
            json_string(id),
            json_string(&path)
        ),
        Err(error) => error_response(id, &format!("failed to write file: {error}")),
    }
}

fn fs_glob(id: &str, line: &str) -> String {
    let pattern = match json_field(line, "pattern") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing pattern"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    let root = match json_field(line, "root") {
        Ok(Some(value)) => value,
        Ok(None) => ".".to_string(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    let glob_pattern = if root == "." {
        pattern.clone()
    } else {
        format!("{}/{}", root.trim_end_matches('/'), pattern)
    };

    let mut matches = Vec::new();
    if let Ok(paths) = glob::glob(&glob_pattern) {
        for path in paths.flatten() {
            if let Some(path_str) = path.to_str() {
                matches.push(path_str.to_string());
            }
        }
    }

    let mut json_matches = String::from("[");
    for (i, m) in matches.iter().enumerate() {
        if i > 0 {
            json_matches.push(',');
        }
        json_matches.push_str(json_string(m).as_str());
    }
    json_matches.push(']');

    format!(
        "{{\"id\":{},\"ok\":true,\"event\":\"fs.glob\",\"output\":{{\"pattern\":{},\"matches\":{}}}}}",
        json_string(id),
        json_string(&pattern),
        json_matches
    )
}

fn shell_run(id: &str, line: &str) -> String {
    let command = match json_field(line, "command") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing command"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    let shell = if cfg!(target_os = "windows") {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    };

    let output = match std::process::Command::new(shell.0)
        .arg(shell.1)
        .arg(&command)
        .output()
    {
        Ok(output) => output,
        Err(error) => return error_response(id, &format!("failed to run command: {error}")),
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    format!(
        "{{\"id\":{},\"ok\":true,\"event\":\"shell.run\",\"output\":{{\"command\":{},\"exit_code\":{},\"stdout\":{},\"stderr\":{}}}}}",
        json_string(id),
        json_string(&command),
        exit_code,
        json_string(&stdout),
        json_string(&stderr)
    )
}

fn memory_save(id: &str, line: &str) -> String {
    let path = match json_field(line, "path") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing path"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let contents = match json_field(line, "contents") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing contents"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    if let Some(parent) = std::path::Path::new(&path).parent()
        && !parent.as_os_str().is_empty()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        return error_response(id, &format!("failed to create memory directory: {error}"));
    }

    let mut file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) => return error_response(id, &format!("failed to open memory file: {error}")),
    };

    use std::io::Write;
    match writeln!(file, "{contents}") {
        Ok(()) => format!(
            "{{\"id\":{},\"ok\":true,\"event\":\"memory.saved\",\"output\":{{\"path\":{}}}}}",
            json_string(id),
            json_string(&path)
        ),
        Err(error) => error_response(id, &format!("failed to save memory: {error}")),
    }
}

fn memory_list(id: &str, line: &str) -> String {
    let path = match json_field(line, "path") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing path"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            let lines: Vec<&str> = contents.lines().filter(|l| !l.trim().is_empty()).collect();
            let mut json_lines = String::from("[");
            for (i, l) in lines.iter().enumerate() {
                if i > 0 {
                    json_lines.push(',');
                }
                json_lines.push_str(json_string(l).as_str());
            }
            json_lines.push(']');
            format!(
                "{{\"id\":{},\"ok\":true,\"event\":\"memory.list\",\"output\":{{\"path\":{},\"items\":{}}}}}",
                json_string(id),
                json_string(&path),
                json_lines
            )
        }
        Err(_) => format!(
            "{{\"id\":{},\"ok\":true,\"event\":\"memory.list\",\"output\":{{\"path\":{},\"items\":[]}}}}",
            json_string(id),
            json_string(&path)
        ),
    }
}

fn extension_list(id: &str, line: &str) -> String {
    let dir = match json_field(line, "dir") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing dir"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => {
            return format!(
                "{{\"id\":{},\"ok\":true,\"event\":\"extension.list\",\"output\":{{\"dir\":{},\"extensions\":[]}}}}",
                json_string(id),
                json_string(&dir)
            );
        }
    };

    let mut extensions = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir()
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
        {
            extensions.push(name.to_string());
        }
    }

    let mut json_list = String::from("[");
    for (i, ext) in extensions.iter().enumerate() {
        if i > 0 {
            json_list.push(',');
        }
        json_list.push_str(json_string(ext).as_str());
    }
    json_list.push(']');

    format!(
        "{{\"id\":{},\"ok\":true,\"event\":\"extension.list\",\"output\":{{\"dir\":{},\"extensions\":{}}}}}",
        json_string(id),
        json_string(&dir),
        json_list
    )
}

fn extension_read(id: &str, line: &str) -> String {
    let path = match json_field(line, "path") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing path"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    match std::fs::read_to_string(&path) {
        Ok(contents) => format!(
            "{{\"id\":{},\"ok\":true,\"event\":\"extension.read\",\"output\":{{\"path\":{},\"manifest\":{}}}}}",
            json_string(id),
            json_string(&path),
            json_string(&contents)
        ),
        Err(error) => error_response(id, &format!("failed to read extension manifest: {error}")),
    }
}

fn extension_write(id: &str, line: &str) -> String {
    let path = match json_field(line, "path") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing path"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let contents = match json_field(line, "contents") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing contents"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    if let Some(parent) = std::path::Path::new(&path).parent()
        && !parent.as_os_str().is_empty()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        return error_response(id, &format!("failed to create extension directory: {error}"));
    }

    match std::fs::write(&path, contents) {
        Ok(()) => format!(
            "{{\"id\":{},\"ok\":true,\"event\":\"extension.written\",\"output\":{{\"path\":{}}}}}",
            json_string(id),
            json_string(&path)
        ),
        Err(error) => error_response(id, &format!("failed to write extension manifest: {error}")),
    }
}

fn telegram_poll(id: &str, line: &str) -> String {
    let token = match json_field(line, "token") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing token"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let offset = match json_field(line, "offset") {
        Ok(Some(value)) => value,
        Ok(None) => "0".to_string(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    let url = format!("https://api.telegram.org/bot{}/getUpdates?offset={}", token, offset);

    let output = match std::process::Command::new("curl")
        .arg("-s").arg("-S")
        .arg(&url)
        .output()
    {
        Ok(output) => output,
        Err(error) => return error_response(id, &format!("failed to call Telegram API: {error}")),
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return error_response(id, &format!("telegram poll failed: {stderr}"));
    }

    format!(
        "{{\"id\":{},\"ok\":true,\"event\":\"telegram.poll\",\"output\":{{\"response\":{}}}}}",
        json_string(id),
        json_string(&stdout)
    )
}

fn telegram_send(id: &str, line: &str) -> String {
    let chat_id = match json_field(line, "chat_id") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing chat_id"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let text = match json_field(line, "text") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing text"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let token = match json_field(line, "token") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing token"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
    let body = format!("{{\"chat_id\":\"{}\",\"text\":\"{}\"}}", chat_id, text.replace('"', "\\\""));

    let output = match std::process::Command::new("curl")
        .arg("-s").arg("-S")
        .arg("-X").arg("POST")
        .arg("-H").arg("Content-Type: application/json")
        .arg("-d").arg(&body)
        .arg(&url)
        .output()
    {
        Ok(output) => output,
        Err(error) => return error_response(id, &format!("failed to call Telegram API: {error}")),
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return error_response(id, &format!("telegram send failed: {stderr}"));
    }

    format!(
        "{{\"id\":{},\"ok\":true,\"event\":\"telegram.sent\",\"output\":{{\"chat_id\":{},\"response\":{}}}}}",
        json_string(id),
        json_string(&chat_id),
        json_string(&stdout)
    )
}

fn telegram_loop(id: &str, line: &str) -> String {
    let token = match json_field(line, "token") {
        Ok(Some(t)) => t,
        Ok(None) => return error_response(id, "missing token"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let allowed_users = match json_field(line, "allowed_users") {
        Ok(Some(u)) => u,
        Ok(None) => String::new(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let api_key = match json_field(line, "api_key") {
        Ok(Some(k)) => k,
        Ok(None) => String::new(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let model = match json_field(line, "model") {
        Ok(Some(m)) => m,
        Ok(None) => "anthropic/claude-sonnet-4".to_string(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let provider = match json_field(line, "provider") {
        Ok(Some(p)) => p,
        Ok(None) => "openrouter".to_string(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let session_dir = match json_field(line, "session_dir") {
        Ok(Some(d)) => d,
        Ok(None) => ".zero-agent/sessions".to_string(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    let mut offset: i64 = 0;
    let mut updates_processed = 0;

    loop {
        // Poll for updates
        let poll_url = format!(
            "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=30",
            token, offset
        );

        let output = match std::process::Command::new("curl")
            .arg("-s").arg("-S")
            .arg("--max-time").arg("35")
            .arg(&poll_url)
            .output()
        {
            Ok(output) => output,
            Err(error) => {
                eprintln!("telegram poll error: {error}");
                std::thread::sleep(std::time::Duration::from_secs(5));
                continue;
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if !output.status.success() {
            eprintln!("telegram poll failed: {}", String::from_utf8_lossy(&output.stderr));
            std::thread::sleep(std::time::Duration::from_secs(5));
            continue;
        }

        // Parse updates - look for "update_id" and "message" fields
        let mut pos = 0;
        while let Some(update_start) = stdout[pos..].find("\"update_id\":") {
            let abs_pos = pos + update_start;
            // Extract update_id
            let after_key = &stdout[abs_pos + 12..];
            let update_id_end = after_key.find(|c: char| !c.is_ascii_digit()).unwrap_or(after_key.len());
            let update_id: i64 = after_key[..update_id_end].trim().parse().unwrap_or(0);

            if update_id >= offset {
                offset = update_id + 1;
            }

            // Extract message text
            let msg_start = stdout[abs_pos..].find("\"text\":");
            let chat_id_start = stdout[abs_pos..].find("\"chat\":{").and_then(|ci| {
                stdout[abs_pos + ci..].find("\"id\":")
            });
            let user_id_start = stdout[abs_pos..].find("\"from\":{").and_then(|ui| {
                stdout[abs_pos + ui..].find("\"id\":")
            });

            if let (Some(msg_off), Some(chat_off)) = (msg_start, chat_id_start) {
                let text_abs = abs_pos + msg_off;
                let chat_abs = abs_pos + chat_off;

                // Extract text value
                let text_after = &stdout[text_abs + 7..];
                let text_val = if let Some(stripped) = text_after.strip_prefix('"') {
                    let val_end = stripped.find('"').unwrap_or(stripped.len());
                    stripped[..val_end].to_string()
                } else {
                    String::new()
                };

                // Extract chat_id
                let chat_after = &stdout[chat_abs..];
                let chat_id_val = if let Some(cid_start) = chat_after.find("\"id\":") {
                    let cid_after = &chat_after[cid_start + 5..];
                    let cid_end = cid_after.find(|c: char| !c.is_ascii_digit() && c != '-').unwrap_or(cid_after.len());
                    cid_after[..cid_end].trim().to_string()
                } else {
                    String::new()
                };

                // Extract user_id for auth check
                let user_id_val = if let Some(uid_off) = user_id_start {
                    let uid_abs = abs_pos + uid_off;
                    let uid_after = &stdout[uid_abs..];
                    if let Some(uid_start) = uid_after.find("\"id\":") {
                        let uid_val_after = &uid_after[uid_start + 5..];
                        let uid_end = uid_val_after.find(|c: char| !c.is_ascii_digit()).unwrap_or(uid_val_after.len());
                        uid_val_after[..uid_end].trim().to_string()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                // Auth check
                if !allowed_users.is_empty() && !user_id_val.is_empty()
                    && !allowed_users.contains(&user_id_val) {
                    eprintln!("telegram: unauthorized user {user_id_val}");
                    pos = abs_pos + 1;
                    continue;
                }

                // Handle commands
                if text_val.starts_with('/') {
                    let cmd_response = match text_val.split_whitespace().next().unwrap_or("") {
                        "/start" => "Welcome to Zero-Agent! Send me a message to get started.",
                        "/help" => "Available commands:\n/start - Start session\n/help - Show help\n/status - Show status",
                        "/status" => "Zero-Agent is running.",
                        _ => "Unknown command. Type /help for available commands.",
                    };
                    let _ = send_telegram_message(&token, &chat_id_val, cmd_response);
                } else if !text_val.is_empty() && !chat_id_val.is_empty() {
                    // Send typing indicator
                    let _ = send_telegram_action(&token, &chat_id_val, "typing");

                    // Call agent.run
                    let session_path = format!("{}/{}.jsonl", session_dir, chat_id_val);
                    let agent_line = format!(
                        "{{\"id\":\"{}\",\"op\":\"agent.run\",\"provider\":{},\"model\":{},\"api_key\":{},\"prompt\":{},\"session_path\":{}}}",
                        id,
                        json_string(&provider),
                        json_string(&model),
                        json_string(&api_key),
                        json_string(&text_val),
                        json_string(&session_path)
                    );
                    let agent_response = agent_run(id, &agent_line);

                    // Extract text from agent response
                    let response_text = extract_agent_response_text(&agent_response);

                    if !response_text.is_empty() {
                        // Chunk and send response
                        let chunks = chunk_message(&response_text, 4096);
                        for chunk in chunks {
                            let _ = send_telegram_message(&token, &chat_id_val, &chunk);
                        }
                    }
                }

                updates_processed += 1;
            }

            pos = abs_pos + 1;
        }

        // Safety: exit after processing some updates in test mode
        if updates_processed > 0 && std::env::var("TELEGRAM_TEST_MODE").is_ok() {
            break;
        }
    }

    format!(
        "{{\"id\":{},\"ok\":true,\"event\":\"telegram.loop\",\"output\":{{\"updates_processed\":{}}}}}",
        json_string(id),
        updates_processed
    )
}

fn send_telegram_message(token: &str, chat_id: &str, text: &str) -> Result<(), String> {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
    let body = format!("{{\"chat_id\":\"{}\",\"text\":\"{}\"}}", chat_id, text.replace('"', "\\\"").replace('\n', "\\n"));

    let output = std::process::Command::new("curl")
        .arg("-s").arg("-S")
        .arg("-X").arg("POST")
        .arg("-H").arg("Content-Type: application/json")
        .arg("-d").arg(&body)
        .arg(&url)
        .output()
        .map_err(|e| format!("curl error: {e}"))?;

    if !output.status.success() {
        return Err(format!("send failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(())
}

fn send_telegram_action(token: &str, chat_id: &str, action: &str) -> Result<(), String> {
    let url = format!("https://api.telegram.org/bot{}/sendChatAction", token);
    let body = format!("{{\"chat_id\":\"{}\",\"action\":\"{}\"}}", chat_id, action);

    let output = std::process::Command::new("curl")
        .arg("-s").arg("-S")
        .arg("-X").arg("POST")
        .arg("-H").arg("Content-Type: application/json")
        .arg("-d").arg(&body)
        .arg(&url)
        .output()
        .map_err(|e| format!("curl error: {e}"))?;

    if !output.status.success() {
        return Err(format!("action failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(())
}

fn extract_agent_response_text(response: &str) -> String {
    // Extract text from events array in agent.run response
    let mut text = String::new();
    let mut pos = 0;
    while let Some(kind_start) = response[pos..].find("\"kind\":\"text\"") {
        let abs = pos + kind_start;
        if let Some(text_start) = response[abs..].find("\"text\":") {
            let text_abs = abs + text_start + 7;
            if response[text_abs..].starts_with('"') {
                let val_end = response[text_abs + 1..].find('"').unwrap_or(0);
                text.push_str(&response[text_abs + 1..text_abs + 1 + val_end]);
            }
        }
        pos = abs + 1;
    }
    text
}

fn chunk_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;

    while remaining.len() > max_len {
        // Try to split at paragraph boundary
        let split_at = if let Some(para_pos) = remaining[..max_len].rfind("\n\n") {
            para_pos + 2
        } else if let Some(newline_pos) = remaining[..max_len].rfind('\n') {
            newline_pos + 1
        } else if let Some(space_pos) = remaining[..max_len].rfind(' ') {
            space_pos + 1
        } else {
            max_len
        };

        chunks.push(remaining[..split_at].to_string());
        remaining = &remaining[split_at..];
    }

    if !remaining.is_empty() {
        chunks.push(remaining.to_string());
    }

    chunks
}

fn http_request(id: &str, line: &str) -> String {
    let url = match json_field(line, "url") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing url"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let method = match json_field(line, "method") {
        Ok(Some(value)) => value,
        Ok(None) => "GET".to_string(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let headers = match json_field(line, "headers") {
        Ok(Some(value)) => value,
        Ok(None) => String::new(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let body = match json_field(line, "body") {
        Ok(Some(value)) => value,
        Ok(None) => String::new(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    let mut cmd = std::process::Command::new("curl");
    cmd.arg("-s").arg("-S").arg("-w").arg("\n%{http_code}");
    cmd.arg("-X").arg(&method);

    // Parse headers from JSON string like "{\"key\":\"value\"}"
    if !headers.is_empty() {
        let mut h = headers.as_str();
        while let Some(key_start) = h.find('"') {
            h = &h[key_start + 1..];
            let Some(key_end) = h.find('"') else { break };
            let key = &h[..key_end];
            h = &h[key_end + 1..];
            let Some(colon) = h.find(':') else { break };
            h = &h[colon + 1..];
            let Some(val_start) = h.find('"') else { break };
            h = &h[val_start + 1..];
            let Some(val_end) = h.find('"') else { break };
            let val = &h[..val_end];
            h = &h[val_end + 1..];
            cmd.arg("-H").arg(format!("{key}: {val}"));
        }
    }

    if !body.is_empty() {
        cmd.arg("-d").arg(&body);
    }

    cmd.arg(&url);

    let output = match cmd.output() {
        Ok(output) => output,
        Err(error) => return error_response(id, &format!("failed to run curl: {error}")),
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return error_response(id, &format!("http request failed: {stderr}"));
    }

    // Split status code from body (last line)
    let (body_str, status_code) = if let Some(last_nl) = stdout.rfind('\n') {
        let code_str = stdout[last_nl + 1..].trim();
        let code = code_str.parse::<i64>().unwrap_or(0);
        (stdout[..last_nl].to_string(), code)
    } else {
        (stdout, 0)
    };

    format!(
        "{{\"id\":{},\"ok\":true,\"event\":\"http.response\",\"output\":{{\"status\":{},\"body\":{}}}}}",
        json_string(id),
        status_code,
        json_string(&body_str)
    )
}

fn http_stream_sse(id: &str, line: &str) -> String {
    let url = match json_field(line, "url") {
        Ok(Some(value)) => value,
        Ok(None) => return error_response(id, "missing url"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let method = match json_field(line, "method") {
        Ok(Some(value)) => value,
        Ok(None) => "POST".to_string(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let headers = match json_field(line, "headers") {
        Ok(Some(value)) => value,
        Ok(None) => String::new(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let body = match json_field(line, "body") {
        Ok(Some(value)) => value,
        Ok(None) => String::new(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    let mut cmd = std::process::Command::new("curl");
    cmd.arg("-s").arg("-S").arg("-N").arg("--no-buffer");
    cmd.arg("-X").arg(&method);
    cmd.arg("-H").arg("Accept: text/event-stream");

    if !headers.is_empty() {
        let mut h = headers.as_str();
        while let Some(key_start) = h.find('"') {
            h = &h[key_start + 1..];
            let Some(key_end) = h.find('"') else { break };
            let key = &h[..key_end];
            h = &h[key_end + 1..];
            let Some(colon) = h.find(':') else { break };
            h = &h[colon + 1..];
            let Some(val_start) = h.find('"') else { break };
            h = &h[val_start + 1..];
            let Some(val_end) = h.find('"') else { break };
            let val = &h[..val_end];
            h = &h[val_end + 1..];
            cmd.arg("-H").arg(format!("{key}: {val}"));
        }
    }

    if !body.is_empty() {
        cmd.arg("-d").arg(&body);
    }

    cmd.arg(&url);

    let output = match cmd.output() {
        Ok(output) => output,
        Err(error) => return error_response(id, &format!("failed to run curl: {error}")),
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return error_response(id, &format!("sse stream failed: {stderr}"));
    }

    // Parse SSE events from the response, extracting text content
    let mut events = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(data) = line.strip_prefix("data: ")
            && data != "[DONE]"
        {
            let text = extract_sse_text(data);
            if !text.is_empty() {
                events.push(text);
            }
        }
    }

    let mut json_events = String::from("[");
    for (i, evt) in events.iter().enumerate() {
        if i > 0 {
            json_events.push(',');
        }
        json_events.push_str(json_string(evt).as_str());
    }
    json_events.push(']');

    format!(
        "{{\"id\":{},\"ok\":true,\"event\":\"http.sse\",\"output\":{{\"events\":{}}}}}",
        json_string(id),
        json_events
    )
}

fn models_discover(id: &str, line: &str) -> String {
    let provider = match json_field(line, "provider") {
        Ok(Some(provider)) => provider,
        Ok(None) => "unknown".to_string(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let api_key = match json_field(line, "api_key") {
        Ok(Some(key)) => key,
        Ok(None) => String::new(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    let url = match provider.as_str() {
        "openrouter" => "https://openrouter.ai/api/v1/models",
        "openai" => "https://api.openai.com/v1/models",
        _ => {
            return format!(
                "{{\"id\":{},\"ok\":true,\"event\":\"models\",\"output\":{{\"provider\":{},\"transport\":\"none\",\"models\":[]}}}}",
                json_string(id),
                json_string(&provider)
            );
        }
    };

    let mut cmd = std::process::Command::new("curl");
    cmd.arg("-s").arg("-S");
    if !api_key.is_empty() {
        cmd.arg("-H").arg(format!("Authorization: Bearer {}", api_key));
    }
    cmd.arg(url);

    let output = match cmd.output() {
        Ok(output) => output,
        Err(error) => return error_response(id, &format!("failed to fetch models: {error}")),
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return error_response(id, &format!("model discovery failed: {stderr}"));
    }

    format!(
        "{{\"id\":{},\"ok\":true,\"event\":\"models\",\"output\":{{\"provider\":{},\"transport\":\"http\",\"response\":{}}}}}",
        json_string(id),
        json_string(&provider),
        json_string(&stdout)
    )
}

fn setup_run(id: &str) -> String {
    let steps = r#"[
        {"step":"provider","prompt":"Select provider","options":["openrouter","anthropic","openai","ollama"],"default":"openrouter"},
        {"step":"api_key","prompt":"Enter API key","type":"password"},
        {"step":"model","prompt":"Select default model","depends":"provider"},
        {"step":"telegram_token","prompt":"Enter Telegram bot token (optional, from @BotFather)","optional":true},
        {"step":"telegram_user_id","prompt":"Enter your Telegram user ID (optional, from @userinfobot)","optional":true}
    ]"#;

    format!(
        "{{\"id\":{},\"ok\":true,\"event\":\"setup\",\"output\":{{\"steps\":{}}}}}",
        json_string(id),
        steps
    )
}

fn setup_save(id: &str, line: &str) -> String {
    let config_path = match json_field(line, "config_path") {
        Ok(Some(path)) => path,
        Ok(None) => return error_response(id, "missing config_path"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let provider = match json_field(line, "provider") {
        Ok(Some(p)) => p,
        Ok(None) => "openrouter".to_string(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let api_key = match json_field(line, "api_key") {
        Ok(Some(k)) => k,
        Ok(None) => String::new(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let model = match json_field(line, "model") {
        Ok(Some(m)) => m,
        Ok(None) => String::new(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let telegram_token = match json_field(line, "telegram_token") {
        Ok(Some(t)) => t,
        Ok(None) => String::new(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let telegram_user_id = match json_field(line, "telegram_user_id") {
        Ok(Some(u)) => u,
        Ok(None) => String::new(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    // Build config JSON
    let config_json = format!(
        "{{\"data_dir\":\".zero-agent\",\"default_provider\":{},\"default_model\":{},\"api_key_env\":{},\"telegram\":{{\"bot_token\":{},\"allowed_users\":{}}}}}",
        json_string(&provider),
        json_string(&model),
        json_string(&api_key),
        json_string(&telegram_token),
        json_string(&telegram_user_id)
    );

    // Write config file
    if let Some(parent) = std::path::Path::new(&config_path).parent()
        && !parent.as_os_str().is_empty()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        return error_response(id, &format!("failed to create config directory: {error}"));
    }

    match std::fs::write(&config_path, &config_json) {
        Ok(()) => format!(
            "{{\"id\":{},\"ok\":true,\"event\":\"setup.saved\",\"output\":{{\"path\":{},\"config\":{}}}}}",
            json_string(id),
            json_string(&config_path),
            config_json
        ),
        Err(error) => error_response(id, &format!("failed to save config: {error}")),
    }
}

fn agent_run(id: &str, line: &str) -> String {
    let config_path = match json_field(line, "config_path") {
        Ok(Some(p)) => p,
        Ok(None) => String::new(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    // Read config if provided
    let config = if !config_path.is_empty() {
        std::fs::read_to_string(&config_path).unwrap_or_default()
    } else {
        String::new()
    };

    let provider = match json_field(line, "provider") {
        Ok(Some(p)) => p,
        Ok(None) => {
            if !config.is_empty() {
                extract_json_nested_field(&config, &["default_provider"]).unwrap_or_else(|| "openrouter".to_string())
            } else {
                "openrouter".to_string()
            }
        }
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let model = match json_field(line, "model") {
        Ok(Some(m)) => m,
        Ok(None) => {
            if !config.is_empty() {
                extract_json_nested_field(&config, &["default_model"]).unwrap_or_default()
            } else {
                return error_response(id, "missing model");
            }
        }
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let api_key = match json_field(line, "api_key") {
        Ok(Some(k)) => k,
        Ok(None) => {
            if !config.is_empty() {
                extract_json_nested_field(&config, &["api_key_env"]).unwrap_or_default()
            } else {
                String::new()
            }
        }
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let prompt = match json_field(line, "prompt") {
        Ok(Some(p)) => p,
        Ok(None) => return error_response(id, "missing prompt"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let session_path = match json_field(line, "session_path") {
        Ok(Some(p)) => p,
        Ok(None) => String::new(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let _tools_json = match json_field(line, "tools") {
        Ok(Some(t)) => t,
        Ok(None) => "[]".to_string(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    // Build messages array from session history + new prompt
    let mut messages = String::from("[");
    if !session_path.is_empty()
        && let Ok(contents) = std::fs::read_to_string(&session_path) {
        let mut first = true;
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            if !first { messages.push(','); }
            messages.push_str(line);
            first = false;
        }
    }
    if !messages.is_empty() && messages != "[" {
        messages.push(',');
    }
    messages.push_str(&format!("{{\"role\":\"user\",\"content\":{}}}", json_string(&prompt)));
    messages.push(']');

    // Build request body based on provider
    let (url, body, headers) = match provider.as_str() {
        "anthropic" => {
            let url = "https://api.anthropic.com/v1/messages".to_string();
            let body = format!(
                "{{\"model\":{},\"max_tokens\":4096,\"messages\":{},\"stream\":true}}",
                json_string(&model),
                messages
            );
            let headers = format!("{{\"x-api-key\":{},\"anthropic-version\":\"2023-06-01\",\"Content-Type\":\"application/json\"}}", json_string(&api_key));
            (url, body, headers)
        }
        _ => {
            // OpenAI-compatible (openrouter, openai, ollama)
            let url = match provider.as_str() {
                "openai" => "https://api.openai.com/v1/chat/completions".to_string(),
                "ollama" => "http://localhost:11434/v1/chat/completions".to_string(),
                _ => "https://openrouter.ai/api/v1/chat/completions".to_string(),
            };
            let body = format!(
                "{{\"model\":{},\"messages\":{},\"stream\":true}}",
                json_string(&model),
                messages
            );
            let headers = format!("{{\"Authorization\":\"Bearer {}\",\"Content-Type\":\"application/json\"}}", api_key);
            (url, body, headers)
        }
    };

    // Call SSE stream
    let mut cmd = std::process::Command::new("curl");
    cmd.arg("-s").arg("-S").arg("-N").arg("--no-buffer");
    cmd.arg("-X").arg("POST");
    cmd.arg("-H").arg("Accept: text/event-stream");

    // Parse headers
    let mut h = headers.as_str();
    while let Some(key_start) = h.find('"') {
        h = &h[key_start + 1..];
        let Some(key_end) = h.find('"') else { break };
        let key = &h[..key_end];
        h = &h[key_end + 1..];
        let Some(colon) = h.find(':') else { break };
        h = &h[colon + 1..];
        let Some(val_start) = h.find('"') else { break };
        h = &h[val_start + 1..];
        let Some(val_end) = h.find('"') else { break };
        let val = &h[..val_end];
        h = &h[val_end + 1..];
        cmd.arg("-H").arg(format!("{key}: {val}"));
    }

    cmd.arg("-d").arg(&body);
    cmd.arg(&url);

    let output = match cmd.output() {
        Ok(output) => output,
        Err(error) => return error_response(id, &format!("failed to run curl: {error}")),
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return error_response(id, &format!("agent run failed: {stderr}"));
    }

    // Parse SSE events into normalized format
    let mut events = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(data) = line.strip_prefix("data: ") {
            if data == "[DONE]" { continue; }
            // Try to extract text content
            let text = extract_sse_text(data);
            if !text.is_empty() {
                events.push(format!("{{\"kind\":\"text\",\"text\":{}}}", json_string(&text)));
            }
            // Try to extract tool calls (OpenAI format)
            if let Some(tool_name) = extract_json_nested_field(data, &["choices", "0", "delta", "tool_calls", "0", "function", "name"]) {
                let tool_args = extract_json_nested_field(data, &["choices", "0", "delta", "tool_calls", "0", "function", "arguments"]).unwrap_or_default();
                events.push(format!("{{\"kind\":\"tool_call\",\"name\":{},\"input\":{}}}", json_string(&tool_name), json_string(&tool_args)));
            }
            // Try to extract tool calls (Anthropic format)
            if let Some(tool_name) = extract_json_nested_field(data, &["content_block", "name"]) {
                let tool_input = extract_json_nested_field(data, &["content_block", "input"]).unwrap_or_default();
                events.push(format!("{{\"kind\":\"tool_call\",\"name\":{},\"input\":{}}}", json_string(&tool_name), json_string(&tool_input)));
            }
        }
    }

    let mut json_events = String::from("[");
    for (i, evt) in events.iter().enumerate() {
        if i > 0 { json_events.push(','); }
        json_events.push_str(evt);
    }
    json_events.push(']');

    // Persist assistant response to session
    if !session_path.is_empty() && !events.is_empty() {
        let _ = std::fs::OpenOptions::new().create(true).append(true).open(&session_path)
            .and_then(|mut f| {
                use std::io::Write;
                writeln!(f, "{{\"role\":\"user\",\"content\":{}}}", json_string(&prompt))
            });
    }

    format!(
        "{{\"id\":{},\"ok\":true,\"event\":\"agent.run\",\"output\":{{\"events\":{},\"provider\":{},\"model\":{}}}}}",
        json_string(id),
        json_events,
        json_string(&provider),
        json_string(&model)
    )
}

fn agent_tool(id: &str, line: &str) -> String {
    let tool_name = match json_field(line, "name") {
        Ok(Some(n)) => n,
        Ok(None) => return error_response(id, "missing tool name"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let tool_input = match json_field(line, "input") {
        Ok(Some(i)) => i,
        Ok(None) => "{}".to_string(),
        Err(_) => return error_response(id, "invalid bridge request"),
    };

    // Dispatch to existing bridge operations
    match tool_name.as_str() {
        "read_file" => {
            let path = extract_json_nested_field(&tool_input, &["path"]).unwrap_or_default();
            fs_read(id, &format!("{{\"id\":{},\"op\":\"fs.read\",\"path\":{}}}", json_string(id), json_string(&path)))
        }
        "write_file" => {
            let path = extract_json_nested_field(&tool_input, &["path"]).unwrap_or_default();
            let contents = extract_json_nested_field(&tool_input, &["contents"]).unwrap_or_default();
            fs_write(id, &format!("{{\"id\":{},\"op\":\"fs.write\",\"path\":{},\"contents\":{}}}", json_string(id), json_string(&path), json_string(&contents)))
        }
        "edit_file" => {
            let path = extract_json_nested_field(&tool_input, &["path"]).unwrap_or_default();
            let old_string = extract_json_nested_field(&tool_input, &["old_string"]).unwrap_or_default();
            let new_string = extract_json_nested_field(&tool_input, &["new_string"]).unwrap_or_default();
            fs_edit(id, &format!("{{\"id\":{},\"op\":\"fs.edit\",\"path\":{},\"old_string\":{},\"new_string\":{}}}", json_string(id), json_string(&path), json_string(&old_string), json_string(&new_string)))
        }
        "shell" => {
            let command = extract_json_nested_field(&tool_input, &["command"]).unwrap_or_default();
            shell_run(id, &format!("{{\"id\":{},\"op\":\"shell.run\",\"command\":{}}}", json_string(id), json_string(&command)))
        }
        "glob" => {
            let pattern = extract_json_nested_field(&tool_input, &["pattern"]).unwrap_or_default();
            fs_glob(id, &format!("{{\"id\":{},\"op\":\"fs.glob\",\"pattern\":{}}}", json_string(id), json_string(&pattern)))
        }
        _ => error_response(id, &format!("unknown tool: {}", tool_name))
    }
}

/// Extract text content from SSE data (supports Anthropic and OpenAI formats)
fn extract_sse_text(data: &str) -> String {
    // Anthropic format: {"type":"content_block_delta","delta":{"type":"text_delta","text":"..."}}
    if let Some(text) = extract_json_nested_field(data, &["delta", "text"]) {
        return text;
    }
    // OpenAI format: {"choices":[{"delta":{"content":"..."}}]}
    if let Some(text) = extract_json_nested_field(data, &["choices", "0", "delta", "content"]) {
        return text;
    }
    String::new()
}

/// Extract a nested JSON field by walking a path of keys
fn extract_json_nested_field(data: &str, path: &[&str]) -> Option<String> {
    let mut current = data;
    for (i, key) in path.iter().enumerate() {
        // Find the key
        let pattern = format!("\"{key}\"");
        let start = current.find(&pattern)?;
        current = &current[start + pattern.len()..];
        // Skip colon and whitespace
        let colon = current.find(':')?;
        current = current[colon + 1..].trim_start();
        if i == path.len() - 1 {
            // Last key - extract the value
            return if let Some(val) = current.strip_prefix('"') {
                let end = find_unescaped_quote(val)?;
                Some(val[..end].to_string())
            } else if current.starts_with('[') || current.starts_with('{') {
                // For objects/arrays, return the full JSON value
                let end = find_json_end(current)?;
                Some(current[..end].to_string())
            } else {
                // Number, bool, null
                let end = current.find(|c: char| c == ',' || c == '}' || c == ']' || c.is_whitespace()).unwrap_or(current.len());
                Some(current[..end].to_string())
            };
        }
        // Not last key - if it's an object, continue searching inside
        if current.starts_with('{') {
            current = &current[1..];
        } else if current.starts_with('[') {
            // For arrays, find the next element
            current = &current[1..];
        }
    }
    None
}

fn find_unescaped_quote(s: &str) -> Option<usize> {
    let mut chars = s.char_indices();
    while let Some((i, ch)) = chars.next() {
        match ch {
            '"' => return Some(i),
            '\\' => { chars.next(); } // skip escaped char
            _ => {}
        }
    }
    None
}

fn find_json_end(s: &str) -> Option<usize> {
    let open = s.as_bytes()[0];
    let close = if open == b'{' { b'}' } else { b']' };
    let mut depth = 0;
    let mut in_string = false;
    let mut prev = 0u8;
    for (i, &b) in s.as_bytes().iter().enumerate() {
        if in_string {
            if b == b'"' && prev != b'\\' {
                in_string = false;
            }
        } else {
            match b {
                b'"' => in_string = true,
                b if b == open => depth += 1,
                b if b == close => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i + 1);
                    }
                }
                _ => {}
            }
        }
        prev = b;
    }
    None
}

fn error_response(id: &str, message: &str) -> String {
    format!(
        "{{\"id\":{},\"ok\":false,\"event\":\"error\",\"output\":{{}},\"error\":{}}}",
        json_string(id),
        json_string(message)
    )
}

fn looks_like_json_object(input: &str) -> bool {
    let trimmed = input.trim();
    trimmed.starts_with('{') && trimmed.ends_with('}')
}

fn json_field(input: &str, field: &str) -> Result<Option<String>, ()> {
    let pattern = format!("\"{field}\"");
    let Some(start) = input.find(&pattern) else {
        return Ok(None);
    };
    let after_name = &input[start + pattern.len()..];
    let Some(colon) = after_name.find(':') else {
        return Err(());
    };
    let after_colon = after_name[colon + 1..].trim_start();
    let Some(value) = after_colon.strip_prefix('"') else {
        return Err(());
    };

    parse_json_string(value).map(Some)
}

fn parse_json_string(input_after_open_quote: &str) -> Result<String, ()> {
    let mut out = String::new();
    let mut chars = input_after_open_quote.chars();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Ok(out),
            '\\' => match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('/') => out.push('/'),
                Some('b') => out.push('\u{0008}'),
                Some('f') => out.push('\u{000c}'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('u') => {
                    let code = read_hex4(&mut chars)?;
                    if (0xD800..=0xDBFF).contains(&code) {
                        if chars.next() != Some('\\') || chars.next() != Some('u') {
                            return Err(());
                        }
                        let low = read_hex4(&mut chars)?;
                        if !(0xDC00..=0xDFFF).contains(&low) {
                            return Err(());
                        }
                        let scalar = 0x10000 + (((code - 0xD800) << 10) | (low - 0xDC00));
                        out.push(char::from_u32(scalar).ok_or(())?);
                    } else if (0xDC00..=0xDFFF).contains(&code) {
                        return Err(());
                    } else {
                        out.push(char::from_u32(code).ok_or(())?);
                    }
                }
                _ => return Err(()),
            },
            c if c.is_control() => return Err(()),
            c => out.push(c),
        }
    }

    Err(())
}

fn read_hex4(chars: &mut std::str::Chars<'_>) -> Result<u32, ()> {
    let mut value = 0;
    for _ in 0..4 {
        let Some(ch) = chars.next() else {
            return Err(());
        };
        let Some(digit) = ch.to_digit(16) else {
            return Err(());
        };
        value = (value << 4) | digit;
    }
    Ok(value)
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_valid_unicode_escape() {
        assert_eq!(json_field(r#"{"id":"id\u0041","op":"ping"}"#, "id"), Ok(Some("idA".to_string())));
    }

    #[test]
    fn rejects_invalid_escape() {
        assert_eq!(json_field(r#"{"id":"id\q","op":"ping"}"#, "id"), Err(()));
    }

    #[test]
    fn rejects_non_json_input() {
        let response = handle_line("not json");
        assert!(response.contains(r#""ok":false"#));
        assert!(response.contains("invalid bridge request"));
    }


    #[test]
    fn returns_model_discovery_stub() {
        let response = handle_line(r#"{"id":"2","op":"models.discover","provider":"openrouter"}"#);
        assert!(response.contains(r#""event":"models""#));
        assert!(response.contains(r#""provider":"openrouter""#));
    }

    #[test]
    fn validates_default_config_fields() {
        let response = handle_line(r#"{"id":"3","op":"config.validate","data_dir":".zero-agent","default_provider":"openrouter"}"#);
        assert!(response.contains(r#""event":"config.validation""#));
        assert!(response.contains(r#""valid":true"#));
    }

    #[test]
    fn rejects_invalid_config_fields() {
        let response = handle_line(r#"{"id":"4","op":"config.validate","data_dir":"","default_provider":"openrouter"}"#);
        assert!(response.contains(r#""event":"config.validation""#));
        assert!(response.contains(r#""valid":false"#));
    }
}
