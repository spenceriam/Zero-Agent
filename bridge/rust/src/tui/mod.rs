use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// ─── ANSI Color Codes ─────────────────────────────────────────────────────────
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
const BRIGHT_WHITE: &str = "\x1b[97m";
const BRIGHT_CYAN: &str = "\x1b[96m";

// Coral / accent color (true color)
const CORAL: &str = "\x1b[38;2;255;127;80m";

// Dimmed thinking text color (dark gray)
const THINKING_TEXT: &str = "\x1b[38;5;240m";
// Accent for "Thinking:" title
const THINKING_TITLE: &str = "\x1b[38;5;67m";

// Shimmer frames: cycles dim → white → bright_white → white → dim
const SHIMMER_FRAMES: &[&str] = &[
    "\x1b[38;5;240m",
    "\x1b[38;5;243m",
    "\x1b[38;5;246m",
    "\x1b[38;5;249m",
    "\x1b[38;5;252m",
    "\x1b[38;5;255m",
    "\x1b[38;5;252m",
    "\x1b[38;5;249m",
    "\x1b[38;5;246m",
    "\x1b[38;5;243m",
];
const SHIMMER_INTERVAL_MS: u64 = 80;

// Max lines before truncation
const THINKING_MAX_LINES: usize = 100;
const TOOL_RESULT_MAX_LINES: usize = 50;

// ─── Status Mode ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq)]
pub enum StatusMode {
    Idle,
    Flowing,
    Executing(String), // tool name
    Interrupted,
}

// ─── Approval Choice ──────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalChoice {
    Deny,
    ApproveOnce,
    ApproveSession,
    ApproveAlways,
}

// ─── Approval Modal State ─────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct ApprovalModal {
    pub tool_name: String,
    pub command: String,
    pub risk_level: String,
    pub selected: usize, // 0=Deny 1=Once 2=Session 3=Always
}

// ─── Slash Command ────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
}

// All commands alphabetically
pub fn all_slash_commands() -> Vec<SlashCommand> {
    vec![
        SlashCommand { name: "clear".into(),     description: "Clear the current conversation scroll".into() },
        SlashCommand { name: "config".into(),    description: "Show or edit agent configuration".into() },
        SlashCommand { name: "help".into(),      description: "Show all available commands".into() },
        SlashCommand { name: "jobs".into(),      description: "List or cancel background jobs".into() },
        SlashCommand { name: "memory".into(),    description: "Show or manage saved memories".into() },
        SlashCommand { name: "model".into(),     description: "Switch the active model (interactive picker)".into() },
        SlashCommand { name: "profile".into(),   description: "Show or edit your user profile".into() },
        SlashCommand { name: "provider".into(),  description: "List providers or change provider".into() },
        SlashCommand { name: "quit".into(),      description: "Exit Zero-Agent".into() },
        SlashCommand { name: "reasoning".into(), description: "Set reasoning: off | low | med | high | x-high".into() },
        SlashCommand { name: "session".into(),   description: "Manage sessions: list | new | resume | delete".into() },
        SlashCommand { name: "status".into(),    description: "Show model, provider, context, and session info".into() },
        SlashCommand { name: "stop".into(),      description: "Stop the current agent turn".into() },
        SlashCommand { name: "style".into(),     description: "Set style: concise | verbose | technical".into() },
        SlashCommand { name: "tools".into(),     description: "List or toggle tool permissions".into() },
    ]
}

pub fn filter_slash_commands(prefix: &str) -> Vec<SlashCommand> {
    let p = prefix.to_lowercase();
    all_slash_commands()
        .into_iter()
        .filter(|c| c.name.starts_with(&p))
        .collect()
}

// ─── Response Style ───────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq)]
pub enum ResponseStyle {
    Concise,
    Verbose,
    Technical,
    Persona(String),
}

impl ResponseStyle {
    pub fn name(&self) -> &str {
        match self {
            ResponseStyle::Concise => "concise",
            ResponseStyle::Verbose => "verbose",
            ResponseStyle::Technical => "technical",
            ResponseStyle::Persona(n) => n,
        }
    }
}

// ─── Message Role ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

// ─── Tool Status ──────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq)]
pub enum ToolStatus {
    Running,
    Success,
    Error,
}

