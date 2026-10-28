//! Font declarations and authored test steps.

use super::*;

/// The hosts a test's `on` names, as the driver names them.
const TEST_HOSTS: [&str; 7] = ["web", "macos", "ios", "tvos", "linux", "windows", "android"];

impl Parser {
    /// `font "Name" = "path.ttf"`, or a block of `weight [weight] [italic] =
    /// path`: one weight, or a variable face's range of them.
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
                    weight: None,
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
            let weight = |n: f64, span| {
                if n.fract() != 0.0 || !(1.0..=1000.0).contains(&n) {
                    return Err(SyntaxError {
                        id: "syntax-font-weight",
                        message: format!("font face weight `{n}` is not a whole number from 1 to 1000"),
                        span,
                    });
                }
                Ok(n as u16)
            };
            let TokenKind::Number(n) = token.kind else {
                return Err(SyntaxError {
                    id: "syntax-font-weight",
                    message: "a font face starts with a whole CSS weight from 1 to 1000".into(),
                    span: token.span,
                });
            };
            let low = weight(n, token.span)?;
            let high = match p.peek_kind().clone() {
                TokenKind::Number(n) => {
                    let span = p.next().span;
                    let high = weight(n, span)?;
                    if high < low {
                        return Err(SyntaxError {
                            id: "syntax-font-weight",
                            message: format!("font face weight range `{low} {high}` runs backwards; write the lighter weight first"),
                            span,
                        });
                    }
                    high
                }
                _ => low,
            };
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
                weight: Some([low, high]),
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
        let hosts = self.test_hosts()?;
        self.newline()?;
        let steps = self.block(|p| p.step())?;
        // The viewport, the date, the zone, the locale and the seed are the
        // session's: it opens with them, before any other step.
        let leading = steps
            .iter()
            .take_while(|s| !launch_word(s).is_empty())
            .count();
        // Fetch faults and media preferences may also change between steps.
        let late = steps[leading..].iter().find(|s| {
            !launch_word(s).is_empty() && !matches!(s, Step::FailFetch { .. } | Step::Prefer { .. })
        });
        if let Some(late) = late {
            return Err(SyntaxError {
                id: "syntax-expected-step",
                message: format!(
                    "`{}` leads a test's steps: its session opens with it, before any other step",
                    launch_word(late)
                ),
                span: late.span(),
            });
        }
        for (i, a) in steps[..leading].iter().enumerate() {
            if let Some(first) = steps[..i].iter().find(|b| same_launch(a, b)) {
                return duplicate("launch line", launch_word(a), a.span(), first.span());
            }
        }
        if let Some(Step::ExpectPerf { span, .. }) = steps
            .iter()
            .take_while(|s| !matches!(s, Step::Perf { .. }))
            .find(|s| matches!(s, Step::ExpectPerf { .. }))
        {
            return Err(SyntaxError {
                id: "syntax-expected-step",
                message: "`expect perf` reads the counts of a `perf` block before it, and this test has none yet".into(),
                span: *span,
            });
        }
        Ok(TestDecl {
            name,
            hosts,
            steps,
            span,
        })
    }

    /// `perf` and its indented steps (LLP 1146 D3). The counts repeat only on
    /// the agent's clock, so a step that waits on the wall clock is refused.
    fn perf_block(&mut self, span: Span) -> R<Step> {
        self.newline()?;
        let steps = self.required_block(span, "`perf`", |p| p.step())?;
        for step in &steps {
            let refused = match step {
                Step::Perf { .. } | Step::ExpectPerf { .. } => Some("a `perf` block measures steps; its `expect perf` lines follow it, and blocks do not nest".to_string()),
                Step::Clock { arg, .. } if arg.ends_with(" real") => Some(format!("`clock {arg}` lets the wall clock run, so the counts would depend on the machine's speed; step the agent's clock (`clock +N`, `clock data`, `clock settle`)")),
                s if !launch_word(s).is_empty() => Some(format!("`{}` is a launch line and leads the test, not a `perf` block", launch_word(s))),
                _ => None,
            };
            if let Some(message) = refused {
                return Err(SyntaxError {
                    id: "syntax-expected-step",
                    message,
                    span: step.span(),
                });
            }
        }
        Ok(Step::Perf { steps, span })
    }

    /// `expect perf query <resource>|built "<name>"|evaluated "<name>" ==|<= n`.
    fn expect_perf(&mut self, span: Span) -> R<Step> {
        let subject = match self.peek_kind() {
            TokenKind::Ident(w) if ["query", "built", "evaluated"].contains(&w.as_str()) => w.clone(),
            other => {
                return self.err(
                    "syntax-expected-step",
                    format!("`expect perf` counts `query <resource>`, `built \"<testId or component>\"` or `evaluated \"<testId or component>\"`, found {}", describe(other)),
                )
            }
        };
        self.next();
        let target = if subject == "query" {
            self.ident()?.0
        } else {
            self.str_lit("a testId or a component's name")?
        };
        let at_most = if self.eat_punct("<=") {
            true
        } else {
            self.expect_punct("==")?;
            false
        };
        let value = match *self.peek_kind() {
            TokenKind::Number(n) if n >= 0.0 && n.fract() == 0.0 && n <= u32::MAX as f64 => {
                n as u32
            }
            ref other => {
                return self.err(
                    "syntax-expected-step",
                    format!(
                        "`expect perf` compares with a whole count, found {}",
                        describe(other)
                    ),
                )
            }
        };
        self.next();
        Ok(Step::ExpectPerf {
            subject,
            target,
            at_most,
            value,
            span,
        })
    }

    /// `on web, macos` after a test's name: the hosts the driver runs it on.
    fn test_hosts(&mut self) -> R<Vec<String>> {
        let mut hosts = Vec::new();
        if !self.at_ident("on") {
            return Ok(hosts);
        }
        self.next();
        loop {
            let host = match self.peek_kind() {
                TokenKind::Ident(w) if TEST_HOSTS.contains(&w.as_str()) => w.clone(),
                other => {
                    return self.err(
                        "syntax-expected",
                        format!(
                            "expected a host ({}), found {}",
                            TEST_HOSTS.join(", "),
                            describe(other)
                        ),
                    )
                }
            };
            let span = self.next().span;
            if hosts.contains(&host) {
                return Err(SyntaxError {
                    id: "syntax-expected",
                    message: format!("`{host}` is named twice"),
                    span,
                });
            }
            hosts.push(host);
            if !self.eat_punct(",") {
                return Ok(hosts);
            }
        }
    }

    /// A launch line's quoted value.
    fn launch_str(&mut self, word: &str, what: &str) -> R<String> {
        match self.peek_kind().clone() {
            TokenKind::Str(s) if !s.is_empty() => {
                self.next();
                Ok(s)
            }
            other => self.err(
                "syntax-expected-step",
                format!(
                    "`{word}` takes {what} in quotes, found {}",
                    describe(&other)
                ),
            ),
        }
    }

    /// A viewport, `1200x800` (the lexer reads `1200` and then the word
    /// `x800`), in whole points.
    fn viewport(&mut self, word: &str) -> R<(f64, f64)> {
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
                        "`{word}` takes a viewport as 1200x800, found {}",
                        describe(&other)
                    ),
                )
            }
        };
        if width.fract() != 0.0 || width < 1.0 || height < 1.0 {
            return self.err(
                "syntax-expected-step",
                format!("`{word}` takes whole points, as 1200x800"),
            );
        }
        Ok((width, height))
    }

    /// A step's number, a leading `-` included (a drag's offsets).
    pub(super) fn step_number(&mut self, what: &str) -> R<f64> {
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

    /// `request METHOD "/path" [json] [as persona]` (LLP 1116.006 D8): a
    /// write through the app's server as another person, by the driver.
    fn request_step(&mut self, span: Span) -> R<Step> {
        const METHODS: [&str; 5] = ["GET", "POST", "PUT", "PATCH", "DELETE"];
        let method = match self.peek_kind().clone() {
            TokenKind::Ident(m) if METHODS.contains(&m.as_str()) => {
                self.next();
                m
            }
            other => {
                return self.err(
                    "syntax-expected-step",
                    format!(
                        "`request` takes a method (GET, POST, PUT, PATCH or DELETE), then a path in quotes, as `request POST \"/items\" {{\"title\": \"Bread\"}} as grace`, then optionally `expect 409`; found {}",
                        describe(&other)
                    ),
                )
            }
        };
        let path = self.launch_str("request", "a path under the app's server, as \"/items\",")?;
        if !(path.starts_with('/') || path.starts_with("http://") || path.starts_with("https://")) {
            return Err(SyntaxError {
                id: "syntax-expected-step",
                message: format!("`request` takes a path under the app's server, starting with `/` (\"/items\"), or a whole URL, not {path:?}"),
                span,
            });
        }
        let body = match self.peek_kind() {
            TokenKind::Punct("{") | TokenKind::Punct("[") => Some(self.json_value()?),
            _ => None,
        };
        let persona = if self.at_ident("as") {
            self.next();
            match self.peek_kind().clone() {
                TokenKind::Ident(w) | TokenKind::Str(w) if !w.is_empty() => {
                    self.next();
                    Some(w)
                }
                other => {
                    return self.err(
                        "syntax-expected-step",
                        format!(
                            "`as` takes the persona whose token the request carries, as `as grace` (`as \"token-grace\"`), found {}",
                            describe(&other)
                        ),
                    )
                }
            }
        } else {
            None
        };
        // LLP 1116.010 D3: `expect 409`, the status a refusal answers.
        let status = if self.at_ident("expect") {
            self.next();
            match self.peek_kind().clone() {
                TokenKind::Number(n) if n.fract() == 0.0 && (100.0..=599.0).contains(&n) => {
                    self.next();
                    Some(n as u16)
                }
                other => {
                    return self.err(
                        "syntax-expected-step",
                        format!(
                            "`expect` takes the status the server must answer, as `expect 409`, found {}",
                            describe(&other)
                        ),
                    )
                }
            }
        } else {
            None
        };
        Ok(Step::Request {
            method,
            path,
            body,
            persona,
            status,
            span,
        })
    }

    /// A state's name, then fields and list indexes at any depth, joined by
    /// `.` (`gateway.state`, `rows.0`: feed F10, drums R7).
    fn state_path(&mut self) -> R<String> {
        let (mut name, _) = self.ident()?;
        while self.eat_punct(".") {
            name.push('.');
            match self.peek_kind().clone() {
                TokenKind::Number(n)
                    if n.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(&n) =>
                {
                    self.next();
                    name.push_str(&format!("{}", n as u64));
                }
                TokenKind::Ident(_) => name.push_str(&self.ident()?.0),
                other => {
                    return self.err(
                        "syntax-expected-name",
                        format!(
                            "expected a field or a list index after `.`, found {}",
                            describe(&other)
                        ),
                    );
                }
            }
        }
        Ok(name)
    }

    /// `clock until state <path> == <literal> within <ms>` (rules/DEFERRED.md,
    /// "Agent API", 2026-10-26), after `until`: the driver's own words, the
    /// literal as JSON text.
    fn clock_until(&mut self) -> R<String> {
        if !self.at_ident("state") {
            return self.err(
                "syntax-expected-step",
                "`clock until` takes `state <name> == <literal> within <ms>`",
            );
        }
        self.next();
        let path = self.state_path()?;
        self.expect_punct("==")?;
        let token = self.next();
        let value = match token.kind {
            TokenKind::Str(s) => json_string(&s),
            // A number too large for a double is no JSON the driver reads back.
            TokenKind::Number(n) if n.is_finite() => format!("{n}"),
            TokenKind::Punct("-") if matches!(self.peek_kind(), TokenKind::Number(n) if n.is_finite()) => {
                format!("-{}", self.step_number("a number")?)
            }
            TokenKind::Ident(w) if w == "true" || w == "false" || w == "null" => w,
            other => {
                return Err(SyntaxError {
                    id: "syntax-expected-step",
                    message: format!(
                        "`clock until state {path} ==` takes a string, a number, true, false or null, not {}",
                        describe(&other)
                    ),
                    span: token.span,
                })
            }
        };
        if !self.at_ident("within") {
            let found = describe(&self.peek_kind().clone());
            return self.err(
                "syntax-expected-step",
                format!("expected `within <ms>` after `clock until state {path} == {value}`, found {found}: the wall-clock bound"),
            );
        }
        self.next();
        let ms = self.step_number("the bound in milliseconds")?;
        if ms.fract() != 0.0 || !(1.0..=3_600_000.0).contains(&ms) {
            return self.err(
                "syntax-expected-step",
                format!("`within` takes whole milliseconds, up to an hour, not {ms}"),
            );
        }
        Ok(format!("until state {path} == {value} within {ms}"))
    }

    /// A JSON value from its tokens, as JSON text: a request's body. Its
    /// brackets may span lines.
    fn json_value(&mut self) -> R<String> {
        let token = self.next();
        Ok(match token.kind {
            TokenKind::Punct(open @ ("{" | "[")) => {
                let close = if open == "{" { "}" } else { "]" };
                let mut out = String::from(open);
                while !self.eat_punct(close) {
                    if out.len() > 1 {
                        self.expect_punct(",")?;
                        out.push(',');
                    }
                    if open == "{" {
                        out.push_str(&json_string(&self.str_lit("a key")?));
                        self.expect_punct(":")?;
                        out.push(':');
                    }
                    out.push_str(&self.json_value()?);
                }
                out.push_str(close);
                out
            }
            TokenKind::Str(s) => json_string(&s),
            TokenKind::Number(n) => format!("{n}"),
            TokenKind::Punct("-") if matches!(self.peek_kind(), TokenKind::Number(_)) => {
                format!("-{}", self.step_number("a number")?)
            }
            TokenKind::Ident(w) if w == "true" || w == "false" || w == "null" => w,
            other => {
                return Err(SyntaxError {
                    id: "syntax-expected-step",
                    message: format!(
                        "a request's body is JSON (an object, a list, a string, a number, true, false or null), not {}",
                        describe(&other)
                    ),
                    span: token.span,
                })
            }
        })
    }

    pub(super) fn step(&mut self) -> R<Step> {
        let (word, span) = match self.peek_kind().clone() {
            TokenKind::Ident(w) => (w, self.peek().span),
            other => {
                return self.err(
                    "syntax-expected-step",
                    format!(
                    "expected `tap`, `type`, `pick`, `clock`, `resize`, `reload`, `close`, `back`, `screenshot`, `fail fetch`, `pass fetch`, `prefer`, `request`, `expect`, or a launch line (`size`, `epoch`, `time-zone`, `locale`, `seed`, `fresh`, `before data`, `fail fetch`), found {}",
                    describe(&other)
                ),
                )
            }
        };
        self.next();
        let step = match word.as_str() {
            "tap" if self.at_ident("midi") => return self.tap_midi(span),
            "prefer" if self.at_ident("midi") => return self.prefer_midi(span),
            "prefer" => {
                let (name, _) = self.ident()?;
                let value = match self.next() {
                    Token { kind: TokenKind::Ident(value), .. } => value,
                    token => return Err(SyntaxError {
                        id: "syntax-expected-step",
                        message: "`prefer` takes a CSS media feature and its value, as `prefer prefers-color-scheme dark`".into(),
                        span: token.span,
                    }),
                };
                Step::Prefer { name, value, span }
            }
            "tap" => {
                let target = self.str_lit("a testId")?;
                if self.at_ident("drag") {
                    self.next();
                    // `drag to "B" [at x y]` (LLP 1094 D12) or `drag dx dy`.
                    let (dx, dy, to) = if self.at_ident("to") {
                        self.next();
                        let to = self.str_lit("the testId the drag ends on")?;
                        let at = if self.at_ident("at") {
                            self.next();
                            let x = self.step_number("the end's x in the node's box")?;
                            let y = self.step_number("the end's y in the node's box")?;
                            Some((x, y))
                        } else {
                            None
                        };
                        (0.0, 0.0, Some((to, at)))
                    } else {
                        let dx = self.step_number("the drag's dx in points")?;
                        let dy = self.step_number("the drag's dy in points")?;
                        (dx, dy, None)
                    };
                    let (mut press, mut over, mut hold) = (None, None, None);
                    let (mut from, mut mouse) = (None, false);
                    // Quoted reads or `clock`, last, while the finger is down (drums R8).
                    let mut during = Vec::new();
                    while let TokenKind::Ident(w) = self.peek_kind().clone() {
                        let twice = match w.as_str() {
                            "from" => from.is_some(),
                            "mouse" => mouse,
                            _ => false,
                        };
                        if twice {
                            return self.err(
                                "syntax-expected-step",
                                format!("`{w}` is given twice in one drag"),
                            );
                        }
                        if w == "mouse" {
                            self.next();
                            mouse = true;
                            continue;
                        }
                        if w == "from" {
                            self.next();
                            let x = self.step_number("the start's x in the node's box")?;
                            let y = self.step_number("the start's y in the node's box")?;
                            from = Some((x, y));
                            continue;
                        }
                        if w == "during" {
                            self.next();
                            while let TokenKind::Str(op) = self.peek_kind().clone() {
                                self.next();
                                if !during_read(&op) {
                                    return self.err(
                                        "syntax-expected-step",
                                        format!("`during` {op:?} is not a read or the clock (tree, layout, state, logs, screenshot, clock)"),
                                    );
                                }
                                during.push(op);
                            }
                            if during.is_empty() {
                                return self.err(
                                    "syntax-expected-step",
                                    "`during` takes a quoted op, as during \"clock +500\", and it is the last option",
                                );
                            }
                            if !matches!(self.peek_kind(), TokenKind::Newline | TokenKind::Eof) {
                                return self.err(
                                    "syntax-expected-step",
                                    "`during` is the last option of a drag",
                                );
                            }
                            break;
                        }
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
                        to,
                        from,
                        mouse,
                        press,
                        over,
                        hold,
                        during,
                        span,
                    });
                }
                let form = match self.peek_kind().clone() {
                    TokenKind::Ident(w) if w == "hover" || w == "dblclick" || w == "contextmenu" || w == "into" || w == "pinch" || w == "wheel" => {
                        self.next();
                        match w.as_str() {
                            "hover" => TapForm::Hover,
                            "dblclick" => TapForm::Dblclick,
                            "contextmenu" => TapForm::Contextmenu,
                            "pinch" => {
                                // `tap "id" pinch <scale> [at x y]` (stocks: the agent op, refused as a newline).
                                let scale = self.step_number("the pinch's scale, a number greater than 0")?;
                                if !scale.is_finite() || scale <= 0.0 {
                                    return self.err(
                                        "syntax-expected-step",
                                        "`pinch` takes a scale greater than 0",
                                    );
                                }
                                let at = if self.at_ident("at") {
                                    self.next();
                                    Some((
                                        self.step_number("the pinch's x in the node's box")?,
                                        self.step_number("the pinch's y in the node's box")?,
                                    ))
                                } else {
                                    None
                                };
                                TapForm::Pinch { scale, at }
                            }
                            // `tap "id" wheel <dx> <dy>`: the agent's wheel, so a Mac's scroll can be tested.
                            "wheel" => {
                                let dx = self.step_number("the wheel's dx, in points")?;
                                let dy = self.step_number("the wheel's dy, in points")?;
                                if !dx.is_finite() || !dy.is_finite() {
                                    return self.err("syntax-expected-step", "`wheel` takes two finite deltas");
                                }
                                TapForm::Wheel { dx, dy }
                            }
                            _ => TapForm::Into(self.str_lit("the row's key")?),
                        }
                    }
                    TokenKind::Ident(w) if w == "mediasession" => self.tap_media_session()?,
                    _ => TapForm::Press,
                };
                let modifiers = if form == TapForm::Press && self.at_ident("modifiers") {
                    self.next();
                    self.str_lit("the modifiers held, as \"Shift+Meta\"")?
                } else {
                    String::new()
                };
                Step::Tap {
                    target,
                    form,
                    modifiers,
                    span,
                }
            }
            "type" => {
                let target = self.str_lit("a testId")?;
                if self.at_ident("key") {
                    self.next();
                    let key = self.str_lit("the key's name")?;
                    // `down` / `up` / `for <ms>`: one of them, on the virtual clock (platformer R7).
                    let (mut phase, mut duration) = (None, None);
                    if self.at_ident("down") || self.at_ident("up") {
                        phase = Some(if self.at_ident("down") { "down" } else { "up" }.to_string());
                        self.next();
                    } else if self.at_ident("for") {
                        self.next();
                        let ms = self.step_number("milliseconds the key stays down")?;
                        if !ms.is_finite() || ms < 0.0 {
                            return self.err(
                                "syntax-expected-step",
                                "`for` takes a nonnegative number of milliseconds",
                            );
                        }
                        duration = Some(ms);
                    }
                    if (phase.is_some() && self.at_ident("for"))
                        || (duration.is_some() && (self.at_ident("down") || self.at_ident("up")))
                    {
                        return self.err(
                            "syntax-expected-step",
                            "`key` takes `down`, `up`, or `for <ms>`, not both",
                        );
                    }
                    Step::Key {
                        target,
                        key,
                        phase,
                        duration,
                        span,
                    }
                } else if self.at_ident("copy") || self.at_ident("cut") || self.at_ident("paste") {
                    let edit = if self.at_ident("copy") { "copy" } else if self.at_ident("cut") { "cut" } else { "paste" };
                    self.next();
                    let text = if edit == "paste" { self.str_lit("the pasted text")? } else { String::new() };
                    Step::Clipboard { target, edit: edit.into(), text, span }
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
            // `pick "id" "path"…` or `pick "id" cancel` (files F11).
            "pick" => {
                let target = self.str_lit("the picker's node id, or its capability")?;
                let mut paths = Vec::new();
                if self.at_ident("cancel") {
                    self.next();
                } else {
                    paths.push(self.str_lit("a path to choose, or `cancel`")?);
                    while let TokenKind::Str(path) = self.peek_kind().clone() {
                        self.next();
                        paths.push(path);
                    }
                }
                Step::Pick {
                    target,
                    paths,
                    span,
                }
            }
            "clock" => {
                let arg = match self.peek_kind().clone() {
                    TokenKind::Ident(w) if w == "settle" || w == "data" => {
                        self.next();
                        w
                    }
                    // `clock held [<resource>…] [<n>s|<n>ms]`: real time passes, the
                    // clock frozen, until each held request (LLP 1016.000 D9), or
                    // each named, has had a reply, within the bound (the driver's
                    // `clock held`, 20 s unless given).
                    TokenKind::Ident(w) if w == "held" => {
                        self.next();
                        let mut arg = w;
                        while let TokenKind::Ident(name) = self.peek_kind().clone() {
                            self.next();
                            arg.push(' ');
                            arg.push_str(&name);
                        }
                        if let TokenKind::Number(n) = self.peek_kind().clone() {
                            self.next();
                            match self.peek_kind().clone() {
                                TokenKind::Ident(unit) if unit == "s" || unit == "ms" => {
                                    self.next();
                                    arg.push_str(&format!(" {n}{unit}"));
                                }
                                other => {
                                    return self.err(
                                        "syntax-expected-step",
                                        format!(
                                            "expected `s` or `ms` after `clock held`'s bound {n}, found {}",
                                            describe(&other)
                                        ),
                                    )
                                }
                            }
                        }
                        arg
                    }
                    // `clock until state <path> == <literal> within <ms>`: the wall
                    // clock passes, the clock frozen, until the state holds.
                    TokenKind::Ident(w) if w == "until" => {
                        self.next();
                        self.clock_until()?
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
                                "`clock` takes `settle`, `data`, `held`, `until state`, `+ms`, `+ms real`, or `ms`, found {}",
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
                let (width, height) = self.viewport("size")?;
                Step::Size {
                    width,
                    height,
                    span,
                }
            }
            // `resize 800x600`: the window, mid-test (reader: repagination;
            // the driver's `resize`, a desktop window's or the browser's).
            "resize" => {
                let (width, height) = self.viewport("resize")?;
                Step::Resize {
                    width,
                    height,
                    span,
                }
            }
            // `epoch "2026-09-21T12:00:00Z"`, `epoch 1790000000000` or
            // `epoch now` (the machine's clock, read once at launch, for a
            // test against a live backend): the driver's `--epoch`, which its
            // launch facts check again.
            "epoch" => {
                let value = match self.peek_kind().clone() {
                    TokenKind::Number(n) if n.fract() == 0.0 && n >= 0.0 => {
                        self.next();
                        format!("{n}")
                    }
                    TokenKind::Str(s) if iso_date(&s) => {
                        self.next();
                        s
                    }
                    TokenKind::Ident(w) if w == "now" => {
                        self.next();
                        w
                    }
                    other => {
                        return self.err(
                            "syntax-expected-step",
                            format!(
                                "`epoch` takes an ISO date in quotes, as \"2026-09-21T12:00:00Z\", whole Unix milliseconds, or `now` (the machine's clock at launch), found {}",
                                describe(&other)
                            ),
                        )
                    }
                };
                Step::Epoch { value, span }
            }
            "time-zone" => Step::TimeZone {
                zone: self.launch_str("time-zone", "an IANA zone, as \"America/New_York\",")?,
                span,
            },
            "locale" => Step::Locale {
                tag: self.launch_str("locale", "a BCP 47 tag, as \"fr-FR\",")?,
                span,
            },
            "seed" => {
                let seed = self.step_number("the seed, a whole number")?;
                // Below 2^32, as every host draws it (LLP 1116.005 D5).
                if seed.fract() != 0.0 || !(0.0..=4_294_967_295.0).contains(&seed) {
                    return self.err(
                        "syntax-expected-step",
                        "`seed` takes a whole number from 0 through 2^32 - 1 (4294967295), as every host's launch seed is",
                    );
                }
                Step::Seed { seed, span }
            }
            // @ref LLP 1086 D13 — `fresh ["seed.json"]`: the Snapback dev
            // server emptied (and seeded) before the test opens.
            "fresh" => {
                let seed = match self.peek_kind().clone() {
                    TokenKind::Str(_) => Some(self.launch_str("fresh", "a seed file's path ([{\"table\", \"row\"}, …])")?),
                    _ => None,
                };
                Step::Fresh { seed, span }
            }
            // `before data`: the first step does not wait for the app's data.
            "before" => {
                if !self.at_ident("data") {
                    return self.err(
                        "syntax-expected-step",
                        "`before` takes `data`: `before data`, the test's first step before the app's data lands",
                    );
                }
                self.next();
                Step::BeforeData { span }
            }
            // @ref LLP 1103 D3 — `fail fetch "<prefix>" [times N]`, `pass fetch "<prefix>"`.
            "fail" | "pass" => {
                if !self.at_ident("fetch") {
                    return self.err(
                        "syntax-expected-step",
                        format!("`{word}` takes `fetch`: `{word} fetch \"https://api.example.com/\"`"),
                    );
                }
                self.next();
                let prefix = self.launch_str(&format!("{word} fetch"), "the URL prefix a fetch starts with")?;
                if word == "pass" {
                    Step::PassFetch { prefix, span }
                } else {
                    let times = if self.at_ident("times") {
                        self.next();
                        let n = self.step_number("how many fetches fail, a positive whole number")?;
                        if n.fract() != 0.0 || !(1.0..=u32::MAX as f64).contains(&n) {
                            return self.err(
                                "syntax-expected-step",
                                "`fail fetch … times` takes a positive whole number",
                            );
                        }
                        Some(n as u32)
                    } else {
                        None
                    };
                    Step::FailFetch { prefix, times, span }
                }
            }
            // @ref LLP 1116.006 D8 — `request POST "/items" {"title": "Bread"} as grace`.
            "request" => self.request_step(span)?,
            "reload" => Step::Reload { span },
            "close" => Step::Close { span },
            // @ref LLP 1116.006 D4 — the platform's Back from what is on top: it names no target.
            "back" => {
                if !matches!(self.peek_kind(), TokenKind::Newline | TokenKind::Eof) {
                    return self.err(
                        "syntax-expected-step",
                        "`back` takes nothing: it is the platform's Back from what is on top (a dialog, a sheet, a pushed route); `tap` the app's own control to press it",
                    );
                }
                Step::Back { span }
            }
            "perf" => return self.perf_block(span),
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
                                "`expect` reads `tree`, `text`, `state`, `disabled`, `enabled`, `refusal`, `sound`, `midi`, `wakelock`, `mediasession`, `perf`, or `no failures`, found {}",
                                describe(&other)
                            ),
                        )
                    }
                };
                match what.as_str() {
                    "refusal" => Step::ExpectRefusal { target: self.str_lit("a mutation name")?, span },
                    "tree" => {
                        let present = if self.at_ident("has") {
                            self.next();
                            true
                        } else if self.at_ident("missing") {
                            self.next();
                            false
                        } else {
                            // `lacks`, `contains`, `not has` (app farm waves 1 and 2): the rewrite, not only the forms.
                            let found = match self.peek_kind() {
                                TokenKind::Ident(w) => w.clone(),
                                _ => String::new(),
                            };
                            let hint = match found.as_str() {
                                "lacks" | "not" | "absent" | "excludes" | "hasnt" | "without" => {
                                    "; for a node that is not there, write `expect tree missing \"<testId>\"`"
                                }
                                "contains" | "includes" | "shows" => {
                                    "; it matches a testId only: for a node's text, write `expect text \"<testId>\" == \"the whole text\"`"
                                }
                                _ => "",
                            };
                            let not = if found.is_empty() {
                                String::new()
                            } else {
                                format!(", not `{found}`")
                            };
                            return self.err(
                                "syntax-expected-step",
                                format!("`expect tree` takes `has \"testId\"` or `missing \"testId\"`{not}{hint}"),
                            );
                        };
                        let target = self.str_lit("a testId")?;
                        Step::ExpectTree {
                            target,
                            present,
                            span,
                        }
                    }
                    // LLP 1116.010 D3: a control's `disabled`, asserted rather than tapped.
                    "disabled" | "enabled" => {
                        let target = self.str_lit("a testId")?;
                        Step::ExpectDisabled {
                            target,
                            disabled: what == "disabled",
                            span,
                        }
                    }
                    "text" => {
                        let target = self.str_lit("a testId")?;
                        let other = match self.peek_kind() {
                            TokenKind::Ident(w) if ["contains", "includes", "has", "matches", "startsWith"].contains(&w.as_str()) => Some(w.clone()),
                            TokenKind::Punct(p) if *p != "==" => Some(p.to_string()),
                            _ => None,
                        };
                        if let Some(other) = other {
                            return self.err(
                                "syntax-expected-step",
                                format!("`expect text` compares the whole text with `==`, not `{other}`: the node's text, else its descendants' joined (a row's buttons' labels included). Give the part you mean its own node and testId, and write `expect text \"<that testId>\" == \"its whole text\"`"),
                            );
                        }
                        self.expect_punct("==")?;
                        let value = self.str_lit("the text")?;
                        Step::ExpectText {
                            target,
                            value,
                            span,
                        }
                    }
                    "wakelock" => {
                        let held = if self.at_ident("held") {
                            true
                        } else if self.at_ident("released") {
                            false
                        } else {
                            return self.err(
                                "syntax-expected-step",
                                "`expect wakelock` takes `held` or `released`",
                            );
                        };
                        self.next();
                        Step::ExpectWakelock { held, span }
                    }
                    "sound" => self.expect_sound(span)?,
                    "midi" => self.expect_midi(span)?,
                    "mediasession" => self.expect_media_session(span)?,
                    "perf" => self.expect_perf(span)?,
                    "no" => {
                        self.expect_word("failures")?;
                        Step::ExpectNoFailures { span }
                    }
                    "state" => {
                        let name = self.state_path()?;
                        self.expect_punct("==")?;
                        // `-3` is a number (drums R7). Other expressions stay refused.
                        let value = match self.expr()? {
                            Expr::Unary(UnOp::Neg, inner, span) if matches!(*inner, Expr::Number(..)) => {
                                let Expr::Number(n, _) = *inner else { unreachable!() };
                                Expr::Number(-n, span)
                            }
                            other => other,
                        };
                        let empty = matches!(&value, Expr::List(items, _) if items.is_empty());
                        if !empty
                            && !matches!(
                                value,
                                Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_)
                            )
                        {
                            return Err(SyntaxError {
                                id: "syntax-expected-step",
                                message: "`expect state name ==` takes a number, a string, a bool, `none`, or `[]`".into(),
                                span,
                            });
                        }
                        Step::ExpectState { name, value, span }
                    }
                    other => {
                        // r32 t1 tried `expect value` for a field.
                        let hint = if other == "value" || other == "checked" {
                            "; `expect text` reads a control's value too (a checkbox with a `checked` binding reads `true` or `false`)"
                        } else {
                            ""
                        };
                        return self.err(
                            "syntax-expected-step",
                            format!("`expect` reads `tree`, `text`, `state`, `disabled`, `enabled`, `refusal`, `sound`, `midi`, `wakelock`, `mediasession`, `perf`, or `no failures`, not `{other}`{hint}"),
                        );
                    }
                }
            }
            other => {
                return Err(SyntaxError {
                    id: "syntax-expected-step",
                    message: format!(
                    "expected `tap`, `type`, `pick`, `clock`, `resize`, `reload`, `close`, `back`, `screenshot`, `prefer`, `request`, `expect`, or a launch line (`size`, `epoch`, `time-zone`, `locale`, `seed`, `fresh`, `before data`), found `{other}`"
                ),
                    span,
                })
            }
        };
        self.newline()?;
        Ok(step)
    }
}

