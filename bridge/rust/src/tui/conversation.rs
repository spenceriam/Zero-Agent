use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use super::{App, MessageRole, ToolStatus};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let mut lines: Vec<Line> = Vec::new();

    // Calculate visible range
    let total_lines = calculate_total_lines(app);
    let visible_height = area.height as usize;
    let start = if total_lines > visible_height {
        total_lines.saturating_sub(visible_height + app.scroll_offset)
    } else {
        0
    };
    let end = total_lines.saturating_sub(app.scroll_offset);

    let mut current_line = 0;

    for msg in &app.messages {
        // Skip messages before visible range
        if current_line >= end {
            break;
        }

        // Message header
        if current_line >= start {
            let header = match msg.role {
                MessageRole::User => Line::from(vec![
                    Span::styled("You: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]),
                MessageRole::Assistant => Line::from(vec![
                    Span::styled("Agent: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                ]),
                MessageRole::System => Line::from(vec![
                    Span::styled("System: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ]),
                MessageRole::Tool => Line::from(vec![
                    Span::styled("Tool: ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                ]),
            };
            lines.push(header);
        }
        current_line += 1;

        // Message content
        for content_line in msg.content.lines() {
            if current_line >= start && current_line < end {
                lines.push(Line::from(Span::raw(content_line.to_string())));
            }
            current_line += 1;
        }

        // Tool calls
        for tc in &msg.tool_calls {
            if current_line >= start && current_line < end {
                let icon = match tc.status {
                    ToolStatus::Running => "⏳",
                    ToolStatus::Success => "✓",
                    ToolStatus::Error => "✗",
                };
                let color = match tc.status {
                    ToolStatus::Running => Color::Yellow,
                    ToolStatus::Success => Color::Green,
                    ToolStatus::Error => Color::Red,
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("  {} {}", icon, tc.name), Style::default().fg(color)),
                ]));
            }
            current_line += 1;

            // Tool output (if available)
            if let Some(ref output) = tc.output {
                for output_line in output.lines().take(5) {
                    if current_line >= start && current_line < end {
                        lines.push(Line::from(vec![
                            Span::styled(format!("    {}", output_line), Style::default().fg(Color::DarkGray)),
                        ]));
                    }
                    current_line += 1;
                }
            }
        }

        // Empty line between messages
        if current_line >= start && current_line < end {
            lines.push(Line::from(""));
        }
        current_line += 1;
    }

    // Streaming indicator
    if app.is_streaming {
        lines.push(Line::from(vec![
            Span::styled("Agent: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(&app.current_stream),
            Span::styled("▌", Style::default().fg(Color::White).add_modifier(Modifier::BLINK)),
        ]));
    }

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Conversation"))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn calculate_total_lines(app: &App) -> usize {
    let mut count = 0;
    for msg in &app.messages {
        count += 1; // header
        count += msg.content.lines().count();
        for tc in &msg.tool_calls {
            count += 1; // tool call line
            if let Some(ref output) = tc.output {
                count += output.lines().take(5).count();
            }
        }
        count += 1; // empty line
    }
    if app.is_streaming {
        count += 1;
    }
    count
}
