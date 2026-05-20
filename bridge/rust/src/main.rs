#![allow(dead_code)]

#[cfg(feature = "tui")]
pub mod tui;

mod agent;
mod config;
mod debug;
mod memory;
mod gateway;
mod provider;
mod tools;
#[cfg(feature = "tui")]
mod profile;

use std::char;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    debug::init_from_env_and_args(&args);

    if args.iter().any(|a| a == "--bridge") {
        bridge_mode().expect("bridge mode failed");
    } else {
        interactive_mode().await;
    }
}

fn parse_config_flag(args: &[String]) -> Option<PathBuf> {
    args.iter()
        .position(|a| a == "--config")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
}

async fn interactive_mode() {
    let args: Vec<String> = std::env::args().collect();
    let config_path = parse_config_flag(&args);
    let mut cfg = if let Some(ref path) = config_path {
        config::Config::load_from(Some(path))
    } else {
        config::Config::load()
    };

    let provider = cfg.default_provider();
    if provider.requires_api_key() && provider.api_key.is_empty() {
        eprintln!(
            "\x1b[1;33m!\x1b[0m No API key configured for provider '{}'",
            provider.name
        );
        eprintln!("  Edit: {}", cfg.config_display_path());
        eprintln!("  Set \"api_key\" for your provider.\n");
    }

    let mut agent = agent::Agent::new(cfg.clone(), None);

    debug::set_log_path(
        cfg.data_dir_path()
            .join("sessions")
            .join(agent.session_id())
            .join("debug.log"),
    );

    #[cfg(feature = "tui")]
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".into());
    #[cfg(feature = "tui")]
    let tool_names: Vec<String> = tools::ToolRegistry::default()
        .list()
        .into_iter()
        .map(|(name, _, _)| name.to_string())
        .collect();

    #[cfg(feature = "tui")]
    let mut app = {
        let mut app = tui::App::new();
        app.model = agent.model().to_string();
        app.provider = agent.provider_id().to_string();
        app.session_name = "main".to_string();
        app.session_id = agent.session_id().to_string();
        app
    };

    #[cfg(feature = "tui")]
    let mut input = tui::input::InteractiveInput::new();

    #[cfg(feature = "tui")]
    let _terminal = match tui::layout::TerminalSession::enter() {
        Ok(session) => session,
        Err(e) => {
            eprintln!("TUI init failed: {e}");
            return;
        }
    };

    #[cfg(feature = "tui")]
    {
        match tui::layout::ScreenLayout::init(&app, &tool_names, &cwd) {
            Ok(layout) => layout.install_global(),
            Err(e) => {
                tui::layout::append_system_note(&format!("TUI layout init failed: {e}"));
            }
        }
        tui::layout::set_footer_app(&app);

        let data_dir = cfg.data_dir_path();
        let profile_file = profile::ProfileFile::load(&data_dir);
        app.profile = profile_file.to_user_profile();
        app.response_style = app.profile.style.clone();
        if !app.profile.name.is_empty() {
            tui::layout::set_user_display_name(&app.profile.name);
        }
        agent.apply_profile(&app.profile);

        if profile::needs_onboarding(&data_dir) {
            match tui::onboarding::run_onboarding(&app) {
                Ok(tui::onboarding::OnboardingResult::Completed(profile)) => {
                    app.profile = profile.clone();
                    app.response_style = profile.style.clone();
                    tui::layout::set_user_display_name(&profile.name);
                    agent.apply_profile(&profile);
                    let saved = profile::ProfileFile::from_user_profile(&profile);
                    let mut complete = saved;
                    complete.onboarding_complete = true;
                    if let Err(e) = complete.save(&data_dir) {
                        tui::print_system_note(&format!("Could not save profile: {e}"));
                    }
                    tui::layout::enter_chat_mode(&app, &tool_names, &cwd);
                }
                Ok(tui::onboarding::OnboardingResult::Cancelled) => return,
                Err(e) => {
                    tui::print_system_note(&format!("Onboarding error: {e}"));
                    return;
                }
            }
        }
    }

    #[cfg(not(feature = "tui"))]
    {
        println!("\x1b[1;36m\u{250c}\u{2500}\x1b[0m ZERO Agent");
        println!("\x1b[1;36m\u{2502}\x1b[0m Provider: \x1b[33m{}\x1b[0m / \x1b[33m{}\x1b[0m", provider.name, provider.default_model);
        println!("\x1b[1;36m\u{2502}\x1b[0m Type your message, or \x1b[1m/quit\x1b[0m to exit.");
        println!("\x1b[1;36m\u{2514}\x1b[0m");
    }

    loop {
        #[cfg(feature = "tui")]
        let input_line = {
            app.model = agent.model().to_string();
            app.provider = agent.provider_id().to_string();
            app.session_id = agent.session_id().to_string();
            tui::layout::set_footer_app(&app);
            tui::layout::set_status_mode(tui::StatusMode::Idle);
            match input.read_line(&app) {
                Ok(tui::input::InputResult::Submit(line)) => line,
                Ok(tui::input::InputResult::Interrupt) => {
                    agent.request_interrupt();
                    tui::print_status_stopped();
                    continue;
                }
                Ok(tui::input::InputResult::Empty) => continue,
                Err(e) => {
                    eprintln!("Input error: {e}");
                    break;
                }
            }
        };

        #[cfg(not(feature = "tui"))]
        let input_line = {
            print!("\n\x1b[1;32m>\x1b[0m ");
            io::stdout().flush().unwrap();
            let mut line = String::new();
            match io::stdin().lock().read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => line.trim().to_string(),
                Err(e) => {
                    eprintln!("Input error: {e}");
                    break;
                }
            }
        };

        if input_line.is_empty() {
            continue;
        }

        #[cfg(feature = "tui")]
        if input_line.starts_with('/') {
            let rest = input_line.trim_start_matches('/');
            let cmd_name = rest.split_whitespace().next().unwrap_or("");
            if cmd_name.is_empty() {
                tui::print_slash_palette("");
                continue;
            }
            let is_known = input_line.starts_with("/reasoning")
                || input_line.starts_with("/debug")
                || tui::all_slash_commands().iter().any(|c| {
                    c.name == cmd_name
                        && (rest == c.name || rest.starts_with(&format!("{} ", c.name)))
                });
            if !is_known {
                tui::print_slash_palette(cmd_name);
                continue;
            }
        }

        match input_line.as_str() {
            "/quit" | "/exit" | "/q" => break,
            "/help" => {
                #[cfg(feature = "tui")]
                tui::print_help();
                continue;
            }
            "/clear" => {
                #[cfg(feature = "tui")]
                {
                    app.session_id = agent.session_id().to_string();
                    tui::layout::clear_transcript(&app, &tool_names, &cwd);
                    tui::layout::set_footer_app(&app);
                }
                continue;
            }
            "/provider" => {
                #[cfg(feature = "tui")]
                {
                    let providers: Vec<String> = cfg.providers.iter().map(|p| p.id.clone()).collect();
                    if let Some(selected) = tui::input::run_provider_picker(&providers, agent.provider_id()) {
                        match agent.set_provider(&selected) {
                            Ok(()) => {
                                cfg = config::Config::load();
                                app.provider = agent.provider_id().to_string();
                                app.model = agent.model().to_string();
                                tui::print_system_note(&format!("Provider set to {selected}"));
                            }
                            Err(e) => tui::print_system_note(&e),
                        }
                    }
                }
                #[cfg(not(feature = "tui"))]
                println!("Provider: {}", agent.provider_info());
                continue;
            }
            "/model" => {
                #[cfg(feature = "tui")]
                {
                    let models = agent.discover_models().await;
                    match tui::input::run_model_picker(&models, agent.provider_id()) {
                        tui::input::ModelPickerResult::Selected(model) => {
                            match agent.set_model(&model) {
                                Ok(()) => {
                                    cfg = config::Config::load();
                                    app.model = model.clone();
                                    tui::print_system_note(&format!("Model set to {model}"));
                                }
                                Err(e) => tui::print_system_note(&e),
                            }
                        }
                        tui::input::ModelPickerResult::ChangeProvider => {
                            let providers: Vec<String> =
                                cfg.providers.iter().map(|p| p.id.clone()).collect();
                            if let Some(selected) =
                                tui::input::run_provider_picker(&providers, agent.provider_id())
                            {
                                let _ = agent.set_provider(&selected);
                                cfg = config::Config::load();
                                app.provider = agent.provider_id().to_string();
                                app.model = agent.model().to_string();
                            }
                        }
                        tui::input::ModelPickerResult::Cancelled => {}
                    }
                }
                continue;
            }
            "/memory" => {
                let data_dir = cfg.data_dir_path().to_string_lossy().into_owned();
                let listing = memory::list_memories(&data_dir);
                #[cfg(feature = "tui")]
                {
                    for line in listing.lines() {
                        tui::print_system_note(line);
                    }
                }
                #[cfg(not(feature = "tui"))]
                println!("{listing}");
                continue;
            }
            "/status" => {
                #[cfg(feature = "tui")]
                {
                    app.model = agent.model().to_string();
                    app.provider = agent.provider_id().to_string();
                    app.session_id = agent.session_id().to_string();
                    tui::print_status_info(&app);
                    tui::print_system_note(&format!("Config: {}", cfg.config_display_path()));
                    if !cfg.default_provider().requires_api_key() {
                        tui::print_system_note("Auth: no API key required for this provider");
                    }
                }
                #[cfg(not(feature = "tui"))]
                println!("Status: {}", agent.provider_info());
                continue;
            }
            "/profile" => {
                #[cfg(feature = "tui")]
                tui::print_profile(&app.profile);
                continue;
            }
            cmd if cmd.starts_with("/debug") => {
                #[cfg(feature = "tui")]
                {
                    let rest = input_line.trim_start_matches("/debug").trim();
                    match rest {
                        "on" => {
                            debug::set_enabled(true);
                            tui::print_system_note("Debug logging enabled");
                        }
                        "off" => {
                            debug::set_enabled(false);
                            tui::print_system_note("Debug logging disabled");
                        }
                        "status" => {
                            let state = if debug::is_enabled() { "on" } else { "off" };
                            let path =
                                debug::log_path_display().unwrap_or_else(|| "(not set)".into());
                            tui::print_system_note(&format!("Debug: {state} — log: {path}"));
                        }
                        "" => {
                            let on = debug::toggle();
                            let state = if on { "on" } else { "off" };
                            tui::print_system_note(&format!("Debug logging {state}"));
                        }
                        _ => {
                            tui::print_system_note("Usage: /debug | /debug on | off | status");
                        }
                    }
                }
                continue;
            }
            "/stop" => {
                agent.request_interrupt();
                tui::print_status_stopped();
                continue;
            }
            cmd if cmd.starts_with("/reasoning") => {
                let level = cmd.trim_start_matches("/reasoning").trim();
                let label = match level {
                    "low" => "low",
                    "med" | "medium" => "med",
                    "high" => "high",
                    "x-high" | "xhigh" | "extra" => "x-high",
                    "thinking" => "thinking",
                    "off" | "" => "",
                    _ => {
                        #[cfg(feature = "tui")]
                        tui::print_system_note(
                            "Usage: /reasoning off | low | med | high | x-high | thinking",
                        );
                        continue;
                    }
                };
                #[cfg(feature = "tui")]
                {
                    app.reasoning_label = label.to_string();
                    if label.is_empty() {
                        tui::print_system_note(&format!("Reasoning off — model: {}", app.model));
                    } else {
                        tui::print_system_note(&format!(
                            "Reasoning set to '{label}' — model: {} ({label})",
                            app.model
                        ));
                    }
                }
                continue;
            }
            cmd if cmd.starts_with('/') => {
                #[cfg(feature = "tui")]
                {
                    let cmd_name = cmd.trim_start_matches('/').split_whitespace().next().unwrap_or("");
                    if tui::filter_slash_commands(cmd_name).is_empty() {
                        tui::print_system_note(&format!("Unknown command: {cmd}"));
                    } else {
                        tui::print_system_note(&format!("Command '{cmd}' is not implemented yet."));
                    }
                }
                continue;
            }
            _ => {}
        }

        #[cfg(feature = "tui")]
        {
            let listener = tui::input::TurnInputListener::start(agent.interrupt_handle());
            let chat_result = agent.chat(&input_line).await;
            drop(listener);
            if let Err(e) = chat_result {
                tui::print_system_note(&format!("Error: {e}"));
            }
        }

        #[cfg(not(feature = "tui"))]
        if let Err(e) = agent.chat(&input_line).await {
            eprintln!("\x1b[31mError: {e}\x1b[0m");
        }

        #[cfg(feature = "tui")]
        {
            app.status_mode = tui::StatusMode::Idle;
            tui::layout::set_status_mode(tui::StatusMode::Idle);
            tui::layout::set_footer_app(&app);
        }
    }

    #[cfg(feature = "tui")]
    {
        app.start_time =
            std::time::Instant::now() - std::time::Duration::from_secs_f64(agent.elapsed_secs());
        tui::print_exit_summary(&app);
    }

    #[cfg(not(feature = "tui"))]
    {
        println!("\x1b[2mGoodbye.\x1b[0m");
    }
}

