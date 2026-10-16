//! MIDI's test steps (LLP 1125 §4): `prefer midi.…`, `tap midi "in" 90 3c
//! 64` and `expect midi sent|missing "out" 90 3c 64 [at N]`. A byte is a word
//! of exactly two hex digits; the lexer splits `3c` into a number and a
//! name, so a word is rebuilt from the tokens that touch (rulings #12).

use super::*;
use crate::MidiStep;

impl Parser {
    /// The bytes after an input or output: two hex digits a word, one or more.
    fn midi_bytes(&mut self, usage: &str) -> R<Vec<u8>> {
        let mut out = Vec::new();
        loop {
            let first = self.peek().clone();
            let mut word = match &first.kind {
                TokenKind::Number(n) if n.fract() == 0.0 && *n >= 0.0 => {
                    let width = (first.span.end_col - first.span.col) as usize;
                    format!("{:0width$}", *n as u64)
                }
                TokenKind::Ident(w) => w.clone(),
                _ => break,
            };
            self.next();
            let mut end = first.span;
            loop {
                let t = self.peek().clone();
                let touches = t.span.line == end.line && t.span.col == end.end_col;
                let piece = match &t.kind {
                    TokenKind::Number(n) if touches && n.fract() == 0.0 && *n >= 0.0 => {
                        let width = (t.span.end_col - t.span.col) as usize;
                        format!("{:0width$}", *n as u64)
                    }
                    TokenKind::Ident(w) if touches => w.clone(),
                    _ => break,
                };
                word.push_str(&piece);
                end = t.span;
                self.next();
            }
            if word == "at" {
                // `at N` follows the bytes in `expect midi`.
                self.pos -= 1;
                break;
            }
            let byte = (word.len() == 2)
                .then(|| u8::from_str_radix(&word, 16).ok())
                .flatten();
            let Some(byte) = byte else {
                return Err(SyntaxError {
                    id: "syntax-expected-step",
                    message: format!("`{word}` is not a byte: each is two hex digits, {usage}"),
                    span: first.span,
                });
            };
            out.push(byte);
        }
        if out.is_empty() {
            return self.err(
                "syntax-expected-step",
                format!("expected a byte, two hex digits: {usage}"),
            );
        }
        Ok(out)
    }

    /// After `tap`: `midi "<input>" <hex…>`.
    pub(super) fn tap_midi(&mut self, span: Span) -> R<Step> {
        const USAGE: &str = "as in `tap midi \"keys\" 90 3c 64`";
        self.next();
        let input = self.str_lit("the input's id")?;
        let data = self.midi_bytes(USAGE)?;
        self.newline()?;
        Ok(Step::Midi {
            step: MidiStep::Tap { input, data },
            span,
        })
    }

    /// After `expect midi`: `sent|missing "<output>" <hex…> [at N]`.
    pub(super) fn expect_midi(&mut self, span: Span) -> R<Step> {
        const USAGE: &str = "as in `expect midi sent \"synth\" 90 3c 64 at 500`";
        let present = if self.at_ident("sent") {
            true
        } else if self.at_ident("missing") {
            false
        } else {
            return self.err(
                "syntax-expected-step",
                format!("`expect midi` takes `sent` or `missing`, {USAGE}"),
            );
        };
        self.next();
        let output = self.str_lit("the output's id")?;
        let data = self.midi_bytes(USAGE)?;
        let at = if self.at_ident("at") {
            self.next();
            Some(self.step_number("its time, runner milliseconds")?)
        } else {
            None
        };
        Ok(Step::Midi {
            step: MidiStep::Expect {
                output,
                data,
                at,
                present,
            },
            span,
        })
    }

    /// After `prefer`: `midi.state <word>`, `midi.inputs "<a, b>"`,
    /// `midi.outputs "<a, b>"`, each at most once.
    pub(super) fn prefer_midi(&mut self, span: Span) -> R<Step> {
        const USAGE: &str =
            "`prefer midi.state granted midi.outputs \"synth\" midi.inputs \"keys, pads\"`";
        let (mut state, mut inputs, mut outputs) = (None, None, None);
        while self.at_ident("midi") {
            self.next();
            self.expect_punct(".")?;
            let (fact, at) = self.ident()?;
            let names = |text: String| -> Vec<String> {
                text.split(',')
                    .map(|n| n.trim().to_string())
                    .filter(|n| !n.is_empty())
                    .collect()
            };
            let twice = match fact.as_str() {
                "state" => {
                    let (word, word_at) = self.ident()?;
                    if !["prompt", "granted", "denied", "unavailable"].contains(&word.as_str()) {
                        return Err(SyntaxError {
                            id: "syntax-expected-step",
                            message: format!("`midi.state` is prompt, granted, denied or unavailable, not `{word}`"),
                            span: word_at,
                        });
                    }
                    state.replace(word).is_some()
                }
                "inputs" => inputs.replace(names(self.str_lit("the inputs' names, comma-separated")?)).is_some(),
                "outputs" => outputs.replace(names(self.str_lit("the outputs' names, comma-separated")?)).is_some(),
                other => {
                    return Err(SyntaxError {
                        id: "syntax-expected-step",
                        message: format!("`prefer` sets `midi.state`, `midi.inputs` and `midi.outputs`, not `midi.{other}`: {USAGE}"),
                        span: at,
                    })
                }
            };
            if twice {
                return Err(SyntaxError {
                    id: "syntax-expected-step",
                    message: format!("`midi.{fact}` is given twice"),
                    span: at,
                });
            }
        }
        if state.is_none() && inputs.is_none() && outputs.is_none() {
            return self.err(
                "syntax-expected-step",
                format!("`prefer` in a test sets MIDI's facts: {USAGE}"),
            );
        }
        self.newline()?;
        Ok(Step::Midi {
            step: MidiStep::Prefer {
                state,
                inputs,
                outputs,
            },
            span,
        })
    }
}
