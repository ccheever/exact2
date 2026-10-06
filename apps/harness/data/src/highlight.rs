//! Code highlighting, one line at a time, into coloured runs: keywords,
//! strings, comments, numbers and capitalised names (types), in One Dark's
//! colours. Small and forgiving: a token the lexer does not know is text.

use crate::state::{Line, Run};

pub const KEYWORD: &str = "#c678dd";
pub const STRING: &str = "#98c379";
pub const COMMENT: &str = "#7f848e";
pub const NUMBER: &str = "#d19a66";
pub const TYPE: &str = "#e5c07b";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lang {
    Rust,
    Js,
    Python,
    Shell,
    Json,
    Toml,
    Plain,
}

fn lang_of(name: &str) -> Lang {
    match name.trim().to_ascii_lowercase().as_str() {
        "rust" | "rs" => Lang::Rust,
        "js" | "javascript" | "ts" | "typescript" | "jsx" | "tsx" | "mjs" => Lang::Js,
        "py" | "python" => Lang::Python,
        "sh" | "bash" | "shell" | "zsh" | "console" => Lang::Shell,
        "json" | "jsonc" => Lang::Json,
        "toml" | "ini" => Lang::Toml,
        _ => Lang::Plain,
    }
}

fn keywords(lang: Lang) -> &'static [&'static str] {
    match lang {
        Lang::Rust => &[
            "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
            "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
            "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
            "trait", "true", "type", "unsafe", "use", "where", "while",
        ],
        Lang::Js => &[
            "async",
            "await",
            "break",
            "case",
            "catch",
            "class",
            "const",
            "continue",
            "default",
            "delete",
            "do",
            "else",
            "export",
            "extends",
            "false",
            "finally",
            "for",
            "from",
            "function",
            "if",
            "import",
            "in",
            "instanceof",
            "interface",
            "let",
            "new",
            "null",
            "of",
            "return",
            "switch",
            "this",
            "throw",
            "true",
            "try",
            "type",
            "typeof",
            "undefined",
            "var",
            "void",
            "while",
            "yield",
        ],
        Lang::Python => &[
            "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
            "elif", "else", "except", "False", "finally", "for", "from", "global", "if", "import",
            "in", "is", "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return",
            "True", "try", "while", "with", "yield",
        ],
        Lang::Shell => &[
            "case", "do", "done", "elif", "else", "esac", "export", "fi", "for", "function", "if",
            "in", "local", "return", "then", "until", "while", "echo", "cd", "set",
        ],
        Lang::Json => &["true", "false", "null"],
        Lang::Toml => &["true", "false"],
        Lang::Plain => &[],
    }
}

fn line_comment(lang: Lang) -> &'static str {
    match lang {
        Lang::Rust | Lang::Js => "//",
        Lang::Python | Lang::Shell | Lang::Toml => "#",
        Lang::Json | Lang::Plain => "",
    }
}

/// Expand tabs to spaces at stops of `stop` columns.
pub fn expand_tabs(text: &str, stop: usize) -> String {
    if !text.contains('\t') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + 8);
    let mut col = 0;
    for c in text.chars() {
        if c == '\t' {
            let n = stop - col % stop;
            out.extend(std::iter::repeat_n(' ', n));
            col += n;
        } else {
            out.push(c);
            col += unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        }
    }
    out
}

/// Highlight `code` written in `lang`: one line per source line.
pub fn highlight(code: &str, lang: &str) -> Vec<Line> {
    let lang = lang_of(lang);
    let mut in_block = false;
    code.split('\n')
        .map(|l| highlight_line(&expand_tabs(l, 4), lang, &mut in_block))
        .collect()
}

fn push(line: &mut Line, text: &str, fg: &str, italic: bool) {
    line.push(Run {
        text: text.to_string(),
        fg: fg.to_string(),
        italic,
        ..Run::default()
    });
}