// ─── Tool Call ────────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub input: String,
    pub output: Option<String>,
    pub status: ToolStatus,
    pub elapsed: Option<Duration>,
    pub start_time: Instant,
}

// ─── Message ─────────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub thinking: Option<String>,
    pub timestamp: Instant,
}

// ─── User Profile ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct UserProfile {
    pub name: String,
    pub role: String,
    pub about: String,
    pub style: ResponseStyle,
}

impl UserProfile {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            role: String::new(),
            about: String::new(),
            style: ResponseStyle::Concise,
        }
    }
}

// ─── App State ────────────────────────────────────────────────────────────────
pub struct App {
    pub model: String,
    pub provider: String,
    pub reasoning_label: String, // "" means off/no reasoning
    pub session_name: String,
    pub session_id: String,
    pub messages: Vec<Message>,
    pub start_time: Instant,
    pub terminal_width: usize,
    pub response_style: ResponseStyle,
    pub profile: UserProfile,
    pub input_history: Vec<String>,
    pub history_index: Option<usize>,
    pub context_pct: usize,
    pub status_mode: StatusMode,
}

impl App {
    pub fn new() -> Self {
        Self {
            model: String::from("unknown"),
            provider: String::from("unknown"),
            reasoning_label: String::new(),
            session_name: String::from("main"),
            session_id: String::new(),
            messages: Vec::new(),
            start_time: Instant::now(),
            terminal_width: 90,
            response_style: ResponseStyle::Concise,
            profile: UserProfile::new(),
            input_history: Vec::new(),
            history_index: None,
            context_pct: 0,
            status_mode: StatusMode::Idle,
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
        if let Some(msg) = self.messages.last_mut() {
            if let Some(tc) = msg.tool_calls.iter_mut().rev().find(|t| t.name == name) {
                tc.elapsed = Some(tc.start_time.elapsed());
                tc.status = status;
                tc.output = output;
            }
        }
    }

    /// Model display string for header: "claude-3.5-sonnet (high)" or "claude-3.5-sonnet"
    pub fn model_display(&self) -> String {
        if self.reasoning_label.is_empty() {
            self.model.clone()
        } else {
            format!("{} ({})", self.model, self.reasoning_label)
        }
    }
}

// ─── ANSI Helpers ─────────────────────────────────────────────────────────────
pub fn clear_screen() {
    print!("\x1b[2J\x1b[H");
}

pub fn clear_line() {
    print!("\x1b[K");
}

pub fn move_cursor(row: u16, col: u16) {
    print!("\x1b[{};{}H", row, col);
}

pub fn get_terminal_size() -> (usize, usize) {
    // Default; real implementation would use termios/ioctl
    (100, 30)
}

/// Calculate visible length of string (strips ANSI escape codes)
pub fn visible_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for ch in s.chars() {
        if ch == '\x1b' { in_escape = true; continue; }
        if in_escape { if ch == 'm' { in_escape = false; } continue; }
        len += 1;
    }
    len
}

// ─── Header Bar ──────────────────────────────────────────────────────────────
/// Prints the persistent header bar at the top of the session.
/// Format: Zero-Agent  provider: X  model: Y (reasoning)  session: Z  context: N%
pub fn print_header(app: &App) {
    let (width, _) = get_terminal_size();
    let ctx = if app.context_pct > 0 {
        format!("  context: {}%", app.context_pct)
    } else {
        String::new()
    };
    let line = format!(
        " Zero-Agent  provider: {}  model: {}  session: {}{}",
        app.provider,
        app.model_display(),
        app.session_name,
        ctx
    );
    println!("{BOLD}{BRIGHT_CYAN}{}{RESET}", line);
    println!("{DIM}{}{RESET}", "\u{2500}".repeat(width));
}

// ─── Shimmering Status Line ───────────────────────────────────────────────────
/// Format elapsed time as "0ms", "1.2s", "2m 4s"
pub fn format_elapsed(elapsed: Duration) -> String {
    let ms = elapsed.as_millis();
    if ms < 1000 {
        format!("{}ms", ms)
    } else if elapsed.as_secs() < 60 {
        format!("{:.1}s", elapsed.as_secs_f64())
    } else {
        let m = elapsed.as_secs() / 60;
        let s = elapsed.as_secs() % 60;
        format!("{}m {}s", m, s)
    }
}

