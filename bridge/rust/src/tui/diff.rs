//! Pi-inspired diff rendering for edit_file / write_file tool results.

use similar::{ChangeTag, TextDiff};

use super::{visible_len, wrap_text_visible, BOLD, DIM, GREEN, RED, RESET};

const DIFF_INDENT: &str = "    ";
const BG_RED: &str = "\x1b[41m\x1b[97m";
const BG_GREEN: &str = "\x1b[48;5;28m\x1b[97m";

const DIFF_SPLIT_MIN_WIDTH: usize = 120;
const DIFF_COLLAPSED_LINES: usize = 24;
const MIN_COMPACT_WIDTH: usize = 18;
const MIN_SPLIT_COLUMN_WIDTH: usize = 24;
const SPLIT_SEPARATOR: &str = " \u{2502} ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffKind {
    Edit,
    WriteCreate,
    WriteOverwrite,
}

#[derive(Debug, Clone)]
pub struct DiffInput {
    pub path: String,
    pub old_text: String,
    pub new_text: String,
    pub kind: DiffKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffPresentationMode {
    Summary,
    Compact,
    Unified,
    Split,
}

#[derive(Debug, Default, Clone)]
struct DiffStats {
    added: usize,
    removed: usize,
    hunks: usize,
}

#[derive(Debug, Clone)]
struct DiffRow {
    text: String,
    hunk_index: Option<usize>,
}

pub fn render_diff_lines(input: &DiffInput, content_width: usize, terminal_width: usize) -> Vec<String> {
    let mode = resolve_presentation_mode(content_width, terminal_width);
    let stats = compute_stats(&input.old_text, &input.new_text);

    if mode == DiffPresentationMode::Summary {
        return vec![render_summary_line(&input.path, &stats, content_width)];
    }

    let mut rows: Vec<DiffRow> = render_header(&input.path, &input.kind, &stats, content_width);
    rows.extend(render_body(
        &input.old_text,
        &input.new_text,
        mode,
        content_width,
        terminal_width,
    ));
    apply_line_limit(rows, content_width, DIFF_COLLAPSED_LINES, stats.hunks)
}

fn resolve_presentation_mode(content_width: usize, _terminal_width: usize) -> DiffPresentationMode {
    if content_width < MIN_COMPACT_WIDTH {
        return DiffPresentationMode::Summary;
    }
    if content_width < MIN_COMPACT_WIDTH + 6 {
        return DiffPresentationMode::Compact;
    }
    DiffPresentationMode::Unified
}

fn can_render_split(content_width: usize) -> bool {
    let sep = visible_len(SPLIT_SEPARATOR);
    let per_side = content_width.saturating_sub(sep) / 2;
    per_side >= MIN_SPLIT_COLUMN_WIDTH
}

fn compute_stats(old_text: &str, new_text: &str) -> DiffStats {
    let diff = TextDiff::from_lines(old_text, new_text);
    let mut stats = DiffStats::default();
    let mut in_hunk = false;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => stats.added += 1,
            ChangeTag::Delete => stats.removed += 1,
            ChangeTag::Equal => {}
        }
        if change.tag() != ChangeTag::Equal {
            if !in_hunk {
                stats.hunks += 1;
                in_hunk = true;
            }
        } else {
            in_hunk = false;
        }
    }
    stats
}

fn format_change_line(sign: &str, body: &str, bg: &str) -> String {
    format!("{DIFF_INDENT}{bg} {sign} {body}{RESET}")
}

fn render_summary_line(path: &str, stats: &DiffStats, width: usize) -> String {
    let candidates = [
        format!(
            "{DIFF_INDENT}{DIM}\u{21b3} diff {path} +{} -{} \u{2022} {} {}{RESET}",
            stats.added,
            stats.removed,
            stats.hunks,
            if stats.hunks == 1 { "hunk" } else { "hunks" }
        ),
        format!(
            "{DIFF_INDENT}{DIM}\u{21b3} diff +{} -{}{RESET}",
            stats.added, stats.removed
        ),
        format!(
            "{DIFF_INDENT}{DIM}\u{21b3} +{} -{}{RESET}",
            stats.added, stats.removed
        ),
    ];
    for candidate in candidates {
        if visible_len(&candidate) <= width {
            return candidate;
        }
    }
    truncate_visible(
        &format!("{DIFF_INDENT}{DIM}\u{21b3} diff{RESET}"),
        width,
    )
}

