use std::io::{self, IsTerminal};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossterm::cursor::Show;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use crossterm::terminal::{self};
use crossterm::execute;

use super::layout::{self, ScreenLayout, TuiMode};
use super::{
    build_approval_card, build_prompt_grid, cursor_to_screen, filter_slash_commands,
    format_paste_preview, modal::center_card_in_viewport, next_char_boundary, prev_char_boundary,
    print_model_picker, print_system_note, screen_to_cursor, should_paste_as_badge,
    ApprovalCardState, ApprovalChoice, ApprovalFocus, ApprovalModal, App,
};

#[derive(Debug)]
pub enum InputResult {
    Submit(String),
    Interrupt,
    Empty,
}

pub enum ModelPickerResult {
    Selected(String),
    ChangeProvider,
    Cancelled,
}

pub struct InteractiveInput {
    pub history: Vec<String>,
    history_index: Option<usize>,
    draft: String,
    pending_paste: Option<String>,
}

impl InteractiveInput {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            history_index: None,
            draft: String::new(),
            pending_paste: None,
        }
    }

    pub fn read_line(&mut self, app: &App) -> io::Result<InputResult> {
        if layout::get_tui_mode() != TuiMode::Chat {
            return Ok(InputResult::Empty);
        }

        crate::debug::log("input", "read_line enter");

        let mut buffer = if self.draft.is_empty() {
            String::new()
        } else {
            std::mem::take(&mut self.draft)
        };
        let mut cursor = buffer.len();
        self.history_index = None;

        terminal::enable_raw_mode()?;

        let result = loop {
            redraw_input(app, &buffer, cursor, self.pending_paste.as_ref());

            match event::read()? {
                Event::Key(key) => match key {
                    KeyEvent {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    }
                    | KeyEvent {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::SUPER,
                        ..
                    } => {
                        break InputResult::Interrupt;
                    }
                    KeyEvent {
                        code: KeyCode::Esc, ..
                    } => {
                        if buffer.is_empty() && self.pending_paste.is_none() {
                            break InputResult::Empty;
                        }
                        buffer.clear();
                        cursor = 0;
                        self.pending_paste = None;
                    }
                    KeyEvent {
                        code: KeyCode::Enter,
                        modifiers: KeyModifiers::ALT,
                        ..
                    }
                    | KeyEvent {
                        code: KeyCode::Char('\\'),
                        modifiers: KeyModifiers::NONE,
                        ..
                    } if cursor > 0 && buffer[..cursor].ends_with('\\') => {
                        buffer.remove(cursor - 1);
                        cursor -= 1;
                        buffer.insert(cursor, '\n');
                        cursor += 1;
                    }
                    KeyEvent {
                        code: KeyCode::Enter, ..
                    } => {
                        let mut submit = buffer.trim().to_string();
                        if let Some(paste) = self.pending_paste.take() {
                            if submit.is_empty() {
                                submit = paste;
                            } else {
                                submit.push('\n');
                                submit.push_str(&paste);
                            }
                        }
                        break InputResult::Submit(submit);
                    }
                    KeyEvent {
                        code: KeyCode::Backspace, ..
                    } => {
                        if self.pending_paste.is_some() {
                            self.pending_paste = None;
                        } else if cursor > 0 {
                            let prev = prev_char_boundary(&buffer, cursor);
                            buffer.drain(prev..cursor);
                            cursor = prev;
                        }
                    }
                    KeyEvent {
                        code: KeyCode::Delete, ..
                    } => {
                        if self.pending_paste.is_some() {
                            self.pending_paste = None;
                        } else if cursor < buffer.len() {
                            let next = next_char_boundary(&buffer, cursor);
                            buffer.drain(cursor..next);
                        }
                    }
                    KeyEvent {
                        code: KeyCode::Left, ..
                    } => {
                        if cursor > 0 {
                            cursor = prev_char_boundary(&buffer, cursor);
                        }
                    }
                    KeyEvent {
                        code: KeyCode::Right, ..
                    } => {
                        if cursor < buffer.len() {
                            cursor = next_char_boundary(&buffer, cursor);
                        }
                    }
                    KeyEvent {
                        code: KeyCode::Up, ..
                    } => {
                        if try_move_cursor_vertical(&buffer, &mut cursor, -1, app) {
                            // moved within prompt
                        } else if !self.history.is_empty() {
                            let idx = self.history_index.unwrap_or(self.history.len());
                            if idx > 0 {
                                self.history_index = Some(idx - 1);
                                buffer = self.history[idx - 1].clone();
                                cursor = buffer.len();
                            }
                        }
                    }
                    KeyEvent {
                        code: KeyCode::Down, ..
                    } => {
                        if try_move_cursor_vertical(&buffer, &mut cursor, 1, app) {
                            // moved within prompt
                        } else if let Some(idx) = self.history_index {
                            if idx + 1 < self.history.len() {
                                self.history_index = Some(idx + 1);
                                buffer = self.history[idx + 1].clone();
                                cursor = buffer.len();
                            } else {
                                self.history_index = None;
                                buffer.clear();
                                cursor = 0;
                            }
                        }
                    }
                    KeyEvent {
                        code: KeyCode::Char(ch), ..
                    } => {
                        if self.pending_paste.is_some() {
                            self.pending_paste = None;
                        }
                        buffer.insert(cursor, ch);
                        cursor += ch.len_utf8();
                    }
                    _ => {}
                },
                Event::Paste(paste) => {
                    let width = super::get_terminal_size().0;
                    if should_paste_as_badge(&paste, width) {
                        self.pending_paste = Some(paste);
                    } else {
                        buffer.insert_str(cursor, &paste);
                        cursor += paste.len();
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => layout::scroll_by(-3),
                    MouseEventKind::ScrollDown => layout::scroll_by(3),
                    _ => {}
                },
                Event::Resize(w, h) => layout::handle_resize(w, h),
                _ => {}
            }
        };

        layout::set_prompt_hint("");
        layout::set_footer_app(app);
        layout::redraw_footer_with_palette(app, "", 0, None, Vec::new());
        terminal::disable_raw_mode()?;
        execute!(io::stdout(), Show)?;

        if let InputResult::Submit(ref line) = result {
            crate::debug::log("input", &format!("read_line Submit len={}", line.len()));
            if !line.is_empty() {
                if self.history.last().map(|s| s.as_str()) != Some(line.as_str()) {
                    self.history.push(line.clone());
                }
            }
        } else {
            crate::debug::log("input", &format!("read_line exit {:?}", result));
        }

        Ok(result)
    }
}

