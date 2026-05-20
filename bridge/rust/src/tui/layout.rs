//! Alternate-screen frame buffer: pinned footer + scrollable transcript viewport.

use std::io::{self, Write};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::terminal::{Clear, ClearType};

use super::{
    build_prompt_grid, format_elapsed, format_prompt_border, prompt_screen_row, shorten_home,
    visible_len, App, PromptGrid, StatusMode, ToolStatus, BOLD, CORAL, CYAN, DIM, RESET,
};

const HEADER_ROWS: u16 = 2; // pinned title + blank spacer
const MIN_FOOTER_ROWS: u16 = 4; // top rule + prompt + bottom rule + status
const MIN_VIEWPORT_ROWS: u16 = 1;

/// Crossterm `MoveTo` is (column, row).
fn goto(out: &mut io::Stdout, row: u16, col: u16) -> io::Result<()> {
    execute!(out, MoveTo(col, row))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiMode {
    Chat,
    OnboardingOverlay,
    ApprovalOverlay,
}

static LAYOUT: OnceLock<Mutex<Option<ScreenLayout>>> = OnceLock::new();
static FOOTER_APP: OnceLock<Mutex<Option<FooterSnapshot>>> = OnceLock::new();
static USER_DISPLAY_NAME: OnceLock<Mutex<String>> = OnceLock::new();

// ─── Terminal session guard ───────────────────────────────────────────────────

pub struct TerminalSession {
    active: bool,
}

impl TerminalSession {
    pub fn enter() -> io::Result<Self> {
        let mut out = io::stdout();
        execute!(
            out,
            EnterAlternateScreen,
            Hide,
            EnableMouseCapture,
            Clear(ClearType::All),
            MoveTo(0, 0)
        )?;
        out.flush()?;
        Ok(Self { active: true })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if self.active {
            let mut out = io::stdout();
            let _ = execute!(
                out,
                Show,
                DisableMouseCapture,
                LeaveAlternateScreen
            );
            let _ = disable_raw_mode();
            let _ = out.flush();
            self.active = false;
        }
    }
}

// ─── Footer snapshot ──────────────────────────────────────────────────────────

#[derive(Clone)]
struct FooterSnapshot {
    model: String,
    context_pct: usize,
}

impl FooterSnapshot {
    fn from_app(app: &App) -> Self {
        Self {
            model: app.model.clone(),
            context_pct: app.context_pct,
        }
    }
}

fn footer_app_lock() -> &'static Mutex<Option<FooterSnapshot>> {
    FOOTER_APP.get_or_init(|| Mutex::new(None))
}

pub fn set_footer_app(app: &App) {
    *footer_app_lock().lock().unwrap() = Some(FooterSnapshot::from_app(app));
    let _ = ScreenLayout::with_global(|layout| {
        layout.session_id = app.session_id.clone();
    });
}

fn user_name_lock() -> &'static Mutex<String> {
    USER_DISPLAY_NAME.get_or_init(|| Mutex::new(String::new()))
}

pub fn set_user_display_name(name: &str) {
    *user_name_lock().lock().unwrap() = name.to_string();
}

fn user_display_name() -> String {
    user_name_lock().lock().unwrap().clone()
}

fn layout_lock() -> &'static Mutex<Option<ScreenLayout>> {
    LAYOUT.get_or_init(|| Mutex::new(None))
}

// ─── Screen layout ────────────────────────────────────────────────────────────

pub struct ScreenLayout {
    width: u16,
    height: u16,
    footer_start: u16,
    chat_start: u16,
    viewport_height: usize,
    transcript: Vec<String>,
    scroll_offset: usize,
    follow_tail: bool,
    cwd: String,
    session_id: String,
    status_mode: StatusMode,
    turn_start: Option<Instant>,
    prompt_hint: String,
    input_buffer: String,
    input_cursor: usize,
    paste_badge: String,
    prompt_line_count: u16,
    prompt_grid: PromptGrid,
    stream_buffer: String,
    agent_pending_lines: Vec<String>,
    last_stream_flush: Instant,
    stream_style: StreamStyle,
    tui_mode: TuiMode,
    modal_card_rows: Vec<(u16, String)>,
    slash_palette_lines: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StreamStyle {
    Plain,
    DimThinking,
    AgentText,
    InlineMarkdown,
}

fn overlay_active(mode: TuiMode) -> bool {
    matches!(mode, TuiMode::OnboardingOverlay | TuiMode::ApprovalOverlay)
}

impl ScreenLayout {
    pub fn init(app: &App, _tools: &[String], cwd: &str) -> io::Result<Self> {
        let (width, height) = super::get_terminal_size();
        let width = width.max(1).min(u16::MAX as usize) as u16;
        let height = height
            .max((HEADER_ROWS + MIN_FOOTER_ROWS + MIN_VIEWPORT_ROWS) as usize)
            .min(u16::MAX as usize) as u16;

        let mut layout = Self {
            width,
            height,
            footer_start: height.saturating_sub(MIN_FOOTER_ROWS),
            chat_start: HEADER_ROWS,
            viewport_height: 0,
            transcript: Vec::new(),
            scroll_offset: 0,
            follow_tail: true,
            cwd: cwd.to_string(),
            session_id: app.session_id.clone(),
            status_mode: StatusMode::Idle,
            turn_start: None,
            prompt_hint: String::new(),
            input_buffer: String::new(),
            input_cursor: 0,
            paste_badge: String::new(),
            prompt_line_count: 1,
            prompt_grid: build_prompt_grid("", width as usize, "", None),
            stream_buffer: String::new(),
            agent_pending_lines: Vec::new(),
            last_stream_flush: Instant::now(),
            stream_style: StreamStyle::Plain,
            tui_mode: TuiMode::Chat,
            modal_card_rows: Vec::new(),
            slash_palette_lines: Vec::new(),
        };

        layout.sync_terminal_size();
        layout.recompute_footer();
        layout.render_frame_with_input("", 0, None)?;
        Ok(layout)
    }

