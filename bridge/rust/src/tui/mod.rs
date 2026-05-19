use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// ANSI color codes
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const ITALIC: &str = "\x1b[3m";
const UNDERLINE: &str = "\x1b[4m";
const STRIKETHROUGH: &str = "\x1b[9m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const WHITE: &str = "\x1b[37m";
const BRIGHT_CYAN: &str = "\x1b[96m";

// Coral color (true color)
const CORAL: &str = "\x1b[38;2;255;127;80m";

// Braille spinner frames — Core Pulse pattern
const SPINNER_FRAMES: &[&str] = &["\u{2800}\u{2836}\u{2800}", "\u{2830}\u{28ff}\u{2806}", "\u{283e}\u{28ff}\u{2837}", "\u{283e}\u{2809}\u{2837}", "\u{28cf}\u{2809}\u{2839}", "\u{2801}\u{2800}\u{2808}", "\u{2800}\u{2836}\u{2800}"];
const SPINNER_MSG: &str = "Zeroing in";
const SPINNER_INTERVAL_MS: u64 = 120;

/// Response style for agent output
#[derive(Debug, Clone, PartialEq)]
pub enum ResponseStyle {
    Concise,
    Verbose,
    Technical,
    NonTechnical,
    Persona(String),
}

impl ResponseStyle {
    pub fn name(&self) -> &str {
        match self {
            ResponseStyle::Concise => "concise",
            ResponseStyle::Verbose => "verbose",
            ResponseStyle::Technical => "technical",
            ResponseStyle::NonTechnical => "non-technical",
            ResponseStyle::Persona(name) => name,
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "concise" => ResponseStyle::Concise,
            "verbose" => ResponseStyle::Verbose,
            "technical" => ResponseStyle::Technical,
            "non-technical" => ResponseStyle::NonTechnical,
            other => ResponseStyle::Persona(other.to_string()),
        }
    }

    pub fn system_prompt(&self) -> &str {
        match self {
            ResponseStyle::Concise => "Be concise and direct. No fluff. Like talking to a senior engineer.",
            ResponseStyle::Verbose => "Provide detailed explanations with context. Good for learning.",
            ResponseStyle::Technical => "Be code-focused. Include implementation details, architecture notes.",
            ResponseStyle::NonTechnical => "Use plain language, no jargon. Good for non-developers.",
            ResponseStyle::Persona(_) => "Respond in the style and personality of the specified character.",
        }
    }
}

/// A message in the conversation
#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub thinking: Option<String>,
    pub timestamp: Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

/// A tool call displayed in the conversation
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub input: String,
    pub output: Option<String>,
    pub status: ToolStatus,
    pub elapsed: Option<Duration>,
    pub start_time: Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolStatus {
    Running,
    Success,
    Error,
}

/// Activity log entry
#[derive(Debug, Clone)]
pub struct ActivityEntry {
    pub tool_name: String,
    pub status: ToolStatus,
    pub elapsed: Option<Duration>,
    pub timestamp: Instant,
}

/// Approval request for tool execution
#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub tool_name: String,
    pub description: String,
    pub risk_level: String,
    pub input_preview: String,
}

/// User profile (soul.md concept)
#[derive(Debug, Clone)]
pub struct UserProfile {
    pub name: String,
    pub role: String,
    pub about: String,
    pub style: ResponseStyle,
    pub persona: String,
}

impl UserProfile {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            role: String::new(),
            about: String::new(),
            style: ResponseStyle::Concise,
            persona: String::new(),
        }
    }

    pub fn to_soul_md(&self) -> String {
        let mut soul = String::new();
        if !self.name.is_empty() {
            soul.push_str(&format!("Name: {}\n", self.name));
        }
        if !self.role.is_empty() {
            soul.push_str(&format!("Role: {}\n", self.role));
        }
        if !self.about.is_empty() {
            soul.push_str(&format!("About: {}\n", self.about));
        }
        if !self.persona.is_empty() {
            soul.push_str(&format!("Persona: {}\n", self.persona));
        }
        soul.push_str(&format!("Communication style: {}\n", self.style.name()));
        soul
    }
}