/// A string as JSON writes it.
fn json_string(s: &str) -> String {
    let mut out = String::from('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A drag's `during` op is a read or the clock, not a second finger (drums R8).
/// A filmed screenshot (`over … every`) loops on the clock.
fn during_read(op: &str) -> bool {
    let mut words = op.split_whitespace();
    let Some(head) = words.next() else {
        return false;
    };
    matches!(
        head,
        "tree" | "layout" | "state" | "logs" | "screenshot" | "clock"
    ) && !(head == "screenshot" && words.nth(1) == Some("over"))
}

/// The words of a test's launch lines, which a test file may also write at
/// its top level for every test in it (habits F7, calendar F13).
pub(super) const LAUNCH: [&str; 9] = [
    "size",
    "epoch",
    "time-zone",
    "locale",
    "seed",
    "fresh",
    "before",
    "fail",
    "prefer",
];

/// A launch line's word, or `""` for any other step.
pub(super) fn launch_word(step: &Step) -> &'static str {
    match step {
        Step::Prefer { .. } => "prefer",
        Step::Size { .. } => "size",
        Step::Epoch { .. } => "epoch",
        Step::TimeZone { .. } => "time-zone",
        Step::Locale { .. } => "locale",
        Step::Seed { .. } => "seed",
        Step::Fresh { .. } => "fresh",
        Step::BeforeData { .. } => "before data",
        Step::FailFetch { .. } => "fail fetch",
        _ => "",
    }
}

/// Whether a step is a launch line where it leads a test's steps.
pub fn is_launch(step: &Step) -> bool {
    !launch_word(step).is_empty()
}

/// Whether two launch lines set the same fact: `fail fetch` lines, the same
/// prefix; media preferences, the same feature name.
pub fn same_launch(a: &Step, b: &Step) -> bool {
    if let (Step::FailFetch { prefix: x, .. }, Step::FailFetch { prefix: y, .. }) = (a, b) {
        return x == y;
    }
    if let (Step::Prefer { name: x, .. }, Step::Prefer { name: y, .. }) = (a, b) {
        return x == y;
    }
    let word = launch_word(a);
    !word.is_empty() && word == launch_word(b)
}

/// `YYYY-MM-DD`, alone or before a `T` time: what the driver's epoch reads.
fn iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 10
        && b[..10].iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
        && (b.len() == 10 || b[10] == b'T')
}
