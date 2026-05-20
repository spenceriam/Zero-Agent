//! Maps Zero Lang TuiEvent shapes (via bridge JSON) to ANSI render calls.
//! Zero runtime will emit these events; Rust is the display driver until then.

use super::{
    print_agent_text, print_memory_footer, print_slash_palette, print_system_note,
    print_thinking_block, print_tool_call, print_tool_result, print_user_block,
    ApprovalModal, App, ToolStatus,
};
use crate::tools::ToolRegistry;

#[derive(Debug)]
pub struct RenderContext {
    pub app: App,
    pub width: usize,
}

/// Handle a single bridge `tui.render` event payload.
pub fn render_event(ctx: &mut RenderContext, payload: &str) -> Result<(), String> {
    let kind = extract_json_field(payload, "kind").unwrap_or_default();

    match kind.as_str() {
        "AppendScroll" => render_scroll_entry(ctx, payload),
        "SetStatus" => Ok(()), // status bar refreshed by main loop
        "MemoryFooter" => {
            let desc = extract_json_field(payload, "description").unwrap_or_default();
            let file = extract_json_field(payload, "file_name").unwrap_or_default();
            print_memory_footer(&desc, &file);
            Ok(())
        }
        "Interrupted" => {
            print_system_note("Interrupted");
            Ok(())
        }
        "ShowSlashPalette" => {
            let filter = extract_json_field(payload, "text").unwrap_or_default();
            print_slash_palette(&filter);
            Ok(())
        }
        _ => Ok(()),
    }
}

fn render_scroll_entry(ctx: &mut RenderContext, payload: &str) -> Result<(), String> {
    let entry_kind = extract_json_field(payload, "entry_kind").unwrap_or_default();
    let content = extract_json_field(payload, "content").unwrap_or_default();
    let tool_name = extract_json_field(payload, "tool_name").unwrap_or_default();
    let log_path = extract_json_field(payload, "log_path").unwrap_or_default();

    match entry_kind.as_str() {
        "UserMessage" => {
            print_user_block(&content, ctx.width);
        }
        "AgentText" => {
            print_agent_text(&content);
        }
        "ThinkingBlock" => {
            print_thinking_block(&content, &log_path);
        }
        "ToolCall" => {
            print_tool_call(&tool_name, &content, &ToolStatus::Running, None);
        }
        "ToolResult" => {
            print_tool_result(&tool_name, &content, &log_path);
        }
        "SystemNote" => {
            print_system_note(&content);
        }
        "MemoryFooter" => {
            let file = extract_json_field(payload, "file_name").unwrap_or_default();
            print_memory_footer(&content, &file);
        }
        _ => {}
    }
    Ok(())
}

/// Render startup chrome from bridge `tui.emit` with kind SessionStarted.
pub fn render_session_started(
    ctx: &mut RenderContext,
    config_path: &str,
    cwd: &str,
    no_auth: bool,
) {
    let tools: Vec<String> = ToolRegistry::default()
        .list()
        .into_iter()
        .map(|(name, _, _)| name.to_string())
        .collect();
    super::print_startup_banner(&ctx.app, config_path, &tools, cwd, no_auth);
    super::print_status_bar(&ctx.app);
}

pub fn approval_from_json(payload: &str) -> ApprovalModal {
    ApprovalModal {
        tool_name: extract_json_field(payload, "tool_name").unwrap_or_default(),
        command: extract_json_field(payload, "command").unwrap_or_default(),
        risk_level: extract_json_field(payload, "risk_level").unwrap_or_else(|| "Mutating".into()),
        selected: 0,
    }
}

fn extract_json_field(json: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{field}\"");
    let start = json.find(&pattern)?;
    let after = &json[start + pattern.len()..];
    let colon = after.find(':')?;
    let value = after[colon + 1..].trim_start();
    if let Some(v) = value.strip_prefix('"') {
        let end = v.find('"')?;
        Some(v[..end].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_system_note_event() {
        let mut ctx = RenderContext {
            app: App::new(),
            width: 80,
        };
        let payload = r#"{"kind":"AppendScroll","entry_kind":"SystemNote","content":"hello"}"#;
        render_event(&mut ctx, payload).expect("render should succeed");
    }
}
