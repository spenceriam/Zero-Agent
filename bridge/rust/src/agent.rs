use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::config::Config;
use crate::debug;
use crate::memory::{self, MemoryScope};
use crate::provider::{Message, Provider, StreamEventType};
use crate::tools::ToolRegistry;

#[cfg(feature = "tui")]
use crate::tui;
#[cfg(feature = "tui")]
use crate::tui::input::run_approval_modal;

const SYSTEM_PROMPT: &str = r#"You are ZERO, a personal AI assistant for developers.
You are running locally on the user's machine.
You have access to tools for reading/writing files, running shell commands, and searching files.
Be concise and direct. When you need to do something, use your tools.
Always use tools to interact with the filesystem or run commands - never just describe what to do."#;

const MAX_EMPTY_CONTINUATION_RETRIES: u32 = 2;
const CONTINUATION_NUDGE: &str =
    "Continue executing the remaining tool tests from the original request. Use tools for each step.";

/// True when the provider returned no text, reasoning, or tool calls.
pub(crate) fn is_empty_provider_response(
    assistant_text: &str,
    thinking_text: &str,
    tool_calls_len: usize,
) -> bool {
    tool_calls_len == 0 && assistant_text.is_empty() && thinking_text.is_empty()
}

/// Post-tool round that came back empty — candidate for retry/nudge.
pub(crate) fn should_retry_post_tool_continuation(
    tools_ran_this_turn: bool,
    assistant_text: &str,
    thinking_text: &str,
    tool_calls_len: usize,
) -> bool {
    tools_ran_this_turn
        && is_empty_provider_response(assistant_text, thinking_text, tool_calls_len)
}

pub struct Agent {
    config: Config,
    provider: Provider,
    messages: Vec<Message>,
    session_path: Option<String>,
    session_id: String,
    message_count: usize,
    start_time: std::time::Instant,
    session_approved_tools: Vec<String>,
    global_approved_tools: Vec<String>,
    interrupt: Arc<AtomicBool>,
    turn_memory_footer: Option<(String, String)>,
}

impl Agent {
    pub fn new(config: Config, provider_id: Option<&str>) -> Self {
        let provider_config = provider_id
            .and_then(|id| config.get_provider(id))
            .unwrap_or_else(|| config.default_provider())
            .clone();

        let provider = Provider::new(provider_config);
        let global_approved_tools = config.tool_policy.globally_approved_tools.clone();

        let session_dir = config.sessions_dir();
        let _ = std::fs::create_dir_all(&session_dir);

        let session_id = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let session_path = session_dir.join(format!("{session_id}.jsonl"));

        let messages = vec![Message {
            role: "system".to_string(),
            content: Some(SYSTEM_PROMPT.to_string()),
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
        }];

        Self {
            config,
            provider,
            messages,
            session_path: Some(session_path.to_string_lossy().into_owned()),
            session_id,
            message_count: 0,
            start_time: std::time::Instant::now(),
            session_approved_tools: Vec::new(),
            global_approved_tools,
            interrupt: Arc::new(AtomicBool::new(false)),
            turn_memory_footer: None,
        }
    }

    pub fn provider_info(&self) -> String {
        format!("{} / {}", self.provider.id(), self.provider.model())
    }

    pub fn model(&self) -> &str {
        self.provider.model()
    }

    pub fn provider_id(&self) -> &str {
        self.provider.id()
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn elapsed_secs(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    pub fn set_model(&mut self, model: &str) -> Result<(), String> {
        self.provider.set_model(model.to_string());
        self.config.set_default_model(model)
    }

    pub fn set_provider(&mut self, provider_id: &str) -> Result<(), String> {
        let provider_config = self
            .config
            .get_provider(provider_id)
            .ok_or_else(|| format!("unknown provider: {provider_id}"))?
            .clone();
        self.provider = Provider::new(provider_config);
        self.config.set_default_provider(provider_id)
    }

    pub async fn discover_models(&self) -> Vec<String> {
        self.provider.discover_models().await
    }

    pub fn interrupt_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.interrupt)
    }

    pub fn request_interrupt(&self) {
        self.interrupt.store(true, Ordering::Relaxed);
    }

    #[cfg(feature = "tui")]
    pub fn apply_profile(&mut self, profile: &tui::UserProfile) {
        let section = crate::profile::profile_system_prompt_section(profile);
        if section.is_empty() {
            return;
        }
        let prompt = format!("{SYSTEM_PROMPT}\n\n{section}");
        if let Some(msg) = self.messages.first_mut() {
            if msg.role == "system" {
                msg.content = Some(prompt);
                return;
            }
        }
        self.messages.insert(
            0,
            Message {
                role: "system".to_string(),
                content: Some(prompt),
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
            },
        );
    }