fn try_move_cursor_vertical(buffer: &str, cursor: &mut usize, delta: i32, app: &App) -> bool {
    let width = super::get_terminal_size().0;
    let grid = build_prompt_grid(buffer, width, "", None);
    let (row, col) = cursor_to_screen(&grid, *cursor);
    let target_row = if delta < 0 {
        row.saturating_sub((-delta) as usize)
    } else {
        row + delta as usize
    };

    if target_row == row {
        return false;
    }
    if target_row >= grid.display_lines.len() {
        return false;
    }

    *cursor = screen_to_cursor(&grid, target_row, col);
    let _ = app;
    true
}

fn paste_badge_text(paste: Option<&String>) -> Option<String> {
    paste.map(|p| {
        let lines = p.lines().count().max(1);
        let chars = p.chars().count();
        format_paste_preview(lines, chars)
    })
}

/// Background listener during agent turns: Ctrl+C interrupt + mouse scroll.
pub struct TurnInputListener {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TurnInputListener {
    pub fn start(interrupt: Arc<AtomicBool>) -> Self {
        crate::debug::log("listener", "TurnInputListener start");
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            let _ = terminal::enable_raw_mode();
            while !stop_clone.load(Ordering::Relaxed) {
                if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                    if stop_clone.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Ok(ev) = event::read() {
                        match ev {
                            Event::Key(KeyEvent {
                                code: KeyCode::Char('c'),
                                modifiers: KeyModifiers::CONTROL,
                                ..
                            })
                            | Event::Key(KeyEvent {
                                code: KeyCode::Char('c'),
                                modifiers: KeyModifiers::SUPER,
                                ..
                            }) => {
                                interrupt.store(true, Ordering::Relaxed);
                            }
                            Event::Mouse(mouse) => match mouse.kind {
                                MouseEventKind::ScrollUp => layout::scroll_by(-3),
                                MouseEventKind::ScrollDown => layout::scroll_by(3),
                                _ => {}
                            },
                            Event::Resize(w, h) => layout::handle_resize(w, h),
                            _ => {}
                        }
                    }
                }
            }
            let _ = terminal::disable_raw_mode();
        });
        Self {
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for TurnInputListener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        while event::poll(Duration::from_millis(0)).unwrap_or(false) {
            let _ = event::read();
        }
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        crate::debug::log("listener", "TurnInputListener stop (join complete)");
    }
}

