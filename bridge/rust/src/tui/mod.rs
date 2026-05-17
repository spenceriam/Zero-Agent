use std::io::{self, Write};
use std::time::{Duration, Instant};

// ANSI color codes
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const ITALIC: &str = "\x1b[3m";
const UNDERLINE: &str = "\x1b[4m";
const STRIKETHROUGH: &str = "\x1b[9m";
const BLACK: &str = "\x1b[30m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const MAGENTA: &str = "\x1b[35m";
const CYAN: &str = "\x1b[36m";
const WHITE: &str = "\x1b[37m";
const BRIGHT_BLACK: &str = "\x1b[90m";
const BRIGHT_GREEN: &str = "\x1b[92m";
const BRIGHT_YELLOW: &str = "\x1b[93m";
const BRIGHT_CYAN: &str = "\x1b[96m";

// Coral color (true color)
const CORAL: &str = "\x1b[38;2;255;127;80m";

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
    pub messages: Vec<Message>,
    pub activity_log: Vec<ActivityEntry>,
    pub input: String,
    pub input_cursor: usize,
    pub input_history: Vec<String>,
    pub history_index: Option<usize>,
    pub show_activity: bool,
    pub response_style: ResponseStyle,
    pub model: String,
    pub session_name: String,
    pub token_count: usize,
    pub is_streaming: bool,
    pub current_stream: String,
    pub stream_line_buffer: String,
    pub stream_is_first_line: bool,
    pub thinking_text: String,
    pub thinking_visible: bool,
    pub approval: Option<ApprovalRequest>,
    pub should_quit: bool,
    pub profile: UserProfile,
    pub terminal_width: usize,
    pub thinking_start: Option<Instant>,
}

impl App {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            activity_log: Vec::new(),
            input: String::new(),
            input_cursor: 0,
            input_history: Vec::new(),
            history_index: None,
            show_activity: true,
            response_style: ResponseStyle::Concise,
            model: String::from("unknown"),
            session_name: String::from("main"),
            token_count: 0,
            is_streaming: false,
            current_stream: String::new(),
            stream_line_buffer: String::new(),
            stream_is_first_line: true,
            thinking_text: String::new(),
            thinking_visible: false,
            approval: None,
            should_quit: false,
            profile: UserProfile::new(),
            terminal_width: 80,
            thinking_start: None,
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

// ─── Rendering ───────────────────────────────────────────────────────────────

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
    // Default to 80x24 if we can't detect
    (80, 24)
}

/// Print the welcome banner
pub fn print_banner() {
    println!();
    println!("  {BOLD}{WHITE}██    ██  ███████  ██████   ██████  {CORAL}-{WHITE}  █████  ██████  ███████ ███    ██ ████████ {RESET}");
    println!("  {BOLD}{WHITE}██    ██  ██      ██    ██ ██    ██ {CORAL}-{WHITE} ██   ██ ██   ██ ██      ████   ██    ██    {RESET}");
    println!("  {BOLD}{WHITE}██    ██  █████   ██    ██ ██    ██ {CORAL}-{WHITE} ███████ ██████  █████   ██ ██  ██    ██    {RESET}");
    println!("  {BOLD}{WHITE} ██  ██   ██      ██    ██ ██    ██ {CORAL}-{WHITE} ██   ██ ██   ██ ██      ██  ██ ██    ██    {RESET}");
    println!("  {BOLD}{WHITE}  ████    ███████  ██████   ██████  {CORAL}-{WHITE} ██   ██ ██   ██ ███████ ██   ████    ██    {RESET}");
    println!();
    println!("  {DIM}personal AI assistant for developers{RESET}");
    println!();
}

/// Print the status line
pub fn print_status(app: &App) {
    let style_name = app.response_style.name();
    println!("  {DIM}model: {CYAN}{}{RESET}{DIM} | tokens: {} | session: {} | style: {}{RESET}",
        app.model, app.token_count, app.session_name, style_name);
    println!("  {DIM}/help for commands · Tab to toggle activity · Ctrl+C to quit{RESET}");
    println!();
}

