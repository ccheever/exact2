//! `keyframes Name`: CSS `@keyframes` in Contract's indented form
//! (LLP 1055 D5). Each line is one or more selectors — `from`, `to`, or a
//! percentage, separated by commas — then `attr=literal` values.

use super::*;

impl Parser {
    pub(super) fn keyframes_decl(&mut self) -> R<KeyframesDecl> {
        let span = self.expect_word("keyframes")?;
        let name = self.named_ident(span)?;
        self.newline()?;
        let frames = self.required_block(span, "keyframes", |p| {
            let line = p.peek().span;
            let mut selectors = Vec::new();
            loop {
                let token = p.next();
                let offset = match token.kind {
                    TokenKind::Ident(w) if w == "from" => 0.0,
                    TokenKind::Ident(w) if w == "to" => 100.0,
                    TokenKind::Number(n) if p.eat_punct("%") => n,
                    other => {
                        return Err(SyntaxError {
                            id: "syntax-keyframe-selector",
                            message: format!(
                                "a keyframe starts with `from`, `to` or a percentage (`50%`), found {}",
                                describe(&other)
                            ),
                            span: token.span,
                        })
                    }
                };
                if !(0.0..=100.0).contains(&offset) {
                    return Err(SyntaxError {
                        id: "syntax-keyframe-selector",
                        message: format!("a keyframe selector is between 0% and 100%, not {offset}%"),
                        span: token.span,
                    });
                }
                selectors.push(offset);
                if !p.eat_punct(",") {
                    break;
                }
            }
            let mut attrs = Vec::new();
            while !matches!(p.peek_kind(), TokenKind::Newline | TokenKind::Eof | TokenKind::Dedent) {
                let (aname, aspan) = match (p.peek_kind().clone(), p.peek2().clone()) {
                    (TokenKind::Ident(n), TokenKind::Punct("=")) => (n, p.next().span),
                    (other, _) => {
                        return p.err(
                            "syntax-expected-attr",
                            format!("expected `property=literal` in a keyframe, found {}", describe(&other)),
                        )
                    }
                };
                p.next();
                let value = p.expr()?;
                if !matches!(value, Expr::Number(..) | Expr::Str(..)) {
                    return Err(SyntaxError {
                        id: "contract-keyframe-literal",
                        message: format!("`{aname}` in `keyframes {name}` must be a literal: keyframes are constant"),
                        span: aspan,
                    });
                }
                if attrs.iter().any(|a: &Attr| a.name == aname) {
                    return Err(SyntaxError {
                        id: "syntax-duplicate-attr",
                        message: format!("attribute `{aname}` appears twice in one keyframe"),
                        span: aspan,
                    });
                }
                attrs.push(Attr {
                    name: aname,
                    value,
                    span: aspan,
                });
            }
            p.newline()?;
            Ok(KeyframeDecl {
                selectors,
                attrs,
                span: line,
            })
        })?;
        Ok(KeyframesDecl { name, frames, span })
    }
}
