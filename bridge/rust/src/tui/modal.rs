//! Frame-buffer modal overlay (centered card + dimmed viewport).

use std::io::{self, Write};

use crossterm::cursor::MoveTo;
use crossterm::execute;
use crossterm::terminal::ClearType;
use crossterm::terminal::Clear;

use super::{visible_len, BOLD, DIM, RESET, YELLOW};

const MIN_CARD_WIDTH: usize = 44;
const MAX_CARD_WIDTH: usize = 62;

/// Build boxed card lines (visible-width aware padding).
pub fn build_card_lines(title: &str, body: &[String], footer: &str, card_width: usize) -> Vec<String> {
    let w = card_width.clamp(MIN_CARD_WIDTH, MAX_CARD_WIDTH);
    let inner = w.saturating_sub(4);
    let mut lines = Vec::new();

    let title_vis = visible_len(title);
    let title_pad = inner.saturating_sub(title_vis.min(inner));
    lines.push(format!(
        "{YELLOW}{BOLD}\u{250c}\u{2500} {title}{}{RESET}",
        " ".repeat(title_pad)
    ));

    for row in body {
        let vis = visible_len(row);
        let pad = inner.saturating_sub(vis.min(inner));
        lines.push(format!("{YELLOW}\u{2502}{RESET} {row}{} {YELLOW}\u{2502}{RESET}", " ".repeat(pad)));
    }

    if !footer.is_empty() {
        let vis = visible_len(footer);
        let pad = inner.saturating_sub(vis.min(inner));
        lines.push(format!(
            "{YELLOW}\u{2502}{RESET} {DIM}{footer}{RESET}{} {YELLOW}\u{2502}{RESET}",
            " ".repeat(pad)
        ));
    }

    lines.push(format!("{YELLOW}{BOLD}\u{2514}{}{RESET}", "\u{2500}".repeat(w.saturating_sub(2))));

    lines
}

/// Position card lines vertically centered in the chat viewport; returns (row, line) pairs.
pub fn center_card_in_viewport(
    card_lines: &[String],
    term_width: u16,
    chat_start: u16,
    footer_start: u16,
) -> Vec<(u16, String)> {
    if card_lines.is_empty() || footer_start <= chat_start {
        return Vec::new();
    }
    let viewport_h = footer_start - chat_start;
    let card_h = card_lines.len() as u16;
    let start_row = if viewport_h > card_h {
        chat_start + (viewport_h - card_h) / 2
    } else {
        chat_start
    };
    let pad = (term_width as usize)
        .saturating_sub(card_lines.first().map(|l| visible_len(l)).unwrap_or(0))
        / 2;

    card_lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let row = start_row + i as u16;
            if row >= footer_start {
                return (row, String::new());
            }
            (row, format!("{}{}", " ".repeat(pad), line))
        })
        .filter(|(_, l)| !l.is_empty())
        .collect()
}

/// Dim-fill chat viewport rows and draw modal card on top.
pub fn draw_modal_overlay(
    out: &mut io::Stdout,
    term_width: u16,
    chat_start: u16,
    footer_start: u16,
    card_rows: &[(u16, String)],
) -> io::Result<()> {
    let dim_line = format!("{DIM}{}", " ".repeat(term_width as usize));
    for row in chat_start..footer_start {
        execute!(out, MoveTo(0, row), Clear(ClearType::CurrentLine))?;
        write!(out, "{dim_line}")?;
    }
    for (row, line) in card_rows {
        if *row >= chat_start && *row < footer_start {
            execute!(out, MoveTo(0, *row), Clear(ClearType::CurrentLine))?;
            write!(out, "{line}")?;
        }
    }
    Ok(())
}

pub fn card_width_for_terminal(term_width: u16) -> usize {
    (term_width as usize).min(MAX_CARD_WIDTH).max(MIN_CARD_WIDTH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_has_title_and_footer() {
        let lines = build_card_lines(
            "Welcome",
            &["Question?".to_string()],
            "Enter continue",
            50,
        );
        assert!(lines.len() >= 4);
        assert!(lines[0].contains("Welcome"));
        assert!(lines.iter().any(|l| l.contains("Enter continue")));
    }

    #[test]
    fn center_card_stays_in_viewport() {
        let card = vec!["line".to_string(); 5];
        let rows = center_card_in_viewport(&card, 80, 2, 40);
        assert!(rows.iter().all(|(r, _)| *r >= 2 && *r < 40));
    }
}
