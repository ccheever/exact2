//! An indent-aware lexer.
//!
//! Lines become `Newline`; a deeper indent emits `Indent`, a shallower one
//! emits as many `Dedent`s as levels closed. Blank lines and `//` comments
//! are trivia — `Blank` and `Comment` tokens that `parse` lifts off the
//! stream onto the file, so the parser never sees them and the printer keeps
//! them (LLP 1035.005 D1). Inside brackets, newlines and indentation are
//! ignored, so a call may span lines. Template strings are lexed whole
//! (backtick to backtick); the parser re-lexes their `${…}` parts.

use crate::Span;

/// A token kind.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// An identifier or keyword.
    Ident(String),
    /// A number literal.
    Number(f64),
    /// A `"…"` string literal, unescaped.
    Str(String),
    /// A `` `…` `` template, raw (escapes preserved for the parser).
    Template(String),
    /// Punctuation or an operator, as written.
    Punct(&'static str),
    /// A `//` comment: `text` is everything after the `//`; `trailing` when
    /// code precedes it on its line. Trivia.
    Comment {
        /// After the `//`, as written.
        text: String,
        /// After code on the same line, rather than on a line of its own.
        trailing: bool,
    },
    /// A run of blank lines outside brackets. Trivia.
    Blank(u32),
    /// End of a logical line.
    Newline,
    /// Indentation increased.
    Indent,
    /// Indentation decreased by one level.
    Dedent,
    /// End of input.
    Eof,
}

/// A token with its position.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// What.
    pub kind: TokenKind,
    /// Where.
    pub span: Span,
}

/// A lexing failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    /// Stable id.
    pub id: &'static str,
    /// What went wrong.
    pub message: String,
    /// Where.
    pub span: Span,
}

const PUNCT: &[&str] = &[
    "==", "!=", "<=", ">=", "&&", "||", "=>", "(", ")", "{", "}", "[", "]", ",", ":", "?", ".",
    "=", "+", "-", "*", "/", "%", "<", ">", "!",
];

/// Tokenizes one source text.
pub struct Lexer;