    pub async fn chat(&mut self, user_input: &str) -> Result<(), String> {
        self.interrupt.store(false, Ordering::Relaxed);
        self.turn_memory_footer = None;

        let term_width = tui_width();
        let log_dir = session_log_dir(&self.config, &self.session_id);
        let _ = std::fs::create_dir_all(&log_dir);
        debug::set_log_path(log_dir.join("debug.log"));
        let thinking_log = log_dir.join("thinking.log");
        let tool_log = log_dir.join("tool-output.log");
        let chat_log = log_dir.join("chat.log");

        debug::log("agent", &format!("chat() start len={}", user_input.len()));

        append_to_log(
            chat_log.to_str().unwrap_or("chat.log"),
            &format!("USER: {user_input}\n"),
        );

        #[cfg(feature = "tui")]
        tui::print_user_block(user_input, term_width);

        self.messages.push(Message {
            role: "user".to_string(),
            content: Some(user_input.to_string()),
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
        });
        self.message_count += 1;

        let tool_registry = ToolRegistry::default();
        let tools = tool_registry.definitions();

        let mut loop_iter = 0u32;
        let mut tools_ran_this_turn = false;
        let mut empty_continuation_retries = 0u32;
        let mut continuation_nudge_injected = false;

        loop {
            loop_iter += 1;
            debug::log("agent", &format!("inner loop iteration={loop_iter}"));

            if self.interrupt.load(Ordering::Relaxed) {
                debug::log("agent", "interrupted before provider call");
                #[cfg(feature = "tui")]
                tui::print_status_stopped();
                break;
            }

            #[cfg(feature = "tui")]
            tui::layout::set_status_mode(tui::StatusMode::Flowing);

            #[cfg(feature = "tui")]
            let mut stream_writer = tui::AgentStreamWriter::new();

            debug::log("provider", "stream_chat start");
            let events = {
                #[cfg(feature = "tui")]
                {
                    self.provider
                        .stream_chat_with(
                            &self.messages,
                            &tools,
                            Some(&self.interrupt),
                            Some(|event| stream_writer.on_event(&event)),
                        )
                        .await
                }
                #[cfg(not(feature = "tui"))]
                {
                    self.provider
                        .stream_chat(&self.messages, &tools, Some(&self.interrupt))
                        .await
                }
            };

            #[cfg(feature = "tui")]
            {
                stream_writer.finish();
            }

            if self.interrupt.load(Ordering::Relaxed) {
                debug::log("agent", "interrupted after provider stream");
                #[cfg(feature = "tui")]
                tui::print_status_stopped();
                break;
            }

            let events = match events {
                Ok(ev) => {
                    let mut text_n = 0usize;
                    let mut reasoning_n = 0usize;
                    let mut tool_n = 0usize;
                    for e in &ev {
                        match &e.event_type {
                            StreamEventType::Text(_) => text_n += 1,
                            StreamEventType::Reasoning(_) => reasoning_n += 1,
                            StreamEventType::ToolCall { .. } => tool_n += 1,
                            StreamEventType::Done => {}
                        }
                    }
                    debug::log(
                        "provider",
                        &format!(
                            "stream_chat ok events={} text={text_n} reasoning={reasoning_n} tools={tool_n}",
                            ev.len()
                        ),
                    );
                    ev
                }
                Err(e) => {
                    debug::log("provider", &format!("stream_chat error: {e}"));
                    return Err(e);
                }
            };

            let mut assistant_text = String::new();
            let mut thinking_text = String::new();
            let mut tool_calls: Vec<(String, String, String)> = Vec::new();

            #[cfg(feature = "tui")]
            {
                assistant_text = stream_writer.text.clone();
                thinking_text = stream_writer.reasoning.clone();
            }

            #[cfg(not(feature = "tui"))]
            for event in &events {
                match &event.event_type {
                    StreamEventType::Text(text) => assistant_text.push_str(text),
                    StreamEventType::Reasoning(text) => thinking_text.push_str(text),
                    StreamEventType::ToolCall { id, name, arguments } => {
                        tool_calls.push((id.clone(), name.clone(), arguments.clone()));
                    }
                    StreamEventType::Done => {}
                }
            }

            #[cfg(feature = "tui")]
            let agent_started = stream_writer.agent_started;

            #[cfg(feature = "tui")]
            for event in &events {
                match &event.event_type {
                    StreamEventType::ToolCall { id, name, arguments } => {
                        tool_calls.push((id.clone(), name.clone(), arguments.clone()));
                    }
                    _ => {}
                }
            }

            if tool_calls.is_empty() {
                if should_retry_post_tool_continuation(
                    tools_ran_this_turn,
                    &assistant_text,
                    &thinking_text,
                    tool_calls.len(),
                ) {
                    if empty_continuation_retries < MAX_EMPTY_CONTINUATION_RETRIES {
                        empty_continuation_retries += 1;
                        debug::log(
                            "agent",
                            &format!("empty continuation retry {empty_continuation_retries}"),
                        );
                        #[cfg(feature = "tui")]
                        tui::print_system_note("Retrying…");
                        continue;
                    }
                    if !continuation_nudge_injected {
                        continuation_nudge_injected = true;
                        debug::log("agent", "continuation nudge injected");
                        self.messages.push(Message {
                            role: "user".to_string(),
                            content: Some(CONTINUATION_NUDGE.to_string()),
                            tool_calls: None,
                            tool_call_id: None,
                            reasoning_content: None,
                        });
                        #[cfg(feature = "tui")]
                        tui::print_system_note("Retrying…");
                        continue;
                    }

                    #[cfg(feature = "tui")]
                    {
                        if !thinking_text.is_empty() {
                            // reasoning-only after tools — unlikely here
                        } else {
                            tui::print_system_note("No response from model after tools.");
                        }
                    }

                    debug::log(
                        "agent",
                        &format!(
                            "inner loop exit: empty_tools assistant_len={} tools_ran={tools_ran_this_turn} retries={empty_continuation_retries} nudge={continuation_nudge_injected}",
                            assistant_text.len()
                        ),
                    );

                    append_to_log(
                        chat_log.to_str().unwrap_or("chat.log"),
                        &format!("ASSISTANT: {assistant_text}\n"),
                    );
                    break;
                }

                self.messages.push(Message {
                    role: "assistant".to_string(),
                    content: if assistant_text.is_empty() {
                        None
                    } else {
                        Some(assistant_text.clone())
                    },
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: None,
                });
                self.message_count += 1;

                #[cfg(feature = "tui")]
                {
                    if !thinking_text.is_empty() {
                        let line_count = thinking_text.lines().count();
                        if line_count > 100 {
                            let _ = std::fs::write(&thinking_log, &thinking_text);
                        }
                    }
                    if assistant_text.is_empty() && !thinking_text.is_empty() {
                        // Reasoning-only turn: already streamed inline
                    } else if assistant_text.is_empty() {
                        tui::print_system_note("No response from model.");
                    }
                }

                debug::log(
                    "agent",
                    &format!(
                        "inner loop exit: empty_tools assistant_len={} tools_ran={tools_ran_this_turn}",
                        assistant_text.len()
                    ),
                );

                append_to_log(
                    chat_log.to_str().unwrap_or("chat.log"),
                    &format!("ASSISTANT: {assistant_text}\n"),
                );
                break;
            } else {
                empty_continuation_retries = 0;

                debug::log(
                    "agent",
                    &format!("tool_calls count={}", tool_calls.len()),
                );

                #[cfg(feature = "tui")]
                if assistant_text.is_empty() && !thinking_text.is_empty() {
                    // partial stream already shown
                } else if !assistant_text.is_empty() && !agent_started {
                    tui::print_agent_text(&assistant_text);
                }

                let tc_objects: Vec<crate::provider::ToolCall> = tool_calls
                    .iter()
                    .map(|(id, name, args)| crate::provider::ToolCall {
                        id: id.clone(),
                        call_type: "function".to_string(),
                        function: crate::provider::FunctionCall {
                            name: name.clone(),
                            arguments: args.clone(),
                        },
                    })
                    .collect();

                self.messages.push(Message {
                    role: "assistant".to_string(),
                    content: if assistant_text.is_empty() {
                        None
                    } else {
                        Some(assistant_text)
                    },
                    tool_calls: Some(tc_objects),
                    tool_call_id: None,
                    reasoning_content: Some(thinking_text.clone()),
                });

                for (id, name, args_str) in &tool_calls {
                    if self.interrupt.load(Ordering::Relaxed) {
                        #[cfg(feature = "tui")]
                        tui::print_status_stopped();
                        break;
                    }

                    let args: serde_json::Value =
                        serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));

                    let risk = crate::tools::effective_risk(name, &args, &tool_registry);

                    let approved = self.global_approved_tools.contains(name)
                        || self.session_approved_tools.contains(name);

                    let needs_approval = risk == crate::tools::RiskLevel::Destructive
                        && self.config.tool_policy.ask_before_destructive
                        && !approved;

                    if needs_approval {
                        #[cfg(feature = "tui")]
                        {
                            let modal = tui::ApprovalModal {
                                tool_name: name.clone(),
                                command: args_str.clone(),
                                risk_level: format!("{:?}", risk),
                                selected: 0,
                            };
                            match run_approval_modal(&modal) {
                                tui::ApprovalChoice::Deny { comment } => {
                                    tui::print_system_note(&format!("Denied: {name}"));
                                    let msg = match comment.filter(|c| !c.trim().is_empty()) {
                                        Some(c) => {
                                            format!("[Tool '{name}' was denied by user: {c}]")
                                        }
                                        None => format!("[Tool '{name}' was denied by user]"),
                                    };
                                    self.messages.push(Message {
                                        role: "tool".to_string(),
                                        content: Some(msg),
                                        tool_calls: None,
                                        tool_call_id: Some(id.clone()),
                                        reasoning_content: None,
                                    });
                                    continue;
                                }
                                tui::ApprovalChoice::ApproveSession => {
                                    self.session_approved_tools.push(name.clone());
                                    tui::print_system_note(&format!("Approved for session: {name}"));
                                }
                                tui::ApprovalChoice::ApproveAlways => {
                                    self.session_approved_tools.push(name.clone());
                                    self.global_approved_tools.push(name.clone());
                                    let _ = self.config.approve_tool_globally(name);
                                    tui::print_system_note(&format!("Approved always: {name}"));
                                }
                                tui::ApprovalChoice::ApproveOnce => {
                                    tui::print_system_note(&format!("Approved once: {name}"));
                                }
                            }
                        }
                        #[cfg(not(feature = "tui"))]
                        {
                            println!("Approval required for tool '{name}'. Allow? [y/N]");
                            let mut ans = String::new();
                            let _ = std::io::stdin().read_line(&mut ans);
                            if !ans.trim().eq_ignore_ascii_case("y") {
                                self.messages.push(Message {
                                    role: "tool".to_string(),
                                    content: Some(format!("[Tool '{name}' was denied by user]")),
                                    tool_calls: None,
                                    tool_call_id: Some(id.clone()),
                                    reasoning_content: None,
                                });
                                continue;
                            }
                        }
                    }

                    #[cfg(feature = "tui")]
                    {
                        tui::layout::set_status_mode(tui::StatusMode::Executing(name.clone()));
                        let (tool_line_idx, tool_line_count) =
                            tui::begin_tool_call(name, &args_str);
                        debug::log("tool", &format!("begin {name} idx={tool_line_idx}"));

                        let tool_start = std::time::Instant::now();
                        let prev_content = file_content_before_tool(name, &args);
                        let result = tool_registry.execute(name, &args);
                        let tool_elapsed = tool_start.elapsed();

                        self.track_memory_write(name, &args, &result);
                        tools_ran_this_turn = true;

                        tui::layout::set_status_mode(tui::StatusMode::Flowing);

                        let line_count = result.lines().count();
                        let log_ref = if line_count > 50 {
                            append_to_log(
                                tool_log.to_str().unwrap_or("tool-output.log"),
                                &format!("=== {name} ===\n{result}\n"),
                            );
                            tool_log.display().to_string()
                        } else {
                            String::new()
                        };

                        let status = tui::tool_status_from_result(name, &result);
                        tui::update_tool_call(
                            tool_line_idx,
                            tool_line_count,
                            name,
                            &args_str,
                            &status,
                            Some(tool_elapsed),
                        );
                        debug::log(
                            "tool",
                            &format!(
                                "done {name} status={status:?} elapsed_ms={}",
                                tool_elapsed.as_millis()
                            ),
                        );

                        if let Some(diff_input) =
                            diff_input_for_file_tool(name, &args, prev_content.as_deref(), &result)
                        {
                            tui::print_tool_diff(diff_input);
                        } else {
                            tui::print_tool_result(name, &result, &log_ref);
                        }

                        self.messages.push(Message {
                            role: "tool".to_string(),
                            content: Some(result),
                            tool_calls: None,
                            tool_call_id: Some(id.clone()),
                            reasoning_content: None,
                        });
                        self.message_count += 1;
                    }

                    #[cfg(not(feature = "tui"))]
                    {
                        let tool_start = std::time::Instant::now();
                        let result = tool_registry.execute(name, &args);
                        self.track_memory_write(name, &args, &result);
                        tools_ran_this_turn = true;

                        self.messages.push(Message {
                            role: "tool".to_string(),
                            content: Some(result),
                            tool_calls: None,
                            tool_call_id: Some(id.clone()),
                            reasoning_content: None,
                        });
                        self.message_count += 1;
                    }
                }
            }
        }

        debug::log("agent", "chat() end");

        #[cfg(feature = "tui")]
        if let Some((desc, file)) = self.turn_memory_footer.clone() {
            tui::print_memory_footer(&desc, &file);
        }

        if let Some(path) = &self.session_path {
            self.persist_session(path);
        }

        Ok(())
    }

    fn track_memory_write(&mut self, tool_name: &str, args: &serde_json::Value, result: &str) {
        if tool_name != "write_file" {
            return;
        }
        let Some(path) = args.get("path").and_then(|p| p.as_str()) else {
            return;
        };
        let Some(contents) = args.get("contents").and_then(|c| c.as_str()) else {
            return;
        };

        let project_mem = memory::project_memory_path();
        let global_mem = memory::global_memory_path(&self.config.data_dir);
        let path_buf = PathBuf::from(path);

        let scope = if path_buf == project_mem {
            Some(MemoryScope::Project)
        } else if path_buf == global_mem {
            Some(MemoryScope::Global)
        } else {
            None
        };

        let Some(scope) = scope else {
            return;
        };

        match memory::append_memory(scope, contents, &self.config.data_dir) {
            Ok((desc, file)) => self.turn_memory_footer = Some((desc, file)),
            Err(err) => {
                #[cfg(feature = "tui")]
                tui::print_system_note(&err);
                let _ = result;
            }
        }
    }

    fn persist_session(&self, path: &str) {
        let entries: Vec<serde_json::Value> = self
            .messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| serde_json::to_value(m).unwrap_or(serde_json::json!({})))
            .collect();

        if let Ok(json) = serde_json::to_string(&entries) {
            let _ = std::fs::write(path, json);
        }
    }
}