/// Print the user prompt
pub fn print_prompt(app: &App) {
    let name = if app.profile.name.is_empty() {
        "YOU".to_string()
    } else {
        app.profile.name.to_uppercase()
    };
    print!("  {CORAL}{BOLD}> {RESET}");
    io::stdout().flush().unwrap();
}

/// Print a user message
pub fn print_user_message(text: &str) {
    let name = "YOU";
    println!("  {CORAL}{BOLD}{name:10}{RESET}{text}");
    println!();
}

/// Print an agent message with markdown rendering
pub fn print_agent_message(text: &str) {
    print!("  {CYAN}{BOLD}MAX     {RESET}");
    render_markdown(text, 10);
    println!();
}

/// Print a system message
pub fn print_system_message(text: &str) {
    println!("  {DIM}SYS     {text}{RESET}");
    println!();
}

/// Print a tool call in progress
pub fn print_tool_call(name: &str, status: &ToolStatus) {
    let (icon, color) = match status {
        ToolStatus::Running => ("⏳", YELLOW),
        ToolStatus::Success => ("✓", GREEN),
        ToolStatus::Error => ("✗", RED),
    };
    println!("  {DIM}  {color}{icon} {name}{RESET}");
}

/// Print tool completion with elapsed time (Hermes-style)
pub fn print_tool_completion(name: &str, status: &ToolStatus, elapsed: Duration) {
    let (icon, color) = match status {
        ToolStatus::Running => ("⏳", YELLOW),
        ToolStatus::Success => ("✓", GREEN),
        ToolStatus::Error => ("✗", RED),
    };
    let verb = match name {
        "read_file" => "read",
        "write_file" => "write",
        "edit_file" => "edit",
        "shell" => "run",
        "glob" => "search",
        _ => name,
    };
    println!("  {DIM}  {color}{icon} {verb:9}{RESET}{DIM}  {:.1}s{RESET}", elapsed.as_secs_f64());
}

/// Print tool output (truncated)
pub fn print_tool_output(output: &str, max_lines: usize) {
    let lines: Vec<&str> = output.lines().collect();
    let shown = lines.len().min(max_lines);
    for line in &lines[..shown] {
        println!("  {DIM}    | {line}{RESET}");
    }
    if lines.len() > shown {
        println!("  {DIM}    | ... ({} more lines){RESET}", lines.len() - shown);
    }
}

/// Print a file diff
pub fn print_diff(old: &str, new: &str, filename: &str) {
    println!("  {DIM}  ── diff {filename} ──{RESET}");
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    // Simple line-by-line diff
    let max_len = old_lines.len().max(new_lines.len());
    for i in 0..max_len {
        let old_line = old_lines.get(i).unwrap_or(&"");
        let new_line = new_lines.get(i).unwrap_or(&"");

        if old_line != new_line {
            if !old_line.is_empty() {
                println!("  {DIM}    {RED}- {old_line}{RESET}");
            }
            if !new_line.is_empty() {
                println!("  {DIM}    {GREEN}+ {new_line}{RESET}");
            }
        } else {
            println!("  {DIM}      {old_line}{RESET}");
        }
    }
}

