//! Expressions: precedence climbing over binary operators, prefix
//! operators, postfix field access, and primaries (literals, templates,
//! calls, `some`, inline `match`), with the depth each node is built at.

use super::*;

/// How deeply expressions may nest in the source (parentheses, prefix
/// operators, arguments, branches), and how deep one expression's tree may
/// be (a long `+` chain is a deep tree). Every later pass recurses over the
/// tree; these bounds keep a compile inside a 2 MB thread's stack even
/// unoptimized (a release build has room for ten times as much), and past
/// them is a refusal, never a stack overflow. No app's deepest expression
/// is a third of either (21 levels, 2026-09-22).
const MAX_NESTING: u32 = 64;
const MAX_TREE_DEPTH: usize = 100;

impl Parser {
    pub(super) fn expr(&mut self) -> R<Expr> {
        self.nest()?;
        let e = self.ternary();
        self.depth -= 1;
        e
    }

    /// Record the depth of a node just built over children `below` deep,
    /// refusing it past the bound before anything deeper exists.
    pub(super) fn built(&mut self, below: usize, span: Span) -> R<()> {
        self.last = below + 1;
        if self.last > MAX_TREE_DEPTH {
            return Err(SyntaxError {
                id: "syntax-expression-depth",
                message: format!(
                    "this expression is more than {MAX_TREE_DEPTH} operations deep: split it into `derive`s"
                ),
                span,
            });
        }
        Ok(())
    }

    pub(super) fn nest(&mut self) -> R<()> {
        self.depth += 1;
        if self.depth > MAX_NESTING {
            self.depth -= 1;
            return self.err(
                "syntax-expression-depth",
                format!("expressions nest more than {MAX_NESTING} deep here: name the inner ones with `derive`"),
            );
        }
        Ok(())
    }

    pub(super) fn ternary(&mut self) -> R<Expr> {
        let cond = self.binary(0)?;
        if self.eat_punct("?") {
            let below = self.last;
            let a = self.expr()?;
            let below = below.max(self.last);
            // A ternary broken over lines outside brackets (authoring bench): the
            // next line starts with its `:`.
            let next = self.tokens[self.pos..]
                .iter()
                .find(|t| !matches!(t.kind, TokenKind::Newline | TokenKind::Indent));
            if matches!(self.peek_kind(), TokenKind::Newline | TokenKind::Indent)
                && matches!(next.map(|t| &t.kind), Some(TokenKind::Punct(":")))
            {
                return self.err(
                    "syntax-expected",
                    "the ternary's `:` is on the next line, and a line ends an expression \
                     outside brackets: wrap the whole ternary in parentheses, as in \
                     `let label = (done\n      ? \"Done\"\n      : \"Open\")`",
                );
            }
            self.expect_punct(":")?;
            let b = self.expr()?;
            let span = cond.span();
            self.built(below.max(self.last), span)?;
            return Ok(Expr::Ternary(
                Box::new(cond),
                Box::new(a),
                Box::new(b),
                span,
            ));
        }
        Ok(cond)
    }

    pub(super) fn binary(&mut self, min_prec: u8) -> R<Expr> {
        let mut left = self.unary()?;
        let mut depth = self.last;
        loop {
            let (op, prec) = match self.peek_kind() {
                TokenKind::Punct("||") => (BinOp::Or, 1),
                TokenKind::Ident(w) if w == "or" => (BinOp::Or, 1),
                TokenKind::Punct("&&") => (BinOp::And, 2),
                TokenKind::Ident(w) if w == "and" => (BinOp::And, 2),
                TokenKind::Punct("==") => (BinOp::Eq, 3),
                TokenKind::Punct("!=") => (BinOp::Ne, 3),
                TokenKind::Punct("<") => (BinOp::Lt, 4),
                TokenKind::Punct("<=") => (BinOp::Le, 4),
                TokenKind::Punct(">") => (BinOp::Gt, 4),
                TokenKind::Punct(">=") => (BinOp::Ge, 4),
                TokenKind::Punct("+") => (BinOp::Add, 5),
                TokenKind::Punct("-") => (BinOp::Sub, 5),
                TokenKind::Punct("*") => (BinOp::Mul, 6),
                TokenKind::Punct("/") => (BinOp::Div, 6),
                TokenKind::Punct("%") => (BinOp::Rem, 6),
                _ => break,
            };
            if prec < min_prec {
                break;
            }
            self.next();
            let right = self.binary(prec + 1)?;
            let span = left.span();
            self.built(depth.max(self.last), span)?;
            depth = self.last;
            left = Expr::Binary(op, Box::new(left), Box::new(right), span);
        }
        self.last = depth;
        Ok(left)
    }

    pub(super) fn unary(&mut self) -> R<Expr> {
        let op = if self.at_punct("-") {
            UnOp::Neg
        } else if self.at_punct("!") || self.at_ident("not") {
            UnOp::Not
        } else {
            return self.postfix();
        };
        let span = self.next().span;
        self.nest()?;
        let e = self.unary();
        self.depth -= 1;
        let e = e?;
        self.built(self.last, span)?;
        Ok(Expr::Unary(op, Box::new(e), span))
    }