fn redraw_input(app: &App, buffer: &str, cursor: usize, pending_paste: Option<&String>) {
    layout::set_prompt_hint("");
    let badge = paste_badge_text(pending_paste);
    let palette = if buffer.starts_with('/') && pending_paste.is_none() {
        palette_lines(buffer)
    } else {
        Vec::new()
    };
    layout::redraw_footer_with_palette(app, buffer, cursor, badge.as_deref(), palette);
}

fn palette_lines(buffer: &str) -> Vec<String> {
    let filter = buffer.trim_start_matches('/');
    let commands = filter_slash_commands(filter);
    if commands.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    lines.push(format!("  \x1b[2m{}\x1b[0m", "\u{2500}".repeat(50)));
    for cmd in &commands {
        lines.push(format!(
            "  \x1b[38;2;255;127;80m/\x1b[0m\x1b[1m{:<12}\x1b[0m  \x1b[2m{}\x1b[0m",
            cmd.name, cmd.description
        ));
    }
    lines.push(format!("  \x1b[2m{}\x1b[0m", "\u{2500}".repeat(51)));
    lines
}

pub fn run_approval_modal(modal: &ApprovalModal) -> ApprovalChoice {
    let mut state = ApprovalCardState {
        selected: modal.selected,
        ..ApprovalCardState::default()
    };
    if !std::io::stdin().is_terminal() {
        return ApprovalChoice::Deny { comment: None };
    }

    terminal::enable_raw_mode().ok();

    let draw = |state: &ApprovalCardState, first: bool| {
        let _ = ScreenLayout::with_global(|layout| {
            let card = build_approval_card(modal, state, layout.width());
            let rows = center_card_in_viewport(
                &card,
                layout.width(),
                layout.chat_start(),
                layout.footer_start(),
            );
            if first {
                layout.set_tui_mode(TuiMode::ApprovalOverlay);
            }
            layout.set_modal_card_rows(rows);
            layout.render_frame()
        });
    };

    draw(&state, true);

    let choice = loop {
        if let Ok(Event::Key(key)) = event::read() {
            match key.code {
                KeyCode::Up if state.focus == ApprovalFocus::Pills => {
                    state.selected = state.selected.saturating_sub(1);
                    if state.selected == 3 {
                        state.focus = ApprovalFocus::DenyComment;
                    } else {
                        state.focus = ApprovalFocus::Pills;
                    }
                    draw(&state, false);
                }
                KeyCode::Down if state.focus == ApprovalFocus::Pills => {
                    state.selected = (state.selected + 1).min(3);
                    if state.selected == 3 {
                        state.focus = ApprovalFocus::DenyComment;
                    }
                    draw(&state, false);
                }
                KeyCode::Up if state.focus == ApprovalFocus::DenyComment => {
                    state.focus = ApprovalFocus::Pills;
                    draw(&state, false);
                }
                KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if state.selected == 3 {
                        break ApprovalChoice::Deny {
                            comment: non_empty_comment(&state.deny_comment),
                        };
                    }
                }
                KeyCode::Enter if state.focus == ApprovalFocus::DenyComment => {
                    state.deny_comment.insert(state.deny_cursor, '\n');
                    state.deny_cursor += 1;
                    draw(&state, false);
                }
                KeyCode::Enter => {
                    break match state.selected {
                        0 => ApprovalChoice::ApproveOnce,
                        1 => ApprovalChoice::ApproveSession,
                        2 => ApprovalChoice::ApproveAlways,
                        _ => ApprovalChoice::Deny {
                            comment: non_empty_comment(&state.deny_comment),
                        },
                    };
                }
                KeyCode::Esc => break ApprovalChoice::Deny { comment: None },
                KeyCode::Backspace if state.focus == ApprovalFocus::DenyComment => {
                    if state.deny_cursor > 0 {
                        let prev = prev_char_boundary(&state.deny_comment, state.deny_cursor);
                        state.deny_comment.drain(prev..state.deny_cursor);
                        state.deny_cursor = prev;
                        draw(&state, false);
                    }
                }
                KeyCode::Delete if state.focus == ApprovalFocus::DenyComment => {
                    if state.deny_cursor < state.deny_comment.len() {
                        let next = next_char_boundary(&state.deny_comment, state.deny_cursor);
                        state.deny_comment.drain(state.deny_cursor..next);
                        draw(&state, false);
                    }
                }
                KeyCode::Char(c)
                    if state.focus == ApprovalFocus::DenyComment
                        && !key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    state.deny_comment.insert(state.deny_cursor, c);
                    state.deny_cursor += c.len_utf8();
                    draw(&state, false);
                }
                _ => {}
            }
        }
    };

    layout::dismiss_approval_overlay();
    terminal::disable_raw_mode().ok();
    let _ = execute!(io::stdout(), Show);
    choice
}

