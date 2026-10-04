//! The declarations whose clauses order work: `mutation` (its reply, what it
//! refreshes, its `then`, and whether its sends wait their turn) and `task`
//! (its schedule).

use super::*;

impl Parser {
    /// `mutation NAME as shape T [queue] [refreshes a, b] [then action]`.
    pub(super) fn mutation(&mut self) -> R<MutationDecl> {
        let span = self.expect_word("mutation")?;
        let name = self.named_ident(span)?;
        self.expect_word("as")?;
        self.expect_word("shape")?;
        let shape = self.type_expr()?;
        // @ref LLP 1092 D1 — a keyword only here: every send of the
        // mutation waits for the one before it.
        let queue = self.at_ident("queue");
        if queue {
            self.next();
        }
        let mut refreshes = Vec::new();
        if self.at_ident("refreshes") {
            self.next();
            loop {
                refreshes.push(self.ident()?);
                if !self.eat_punct(",") {
                    break;
                }
            }
        }
        let then = if self.at_ident("then") {
            let then_span = self.next().span;
            Some(self.named_ident(then_span).map(|name| (name, then_span))?)
        } else {
            None
        };
        self.newline()?;
        Ok(MutationDecl {
            name,
            shape,
            queue,
            refreshes,
            then,
            span,
        })
    }

    pub(super) fn task(&mut self) -> R<Task> {
        let span = self.expect_word("task")?;
        let name = self.named_ident(span)?;
        self.expect_word("mount")?;
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
            timer,
            span,
        })
    }
}
