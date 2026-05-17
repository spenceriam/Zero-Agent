use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::ApprovalRequest;

pub fn render(f: &mut Frame, area: Rect, approval: &ApprovalRequest) {
    let popup_area = centered_rect(60, 50, area);

    f.render_widget(Clear, popup_area);

    let risk_color = match approval.risk_level.as_str() {
        "Safe" => Color::Green,
        "Mutating" => Color::Yellow,
        "Destructive" => Color::Red,
        "Blocked" => Color::Red,
        _ => Color::White,
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("Tool Approval Required", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Tool: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&approval.tool_name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Risk: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&approval.risk_level, Style::default().fg(risk_color)),
        ]),
        Line::from(""),
        Line::from(Span::styled(&approval.description, Style::default())),
        Line::from(""),
        Line::from(Span::styled("Input:", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(&approval.input_preview, Style::default().fg(Color::Cyan))),
        Line::from(""),
        Line::from(vec![
            Span::styled("[y]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("es  "),
            Span::styled("[n]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw("o  "),
            Span::styled("[a]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("lways  "),
            Span::styled("[v]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("iew details  "),
            Span::styled("[Esc]", Style::default().fg(Color::DarkGray)),
            Span::raw(" cancel"),
        ]),
    ];

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Approval").border_style(Style::default().fg(Color::Yellow)))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