/// Print thinking indicator
pub fn print_thinking(start: Instant) {
    let elapsed = start.elapsed().as_millis() / 400;
    let dots = ".".repeat((elapsed % 4) as usize + 1);
    print!("\r  {DIM}💭 thinking{dots:4}{RESET}");
    io::stdout().flush().unwrap();
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
    println!("  {YELLOW}{BOLD}╭─ Tool Approval Required ─────────────────────────╮{RESET}");
    println!("  {YELLOW}│{RESET}                                                   {YELLOW}│{RESET}");
    println!("  {YELLOW}│{RESET}  Tool: {BOLD}{}{RESET}", approval.tool_name);
    println!("  {YELLOW}│{RESET}  Risk: {risk_color}{}{RESET}", approval.risk_level);
    println!("  {YELLOW}│{RESET}                                                   {YELLOW}│{RESET}");
    println!("  {YELLOW}│{RESET}  {}", approval.description);
    println!("  {YELLOW}│{RESET}                                                   {YELLOW}│{RESET}");
    println!("  {YELLOW}│{RESET}  Input:");
    for line in approval.input_preview.lines().take(10) {
        println!("  {YELLOW}│{RESET}  {CYAN}{line}{RESET}");
    }
    println!("  {YELLOW}│{RESET}                                                   {YELLOW}│{RESET}");
    println!("  {YELLOW}│{RESET}  {BOLD}[y]{RESET}es  {BOLD}[n]{RESET}o  {BOLD}[a]{RESET}lways  {BOLD}[v]{RESET}iew details  {DIM}[Esc]{RESET} cancel");
    println!("  {YELLOW}╰───────────────────────────────────────────────────╯{RESET}");
    println!();
}

/// Print the activity pane
pub fn print_activity(app: &App) {
    if !app.show_activity {
        return;
    }

    println!("  {DIM}── Activity ──────────────────────────────────────{RESET}");
    let start = app.activity_log.len().saturating_sub(10);
    for entry in app.activity_log.iter().skip(start) {
        let (icon, color) = match entry.status {
            ToolStatus::Running => ("⏳", YELLOW),
            ToolStatus::Success => ("✓", GREEN),
            ToolStatus::Error => ("✗", RED),
        };
        let elapsed_str = entry.elapsed
            .map(|d| format!(" {:.1}s", d.as_secs_f64()))
            .unwrap_or_default();
        println!("  {DIM}  {color}{icon} {}{elapsed_str}{RESET}", entry.tool_name);
    }
    if app.activity_log.is_empty() {
        println!("  {DIM}  No activity yet{RESET}");
    }
    println!();
}

// ─── Markdown Rendering ──────────────────────────────────────────────────────

/// Render markdown text with ANSI formatting
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
            println!("{indent_str}{DIM}────────────────────────────────────────────────{RESET}");
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
        if line.len() > 2 && line.chars().nth(0).map_or(false, |c| c.is_ascii_digit()) {
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
    // For now, just read normally (masking requires raw terminal mode)
    read_input()
}

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
    println!("  {DIM}Model:   {CYAN}{}{RESET}", app.model);
    println!("  {DIM}Session: {}{RESET}", app.session_name);
    println!("  {DIM}Style:   {}{RESET}", app.response_style.name());
    println!("  {DIM}Tokens:  {}{RESET}", app.token_count);
    println!("  {DIM}Messages:{}{RESET}", app.messages.len());
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

// ─── Streaming ───────────────────────────────────────────────────────────────

/// Process a streaming chunk
pub fn process_stream_chunk(app: &mut App, chunk: &str) {
    for ch in chunk.chars() {
        if ch == '\n' {
            // Line complete - print with formatting
            if app.stream_is_first_line {
                print!("  {CYAN}{BOLD}MAX     {RESET}");
                app.stream_is_first_line = false;
            } else {
                print!("  {DIM}        {RESET}");
            }
            render_markdown(&app.stream_line_buffer, 10);
            app.stream_line_buffer.clear();
        } else {
            app.stream_line_buffer.push(ch);
            // Print raw character for immediate feedback
            print!("{ch}");
            io::stdout().flush().unwrap();
        }
    }
}

/// Finish streaming
pub fn finish_streaming(app: &mut App) {
    if !app.stream_line_buffer.is_empty() {
        if app.stream_is_first_line {
            print!("  {CYAN}{BOLD}MAX     {RESET}");
        } else {
            print!("  {DIM}        {RESET}");
        }
        render_markdown(&app.stream_line_buffer, 10);
        app.stream_line_buffer.clear();
    }
    app.is_streaming = false;
    app.stream_is_first_line = true;
    println!();
}
