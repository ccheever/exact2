// T3 Code 9c161462c9 (MIT, see LICENSE-T3), apps/web/src/components/ChatMarkdown.tsx:
// `rehypeHeadingIds` and `githubHeadingSlug` (each heading's id), `findMarkdownFragmentTarget`
// (which heading a `#fragment` link scrolls to) and `orderedListGutterStyle` (the marker gutter of
// an ordered list whose widest marker has several digits).

/// The sanitizer's id prefix, which heading ids carry and fragment lookup strips.
const SANITIZED_FRAGMENT_PREFIX: &str = "user-content-";

/// githubHeadingSlug: trimmed and lowercased; every character but a letter, a mark, a number, a
/// connector punctuation (`_`), a space or `-` dropped; each space a `-`.
fn github_heading_slug(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .filter(|&c| c == ' ' || c == '-' || slug_character(c))
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// GitHub keeps Unicode letters, marks, numbers and connector punctuation.
fn slug_character(c: char) -> bool {
    let category = unicode_general_category::get_general_category(c).abbreviation();
    category.starts_with(['L', 'M', 'N']) || category == "Pc"
}

/// rehypeHeadingIds: each heading's id (`user-content-` and its slug) in its `href`, unique in the
/// document: a repeated slug takes the first free `-1`, `-2` … suffix, so `Setup`, `Setup`,
/// `Setup-1` get three ids. A heading with no slug keeps none ("").
fn heading_ids(blocks: &mut [markdown_parse::Block]) {
    let mut taken = std::collections::HashSet::new();
    let mut next_suffix = std::collections::HashMap::new();
    for block in blocks.iter_mut() {
        if !matches!(block.kind, markdown_parse::Kind::Heading) {
            continue;
        }
        let slug = github_heading_slug(&markdown_parse::plain(&block.runs));
        if slug.is_empty() {
            block.href.clear();
            continue;
        }
        let mut count = next_suffix.get(&slug).copied().unwrap_or(0);
        let mut id = format!("{SANITIZED_FRAGMENT_PREFIX}{slug}");
        while taken.contains(&id) {
            count += 1;
            id = format!("{SANITIZED_FRAGMENT_PREFIX}{slug}-{count}");
        }
        next_suffix.insert(slug, count);
        taken.insert(id.clone());
        block.href = id;
    }
}

/// decodeMarkdownFragmentId: the fragment percent-decoded, or as written when it does not decode.
fn decode_fragment(fragment: &str) -> String {
    let bytes = fragment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = bytes
                .get(at + 1..at + 3)
                .and_then(|pair| std::str::from_utf8(pair).ok())
                .and_then(|pair| u8::from_str_radix(pair, 16).ok());
            match hex {
                Some(byte) => {
                    out.push(byte);
                    at += 3;
                    continue;
                }
                None => return fragment.to_string(),
            }
        }
        out.push(bytes[at]);
        at += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| fragment.to_string())
}

/// normalizeSanitizedFragmentId: every leading `user-content-` removed.
fn normalize_fragment(id: &str) -> &str {
    let mut id = id;
    while let Some(rest) = id.strip_prefix(SANITIZED_FRAGMENT_PREFIX) {
        id = rest;
    }
    id
}

/// findMarkdownFragmentTarget within the message: a `#fragment` link's `href` becomes `#` and the id
/// of the first heading it names (by its decoded id, or with the `user-content-` prefix ignored), so
/// the press scrolls to that heading (markdown.contract `fragmentJump`). A fragment no heading has
/// stays as written, and its press does nothing.
fn fragment_links(blocks: &mut [markdown_parse::Block]) {
    let ids: Vec<String> = blocks
        .iter()
        .filter(|block| matches!(block.kind, markdown_parse::Kind::Heading) && !block.href.is_empty())
        .map(|block| block.href.clone())
        .collect();
    if ids.is_empty() {
        return;
    }
    let target = |href: &str| -> Option<String> {
        let decoded = decode_fragment(href.strip_prefix('#')?);
        let normalized = normalize_fragment(&decoded);
        ids.iter()
            .find(|id| **id == decoded || normalize_fragment(id) == normalized)
            .map(|id| format!("#{id}"))
    };
    for block in blocks.iter_mut() {
        let cells = block.cells.iter_mut().flat_map(|cell| cell.iter_mut());
        for run in block.runs.iter_mut().chain(cells) {
            if let Some(resolved) = run.href.starts_with('#').then(|| target(&run.href)).flatten() {
                run.href = resolved;
            }
        }
    }
}

