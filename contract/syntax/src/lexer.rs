//! An indent-aware lexer.
//!
//! Lines become `Newline`; a deeper indent emits `Indent`, a shallower one
//! emits as many `Dedent`s as levels closed. Blank lines and `//` comments
//! are skipped. Inside brackets, newlines and indentation are ignored, so a
//! call may span lines. Template strings are lexed whole (backtick to
//! backtick); the parser re-lexes their `${…}` parts.

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
    /// Tokenize `src`, starting line numbers at `first_line`.
    pub fn tokenize(src: &str, first_line: u32) -> Result<Vec<Token>, LexError> {
        Self::tokenize_at(src, Span::point(first_line, 1))
    }

    /// Tokenize source starting at `origin`, retaining its file identity.
    pub fn tokenize_at(src: &str, origin: Span) -> Result<Vec<Token>, LexError> {
        Self::tokenize_all(src, origin).map_err(|mut all| all.swap_remove(0))
    }

    /// Tokenize, reporting a refusal on every line that has one (at most
    /// [`crate::parser::MAX_REFUSALS`]).
    pub fn tokenize_all(src: &str, origin: Span) -> Result<Vec<Token>, Vec<LexError>> {
        match Self::tokenize_recovering(src, origin) {
            (tokens, errors) if errors.is_empty() => Ok(tokens),
            (_, errors) => Err(errors),
        }
    }

    /// Whether the innermost open block holds only `name=` lines: an
    /// element's continued attributes, with no nested block of its own.
    fn attributes_only(out: &[Token]) -> bool {
        let Some(open) = out
            .iter()
            .rposition(|t| matches!(t.kind, TokenKind::Indent))
        else {
            return false;
        };
        let block = &out[open + 1..];
        !block.iter().any(|t| matches!(t.kind, TokenKind::Dedent))
            && block
                .split(|t| matches!(t.kind, TokenKind::Newline))
                .filter(|line| !line.is_empty())
                .all(|line| {
                    matches!(
                        (line.first().map(|t| &t.kind), line.get(1).map(|t| &t.kind)),
                        (Some(TokenKind::Ident(_)), Some(TokenKind::Punct("=")))
                    )
                })
    }

    /// Tokens and every line's refusal. A refused line ends where it failed
    /// (brackets it opened are closed), so the lines after it lex, and parse,
    /// as they would without it.
    pub(crate) fn tokenize_recovering(src: &str, origin: Span) -> (Vec<Token>, Vec<LexError>) {
        let mut out = Vec::new();
        let mut indents: Vec<usize> = vec![0];
        let mut depth = 0usize; // bracket depth
        let mut errors = Vec::new();
        let line = |out: &mut Vec<Token>,
                    indents: &mut Vec<usize>,
                    depth: &mut usize,
                    i: usize,
                    raw: &str|
         -> Result<(), LexError> {
            let line_no = origin.line + i as u32;
            let first_col = if i == 0 { origin.col } else { 1 };
            let line = raw.trim_end();
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with("//") {
                return Ok(());
            }
            let indent = line.len() - trimmed.len();
            // A declaration keyword in the first column cannot continue an
            // expression (it is never a value): a bracket left open above
            // ends here, so what follows lexes as its own declaration.
            let word = trimmed.split(|c: char| !c.is_ascii_alphanumeric()).next();
            if *depth > 0
                && indent == 0
                && matches!(
                    word,
                    Some("component" | "font" | "shape" | "style" | "fn" | "test")
                )
            {
                *depth = 0;
                // The open line ends just after its last token.
                let span = out.last().map_or(
                    Span {
                        source_id: origin.source_id,
                        ..Span::point(line_no, first_col)
                    },
                    |last| Span {
                        col: last.span.end_col,
                        ..last.span
                    },
                );
                out.push(Token {
                    kind: TokenKind::Newline,
                    span,
                });
            }
            if *depth == 0 {
                if line[..indent].contains('\t') {
                    return Err(LexError {
                        id: "syntax-tab-indent",
                        message: "indent with spaces, not tabs".into(),
                        span: Span {
                            source_id: origin.source_id,
                            ..Span::point(line_no, first_col)
                        },
                    });
                }
                let current = *indents.last().unwrap();
                if indent > current {
                    indents.push(indent);
                    out.push(Token {
                        kind: TokenKind::Indent,
                        span: Span {
                            source_id: origin.source_id,
                            ..Span::point(line_no, first_col)
                        },
                    });
                } else {
                    let mut popped = 0;
                    while indent < *indents.last().unwrap() {
                        indents.pop();
                        popped += 1;
                        out.push(Token {
                            kind: TokenKind::Dedent,
                            span: Span {
                                source_id: origin.source_id,
                                ..Span::point(line_no, first_col)
                            },
                        });
                    }
                    // An element's continued `name=` lines may sit deeper than
                    // its children: the children then open the same block at
                    // their own level, as if the attributes had been indented to it.
                    if popped == 1
                        && indent > *indents.last().unwrap()
                        && Self::attributes_only(&out[..out.len() - 1])
                    {
                        out.pop();
                        indents.push(indent);
                    }
                    if indent != *indents.last().unwrap() {
                        return Err(LexError {
                            id: "syntax-bad-dedent",
                            message: "indentation does not match any enclosing level".into(),
                            span: Span {
                                source_id: origin.source_id,
                                ..Span::point(line_no, first_col)
                            },
                        });
                    }
                }
            }
            let bytes = trimmed.as_bytes();
            let mut pos = 0usize;
            let col_of = |pos: usize| Span {
                source_id: origin.source_id,
                ..Span::point(line_no, first_col + (indent + pos) as u32)
            };
            while pos < bytes.len() {
                let c = bytes[pos] as char;
                if c == ' ' {
                    pos += 1;
                    continue;
                }
                if trimmed[pos..].starts_with("//") {
                    break;
                }
                let span = col_of(pos);
                // A vendor-prefixed CSS name, as the Compat Standard spells
                // some (`-webkit-text-stroke`, LLP 1077 D7): the two
                // prefixes only, and only as an attribute's name (followed
                // by `=`), so `-webkit-x` in an expression stays a negation.
                let vendor = ["-webkit-", "-apple-"].iter().any(|p| {
                    trimmed[pos..].starts_with(p)
                        && bytes
                            .get(pos + p.len())
                            .is_some_and(|n| (*n as char).is_ascii_alphabetic())
                        && {
                            let rest = &trimmed[pos + 1..];
                            let end = rest
                                .find(|c: char| {
                                    !(c.is_ascii_alphanumeric() || c == '-' || c == '_')
                                })
                                .unwrap_or(rest.len());
                            let after = rest[end..].trim_start_matches([' ', '\t']);
                            after.starts_with('=') && !after.starts_with("==")
                        }
                });
                if c.is_ascii_alphabetic() || c == '_' || vendor {
                    // An identifier may contain hyphens — `font-size`,
                    // `aria-label` — as CSS's do; so, as in CSS `calc()`,
                    // subtraction between two names needs spaces (`a - b`),
                    // while `x-1` still lexes as `x`, `-`, `1` (LLP 1017 §8.1).
                    let start = pos;
                    if vendor {
                        pos += 1;
                    }
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
                        span: Span {
                            end_col: col_of(pos).col,
                            ..span
                        },
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
                        span: Span {
                            end_col: col_of(pos).col,
                            ..span
                        },
                    });
                    continue;
                }
                if c == '"' {
                    let (s, end) = Self::string(trimmed, pos, '"', span)?;
                    out.push(Token {
                        kind: TokenKind::Str(s),
                        span: Span {
                            end_col: col_of(end).col,
                            ..span
                        },
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
                        span: Span {
                            end_col: col_of(end + 1).col,
                            ..span
                        },
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
                    "(" | "[" | "{" => *depth += 1,
                    ")" | "]" | "}" => *depth = depth.saturating_sub(1),
                    _ => {}
                }
                out.push(Token {
                    kind: TokenKind::Punct(p),
                    span: Span {
                        end_col: col_of(pos + p.len()).col,
                        ..span
                    },
                });
                pos += p.len();
            }
            if *depth == 0 {
                out.push(Token {
                    kind: TokenKind::Newline,
                    span: col_of(bytes.len()),
                });
            }
            Ok(())
        };
        for (i, raw) in src.lines().enumerate() {
            let before = out.len();
            if let Err(e) = line(&mut out, &mut indents, &mut depth, i, raw) {
                if out.len() > before {
                    depth = 0;
                    out.push(Token {
                        kind: TokenKind::Newline,
                        span: e.span,
                    });
                }
                errors.push(e);
                if errors.len() >= crate::parser::MAX_REFUSALS {
                    break;
                }
            }
        }
        let end = Span {
            source_id: origin.source_id,
            ..Span::point(origin.line + src.lines().count() as u32, 1)
        };
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
        (out, errors)
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