    pub(super) fn postfix(&mut self) -> R<Expr> {
        let mut e = self.primary()?;
        while self.at_punct(".") {
            self.next();
            let (field, span) = self.field_name()?;
            // `xs.map(f)`: the web's method call, directly after the name.
            let next = self.peek().span;
            if self.at_punct("(")
                && next.line == span.line
                && next.col == span.col + field.chars().count() as u32
            {
                return Err(SyntaxError {
                    id: "syntax-method-call",
                    message: format!(
                        "`.{field}(…)` is a method call: {}",
                        crate::idioms::method_fix(&field)
                    ),
                    span,
                });
            }
            self.built(self.last, span)?;
            e = Expr::Member(Box::new(e), field, span);
        }
        Ok(e)
    }

    pub(super) fn primary(&mut self) -> R<Expr> {
        let t = self.next();
        let span = t.span;
        // A leaf's depth; a composite records its own below.
        self.last = 1;
        match t.kind {
            TokenKind::Number(n) => Ok(Expr::Number(n, span)),
            TokenKind::Str(s) => Ok(Expr::Str(s, span)),
            TokenKind::Template(raw) => self.template(&raw, span),
            TokenKind::Punct("(") => {
                let e = self.expr()?;
                self.expect_punct(")")?;
                Ok(e)
            }
            // `[]` is the empty list; `[a, b]` is not a Contract expression
            // (LLP 1017.003 D4): a list with items comes from a source, a
            // shape field, or `map`/`filter`.
            TokenKind::Punct("[") => {
                if !self.eat_punct("]") {
                    return self.err(
                        "syntax-expected",
                        format!(
                            "expected `]`, found {}; `[]` is the empty list, and Contract has no list literal with items yet (LLP 1088 §9's follow-up): a list comes from the data module, a shape field, or `map`/`filter`",
                            describe(self.peek_kind())
                        ),
                    );
                }
                Ok(Expr::EmptyList(span))
            }
            // `none(value=1)`: a reserved word names no shape (LLP 1088 D5).
            TokenKind::Ident(w)
                if matches!(w.as_str(), "true" | "false" | "none") && self.at_punct("(") =>
            {
                Err(SyntaxError {
                    id: "syntax-keyword-as-value",
                    message: format!(
                        "{}, so it names no shape or function",
                        super::names::reserved_message(&w).replace("; choose another name", "")
                    ),
                    span,
                })
            }
            TokenKind::Ident(w) => match w.as_str() {
                "true" => Ok(Expr::Bool(true, span)),
                "false" => Ok(Expr::Bool(false, span)),
                "none" => Ok(Expr::None(span)),
                "some" => {
                    self.expect_punct("(")?;
                    let e = self.expr()?;
                    // `some(xs, x => …)`: the web's `Array.prototype.some`.
                    if self.at_punct(",") {
                        return self.err(
                            "syntax-refused-idiom",
                            format!(
                                "`some(x)` makes an option; {}",
                                crate::idioms::refusal("some").expect("refused")
                            ),
                        );
                    }
                    self.expect_punct(")")?;
                    self.built(self.last, span)?;
                    Ok(Expr::Some(Box::new(e), span))
                }
                "match" => {
                    let subject = self.expr()?;
                    let below = self.last;
                    self.expect_punct("{")?;
                    self.expect_word("case")?;
                    self.expect_word("some")?;
                    self.expect_punct("(")?;
                    let var = self.named_ident(span)?;
                    self.expect_punct(")")?;
                    self.expect_punct("=>")?;
                    let some = self.expr()?;
                    let below = below.max(self.last);
                    self.expect_punct(",")?;
                    self.expect_word("case")?;
                    self.expect_word("none")?;
                    self.expect_punct("=>")?;
                    let none = self.expr()?;
                    self.built(below.max(self.last), span)?;
                    self.eat_punct(",");
                    self.expect_punct("}")?;
                    Ok(Expr::Match {
                        subject: Box::new(subject),
                        var,
                        some: Box::new(some),
                        none: Box::new(none),
                        span,
                    })
                }
                // A contextual keyword (`state`, `key`, `refresh`) reads as the
                // name it is here; a reserved one is never a value (LLP 1088 D5).
                _ if super::names::is_reserved(&w) => Err(SyntaxError {
                    id: "syntax-keyword-as-value",
                    message: format!(
                        "`{w}` is reserved in Contract (it shapes an expression), so it is not a value"
                    ),
                    span,
                }),
                _ => {
                    if self.eat_punct("(") {
                        let args = self.call_args()?;
                        self.built(self.last, span)?;
                        Ok(Expr::Call(w, args, span))
                    } else {
                        Ok(Expr::Ident(w, span))
                    }
                }
            },
            other => {
                // `press={() => add()}`: a JSX handler.
                let hint = if matches!(other, TokenKind::Punct("{")) {
                    "; Contract has no `{…}`: a handler names an action (`press=add`, `press=add(item)`) and a value is written directly (`width=10`)"
                } else {
                    ""
                };
                Err(SyntaxError {
                    id: "syntax-expected-expression",
                    message: format!("expected an expression, found {}{hint}", describe(&other)),
                    span,
                })
            }
        }
    }

