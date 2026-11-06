// The gap before each chat Markdown block (`.chat-markdown`'s collapsed margins), included by markdown.rs.

/// The collapsed vertical margin T3's `.chat-markdown` CSS leaves before a
/// block: headings take 1.25rem, prose/lists/code .65rem, sibling list items
/// .25rem, and the rows of one table none. A line of raw HTML images no `<p>`
/// holds has no margins (`bare`), so the gap beside it is its neighbour's
/// own: a heading's 1.25rem above and .5rem below, a rule's 0.5em (`hr`'s
/// default; `.chat-markdown` sets none), any other block's .65rem.
fn block_gap(
    previous: Option<&markdown_parse::Block>,
    block: &markdown_parse::Block,
    index: usize,
) -> f64 {
    use markdown_parse::Kind;
    let Some(previous) = previous.filter(|_| index > 0) else {
        return 0.0;
    };
    if previous.bare || block.bare {
        let margin = |b: &markdown_parse::Block, heading: f64| match b.kind {
            _ if b.bare => 0.0,
            Kind::Heading => heading,
            Kind::Rule => 7.0,
            _ => 10.4,
        };
        return f64::max(margin(previous, 8.0), margin(block, 20.0));
    }
    match (previous.kind, block.kind) {
        (_, Kind::Heading) => 20.0,
        (Kind::Item, Kind::Item) if previous.depth == block.depth => 4.0,
        (Kind::TableRow, Kind::TableRow) => 0.0,
        _ => 10.4,
    }
}

#[cfg(test)]
mod gap_tests {
    use super::*;

    /// Each block's gap before it, as `block_gap` gives it.
    fn gaps(source: &str) -> Vec<f64> {
        let doc = markdown_parse::parse(source, &|href| href.to_string());
        let mut previous = None;
        let mut gaps = Vec::new();
        for (index, block) in doc.blocks.iter().enumerate() {
            gaps.push(block_gap(previous, block, index));
            previous = Some(block);
        }
        gaps
    }

    #[test]
    fn gaps_follow_collapsed_chat_markdown_margins() {
        let gaps = gaps(
            "Intro.\n\n## Section\n\n- One\n  - Nested\n- Two\n\n| a | b |\n|---|---|\n| 1 | 2 |",
        );
        assert_eq!(gaps, vec![0.0, 20.0, 10.4, 10.4, 10.4, 10.4, 0.0]);
    }

    #[test]
    fn a_line_of_html_images_no_paragraph_holds_has_no_margins() {
        // "Show the HTML image sample"'s shape: the top-level line sits right under the `<div>`'s image, as the
        // reference draws it; a paragraph or a heading beside such a line keeps its own margin.
        let gaps = gaps(
            "Intro.\n\n<div><img src=\"a.png\"></div>\n\n<img src=\"b.png\">\n\n## Next\n\n<img src=\"c.png\">\n\n<p><img src=\"d.png\"></p>\n\nAfter.",
        );
        assert_eq!(gaps, vec![0.0, 10.4, 0.0, 20.0, 8.0, 10.4, 10.4]);
    }
}