/// Start the shimmering status line in a background thread.
/// The status line sits fixed above the prompt box.
/// Returns (JoinHandle, stop_flag, tool_name_arc).
pub fn start_shimmer_status(
    initial_mode: StatusMode,
) -> (thread::JoinHandle<()>, Arc<AtomicBool>, Arc<std::sync::Mutex<StatusMode>>) {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    let mode_arc = Arc::new(std::sync::Mutex::new(initial_mode));
    let mode_clone = Arc::clone(&mode_arc);

    let handle = thread::spawn(move || {
        let start = Instant::now();
        let mut frame_idx: usize = 0;
        while !stop_clone.load(Ordering::Relaxed) {
            let elapsed = start.elapsed();
            let elapsed_str = format_elapsed(elapsed);
            let color = SHIMMER_FRAMES[frame_idx % SHIMMER_FRAMES.len()];

            let mode = mode_clone.lock().unwrap().clone();
            let label = match &mode {
                StatusMode::Flowing => "Flowing...".to_string(),
                StatusMode::Executing(tool) => format!("Executing: {}...", tool),
                StatusMode::Interrupted => "Interrupted".to_string(),
                StatusMode::Idle => String::new(),
            };

            if !label.is_empty() {
                print!("\r  {}{}{RESET}  {DIM}⎋ to interrupt  {}{RESET}   ",
                    color, label, elapsed_str);
                io::stdout().flush().unwrap();
            }

            frame_idx = frame_idx.wrapping_add(1);
            thread::sleep(Duration::from_millis(SHIMMER_INTERVAL_MS));
        }
    });

    (handle, stop, mode_arc)
}

/// Stop the shimmer and clear the status line.
pub fn stop_shimmer(
    handle: thread::JoinHandle<()>,
    stop: Arc<AtomicBool>,
) {
    stop.store(true, Ordering::Relaxed);
    let _ = handle.join();
    print!("\r");
    clear_line();
    io::stdout().flush().unwrap();
}

/// Print a static (non-animated) stopped/interrupted status.
pub fn print_status_stopped() {
    println!("\r  {DIM}Stopped{RESET}");
}

// ─── Scroll Blocks ────────────────────────────────────────────────────────────

/// Print a user message block.
/// ┌─ You ────────────────────────────────────────────────────┐
/// │ message text                                             │
/// └──────────────────────────────────────────────────────────┘
pub fn print_user_block(text: &str, width: usize) {
    let inner = width.saturating_sub(4);
    let title = "You";
    let dashes = inner.saturating_sub(title.len() + 2);
    println!("\n  {CORAL}{BOLD}\u{250c}\u{2500} {title} {}{RESET}",
        "\u{2500}".repeat(dashes));
    for line in text.lines() {
        let vlen = visible_len(line);
        let pad = inner.saturating_sub(vlen);
        println!("  {CORAL}\u{2502}{RESET} {}{} {CORAL}\u{2502}{RESET}", line, " ".repeat(pad));
    }
    println!("  {CORAL}{BOLD}\u{2514}{}{RESET}", "\u{2500}".repeat(inner + 2));
}

/// Print a streaming agent text block — no box, just labeled lines.
/// ZERO  text flows here with inline markdown rendering
pub fn print_agent_text(text: &str) {
    println!();
    // Print the label on a fresh line
    print!("  {CYAN}{BOLD}ZERO{RESET}  ");
    let mut first = true;
    for line in text.lines() {
        if first {
            println!("{}", render_inline(line));
            first = false;
        } else {
            println!("        {}", render_inline(line));
        }
    }
    if first {
        println!(); // empty text, just newline
    }
}

/// Print a thinking block (dimmed, accent title, truncated).
pub fn print_thinking_block(content: &str, log_path: &str) {
    let lines: Vec<&str> = content.lines().collect();
    let truncated = lines.len() > THINKING_MAX_LINES;
    let display_lines = if truncated { &lines[..THINKING_MAX_LINES] } else { &lines[..] };

    println!();
    println!("  {THINKING_TITLE}{BOLD}Thinking:{RESET}");
    for line in display_lines {
        println!("  {THINKING_TEXT}  {}{RESET}", line);
    }
    if truncated && !log_path.is_empty() {
        println!("  {THINKING_TEXT}  ... [{} lines truncated]{RESET}", lines.len() - THINKING_MAX_LINES);
        println!("  {DIM}  See full trace → {UNDERLINE}{}{RESET}", log_path);
    }
}

