//! `sound "assets/kit/kick.wav"` and the test step `expect sound` (LLP 1096
//! D1, D10). `sound` is a keyword only at the start of a top-level line, as
//! `font` is, so `state sound = true` still declares a slot.

use super::*;

impl Parser {
    /// `sound "path"`: one WAV, which lowering reads and checks.
    pub(super) fn sound_decl(&mut self) -> R<SoundDecl> {
        let span = self.expect_word("sound")?;
        let source = self.str_lit("a WAV source path under `assets/`")?;
        self.newline()?;
        Ok(SoundDecl { source, span })
    }

    /// After `expect sound`: `has` or `missing`, the source, then any of
    /// `at N`, `gain N`, `ends N` and `by word`, each at most once.
    pub(super) fn expect_sound(&mut self, span: Span) -> R<Step> {
        const USAGE: &str = "`expect sound has \"assets/x.wav\"` or `missing`, then any of `at 0`, `gain 0.8`, `ends 125` and `by cancelled`";
        let present = if self.at_ident("has") {
            self.next();
            true
        } else if self.at_ident("missing") {
            self.next();
            false
        } else {
            return self.err("syntax-expected-step", USAGE);
        };
        let src = self.str_lit("a declared sound's path")?;
        let (mut at, mut gain, mut ends, mut by) = (None, None, None, None);
        while let TokenKind::Ident(word) = self.peek_kind().clone() {
            let clause = self.peek().span;
            self.next();
            let given = match word.as_str() {
                "at" | "gain" | "ends" => {
                    let token = self.next();
                    let TokenKind::Number(n) = token.kind else {
                        return Err(SyntaxError {
                            id: "syntax-expected-step",
                            message: format!(
                                "`{word}` takes a number, found {}",
                                describe(&token.kind)
                            ),
                            span: token.span,
                        });
                    };
                    let slot = match word.as_str() {
                        "at" => &mut at,
                        "gain" => &mut gain,
                        _ => &mut ends,
                    };
                    slot.replace(n).is_some()
                }
                "by" => {
                    let (how, how_span) = self.ident()?;
                    if !["end", "group", "cut", "stop", "cancelled"].contains(&how.as_str()) {
                        return Err(SyntaxError {
                            id: "syntax-expected-step",
                            message: format!(
                                "a voice ends by `end`, `group`, `cut`, `stop` or `cancelled`, not `{how}`"
                            ),
                            span: how_span,
                        });
                    }
                    by.replace(how).is_some()
                }
                other => {
                    return Err(SyntaxError {
                        id: "syntax-expected-step",
                        message: format!("`expect sound` has no clause `{other}`: {USAGE}"),
                        span: clause,
                    })
                }
            };
            if given {
                return Err(SyntaxError {
                    id: "syntax-expected-step",
                    message: format!("`{word}` is given twice"),
                    span: clause,
                });
            }
        }
        Ok(Step::ExpectSound {
            src,
            present,
            at,
            gain,
            ends,
            by,
            span,
        })
    }
}
