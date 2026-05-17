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
        "models.discover" => {
            let provider = match json_field(line, "provider") {
                Ok(Some(provider)) => provider,
                Ok(None) => "unknown".to_string(),
                Err(_) => return error_response(&id, "invalid bridge request"),
            };
            format!(
                "{{\"id\":{},\"ok\":true,\"event\":\"models\",\"output\":{{\"provider\":{},\"transport\":\"stub\",\"models\":[]}}}}",
                json_string(&id),
                json_string(&provider)
            )
        }
        "http.request" | "http.stream_sse" | "process.spawn" | "telegram.poll" | "telegram.send" => {
            format!(
                "{{\"id\":{},\"ok\":false,\"event\":\"not_implemented\",\"output\":{{\"op\":{}}},\"error\":\"bridge operation is not implemented yet\"}}",
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
