//! Inline Markdown as one line of text (markdown.rs `render_inline`, minus
//! the styling): the summary lines and the rail's statuses render an
//! agent's last message, which often carries `**bold**`, `` `code` `` and
//! `[links](url)`. The words stay; the marks go. Like upstream, the line is
//! parsed as a whole document after its whitespace is collapsed, block
//! markers (`#`, `-`, `1.`) vanish, a task shows "☑ " / "☐ ", and formulas
//! stay literal with their delimiters (`literal_math`): `\(x\)` reads `$x$`.

use crate::markdown_doc::{self, Block, Run};

/// `text` with inline marks removed: emphasis and code fences dropped,
/// links reduced to their text, autolinks kept, whitespace collapsed.
pub fn inline_text(text: &str) -> String {
    let doc = markdown_doc::parse(&collapse(text));
    let mut out = String::new();
    append(&doc, &mut out);
    collapse(&out)
}

fn append(blocks: &[Block], out: &mut String) {
    let runs = |runs: &[Run], out: &mut String| out.push_str(&markdown_doc::runs_text(runs));
    for block in blocks {
        match block {
            Block::Heading(_, r) | Block::Paragraph(r) => runs(r, out),
            Block::BlockQuote(inner) => append(inner, out),
            Block::List(_, items) => {
                for item in items {
                    match item.task {
                        Some(true) => out.push_str("☑ "),
                        Some(false) => out.push_str("☐ "),
                        None => {}
                    }
                    append(&item.blocks, out);
                }
            }
            Block::Code(_, text) => out.push_str(text),
            Block::Table(_, header, rows) => {
                for cell in header.iter().chain(rows.iter().flatten()) {
                    runs(cell, out);
                }
            }
            Block::Rule => {}
        }
    }
}

/// Every run of whitespace as one space, trimmed.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::inline_text;

    #[test]
    fn marks_go_and_words_stay() {
        assert_eq!(
            inline_text("Done. **tests pass**, see `foo` and [PR #241](https://x/y)."),
            "Done. tests pass, see foo and PR #241."
        );
        assert_eq!(
            inline_text("snake_case stays, _em_ goes"),
            "snake_case stays, em goes"
        );
        assert_eq!(inline_text("a\nb  c"), "a b c");
        assert_eq!(inline_text("<https://example.com>"), "https://example.com");
        assert_eq!(inline_text("[unlinked]"), "[unlinked]");
    }

    #[test]
    fn compact_summaries_keep_math_literal() {
        assert_eq!(
            inline_text("Found $x^2$ and $$y^2$$."),
            "Found $x^2$ and $$y^2$$."
        );
        assert_eq!(inline_text(r"Euler \(e^{i\pi}\)"), r"Euler $e^{i\pi}$");
        assert_eq!(inline_text("# Title"), "Title");
        assert_eq!(inline_text("- [x] shipped"), "☑ shipped");
    }
}