/// TUI application state
pub struct App {
    pub model: String,
    pub provider: String,
    pub session_name: String,
    pub messages: Vec<Message>,
    pub activity_log: Vec<ActivityEntry>,
    pub start_time: Instant,
    pub terminal_width: usize,
    pub response_style: ResponseStyle,
    pub profile: UserProfile,
    pub input_history: Vec<String>,
    pub history_index: Option<usize>,
    pub tokens_used: usize,
    pub current_job: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            model: String::from("unknown"),
            provider: String::from("unknown"),
            session_name: String::from("main"),
            messages: Vec::new(),
            activity_log: Vec::new(),
            start_time: Instant::now(),
            terminal_width: 80,
            response_style: ResponseStyle::Concise,
            profile: UserProfile::new(),
            input_history: Vec::new(),
            history_index: None,
            tokens_used: 0,
            current_job: None,
        }
    }

    pub fn add_message(&mut self, role: MessageRole, content: String) {
        self.messages.push(Message {
            role,
            content,
            tool_calls: Vec::new(),
            thinking: None,
            timestamp: Instant::now(),
        });
    }

    pub fn add_tool_call(&mut self, name: String, input: String) {
        let entry = ActivityEntry {
            tool_name: name.clone(),
            status: ToolStatus::Running,
            elapsed: None,
            timestamp: Instant::now(),
        };
        self.activity_log.push(entry);

        if let Some(msg) = self.messages.last_mut() {
            msg.tool_calls.push(ToolCall {
                name,
                input,
                output: None,
                status: ToolStatus::Running,
                elapsed: None,
                start_time: Instant::now(),
            });
        }
    }

    pub fn update_tool_status(&mut self, name: &str, status: ToolStatus, output: Option<String>) {
        if let Some(entry) = self.activity_log.iter_mut().rev().find(|e| e.tool_name == name) {
            entry.status = status.clone();
            entry.elapsed = Some(entry.timestamp.elapsed());
        }

        if let Some(msg) = self.messages.last_mut() {
            if let Some(tc) = msg.tool_calls.iter_mut().rev().find(|t| t.name == name) {
                tc.status = status;
                tc.output = output;
                tc.elapsed = Some(tc.start_time.elapsed());
            }
        }
    }
}

// ─── ANSI Helpers ────────────────────────────────────────────────────────────

/// Clear the screen and move cursor to top
pub fn clear_screen() {
    print!("\x1b[2J\x1b[H");
}

/// Move cursor to specific position
pub fn move_cursor(row: u16, col: u16) {
    print!("\x1b[{};{}H", row, col);
}

/// Clear from cursor to end of line
pub fn clear_line() {
    print!("\x1b[K");
}

/// Clear from cursor to end of screen
pub fn clear_to_end() {
    print!("\x1b[J");
}

/// Get terminal size (columns, rows)
pub fn get_terminal_size() -> (usize, usize) {
    // In a real TUI we'd use termion or similar
    (100, 30)
}

/// Render the full Hermes-style split-pane layout
pub fn render_layout(app: &App) {
    clear_screen();
    let (width, _height) = get_terminal_size();
    
    // Header
    print_status_bar(app);
    println!("{}", "\u{2500}".repeat(width));
    
    // Split Panes: Conversation (left) | Activity (right)
    let left_width = (width as f64 * 0.7) as usize;
    let right_width = width - left_width - 1;
    
    // For a scrolling ANSI TUI, we might just print things in order,
    // but the layout concept suggests we want to show both.
    // Since we are in a terminal, we'll focus on a "Status-rich scroll"
    // rather than a full fixed-position TUI for now, as that's more robust
    // without heavy dependencies.
    
    for msg in &app.messages {
        match msg.role {
            MessageRole::User => print_user_block(&msg.content),
            MessageRole::Assistant => {
                print_agent_block(&msg.content);
                for tc in &msg.tool_calls {
                    print_tool_line(&tc.name, &tc.input, &tc.status, tc.elapsed);
                }
            }
            MessageRole::System => print_system_block(&msg.content),
            MessageRole::Tool => {} // Handled inside assistant block usually
        }
    }
    
    if let Some(job) = &app.current_job {
        println!("\n  {DIM}Job: {}{RESET}", job);
    }
}

// ─── Braille Spinner ─────────────────────────────────────────────────────────