impl Lexer {
    /// Tokenize `src`, starting line numbers at `first_line`; `first_col`
    /// is the column of `src`'s first byte in its line (1 for a file; where
    /// a `${` ends for a template's expression).
    pub fn tokenize(src: &str, first_line: u32, first_col: u32) -> Result<Vec<Token>, LexError> {
        let mut out = Vec::new();
        let shift = first_col.saturating_sub(1);
        let mut indents: Vec<usize> = vec![0];
        let mut depth = 0usize; // bracket depth
        let mut blank: Option<(u32, u32)> = None; // (first line, count)
        for (i, raw) in src.lines().enumerate() {
            let line_no = first_line + i as u32;
            let line = raw.trim_end();
            let trimmed = line.trim_start();
            if trimmed.is_empty() {
                if depth == 0 {
                    blank = Some(blank.map_or((line_no, 1), |(l, n)| (l, n + 1)));
                }
                continue;
            }
            if let Some((l, n)) = blank.take() {
                out.push(Token {
                    kind: TokenKind::Blank(n),
                    span: Span {
                        line: l,
                        col: 1,
                        end_col: 1,
                    },
                });
            }
            let indent = line.len() - trimmed.len();
            if let Some(text) = trimmed.strip_prefix("//") {
                out.push(Token {
                    kind: TokenKind::Comment {
                        text: text.to_string(),
                        trailing: false,
                    },
                    span: Span {
                        line: line_no,
                        col: indent as u32 + 1 + shift,
                        end_col: line.len() as u32 + 1 + shift,
                    },
                });
                continue;
            }
            let at = |col: usize| Span {
                line: line_no,
                col: col as u32 + 1 + shift,
                end_col: col as u32 + 1 + shift,
            };
            if depth == 0 {
                if line[..indent].contains('\t') {
                    return Err(LexError {
                        id: "syntax-tab-indent",
                        message: "indent with spaces, not tabs".into(),
                        span: at(0),
                    });
                }
                let current = *indents.last().unwrap();
                if indent > current {
                    indents.push(indent);
                    out.push(Token {
                        kind: TokenKind::Indent,
                        span: at(0),
                    });
                } else {
                    while indent < *indents.last().unwrap() {
                        indents.pop();
                        out.push(Token {
                            kind: TokenKind::Dedent,
                            span: at(0),
                        });
                    }
                    if indent != *indents.last().unwrap() {
                        return Err(LexError {
                            id: "syntax-bad-dedent",
                            message: "indentation does not match any enclosing level".into(),
                            span: at(0),
                        });
                    }
                }
            }
            let bytes = trimmed.as_bytes();
            let mut pos = 0usize;
            let span_of = |start: usize, end: usize| Span {
                line: line_no,
                col: (indent + start + 1) as u32 + shift,
                end_col: (indent + end + 1) as u32 + shift,
            };
            while pos < bytes.len() {
                let c = bytes[pos] as char;
                if c == ' ' {
                    pos += 1;
                    continue;
                }
                if let Some(text) = trimmed[pos..].strip_prefix("//") {
                    out.push(Token {
                        kind: TokenKind::Comment {
                            text: text.to_string(),
                            trailing: true,
                        },
                        span: span_of(pos, bytes.len()),
                    });
                    break;
                }
                let span = span_of(pos, pos);
                if c.is_ascii_alphabetic() || c == '_' {
                    // An identifier may contain hyphens — `font-size`,
                    // `aria-label` — as CSS's do; so, as in CSS `calc()`,
                    // subtraction between two names needs spaces (`a - b`),
                    // while `x-1` still lexes as `x`, `-`, `1` (LLP 1017 §8.1).
                    let start = pos;
                    while pos < bytes.len()
                        && ((bytes[pos] as char).is_ascii_alphanumeric()
                            || bytes[pos] == b'_'
                            || (bytes[pos] == b'-'
                                && bytes
                                    .get(pos + 1)
                                    .is_some_and(|n| (*n as char).is_ascii_alphabetic())))
                    {
                        pos += 1;
                    }
                    out.push(Token {
                        kind: TokenKind::Ident(trimmed[start..pos].to_string()),
                        span: span_of(start, pos),
                    });
                    continue;
                }
                if c.is_ascii_digit() {
                    let start = pos;
                    while pos < bytes.len()
                        && ((bytes[pos] as char).is_ascii_digit() || bytes[pos] == b'.')
                    {
                        pos += 1;
                    }
                    let text = &trimmed[start..pos];
                    let n: f64 = text.parse().map_err(|_| LexError {
                        id: "syntax-bad-number",
                        message: format!("`{text}` is not a number"),
                        span,
                    })?;
                    out.push(Token {
                        kind: TokenKind::Number(n),
                        span: span_of(start, pos),
                    });
                    continue;
                }
                if c == '"' {
                    let (s, end) = Self::string(trimmed, pos, '"', span)?;
                    out.push(Token {
                        kind: TokenKind::Str(s),
                        span: span_of(pos, end),
                    });
                    pos = end;
                    continue;
                }
                if c == '`' {
                    let end = template_literal_end(trimmed, pos).ok_or(LexError {
                        id: "syntax-unterminated-template",
                        message: "template string never closes".into(),
                        span,
                    })?;
                    out.push(Token {
                        kind: TokenKind::Template(trimmed[pos + 1..end].to_string()),
                        span: span_of(pos, end + 1),
                    });
                    pos = end + 1;
                    continue;
                }
                let mut matched = None;
                for p in PUNCT {
                    if trimmed[pos..].starts_with(p) {
                        matched = Some(*p);
                        break;
                    }
                }
                let p = matched.ok_or(LexError {
                    id: "syntax-unexpected-char",
                    message: format!("unexpected `{c}`"),
                    span,
                })?;
                match p {
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" => depth = depth.saturating_sub(1),
                    _ => {}
                }
                out.push(Token {
                    kind: TokenKind::Punct(p),
                    span: span_of(pos, pos + p.len()),
                });
                pos += p.len();
            }
            if depth == 0 {
                out.push(Token {
                    kind: TokenKind::Newline,
                    span: span_of(bytes.len(), bytes.len()),
                });
            }
        }
        let end = Span {
            line: first_line + src.lines().count() as u32,
            col: 1,
            end_col: 1,
        };
        if let Some((l, n)) = blank.take() {
            out.push(Token {
                kind: TokenKind::Blank(n),
                span: Span {
                    line: l,
                    col: 1,
                    end_col: 1,
                },
            });
        }
        while indents.len() > 1 {
            indents.pop();
            out.push(Token {
                kind: TokenKind::Dedent,
                span: end,
            });
        }
        out.push(Token {
            kind: TokenKind::Eof,
            span: end,
        });
        Ok(out)
    }

