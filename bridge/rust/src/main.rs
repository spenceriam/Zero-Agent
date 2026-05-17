use std::char;
use std::io::{self, BufRead, Write};

fn main() -> io::Result<()> {
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
        "telegram.poll" => telegram_poll(&id, line),
        "telegram.send" => telegram_send(&id, line),
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
    "{\"data_dir\":\".zero-agent\",\"default_provider\":\"openrouter\",\"default_model\":\"\",\"tool_policy\":{\"allow_safe_without_prompt\":true,\"ask_before_mutating\":true,\"ask_before_destructive\":true}}"
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
