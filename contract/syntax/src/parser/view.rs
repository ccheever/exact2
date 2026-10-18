//! The view arms live in separate frames from the recursive dispatcher.

use super::*;

impl Parser {
    /// `if cond`, with `else if` and `else` as an action's (@ref LLP 1116.015 D1).
    #[inline(never)]
    pub(super) fn node_if(&mut self, span: Span) -> R<Node> {
        self.next();
        let cond = self.expr()?;
        self.newline()?;
        let then = self.required_block(span, "if", |p| p.node())?;
        let mut otherwise = Vec::new();
        if self.at_ident("else") {
            self.next();
            // `else if` is an `else` whose one node is the next `if`.
            if self.at_ident("if") {
                otherwise = vec![self.node()?];
            } else if self.at_ident("when") {
                let span = self.peek().span;
                self.retired_view(span, "if");
                self.retired.last_mut().unwrap().id = "syntax-else-keyword";
                otherwise = vec![self.node_if(span)?];
            } else {
                self.after_else("if")?;
                otherwise = self.required_block(span, "else", |p| p.node())?;
            }
        }
        Ok(Node::When {
            tag: 0,
            cond,
            then,
            otherwise,
            span,
        })
    }
    /// `for x in xs key=…`, or `for x, i in xs key=…` (@ref LLP 1116.015 D1).
    #[inline(never)]
    pub(super) fn node_for(&mut self, span: Span) -> R<Node> {
        self.next();
        let var = self.named_ident(span)?;
        let index = if self.at_punct(",") {
            self.next();
            Some(self.named_ident(span)?)
        } else {
            None
        };
        self.expect_word("in")?;
        let list = self.expr()?;
        // @ref LLP 1116.014 D1 — the edit, with the variable the author named. The
        // parser knows no item type, so the field is described, not named.
        if !self.at_ident("key") {
            return self.err(
                "syntax-expected",
                format!(
                    "add `key={var}.<its id field>`, a field no two items share: a `for` keeps \
                     each row's state by its key"
                ),
            );
        }
        self.expect_word("key")?;
        self.expect_punct("=")?;
        let key = self.expr()?;
        self.newline()?;
        let body = self.block(|p| p.node())?;
        Ok(Node::Each {
            tag: 0,
            var,
            index,
            list,
            key,
            body,
            span,
        })
    }
    #[inline(never)]
    pub(super) fn node_match(&mut self, span: Span) -> R<Node> {
        self.next();
        let subject = self.expr()?;
        self.newline()?;
        let mut some = None;
        let mut none = None;
        if matches!(self.peek_kind(), TokenKind::Indent) {
            self.next();
            loop {
                match self.peek_kind() {
                    TokenKind::Dedent => {
                        self.next();
                        break;
                    }
                    TokenKind::Eof => break,
                    TokenKind::Newline => {
                        self.next();
                    }
                    _ => {
                        let (var, case_span) = self.match_case_header(span)?;
                        if let Some(var) = var {
                            if some.is_some() {
                                return duplicate("match arm", "some", case_span, span);
                            }
                            some = Some((var, self.block(|p| p.node())?));
                        } else {
                            if none.is_some() {
                                return duplicate("match arm", "none", case_span, span);
                            }
                            none = Some(self.block(|p| p.node())?);
                        }
                    }
                }
            }
        }
        let some = some.ok_or(SyntaxError {
            id: "contract-match-arms",
            message: "`match` needs `case some(x)`".into(),
            span,
        })?;
        let none = none.ok_or(SyntaxError {
            id: "contract-match-arms",
            message: "`match` needs `case none`".into(),
            span,
        })?;
        Ok(Node::Match {
            tag: 0,
            subject,
            some,
            none,
            span,
        })
    }

    #[inline(never)]
    fn match_case_header(&mut self, span: Span) -> R<(Option<String>, Span)> {
        let case_span = self.expect_word("case")?;
        if self.at_ident("some") {
            self.next();
            self.expect_punct("(")?;
            let var = self.named_ident(span)?;
            self.expect_punct(")")?;
            self.newline()?;
            Ok((Some(var), case_span))
        } else {
            self.expect_word("none")?;
            self.newline()?;
            Ok((None, case_span))
        }
    }
    #[inline(never)]
    pub(super) fn node_use(&mut self, word: String, span: Span) -> R<Node> {
        self.next();
        self.expect_punct("(")?;
        let args = self.named_args()?;
        self.newline()?;
        let children = self.block(|p| p.node())?;
        Ok(Node::Use {
            name: word,
            args,
            children,
            span,
        })
    }
    #[inline(never)]
    pub(super) fn node_element(&mut self, word: String, span: Span) -> R<Node> {
        self.next();
        let mut positional = Vec::new();
        let mut attrs = Vec::new();
        self.attr_line(&mut positional, &mut attrs, false)?;
        self.newline()?;
        let mut children = Vec::new();
        if matches!(self.peek_kind(), TokenKind::Indent) {
            self.next();
            // The attribute list may continue on deeper lines that
            // begin with `name=`; a child never does (LLP 1035.005 D1).
            // So may a part's block, `::placeholder` (parts.rs).
            // And a state part's, `:disabled` (states.rs).
            while self.at_continuation() || self.at_part() || self.at_state() {
                if self.at_part() {
                    self.part(&format!("`{word}`"), false, &mut attrs)?;
                    continue;
                }
                if self.at_state() {
                    self.state_part(&format!("`{word}`"), false, &mut attrs)?;
                    continue;
                }
                self.attr_line(&mut positional, &mut attrs, true)?;
                self.newline()?;
            }
            children = self.block_rest(|p| p.node())?;
        }
        // LLP 1017.001's primary button argument is its visible text
        // child, not a prop on the pressable itself. Keeping it as a
        // real text node gives every host the same visible and
        // accessible subtree.
        if word == "button" && positional.len() == 1 {
            if self.view_depth >= 256 {
                return self.err("syntax-view-depth", "views nest more than 256 sites deep");
            }
            children.insert(
                0,
                Node::Element {
                    tag: "text".into(),
                    positional: vec![positional.remove(0)],
                    attrs: Vec::new(),
                    children: Vec::new(),
                    span,
                    instance: 0,
                },
            );
        }
        let tag = heading(word, span, &mut attrs)?;
        Ok(Node::Element {
            tag,
            positional,
            attrs,
            children,
            span,
            instance: 0,
        })
    }
}

/// LLP 1116.017 D1: HTML's heading elements are headings, the level in the
/// name; every host then sees the one form, `text role="heading"`. Apart from
/// `node_element`, whose frame every nested view node costs.
#[inline(never)]
fn heading(word: String, span: Span, attrs: &mut Vec<Attr>) -> R<String> {
    let level = match word.as_str() {
        "h1" => 1.0,
        "h2" => 2.0,
        "h3" => 3.0,
        "h4" => 4.0,
        "h5" => 5.0,
        "h6" => 6.0,
        _ => return Ok(word),
    };
    if let Some(a) = attrs
        .iter()
        .find(|a| matches!(a.name.as_str(), "role" | "aria-level"))
    {
        return Err(SyntaxError {
            id: "syntax-heading-level",
            message: format!(
                "an `{word}` is a heading of level {level}: drop its `{}`, or write `text … role=\"heading\" aria-level=…` for a level the app computes",
                a.name
            ),
            span: a.span,
        });
    }
    for (name, value) in [
        ("role", Expr::Str("heading".into(), span)),
        ("aria-level", Expr::Number(level, span)),
    ] {
        attrs.push(Attr {
            name: name.into(),
            value,
            span,
            written: true,
        });
    }
    Ok("text".into())
}