fn non_empty_comment(comment: &str) -> Option<String> {
    let trimmed = comment.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(comment.to_string())
    }
}

pub fn run_model_picker(models: &[String], provider: &str) -> ModelPickerResult {
    if models.is_empty() {
        print_system_note("No models available. Check provider API key and try again.");
        return ModelPickerResult::Cancelled;
    }

    let mut selected = models
        .iter()
        .position(|m| m.contains("sonnet") || m.contains("gpt-4"))
        .unwrap_or(0);
    let change_idx = models.len();
    terminal::enable_raw_mode().ok();

    let result = loop {
        print_model_picker(models, selected, provider);

        if let Ok(Event::Key(key)) = event::read() {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if selected == 0 {
                        selected = change_idx;
                    } else {
                        selected -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if selected >= change_idx {
                        selected = 0;
                    } else {
                        selected += 1;
                    }
                }
                KeyCode::Enter => {
                    break if selected == change_idx {
                        ModelPickerResult::ChangeProvider
                    } else {
                        ModelPickerResult::Selected(models[selected].clone())
                    };
                }
                KeyCode::Esc => break ModelPickerResult::Cancelled,
                _ => {}
            }
        }
    };

    terminal::disable_raw_mode().ok();
    let _ = execute!(io::stdout(), Show);
    let _ = ScreenLayout::with_global(|layout| layout.render_frame());
    result
}

pub fn run_provider_picker(providers: &[String], current: &str) -> Option<String> {
    if providers.is_empty() {
        return None;
    }
    let mut selected = providers
        .iter()
        .position(|p| p == current)
        .unwrap_or(0);
    terminal::enable_raw_mode().ok();

    let result = loop {
        println!();
        println!("  \x1b[1m\x1b[37mSelect provider\x1b[0m");
        println!("  \x1b[2m{}\x1b[0m", "\u{2500}".repeat(50));
        for (i, provider) in providers.iter().enumerate() {
            if i == selected {
                println!("  \x1b[38;2;255;127;80m\x1b[1m\u{25b6} {provider}\x1b[0m");
            } else {
                println!("  \x1b[2m  {provider}\x1b[0m");
            }
        }
        println!("  \x1b[2m{}\x1b[0m", "\u{2500}".repeat(50));
        println!("  \x1b[2m\u{2191}\u{2193} navigate  Enter select  Esc cancel\x1b[0m");

        if let Ok(Event::Key(key)) = event::read() {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    selected = if selected == 0 {
                        providers.len() - 1
                    } else {
                        selected - 1
                    };
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1) % providers.len();
                }
                KeyCode::Enter => break Some(providers[selected].clone()),
                KeyCode::Esc => break None,
                _ => {}
            }
        }
    };

    terminal::disable_raw_mode().ok();
    let _ = execute!(io::stdout(), Show);
    let _ = ScreenLayout::with_global(|layout| layout.render_frame());
    result
}
