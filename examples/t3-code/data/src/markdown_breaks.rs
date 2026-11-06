// Included by markdown.rs: single newlines kept as line breaks, in a user's message (UserMessageBody, remark-breaks)
// and in a pull request's text on a host that renders them so (PullRequestMarkdown `lineBreaks`).
/// A private-use mark (never whitespace, so the parser's line trim keeps it).
const LINE_BREAK: &str = "\u{e000}";

/// Marks each single newline between two lines of prose (outside fences) so
/// the paragraph keeps it as a line break.
fn line_breaks(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    // A pipe line outside a table is prose (remark-breaks keeps its newline).
    let tables = table_lines(&lines);
    let prose = |index: usize| {
        prose_line(lines[index]) || (lines[index].trim().starts_with('|') && !tables[index])
    };
    let mut out = String::new();
    let mut fenced = false;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
        }
        out.push_str(line);
        if index + 1 < lines.len() {
            if !fenced && prose(index) && prose(index + 1) {
                out.push_str(LINE_BREAK);
            }
            out.push('\n');
        }
    }
    out
}
/// A line that continues a paragraph rather than starting another block.
fn prose_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    let ordered = trimmed.split_once(['.', ')']).is_some_and(|(digits, _)| {
        !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
    });
    !(trimmed.starts_with('#')
        || trimmed.starts_with('|')
        || trimmed.starts_with('>')
        || trimmed.starts_with("```")
        || trimmed.starts_with("~~~")
        || trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("+ ")
        || ordered
        || trimmed == "---"
        || trimmed == "***")
}
