//! Repairs recorded while recovering a retired spelling.

use super::*;
use crate::migrate::Edit;

impl Parser {
    pub(super) fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            names: NameSpans::default(),
            depth: 0,
            view_depth: 0,
            last: 0,
            free: Vec::new(),
            methods: Vec::new(),
            retired: Vec::new(),
            edits: Vec::new(),
        }
    }

    pub(super) fn retired_view(&mut self, span: Span, replacement: &str) {
        self.retired.push(SyntaxError {
            id: "syntax-view-keyword",
            message: if replacement == "if" {
                "a view chooses with `if`: write `if <condition>`, and `else if` for `else when`"
                    .into()
            } else {
                "a view repeats with `for`: write `for x in xs key=x.id`".into()
            },
            span,
        });
        self.edits.push(Edit::new(span, span, replacement));
    }

    pub(super) fn method_edits(&self, start: usize, name: &str, count: usize) -> Vec<Edit> {
        if name == "length" && count != 1 {
            return Vec::new();
        }
        let close = self.tokens[self.pos - 1].span;
        let mut depth = 0;
        let mut comma = None;
        for token in &self.tokens[start + 2..self.pos - 1] {
            match token.kind {
                TokenKind::Punct("(" | "[" | "{") => depth += 1,
                TokenKind::Punct(")" | "]" | "}") => depth -= 1,
                TokenKind::Punct(",") if depth == 0 => {
                    comma = Some(token.span);
                    break;
                }
                _ => {}
            }
        }
        let name_span = self.tokens[start].span;
        let mut edits = vec![Edit::new(name_span, name_span, "")];
        if let Some(comma) = comma.filter(|_| count > 1) {
            edits.push(Edit::new(comma, comma, format!(").{name}(")));
        } else {
            if let Some(comma) = comma {
                edits.push(Edit::new(comma, comma, ""));
            }
            edits.push(Edit::new(
                close,
                close,
                if name == "length" {
                    ").length".into()
                } else {
                    format!(").{name}()")
                },
            ));
        }
        edits
    }
}

pub(crate) fn parsed_with_edits(
    src: &str,
    source_id: u32,
) -> (Result<File, Vec<SyntaxError>>, Vec<crate::migrate::Edit>) {
    let (tokens, lexed) = Lexer::tokenize_recovering(
        src,
        Span {
            source_id,
            ..Span::point(1, 1)
        },
    );
    let mut p = Parser::new(tokens);
    let parsed = p.file_all();
    if lexed.is_empty() {
        return (parsed, p.edits);
    }
    // A line the lexer refused parses as whatever it managed: not news.
    let bad: Vec<u32> = lexed.iter().map(|e| e.span.line).collect();
    let mut all: Vec<SyntaxError> = lexed.into_iter().map(SyntaxError::from).collect();
    all.extend(
        parsed
            .err()
            .unwrap_or_default()
            .into_iter()
            .filter(|e| !bad.contains(&e.span.line)),
    );
    all.sort_by_key(|e| (e.span.line, e.span.col));
    if !all.iter().any(|e| crate::migrate::is_retired(e.id)) {
        all.truncate(MAX_REFUSALS);
    }
    (Err(all), p.edits)
}
