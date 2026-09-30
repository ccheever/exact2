//! Inline Markdown as one line of text (markdown.rs `render_inline`, minus
//! the styling): the summary lines and the rail's statuses render an
//! agent's last message, which often carries `**bold**`, `` `code` `` and
//! `[links](url)`. The words stay; the marks go.

/// `text` with inline marks removed: emphasis and code fences dropped,
/// links reduced to their text, autolinks kept, whitespace collapsed.
pub fn inline_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' if i + 1 < chars.len() => {
                out.push(chars[i + 1]);
                i += 2;
            }
            '*' | '_' | '`' | '~' => {
                // A run of marks is a mark; a lone underscore inside a word is text.
                if c == '_'
                    && i > 0
                    && chars[i - 1].is_alphanumeric()
                    && chars.get(i + 1).is_some_and(|n| n.is_alphanumeric())
                {
                    out.push(c);
                }
                i += 1;
            }
            '[' => {
                // `[text](url)` → text; `[text]` alone stays.
                if let Some(close) = chars[i + 1..]
                    .iter()
                    .position(|&x| x == ']')
                    .map(|p| p + i + 1)
                {
                    if chars.get(close + 1) == Some(&'(') {
                        if let Some(end) = chars[close + 2..]
                            .iter()
                            .position(|&x| x == ')')
                            .map(|p| p + close + 2)
                        {
                            out.push_str(&inline_text(
                                &chars[i + 1..close].iter().collect::<String>(),
                            ));
                            i = end + 1;
                            continue;
                        }
                    }
                }
                out.push(c);
                i += 1;
            }
            '<' => {
                // `<https://…>` autolinks keep their address.
                if let Some(close) = chars[i + 1..]
                    .iter()
                    .position(|&x| x == '>')
                    .map(|p| p + i + 1)
                {
                    let inner: String = chars[i + 1..close].iter().collect();
                    if inner.starts_with("http://")
                        || inner.starts_with("https://")
                        || inner.starts_with("mailto:")
                    {
                        out.push_str(&inner);
                        i = close + 1;
                        continue;
                    }
                }
                out.push(c);
                i += 1;
            }
            '\n' | '\r' | '\t' => {
                out.push(' ');
                i += 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    let mut collapsed = String::with_capacity(out.len());
    let mut space = false;
    for c in out.trim().chars() {
        if c == ' ' {
            if !space {
                collapsed.push(' ');
            }
            space = true;
        } else {
            collapsed.push(c);
            space = false;
        }
    }
    collapsed
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
}