fn highlight_line(text: &str, lang: Lang, in_block: &mut bool) -> Line {
    let mut line = Line::default();
    if lang == Lang::Plain {
        push(&mut line, text, "", false);
        return line;
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let at = |i: usize| chars.get(i).map(|c| c.1);
    let byte = |i: usize| chars.get(i).map(|c| c.0).unwrap_or(text.len());
    let comment = line_comment(lang);
    let block_comments = matches!(lang, Lang::Rust | Lang::Js);
    let words = keywords(lang);
    let mut i = 0;
    while i < chars.len() {
        let rest = &text[byte(i)..];
        if *in_block {
            match rest.find("*/") {
                Some(end) => {
                    push(&mut line, &rest[..end + 2], COMMENT, true);
                    *in_block = false;
                    let stop = byte(i) + end + 2;
                    while i < chars.len() && byte(i) < stop {
                        i += 1;
                    }
                }
                None => {
                    push(&mut line, rest, COMMENT, true);
                    i = chars.len();
                }
            }
            continue;
        }
        let c = chars[i].1;
        if !comment.is_empty() && rest.starts_with(comment) {
            // `#` in a shell word (`$#`, `a#b`) is not a comment.
            let prev = i.checked_sub(1).and_then(at);
            if comment != "#" || prev.is_none_or(|p| p.is_whitespace()) {
                push(&mut line, rest, COMMENT, true);
                break;
            }
        }
        if block_comments && rest.starts_with("/*") {
            *in_block = true;
            continue;
        }
        let quote = match c {
            '"' => true,
            '\'' if lang == Lang::Rust => {
                // A char literal ('a', '\n'), not a lifetime ('a).
                at(i + 2) == Some('\'') || (at(i + 1) == Some('\\') && at(i + 3) == Some('\''))
            }
            '\'' | '`' => lang != Lang::Toml || c == '\'',
            _ => false,
        };
        if quote {
            let mut j = i + 1;
            while j < chars.len() && chars[j].1 != c {
                if chars[j].1 == '\\' {
                    j += 1;
                }
                j += 1;
            }
            let end = (j + 1).min(chars.len());
            push(&mut line, &text[byte(i)..byte(end)], STRING, false);
            i = end;
            continue;
        }
        if c.is_ascii_digit() && i.checked_sub(1).and_then(at).is_none_or(|p| !is_word(p)) {
            let mut j = i;
            while at(j).is_some_and(|d| d.is_ascii_alphanumeric() || d == '.' || d == '_') {
                j += 1;
            }
            push(&mut line, &text[byte(i)..byte(j)], NUMBER, false);
            i = j;
            continue;
        }
        if is_word(c) && !c.is_ascii_digit() {
            let mut j = i;
            while at(j).is_some_and(is_word) {
                j += 1;
            }
            let word = &text[byte(i)..byte(j)];
            let fg = if words.contains(&word) {
                KEYWORD
            } else if lang == Lang::Toml && line.runs.iter().all(|r| r.text.trim().is_empty()) {
                // A TOML key.
                "#e06c75"
            } else if word.starts_with(|c: char| c.is_ascii_uppercase())
                && lang != Lang::Shell
                && lang != Lang::Json
            {
                TYPE
            } else {
                ""
            };
            push(&mut line, word, fg, false);
            i = j;
            continue;
        }
        if lang == Lang::Toml && c == '[' && line.runs.is_empty() {
            push(&mut line, rest, KEYWORD, false);
            break;
        }
        push(&mut line, &text[byte(i)..byte(i + 1)], "", false);
        i += 1;
    }
    line
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_tokens_take_their_colours() {
        let lines = highlight(
            "fn main() { let x = \"hi\"; // note\n  Foo::new(42) }",
            "rust",
        );
        assert_eq!(lines.len(), 2);
        let find = |l: &Line, t: &str| l.runs.iter().find(|r| r.text == t).cloned();
        assert_eq!(find(&lines[0], "fn").unwrap().fg, KEYWORD);
        assert_eq!(find(&lines[0], "\"hi\"").unwrap().fg, STRING);
        assert!(find(&lines[0], "// note").unwrap().italic);
        assert_eq!(find(&lines[1], "Foo").unwrap().fg, TYPE);
        assert_eq!(find(&lines[1], "42").unwrap().fg, NUMBER);
        assert_eq!(lines[0].text(), "fn main() { let x = \"hi\"; // note");
    }

    #[test]
    fn block_comments_span_lines_and_tabs_expand() {
        let lines = highlight("a /* one\ntwo */ b\n\tc", "js");
        assert_eq!(lines[1].runs[0].text, "two */");
        assert_eq!(lines[2].text(), "    c");
    }
}
