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
            let (field, span) = self.ident()?;
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
            TokenKind::Ident(w) => match w.as_str() {
                "true" => Ok(Expr::Bool(true, span)),
                "false" => Ok(Expr::Bool(false, span)),
                "none" => Ok(Expr::None(span)),
                "some" => {
                    self.expect_punct("(")?;
                    let e = self.expr()?;
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
                _ if is_keyword(&w) => Err(SyntaxError {
                    id: "syntax-keyword-as-value",
                    message: format!("`{w}` is a keyword"),
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

    pub(super) fn template(&mut self, raw: &str, span: Span) -> R<Expr> {
        let (mut parts, mut deepest) = (Vec::new(), 0);
        let mut text = String::new();
        let mut rest = raw;
        while let Some(i) = rest.find("${") {
            text.push_str(&rest[..i]);
            let after = &rest[i + 2..];
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
            rest = &after[end + 1..];
        }
        text.push_str(rest);
        if !text.is_empty() {
            parts.push(TemplatePart::Text(text));
        }
        self.built(deepest, span)?;
        Ok(Expr::Template(parts, span))
    }
}