/// Start the braille spinner in a background thread.
/// Returns (JoinHandle, stop_flag) — set stop_flag to true then join to stop.
pub fn start_spinner() -> (thread::JoinHandle<()>, Arc<AtomicBool>) {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    let handle = thread::spawn(move || {
        let start = Instant::now();
        let mut frame_idx: usize = 0;
        while !stop_clone.load(Ordering::Relaxed) {
            let elapsed = start.elapsed().as_secs();
            let frame = SPINNER_FRAMES[frame_idx % SPINNER_FRAMES.len()];
            print!("\r  {} {}{}... ({}s){RESET}   ", frame, CYAN, SPINNER_MSG, elapsed);
            io::stdout().flush().unwrap();
            frame_idx = frame_idx.wrapping_add(1);
            thread::sleep(Duration::from_millis(SPINNER_INTERVAL_MS));
        }
    });
    (handle, stop)
}

/// Stop the spinner thread and clear the spinner line.
pub fn stop_spinner(handle: thread::JoinHandle<()>, stop: Arc<AtomicBool>) {
    stop.store(true, Ordering::Relaxed);
    let _ = handle.join();
    clear_line();
    print!("\r");
    io::stdout().flush().unwrap();
}

// ─── Status Bar (Hermes-style) ───────────────────────────────────────────────

/// Print the Hermes-style status bar:
///  model-name │ provider │ N msgs │ X.Xs
pub fn print_status_bar(app: &App) {
    let msg_count = app.messages.len();
    let elapsed = app.start_time.elapsed().as_secs_f64();
    let elapsed_str = if elapsed >= 60.0 {
        format!("{}m {:.0}s", (elapsed / 60.0) as u64, elapsed % 60.0)
    } else {
        format!("{:.1}s", elapsed)
    };
    println!(
        "  {DIM} {BRIGHT_CYAN}{}{RESET}{DIM} \u{2502} {} \u{2502} {} msgs \u{2502} {}{RESET}",
        app.model, app.provider, msg_count, elapsed_str
    );
}

// ─── Tool Call Display (Hermes-style) ────────────────────────────────────────

/// Get the emoji icon for a tool name.
fn tool_icon(name: &str) -> &'static str {
    match name {
        "read_file" => "\u{1f4c4}",   // page facing up
        "write_file" => "\u{270f}\u{fe0f}",  // pencil
        "edit_file" => "\u{270f}\u{fe0f}",   // pencil
        "shell" => "\u{1f4bb}",       // laptop
        "glob" => "\u{1f50d}",        // magnifying glass
        _ => "\u{2699}\u{fe0f}",      // gear
    }
}

/// Print a single Hermes-style tool call line:
///   name args_preview (X.Xs)
pub fn print_tool_line(name: &str, args_preview: &str, status: &ToolStatus, elapsed: Option<Duration>) {
    let (icon, color) = match status {
        ToolStatus::Running => (tool_icon(name), YELLOW),
        ToolStatus::Success => (tool_icon(name), GREEN),
        ToolStatus::Error => ("\u{2717}", RED),
    };
    let elapsed_str = elapsed
        .map(|d| format!(" ({:.1}s)", d.as_secs_f64()))
        .unwrap_or_default();
    let preview = if args_preview.len() > 40 {
        format!("{}...", &args_preview[..37])
    } else {
        args_preview.to_string()
    };
    println!(
        "  {DIM}  \u{250a} {color}{icon} {name} {DIM}`{preview}`{elapsed_str}{RESET}"
    );
}

// ─── Conversation Blocks ─────────────────────────────────────────────────────

/// Default box width for conversation blocks.
const BOX_WIDTH: usize = 60;

/// Print a user message in a box:
pub fn print_user_block(text: &str) {
    let width = BOX_WIDTH;
    let title = "You";
    let top_pad = width.saturating_sub(title.len() + 4);
    println!(
        "\n  {CORAL}{BOLD}\u{250c}\u{2500} {title} {}\u{2510}{RESET}",
        "\u{2500}".repeat(top_pad)
    );
    for line in text.lines() {
        let vis_len = visible_len(line);
        let pad = width.saturating_sub(vis_len).saturating_sub(1);
        println!(
            "  {CORAL}\u{2502}{RESET} {}{}{CORAL}\u{2502}{RESET}",
            line,
            " ".repeat(pad)
        );
    }
    println!(
        "  {CORAL}{BOLD}\u{2514}{}\u{2518}{RESET}",
        "\u{2500}".repeat(width)
    );
}

