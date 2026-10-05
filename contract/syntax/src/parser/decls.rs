//! The declarations whose clauses order work: `mutation` (its reply, what it
//! refreshes, its `then`, and whether its sends wait their turn) and `task`
//! (its schedule).

use super::*;

impl Parser {
    /// `mutation NAME as shape T [queue] [refreshes a, b] [then action]`.
    /// The clauses after the shape may continue on deeper-indented lines, in
    /// the same order (the codeedit diary: `then` on the next line was
    /// `syntax-expected-section`).
    pub(super) fn mutation(&mut self) -> R<MutationDecl> {
        let span = self.expect_word("mutation")?;
        let name = self.named_ident(span)?;
        self.expect_word("as")?;
        self.expect_word("shape")?;
        let shape = self.type_expr()?;
        let mut continued = false;
        // @ref LLP 1092 D1 — a keyword only here: every send of the
        // mutation waits for the one before it.
        let queue = self.clause_ahead("queue", &mut continued);
        if queue {
            self.next();
        }
        let mut refreshes = Vec::new();
        if self.clause_ahead("refreshes", &mut continued) {
            self.next();
            loop {
                refreshes.push(self.ident()?);
                if !self.eat_punct(",") {
                    break;
                }
            }
        }
        let then = if self.clause_ahead("then", &mut continued) {
            let then_span = self.next().span;
            Some(self.named_ident(then_span).map(|name| (name, then_span))?)
        } else {
            None
        };
        self.newline()?;
        if continued && !matches!(self.peek_kind(), TokenKind::Dedent | TokenKind::Eof) {
            return self.err(
                "syntax-expected",
                format!(
                    "a mutation's continued lines hold `queue`, `refreshes …` and `then …`, in that order; found {}",
                    describe(self.peek_kind())
                ),
            );
        }
        if continued && matches!(self.peek_kind(), TokenKind::Dedent) {
            self.next();
        }
        Ok(MutationDecl {
            name,
            shape,
            queue,
            refreshes,
            then,
            span,
        })
    }

    /// Whether the next clause is `word`, on this line or on a continued
    /// one: the first continued line opens an indented block (`continued`
    /// records it), each later one is a line of that block.
    fn clause_ahead(&mut self, word: &str, continued: &mut bool) -> bool {
        if self.at_ident(word) {
            return true;
        }
        let ahead = |n: usize| &self.tokens[(self.pos + n).min(self.tokens.len() - 1)].kind;
        let is_word = |k: &TokenKind| matches!(k, TokenKind::Ident(w) if w == word);
        let opens = !*continued
            && matches!(ahead(0), TokenKind::Newline)
            && matches!(ahead(1), TokenKind::Indent)
            && is_word(ahead(2));
        let goes_on = *continued && matches!(ahead(0), TokenKind::Newline) && is_word(ahead(1));
        if opens {
            self.next();
            self.next();
            *continued = true;
        } else if goes_on {
            self.next();
        }
        opens || goes_on
    }

    pub(super) fn task(&mut self) -> R<Task> {
        let span = self.expect_word("task")?;
        let name = self.named_ident(span)?;
        // @ref LLP 1092 D7 — `mount`, or a gate: `when cond [key=expr]`, or
        // `key=expr` alone (`when true`), spelled as `each` spells its key.
        let (mut gate, mut key) = (None, None);
        if self.at_ident("when") {
            self.next();
            gate = Some(self.expr()?);
            if self.at_ident("key") {
                self.next();
                self.expect_punct("=")?;
                key = Some(self.expr()?);
            }
        } else if self.at_ident("key") {
            self.next();
            self.expect_punct("=")?;
            key = Some(self.expr()?);
        } else if !self.at_ident("mount") {
            return self.err(
                "syntax-task-start",
                format!("`task {name}` starts at `mount`, or while a condition holds: `when cond`, `when cond key=expr` or `key=expr`"),
            );
        } else {
            self.next();
        }
        self.newline()?;
        let mut timer = None;
        self.block(|p| {
            let (f, fspan) = p.ident()?;
            let mut kind = match f.as_str() {
                "every" => TaskKind::Every,
                "after" => TaskKind::After,
                _ => {
                    return p.err(
                        "contract-task-body",
                        "a task body is `every(ms, action)`, `every(frame, action)` or `after(ms, action)`",
                    )
                }
            };
            p.expect_punct("(")?;
            // `every(frame, a)`: `frame` there is a word, not an expression (LLP 1073 D1).
            let frame = p.at_ident("frame") && matches!(p.peek2(), TokenKind::Punct(","));
            let ms = if frame {
                if kind == TaskKind::After {
                    return p.err(
                        "contract-task-body",
                        "`after` takes milliseconds; `every(frame, action)` fires each frame",
                    );
                }
                kind = TaskKind::Frame;
                let at = p.next().span;
                Expr::Number(0.0, at)
            } else {
                p.expr()?
            };
            p.expect_punct(",")?;
            let action = p.named_ident(fspan)?;
            p.expect_punct(")")?;
            p.newline()?;
            if timer.is_some() {
                return duplicate("task entry", &f, fspan, span);
            }
            timer = Some((kind, (ms, action, fspan)));
            Ok(())
        })?;
        let (kind, timer) = timer.ok_or(SyntaxError {
            id: "contract-task-body",
            message:
                "a task needs `every(ms, action)`, `every(frame, action)` or `after(ms, action)`"
                    .into(),
            span,
        })?;
        Ok(Task {
            name,
            kind,
            gate,
            key,
            timer,
            span,
        })
    }
}