    pub fn sync_terminal_size(&mut self) {
        let (w, h) = super::get_terminal_size();
        self.width = w.max(1).min(u16::MAX as usize) as u16;
        self.height = h
            .max((HEADER_ROWS + MIN_FOOTER_ROWS + MIN_VIEWPORT_ROWS) as usize)
            .min(u16::MAX as usize) as u16;
        self.chat_start = HEADER_ROWS;
        self.recompute_footer();
    }

    pub fn tui_mode(&self) -> TuiMode {
        self.tui_mode
    }

    pub fn set_tui_mode(&mut self, mode: TuiMode) {
        self.tui_mode = mode;
        if mode == TuiMode::Chat {
            self.modal_card_rows.clear();
            self.set_input_state("", 0, None);
        }
        self.recompute_footer();
    }

    pub fn dismiss_modal_overlay(&mut self) {
        self.modal_card_rows.clear();
        self.tui_mode = TuiMode::Chat;
        self.recompute_footer();
    }

    pub fn set_modal_card_rows(&mut self, rows: Vec<(u16, String)>) {
        self.modal_card_rows = rows;
    }

    pub fn set_slash_palette_lines(&mut self, lines: Vec<String>) {
        self.slash_palette_lines = lines;
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn transcript_len(&self) -> usize {
        self.transcript.len()
    }

    pub fn install_global(self) {
        *layout_lock().lock().unwrap() = Some(self);
    }

    pub fn with_global<F, R>(f: F) -> Option<R>
    where
        F: FnOnce(&mut ScreenLayout) -> R,
    {
        let mut guard = layout_lock().lock().ok()?;
        guard.as_mut().map(f)
    }

    pub fn set_status_mode(&mut self, mode: StatusMode) {
        match &mode {
            StatusMode::Flowing | StatusMode::Executing(_) => {
                if !matches!(
                    self.status_mode,
                    StatusMode::Flowing | StatusMode::Executing(_)
                ) {
                    self.turn_start = Some(Instant::now());
                }
            }
            StatusMode::Idle => self.turn_start = None,
            StatusMode::Interrupted => {}
        }
        self.status_mode = mode;
    }

    pub fn chat_start(&self) -> u16 {
        self.chat_start
    }

    pub fn set_prompt_hint(&mut self, hint: &str) {
        self.prompt_hint = hint.to_string();
    }

    pub fn footer_start(&self) -> u16 {
        self.footer_start
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn content_width(&self) -> usize {
        super::chat_content_width(self.width as usize)
    }

    pub fn scroll_by(&mut self, delta_lines: i32) {
        if delta_lines < 0 {
            self.follow_tail = false;
            let up = (-delta_lines) as usize;
            let max_offset = self.max_scroll_offset();
            self.scroll_offset = (self.scroll_offset + up).min(max_offset);
        } else if delta_lines > 0 {
            let down = delta_lines as usize;
            self.scroll_offset = self.scroll_offset.saturating_sub(down);
            if self.scroll_offset == 0 {
                self.follow_tail = true;
            }
        }
        let _ = self.render_frame();
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width.max(1);
        self.height = height.max(HEADER_ROWS + MIN_FOOTER_ROWS + MIN_VIEWPORT_ROWS);
        self.recompute_footer();
        let max = self.max_scroll_offset();
        if self.scroll_offset > max {
            self.scroll_offset = max;
        }
        let _ = self.render_frame();
    }

    fn recompute_footer(&mut self) {
        let hint = match self.tui_mode {
            TuiMode::OnboardingOverlay => "(setup)",
            TuiMode::ApprovalOverlay => "(approval)",
            TuiMode::Chat => &self.prompt_hint,
        };
        self.prompt_grid = build_prompt_grid(
            &self.input_buffer,
            self.width as usize,
            hint,
            if self.paste_badge.is_empty() {
                None
            } else {
                Some(&self.paste_badge)
            },
        );
        self.prompt_line_count = self.prompt_grid.display_lines.len().max(1) as u16;
        // top rule + prompt + bottom rule + status
        let footer_rows = 1 + self.prompt_line_count + 1 + 1;
        let max_footer_start = self.height.saturating_sub(footer_rows);
        self.footer_start = max_footer_start.max(self.chat_start + MIN_VIEWPORT_ROWS);
        self.viewport_height = (self.footer_start - self.chat_start) as usize;
    }

    fn max_scroll_offset(&self) -> usize {
        self.transcript.len().saturating_sub(self.viewport_height)
    }

    fn push_transcript_line(&mut self, line: &str) {
        self.transcript.push(super::apply_chat_gutter(line));
    }

    fn is_blank_line(line: &str) -> bool {
        super::visible_len(line.trim()) == 0
    }

    fn last_transcript_line_is_blank(&self) -> bool {
        self.transcript
            .last()
            .is_some_and(|l| Self::is_blank_line(l))
    }

    fn push_transcript_line_collapse_blank(&mut self, line: &str) {
        if Self::is_blank_line(line) && self.last_transcript_line_is_blank() {
            return;
        }
        self.push_transcript_line(line);
    }

    fn replace_transcript_line(&mut self, index: usize, line: &str) -> io::Result<()> {
        if index < self.transcript.len() {
            self.transcript[index] = super::apply_chat_gutter(line);
        }
        self.render_frame()
    }

    fn splice_transcript_at(
        &mut self,
        index: usize,
        remove_count: usize,
        insert: &[String],
    ) -> io::Result<()> {
        if index > self.transcript.len() {
            return Ok(());
        }
        let end = (index + remove_count).min(self.transcript.len());
        self.transcript.drain(index..end);
        for (i, line) in insert.iter().enumerate() {
            self.transcript
                .insert(index + i, super::apply_chat_gutter(line));
        }
        self.render_frame()
    }

    fn push_lines_preformatted(&mut self, text: &str) {
        self.push_lines_preformatted_with_collapse(text, false);
    }

    fn push_lines_preformatted_collapse_blanks(&mut self, text: &str) {
        self.push_lines_preformatted_with_collapse(text, true);
    }

    fn push_lines_preformatted_with_collapse(&mut self, text: &str, collapse_blanks: bool) {
        if text.is_empty() {
            return;
        }
        for line in text.split_inclusive('\n') {
            let line = line.strip_suffix('\n').unwrap_or(line);
            if line.is_empty() && text.ends_with('\n') {
                if collapse_blanks {
                    self.push_transcript_line_collapse_blank("");
                } else {
                    self.push_transcript_line("");
                }
            } else if !line.is_empty() || text.ends_with('\n') {
                if collapse_blanks {
                    self.push_transcript_line_collapse_blank(line);
                } else {
                    self.push_transcript_line(line);
                }
            }
        }
        if self.follow_tail {
            self.scroll_offset = 0;
        } else {
            let max = self.max_scroll_offset();
            if self.scroll_offset > max {
                self.scroll_offset = max;
            }
        }
    }

    fn push_lines_internal(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let wrap_width = self.content_width();
        for line in text.split_inclusive('\n') {
            let line = line.strip_suffix('\n').unwrap_or(line);
            if line.is_empty() && text.ends_with('\n') {
                self.push_transcript_line("");
            } else if !line.is_empty() || text.ends_with('\n') {
                for wrapped in super::wrap_text_visible(line, wrap_width) {
                    self.push_transcript_line(&wrapped);
                }
            }
        }
        if self.follow_tail {
            self.scroll_offset = 0;
        } else {
            let max = self.max_scroll_offset();
            if self.scroll_offset > max {
                self.scroll_offset = max;
            }
        }
    }

    pub fn push_lines(&mut self, text: &str) -> io::Result<()> {
        self.push_lines_internal(text);
        self.render_frame()
    }

    pub fn append_stream_chunk(&mut self, chunk: &str) -> io::Result<()> {
        self.stream_buffer.push_str(chunk);
        if chunk.contains('\n') {
            self.flush_stream_buffer(false)?;
        }
        Ok(())
    }

    pub fn flush_stream_buffer(&mut self, flush_all: bool) -> io::Result<()> {
        let mut complete = String::new();
        if flush_all {
            complete = std::mem::take(&mut self.stream_buffer);
        } else if let Some(pos) = self.stream_buffer.rfind('\n') {
            complete = self.stream_buffer.drain(..=pos).collect();
        }

        if complete.is_empty() && !flush_all {
            return Ok(());
        }

        let content_w = self.content_width();
        let styled = match self.stream_style {
            StreamStyle::DimThinking => {
                let indent = 2usize;
                let max = content_w.saturating_sub(indent);
                let mut out = String::new();
                for line in super::wrap_text_visible(complete.trim_end_matches('\n'), max) {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(&format!("  {DIM}{line}{RESET}"));
                }
                out
            }
            StreamStyle::AgentText => {
                let indent = 2usize;
                let max = content_w.saturating_sub(indent);
                for line in complete.lines() {
                    self.agent_pending_lines.push(line.to_string());
                }
                let body_lines =
                    super::markdown::drain_pending_lines(&mut self.agent_pending_lines, max, flush_all);
                let mut out = String::new();
                for line in body_lines {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    if line.is_empty() {
                        continue;
                    }
                    out.push_str(&format!("  {line}"));
                }
                out
            }
            StreamStyle::InlineMarkdown | StreamStyle::Plain => {
                super::wrap_text_visible(complete.trim_end_matches('\n'), content_w).join("\n")
            }
        };

        if !styled.is_empty() {
            match self.stream_style {
                StreamStyle::AgentText => self.push_lines_preformatted_collapse_blanks(&styled),
                _ => self.push_lines_preformatted(&styled),
            };
        } else if flush_all && self.stream_style == StreamStyle::AgentText {
            let indent = 2usize;
            let max = content_w.saturating_sub(indent);
            let body_lines =
                super::markdown::drain_pending_lines(&mut self.agent_pending_lines, max, true);
            let mut out = String::new();
            for line in body_lines {
                if !out.is_empty() {
                    out.push('\n');
                }
                if !line.is_empty() {
                    out.push_str(&format!("  {line}"));
                }
            }
            if !out.is_empty() {
                self.push_lines_preformatted_collapse_blanks(&out);
            }
        }

        self.last_stream_flush = Instant::now();
        self.render_frame()
    }

    pub fn clear_transcript(&mut self, app: &App) -> io::Result<()> {
        self.transcript.clear();
        self.scroll_offset = 0;
        self.follow_tail = true;
        self.session_id = app.session_id.clone();
        self.render_frame()
    }

    pub fn set_input_state(&mut self, buffer: &str, cursor: usize, paste_badge: Option<&str>) {
        self.input_buffer = buffer.to_string();
        self.input_cursor = cursor.min(buffer.len());
        self.paste_badge = paste_badge.unwrap_or("").to_string();
        self.recompute_footer();
    }

    pub fn render_frame(&mut self) -> io::Result<()> {
        self.sync_terminal_size();
        let mut out = io::stdout();
        let chrome_dim = overlay_active(self.tui_mode);

        for row in 0..self.height {
            goto(&mut out, row, 0)?;
            execute!(out, Clear(ClearType::CurrentLine))?;
        }

        let (start, end) = visible_slice(
            self.transcript.len(),
            self.viewport_height,
            self.scroll_offset,
        );

        let transcript_count = end.saturating_sub(start);
        let palette_start = palette_start_row(
            self.footer_start,
            self.slash_palette_lines.len(),
            self.viewport_height,
        );
        for (i, line) in self.transcript[start..end].iter().enumerate() {
            goto(&mut out, self.chat_start + i as u16, 0)?;
            if chrome_dim {
                write!(out, "{DIM}{line}{RESET}")?;
            } else {
                write!(out, "{line}")?;
            }
        }

        for row in self.chat_start + transcript_count as u16..palette_start {
            goto(&mut out, row, 0)?;
            execute!(out, Clear(ClearType::CurrentLine))?;
            if chrome_dim {
                write!(out, "{DIM}{}", " ".repeat(self.width as usize))?;
            }
        }

        self.render_slash_palette(&mut out)?;

        self.render_pinned_header(&mut out)?;

        if chrome_dim && !self.modal_card_rows.is_empty() {
            super::modal::draw_modal_overlay(
                &mut out,
                self.width,
                self.chat_start,
                self.footer_start,
                &self.modal_card_rows,
            )?;
        }

        self.render_footer_rows(&mut out, chrome_dim)?;

        if chrome_dim {
            execute!(out, Hide)?;
        } else {
            let (prompt_row, cursor_col) =
                super::cursor_to_screen(&self.prompt_grid, self.input_cursor);
            let screen_row = prompt_screen_row(self.footer_start, prompt_row);
            goto(&mut out, screen_row, cursor_col as u16)?;
            execute!(out, Show)?;
        }
        out.flush()
    }

    pub fn render_frame_with_input(
        &mut self,
        input_buffer: &str,
        cursor: usize,
        paste_badge: Option<&str>,
    ) -> io::Result<()> {
        self.set_input_state(input_buffer, cursor, paste_badge);
        self.render_frame()
    }

    pub fn render_footer_only(&mut self) -> io::Result<()> {
        let mut out = io::stdout();
        let chrome_dim = overlay_active(self.tui_mode);
        self.render_footer_rows(&mut out, chrome_dim)?;
        if !chrome_dim {
            let (prompt_row, cursor_col) =
                super::cursor_to_screen(&self.prompt_grid, self.input_cursor);
            let screen_row = prompt_screen_row(self.footer_start, prompt_row);
            goto(&mut out, screen_row, cursor_col as u16)?;
            execute!(out, Show)?;
        } else {
            execute!(out, Hide)?;
        }
        out.flush()
    }

    fn render_pinned_header(&self, out: &mut io::Stdout) -> io::Result<()> {
        let title = format!(
            "{}{CORAL}{BOLD}Zero-Agent{RESET}{DIM} · session: {}{RESET}",
            super::apply_chat_gutter(""),
            self.session_id
        );
        goto(out, 0, 0)?;
        write!(out, "{title}")?;
        goto(out, 1, 0)?;
        execute!(out, Clear(ClearType::CurrentLine))?;
        Ok(())
    }

    fn render_footer_rows(&self, out: &mut io::Stdout, dim: bool) -> io::Result<()> {
        let snap = footer_app_lock()
            .lock()
            .unwrap()
            .clone()
            .unwrap_or(FooterSnapshot {
                model: "unknown".into(),
                context_pct: 0,
            });

        let status_line = self.format_status_line_snapshot(&snap);
        let status_out = if dim {
            format!("{DIM}{status_line}{RESET}")
        } else {
            status_line
        };
        let top_border = format_prompt_border(self.width as usize);
        let bottom_border = top_border.clone();

        for row in self.footer_start..self.height {
            goto(out, row, 0)?;
            execute!(out, Clear(ClearType::CurrentLine))?;
        }

        goto(out, self.footer_start, 0)?;
        if dim {
            write!(out, "{DIM}{top_border}{RESET}")?;
        } else {
            write!(out, "{top_border}")?;
        }

        for (i, line) in self.prompt_grid.display_lines.iter().enumerate() {
            goto(out, self.footer_start + 1 + i as u16, 0)?;
            if dim {
                write!(out, "{DIM}{line}{RESET}")?;
            } else {
                write!(out, "{line}")?;
            }
        }

        let bottom_row = self.footer_start + 1 + self.prompt_line_count;
        goto(out, bottom_row, 0)?;
        if dim {
            write!(out, "{DIM}{bottom_border}{RESET}")?;
        } else {
            write!(out, "{bottom_border}")?;
        }

        goto(out, self.height - 1, 0)?;
        write!(out, "{status_out}")?;
        Ok(())
    }

    fn format_status_line_snapshot(&self, snap: &FooterSnapshot) -> String {
        let ctx_window = "0/128K";
        let ctx_pct = format!("CTX {}%", snap.context_pct);
        let cwd_short = shorten_home(&self.cwd);
        let cwd_suffix = if self.width >= 76 {
            format!(" {DIM}|{RESET} {cwd_short}")
        } else {
            String::new()
        };
        let turn_elapsed = self
            .turn_start
            .map(|t| format!("{DIM}|{RESET} {} ", format_elapsed(t.elapsed())))
            .unwrap_or_default();

        match &self.status_mode {
            StatusMode::Executing(tool) => format!(
                " {CYAN}\u{2695}{RESET} {BOLD}{}{RESET} {DIM}|{RESET} {ctx_window} {DIM}|{RESET} {ctx_pct}{turn_elapsed}{DIM}|{RESET} running {tool}…{cwd_suffix}",
                snap.model
            ),
            StatusMode::Flowing => format!(
                " {CYAN}\u{2695}{RESET} {BOLD}{}{RESET} {DIM}|{RESET} {ctx_window} {DIM}|{RESET} {ctx_pct}{turn_elapsed}{DIM}|{RESET} thinking…{cwd_suffix}",
                snap.model
            ),
            StatusMode::Interrupted => format!(
                " {CYAN}\u{2695}{RESET} {BOLD}{}{RESET} {DIM}|{RESET} {ctx_window} {DIM}|{RESET} {ctx_pct}{turn_elapsed}{DIM}|{RESET} stopped{cwd_suffix}",
                snap.model
            ),
            StatusMode::Idle if overlay_active(self.tui_mode) => format!(
                " {CYAN}\u{2695}{RESET} {BOLD}{}{RESET} {DIM}|{RESET} {DIM}{}{RESET}",
                snap.model,
                if self.tui_mode == TuiMode::OnboardingOverlay {
                    "setup"
                } else {
                    "approval"
                }
            ),
            StatusMode::Idle => format!(
                " {CYAN}\u{2695}{RESET} {BOLD}{}{RESET} {DIM}|{RESET} {ctx_window} {DIM}|{RESET} {ctx_pct}{cwd_suffix}",
                snap.model
            ),
        }
    }

    fn render_slash_palette(&self, out: &mut io::Stdout) -> io::Result<()> {
        if self.slash_palette_lines.is_empty() {
            return Ok(());
        }
        let palette_rows = self
            .slash_palette_lines
            .len()
            .min(self.viewport_height.saturating_sub(1));
        let start_row = palette_start_row(
            self.footer_start,
            self.slash_palette_lines.len(),
            self.viewport_height,
        );
        let content_w = self.content_width();
        for (i, line) in self.slash_palette_lines.iter().take(palette_rows).enumerate() {
            let wrapped = super::wrap_text_visible(line, content_w);
            let display = super::apply_chat_gutter(wrapped.first().map(String::as_str).unwrap_or(""));
            goto(out, start_row + i as u16, 0)?;
            write!(out, "{display}")?;
        }
        Ok(())
    }
}

/// Row where the slash palette begins (bottom-aligned above footer).
pub fn palette_start_row(footer_start: u16, line_count: usize, viewport_height: usize) -> u16 {
    if line_count == 0 {
        return footer_start;
    }
    let palette_rows = line_count.min(viewport_height.saturating_sub(1));
    footer_start.saturating_sub(palette_rows as u16)
}

/// Compute visible transcript line range. `scroll_offset` is lines from bottom (0 = latest).
pub fn visible_slice(total_lines: usize, viewport_height: usize, scroll_offset: usize) -> (usize, usize) {
    if total_lines == 0 || viewport_height == 0 {
        return (0, 0);
    }
    let max_offset = total_lines.saturating_sub(viewport_height);
    let offset = scroll_offset.min(max_offset);
    let end = total_lines.saturating_sub(offset);
    let start = end.saturating_sub(viewport_height);
    (start, end)
}

pub fn with_global_for_overlay<F>(f: F) -> Option<io::Result<()>>
where
    F: FnOnce(&mut ScreenLayout) -> io::Result<()>,
{
    ScreenLayout::with_global(|layout| f(layout))
}

fn layout_is_active() -> bool {
    layout_lock().lock().map(|g| g.is_some()).unwrap_or(false)
}

pub fn set_tui_mode(mode: TuiMode) {
    let _ = ScreenLayout::with_global(|layout| {
        layout.set_tui_mode(mode);
        layout.render_frame()
    });
}

pub fn get_tui_mode() -> TuiMode {
    ScreenLayout::with_global(|layout| layout.tui_mode())
        .unwrap_or(TuiMode::Chat)
}

pub fn dismiss_approval_overlay() {
    let _ = ScreenLayout::with_global(|layout| {
        layout.dismiss_modal_overlay();
        layout.render_frame()
    });
}

pub fn set_modal_overlay(card_rows: Vec<(u16, String)>) {
    let _ = ScreenLayout::with_global(|layout| {
        layout.set_modal_card_rows(card_rows);
        layout.render_frame()
    });
}

pub fn enter_chat_mode(app: &App, _tools: &[String], cwd: &str) {
    let _ = ScreenLayout::with_global(|layout| {
        layout.set_tui_mode(TuiMode::Chat);
        layout.cwd = cwd.to_string();
        layout.clear_transcript(app)
    });
}

// ─── Global transcript API ────────────────────────────────────────────────────

pub fn append_transcript(text: &str) {
    let used = ScreenLayout::with_global(|layout| {
        layout.stream_style = StreamStyle::InlineMarkdown;
        layout.append_stream_chunk(text)
    })
    .is_some();
    if !used && !layout_is_active() {
        print!("{text}");
        let _ = io::stdout().flush();
    }
}

pub fn flush_stream() {
    let _ = ScreenLayout::with_global(|layout| layout.flush_stream_buffer(true));
}

pub fn append_line(line: &str) {
    let used = ScreenLayout::with_global(|layout| layout.push_lines(&format!("{line}\n"))).is_some();
    if !used && !layout_is_active() {
        println!("{line}");
    }
}

pub fn set_status_mode(mode: StatusMode) {
    let _ = ScreenLayout::with_global(|layout| {
        layout.set_status_mode(mode);
        if matches!(
            layout.tui_mode(),
            TuiMode::OnboardingOverlay | TuiMode::ApprovalOverlay
        ) {
            layout.render_frame()
        } else {
            layout.render_footer_only()
        }
    });
}

pub fn redraw_footer(app: &App, input_buffer: &str, cursor: usize, paste_badge: Option<&str>) {
    redraw_footer_with_palette(app, input_buffer, cursor, paste_badge, Vec::new());
}

pub fn redraw_footer_with_palette(
    app: &App,
    input_buffer: &str,
    cursor: usize,
    paste_badge: Option<&str>,
    palette_lines: Vec<String>,
) {
    set_footer_app(app);
    let _ = ScreenLayout::with_global(|layout| {
        layout.set_slash_palette_lines(palette_lines);
        layout.set_input_state(input_buffer, cursor, paste_badge);
        layout.render_frame()
    });
}

pub fn refresh_footer(input_buffer: &str, cursor: usize, paste_badge: Option<&str>) {
    let _ = ScreenLayout::with_global(|layout| {
        layout.set_input_state(input_buffer, cursor, paste_badge);
        layout.render_footer_only()
    });
}

pub fn set_prompt_hint(hint: &str) {
    let _ = ScreenLayout::with_global(|layout| layout.set_prompt_hint(hint));
}

pub fn clear_transcript(app: &App, _tools: &[String], cwd: &str) {
    let _ = ScreenLayout::with_global(|layout| {
        layout.cwd = cwd.to_string();
        layout.clear_transcript(app)
    });
}

pub fn scroll_by(delta: i32) {
    let _ = ScreenLayout::with_global(|layout| layout.scroll_by(delta));
}

pub fn handle_resize(width: u16, height: u16) {
    let _ = ScreenLayout::with_global(|layout| layout.resize(width, height));
}

// ─── High-level transcript blocks ────────────────────────────────────────────

pub fn append_user_block(text: &str, terminal_width: usize) {
    let content_w = super::chat_content_width(terminal_width);
    let name = user_display_name();
    let title = if name.is_empty() { "You".to_string() } else { name };
    let mut block = format!("\n  {CORAL}{BOLD}{title}{RESET}\n");
    for line in text.lines() {
        for wrapped in super::wrap_text_visible(line, content_w) {
            block.push_str(&format!("  {wrapped}\n"));
        }
    }
    block.push('\n');
    let _ = ScreenLayout::with_global(|layout| layout.push_lines(&block));
}

pub fn append_agent_text(text: &str) {
    let _ = ScreenLayout::with_global(|layout| {
        let cw = layout.content_width();
        let label = format!("  {CYAN}{BOLD}ZERO{RESET}  ");
        let mut block = String::new();
        if text.is_empty() {
            block.push('\n');
        } else {
            block.push('\n');
            let rendered = super::markdown::render_markdown_text(text, cw.saturating_sub(8));
            let mut first = true;
            for line in rendered {
                if first {
                    block.push_str(&label);
                    block.push_str(&line);
                    first = false;
                } else {
                    block.push_str(&format!("        {line}"));
                }
                block.push('\n');
            }
        }
        layout.push_lines_internal(&block);
    });
}

pub fn append_agent_start() {
    let _ = ScreenLayout::with_global(|layout| {
        layout.stream_style = StreamStyle::AgentText;
        layout.agent_pending_lines.clear();
        layout.push_lines(&format!("\n  {CYAN}{BOLD}ZERO{RESET}  "))
    });
}

pub fn append_agent_chunk(chunk: &str) {
    let _ = ScreenLayout::with_global(|layout| {
        layout.stream_style = StreamStyle::AgentText;
        let _ = layout.append_stream_chunk(chunk);
    });
}

pub fn append_agent_end() {
    let _ = ScreenLayout::with_global(|layout| {
        let _ = layout.flush_stream_buffer(true);
        layout.stream_style = StreamStyle::Plain;
        layout.agent_pending_lines.clear();
        if !layout.last_transcript_line_is_blank() {
            layout.push_lines_preformatted_collapse_blanks("\n");
        }
        layout.push_lines_preformatted_collapse_blanks("\n");
    });
}

pub fn append_thinking_start() {
    let _ = ScreenLayout::with_global(|layout| {
        layout.stream_style = StreamStyle::DimThinking;
        layout.push_lines(&format!("\n  \x1b[38;5;67m{BOLD}Thinking:{RESET}"))
    });
}

pub fn append_thinking_chunk(chunk: &str) {
    let _ = ScreenLayout::with_global(|layout| {
        layout.stream_style = StreamStyle::DimThinking;
        let _ = layout.append_stream_chunk(chunk);
    });
}

pub fn append_thinking_end() {
    let _ = ScreenLayout::with_global(|layout| {
        let _ = layout.flush_stream_buffer(true);
        layout.stream_style = StreamStyle::Plain;
        layout.push_lines("\n")
    });
}

const BG_YELLOW: &str = "\x1b[48;5;214m\x1b[30m";
const BG_GREEN: &str = "\x1b[48;5;28m\x1b[97m";
const BG_RED: &str = "\x1b[41m\x1b[97m";

fn format_tool_call_lines(
    name: &str,
    args: &str,
    status: &ToolStatus,
    elapsed: Option<std::time::Duration>,
    content_width: usize,
) -> Vec<String> {
    let label = super::tool_display_label(name);
    let bg = match status {
        ToolStatus::Running => BG_YELLOW,
        ToolStatus::Success => BG_GREEN,
        ToolStatus::Error => BG_RED,
    };
    let elapsed_str = elapsed
        .map(|d| format!("  ({})", format_elapsed(d)))
        .unwrap_or_default();
    let pill = format!("{bg} {label} {RESET}");
    let prefix_visible = 2 + super::visible_len(&pill) + 2;
    let args_budget = content_width.saturating_sub(prefix_visible).max(8);
    let wrapped_args = super::wrap_text_visible(args, args_budget);
    let mut lines = Vec::new();
    if wrapped_args.is_empty() {
        lines.push(format!("  {pill}  {DIM}`{elapsed_str}`{RESET}"));
        return lines;
    }
    for (i, segment) in wrapped_args.iter().enumerate() {
        if i == 0 {
            lines.push(format!(
                "  {pill}  {DIM}`{segment}`{elapsed_str}{RESET}"
            ));
        } else {
            let indent = " ".repeat(prefix_visible);
            lines.push(format!("  {indent}{DIM}`{segment}`{RESET}"));
        }
    }
    lines
}

fn format_tool_call_line(
    name: &str,
    args_preview: &str,
    status: &ToolStatus,
    elapsed: Option<std::time::Duration>,
) -> String {
    format_tool_call_lines(name, args_preview, status, elapsed, 80)
        .join("\n")
}

pub fn begin_tool_call(name: &str, args: &str) -> (usize, usize) {
    ScreenLayout::with_global(|layout| {
        let cw = layout.content_width();
        let lines = format_tool_call_lines(name, args, &ToolStatus::Running, None, cw);
        let idx = layout.transcript.len();
        for line in &lines {
            layout.push_transcript_line(line);
        }
        let _ = layout.render_frame();
        crate::debug::log("tui", &format!("begin_tool_call idx={idx} name={name}"));
        (idx, lines.len())
    })
    .unwrap_or((0, 1))
}

pub fn update_tool_call(
    index: usize,
    line_count: usize,
    name: &str,
    args: &str,
    status: &ToolStatus,
    elapsed: Option<std::time::Duration>,
) {
    let _ = ScreenLayout::with_global(|layout| {
        let cw = layout.content_width();
        let lines = format_tool_call_lines(name, args, status, elapsed, cw);
        let _ = layout.splice_transcript_at(index, line_count, &lines);
        crate::debug::log(
            "tui",
            &format!("update_tool_call idx={index} name={name} status={status:?}"),
        );
    });
}

pub fn append_tool_call(name: &str, args: &str, status: &ToolStatus, elapsed: Option<std::time::Duration>) {
    let _ = ScreenLayout::with_global(|layout| {
        let cw = layout.content_width();
        let lines = format_tool_call_lines(name, args, status, elapsed, cw);
        for line in lines {
            layout.push_transcript_line(&line);
        }
    });
}

pub fn append_tool_result(output: &str, log_path: &str) {
    let _ = ScreenLayout::with_global(|layout| {
        let cw = layout.content_width();
        let text_budget = cw.saturating_sub(4);
        for line in output.lines().take(super::TOOL_RESULT_MAX_LINES) {
            for wrapped in super::wrap_text_visible(line, text_budget) {
                layout.push_transcript_line(&format!("    {DIM}{wrapped}{RESET}"));
            }
        }
        if !log_path.is_empty() {
            layout.push_transcript_line(&format!("    {DIM}... see {log_path}{RESET}"));
        }
        layout.push_transcript_line("");
        let _ = layout.render_frame();
    });
}

pub fn append_diff_lines(lines: &[String]) {
    let _ = ScreenLayout::with_global(|layout| {
        for line in lines {
            let _ = layout.push_lines(&format!("{line}\n"));
        }
        layout.push_transcript_line("");
    });
}

pub fn append_system_note(text: &str) {
    append_line(&format!("  {DIM}\x1b[3m{text}\x1b[0m{RESET}"));
}

pub fn format_footer_status(app: &App, cwd: &str, mode: &StatusMode) -> String {
    let snap = FooterSnapshot::from_app(app);
    let layout = ScreenLayout {
        width: 80,
        height: 24,
        footer_start: 20,
        chat_start: HEADER_ROWS,
        viewport_height: 18,
        transcript: Vec::new(),
        scroll_offset: 0,
        follow_tail: true,
        cwd: cwd.to_string(),
        session_id: app.session_id.clone(),
        status_mode: mode.clone(),
        turn_start: None,
        prompt_hint: String::new(),
        input_buffer: String::new(),
        input_cursor: 0,
        paste_badge: String::new(),
        prompt_line_count: 1,
        prompt_grid: build_prompt_grid("", 80, "", None),
        stream_buffer: String::new(),
        agent_pending_lines: Vec::new(),
        last_stream_flush: Instant::now(),
        stream_style: StreamStyle::Plain,
        tui_mode: TuiMode::Chat,
        modal_card_rows: Vec::new(),
        slash_palette_lines: Vec::new(),
    };
    layout.format_status_line_snapshot(&snap)
}

/// Compute safe footer_start given terminal height and footer row count.
pub fn compute_footer_start(height: u16, footer_rows: u16) -> u16 {
    let max_start = height.saturating_sub(footer_rows);
    max_start.max(MIN_VIEWPORT_ROWS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{chat_content_width, visible_len, wrap_text_visible, CHAT_GUTTER};

    #[test]
    fn footer_start_leaves_viewport_room() {
        let start = compute_footer_start(42, 2);
        assert!(start >= MIN_VIEWPORT_ROWS);
        assert!(start + 2 <= 42);
    }

    #[test]
    fn footer_start_clamps_on_tiny_height() {
        let start = compute_footer_start(3, 2);
        assert!(start >= MIN_VIEWPORT_ROWS);
    }

    #[test]
    fn visible_slice_at_bottom_shows_latest() {
        let (start, end) = visible_slice(100, 20, 0);
        assert_eq!(start, 80);
        assert_eq!(end, 100);
    }

    #[test]
    fn visible_slice_scrolled_up() {
        let (start, end) = visible_slice(100, 20, 10);
        assert_eq!(start, 70);
        assert_eq!(end, 90);
    }

    #[test]
    fn visible_slice_clamps_offset() {
        let (start, end) = visible_slice(15, 20, 100);
        assert_eq!(start, 0);
        assert_eq!(end, 15);
    }

    #[test]
    fn footer_status_idle_contains_model_and_cwd() {
        let app = App::new();
        let line = format_footer_status(&app, "/home/user/proj", &StatusMode::Idle);
        assert!(line.contains("0/128K"));
        assert!(line.contains("CTX"));
        assert!(!line.contains("idle"));
        assert!(!line.contains('\u{2588}')); // █
        assert!(!line.contains('\u{2591}')); // ░
        assert!(!has_elapsed_timer(&line));
    }

    #[test]
    fn footer_status_flowing_has_turn_timer() {
        let app = App::new();
        let mut layout = test_layout(StatusMode::Flowing);
        layout.turn_start = Some(Instant::now());
        let snap = FooterSnapshot::from_app(&app);
        let line = layout.format_status_line_snapshot(&snap);
        assert!(line.contains("thinking"));
        assert!(has_elapsed_timer(&line));
        assert!(line.contains("CTX"));
    }

    #[test]
    fn status_uses_ctx_percent_not_bar() {
        let app = App::new();
        let line = format_footer_status(&app, "/home/user/proj", &StatusMode::Idle);
        assert!(line.contains("CTX 0%"));
        assert!(!line.contains('\u{2588}'));
        assert!(!line.contains('\u{2591}'));
    }

    #[test]
    fn stream_flush_does_not_split_mid_line() {
        let mut layout = test_layout(StatusMode::Idle);
        layout.stream_style = StreamStyle::DimThinking;
        let chunk = "The user said hello and this is a longer reasoning string without any newline character until we explicitly flush the buffer at end.";
        layout.append_stream_chunk(chunk).unwrap();
        assert_eq!(layout.transcript_len(), 0);
        layout.flush_stream_buffer(true).unwrap();
        let lines = layout.transcript_snapshot();
        assert!(!lines.is_empty());
        let cw = layout.content_width();
        let min_width = (cw as f64 * 0.8) as usize;
        let longest = lines
            .iter()
            .map(|l| super::visible_len(l))
            .max()
            .unwrap_or(0);
        assert!(
            longest >= min_width || lines.len() == 1,
            "expected wrapped lines to use viewport width, longest={longest} cw={cw}"
        );
    }

    #[test]
    fn user_block_inserts_trailing_blank_line() {
        let content_w = chat_content_width(80);
        let mut block = format!("\n  {CORAL}{BOLD}Spencer{RESET}\n");
        for wrapped in wrap_text_visible("Hello", content_w) {
            block.push_str(&format!("  {wrapped}\n"));
        }
        block.push('\n');
        assert!(block.ends_with("\n\n") || block.ends_with(&format!("  Hello\n\n")));
    }

    #[test]
    fn agent_stream_body_honors_content_indent() {
        let mut layout = test_layout(StatusMode::Idle);
        layout.stream_style = StreamStyle::AgentText;
        layout
            .append_stream_chunk("Hey Spencer! This is a response that should honor gutter indent.")
            .unwrap();
        layout.flush_stream_buffer(true).unwrap();
        let lines = layout.transcript_snapshot();
        assert!(!lines.is_empty());
        for line in &lines {
            let prefix = visible_len(line) - visible_len(line.trim_start());
            assert!(prefix >= CHAT_GUTTER + 2, "line should have gutter + indent: {line}");
        }
    }

    #[test]
    fn slash_palette_start_row_above_footer() {
        let footer = 25u16;
        let lines = 17usize;
        let viewport = 23usize;
        let start = palette_start_row(footer, lines, viewport);
        let rows = lines.min(viewport.saturating_sub(1));
        assert_eq!(start + rows as u16, footer);
    }

    #[test]
    fn slash_palette_empty_clears_state() {
        assert_eq!(palette_start_row(25, 0, 23), 25);
    }

    #[test]
    fn user_block_no_box_chars() {
        let content_w = chat_content_width(80);
        let mut block = format!("\n  {CORAL}{BOLD}Spencer{RESET}\n");
        for wrapped in wrap_text_visible("Hello world", content_w) {
            block.push_str(&format!("  {wrapped}\n"));
        }
        assert!(!block.contains('\u{250c}'));
        assert!(!block.contains('\u{2502}'));
        assert!(!block.contains('\u{2514}'));
    }

    fn has_elapsed_timer(s: &str) -> bool {
        s.split_whitespace().any(|w| {
            (w.ends_with('s')
                && w.contains('.')
                && w.trim_end_matches('s').parse::<f64>().is_ok())
                || (w.ends_with("ms") && w.trim_end_matches("ms").parse::<u64>().is_ok())
        })
    }

    #[test]
    fn tool_result_trailing_blank_line() {
        let mut layout = test_layout(StatusMode::Idle);
        let text_budget = layout.content_width().saturating_sub(4);
        for wrapped in wrap_text_visible("output line", text_budget) {
            layout.push_transcript_line(&format!("    {DIM}{wrapped}{RESET}"));
        }
        layout.push_transcript_line("");
        let lines = layout.transcript_snapshot();
        assert!(lines.len() >= 2);
        assert!(is_blank_line(lines.last().unwrap()));
    }

    #[test]
    fn agent_stream_inline_not_applied_per_chunk() {
        let mut layout = test_layout(StatusMode::Idle);
        layout.stream_style = StreamStyle::AgentText;
        layout.append_stream_chunk("Now let me ").unwrap();
        layout.append_stream_chunk("**continue**\n").unwrap();
        layout.flush_stream_buffer(true).unwrap();
        let joined = layout.transcript_snapshot().join("\n");
        const BOLD: &str = "\x1b[1m";
        const RESET: &str = "\x1b[0m";
        assert!(
            !joined.contains(&format!("{BOLD}Now let me {RESET}")),
            "partial chunks should not bold mid-sentence: {joined}"
        );
        assert!(joined.contains(BOLD), "complete markdown should still bold");
    }

    fn is_blank_line(line: &str) -> bool {
        visible_len(line.trim()) == 0
    }

    #[test]
    fn agent_stream_collapses_double_blank_lines() {
        let mut layout = test_layout(StatusMode::Idle);
        layout.stream_style = StreamStyle::AgentText;
        layout.append_stream_chunk("Hello\n\nStarting").unwrap();
        layout.flush_stream_buffer(true).unwrap();
        let lines = layout.transcript_snapshot();
        let blank_runs = lines
            .windows(2)
            .filter(|w| is_blank_line(&w[0]) && is_blank_line(&w[1]))
            .count();
        assert_eq!(blank_runs, 0, "should not have consecutive blank lines");
    }

    #[test]
    fn tool_call_updates_in_place() {
        let mut layout = test_layout(StatusMode::Idle);
        let args = r#"{"command":"pwd"}"#;
        let cw = layout.content_width();
        let running = format_tool_call_lines("shell", args, &ToolStatus::Running, None, cw);
        let idx = layout.transcript.len();
        for line in &running {
            layout.push_transcript_line(line);
        }
        let done = format_tool_call_lines(
            "shell",
            args,
            &ToolStatus::Success,
            Some(std::time::Duration::from_millis(17)),
            cw,
        );
        layout
            .splice_transcript_at(idx, running.len(), &done)
            .unwrap();
        let lines = layout.transcript_snapshot();
        assert_eq!(lines.len(), done.len());
        assert!(lines[0].contains("shell"));
        assert!(lines.iter().any(|l| l.contains("17ms")));
    }

    #[test]
    fn tool_call_wraps_long_args() {
        let long = format!(
            r#"{{"path":"/Users/spencer/GitHub/zero-agent/{}"}}"#,
            "nested/deep/path/to/file.rs"
        );
        let lines = format_tool_call_lines("read", &long, &ToolStatus::Running, None, 60);
        assert!(lines.len() > 1 || lines[0].len() > 40);
        let joined = lines.join("\n");
        assert!(!joined.contains("..."));
        assert!(joined.contains("/Users/spencer"));
    }

    #[test]
    fn tool_result_indented_without_pipe() {
        let mut layout = test_layout(StatusMode::Idle);
        layout.push_transcript_line(&format!("    {DIM}/tmp/project{RESET}"));
        let lines = layout.transcript_snapshot();
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].contains('\u{2502}'));
        assert!(lines[0].contains("/tmp/project"));
    }

    fn test_layout(mode: StatusMode) -> ScreenLayout {
        ScreenLayout {
            width: 100,
            height: 30,
            footer_start: 25,
            chat_start: HEADER_ROWS,
            viewport_height: 23,
            transcript: Vec::new(),
            scroll_offset: 0,
            follow_tail: true,
            cwd: "/home/user/proj".into(),
            session_id: "test".into(),
            status_mode: mode,
            turn_start: None,
            prompt_hint: String::new(),
            input_buffer: String::new(),
            input_cursor: 0,
            paste_badge: String::new(),
            prompt_line_count: 1,
            prompt_grid: build_prompt_grid("", 100, "", None),
            stream_buffer: String::new(),
            agent_pending_lines: Vec::new(),
            last_stream_flush: Instant::now(),
            stream_style: StreamStyle::Plain,
            tui_mode: TuiMode::Chat,
            modal_card_rows: Vec::new(),
            slash_palette_lines: Vec::new(),
        }
    }

    impl ScreenLayout {
        #[cfg(test)]
        fn transcript_snapshot(&self) -> Vec<String> {
            self.transcript.clone()
        }
    }
}
