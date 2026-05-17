use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::App;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let input_text = format!("{} ", app.input);
    let cursor_pos = app.input_cursor;

    let mut spans = vec![
        Span::styled("> ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ];

    // Input text before cursor
    if cursor_pos > 0 {
        spans.push(Span::raw(&input_text[..cursor_pos]));
    }

    // Cursor
    if cursor_pos < input_text.len() {
        spans.push(Span::styled(
            &input_text[cursor_pos..cursor_pos + 1],
            Style::default().bg(Color::White).fg(Color::Black),
        ));
        // Text after cursor
        if cursor_pos + 1 < input_text.len() {
            spans.push(Span::raw(&input_text[cursor_pos + 1..]));
        }
    } else {
        spans.push(Span::styled(" ", Style::default().bg(Color::White).fg(Color::Black)));
    }

    let paragraph = Paragraph::new(Line::from(spans))
        .block(Block::default().borders(Borders::ALL).title("Input (Enter to send, Tab for activity)"));

    f.render_widget(paragraph, area);
}
