use crate::config::Config;
use crate::provider::{Message, Provider, StreamEventType};
use crate::tools::ToolRegistry;

#[cfg(feature = "tui")]
use crate::tui;

const SYSTEM_PROMPT: &str = r#"You are ZERO, a personal AI assistant for developers.
You are running locally on the user's machine.
You have access to tools for reading/writing files, running shell commands, and searching files.
Be concise and direct. When you need to do something, use your tools.
Always use tools to interact with the filesystem or run commands - never just describe what to do."#;

pub struct Agent {
    provider: Provider,
    messages: Vec<Message>,
    session_path: Option<String>,
    session_id: String,
    message_count: usize,
    start_time: std::time::Instant,
}

impl Agent {
    pub fn new(config: &Config, provider_id: Option<&str>) -> Self {
        let provider_config = provider_id
            .and_then(|id| config.get_provider(id))
            .unwrap_or_else(|| config.default_provider())
            .clone();

        let provider = Provider::new(provider_config);

        let session_dir = format!("{}/sessions", config.data_dir);
        let _ = std::fs::create_dir_all(&session_dir);

        let session_id = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let session_path = format!("{}/{}.jsonl", session_dir, session_id);

        let messages = vec![Message {
            role: "system".to_string(),
            content: Some(SYSTEM_PROMPT.to_string()),
            tool_calls: None,
            tool_call_id: None,
        }];

        Self {
            provider,
            messages,
            session_path: Some(session_path),
            session_id,
            message_count: 0,
            start_time: std::time::Instant::now(),
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

    pub async fn chat(&mut self, user_input: &str) -> Result<(), String> {
        let term_width = tui_width();

        // Show user block
        #[cfg(feature = "tui")]
        tui::print_user_block(user_input, term_width);

        // Add user message
        self.messages.push(Message {
            role: "user".to_string(),
            content: Some(user_input.to_string()),
            tool_calls: None,
            tool_call_id: None,
        });
        self.message_count += 1;

        let tool_registry = ToolRegistry::default();
        let tools = tool_registry.definitions();

        // Build log path for this session
        let log_dir = format!("{}/.zero-agent/sessions/{}", home_dir(), self.session_id);
        let _ = std::fs::create_dir_all(&log_dir);
        let thinking_log = format!("{}/thinking.log", log_dir);
        let tool_log = format!("{}/tool-output.log", log_dir);

        loop {
            // Start shimmer status line
            #[cfg(feature = "tui")]
            let (shimmer_handle, shimmer_stop, shimmer_mode) =
                tui::start_shimmer_status(tui::StatusMode::Flowing);

            let events = self.provider.stream_chat(&self.messages, &tools).await;

            // Stop shimmer
            #[cfg(feature = "tui")]
            tui::stop_shimmer(shimmer_handle, shimmer_stop);

            let events = events?;

            let mut assistant_text = String::new();
            let mut thinking_text = String::new();
            let mut tool_calls: Vec<(String, String, String)> = Vec::new();

            for event in &events {
                match &event.event_type {
                    StreamEventType::Text(text) => {
                        assistant_text.push_str(text);
                    }
                    StreamEventType::ToolCall { id, name, arguments } => {
                        tool_calls.push((id.clone(), name.clone(), arguments.clone()));
                    }
                    StreamEventType::Done => {}
                }
            }

            if tool_calls.is_empty() {
                // Text-only response
                self.messages.push(Message {
                    role: "assistant".to_string(),
                    content: Some(assistant_text.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                });
                self.message_count += 1;

                // Show thinking block if any
                #[cfg(feature = "tui")]
                if !thinking_text.is_empty() {
                    let line_count = thinking_text.lines().count();
                    let display = if line_count > 100 {
                        // Write full to log
                        let _ = std::fs::write(&thinking_log, &thinking_text);
                        thinking_text.lines().take(100).collect::<Vec<_>>().join("\n")
                    } else {
                        thinking_text.clone()
                    };
                    let log_ref = if line_count > 100 { thinking_log.as_str() } else { "" };
                    tui::print_thinking_block(&display, log_ref);
                }

                // Show agent text
                #[cfg(feature = "tui")]
                tui::print_agent_text(&assistant_text);

                break;
            } else {
                // Response with tool calls — may have partial text first
                #[cfg(feature = "tui")]
                if !assistant_text.is_empty() {
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
                    content: if assistant_text.is_empty() { None } else { Some(assistant_text) },
                    tool_calls: Some(tc_objects),
                    tool_call_id: None,
                });

                // Execute each tool
                for (id, name, args_str) in &tool_calls {
                    // Show tool call line — Running
                    #[cfg(feature = "tui")]
                    {
                        let preview = args_preview(args_str, 50);
                        tui::print_tool_call(name, &preview, &tui::ToolStatus::Running, None);
                    }

                    // Update shimmer mode to Executing
                    #[cfg(feature = "tui")]
                    let (exec_shimmer_handle, exec_shimmer_stop, _exec_mode) =
                        tui::start_shimmer_status(tui::StatusMode::Executing(name.clone()));

                    let tool_start = std::time::Instant::now();
                    let args: serde_json::Value =
                        serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));
                    let result = tool_registry.execute(name, &args);
                    let tool_elapsed = tool_start.elapsed();

                    // Stop exec shimmer
                    #[cfg(feature = "tui")]
                    tui::stop_shimmer(exec_shimmer_handle, exec_shimmer_stop);

                    // Write tool result to log and show truncated
                    #[cfg(feature = "tui")]
                    {
                        let line_count = result.lines().count();
                        let log_ref = if line_count > 50 {
                            append_to_log(&tool_log, &format!("=== {} ===\n{}\n", name, result));
                            tool_log.as_str()
                        } else {
                            ""
                        };

                        // Show completed tool call line
                        let preview = args_preview(args_str, 50);
                        tui::print_tool_call(name, &preview, &tui::ToolStatus::Success, Some(tool_elapsed));
                        tui::print_tool_result(name, &result, log_ref);
                    }

                    self.messages.push(Message {
                        role: "tool".to_string(),
                        content: Some(result),
                        tool_calls: None,
                        tool_call_id: Some(id.clone()),
                    });
                    self.message_count += 1;
                }
            }
        }

        // Persist session
        if let Some(path) = &self.session_path {
            self.persist_session(path);
        }

        Ok(())
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

fn args_preview(args_str: &str, max_len: usize) -> String {
    if args_str.len() > max_len {
        format!("{}...", &args_str[..max_len.saturating_sub(3)])
    } else {
        args_str.to_string()
    }
}

fn append_to_log(path: &str, content: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(content.as_bytes());
    }
}

fn home_dir() -> String {
    std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
}

#[cfg(feature = "tui")]
fn tui_width() -> usize {
    tui::get_terminal_size().0
}

#[cfg(not(feature = "tui"))]
fn tui_width() -> usize {
    80
}
