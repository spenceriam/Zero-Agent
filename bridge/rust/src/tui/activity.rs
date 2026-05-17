use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use super::{App, ToolStatus};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = Vec::new();

    // Show recent activity entries
    let start = app.activity_log.len().saturating_sub(area.height as usize);

    for entry in app.activity_log.iter().skip(start) {
        let icon = match entry.status {
            ToolStatus::Running => "⏳",
            ToolStatus::Success => "✓",
            ToolStatus::Error => "✗",
        };
        let color = match entry.status {
            ToolStatus::Running => Color::Yellow,
            ToolStatus::Success => Color::Green,
            ToolStatus::Error => Color::Red,
        };

        let elapsed_str = entry.elapsed
            .map(|d| format!(" ({:.1}s)", d.as_secs_f64()))
            .unwrap_or_default();

        lines.push(Line::from(vec![
            Span::styled(format!("  {} {}{}", icon, entry.tool_name, elapsed_str), Style::default().fg(color)),
        ]));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No activity yet",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Activity"))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}
