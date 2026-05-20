#[cfg(feature = "tui")]
pub mod layout;
#[cfg(feature = "tui")]
pub mod input;
#[cfg(feature = "tui")]
pub mod onboarding;
#[cfg(feature = "tui")]
pub mod render;
#[cfg(feature = "tui")]
mod diff;
#[cfg(feature = "tui")]
mod markdown;
#[cfg(feature = "tui")]
pub mod modal;

pub use layout::TuiMode;

use std::io::{self, Write};
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

// Max lines before truncation
const THINKING_MAX_LINES: usize = 100;
pub const TOOL_RESULT_MAX_LINES: usize = 50;

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
    Deny { comment: Option<String> },
    ApproveOnce,
    ApproveSession,
    ApproveAlways,
}

// ─── Approval Modal State ─────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalFocus {
    Pills,
    DenyComment,
}

#[derive(Debug, Clone)]
pub struct ApprovalCardState {
    pub selected: usize,
    pub deny_comment: String,
    pub deny_cursor: usize,
    pub focus: ApprovalFocus,
}

impl Default for ApprovalCardState {
    fn default() -> Self {
        Self {
            selected: 0,
            deny_comment: String::new(),
            deny_cursor: 0,
            focus: ApprovalFocus::Pills,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApprovalModal {
    pub tool_name: String,
    pub command: String,
    pub risk_level: String,
    pub selected: usize, // 0=Once 1=Session 2=Always 3=Deny
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
        SlashCommand { name: "debug".into(),    description: "Toggle debug logging (or: on | off | status)".into() },
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
#[derive(Debug)]
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
    #[cfg(feature = "tui")]
    {
        if let Ok((w, h)) = crossterm::terminal::size() {
            return (w as usize, h as usize);
        }
    }
    (100, 30)
}

/// Left/right inset for scrollable chat transcript (not the prompt footer).
pub const CHAT_GUTTER: usize = 2;

pub fn chat_content_width(terminal_width: usize) -> usize {
    terminal_width.saturating_sub(CHAT_GUTTER * 2)
}

pub fn apply_chat_gutter(line: &str) -> String {
    format!("{}{}", " ".repeat(CHAT_GUTTER), line)
}

/// Calculate visible length of string (strips ANSI escape codes)
pub fn visible_len(s: &str) -> usize {
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

/// Word-wrap text to `max_visible` terminal columns. Preserves inline ANSI sequences.
/// Hard-breaks on `\n`; soft-wraps long lines at spaces when possible.
pub fn wrap_text_visible(text: &str, max_visible: usize) -> Vec<String> {
    if max_visible == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        lines.extend(wrap_paragraph_visible(paragraph, max_visible));
    }
    lines
}

fn wrap_paragraph_visible(text: &str, max_width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    if visible_len(text) <= max_width {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if visible_len(rest) <= max_width {
            lines.push(rest.to_string());
            break;
        }
        let byte_limit = visible_byte_limit(rest, max_width);
        let slice = &rest[..byte_limit];
        let break_at = rfind_space_before(slice).unwrap_or(byte_limit);
        if break_at == 0 {
            // No space — hard break at column limit
            lines.push(rest[..byte_limit].to_string());
            rest = &rest[byte_limit..];
        } else {
            lines.push(rest[..break_at].trim_end().to_string());
            rest = rest[break_at..].trim_start();
        }
    }
    lines
}

/// Byte index after `max_visible` visible characters (may fall inside a UTF-8 char — walks safely).
fn visible_byte_limit(s: &str, max_visible: usize) -> usize {
    let mut vis = 0usize;
    let mut in_escape = false;
    let mut end = 0usize;
    for (i, ch) in s.char_indices() {
        if ch == '\x1b' {
            in_escape = true;
            end = i + 1;
            continue;
        }
        if in_escape {
            end = i + ch.len_utf8();
            if ch == 'm' {
                in_escape = false;
            }
            continue;
        }
        vis += 1;
        end = i + ch.len_utf8();
        if vis >= max_visible {
            break;
        }
    }
    end.min(s.len())
}

fn rfind_space_before(s: &str) -> Option<usize> {
    let mut in_escape = false;
    let mut last_space: Option<usize> = None;
    for (i, ch) in s.char_indices() {
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
        if ch == ' ' {
            last_space = Some(i);
        }
    }
    last_space
}

/// Visible width of `  ❯ ` plus optional hint.
pub fn prompt_first_prefix_cols(hint: &str) -> usize {
    let hint_part = if hint.is_empty() {
        0
    } else {
        hint.len() + 1
    };
    4 + hint_part
}

pub const PROMPT_CONTINUATION_INDENT: usize = 6;

/// Max chars for inline single-line paste before showing badge preview.
pub const PASTE_INLINE_MAX: usize = 120;

/// One row of the growing prompt composer.
#[derive(Clone, Debug)]
pub struct PromptGrid {
    pub display_lines: Vec<String>,
    pub row_starts: Vec<usize>,
    pub row_ends: Vec<usize>,
    pub text_start_col: Vec<usize>,
}

fn wrap_paragraph_first_cont(text: &str, first_width: usize, cont_width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut rest = text;
    let mut width = first_width;
    while !rest.is_empty() {
        if visible_len(rest) <= width {
            lines.push(rest.to_string());
            break;
        }
        let byte_limit = visible_byte_limit(rest, width);
        let slice = &rest[..byte_limit];
        let break_at = rfind_space_before(slice).unwrap_or(byte_limit);
        if break_at == 0 {
            lines.push(rest[..byte_limit].to_string());
            rest = &rest[byte_limit..];
        } else {
            lines.push(rest[..break_at].trim_end().to_string());
            rest = rest[break_at..].trim_start();
        }
        width = cont_width;
    }
    lines
}

/// Build wrapped prompt display rows and byte-index mapping for cursor navigation.
pub fn build_prompt_grid(
    input: &str,
    width: usize,
    hint: &str,
    paste_badge: Option<&str>,
) -> PromptGrid {
    let first_prefix = prompt_first_prefix_cols(hint);
    let first_wrap = width.saturating_sub(first_prefix);
    let cont_wrap = width.saturating_sub(first_prefix);

    let hint_str = if hint.is_empty() {
        String::new()
    } else {
        format!("{hint} ")
    };

    let mut display_lines = Vec::new();
    let mut row_starts = Vec::new();
    let mut row_ends = Vec::new();
    let mut text_start_col = Vec::new();

    let mut byte_offset = 0usize;
    let mut is_first_row = true;

    let paragraphs: Vec<&str> = if input.is_empty() {
        vec![""]
    } else {
        input.split('\n').collect()
    };

    for (pi, paragraph) in paragraphs.iter().enumerate() {
        let para_start = byte_offset;
        let wrapped = if is_first_row {
            wrap_paragraph_first_cont(paragraph, first_wrap, cont_wrap)
        } else {
            wrap_paragraph_first_cont(paragraph, cont_wrap, cont_wrap)
        };

        for (wi, chunk) in wrapped.iter().enumerate() {
            let chunk_start = if wi == 0 { para_start } else { byte_offset };
            let chunk_end = chunk_start + chunk.len();

            row_starts.push(chunk_start);
            row_ends.push(chunk_end);
            byte_offset = chunk_end;

            let line = if is_first_row {
                text_start_col.push(first_prefix);
                format!("  {CORAL}{BOLD}\u{276f}{RESET} {hint_str}{chunk}")
            } else if pi > 0 && wi == 0 {
                text_start_col.push(PROMPT_CONTINUATION_INDENT);
                format!("{}{}", " ".repeat(PROMPT_CONTINUATION_INDENT), chunk)
            } else {
                text_start_col.push(first_prefix);
                format!("{}{}", " ".repeat(first_prefix), chunk)
            };
            display_lines.push(line);
            is_first_row = false;
        }

        byte_offset = para_start + paragraph.len();
        if pi + 1 < paragraphs.len() {
            byte_offset += 1;
        }
    }

    if display_lines.is_empty() {
        row_starts.push(0);
        row_ends.push(0);
        text_start_col.push(first_prefix);
        display_lines.push(format!("  {CORAL}{BOLD}\u{276f}{RESET} {hint_str}"));
    }

    if let Some(badge) = paste_badge {
        let last = display_lines.len() - 1;
        if display_lines[last].ends_with(' ') || row_ends[last] == row_starts[last] {
            display_lines[last] = format!("{}{badge}", display_lines[last]);
        } else {
            display_lines[last] = format!("{} {badge}", display_lines[last]);
        }
    }

    PromptGrid {
        display_lines,
        row_starts,
        row_ends,
        text_start_col,
    }
}

/// Map buffer byte index to (prompt_row, screen_col).
pub fn cursor_to_screen(grid: &PromptGrid, cursor: usize) -> (usize, usize) {
    cursor_to_screen_simple(grid, cursor)
}

/// Map (prompt_row, screen_col) to buffer byte index.
pub fn screen_to_cursor(grid: &PromptGrid, row: usize, col: usize) -> usize {
    screen_to_cursor_simple(grid, row, col)
}

fn text_visible_len(grid: &PromptGrid, row: usize) -> usize {
    let start = grid.row_starts[row];
    let end = grid.row_ends[row];
    end.saturating_sub(start)
}

fn cursor_to_screen_simple(grid: &PromptGrid, cursor: usize) -> (usize, usize) {
    let cursor = cursor.min(grid.row_ends.last().copied().unwrap_or(0));
    for (i, (&start, &end)) in grid.row_starts.iter().zip(grid.row_ends.iter()).enumerate() {
        if cursor <= end || i + 1 == grid.row_starts.len() {
            let col_in_text = cursor.saturating_sub(start);
            return (i, grid.text_start_col[i] + col_in_text);
        }
    }
    (0, grid.text_start_col.first().copied().unwrap_or(4))
}

pub fn screen_to_cursor_simple(grid: &PromptGrid, row: usize, col: usize) -> usize {
    let row = row.min(grid.row_starts.len().saturating_sub(1));
    let text_start = grid.text_start_col[row];
    let col_in_text = col.saturating_sub(text_start);
    let max_col = text_visible_len(grid, row);
    grid.row_starts[row] + col_in_text.min(max_col)
}

pub fn prev_char_boundary(s: &str, cursor: usize) -> usize {
    if cursor == 0 {
        return 0;
    }
    let mut idx = cursor - 1;
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

pub fn next_char_boundary(s: &str, cursor: usize) -> usize {
    if cursor >= s.len() {
        return s.len();
    }
    let mut idx = cursor + 1;
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

pub fn should_paste_as_badge(paste: &str, term_width: usize) -> bool {
    if paste.contains('\n') || paste.lines().count() > 1 {
        return true;
    }
    paste.len() > PASTE_INLINE_MAX || visible_len(paste) > term_width.saturating_sub(10)
}

#[cfg(test)]
mod wrap_tests {
    use super::*;

    #[test]
    fn wrap_breaks_long_plain_line() {
        let text = "hello world this is a long line of text";
        let lines = wrap_text_visible(text, 12);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| visible_len(l) <= 12));
    }

    #[test]
    fn wrap_preserves_hard_newlines() {
        let lines = wrap_text_visible("line one\nline two", 80);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "line one");
        assert_eq!(lines[1], "line two");
    }

    #[test]
    fn prompt_grid_wraps_long_line() {
        let input = "hello world this is a longer typed prompt line";
        let grid = build_prompt_grid(input, 20, "", None);
        assert!(grid.display_lines.len() > 1);
        assert!(grid.display_lines[0].contains('\u{276f}'));
    }

    #[test]
    fn prompt_grid_cursor_roundtrip() {
        let input = "hello world wrap here";
        let grid = build_prompt_grid(input, 12, "", None);
        for cursor in 0..=input.len() {
            let (row, col) = cursor_to_screen(&grid, cursor);
            let back = screen_to_cursor(&grid, row, col);
            assert!(back <= input.len());
        }
    }

    #[test]
    fn paste_badge_does_not_add_prompt_rows() {
        let grid = build_prompt_grid("fix bug", 80, "", Some("[pasted: 42 lines, 100 chars — press Enter to send]"));
        assert_eq!(grid.display_lines.len(), 1);
        assert!(grid.display_lines[0].contains("pasted:"));
    }

    #[test]
    fn chat_content_width_applies_gutter() {
        assert_eq!(chat_content_width(80), 76);
        assert_eq!(apply_chat_gutter("hello"), "  hello");
    }

    #[test]
    fn prompt_grid_wrap_aligns_continuation_with_first_line() {
        let input = "hello world this is a longer typed prompt line that should wrap";
        let grid = build_prompt_grid(input, 30, "", None);
        assert!(grid.display_lines.len() > 1);
        let first_col = grid.text_start_col[0];
        for &col in &grid.text_start_col[1..] {
            assert_eq!(
                col, first_col,
                "soft-wrap rows should share the same text start column"
            );
        }
    }

    #[test]
    fn prompt_grid_hard_newline_uses_continuation_indent() {
        let input = "line one\nline two";
        let grid = build_prompt_grid(input, 40, "", None);
        assert!(grid.display_lines.len() >= 2);
        assert_eq!(grid.text_start_col[0], prompt_first_prefix_cols(""));
        assert_eq!(grid.text_start_col[1], PROMPT_CONTINUATION_INDENT);
    }

    #[test]
    fn tool_status_detects_shell_exit_code() {
        assert_eq!(
            tool_status_from_result("shell", "stderr: fail\nexit code: 1"),
            ToolStatus::Error
        );
        assert_eq!(
            tool_status_from_result("shell", "/tmp\nexit code: 0"),
            ToolStatus::Success
        );
    }

    #[test]
    fn tool_display_label_uses_shell_not_dollar() {
        assert_eq!(tool_display_label("shell"), "shell");
    }
}

// ─── Startup Banner & Status Bar ──────────────────────────────────────────────

/// Basic 2-line startup banner (session init only; layout prints this).
pub fn print_startup_banner(
    app: &App,
    _config_path: &str,
    tools: &[String],
    cwd: &str,
    _no_auth: bool,
) {
    let _ = (app, tools, cwd);
    // Banner is rendered by ScreenLayout::init
}

/// Persistent status bar — delegates to layout footer when active.
pub fn print_status_bar(app: &App) {
    layout::redraw_footer(app, "", 0, None);
}

pub fn shorten_home(path: &str) -> String {
    if let Ok(home) = std::env::var("HOME") {
        if path.starts_with(&home) {
            return path.replacen(&home, "~", 1);
        }
    }
    path.to_string()
}

pub fn context_bar(pct: usize) -> String {
    let filled = (pct.min(100) as f64 / 100.0 * 10.0).round() as usize;
    let color = if pct >= 85 {
        RED
    } else if pct >= 60 {
        YELLOW
    } else {
        GREEN
    };
    let suffix = if pct > 0 {
        format!("] {pct}%")
    } else {
        "] 0%".to_string()
    };
    format!(
        "{color}[{}{}{RESET}{suffix}",
        "\u{2593}".repeat(filled),
        "\u{2591}".repeat(10usize.saturating_sub(filled)),
    )
}

// ─── Incremental Stream Writer ────────────────────────────────────────────────

/// Renders provider stream events incrementally into scrollback.
pub struct AgentStreamWriter {
    reasoning_started: bool,
    pub agent_started: bool,
    pub reasoning: String,
    pub text: String,
}

impl AgentStreamWriter {
    pub fn new() -> Self {
        Self {
            reasoning_started: false,
            agent_started: false,
            reasoning: String::new(),
            text: String::new(),
        }
    }

    pub fn on_event(&mut self, event: &crate::provider::StreamEvent) {
        use crate::provider::StreamEventType;
        match &event.event_type {
            StreamEventType::Reasoning(chunk) => {
                self.reasoning.push_str(chunk);
                if !self.reasoning_started {
                    layout::append_thinking_start();
                    self.reasoning_started = true;
                }
                layout::append_thinking_chunk(chunk);
            }
            StreamEventType::Text(chunk) => {
                self.text.push_str(chunk);
                if !self.agent_started {
                    if self.reasoning_started {
                        layout::append_thinking_end();
                    }
                    layout::append_agent_start();
                    self.agent_started = true;
                }
                layout::append_agent_chunk(chunk);
            }
            _ => {}
        }
    }

    pub fn finish(&self) {
        layout::flush_stream();
        if self.agent_started || self.reasoning_started {
            layout::append_agent_end();
        }
    }
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

/// Full-width horizontal rule for the prompt composer borders.
pub fn format_prompt_border(width: usize) -> String {
    "\u{2500}".repeat(width.max(1))
}

/// Map prompt row index to screen row (below top rule, above bottom rule).
pub fn prompt_screen_row(footer_start: u16, prompt_row: usize) -> u16 {
    footer_start + 1 + prompt_row as u16
}

/// Print a static stopped/interrupted status.
pub fn print_status_stopped() {
    layout::set_status_mode(StatusMode::Interrupted);
}

// ─── Scroll Blocks ────────────────────────────────────────────────────────────

/// Print a user message block.
/// ┌─ You ────────────────────────────────────────────────────┐
/// │ message text                                             │
/// └──────────────────────────────────────────────────────────┘
pub fn print_user_block(text: &str, width: usize) {
    layout::append_user_block(text, width);
}

/// Print a streaming agent text block — no box, just labeled lines.
/// ZERO  text flows here with inline markdown rendering
pub fn print_agent_text(text: &str) {
    layout::append_agent_text(text);
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

/// Print a tool call line (legacy append-only; prefer begin/update).
pub fn print_tool_call(name: &str, args_preview: &str, status: &ToolStatus, elapsed: Option<Duration>) {
    layout::append_tool_call(name, args_preview, status, elapsed);
}

/// Start a tool call block; returns (transcript index, line count) for in-place update.
pub fn begin_tool_call(name: &str, args: &str) -> (usize, usize) {
    layout::begin_tool_call(name, args)
}

/// Update an existing tool call block in place.
pub fn update_tool_call(
    index: usize,
    line_count: usize,
    name: &str,
    args: &str,
    status: &ToolStatus,
    elapsed: Option<Duration>,
) {
    layout::update_tool_call(index, line_count, name, args, status, elapsed);
}

/// Print a tool result (dimmed, indented, truncated if large).
pub fn print_tool_result(name: &str, output: &str, log_path: &str) {
    let _ = name;
    layout::append_tool_result(output, log_path);
}

pub use diff::{DiffInput, DiffKind};

/// Render a Pi-style diff after successful edit_file / write_file.
pub fn print_tool_diff(input: DiffInput) {
    let (content_width, terminal_width) = layout::ScreenLayout::with_global(|layout| {
        (layout.content_width(), layout.width() as usize)
    })
    .unwrap_or_else(|| {
        let (w, _) = get_terminal_size();
        (chat_content_width(w), w)
    });
    let lines = diff::render_diff_lines(&input, content_width, terminal_width);
    layout::append_diff_lines(&lines);
}

/// Print a memory footer line at end of turn.
/// ↳ Memory: prefer concise responses | project.md
pub fn print_memory_footer(description: &str, file_name: &str) {
    let desc = if description.len() > 60 {
        format!("{}...", &description[..57])
    } else {
        description.to_string()
    };
    layout::append_line(&format!("  {DIM}\u{21b3} Memory: {desc} | {file_name}{RESET}"));
}

/// Print a system note (dim, no border).
pub fn print_system_note(text: &str) {
    layout::append_system_note(text);
}

// ─── Tool label + status ──────────────────────────────────────────────────────

pub fn tool_display_label(name: &str) -> &str {
    match name {
        "read_file" | "fs.read" => "read",
        "write_file" | "fs.write" => "write",
        "edit_file" | "fs.edit" => "edit",
        "shell" | "shell.run" => "shell",
        "glob" | "fs.glob" => "glob",
        "memory" | "memory.save" | "memory.list" => "mem",
        _ => "tool",
    }
}

pub fn tool_status_from_result(name: &str, result: &str) -> ToolStatus {
    if result.starts_with("Error running command:") {
        return ToolStatus::Error;
    }
    if name == "shell" {
        for line in result.lines() {
            if let Some(code) = line.strip_prefix("exit code: ") {
                if code.trim() != "0" {
                    return ToolStatus::Error;
                }
            }
        }
    }
    ToolStatus::Success
}

/// Legacy ASCII tag (bridge render path).
pub fn tool_tag(name: &str) -> &'static str {
    match name {
        "read_file" | "fs.read" => "read",
        "write_file" | "fs.write" => "write",
        "edit_file" | "fs.edit" => "edit",
        "shell" | "shell.run" => "$",
        "glob" | "fs.glob" => "glob",
        "memory" | "memory.save" | "memory.list" => "mem",
        _ => "tool",
    }
}

const BG_PILL_UNSELECTED: &str = "\x1b[48;5;236m\x1b[38;5;245m";
const BG_PILL_APPROVE: &str = "\x1b[48;5;28m\x1b[97m";
const BG_PILL_DENY: &str = "\x1b[41m\x1b[97m";
const DENY_TEXTAREA_ROWS: usize = 3;

pub fn format_approval_pill(label: &str, is_deny: bool, selected: bool) -> String {
    let bg = if selected {
        if is_deny {
            BG_PILL_DENY
        } else {
            BG_PILL_APPROVE
        }
    } else {
        BG_PILL_UNSELECTED
    };
    format!("  {bg} {label} {RESET}")
}

/// Wrapped logical lines for a multi-line comment with byte offsets for cursor mapping.
fn comment_wrapped_lines(comment: &str, inner_width: usize) -> Vec<(usize, String)> {
    if inner_width == 0 {
        return vec![(0, String::new())];
    }
    let mut out = Vec::new();
    let mut byte_offset = 0usize;
    for (para_idx, paragraph) in comment.split('\n').enumerate() {
        if para_idx > 0 {
            byte_offset += 1;
        }
        let wrapped = wrap_text_visible(paragraph, inner_width);
        if wrapped.is_empty() || (wrapped.len() == 1 && wrapped[0].is_empty()) {
            out.push((byte_offset, String::new()));
            byte_offset += paragraph.len();
            continue;
        }
        let mut local = 0usize;
        for (i, line) in wrapped.iter().enumerate() {
            out.push((byte_offset + local, line.clone()));
            if i + 1 < wrapped.len() {
                local += line.len();
                if paragraph[local..].starts_with(' ') {
                    local += 1;
                }
            }
        }
        byte_offset += paragraph.len();
    }
    if out.is_empty() {
        out.push((0, String::new()));
    }
    out
}

fn cursor_line_index(wrapped: &[(usize, String)], cursor: usize) -> usize {
    let mut idx = wrapped.len().saturating_sub(1);
    for (i, (start, line)) in wrapped.iter().enumerate() {
        let end = start + line.len();
        if cursor <= end || (i + 1 == wrapped.len() && cursor >= *start) {
            idx = i;
            break;
        }
    }
    idx
}

/// Bordered multi-line text area for optional deny comments (no line numbers).
pub fn build_deny_textarea(
    comment: &str,
    cursor: usize,
    inner_width: usize,
    show_cursor: bool,
) -> Vec<String> {
    let inner_width = inner_width.max(8);
    let wrapped = comment_wrapped_lines(comment, inner_width);
    let cursor_line = cursor_line_index(&wrapped, cursor.min(comment.len()));
    let scroll = cursor_line.saturating_sub(DENY_TEXTAREA_ROWS.saturating_sub(1));

    let mut lines = vec![format!("{DIM}Reason (optional){RESET}")];
    lines.push(format!(
        "{DIM}\u{250c}{}{RESET}",
        "\u{2500}".repeat(inner_width)
    ));

    for vis_idx in 0..DENY_TEXTAREA_ROWS {
        let abs_line = scroll + vis_idx;
        let row = if let Some((line_start, text)) = wrapped.get(abs_line) {
            let mut content = text.clone();
            if show_cursor && abs_line == cursor_line {
                let byte_in_line = cursor.saturating_sub(*line_start).min(text.len());
                let before = &text[..byte_in_line];
                let cursor_char = text[byte_in_line..].chars().next().unwrap_or(' ');
                let after = &text[byte_in_line + cursor_char.len_utf8()..];
                content = format!("{before}{BOLD}{cursor_char}{RESET}{after}");
            }
            pad_visible_line(&content, inner_width)
        } else {
            " ".repeat(inner_width)
        };
        lines.push(format!("{DIM}\u{2502}{RESET} {row} {DIM}\u{2502}{RESET}"));
    }

    lines.push(format!(
        "{DIM}\u{2514}{}{RESET}",
        "\u{2500}".repeat(inner_width)
    ));
    lines
}

fn pad_visible_line(text: &str, width: usize) -> String {
    let mut out = text.to_string();
    while visible_len(&out) < width {
        out.push(' ');
    }
    out
}

pub fn approval_card_footer(state: &ApprovalCardState) -> &'static str {
    if state.selected == 3 && state.focus == ApprovalFocus::DenyComment {
        "Enter newline · Ctrl+Enter confirm · Esc deny"
    } else {
        "↑↓ navigate · Enter confirm · Esc deny"
    }
}

// ─── Approval card (frame-buffer overlay) ────────────────────────────────────
pub fn build_approval_card(
    modal: &ApprovalModal,
    state: &ApprovalCardState,
    term_width: u16,
) -> Vec<String> {
    use crate::tui::modal::{build_card_lines, card_width_for_terminal};

    let card_w = card_width_for_terminal(term_width);
    let inner = card_w.saturating_sub(4);
    let risk_color = match modal.risk_level.as_str() {
        "Safe" => GREEN,
        "Mutating" => YELLOW,
        "Destructive" => RED,
        _ => RED,
    };

    let mut body = vec![format!("Tool: {BOLD}{}{RESET}", modal.tool_name)];
    if !modal.command.is_empty() {
        let cmd_width = inner.saturating_sub(9);
        let wrapped = wrap_text_visible(&modal.command, cmd_width.max(8));
        for (i, line) in wrapped.iter().enumerate() {
            if i == 0 {
                body.push(format!("Command: {CYAN}{line}{RESET}"));
            } else {
                body.push(format!("         {CYAN}{line}{RESET}"));
            }
        }
    }
    body.push(format!(
        "Risk: {risk_color}{BOLD}{}{RESET}",
        modal.risk_level
    ));
    body.push(String::new());

    let options = [
        ("Approve Once", false),
        ("Approve for Session", false),
        ("Approve Always", false),
        ("Deny", true),
    ];
    for (i, (label, is_deny)) in options.iter().enumerate() {
        body.push(format_approval_pill(label, *is_deny, state.selected == i));
    }

    if state.selected == 3 {
        body.push(String::new());
        let textarea_inner = inner.saturating_sub(4).max(8);
        body.extend(build_deny_textarea(
            &state.deny_comment,
            state.deny_cursor,
            textarea_inner,
            state.focus == ApprovalFocus::DenyComment,
        ));
    }

    build_card_lines(
        "Action Required",
        &body,
        approval_card_footer(state),
        card_w,
    )
}

// ─── Approval Modal (legacy println removed — use frame-buffer overlay) ─────

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
/// Print the prompt indicator (Hermes Classic ❯ glyph).
pub fn print_prompt(_app: &App) {
    print!("  {CORAL}{BOLD}\u{276f} {RESET}");
    io::stdout().flush().unwrap();
}

/// Format a pasted multi-line block as a Hermes-style preview annotation.
pub fn format_paste_preview(line_count: usize, char_count: usize) -> String {
    format!(
        "[pasted: {line_count} lines, {char_count} chars — press Enter to send]"
    )
}

/// Legacy alias kept for compatibility.
pub fn format_paste_bracket(line_count: usize) -> String {
    format_paste_preview(line_count, 0)
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

// ─── Tool Icon (deprecated — use tool_tag) ────────────────────────────────────
pub fn tool_icon(name: &str) -> &'static str {
    tool_tag(name)
}

#[cfg(test)]
mod approval_tests {
    use super::*;

    fn has_emoji(s: &str) -> bool {
        s.chars().any(|c| {
            matches!(c as u32,
                0x1F300..=0x1FAFF | 0x2600..=0x27BF | 0x1F1E6..=0x1F1FF
            )
        })
    }

    #[test]
    fn approval_card_has_no_emoji() {
        let modal = ApprovalModal {
            tool_name: "shell".into(),
            command: r#"{"command":"rm -rf dist"}"#.into(),
            risk_level: "Destructive".into(),
            selected: 0,
        };
        let state = ApprovalCardState::default();
        let lines = build_approval_card(&modal, &state, 80);
        for line in &lines {
            assert!(!has_emoji(line), "approval card should not contain emoji: {line}");
        }
    }

    #[test]
    fn approval_card_uses_pills_without_hotkeys() {
        let modal = ApprovalModal {
            tool_name: "shell".into(),
            command: String::new(),
            risk_level: "Destructive".into(),
            selected: 0,
        };
        let state = ApprovalCardState::default();
        let joined = build_approval_card(&modal, &state, 80).join("\n");
        assert!(joined.contains("Approve Once"));
        assert!(joined.contains("Approve for Session"));
        assert!(joined.contains("Approve Always"));
        assert!(joined.contains("Deny"));
        assert!(!joined.contains("[D]"));
        assert!(!joined.contains("[O]"));
    }

    #[test]
    fn approval_card_wraps_long_command() {
        let long_path = "/Users/spencer/GitHub/zero-agent/bridge/rust/src/tui/mod.rs";
        let modal = ApprovalModal {
            tool_name: "read".into(),
            command: format!(r#"{{"path":"{long_path}"}}"#),
            risk_level: "Safe".into(),
            selected: 0,
        };
        let state = ApprovalCardState::default();
        let joined = build_approval_card(&modal, &state, 50).join("\n");
        assert!(!joined.contains("..."), "command should wrap not truncate: {joined}");
        assert!(
            joined.contains("mod.rs") && joined.contains("/Users/spencer/GitHub"),
            "full path should appear across wrapped lines: {joined}"
        );
    }

    #[test]
    fn deny_textarea_has_no_line_numbers() {
        let lines = build_deny_textarea("please stop", 7, 30, true);
        let joined = lines.join("\n");
        assert!(joined.contains("Reason (optional)"));
        assert!(joined.contains('\u{2502}'));
        assert!(!joined.contains(" 1 "));
        assert!(!joined.contains(" 2 "));
    }

    #[test]
    fn tool_tag_has_no_emoji() {
        for name in ["shell", "read_file", "write_file", "edit_file", "glob"] {
            let tag = tool_tag(name);
            assert!(!has_emoji(tag), "tool tag should be ASCII: {tag}");
        }
    }
}

#[cfg(test)]
mod banner_tests {
    use super::*;

    #[test]
    fn status_bar_idle_contains_model() {
        let bar = context_bar(0);
        assert!(bar.contains("0%"));
    }

    #[test]
    fn context_bar_high_usage_is_red() {
        let bar = context_bar(90);
        assert!(bar.contains(RED));
    }

    #[test]
    fn paste_preview_hermes_wording() {
        let s = format_paste_preview(3, 42);
        assert!(s.contains("3 lines"));
        assert!(s.contains("42 chars"));
        assert!(s.contains("press Enter to send"));
    }
}

// ─── Input Handling (basic) ───────────────────────────────────────────────────
pub fn read_input() -> io::Result<String> {
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}