/// Print a tool call line.
/// ┆ 📄 read_file  `src/main.0`  (0.4s)
pub fn print_tool_call(name: &str, args_preview: &str, status: &ToolStatus, elapsed: Option<Duration>) {
    let icon = tool_icon(name);
    let (color, marker) = match status {
        ToolStatus::Running  => (YELLOW, "\u{25cf}"), // ●
        ToolStatus::Success  => (GREEN,  "\u{25cf}"),
        ToolStatus::Error    => (RED,    "\u{25cf}"),
    };
    let elapsed_str = elapsed
        .map(|d| format!("  ({})", format_elapsed(d)))
        .unwrap_or_default();
    let preview = if args_preview.len() > 50 {
        format!("{}...", &args_preview[..47])
    } else {
        args_preview.to_string()
    };
    println!("  {DIM}\u{250a}{RESET}  {color}{marker} {BOLD}{name}{RESET}  {DIM}`{preview}`{elapsed_str}{RESET}");
}

/// Print a tool result (dimmed, truncated if large).
pub fn print_tool_result(name: &str, output: &str, log_path: &str) {
    let lines: Vec<&str> = output.lines().collect();
    let truncated = lines.len() > TOOL_RESULT_MAX_LINES;
    let display = if truncated { &lines[..TOOL_RESULT_MAX_LINES] } else { &lines[..] };

    for line in display {
        println!("  {DIM}  \u{2502} {}{RESET}", line);
    }
    if truncated && !log_path.is_empty() {
        println!("  {DIM}  \u{2502} ... [{} lines] → {UNDERLINE}{}{RESET}", lines.len(), log_path);
    }
}

/// Print a memory footer line at end of turn.
/// ↳ Memory: prefer concise responses | project.md
pub fn print_memory_footer(description: &str, file_name: &str) {
    let desc = if description.len() > 60 {
        format!("{}...", &description[..57])
    } else {
        description.to_string()
    };
    println!("  {DIM}\u{21b3} Memory: {} | {}{RESET}", desc, file_name);
}

/// Print a system note (dim, no border).
pub fn print_system_note(text: &str) {
    println!("  {DIM}{ITALIC}{}{RESET}", text);
}

// ─── Approval Modal ───────────────────────────────────────────────────────────
/// Print the full-focus approval modal overlay.
/// Clears line area and renders a centered card.
pub fn print_approval_modal(modal: &ApprovalModal) {
    let (width, _) = get_terminal_size();
    let w = width.min(62);
    let pad = (width.saturating_sub(w)) / 2;
    let sp = " ".repeat(pad);

    let risk_color = match modal.risk_level.as_str() {
        "Safe"        => GREEN,
        "Mutating"    => YELLOW,
        "Destructive" => RED,
        _             => RED,
    };

    println!();
    println!("{sp}{YELLOW}{BOLD}\u{250c}{}{BOLD}\u{2510}{RESET}", "\u{2500}".repeat(w - 2));
    println!("{sp}{YELLOW}\u{2502}{RESET}{BOLD}{RED}  \u{26a0}  Action Required{RESET}{}  {YELLOW}\u{2502}{RESET}", " ".repeat(w - 20));
    println!("{sp}{YELLOW}\u{2502}{RESET}  Tool: {BOLD}{}{RESET}{}  {YELLOW}\u{2502}{RESET}",
        modal.tool_name, " ".repeat(w.saturating_sub(modal.tool_name.len() + 11)));
    if !modal.command.is_empty() {
        let cmd_preview = if modal.command.len() > w - 10 {
            format!("{}...", &modal.command[..w.saturating_sub(13)])
        } else {
            modal.command.clone()
        };
        println!("{sp}{YELLOW}\u{2502}{RESET}  Command: {CYAN}{}{RESET}{}  {YELLOW}\u{2502}{RESET}",
            cmd_preview, " ".repeat(w.saturating_sub(cmd_preview.len() + 13)));
    }
    println!("{sp}{YELLOW}\u{2502}{RESET}  Risk:  {risk_color}{BOLD}{}{RESET}{}  {YELLOW}\u{2502}{RESET}",
        modal.risk_level, " ".repeat(w.saturating_sub(modal.risk_level.len() + 12)));
    println!("{sp}{YELLOW}\u{2502}{RESET}{}  {YELLOW}\u{2502}{RESET}", " ".repeat(w - 4));

    // Options
    let options = ["Deny", "Approve Once", "Approve for Session", "Approve Always"];
    let keys    = ["D", "O", "S", "A"];
    for (i, (opt, key)) in options.iter().zip(keys.iter()).enumerate() {
        let indicator = if i == modal.selected { format!("{GREEN}{BOLD}\u{25b6} [{key}] {opt}{RESET}") }
                        else { format!("{DIM}  [{key}] {opt}{RESET}") };
        let vis = 5 + opt.len(); // approximate visible len
        println!("{sp}{YELLOW}\u{2502}{RESET}  {}{}  {YELLOW}\u{2502}{RESET}",
            indicator, " ".repeat(w.saturating_sub(vis + 8)));
    }
    println!("{sp}{YELLOW}\u{2502}{RESET}{}  {YELLOW}\u{2502}{RESET}", " ".repeat(w - 4));
    println!("{sp}{YELLOW}{BOLD}\u{2514}{}{BOLD}\u{2518}{RESET}", "\u{2500}".repeat(w - 2));
    println!("{sp}{DIM}  ↑↓ navigate  Enter to confirm  Esc = Deny{RESET}");
    println!();
}

