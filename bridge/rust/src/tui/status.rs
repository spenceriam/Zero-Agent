use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::App;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let status = Line::from(vec![
        Span::styled(format!(" model: {} ", app.model), Style::default().fg(Color::DarkGray)),
        Span::styled("|", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" tokens: {} ", app.token_count), Style::default().fg(Color::DarkGray)),
        Span::styled("|", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" session: {} ", app.session_name), Style::default().fg(Color::DarkGray)),
        Span::styled("|", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" style: {} ", app.response_style.name()), Style::default().fg(Color::DarkGray)),
        Span::styled("|", Style::default().fg(Color::DarkGray)),
        Span::styled(" Ctrl+C quit ", Style::default().fg(Color::DarkGray)),
    ]);

    let paragraph = Paragraph::new(status);
    f.render_widget(paragraph, area);
}