/// Print an agent response in a box with markdown rendering inside.
pub fn print_agent_block(text: &str) {
    let width = BOX_WIDTH;
    let title = "ZERO";
    let top_pad = width.saturating_sub(title.len() + 4);
    println!(
        "\n  {CYAN}{BOLD}\u{250c}\u{2500} {title} {}\u{2510}{RESET}",
        "\u{2500}".repeat(top_pad)
    );
    for line in text.lines() {
        let rendered = render_inline(line);
        let vis = visible_len(&rendered);
        let pad = width.saturating_sub(vis).saturating_sub(1);
        println!(
            "  {CYAN}\u{2502}{RESET} {}{}{CYAN}\u{2502}{RESET}",
            rendered,
            " ".repeat(pad)
        );
    }
    println!(
        "  {CYAN}{BOLD}\u{2514}{}\u{2518}{RESET}",
        "\u{2500}".repeat(width)
    );
}

/// Print a dim system message in a box.
pub fn print_system_block(text: &str) {
    let width = BOX_WIDTH;
    let title = "SYS";
    let top_pad = width.saturating_sub(title.len() + 4);
    println!(
        "\n  {DIM}\u{250c}\u{2500} {title} {}\u{2510}{RESET}",
        "\u{2500}".repeat(top_pad)
    );
    for line in text.lines() {
        let vis = visible_len(line);
        let pad = width.saturating_sub(vis).saturating_sub(1);
        println!(
            "  {DIM}\u{2502}{RESET} {DIM}{}{}{DIM}\u{2502}{RESET}",
            line,
            " ".repeat(pad)
        );
    }
    println!(
        "  {DIM}\u{2514}{}\u{2518}{RESET}",
        "\u{2500}".repeat(width)
    );
}

/// Print the user prompt indicator.
pub fn print_prompt(_app: &App) {
    print!("  {CORAL}{BOLD}> {RESET}");
    io::stdout().flush().unwrap();
}

// ─── Session Summary ─────────────────────────────────────────────────────────

/// Print a compact session summary after each exchange:
///  model-name │ N msgs │ X.Xs
pub fn print_session_summary(app: &App) {
    let msg_count = app.messages.len();
    let elapsed = app.start_time.elapsed().as_secs_f64();
    let elapsed_str = if elapsed >= 60.0 {
        format!("{}m {:.0}s", (elapsed / 60.0) as u64, elapsed % 60.0)
    } else {
        format!("{:.1}s", elapsed)
    };
    println!(
        "\n  {DIM} {BRIGHT_CYAN}{}{RESET}{DIM} \u{2502} {} msgs \u{2502} {}{RESET}",
        app.model, msg_count, elapsed_str
    );
}

// ─── Exit Summary (Hermes-style) ─────────────────────────────────────────────

/// Print the exit summary when the session ends.
pub fn print_exit_summary(app: &App) {
    let duration = app.start_time.elapsed();
    let total_secs = duration.as_secs();
    let minutes = total_secs / 60;
    let seconds = total_secs % 60;
    let duration_str = format!("{}m {}s", minutes, seconds);

    let total_msgs = app.messages.len();
    let user_msgs = app.messages.iter().filter(|m| m.role == MessageRole::User).count();
    let tool_calls: usize = app.messages.iter().map(|m| m.tool_calls.len()).sum();

    println!();
    println!("  {DIM}Session: {}{RESET}", app.session_name);
    println!("  {DIM}Duration: {}{RESET}", duration_str);
    println!(
        "  {DIM}Messages: {} ({} user, {} tool calls){RESET}",
        total_msgs, user_msgs, tool_calls
    );
    println!();
}

// ─── Markdown Rendering ──────────────────────────────────────────────────────

/// Calculate the visible length of a string (ignoring ANSI escape sequences).
fn visible_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for ch in s.chars() {
        if ch == '\x1b' {
            in_escape = true;
            continue;
        }
        if in_escape {
            if ch == 'm' {
                in_escape = false;
            }
            continue;
        }
        len += 1;
    }
    len
}