// ─── Slash Palette ────────────────────────────────────────────────────────────
/// Print the slash command palette (above input line).
pub fn print_slash_palette(filter: &str) {
    let commands = filter_slash_commands(filter);
    if commands.is_empty() {
        println!("  {DIM}No matching commands{RESET}");
        return;
    }
    println!("  {DIM}{}\u{2500}{RESET}", "\u{2500}".repeat(68));
    for cmd in &commands {
        println!("  {CORAL}/{RESET}{BOLD}{}{RESET}  {DIM}{}{RESET}",
            cmd.name,
            cmd.description);
    }
    println!("  {DIM}{}{RESET}", "\u{2500}".repeat(69));
}

// ─── Model Picker ─────────────────────────────────────────────────────────────
/// Print the interactive model picker list.
pub fn print_model_picker(models: &[String], selected: usize, provider: &str) {
    println!();
    println!("  {BOLD}{WHITE}Select model  ({DIM}provider: {CYAN}{}{RESET}{BOLD}{WHITE}){RESET}", provider);
    println!("  {DIM}{}{RESET}", "\u{2500}".repeat(50));
    for (i, model) in models.iter().enumerate() {
        if i == selected {
            println!("  {CORAL}{BOLD}\u{25b6} {}{RESET}", model);
        } else {
            println!("  {DIM}  {}{RESET}", model);
        }
    }
    // Bottom "change provider" option
    let change_idx = models.len();
    if change_idx == selected {
        println!("  {CYAN}{BOLD}\u{25b6} \u{2192} Change provider{RESET}");
    } else {
        println!("  {DIM}  \u{2192} Change provider{RESET}");
    }
    println!("  {DIM}{}{RESET}", "\u{2500}".repeat(50));
    println!("  {DIM}↑↓ navigate  Enter to select  Esc to cancel{RESET}");
    println!();
}

// ─── Input Prompt ─────────────────────────────────────────────────────────────
/// Print the prompt indicator (for display before readline loop).
pub fn print_prompt(app: &App) {
    print!("  {CORAL}{BOLD}> {RESET}");
    io::stdout().flush().unwrap();
}

/// Format a pasted multi-line block as a bracket annotation.
pub fn format_paste_bracket(line_count: usize) -> String {
    format!("[Pasted: {} lines]", line_count)
}

// ─── Help ────────────────────────────────────────────────────────────────────
pub fn print_help() {
    let commands = all_slash_commands();
    println!();
    println!("  {BOLD}{WHITE}Commands:{RESET}");
    for cmd in &commands {
        println!("  {CORAL}/{:<12}{RESET}  {DIM}{}{RESET}", cmd.name, cmd.description);
    }
    println!();
    println!("  {DIM}Keyboard:{RESET}");
    println!("  {DIM}  Esc        Interrupt turn / clear prompt / close menu{RESET}");
    println!("  {DIM}  Ctrl+C     Interrupt current turn{RESET}");
    println!("  {DIM}  ↑↓         Input history{RESET}");
    println!();
}