    fn string(
        text: &str,
        start: usize,
        quote: char,
        span: Span,
    ) -> Result<(String, usize), LexError> {
        let mut out = String::new();
        let mut chars = text[start + 1..].char_indices();
        while let Some((i, c)) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some((_, 'n')) => out.push('\n'),
                    Some((_, 't')) => out.push('\t'),
                    Some((_, '"')) => out.push('"'),
                    Some((_, '\\')) => out.push('\\'),
                    Some((_, '`')) => out.push('`'),
                    Some((_, '$')) => out.push('$'),
                    _ => {
                        return Err(LexError {
                            id: "syntax-bad-escape",
                            message: "unknown escape".into(),
                            span,
                        })
                    }
                },
                c if c == quote => return Ok((out, start + 1 + i + 1)),
                c => out.push(c),
            }
        }
        Err(LexError {
            id: "syntax-unterminated-string",
            message: "string never closes".into(),
            span,
        })
    }
}

/// The byte offset of the backtick closing a template literal. Nested
/// `${…}` expressions may themselves contain strings, braces, or templates.
pub(crate) fn template_literal_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut pos = start + 1;
    while pos < bytes.len() {
        match bytes[pos] {
            b'\\' => pos = skip_escaped(text, pos),
            b'`' => return Some(pos),
            b'$' if bytes.get(pos + 1) == Some(&b'{') => {
                let end = template_expr_end(&text[pos + 2..])?;
                pos += end + 3;
            }
            _ => pos += char_len(text, pos),
        }
    }
    None
}

/// The byte offset of the `}` matching an expression immediately after
/// `${`. Braces in strings and nested template literals do not close it.
pub(crate) fn template_expr_end(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut pos = 0;
    let mut depth = 0usize;
    while pos < bytes.len() {
        match bytes[pos] {
            b'\\' => pos = skip_escaped(text, pos),
            b'"' => pos = quoted_end(text, pos, b'"')?,
            b'`' => pos = template_literal_end(text, pos)? + 1,
            b'{' => {
                depth += 1;
                pos += 1;
            }
            b'}' if depth == 0 => return Some(pos),
            b'}' => {
                depth -= 1;
                pos += 1;
            }
            _ => pos += char_len(text, pos),
        }
    }
    None
}

fn quoted_end(text: &str, start: usize, quote: u8) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut pos = start + 1;
    while pos < bytes.len() {
        match bytes[pos] {
            b'\\' => pos = skip_escaped(text, pos),
            byte if byte == quote => return Some(pos + 1),
            _ => pos += char_len(text, pos),
        }
    }
    None
}

fn skip_escaped(text: &str, slash: usize) -> usize {
    let next = slash + 1;
    if next >= text.len() {
        next
    } else {
        next + char_len(text, next)
    }
}

fn char_len(text: &str, pos: usize) -> usize {
    text[pos..].chars().next().map(char::len_utf8).unwrap_or(1)
}