    /// Whether an arrow function starts here: `x =>`, `() =>`, `(x) =>` or
    /// `(x, i) =>` (LLP 1017.003 D1).
    pub(super) fn arrow_ahead(&self) -> bool {
        let at = |k: usize| &self.tokens[(self.pos + k).min(self.tokens.len() - 1)].kind;
        let name = |k: usize| matches!(at(k), TokenKind::Ident(_));
        let punct = |k: usize, p: &str| matches!(at(k), TokenKind::Punct(q) if *q == p);
        if name(0) {
            return punct(1, "=>");
        }
        if !punct(0, "(") {
            return false;
        }
        let mut k = 1;
        if name(k) {
            k += 1;
            while punct(k, ",") && name(k + 1) {
                k += 2;
            }
        }
        punct(k, ")") && punct(k + 1, "=>")
    }

    /// An arrow function: its parameters, `=>`, and one expression.
    pub(super) fn arrow(&mut self) -> R<Expr> {
        let span = self.peek().span;
        let mut params = Vec::new();
        if self.eat_punct("(") {
            while !self.at_punct(")") {
                params.push(self.ident()?.0);
                if !self.eat_punct(",") {
                    break;
                }
            }
            self.expect_punct(")")?;
        } else {
            params.push(self.ident()?.0);
        }
        self.expect_punct("=>")?;
        let body = self.expr()?;
        // `x => text x.name`: a view node where a value goes.
        if matches!(body, Expr::Ident(..)) && !self.at_punct(",") && !self.at_punct(")") {
            return Err(SyntaxError {
                id: "syntax-callback-view",
                message: "a callback returns one value, not a view node: repeat children with `each x in xs key=x.id` under their parent".into(),
                span: body.span(),
            });
        }
        self.built(self.last, span)?;
        Ok(Expr::Arrow {
            params,
            body: Box::new(body),
            span,
        })
    }

    pub(super) fn template(&mut self, raw: &str, span: Span) -> R<Expr> {
        let (mut parts, mut deepest) = (Vec::new(), 0);
        let mut text = String::new();
        let mut pos = 0;
        // Text decodes the escapes a `"…"` string does; `\${` is literal text.
        while let Some(c) = raw[pos..].chars().next() {
            if c == '\\' {
                let next = raw[pos + 1..].chars().next();
                let Some(decoded) = next.and_then(escaped) else {
                    return Err(SyntaxError {
                        id: "syntax-bad-escape",
                        message: "unknown escape".into(),
                        span: Span {
                            source_id: span.source_id,
                            ..Span::point(span.line, span.col + 1 + pos as u32)
                        },
                    });
                };
                text.push(decoded);
                pos += 1 + next.map_or(0, char::len_utf8);
                continue;
            }
            if !raw[pos..].starts_with("${") {
                text.push(c);
                pos += c.len_utf8();
                continue;
            }
            let after = &raw[pos + 2..];
            let end = template_expr_end(after).ok_or(SyntaxError {
                id: "syntax-unterminated-template-expr",
                message: "`${` never closes".into(),
                span,
            })?;
            if !text.is_empty() {
                parts.push(TemplatePart::Text(std::mem::take(&mut text)));
            }
            let inner = &after[..end];
            // `raw` starts after the backtick. Keep byte offsets through Unicode
            // prefixes, repeated interpolations, and recursively nested templates.
            let col = span.col + 1 + (raw.len() - after.len()) as u32;
            let mut tokens = Lexer::tokenize_at(
                inner,
                Span {
                    source_id: span.source_id,
                    ..Span::point(span.line, col)
                },
            )?;
            // A template fragment ends at its closing brace, not the next file
            // line. In particular, `${}` must report that brace's position.
            tokens.last_mut().unwrap().span = Span {
                source_id: span.source_id,
                ..Span::point(span.line, col + inner.len() as u32)
            };
            let mut sub = Parser {
                tokens,
                pos: 0,
                names: NameSpans::default(),
                depth: self.depth,
                view_depth: self.view_depth,
                last: 0,
            };
            let e = sub.expr()?;
            deepest = deepest.max(sub.last);
            if !matches!(sub.peek_kind(), TokenKind::Newline | TokenKind::Eof) {
                return sub.err("syntax-template-expr", "unexpected token in `${…}`");
            }
            self.names.names.extend(sub.names.names);
            self.names.sources.extend(sub.names.sources);
            parts.push(TemplatePart::Expr(e));
            pos += 2 + end + 1;
        }
        if !text.is_empty() {
            parts.push(TemplatePart::Text(text));
        }
        self.built(deepest, span)?;
        Ok(Expr::Template(parts, span))
    }
}