fn bridge_mode() -> io::Result<()> {
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
        "tui.render" => tui_render(&id, line),
        "tui.emit" => tui_emit(&id, line),
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

    let config = gateway::GatewayConfig {
        token,
        allowed_users,
        api_key,
        model,
        provider,
        session_dir,
    };

    // Run gateway in a blocking tokio runtime
    let rt = tokio::runtime::Runtime::new().unwrap();
    match rt.block_on(gateway::run_gateway(config)) {
        Ok(()) => format!(
            "{{\"id\":{},\"ok\":true,\"event\":\"telegram.loop\",\"output\":{{\"status\":\"stopped\"}}}}",
            json_string(id)
        ),
        Err(e) => error_response(id, &format!("gateway error: {e}")),
    }
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

#[cfg(feature = "tui")]
fn tui_render(id: &str, line: &str) -> String {
    let payload = match json_field(line, "event") {
        Ok(Some(v)) => v,
        Ok(None) => return error_response(id, "missing event"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let mut ctx = tui::render::RenderContext {
        app: tui::App::new(),
        width: tui::get_terminal_size().0,
    };
    match tui::render::render_event(&mut ctx, &payload) {
        Ok(()) => format!(
            "{{\"id\":{},\"ok\":true,\"event\":\"tui.rendered\",\"output\":{{}}}}",
            json_string(id)
        ),
        Err(e) => error_response(id, &e),
    }
}

#[cfg(not(feature = "tui"))]
fn tui_render(id: &str, _line: &str) -> String {
    error_response(id, "tui feature not enabled")
}

#[cfg(feature = "tui")]
fn tui_emit(id: &str, line: &str) -> String {
    let kind = match json_field(line, "kind") {
        Ok(Some(v)) => v,
        Ok(None) => return error_response(id, "missing kind"),
        Err(_) => return error_response(id, "invalid bridge request"),
    };
    let mut ctx = tui::render::RenderContext {
        app: tui::App::new(),
        width: tui::get_terminal_size().0,
    };
    if kind == "SessionStarted" {
        let config_path = json_field(line, "config_path")
            .ok()
            .flatten()
            .unwrap_or_else(|| ".zero-agent/config.json".into());
        let cwd = json_field(line, "cwd")
            .ok()
            .flatten()
            .unwrap_or_else(|| ".".into());
        let no_auth = json_field(line, "no_auth")
            .ok()
            .flatten()
            .map(|v| v == "true")
            .unwrap_or(false);
        tui::render::render_session_started(&mut ctx, &config_path, &cwd, no_auth);
    } else {
        let _ = tui::render::render_event(&mut ctx, line);
    }
    format!(
        "{{\"id\":{},\"ok\":true,\"event\":\"tui.emitted\",\"output\":{{\"kind\":{}}}}}",
        json_string(id),
        json_string(&kind)
    )
}

#[cfg(not(feature = "tui"))]
fn tui_emit(id: &str, _line: &str) -> String {
    error_response(id, "tui feature not enabled")
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
