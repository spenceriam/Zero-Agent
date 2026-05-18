use crate::config::Config;
use crate::provider::{Message, Provider, StreamEventType};
use crate::tools;

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

        let session_path = format!(
            "{}/{}.jsonl",
            session_dir,
            chrono::Utc::now().format("%Y%m%d-%H%M%S")
        );

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

    pub fn message_count(&self) -> usize {
        self.message_count
    }

    pub fn elapsed_secs(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    pub async fn chat(&mut self, user_input: &str) -> Result<(), String> {
        // Show user block
        #[cfg(feature = "tui")]
        tui::print_user_block(user_input);

        // Add user message
        self.messages.push(Message {
            role: "user".to_string(),
            content: Some(user_input.to_string()),
            tool_calls: None,
            tool_call_id: None,
        });
        self.message_count += 1;

        let tools = tools::builtin_tools();

        loop {
            // Start spinner
            #[cfg(feature = "tui")]
            let (spinner_handle, spinner_stop) = tui::start_spinner();

            let events = self.provider.stream_chat(&self.messages, &tools).await;

            // Stop spinner
            #[cfg(feature = "tui")]
            tui::stop_spinner(spinner_handle, spinner_stop);

            let events = events?;

            let mut assistant_text = String::new();
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

                // Show agent block
                #[cfg(feature = "tui")]
                tui::print_agent_block(&assistant_text);

                break;
            } else {
                // Response with tool calls
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
                });

                // Execute each tool
                for (id, name, args_str) in &tool_calls {
                    // Show tool call line (running)
                    #[cfg(feature = "tui")]
                    {
                        let args_preview = if args_str.len() > 40 {
                            format!("{}...", &args_str[..40])
                        } else {
                            args_str.clone()
                        };
                        tui::print_tool_line(name, &args_preview, &tui::ToolStatus::Running, None);
                    }

                    let tool_start = std::time::Instant::now();
                    let args: serde_json::Value =
                        serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));
                    let result = tools::execute_tool(name, &args);
                    let tool_elapsed = tool_start.elapsed();

                    // Show tool call line (completed)
                    #[cfg(feature = "tui")]
                    {
                        let args_preview = if args_str.len() > 40 {
                            format!("{}...", &args_str[..40])
                        } else {
                            args_str.clone()
                        };
                        tui::print_tool_line(
                            name,
                            &args_preview,
                            &tui::ToolStatus::Success,
                            Some(tool_elapsed),
                        );
                    }

                    self.messages.push(Message {
                        role: "tool".to_string(),
                        content: Some(result),
                        tool_calls: None,
                        tool_call_id: Some(id.clone()),
                    });
                    self.message_count += 1;
                }

                // Continue loop for more tool calls or final text
            }
        }

        // Show session summary
        #[cfg(feature = "tui")]
        {
            let mut app = tui::App::new();
            app.model = self.model().to_string();
            app.provider = self.provider_id().to_string();
            app.session_name = "main".to_string();
            app.start_time = self.start_time;
            tui::print_session_summary(&app);
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
