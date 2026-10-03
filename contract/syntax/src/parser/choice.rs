//! Closed choices of strings (LLP 1035.005.000 D4a): the type
//! `"a" | "b"`, and `match` over one with `case "a"` arms. A `match` over a
//! choice is read as nested tests, one [`Expr::Case`] per arm but the last,
//! which is their final `else`: the views' `when`, the actions' `if`, an
//! expression's `?:`. Lowering, the plan and both executors see only those;
//! the checker holds the tests to the subject's choice.

use super::*;

/// One arm: its literals, its `case`, its body.
type Arm<T> = (Vec<String>, Span, T);

/// Whether `s` is spelled as a choice literal: letters, digits, `-`, `_`,
/// `.`, `/` and `:`. A value's type code on the web carries the literals,
/// and a generated Rust enum names its variants after them.
pub(crate) fn choice_literal(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_./:".contains(&b))
}

impl Parser {
    /// `"a" | "b" | …` as a type, at its first literal.
    pub(super) fn choice_type(&mut self) -> R<TypeExpr> {
        let span = self.peek().span;
        let mut literals = Vec::new();
        self.literals(&mut literals, "type")?;
        Ok(TypeExpr::Choice(literals, span))
    }

    /// `"a" { | "b" }`, each literal spelled as one and new to `seen`.
    fn literals(&mut self, seen: &mut Vec<String>, what: &str) -> R<Vec<String>> {
        let mut out = Vec::new();
        loop {
            let t = self.peek().clone();
            let TokenKind::Str(s) = t.kind else {
                return self.err(
                    "syntax-expected",
                    format!(
                        "expected a string literal, found {}: a choice is written `\"a\" | \"b\"`",
                        describe(self.peek_kind())
                    ),
                );
            };
            if !choice_literal(&s) {
                return self.err(
                    "syntax-choice-literal",
                    format!("`\"{s}\"` cannot be a choice's literal: spell it with letters, digits, `-`, `_`, `.`, `/` and `:`, as `\"table-row\"`"),
                );
            }
            if seen.contains(&s) {
                let message = match what {
                    "type" => format!("`\"{s}\"` is named twice in this choice: write it once"),
                    _ => format!("`\"{s}\"` already has an arm in this `match`: name each literal in one arm"),
                };
                return self.err("syntax-choice-duplicate", message);
            }
            self.next();
            seen.push(s.clone());
            out.push(s);
            if !self.eat_punct("|") {
                return Ok(out);
            }
        }
    }

    /// Whether a `match` block's first arm is `case "…"`.
    pub(super) fn at_choice_arms(&self) -> bool {
        let at = |i: usize| &self.tokens[(self.pos + i).min(self.tokens.len() - 1)].kind;
        matches!(at(0), TokenKind::Indent)
            && matches!(at(1), TokenKind::Ident(w) if w == "case")
            && matches!(at(2), TokenKind::Str(_))
    }

    /// The indented arms of a `match` over a choice, each `case "a" | "b"`
    /// then what `body` reads.
    fn choice_arms<T>(&mut self, mut body: impl FnMut(&mut Self) -> R<T>) -> R<Vec<Arm<T>>> {
        let mut seen = Vec::new();
        self.block(|p| {
            if p.at_ident("else") {
                return p.err(
                    "syntax-match-else",
                    "a `match` over a choice has no `else`: give every literal a `case`, several to one arm as `case \"a\" | \"b\"`",
                );
            }
            let case = p.expect_word("case")?;
            if p.at_ident("some") || p.at_ident("none") {
                return p.err(
                    "contract-match-arms",
                    "a `match` takes `case some(x)` and `case none` over an option, or `case \"…\"` over a choice, not both",
                );
            }
            let literals = p.literals(&mut seen, "`match`")?;
            Ok((literals, case, body(p)?))
        })
    }

