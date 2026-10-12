//! Assistant Markdown to the transcript's blocks. The parsing is the
//! Markdown reader's (`markdown-parse`, LLP 1033); this maps its blocks to
//! `shape Block` and its runs to coloured `shape Run`s. An unterminated
//! fence (a reply mid-stream) is a code block to the end of the text.

use crate::highlight::highlight;
use crate::state::{Block, Line, Run};
use markdown_parse::{parse, Kind};

/// Inline code's colour.
pub const CODE: &str = "#e5c07b";
/// A link's colour.
pub const LINK: &str = "#61afef";

fn runs(rs: Vec<markdown_parse::Run>) -> Vec<Run> {
    let mut line = Line {
        runs: Vec::with_capacity(rs.len()),
    };
    for r in rs {
        let mut run = Run {
            text: r.text,
            bold: r.bold,
            italic: r.italic,
            ..Run::default()
        };
        if r.code {
            run.fg = CODE.into();
        }
        if !r.href.is_empty() {
            run.fg = LINK.into();
            run.under = true;
            run.href = r.href;
        }
        line.push(run);
    }
    line.runs
}

/// Render `source` as blocks.
pub fn blocks(source: &str) -> Vec<Block> {
    blocks_with(source, true)
}

/// Render an unfinished streaming tail, which will be rebuilt on the next token.
pub(super) fn streaming_blocks(source: &str) -> Vec<Block> {
    blocks_with(source, false)
}

fn blocks_with(source: &str, compact_code: bool) -> Vec<Block> {
    let doc = parse(source, &|href| href.to_string());
    // The parsed block vector can have spare capacity for much larger rows.
    // Move its contents into exactly the number of transcript blocks needed.
    let mut out = Vec::with_capacity(doc.blocks.len());
    out.extend(doc.blocks.into_iter().map(|b| {
        match b.kind {
            Kind::Heading => Block {
                kind: match b.depth {
                    1 => "h1",
                    2 => "h2",
                    _ => "h3",
                }
                .into(),
                runs: runs(b.runs),
                ..Block::default()
            },
            Kind::Paragraph => Block::p(runs(b.runs)),
            Kind::Code => {
                let mut lines = highlight(&b.text, &b.href);
                if compact_code {
                    for line in &mut lines {
                        line.runs.shrink_to_fit();
                    }
                }
                Block {
                    kind: "code".into(),
                    lines,
                    lang: b.href,
                    ..Block::default()
                }
            }
            Kind::Quote => Block {
                kind: "quote".into(),
                depth: b.depth.saturating_sub(1) as f64,
                runs: runs(b.runs),
                ..Block::default()
            },
            Kind::Rule => Block {
                kind: "rule".into(),
                ..Block::default()
            },
            Kind::Item => Block {
                kind: "li".into(),
                depth: b.depth as f64,
                marker: b.marker,
                runs: runs(b.runs),
                ..Block::default()
            },
            Kind::Image => Block::p(vec![Run::fg(format!("[image: {}]", b.text), LINK)]),
            Kind::TableRow => {
                let mut out = Vec::new();
                for (i, cell) in b.cells.into_iter().enumerate() {
                    if i > 0 {
                        out.push(Run::dim(" │ "));
                    }
                    let mut cell = runs(cell);
                    if b.header {
                        cell.iter_mut().for_each(|r| r.bold = true);
                    }
                    out.extend(cell);
                }
                Block::p(out)
            }
            Kind::Html => Block::p(vec![Run::plain(b.text)]),
        }
    }));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_maps() {
        let md = "# Title\n\n## Sub\n\nSome **bold**, *it*, `code` and [a link](https://x.y).\n\n\
                  - one\n  - nested\n1. first\n\n> quoted\n\n---\n\n```rust\nfn f() {}\n```\n";
        let b = blocks(md);
        let kinds: Vec<&str> = b.iter().map(|b| b.kind.as_str()).collect();
        assert_eq!(
            kinds,
            ["h1", "h2", "p", "li", "li", "li", "quote", "rule", "code"]
        );
        let p = &b[2].runs;
        assert!(p.iter().any(|r| r.text == "bold" && r.bold));
        assert!(p.iter().any(|r| r.text == "it" && r.italic));
        assert!(p.iter().any(|r| r.text == "code" && r.fg == CODE));
        assert!(p
            .iter()
            .any(|r| r.text == "a link" && r.under && r.fg == LINK));
        assert_eq!(b[3].depth, 0.0);
        assert_eq!(b[4].depth, 1.0);
        assert_eq!(b[3].marker, "•");
        assert_eq!(b[5].marker, "1.");
        assert_eq!(b[6].depth, 0.0);
        assert_eq!(b[8].lang, "rust");
        assert_eq!(b[8].lines.len(), 1);
    }

    #[test]
    fn an_unterminated_fence_is_code_so_far() {
        let b = blocks("Here:\n\n```py\ndef f():\n    return 1");
        assert_eq!(b.len(), 2);
        assert_eq!(b[1].kind, "code");
        assert_eq!(b[1].lines.len(), 2);
        assert_eq!(b[1].lines[1].text(), "    return 1");
    }
}
