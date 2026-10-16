//! `sequence NAME when cond [key=expr]` and its one `play(...)` (LLP 1125
//! D2). The gate is a task's (LLP 1092 D7), parsed by [`Parser::gate`].

use super::*;
use crate::sequence::Sequence;

const BODY: &str = "a sequence body is one `play(tracks, bpm=…)`: the tracks, then `bpm=`, `from=`, `countIn=` and `ended=` by name";

impl Parser {
    /// `mount`, or a gate: `when cond [key=expr]`, or `key=expr` alone
    /// (`when true`), spelled as `for` spells its key. `None` when the
    /// next word is none of them, for the caller to refuse.
    pub(super) fn gate(&mut self) -> R<Option<(Option<Expr>, Option<Expr>)>> {
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
        } else if self.at_ident("mount") {
            self.next();
        } else {
            return Ok(None);
        }
        Ok(Some((gate, key)))
    }

    pub(super) fn sequence(&mut self) -> R<Sequence> {
        let span = self.expect_word("sequence")?;
        let name = self.named_ident(span)?;
        let Some((gate, key)) = self.gate()? else {
            return self.err(
                "syntax-task-start",
                format!("`sequence {name}` plays from `mount`, or while a condition holds: `when cond`, `when cond key=expr` or `key=expr`"),
            );
        };
        self.newline()?;
        let mut play = None;
        self.block(|p| {
            let at = p.peek().span;
            if play.is_some() {
                return p.err("contract-sequence-body", BODY);
            }
            let Expr::Call(callee, args, _) = p.expr()? else {
                return Err(SyntaxError {
                    id: "contract-sequence-body",
                    message: BODY.into(),
                    span: at,
                });
            };
            if callee != "play" {
                return Err(SyntaxError {
                    id: "contract-sequence-body",
                    message: format!("`{callee}(…)` is not `play`: {BODY}"),
                    span: at,
                });
            }
            p.newline()?;
            play = Some((args, at));
            Ok(())
        })?;
        let Some((args, at)) = play else {
            return Err(SyntaxError {
                id: "contract-sequence-body",
                message: BODY.into(),
                span,
            });
        };
        let mut args = args.into_iter();
        let tracks = match args.next() {
            Some(e) if !matches!(e, Expr::NamedArg(..)) => e,
            _ => {
                return Err(SyntaxError {
                    id: "contract-sequence-body",
                    message: format!("`play` takes the tracks first: {BODY}"),
                    span: at,
                })
            }
        };
        let (mut options, mut ended) = (Vec::new(), None);
        for arg in args {
            let Expr::NamedArg(option, value, s) = arg else {
                return Err(SyntaxError {
                    id: "contract-sequence-body",
                    message: format!("after the tracks, each argument is named: {BODY}"),
                    span: arg.span(),
                });
            };
            if option == "ended" {
                let Expr::Ident(action, a) = *value else {
                    return Err(SyntaxError {
                        id: "contract-sequence-body",
                        message: "`ended=` names an action: `ended=finished`".into(),
                        span: value.span(),
                    });
                };
                if ended.replace((action, a)).is_some() {
                    return duplicate("play option", "ended", s, s);
                }
            } else {
                options.push((option, *value, s));
            }
        }
        Ok(Sequence {
            name,
            gate,
            key,
            tracks,
            options,
            ended,
            span,
        })
    }
}