    /// The tests a `match`'s arms make, in order; the first carries every
    /// literal. The last arm's is never evaluated except when it is the
    /// only one.
    fn tests<T>(subject: &Expr, arms: &[Arm<T>], span: Span) -> Vec<Expr> {
        let all: Vec<String> = arms.iter().flat_map(|a| a.0.iter().cloned()).collect();
        arms.iter()
            .enumerate()
            .map(|(i, (literals, case, _))| Expr::Case {
                subject: Box::new(subject.clone()),
                literals: literals.clone(),
                all: (i == 0).then(|| all.clone()),
                span: if i == 0 { span } else { *case },
            })
            .collect()
    }

    /// A view's `match` over a choice, after its subject's line: nested
    /// `when`s.
    pub(super) fn choice_view(&mut self, subject: Expr, span: Span) -> R<Node> {
        let arms = self.choice_arms(|p| {
            p.newline()?;
            p.block(|q| q.node())
        })?;
        let tests = Self::tests(&subject, &arms, span);
        let last = arms.len() - 1;
        let mut rest: Vec<Node> = Vec::new();
        for (i, ((_, case, body), cond)) in arms.into_iter().zip(tests).enumerate().rev() {
            rest = if i == last && last > 0 {
                body
            } else {
                vec![Node::When {
                    cond,
                    then: body,
                    otherwise: rest,
                    span: if i == 0 { span } else { case },
                }]
            };
        }
        Ok(rest.pop().expect("one arm"))
    }

    /// An action's `match` over a choice, after its subject's line: nested
    /// `if`s.
    pub(super) fn choice_stmt(&mut self, subject: Expr, span: Span) -> R<Stmt> {
        let arms = self.choice_arms(|p| {
            p.newline()?;
            p.block(|q| q.stmt())
        })?;
        let tests = Self::tests(&subject, &arms, span);
        let last = arms.len() - 1;
        let mut rest: Vec<Stmt> = Vec::new();
        for (i, ((_, case, body), cond)) in arms.into_iter().zip(tests).enumerate().rev() {
            rest = if i == last && last > 0 {
                body
            } else {
                vec![Stmt::If {
                    cond,
                    then: body,
                    otherwise: rest,
                    span: if i == 0 { span } else { case },
                }]
            };
        }
        Ok(rest.pop().expect("one arm"))
    }

    /// `match subject { case "a" => x, case "b" | "c" => y }`, after its
    /// `{`: nested `?:`s. One arm is its body, both ways.
    pub(super) fn choice_expr(&mut self, subject: Expr, span: Span, below: usize) -> R<Expr> {
        let mut seen = Vec::new();
        let mut arms = Vec::new();
        let mut below = below;
        loop {
            if self.at_ident("else") {
                return self.err(
                    "syntax-match-else",
                    "a `match` over a choice has no `else`: give every literal a `case`, several to one arm as `case \"a\" | \"b\"`",
                );
            }
            let case = self.expect_word("case")?;
            let literals = self.literals(&mut seen, "`match`")?;
            self.expect_punct("=>")?;
            let value = self.expr()?;
            below = below.max(self.last);
            arms.push((literals, case, value));
            if !self.eat_punct(",") || self.at_punct("}") {
                break;
            }
        }
        self.expect_punct("}")?;
        // Each arm but the last nests the rest one level deeper.
        self.built(below + arms.len(), span)?;
        let tests = Self::tests(&subject, &arms, span);
        let mut values = arms.into_iter().map(|a| a.2).rev();
        let mut out = values.next().expect("one arm");
        let mut tests = tests.into_iter().rev();
        if values.len() == 0 {
            let test = tests.next().expect("one test");
            out = Expr::Ternary(Box::new(test), Box::new(out.clone()), Box::new(out), span);
        } else {
            tests.next();
        }
        for (value, test) in values.zip(tests) {
            out = Expr::Ternary(Box::new(test), Box::new(value), Box::new(out), span);
        }
        Ok(out)
    }
}
