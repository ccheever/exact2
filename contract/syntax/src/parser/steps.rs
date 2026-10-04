//! Font declarations and authored test steps.

use super::*;

impl Parser {
    /// `font "Name" = "path.ttf"`, or a block of `weight [italic] = path`.
    pub(super) fn font_decl(&mut self) -> R<FontDecl> {
        let span = self.expect_word("font")?;
        let name_span = self.peek().span;
        let name = self.str_lit("a declared family name")?;
        self.names.names.insert(span, name_span);
        if self.eat_punct("=") {
            let source = self.str_lit("a TTF or OTF source path")?;
            self.newline()?;
            return Ok(FontDecl {
                name,
                faces: vec![FontFaceDecl {
                    weight: 400,
                    italic: false,
                    source,
                    span,
                }],
                span,
            });
        }
        self.newline()?;
        let faces = self.block(|p| {
            let token = p.next();
            let TokenKind::Number(n) = token.kind else {
                return Err(SyntaxError {
                    id: "syntax-font-weight",
                    message: "a font face starts with a whole CSS weight from 1 to 1000".into(),
                    span: token.span,
                });
            };
            if n.fract() != 0.0 || !(1.0..=1000.0).contains(&n) {
                return Err(SyntaxError {
                    id: "syntax-font-weight",
                    message: format!("font face weight `{n}` is not a whole number from 1 to 1000"),
                    span: token.span,
                });
            }
            let italic = if p.at_ident("italic") {
                p.next();
                true
            } else {
                false
            };
            p.expect_punct("=")?;
            let source = p.str_lit("a TTF or OTF source path")?;
            p.newline()?;
            Ok(FontFaceDecl {
                weight: n as u16,
                italic,
                source,
                span: token.span,
            })
        })?;
        if faces.is_empty() {
            return Err(SyntaxError {
                id: "syntax-font-faces",
                message: format!("`font \"{name}\"` needs at least one face"),
                span,
            });
        }
        Ok(FontDecl { name, faces, span })
    }

    pub(super) fn str_lit(&mut self, what: &str) -> R<String> {
        match self.peek_kind().clone() {
            TokenKind::Str(s) => {
                self.next();
                Ok(s)
            }
            other => self.err(
                "syntax-expected-string",
                format!("expected {what} in quotes, found {}", describe(&other)),
            ),
        }
    }

    /// `test "name"` with a block of steps (LLP 1017 P7): the agent API's
    /// operations by their names, and `expect` lines over their replies.
    pub(super) fn test_decl(&mut self) -> R<TestDecl> {
        let span = self.expect_word("test")?;
        let name = self.str_lit("the test's name")?;
        self.newline()?;
        let steps = self.block(|p| p.step())?;
        // The viewport is the session's: it opens at it, before any step.
        if let Some(Step::Size { span, .. }) = steps
            .iter()
            .skip(1)
            .find(|s| matches!(s, Step::Size { .. }))
        {
            return Err(SyntaxError {
                id: "syntax-expected-step",
                message: "`size` is a test's first step: its session opens at that viewport".into(),
                span: *span,
            });
        }
        Ok(TestDecl { name, steps, span })
    }

    /// A step's number, a leading `-` included (a drag's offsets).
    fn step_number(&mut self, what: &str) -> R<f64> {
        let negative = self.eat_punct("-");
        match self.peek_kind().clone() {
            TokenKind::Number(n) => {
                self.next();
                Ok(if negative { -n } else { n })
            }
            other => self.err(
                "syntax-expected-step",
                format!("expected {what}, found {}", describe(&other)),
            ),
        }
    }

