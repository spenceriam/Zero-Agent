pub mod activity;
pub mod approval;
pub mod commands;
pub mod conversation;
pub mod input;
pub mod markdown;
pub mod onboarding;
pub mod status;
pub mod streaming;
pub mod thinking;
pub mod tools;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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

/// TUI application state
pub struct App {
    pub messages: Vec<Message>,
    pub activity_log: Vec<ActivityEntry>,
    pub input: String,
    pub input_cursor: usize,
    pub input_history: Vec<String>,
    pub history_index: Option<usize>,
    pub scroll_offset: usize,
    pub activity_scroll_offset: usize,
    pub show_activity: bool,
    pub response_style: ResponseStyle,
    pub model: String,
    pub session_name: String,
    pub token_count: usize,
    pub is_streaming: bool,
    pub current_stream: String,
    pub approval: Option<ApprovalRequest>,
    pub should_quit: bool,
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
            scroll_offset: 0,
            activity_scroll_offset: 0,
            show_activity: true,
            response_style: ResponseStyle::Concise,
            model: String::from("unknown"),
            session_name: String::from("main"),
            token_count: 0,
            is_streaming: false,
            current_stream: String::new(),
            approval: None,
            should_quit: false,
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
        self.scroll_offset = 0;
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
            });
        }
    }

    pub fn update_tool_status(&mut self, name: &str, status: ToolStatus, output: Option<String>) {
        // Update activity log
        if let Some(entry) = self.activity_log.iter_mut().rev().find(|e| e.tool_name == name) {
            entry.status = status.clone();
            entry.elapsed = Some(entry.timestamp.elapsed());
        }

        // Update message tool calls
        if let Some(msg) = self.messages.last_mut() {
            if let Some(tc) = msg.tool_calls.iter_mut().rev().find(|t| t.name == name) {
                tc.status = status;
                tc.output = output;
                tc.elapsed = Some(tc.timestamp.elapsed());
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        // If approval modal is open, handle approval keys
        if self.approval.is_some() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    // TODO: approve
                    self.approval = None;
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    // TODO: deny
                    self.approval = None;
                }
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    // TODO: always approve
                    self.approval = None;
                }
                KeyCode::Char('v') | KeyCode::Char('V') => {
                    // TODO: view details
                }
                KeyCode::Esc => {
                    self.approval = None;
                }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Enter => {
                if !self.input.is_empty() {
                    let input = self.input.clone();
                    self.input_history.push(input.clone());
                    self.history_index = None;
                    self.input.clear();
                    self.input_cursor = 0;
                    // TODO: send to agent
                    self.add_message(MessageRole::User, input);
                }
            }
            KeyCode::Char(c) => {
                self.input.insert(self.input_cursor, c);
                self.input_cursor += 1;
            }
            KeyCode::Backspace => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                    self.input.remove(self.input_cursor);
                }
            }
            KeyCode::Delete => {
                if self.input_cursor < self.input.len() {
                    self.input.remove(self.input_cursor);
                }
            }
            KeyCode::Left => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.input_cursor < self.input.len() {
                    self.input_cursor += 1;
                }
            }
            KeyCode::Home => {
                self.input_cursor = 0;
            }
            KeyCode::End => {
                self.input_cursor = self.input.len();
            }
            KeyCode::Up => {
                if !self.input_history.is_empty() {
                    let idx = self.history_index.unwrap_or(self.input_history.len());
                    if idx > 0 {
                        self.history_index = Some(idx - 1);
                        self.input = self.input_history[idx - 1].clone();
                        self.input_cursor = self.input.len();
                    }
                }
            }
            KeyCode::Down => {
                if let Some(idx) = self.history_index {
                    if idx < self.input_history.len() - 1 {
                        self.history_index = Some(idx + 1);
                        self.input = self.input_history[idx + 1].clone();
                    } else {
                        self.history_index = None;
                        self.input.clear();
                    }
                    self.input_cursor = self.input.len();
                }
            }
            KeyCode::Tab => {
                // Toggle activity pane
                self.show_activity = !self.show_activity;
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_add(10);
            }
            KeyCode::PageDown => {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
            }
            _ => {}
        }
    }
}

/// Render the TUI
pub fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),      // Conversation
            Constraint::Length(8),    // Activity (collapsible)
            Constraint::Length(3),    // Input
            Constraint::Length(1),    // Status bar
        ])
        .split(f.area());

    // Conversation pane
    conversation::render(f, chunks[0], app);

    // Activity pane (collapsible)
    if app.show_activity {
        activity::render(f, chunks[1], app);
    } else {
        let empty = Paragraph::new("").block(Block::default().borders(Borders::ALL).title("Activity (Tab to show)"));
        f.render_widget(empty, chunks[1]);
    }

    // Input pane
    input::render(f, chunks[2], app);

    // Status bar
    status::render(f, chunks[3], app);

    // Approval modal (overlay)
    if let Some(ref approval) = app.approval {
        approval::render(f, f.area(), approval);
    }
}

/// Run the TUI
pub fn run() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    loop {
        terminal.draw(|f| ui(f, &app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
            }
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
