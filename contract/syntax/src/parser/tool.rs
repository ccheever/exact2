//! The `tool` line (LLP 1111 D2; `crate::tool`).

use super::*;
use crate::tool::{Consequence, Tool};

impl Parser {
    /// `tool NAME CONSEQUENCE [MODIFIERS] "DESCRIPTION" = ACTION [CLAUSES]`,
    /// its clauses given to `c` as derives (`Tool::derive`).
    pub(super) fn tool(&mut self, c: &mut Component) -> R<()> {
        let span = self.expect_word("tool")?;
        let name = self.named_ident(span)?;
        let consequence = match self.peek_kind() {
            TokenKind::Ident(w) if w == "reads" => Consequence::Reads,
            TokenKind::Ident(w) if w == "writes" => Consequence::Writes,
            TokenKind::Ident(w) if w == "destroys" => Consequence::Destroys,
            other => {
                return self.err(
                    "syntax-tool-consequence",
                    format!(
                        "a tool says what it does, `reads`, `writes` or `destroys`, after its name; found {}",
                        describe(other)
                    ),
                )
            }
        };
        self.next();
        let (mut additive, mut idempotent, mut closed) = (false, false, false);
        loop {
            let (flag, word) = match self.peek_kind() {
                TokenKind::Ident(w) if w == "additive" => (&mut additive, "additive"),
                TokenKind::Ident(w) if w == "idempotent" => (&mut idempotent, "idempotent"),
                TokenKind::Ident(w) if w == "closed" => (&mut closed, "closed"),
                _ => break,
            };
            if *flag {
                return self.err("syntax-tool-modifier", format!("`{word}` is written twice"));
            }
            *flag = true;
            self.next();
        }
        if additive && consequence != Consequence::Writes {
            return self.err(
                "syntax-tool-modifier",
                "`additive` says how a tool writes: it follows `writes`",
            );
        }
        let TokenKind::Str(description) = self.peek_kind().clone() else {
            return self.err(
                "syntax-tool-description",
                format!(
                    "a tool's description is a string a caller reads, like `tool {name} {} \"What it does.\" = action`",
                    consequence.word()
                ),
            );
        };
        self.next();
        self.expect_punct("=")?;
        let (action, action_span) = self.ident()?;
        let mut continued = false;
        let mut clause = |p: &mut Self, word: &str, when: bool| -> R<Option<Expr>> {
            if !p.clause_ahead(word, &mut continued) {
                return Ok(None);
            }
            p.next();
            if when {
                p.expect_word("when")?;
            }
            p.expr().map(Some)
        };
        let returns = clause(self, "returns", false)?;
        let fails = clause(self, "fails", true)?;
        let available = clause(self, "available", true)?;
        if let Some(word) = ["returns", "fails", "available"]
            .into_iter()
            .find(|w| self.at_ident(w))
        {
            return self.err(
                "syntax-expected-newline",
                format!(
                    "`{word}` is out of place: a tool's clauses go `returns …`, then `fails when …`, then `available when …`, each at most once"
                ),
            );
        }
        self.newline()?;
        if continued {
            if !matches!(self.peek_kind(), TokenKind::Dedent | TokenKind::Eof) {
                return self.err(
                    "syntax-expected",
                    format!(
                        "a tool's continued lines hold `returns …`, `fails when …` and `available when …`, in that order; found {}",
                        describe(self.peek_kind())
                    ),
                );
            }
            if matches!(self.peek_kind(), TokenKind::Dedent) {
                self.next();
            }
        }
        for (word, expr) in [
            ("returns", &returns),
            ("fails", &fails),
            ("available", &available),
        ] {
            if let Some(expr) = expr {
                c.derives.push(Binding {
                    name: Tool::derive(&name, word),
                    expr: expr.clone(),
                    span,
                });
            }
        }
        c.tools.push(Tool {
            name,
            consequence,
            additive,
            idempotent,
            closed,
            description,
            action: (action, action_span),
            returns,
            fails,
            available,
            span,
        });
        Ok(())
    }
}