fn render_header(path: &str, kind: &DiffKind, stats: &DiffStats, width: usize) -> Vec<DiffRow> {
    let label = match kind {
        DiffKind::Edit => "edited",
        DiffKind::WriteCreate => "created",
        DiffKind::WriteOverwrite => "overwritten",
    };
    let header = format!(
        "{DIFF_INDENT}{BOLD}{path}{RESET} {DIM}({label} +{} -{}){RESET}",
        stats.added, stats.removed
    );
    wrap_row(&header, width, None)
}

fn render_body(
    old_text: &str,
    new_text: &str,
    mode: DiffPresentationMode,
    content_width: usize,
    terminal_width: usize,
) -> Vec<DiffRow> {
    let _ = terminal_width;
    match mode {
        DiffPresentationMode::Summary => Vec::new(),
        DiffPresentationMode::Compact => render_compact(old_text, new_text, content_width),
        DiffPresentationMode::Unified => render_unified(old_text, new_text, content_width),
        DiffPresentationMode::Split => render_split(old_text, new_text, content_width),
    }
}

fn render_compact(old_text: &str, new_text: &str, width: usize) -> Vec<DiffRow> {
    let diff = TextDiff::from_lines(old_text, new_text);
    let mut rows = Vec::new();
    let mut hunk = 0usize;
    let mut in_hunk = false;
    for change in diff.iter_all_changes() {
        if change.tag() == ChangeTag::Equal {
            in_hunk = false;
            continue;
        }
        if !in_hunk {
            hunk += 1;
            in_hunk = true;
        }
        let (sign, bg) = match change.tag() {
            ChangeTag::Delete => ("-", BG_RED),
            ChangeTag::Insert => ("+", BG_GREEN),
            ChangeTag::Equal => continue,
        };
        let line = format_change_line(sign, change.value().trim_end_matches('\n'), bg);
        rows.extend(wrap_row(&line, width, Some(hunk)));
    }
    rows
}

fn render_unified(old_text: &str, new_text: &str, width: usize) -> Vec<DiffRow> {
    let diff = TextDiff::from_lines(old_text, new_text);
    let mut rows = Vec::new();
    let mut hunk = 0usize;
    let mut in_hunk = false;
    for change in diff.iter_all_changes() {
        if change.tag() == ChangeTag::Equal {
            in_hunk = false;
            continue;
        }
        if !in_hunk {
            hunk += 1;
            in_hunk = true;
            if hunk > 1 {
                let meta = format!("{DIFF_INDENT}{DIM}@@ hunk {hunk} @@{RESET}");
                rows.extend(wrap_row(&meta, width, Some(hunk)));
            }
        }
        let (sign, bg) = match change.tag() {
            ChangeTag::Delete => ("-", BG_RED),
            ChangeTag::Insert => ("+", BG_GREEN),
            ChangeTag::Equal => continue,
        };
        let line = format_change_line(sign, change.value().trim_end_matches('\n'), bg);
        rows.extend(wrap_row(&line, width, Some(hunk)));
    }
    rows
}

#[derive(Clone)]
enum SplitCell {
    Remove(String),
    Add(String),
    Blank,
}