// ─── Status Info ─────────────────────────────────────────────────────────────
pub fn print_status_info(app: &App) {
    println!();
    println!("  {BOLD}{WHITE}Status:{RESET}");
    println!("  {DIM}Model:     {CYAN}{}{RESET}", app.model_display());
    println!("  {DIM}Provider:  {}{RESET}", app.provider);
    println!("  {DIM}Session:   {}{RESET}", app.session_name);
    println!("  {DIM}Style:     {}{RESET}", app.response_style.name());
    println!("  {DIM}Context:   {}%{RESET}", app.context_pct);
    println!("  {DIM}Messages:  {}{RESET}", app.messages.len());
    println!();
}

// ─── Exit Summary ─────────────────────────────────────────────────────────────
pub fn print_exit_summary(app: &App) {
    let dur = app.start_time.elapsed();
    let m = dur.as_secs() / 60;
    let s = dur.as_secs() % 60;
    let user_msgs = app.messages.iter().filter(|m| m.role == MessageRole::User).count();
    let tool_calls: usize = app.messages.iter().map(|m| m.tool_calls.len()).sum();

    println!();
    println!("  {DIM}Session:   {}{RESET}", app.session_name);
    println!("  {DIM}Duration:  {}m {}s{RESET}", m, s);
    println!("  {DIM}Turns:     {}  Tool calls: {}{RESET}", user_msgs, tool_calls);
    if app.context_pct > 0 {
        println!("  {DIM}Context:   {}%{RESET}", app.context_pct);
    }
    println!();
}

// ─── Profile ─────────────────────────────────────────────────────────────────
pub fn print_profile(profile: &UserProfile) {
    println!();
    println!("  {BOLD}{WHITE}Profile:{RESET}");
    if !profile.name.is_empty()  { println!("  {DIM}Name:  {}{RESET}", profile.name); }
    if !profile.role.is_empty()  { println!("  {DIM}Role:  {}{RESET}", profile.role); }
    if !profile.about.is_empty() { println!("  {DIM}About: {}{RESET}", profile.about); }
    println!("  {DIM}Style: {}{RESET}", profile.style.name());
    println!();
}

// ─── Markdown Inline Renderer ─────────────────────────────────────────────────
pub fn render_inline(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut inner = String::new();
                loop {
                    match chars.next() {
                        Some('*') if chars.peek() == Some(&'*') => { chars.next(); break; }
                        Some(c) => inner.push(c),
                        None => break,
                    }
                }
                result.push_str(&format!("{BOLD}{inner}{RESET}"));
            }
            '*' => {
                let mut inner = String::new();
                loop {
                    match chars.next() {
                        Some('*') => break,
                        Some(c) => inner.push(c),
                        None => break,
                    }
                }
                result.push_str(&format!("{ITALIC}{inner}{RESET}"));
            }
            '`' => {
                let mut inner = String::new();
                loop {
                    match chars.next() {
                        Some('`') => break,
                        Some(c) => inner.push(c),
                        None => break,
                    }
                }
                result.push_str(&format!("{YELLOW}{inner}{RESET}"));
            }
            _ => result.push(ch),
        }
    }
    result
}

// ─── Tool Icon ────────────────────────────────────────────────────────────────
fn tool_icon(name: &str) -> &'static str {
    match name {
        "read_file"  | "fs.read"   => "\u{1f4c4}",
        "write_file" | "fs.write"  => "\u{270f}\u{fe0f}",
        "edit_file"  | "fs.edit"   => "\u{270f}\u{fe0f}",
        "shell"      | "shell.run" => "\u{1f4bb}",
        "glob"       | "fs.glob"   => "\u{1f50d}",
        "memory"     | "memory.*"  => "\u{1f9e0}",
        _                          => "\u{2699}\u{fe0f}",
    }
}

// ─── Input Handling (basic) ───────────────────────────────────────────────────
pub fn read_input() -> io::Result<String> {
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}
