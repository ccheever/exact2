// T3 Code 7c3b3b3605ef (MIT; see LICENSE-T3): a saved notebook's Markdown cell, made inert
// (NotebookPreview.tsx `inertNotebookMarkdown`).

/// NotebookPreview's `inertNotebookMarkdown` (upstream `NotebookPreview.tsx`), on the blocks this parser made, so
/// whatever it reads as an image or a link is caught: a saved notebook loads no URL and turns no authored link into
/// an app action. An image is its alt text and a link its text; inline code is plain text; raw HTML goes, and a
/// Mermaid fence is text.
fn inert_blocks(blocks: &mut Vec<markdown_parse::Block>) {
    blocks.retain(|block| !matches!(block.kind, markdown_parse::Kind::Html));
    for block in blocks.iter_mut() {
        if matches!(block.kind, markdown_parse::Kind::Image) {
            block.kind = markdown_parse::Kind::Paragraph;
            block.runs = vec![markdown_parse::Run::text(std::mem::take(&mut block.text))];
            block.href.clear();
        }
        if matches!(block.kind, markdown_parse::Kind::Code)
            && block.href.eq_ignore_ascii_case("mermaid")
        {
            block.href = "text".into();
        }
        for run in block
            .runs
            .iter_mut()
            .chain(block.cells.iter_mut().flatten())
        {
            run.href.clear();
            run.code = false;
        }
    }
    blocks.retain(|block| {
        !matches!(block.kind, markdown_parse::Kind::Paragraph)
            || block.runs.iter().any(|run| !run.text.is_empty())
    });
}

#[cfg(test)]
mod notebook_inert_tests {
    use super::*;

    /// Every block kind, and every field but a run's or a block's text that names the attacker's host: an image's
    /// source, a link, a media run's source and the link around it (a chat run is
    /// `[index, text, weight, style, code, href, kind, placement, …, link, …]`).
    fn walk(value: &Value, kinds: &mut Vec<String>, links: &mut Vec<String>) {
        match value {
            Value::Record(fields) => {
                if fields.len() > 6 && matches!(fields[2], Value::Number(_)) {
                    if let Some(kind) = fields[1].as_str() {
                        kinds.push(kind.into());
                    }
                }
                for (index, field) in fields.iter().enumerate() {
                    match field.as_str() {
                        Some(text)
                            if index != 1 && index != 4 && text.contains("attacker.example") =>
                        {
                            links.push(text.into())
                        }
                        Some(_) => {}
                        None => walk(field, kinds, links),
                    }
                }
            }
            Value::List(items) => items.iter().for_each(|item| walk(item, kinds, links)),
            _ => {}
        }
    }

    fn inert(text: &str) -> (Vec<String>, Vec<String>, String) {
        let document = chat_document(Value::str("n"), text, None, false, false, false, true);
        let (mut kinds, mut links) = (Vec::new(), Vec::new());
        walk(&document, &mut kinds, &mut links);
        (kinds, links, format!("{document:?}"))
    }

    // The inputs of the review of #1140, each of which the line-by-line pass this replaces let through.
    const ATTACKS: &[&str] = &[
        "![x](https://attacker.example/a.png)",
        "![x\\]y](https://attacker.example/p.png)",
        "![a [b] c](https://attacker.example/p.png)",
        "[![x](https://attacker.example/a.png)](https://attacker.example/)",
        "[click [here]](https://attacker.example/)",
        "[a [b]](https://attacker.example/)",
        "[click\nhere](https://attacker.example/)",
        "![logo]\n\n> [logo]: https://attacker.example/a.png",
        "![logo]\n\n- [logo]: https://attacker.example/a.png",
        "![logo][l]\n\n[l]: https://attacker.example/a.png",
        "<https://attacker.example/>",
        "see https://attacker.example/x and www.attacker.example",
        "<img\nsrc=\"https://attacker.example/a.png\">",
        "``a`b`` ![x](https://attacker.example/a.png)",
        "```\ncode\n    ```\n![x](https://attacker.example/p.png)\n[docs](https://attacker.example/)",
        "| a |\n| - |\n| ![x](https://attacker.example/t.png) [l](https://attacker.example/) |",
    ];

    #[test]
    fn a_notebook_cell_loads_no_image_and_links_nowhere() {
        for attack in ATTACKS {
            let (kinds, links, document) = inert(attack);
            assert!(
                links.is_empty(),
                "{attack:?} keeps a link or an image source: {links:?}\n{document}"
            );
            assert!(
                !kinds.iter().any(|kind| kind == "image" || kind == "html"),
                "{attack:?}: {kinds:?}"
            );
        }
    }

    #[test]
    fn the_same_cells_outside_a_notebook_do_link() {
        // What the pass removes is there without it, so each case above tests it.
        for attack in [ATTACKS[0], ATTACKS[1], ATTACKS[3], ATTACKS[11], ATTACKS[14]] {
            let document = chat_document(Value::str("a"), attack, None, false, false, false, false);
            let (mut kinds, mut links) = (Vec::new(), Vec::new());
            walk(&document, &mut kinds, &mut links);
            assert!(
                !links.is_empty(),
                "{attack:?} has no link even as an attachment: {kinds:?}"
            );
        }
    }

    #[test]
    fn alt_text_and_link_text_stay_as_text() {
        let (_, _, document) = inert("![a chart](https://attacker.example/a.png)\n\nSee [the docs](https://attacker.example/) and `code`.");
        assert!(
            document.contains("a chart")
                && document.contains("the docs")
                && document.contains("code"),
            "{document}"
        );
    }

    #[test]
    fn a_mermaid_fence_is_text_and_other_fences_keep_their_language() {
        let (kinds, _, document) =
            inert("```mermaid\ngraph TD; A-->B\n```\n\n```python\nprint(1)\n```");
        assert_eq!(
            kinds.iter().filter(|kind| *kind == "code").count(),
            2,
            "{document}"
        );
        assert!(
            document.contains("\"text\"")
                && document.contains("\"python\"")
                && !document.contains("\"mermaid\""),
            "{document}"
        );
    }
}