fn render_split(old_text: &str, new_text: &str, content_width: usize) -> Vec<DiffRow> {
    let sep_w = visible_len(SPLIT_SEPARATOR);
    let col_w = content_width.saturating_sub(sep_w) / 2;
    if col_w < MIN_SPLIT_COLUMN_WIDTH {
        return render_unified(old_text, new_text, content_width);
    }

    let diff = TextDiff::from_lines(old_text, new_text);
    let mut pairs: Vec<(SplitCell, SplitCell)> = Vec::new();
    let mut pending_removals: Vec<String> = Vec::new();

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => {
                pending_removals.push(change.value().trim_end_matches('\n').to_string());
            }
            ChangeTag::Insert => {
                if pending_removals.is_empty() {
                    pairs.push((
                        SplitCell::Blank,
                        SplitCell::Add(change.value().trim_end_matches('\n').to_string()),
                    ));
                } else {
                    let old = pending_removals.remove(0);
                    pairs.push((
                        SplitCell::Remove(old),
                        SplitCell::Add(change.value().trim_end_matches('\n').to_string()),
                    ));
                }
            }
            ChangeTag::Equal => {
                while let Some(old) = pending_removals.pop() {
                    pairs.push((SplitCell::Remove(old), SplitCell::Blank));
                }
            }
        }
    }
    while let Some(old) = pending_removals.pop() {
        pairs.push((SplitCell::Remove(old), SplitCell::Blank));
    }

    let mut rows = Vec::new();
    let hunk = 1usize;
    for (left, right) in pairs {
        let left_text = format_split_cell(&left, col_w);
        let right_text = format_split_cell(&right, col_w);
        let line = format!(
            "  {DIM}\u{2502}{RESET} {left_text}{SPLIT_SEPARATOR}{right_text}"
        );
        rows.extend(wrap_row(&line, content_width, Some(hunk)));
    }
    rows
}

fn format_split_cell(cell: &SplitCell, col_w: usize) -> String {
    match cell {
        SplitCell::Blank => " ".repeat(col_w),
        SplitCell::Remove(text) => format_split_signed_cell(text, col_w, RED, "-"),
        SplitCell::Add(text) => format_split_signed_cell(text, col_w, GREEN, "+"),
    }
}

fn format_split_signed_cell(text: &str, col_w: usize, color: &str, sign: &str) -> String {
    let inner = col_w.saturating_sub(2);
    let body = truncate_visible(text, inner);
    format!(
        "{color}{sign}{RESET} {body}{}",
        " ".repeat(inner.saturating_sub(visible_len(&body)))
    )
}

fn wrap_row(text: &str, width: usize, hunk_index: Option<usize>) -> Vec<DiffRow> {
    wrap_text_visible(text, width)
        .into_iter()
        .map(|text| DiffRow { text, hunk_index })
        .collect()
}

fn apply_line_limit(
    rows: Vec<DiffRow>,
    width: usize,
    max_lines: usize,
    total_hunks: usize,
) -> Vec<String> {
    if rows.len() <= max_lines {
        return rows.into_iter().map(|r| r.text).collect();
    }
    let shown: Vec<_> = rows.iter().take(max_lines).collect();
    let remaining = rows.len() - shown.len();
    let visible_hunks: std::collections::HashSet<usize> = shown
        .iter()
        .filter_map(|r| r.hunk_index)
        .collect();
    let hidden_hunks = total_hunks.saturating_sub(visible_hunks.len());
    let mut out: Vec<String> = shown.into_iter().map(|r| r.text.clone()).collect();
    let hint = build_collapsed_hint(remaining, hidden_hunks, width);
    out.push(format!("{DIFF_INDENT}{DIM}\u{2502}{RESET} {DIM}{hint}{RESET}"));
    out
}

fn build_collapsed_hint(remaining_lines: usize, hidden_hunks: usize, width: usize) -> String {
    let candidates = [
        format!(
            "\u{2026} +{remaining_lines} more lines \u{2022} {hidden_hunks} hidden {}",
            if hidden_hunks == 1 { "hunk" } else { "hunks" }
        ),
        format!("\u{2026} +{remaining_lines} more lines"),
        "\u{2026}".to_string(),
    ];
    for candidate in candidates {
        let line = format!("  {DIM}\u{2502}{RESET} {DIM}{candidate}{RESET}");
        if visible_len(&line) <= width {
            return candidate;
        }
    }
    "\u{2026}".to_string()
}

