//! Pipe-table rendering ported from impulse (tables only).

use super::{render_inline, visible_len, wrap_text_visible, BOLD, DIM, RESET};

const BORDER: &str = "\x1b[90m";
const BORDER_RESET: &str = RESET;

#[derive(Debug, Clone)]
struct MarkdownTable {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

pub fn is_table_candidate(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('|') && split_table_row(trimmed).len() >= 2
}

pub fn is_separator_row(line: &str) -> bool {
    if !is_table_candidate(line) {
        return false;
    }
    split_table_row(line)
        .iter()
        .all(|cell| is_separator_cell(cell))
}

fn is_separator_cell(cell: &str) -> bool {
    let t = cell.trim();
    if t.len() < 3 {
        return false;
    }
    let bytes = t.as_bytes();
    let start_ok = bytes[0] == b'-' || (bytes.len() > 1 && bytes[0] == b':' && bytes[1] == b'-');
    if !start_ok {
        return false;
    }
    let end_ok = *bytes.last().unwrap_or(&b'-') == b'-'
        || (bytes.len() > 1 && bytes[bytes.len() - 2] == b'-' && bytes[bytes.len() - 1] == b':');
    t.chars().all(|c| c == '-' || c == ':') && start_ok && end_ok
}

fn split_table_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let without_outer = trimmed
        .strip_prefix('|')
        .unwrap_or(trimmed)
        .strip_suffix('|')
        .unwrap_or(trimmed);
    without_outer
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

fn normalize_row(row: Vec<String>, width: usize) -> Vec<String> {
    if row.len() == width {
        return row;
    }
    if row.len() > width {
        return row.into_iter().take(width).collect();
    }
    let mut out = row;
    out.resize(width, String::new());
    out
}

fn parse_table(lines: &[String], start_index: usize) -> Option<(MarkdownTable, usize)> {
    let header_line = lines.get(start_index)?;
    let separator_line = lines.get(start_index + 1)?;
    if !is_table_candidate(header_line) || !is_separator_row(separator_line) {
        return None;
    }
    let header = split_table_row(header_line);
    let column_count = header.len();
    let mut rows = Vec::new();
    let mut index = start_index + 2;
    while index < lines.len() {
        let line = &lines[index];
        if !is_table_candidate(line) || is_separator_row(line) {
            break;
        }
        let row = split_table_row(line);
        if row.len() < 2 {
            break;
        }
        rows.push(normalize_row(row, column_count));
        index += 1;
    }
    if rows.is_empty() {
        return None;
    }
    Some((
        MarkdownTable { header, rows },
        index - start_index,
    ))
}

/// True when lines[0] looks like the start of a table that is not complete yet.
pub fn pending_table_at_start(lines: &[String]) -> bool {
    let Some(first) = lines.first() else {
        return false;
    };
    if !is_table_candidate(first) {
        return false;
    }
    if lines.len() == 1 {
        return true;
    }
    if lines.len() >= 2 && !is_separator_row(&lines[1]) {
        return true;
    }
    parse_table(lines, 0).is_none()
}

fn truncate_plain(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

fn pad_cell(cell: &str, width: usize) -> String {
    let truncated = truncate_plain(cell, width);
    let padding = width.saturating_sub(truncated.chars().count());
    format!("{truncated}{}", " ".repeat(padding))
}

fn table_total_width(widths: &[usize]) -> usize {
    widths.iter().sum::<usize>() + widths.len() * 3 + 1
}

fn table_column_widths(table: &MarkdownTable) -> Vec<usize> {
    table
        .header
        .iter()
        .enumerate()
        .map(|(column, header)| {
            let row_max = table
                .rows
                .iter()
                .map(|row| visible_len(row.get(column).map(String::as_str).unwrap_or("")))
                .max()
                .unwrap_or(0);
            visible_len(header)
                .max(row_max)
                .clamp(3, 60)
        })
        .collect()
}

fn border_line(left: &str, middle: &str, right: &str, widths: &[usize]) -> String {
    let inner: String = widths
        .iter()
        .map(|w| "─".repeat(w + 2))
        .collect::<Vec<_>>()
        .join(middle);
    format!("{BORDER}{left}{inner}{right}{BORDER_RESET}")
}

fn render_wide_table(table: &MarkdownTable, widths: &[usize]) -> Vec<String> {
    let render_row = |row: &[String], header: bool| -> String {
        let cells: Vec<String> = widths
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let value = pad_cell(row.get(i).map(String::as_str).unwrap_or(""), *w);
                if header {
                    format!(" {BOLD}{value}{RESET} ")
                } else {
                    format!(" {value} ")
                }
            })
            .collect();
        format!(
            "{BORDER}│{BORDER_RESET}{}{BORDER}│{BORDER_RESET}",
            cells.join(&format!("{BORDER}│{BORDER_RESET}"))
        )
    };
    let mut out = vec![
        border_line("┌", "┬", "┐", widths),
        render_row(&table.header, true),
        border_line("├", "┼", "┤", widths),
    ];
    for row in &table.rows {
        out.push(render_row(row, false));
    }
    out.push(border_line("└", "┴", "┘", widths));
    out
}

