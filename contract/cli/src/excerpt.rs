//! A diagnostic's source line with a caret under its columns, for the text
//! reports: what a reader would otherwise open the file to see.

use crate::CompileError;

/// The line `error` names, then a caret under its columns, each line led by
/// a newline; empty when it names no line or its file cannot be read.
pub fn excerpt(error: &CompileError) -> String {
    let (Some(file), line) = (&error.file, error.span.line) else {
        return String::new();
    };
    let Some(index) = (line as usize).checked_sub(1) else {
        return String::new();
    };
    let Ok(text) = std::fs::read_to_string(file) else {
        return String::new();
    };
    match text.lines().nth(index) {
        Some(source) => quoted(source, line, error.span.col, error.span.end_col),
        None => String::new(),
    }
}

/// `source`, line `line`, with carets under its byte columns `col` (1-based)
/// to `end` (exclusive), trimmed to about 140 characters around them.
fn quoted(source: &str, line: u32, col: u32, end: u32) -> String {
    const WIDE: usize = 140;
    let chars = |bytes: u32| {
        let at = (bytes.max(1) as usize - 1).min(source.len());
        source.get(..at).map_or(at, |s| s.chars().count())
    };
    let all: Vec<char> = source.trim_end().chars().collect();
    let from = chars(col).min(all.len());
    let to = chars(end).max(from + 1);
    let start = if all.len() > WIDE {
        from.saturating_sub(WIDE / 3)
    } else {
        0
    };
    let stop = all.len().min(start + WIDE);
    let shown: String = all[start..stop].iter().collect();
    let before = if start > 0 { "…" } else { "" };
    let after = if stop < all.len() { "…" } else { "" };
    // A tab stays a tab under itself, so the caret lines up however tabs show.
    let pad: String = before
        .chars()
        .map(|_| ' ')
        .chain(
            all[start..from]
                .iter()
                .map(|&c| if c == '\t' { '\t' } else { ' ' }),
        )
        .collect();
    let width = to.min(stop.max(from + 1)) - from;
    let gutter = " ".repeat(line.to_string().len());
    format!(
        "\n {line} | {before}{shown}{after}\n {gutter} | {pad}{}",
        "^".repeat(width.max(1))
    )
}

#[cfg(test)]
mod tests {
    use super::quoted;

    #[test]
    fn the_caret_is_under_the_columns_and_a_long_line_is_trimmed_around_it() {
        assert_eq!(
            quoted("\ttext é nope", 3, 10, 14),
            "\n 3 | \ttext é nope\n   | \t       ^^^^"
        );
        let long = format!("{}nope{}", "a, ".repeat(100), ", b".repeat(100));
        let out = quoted(&long, 12, 301, 305);
        let lines: Vec<&str> = out.lines().collect();
        assert!(
            lines[1].starts_with(" 12 | …") && lines[1].ends_with('…'),
            "{out}"
        );
        let caret = lines[2].chars().position(|c| c == '^').unwrap();
        let above: String = lines[1].chars().skip(caret).take(4).collect();
        assert_eq!(above, "nope", "{out}");
    }
}