fn truncate_visible(text: &str, max: usize) -> String {
    if visible_len(text) <= max {
        return text.to_string();
    }
    let mut out = String::new();
    let mut vis = 0usize;
    let mut in_escape = false;
    for ch in text.chars() {
        if ch == '\x1b' {
            in_escape = true;
            out.push(ch);
            continue;
        }
        if in_escape {
            out.push(ch);
            if ch == 'm' {
                in_escape = false;
            }
            continue;
        }
        if vis >= max.saturating_sub(1) {
            out.push('\u{2026}');
            break;
        }
        out.push(ch);
        vis += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_input() -> DiffInput {
        DiffInput {
            path: "src/main.rs".to_string(),
            old_text: "fn main() {\n    println!(\"hi\");\n}\n".to_string(),
            new_text: "fn main() {\n    println!(\"hello\");\n}\n".to_string(),
            kind: DiffKind::Edit,
        }
    }

    #[test]
    fn unified_diff_renders_plus_minus() {
        let lines = render_diff_lines(&sample_input(), 80, 80);
        let joined = lines.join("\n");
        assert!(joined.contains('+'), "expected addition line: {joined}");
        assert!(joined.contains('-'), "expected removal line: {joined}");
    }

    #[test]
    fn wide_terminal_uses_unified_not_split() {
        let input = DiffInput {
            path: "a.txt".to_string(),
            old_text: "old line one\nold line two\n".to_string(),
            new_text: "new line one\nnew line two\n".to_string(),
            kind: DiffKind::Edit,
        };
        let lines = render_diff_lines(&input, 116, 171);
        assert!(
            !lines.iter().any(|l| l.contains(" \u{2502} ")),
            "tool diffs should use unified layout, not split: {:?}",
            lines
        );
        assert!(lines.iter().any(|l| l.contains(BG_GREEN) || l.contains('+')));
    }

    #[test]
    fn unified_diff_uses_tool_result_indent() {
        let lines = render_diff_lines(&sample_input(), 80, 80);
        assert!(
            lines.iter().any(|l| l.starts_with(DIFF_INDENT)),
            "expected 4-space diff indent"
        );
    }

    #[test]
    fn auto_mode_uses_unified_on_narrow_terminal() {
        let input = sample_input();
        let lines = render_diff_lines(&input, 76, 80);
        assert!(
            !lines.iter().any(|l| l.contains(" \u{2502} ")),
            "narrow terminal should not use split layout"
        );
    }

    #[test]
    fn collapse_hint_after_many_lines() {
        let old: String = (0..40).map(|i| format!("line {i}\n")).collect();
        let new: String = (0..40).map(|i| format!("row {i}\n")).collect();
        let input = DiffInput {
            path: "big.txt".to_string(),
            old_text: old,
            new_text: new,
            kind: DiffKind::WriteOverwrite,
        };
        let lines = render_diff_lines(&input, 80, 80);
        assert!(
            lines.iter().any(|l| l.contains('\u{2026}')),
            "expected collapsed hint for large diff"
        );
    }

    #[test]
    fn long_diff_line_wraps_within_width() {
        let long = "x".repeat(200);
        let input = DiffInput {
            path: "wide.txt".to_string(),
            old_text: String::new(),
            new_text: format!("{long}\n"),
            kind: DiffKind::WriteCreate,
        };
        let width = 60;
        let lines = render_diff_lines(&input, width, width);
        for line in &lines {
            assert!(
                visible_len(line) <= width + 2,
                "line exceeded width: len={} line={line}",
                visible_len(line)
            );
        }
    }

    #[test]
    fn write_create_shows_additions_only() {
        let input = DiffInput {
            path: "new.txt".to_string(),
            old_text: String::new(),
            new_text: "hello\nworld\n".to_string(),
            kind: DiffKind::WriteCreate,
        };
        let lines = render_diff_lines(&input, 80, 80);
        let joined = lines.join("\n");
        assert!(joined.contains('+'));
        assert!(!joined.contains("- world"));
    }
}