fn session_log_dir(config: &Config, session_id: &str) -> PathBuf {
    config
        .data_dir_path()
        .join("sessions")
        .join(session_id)
}

fn args_preview(args_str: &str, max_len: usize) -> String {
    if args_str.len() > max_len {
        format!("{}...", &args_str[..max_len.saturating_sub(3)])
    } else {
        args_str.to_string()
    }
}

#[cfg(feature = "tui")]
fn file_content_before_tool(tool_name: &str, args: &serde_json::Value) -> Option<String> {
    if tool_name != "edit_file" && tool_name != "write_file" {
        return None;
    }
    let path = args.get("path")?.as_str()?;
    std::fs::read_to_string(path).ok()
}

#[cfg(feature = "tui")]
fn diff_input_for_file_tool(
    tool_name: &str,
    args: &serde_json::Value,
    prev_content: Option<&str>,
    result: &str,
) -> Option<tui::DiffInput> {
    let path = args.get("path")?.as_str()?.to_string();
    match tool_name {
        "edit_file" if result.starts_with("File edited:") => {
            let old_string = args.get("old_string")?.as_str()?;
            let new_string = args.get("new_string")?.as_str()?;
            let old_text = prev_content.unwrap_or("").to_string();
            let new_text = old_text.replacen(old_string, new_string, 1);
            Some(tui::DiffInput {
                path,
                old_text,
                new_text,
                kind: tui::DiffKind::Edit,
            })
        }
        "write_file" if result.starts_with("File written:") => {
            let new_text = args.get("contents")?.as_str()?.to_string();
            let kind = if prev_content.is_some() {
                tui::DiffKind::WriteOverwrite
            } else {
                tui::DiffKind::WriteCreate
            };
            Some(tui::DiffInput {
                path,
                old_text: prev_content.unwrap_or("").to_string(),
                new_text,
                kind,
            })
        }
        _ => None,
    }
}

fn append_to_log(path: &str, content: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = f.write_all(content.as_bytes());
    }
}

#[cfg(feature = "tui")]
fn tui_width() -> usize {
    tui::get_terminal_size().0
}

#[cfg(not(feature = "tui"))]
fn tui_width() -> usize {
    80
}

#[cfg(test)]
mod continuation_tests {
    use super::*;

    #[test]
    fn empty_response_detected() {
        assert!(is_empty_provider_response("", "", 0));
        assert!(!is_empty_provider_response("hi", "", 0));
        assert!(!is_empty_provider_response("", "think", 0));
        assert!(!is_empty_provider_response("", "", 1));
    }

    #[test]
    fn post_tool_retry_only_when_tools_ran() {
        assert!(should_retry_post_tool_continuation(true, "", "", 0));
        assert!(!should_retry_post_tool_continuation(false, "", "", 0));
        assert!(!should_retry_post_tool_continuation(true, "ok", "", 0));
    }
}
