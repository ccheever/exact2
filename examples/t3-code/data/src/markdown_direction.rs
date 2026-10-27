// T3 Code 7c3b3b3605ef (MIT, see LICENSE-T3): apps/web/src/textDirection.ts and
// components/chat/markdownTextDirection.ts. Each top-level block takes the direction of its
// own words, so an Arabic or Hebrew reply reads right to left beside English ones; a list's or
// quote's lines follow the whole list or quote, and code keeps left to right.

/// Hebrew through Arabic Extended-A, their presentation forms, and the supplementary
/// right-to-left blocks (textDirection's RTL_LETTER).
fn rtl_letter(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
}

/// textDirection: words are counted so mixed prose follows its main language (`dir="auto"` reads only the first
/// strong letter); a tie goes to the first word, and text without letters has none ("").
fn text_direction(text: &str) -> &'static str {
    let (mut rtl, mut ltr, mut first) = (0usize, 0usize, "");
    for word in text.split_whitespace() {
        let Some(letter) = word.chars().find(|c| {
            unicode_general_category::get_general_category(*c)
                .abbreviation()
                .starts_with('L')
        }) else {
            continue;
        };
        let direction = if rtl_letter(letter) { "rtl" } else { "ltr" };
        if first.is_empty() {
            first = direction;
        }
        if direction == "rtl" {
            rtl += 1;
        } else {
            ltr += 1;
        }
    }
    if rtl == ltr {
        first
    } else if rtl > ltr {
        "rtl"
    } else {
        "ltr"
    }
}

/// proseText: a block's words, inline code left out (a space) and each run, cell or item apart.
fn prose_text(block: &markdown_parse::Block, out: &mut String) {
    for run in block.runs.iter().chain(block.cells.iter().flatten()) {
        out.push(' ');
        if !run.code {
            out.push_str(&run.text);
        }
    }
    if matches!(block.kind, markdown_parse::Kind::Html) {
        out.push(' ');
        out.push_str(&block.text);
    }
    out.push(' ');
}

/// rehypeTextDirection over the parser's flat blocks: a run of list items is one list, of quote lines one quote, of
/// table rows one table; a code block, a rule and an image have no words of their own. One direction per block index.
fn block_directions(blocks: &[markdown_parse::Block]) -> Vec<&'static str> {
    use markdown_parse::Kind;
    let group = |kind: Kind| match kind {
        Kind::Item | Kind::Quote | Kind::TableRow => Some(kind),
        _ => None,
    };
    let mut directions = vec![""; blocks.len()];
    let mut start = 0;
    while start < blocks.len() {
        let mut end = start + 1;
        if let Some(kind) = group(blocks[start].kind) {
            while end < blocks.len() && blocks[end].kind == kind {
                end += 1;
            }
        }
        if !matches!(blocks[start].kind, Kind::Code | Kind::Rule | Kind::Image) {
            let mut text = String::new();
            blocks[start..end]
                .iter()
                .for_each(|block| prose_text(block, &mut text));
            let direction = text_direction(&text);
            directions[start..end]
                .iter_mut()
                .for_each(|slot| *slot = direction);
        }
        start = end;
    }
    directions
}

#[cfg(test)]
mod direction_tests {
    use super::*;

    #[test]
    fn words_decide_and_a_tie_goes_to_the_first() {
        for (text, direction) in [
            ("اكتب دالة تجمع الأرقام", "rtl"),
            ("תקן את הבאג", "rtl"),
            ("fix the failing test", "ltr"),
            ("StrictMode: يعيد الـ render مرتين في التطوير", "rtl"),
            ("translate the label مرحبا for the login screen", "ltr"),
            ("React مكتبة", "ltr"),
            ("مكتبة React", "rtl"),
            ("عدّل السطر 42، ثم شغّل الاختبارات (3)", "rtl"),
            ("42 — 7 ✓", ""),
        ] {
            assert_eq!(text_direction(text), direction, "{text}");
        }
    }

    #[test]
    fn a_list_follows_its_words_and_inline_code_is_not_counted() {
        let parse = |text: &str| markdown_parse::parse(text, &|href| href.to_string()).blocks;
        let blocks = parse("- مرحبا\n- hello world\n\nاستخدم `useEffect useLayoutEffect React` هنا\n\n```\n// احسب المجموع هنا\n```");
        let directions = block_directions(&blocks);
        let kinds: Vec<_> = blocks.iter().map(|block| block.kind.name()).collect();
        assert_eq!(kinds, ["item", "item", "paragraph", "code"]);
        assert_eq!(directions, ["ltr", "ltr", "rtl", ""]);
    }
}