/// orderedListGutterStyle: each list item's marker gutter in `ch`, 0 for the default 1.25rem. An
/// ordered list whose widest marker has several digits widens it to the digits, the period and the
/// trailing space (`--list-gutter: <digits + 2>ch`). A list is the items at one depth, up to a
/// shallower item or any other block; the widest marker drawn sizes all of its items.
fn list_gutters(blocks: &[markdown_parse::Block]) -> Vec<f64> {
    let digits = |marker: &str| {
        let count = marker.chars().take_while(char::is_ascii_digit).count();
        (count > 0 && count + 1 == marker.chars().count()).then_some(count)
    };
    let mut lists: Vec<Vec<usize>> = Vec::new();
    // The lists still open, outermost first: (depth, ordered, index into `lists`). A bullet
    // after a numbered item at the same depth starts another list.
    let mut open: Vec<(u32, bool, usize)> = Vec::new();
    for (at, block) in blocks.iter().enumerate() {
        if !matches!(block.kind, markdown_parse::Kind::Item) {
            open.clear();
            continue;
        }
        let ordered = block.marker != "•";
        open.retain(|&(depth, kind, _)| depth < block.depth || depth == block.depth && kind == ordered);
        match open.last() {
            Some(&(depth, _, list)) if depth == block.depth => lists[list].push(at),
            _ => {
                open.push((block.depth, ordered, lists.len()));
                lists.push(vec![at]);
            }
        }
    }
    let mut gutters = vec![0.0; blocks.len()];
    for list in lists {
        let widest = list.iter().filter_map(|&at| digits(&blocks[at].marker)).max();
        if let Some(widest) = widest.filter(|&widest| widest > 1) {
            for at in list {
                gutters[at] = (widest + 2) as f64;
            }
        }
    }
    gutters
}

#[cfg(test)]
mod heading_tests {
    use super::*;

    fn parsed(text: &str) -> Vec<markdown_parse::Block> {
        let mut doc = markdown_parse::parse(text, &|href| link_href(href));
        heading_ids(&mut doc.blocks);
        fragment_links(&mut doc.blocks);
        doc.blocks
    }

    fn heading_hrefs(blocks: &[markdown_parse::Block]) -> Vec<&str> {
        blocks
            .iter()
            .filter(|block| matches!(block.kind, markdown_parse::Kind::Heading))
            .map(|block| block.href.as_str())
            .collect()
    }

    #[test]
    fn headings_take_github_slug_ids_that_never_repeat() {
        let blocks = parsed("## Setup\n\n## Setup\n\n## Setup-1\n\n## Install\n\n## Install");
        assert_eq!(
            heading_hrefs(&blocks),
            [
                "user-content-setup",
                "user-content-setup-1",
                "user-content-setup-1-1",
                "user-content-install",
                "user-content-install-1",
            ]
        );
        let blocks = parsed("# Top\n\n###### Fine print\n\n## 1. Operating model\n\n## Café `crate::x`!");
        assert_eq!(
            heading_hrefs(&blocks),
            [
                "user-content-top",
                "user-content-fine-print",
                "user-content-1-operating-model",
                "user-content-café-cratex",
            ]
        );
    }

    #[test]
    fn fragment_links_name_their_heading_or_stay_as_written() {
        let blocks = parsed(
            "- [Operating model](#1-operating-model)\n- [Prefixed](#user-content-1-operating-model)\n- [Encoded](#caf%C3%A9)\n- [Missing](#nowhere)\n\n## 1. Operating model\n\n## 1. Operating model\n\n## Café",
        );
        let hrefs: Vec<&str> = blocks
            .iter()
            .flat_map(|block| block.runs.iter())
            .filter(|run| run.href.starts_with('#'))
            .map(|run| run.href.as_str())
            .collect();
        assert_eq!(
            hrefs,
            [
                "#user-content-1-operating-model",
                "#user-content-1-operating-model",
                "#user-content-café",
                "#nowhere",
            ]
        );
    }

    #[test]
    fn bare_anchor_placeholders_stay_literal_and_unicode_marks_survive_slugs() {
        let blocks = parsed("A placeholder <A> stays text.\n\n## Music \u{1D165}");
        assert_eq!(markdown_parse::plain(&blocks[0].runs), "A placeholder <A> stays text.");
        assert_eq!(heading_hrefs(&blocks), ["user-content-music-\u{1D165}"]);
    }

    #[test]
    fn wide_ordered_markers_widen_their_whole_list() {
        let blocks = parsed("9. nine\n10. ten\n    - nested\n11. eleven\n\ntext\n\n1. one\n2. two\n\n- a\n- b");
        let gutters: Vec<f64> = list_gutters(&blocks)
            .into_iter()
            .zip(&blocks)
            .filter(|(_, block)| matches!(block.kind, markdown_parse::Kind::Item))
            .map(|(gutter, _)| gutter)
            .collect();
        assert_eq!(gutters, [4.0, 4.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0]);
    }
}