fn render_stacked_table(table: &MarkdownTable, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let label_width = table
        .header
        .iter()
        .map(|h| visible_len(h))
        .max()
        .unwrap_or(3)
        .min(25)
        .max(3);
    let use_two_line = width < label_width + 18;

    for (row_index, row) in table.rows.iter().enumerate() {
        if row_index > 0 {
            lines.push(String::new());
        }
        lines.push(format!("{DIM}[{}]{RESET}", row_index + 1));
        for (column, header) in table.header.iter().enumerate() {
            let value = row.get(column).map(String::as_str).unwrap_or("");
            if use_two_line {
                let label = truncate_plain(header, width.saturating_sub(2).max(8));
                lines.push(format!("  {DIM}{label}{RESET}"));
                for wrapped in wrap_text_visible(
                    if value.is_empty() { " " } else { value },
                    width.saturating_sub(4).max(8),
                ) {
                    lines.push(format!("    {wrapped}"));
                }
            } else {
                let label = pad_cell(header, label_width);
                let prefix = format!("  {DIM}{label}{RESET}  ");
                let wrapped = wrap_text_visible(
                    if value.is_empty() { " " } else { value },
                    width.saturating_sub(visible_len(&prefix)).max(8),
                );
                for (i, wline) in wrapped.iter().enumerate() {
                    if i == 0 {
                        lines.push(format!("{prefix}{wline}"));
                    } else {
                        lines.push(format!("  {}{}", " ".repeat(label_width), format!("  {wline}")));
                    }
                }
            }
        }
    }
    lines
}

fn render_table(table: &MarkdownTable, width: usize) -> Vec<String> {
    let widths = table_column_widths(table);
    if table_total_width(&widths) <= width {
        render_wide_table(table, &widths)
    } else {
        render_stacked_table(table, width)
    }
}

/// Render a single line with inline markdown and wrapping.
pub fn render_formatted_line(line: &str, width: usize) -> Vec<String> {
    if line.is_empty() {
        return vec![String::new()];
    }
    wrap_text_visible(&render_inline(line), width)
}

/// Render markdown-aware lines (tables + inline formatting).
pub fn render_markdown_lines(lines: &[String], width: usize) -> Vec<String> {
    let inner_width = width.max(8);
    let mut rendered = Vec::new();
    let mut index = 0usize;
    while index < lines.len() {
        if let Some((table, consumed)) = parse_table(lines, index) {
            rendered.extend(render_table(&table, inner_width));
            rendered.push(String::new());
            index += consumed;
            continue;
        }
        let line = lines[index].clone();
        if line.is_empty() {
            rendered.push(String::new());
        } else {
            rendered.extend(render_formatted_line(&line, inner_width));
        }
        index += 1;
    }
    rendered
}

pub fn render_markdown_text(text: &str, width: usize) -> Vec<String> {
    let lines: Vec<String> = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(String::from)
        .collect();
    render_markdown_lines(&lines, width)
}

/// Drain lines that can be rendered from the pending buffer (waits for complete tables).
pub fn drain_pending_lines(pending: &mut Vec<String>, width: usize, flush_all: bool) -> Vec<String> {
    let mut out = Vec::new();
    loop {
        if pending.is_empty() {
            break;
        }
        if let Some((table, consumed)) = parse_table(pending, 0) {
            if !flush_all {
                break;
            }
            out.extend(render_table(&table, width));
            pending.drain(0..consumed);
            continue;
        }
        if !flush_all && pending_table_at_start(pending) {
            break;
        }
        let line = pending.remove(0);
        if line.is_empty() {
            out.push(String::new());
        } else {
            out.extend(render_formatted_line(&line, width));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_renders_wide_table() {
        let lines = vec![
            "| Tool | Status |".to_string(),
            "|---|---|".to_string(),
            "| shell | ok |".to_string(),
        ];
        let rendered = render_markdown_lines(&lines, 80);
        let joined = rendered.join("\n");
        assert!(joined.contains('┌'), "expected box top: {joined}");
        assert!(joined.contains("shell"));
        assert!(joined.contains("ok"));
    }

    #[test]
    fn separator_row_detected() {
        assert!(is_separator_row("|---|---|"));
        assert!(!is_separator_row("| a | b |"));
    }

    #[test]
    fn non_table_lines_use_inline_formatting() {
        let lines = vec!["plain **bold** text".to_string()];
        let rendered = render_markdown_lines(&lines, 80);
        assert!(rendered[0].contains(BOLD));
    }

    #[test]
    fn pending_table_waits_for_rows() {
        let mut pending = vec![
            "| Tool | Status |".to_string(),
            "|---|---|".to_string(),
        ];
        let partial = drain_pending_lines(&mut pending, 80, false);
        assert!(partial.is_empty());
        assert_eq!(pending.len(), 2);
        pending.push("| shell | ok |".to_string());
        let held = drain_pending_lines(&mut pending, 80, false);
        assert!(held.is_empty(), "complete table should wait until flush_all");
        assert_eq!(pending.len(), 3);
        let done = drain_pending_lines(&mut pending, 80, true);
        assert!(!done.is_empty());
        assert!(pending.is_empty());
        let joined = done.join("\n");
        assert!(joined.contains('┌'), "expected box table: {joined}");
    }

    #[test]
    fn streaming_table_renders_all_rows_together() {
        let mut pending = vec![
            "| Tool | Status |".to_string(),
            "|---|---|".to_string(),
            "| read | ok |".to_string(),
        ];
        assert!(drain_pending_lines(&mut pending, 80, false).is_empty());
        pending.push("| write | ok |".to_string());
        pending.push("| shell | ok |".to_string());
        let rendered = drain_pending_lines(&mut pending, 80, true);
        let joined = rendered.join("\n");
        assert!(joined.contains('┌'));
        assert!(joined.contains("read"));
        assert!(joined.contains("write"));
        assert!(joined.contains("shell"));
        assert!(!joined.contains("| write |"), "raw pipes should not leak");
    }
}