    fn step(&mut self) -> R<Step> {
        let (word, span) = match self.peek_kind().clone() {
            TokenKind::Ident(w) => (w, self.peek().span),
            other => {
                return self.err(
                    "syntax-expected-step",
                    format!(
                    "expected `tap`, `type`, `clock`, `reload`, `screenshot`, `size`, or `expect`, found {}",
                    describe(&other)
                ),
                )
            }
        };
        self.next();
        let step = match word.as_str() {
            "tap" => {
                let target = self.str_lit("a testId")?;
                if self.at_ident("drag") {
                    self.next();
                    let dx = self.step_number("the drag's dx in points")?;
                    let dy = self.step_number("the drag's dy in points")?;
                    let (mut press, mut over, mut hold) = (None, None, None);
                    while let TokenKind::Ident(w) = self.peek_kind().clone() {
                        let slot = match w.as_str() {
                            "press" => &mut press,
                            "over" => &mut over,
                            "hold" => &mut hold,
                            _ => break,
                        };
                        if slot.is_some() {
                            return self.err(
                                "syntax-expected-step",
                                format!("`{w}` is given twice in one drag"),
                            );
                        }
                        self.next();
                        *slot = Some(self.step_number("milliseconds")?);
                    }
                    self.newline()?;
                    return Ok(Step::Drag {
                        target,
                        dx,
                        dy,
                        press,
                        over,
                        hold,
                        span,
                    });
                }
                let form = match self.peek_kind().clone() {
                    TokenKind::Ident(w) if w == "hover" || w == "dblclick" || w == "contextmenu" || w == "into" => {
                        self.next();
                        match w.as_str() {
                            "hover" => TapForm::Hover,
                            "dblclick" => TapForm::Dblclick,
                            "contextmenu" => TapForm::Contextmenu,
                            _ => TapForm::Into(self.str_lit("the row's key")?),
                        }
                    }
                    _ => TapForm::Press,
                };
                Step::Tap { target, form, span }
            }
            "type" => {
                let target = self.str_lit("a testId")?;
                if self.at_ident("key") {
                    self.next();
                    let key = self.str_lit("the key's name")?;
                    Step::Key { target, key, span }
                } else {
                    let text = self.str_lit("the text")?;
                    let append = self.at_ident("append");
                    if append {
                        self.next();
                    }
                    Step::Type {
                        target,
                        text,
                        append,
                        span,
                    }
                }
            }
            "clock" => {
                let arg = match self.peek_kind().clone() {
                    TokenKind::Ident(w) if w == "settle" => {
                        self.next();
                        "settle".to_string()
                    }
                    TokenKind::Punct("+") => {
                        self.next();
                        match self.peek_kind().clone() {
                            TokenKind::Number(n) => {
                                self.next();
                                // `clock +ms real`: that much real time passes
                                // with the clock (media, the network: LLP 1042 §3).
                                match self.peek_kind() {
                                    TokenKind::Ident(w) if w == "real" => {
                                        self.next();
                                        format!("+{n} real")
                                    }
                                    _ => format!("+{n}"),
                                }
                            }
                            other => {
                                return self.err(
                                    "syntax-expected-step",
                                    format!(
                                        "expected milliseconds after `+`, found {}",
                                        describe(&other)
                                    ),
                                )
                            }
                        }
                    }
                    TokenKind::Number(n) => {
                        self.next();
                        format!("{n}")
                    }
                    other => {
                        return self.err(
                            "syntax-expected-step",
                            format!(
                                "`clock` takes `settle`, `+ms`, `+ms real`, or `ms`, found {}",
                                describe(&other)
                            ),
                        )
                    }
                };
                Step::Clock { arg, span }
            }
            // `size 1200x800`, as the driver's `--size` (the lexer reads
            // `1200` and then the word `x800`).
            "size" => {
                let width = self.step_number("the viewport's width, as 1200x800")?;
                let height = match self.peek_kind().clone() {
                    TokenKind::Ident(w) if w.starts_with('x') && w[1..].parse::<u32>().is_ok() => {
                        self.next();
                        w[1..].parse::<u32>().unwrap_or_default() as f64
                    }
                    other => {
                        return self.err(
                            "syntax-expected-step",
                            format!(
                                "`size` takes a viewport as 1200x800, found {}",
                                describe(&other)
                            ),
                        )
                    }
                };
                if width.fract() != 0.0 || width < 1.0 || height < 1.0 {
                    return self.err(
                        "syntax-expected-step",
                        "`size` takes whole points, as 1200x800",
                    );
                }
                Step::Size {
                    width,
                    height,
                    span,
                }
            }
            "reload" => Step::Reload { span },
            "screenshot" => Step::Screenshot {
                path: self.str_lit("a file name")?,
                span,
            },
            "expect" => {
                // `state` is a keyword elsewhere; here it names the reply.
                let what = match self.peek_kind().clone() {
                    TokenKind::Ident(w) => {
                        self.next();
                        w
                    }
                    other => {
                        return self.err(
                            "syntax-expected-step",
                            format!(
                                "`expect` reads `tree`, `text`, or `state`, found {}",
                                describe(&other)
                            ),
                        )
                    }
                };
                match what.as_str() {
                    "tree" => {
                        let present = if self.at_ident("has") {
                            self.next();
                            true
                        } else if self.at_ident("missing") {
                            self.next();
                            false
                        } else {
                            return self.err(
                                "syntax-expected-step",
                                "`expect tree` takes `has \"testId\"` or `missing \"testId\"`",
                            );
                        };
                        let target = self.str_lit("a testId")?;
                        Step::ExpectTree {
                            target,
                            present,
                            span,
                        }
                    }
                    "text" => {
                        let target = self.str_lit("a testId")?;
                        self.expect_punct("==")?;
                        let value = self.str_lit("the text")?;
                        Step::ExpectText {
                            target,
                            value,
                            span,
                        }
                    }
                    "state" => {
                        let (mut name, _) = self.ident()?;
                        // A field of a record, at any depth (feed F10).
                        while self.eat_punct(".") {
                            name.push('.');
                            name.push_str(&self.ident()?.0);
                        }
                        self.expect_punct("==")?;
                        let value = self.expr()?;
                        if !matches!(
                            value,
                            Expr::Number(..)
                                | Expr::Str(..)
                                | Expr::Bool(..)
                                | Expr::None(_)
                                | Expr::EmptyList(_)
                        ) {
                            return Err(SyntaxError {
                                id: "syntax-expected-step",
                                message: "`expect state name ==` takes a number, a string, a bool, `none`, or `[]`".into(),
                                span,
                            });
                        }
                        Step::ExpectState { name, value, span }
                    }
                    other => {
                        return self.err(
                            "syntax-expected-step",
                            format!("`expect` reads `tree`, `text`, or `state`, not `{other}`"),
                        )
                    }
                }
            }
            other => {
                return Err(SyntaxError {
                    id: "syntax-expected-step",
                    message: format!(
                    "expected `tap`, `type`, `clock`, `reload`, `screenshot`, `size`, or `expect`, found `{other}`"
                ),
                    span,
                })
            }
        };
        self.newline()?;
        Ok(step)
    }
}