/// Render markdown text with ANSI formatting
#[allow(unused_assignments)]
pub fn render_markdown(text: &str, indent: usize) {
    let indent_str = " ".repeat(indent);
    let mut in_code_block = false;
    let mut code_lang = String::new();

    for line in text.lines() {
        // Code block toggle
        if line.starts_with("```") {
            if in_code_block {
                in_code_block = false;
                println!();
            } else {
                in_code_block = true;
                code_lang = line[3..].trim().to_string();
                if !code_lang.is_empty() {
                    println!("{indent_str}{DIM}  | {code_lang}{RESET}");
                }
            }
            continue;
        }

        if in_code_block {
            println!("{indent_str}{DIM}  | {line}{RESET}");
            continue;
        }

        // Headings
        if line.starts_with("### ") {
            println!("{indent_str}{CORAL}{BOLD}{}{RESET}", &line[4..]);
            continue;
        }
        if line.starts_with("## ") {
            println!("{indent_str}{WHITE}{BOLD}{}{RESET}", &line[3..]);
            continue;
        }
        if line.starts_with("# ") {
            println!("{indent_str}{WHITE}{BOLD}{}{RESET}", &line[2..]);
            continue;
        }

        // Horizontal rule
        if line.trim() == "---" || line.trim() == "***" {
            println!("{indent_str}{DIM}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}{RESET}");
            continue;
        }

        // Blockquote
        if line.starts_with("> ") {
            println!("{indent_str}{DIM}  | {}{RESET}", &line[2..]);
            continue;
        }

        // Unordered list
        if line.starts_with("- ") || line.starts_with("* ") {
            let content = &line[2..];
            if line.starts_with("  ") {
                println!("{indent_str}{DIM}    o {}{RESET}", render_inline(content));
            } else {
                println!("{indent_str}{DIM}  * {}{RESET}", render_inline(content));
            }
            continue;
        }

        // Ordered list
        if line.len() > 2 && line.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            if let Some(pos) = line.find(". ") {
                let num = &line[..pos];
                let content = &line[pos + 2..];
                println!("{indent_str}{DIM}  {num}. {}{RESET}", render_inline(content));
                continue;
            }
        }

        // Regular text
        println!("{indent_str}{}", render_inline(line));
    }
}

/// Render inline markdown formatting
fn render_inline(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '*' {
            // Bold or italic
            if chars.peek() == Some(&'*') {
                chars.next(); // consume second *
                // Bold
                let mut bold_text = String::new();
                loop {
                    match chars.next() {
                        Some('*') if chars.peek() == Some(&'*') => {
                            chars.next();
                            break;
                        }
                        Some(c) => bold_text.push(c),
                        None => break,
                    }
                }
                result.push_str(&format!("{BOLD}{bold_text}{RESET}"));
            } else {
                // Italic
                let mut italic_text = String::new();
                loop {
                    match chars.next() {
                        Some('*') => break,
                        Some(c) => italic_text.push(c),
                        None => break,
                    }
                }
                result.push_str(&format!("{ITALIC}{italic_text}{RESET}"));
            }
        } else if ch == '`' {
            // Inline code
            let mut code_text = String::new();
            loop {
                match chars.next() {
                    Some('`') => break,
                    Some(c) => code_text.push(c),
                    None => break,
                }
            }
            result.push_str(&format!("{YELLOW}{code_text}{RESET}"));
        } else if ch == '~' && chars.peek() == Some(&'~') {
            chars.next(); // consume second ~
            // Strikethrough
            let mut strike_text = String::new();
            loop {
                match chars.next() {
                    Some('~') if chars.peek() == Some(&'~') => {
                        chars.next();
                        break;
                    }
                    Some(c) => strike_text.push(c),
                    None => break,
                }
            }
            result.push_str(&format!("{STRIKETHROUGH}{strike_text}{RESET}"));
        } else if ch == '[' {
            // Link [text](url)
            let mut link_text = String::new();
            loop {
                match chars.next() {
                    Some(']') => break,
                    Some(c) => link_text.push(c),
                    None => break,
                }
            }
            if chars.peek() == Some(&'(') {
                chars.next(); // consume (
                let mut url = String::new();
                loop {
                    match chars.next() {
                        Some(')') => break,
                        Some(c) => url.push(c),
                        None => break,
                    }
                }
                result.push_str(&format!("{UNDERLINE}{link_text}{RESET}{DIM} ({url}){RESET}"));
            } else {
                result.push('[');
                result.push_str(&link_text);
            }
        } else {
            result.push(ch);
        }
    }

    result
}

// ─── Input Handling ──────────────────────────────────────────────────────────

/// Read a line of input from the user
pub fn read_input() -> io::Result<String> {
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

/// Read a password (masked input)
pub fn read_password() -> io::Result<String> {
    read_input()
}

// ─── Help & Info ─────────────────────────────────────────────────────────────

/// Print the help menu
pub fn print_help() {
    println!();
    println!("  {BOLD}{WHITE}Commands:{RESET}");
    println!("  {CORAL}/help{RESET}              Show this help");
    println!("  {CORAL}/style{RESET} <name>     Switch response style (concise/verbose/technical/non-technical/<persona>)");
    println!("  {CORAL}/session{RESET} list     List sessions");
    println!("  {CORAL}/session{RESET} new      New session");
    println!("  {CORAL}/session{RESET} resume   Resume session");
    println!("  {CORAL}/session{RESET} delete   Delete session");
    println!("  {CORAL}/clear{RESET}            Clear conversation");
    println!("  {CORAL}/status{RESET}           Show agent status");
    println!("  {CORAL}/config{RESET}           Show/edit config");
    println!("  {CORAL}/profile{RESET}          Show/edit profile");
    println!("  {CORAL}/quit{RESET}             Exit");
    println!();
}

/// Print the status info
pub fn print_status_info(app: &App) {
    println!();
    println!("  {BOLD}{WHITE}Status:{RESET}");
    println!("  {DIM}Model:    {CYAN}{}{RESET}", app.model);
    println!("  {DIM}Provider: {}{RESET}", app.provider);
    println!("  {DIM}Session:  {}{RESET}", app.session_name);
    println!("  {DIM}Style:    {}{RESET}", app.response_style.name());
    println!("  {DIM}Messages: {}{RESET}", app.messages.len());
    println!();
}

/// Print the profile
pub fn print_profile(profile: &UserProfile) {
    println!();
    println!("  {BOLD}{WHITE}Profile:{RESET}");
    if !profile.name.is_empty() {
        println!("  {DIM}Name:    {}{RESET}", profile.name);
    }
    if !profile.role.is_empty() {
        println!("  {DIM}Role:    {}{RESET}", profile.role);
    }
    if !profile.about.is_empty() {
        println!("  {DIM}About:   {}{RESET}", profile.about);
    }
    if !profile.persona.is_empty() {
        println!("  {DIM}Persona: {}{RESET}", profile.persona);
    }
    println!("  {DIM}Style:   {}{RESET}", profile.style.name());
    println!();
}

/// Print the approval modal
pub fn print_approval(approval: &ApprovalRequest) {
    let risk_color = match approval.risk_level.as_str() {
        "Safe" => GREEN,
        "Mutating" => YELLOW,
        "Destructive" => RED,
        "Blocked" => RED,
        _ => WHITE,
    };

    println!();
    println!("  {YELLOW}{BOLD}\u{256d}\u{2500} Tool Approval Required {}\u{256e}{RESET}", "\u{2500}".repeat(35));
    println!("  {YELLOW}\u{2502}{RESET}                                                   {YELLOW}\u{2502}{RESET}");
    println!("  {YELLOW}\u{2502}{RESET}  Tool: {BOLD}{}{RESET}", approval.tool_name);
    println!("  {YELLOW}\u{2502}{RESET}  Risk: {risk_color}{}{RESET}", approval.risk_level);
    println!("  {YELLOW}\u{2502}{RESET}                                                   {YELLOW}\u{2502}{RESET}");
    println!("  {YELLOW}\u{2502}{RESET}  {}", approval.description);
    println!("  {YELLOW}\u{2502}{RESET}                                                   {YELLOW}\u{2502}{RESET}");
    println!("  {YELLOW}\u{2502}{RESET}  Input:");
    for line in approval.input_preview.lines().take(10) {
        println!("  {YELLOW}\u{2502}{RESET}  {CYAN}{line}{RESET}");
    }
    println!("  {YELLOW}\u{2502}{RESET}                                                   {YELLOW}\u{2502}{RESET}");
    println!("  {YELLOW}\u{2502}{RESET}  {BOLD}[y]{RESET}es  {BOLD}[n]{RESET}o  {BOLD}[a]{RESET}lways  {BOLD}[v]{RESET}iew details  {DIM}[Esc]{RESET} cancel");
    println!("  {YELLOW}\u{2570}{}\u{256f}{RESET}", "\u{2500}".repeat(51));
    println!();
}
