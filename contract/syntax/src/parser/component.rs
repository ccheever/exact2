//! A component and its sections. When the parser recovers, a refused
//! member (a `state`, `derive`, `resource`, `mutation`, `action`, `task` or
//! `sequence`) is recorded and skipped, and the component keeps the rest, so
//! later passes still check it; the member's name is hidden from what they
//! report (`Parser::hidden`). A component whose view is refused is refused
//! whole: without its view, later passes would refuse the component itself.

use super::*;
use crate::members::MembersLine;

impl Parser {
    pub(super) fn component(&mut self) -> R<Component> {
        self.component_body("component")
    }

    /// A members file's `members Name` block (LLP 1091.002 D2): a
    /// component's body holding members only.
    pub(super) fn members_block(&mut self) -> R<Component> {
        self.component_body("members")
    }

    fn component_body(&mut self, keyword: &'static str) -> R<Component> {
        let block = keyword == "members";
        let span = self.expect_word(keyword)?;
        // Reset with each top-level declaration.
        self.in_block = block;
        let name = self.named_ident(span)?;
        self.newline()?;
        let mut c = Component {
            name,
            props: Vec::new(),
            injects: Vec::new(),
            provides: Vec::new(),
            slot: false,
            states: Vec::new(),
            persist: Vec::new(),
            derives: Vec::new(),
            resources: Vec::new(),
            mutations: Vec::new(),
            actions: Vec::new(),
            tasks: Vec::new(),
            sequences: Vec::new(),
            tools: Vec::new(),
            view: Vec::new(),
            params: Default::default(),
            anatomy: Default::default(),
            members: Default::default(),
            span,
        };
        if !matches!(self.peek_kind(), TokenKind::Indent) {
            return self.err("syntax-empty-component", "a component needs a body");
        }
        let mut sections: Vec<(String, Span)> = Vec::new();
        self.next();
        loop {
            match self.peek_kind().clone() {
                TokenKind::Dedent => {
                    self.next();
                    break;
                }
                TokenKind::Eof => break,
                TokenKind::Newline => {
                    self.next();
                }
                TokenKind::Ident(w) => {
                    let once = matches!(
                        w.as_str(),
                        "props" | "inject" | "provide" | "slot" | "view" | "params" | "parts"
                    );
                    if block && (once || w == "members") {
                        return self.err(
                            "syntax-members-section",
                            format!(
                                "a `members` block holds `state`, `derive`, `resource`, \
                                 `mutation`, `action`, `task` and `sequence`; write `{w}` in \
                                 the component's own body (LLP 1091.002 D3)"
                            ),
                        );
                    }
                    if once {
                        let section_span = self.peek().span;
                        if let Some((_, first)) = sections.iter().find(|(name, _)| name == &w) {
                            return duplicate("component section", &w, section_span, *first);
                        }
                        sections.push((w.clone(), section_span));
                    }
                    match w.as_str() {
                        "props" | "inject" => {
                            self.next();
                            self.newline()?;
                            let list = self.block(|p| {
                                let (name, span) = p.ident()?;
                                p.expect_punct(":")?;
                                let ty = p.type_expr()?;
                                // `disabled: bool = false`: the euxy port's default.
                                if p.at_punct("=") {
                                    return p.err(
                                        "syntax-expected-newline",
                                        match w.as_str() {
                                            "props" => format!("a prop has no default: pass `{name}=<value>` at every use, or declare `{name}: option<{ty}>`, pass `{name}=none` where it is unset, and read it as `match {name} {{ case some(v) => v, case none => <default> }}`"),
                                            _ => format!("an inject has no default: a component above `provide`s `{name}`"),
                                        },
                                    );
                                }
                                p.newline()?;
                                Ok(Param {
                                    name,
                                    ty: Some(ty),
                                    span,
                                })
                            })?;
                            if w == "props" {
                                c.props = list;
                            } else {
                                c.injects = list;
                            }
                        }
                        // @ref LLP 1035.005.000 D9 — one binding per line; a
                        // bare name provides the in-scope value of that name.
                        "provide" => {
                            self.next();
                            self.newline()?;
                            let mut provides: Vec<Binding> = Vec::new();
                            for b in self.block(|p| {
                                let (name, span) = p.ident()?;
                                let expr = if p.eat_punct("=") {
                                    p.expr()?
                                } else {
                                    Expr::Ident(name.clone(), span)
                                };
                                p.newline()?;
                                Ok(Binding { name, expr, span })
                            })? {
                                if let Some(first) = provides.iter().find(|p| p.name == b.name) {
                                    return duplicate("provided name", &b.name, b.span, first.span);
                                }
                                provides.push(b);
                            }
                            c.provides = provides;
                        }
                        "slot" => {
                            self.next();
                            self.newline()?;
                            c.slot = true;
                        }
                        // @ref LLP 1091.002 D1 — the loader splices the
                        // file's members in where this line stands.
                        "members" => {
                            let span = self.next().span;
                            let TokenKind::Str(path) = self.peek_kind().clone() else {
                                return self.err(
                                    "syntax-expected-path",
                                    "expected the members file's path in quotes, like \
                                     `members \"./composer.contract\"`",
                                );
                            };
                            self.next();
                            self.newline()?;
                            let at = crate::members::Splice::after(&c);
                            c.members.lines.push(MembersLine { path, at, span });
                        }
                        "params" => c.params.list = self.params_section()?,
                        "parts" => c.anatomy.parts = self.parts_section()?,
                        "view" => {
                            self.next();
                            self.newline()?;
                            c.view = self.block(|p| p.node())?;
                            // `part=` is the anatomy's, never a node's (LLP 1139 D2).
                            c.anatomy.marks = crate::anatomy::take_marks(&mut c.view);
                        }
                        "state" | "derive" | "resource" | "mutation" | "action" | "task"
                        | "sequence" | "tool" => {
                            let start = self.pos;
                            if let Err(e) = self.member(&mut c, &w) {
                                if !self.recover {
                                    return Err(e);
                                }
                                if let Some(TokenKind::Ident(name)) =
                                    self.tokens.get(start + 1).map(|t| &t.kind)
                                {
                                    self.hidden.push(name.clone());
                                }
                                self.recovered.push(e);
                                self.skip_member(start);
                            }
                        }
                        // @ref LLP 1035.005.000 D8 — accepted assertion text
                        // that compiled to nothing promised more than a comment.
                        "contract" => {
                            return self.err(
                                "syntax-contract-block",
                                "a component's `contract` section is not part of Contract: \
                                 write the assertions as `test` blocks in `app.test.contract` \
                                 (LLP 1017 P7)",
                            )
                        }
                        "timer" | "interval" => {
                            let end = self.tokens[self.pos..]
                                .iter()
                                .position(|t| t.kind == TokenKind::Newline)
                                .map_or(self.tokens.len(), |n| self.pos + n);
                            return self.err(
                                "syntax-unknown-section",
                                names::timer_hint(&w, &self.tokens[self.pos + 1..end]),
                            );
                        }
                        other => {
                            return self.err(
                                "syntax-unknown-section",
                                format!("unknown section `{other}`{}", names::section_hint(other)),
                            )
                        }
                    }
                }
                // A one-line declaration continued on an indented line (a
                // resource's `else` under it; authoring bench).
                TokenKind::Indent
                    if matches!(self.peek2(), TokenKind::Ident(w)
                        if matches!(w.as_str(), "else" | "then" | "refreshes" | "as")) =>
                {
                    let TokenKind::Ident(w) = self.peek2().clone() else {
                        unreachable!()
                    };
                    let example = if w == "else" {
                        " (like `resource tasks = loadTasks() as shape Task[] else empty()`)"
                    } else {
                        ""
                    };
                    return self.err(
                        "syntax-expected-section",
                        format!(
                            "`{w} …` is indented under the line above, and a declaration is one \
                             line: join them{example}"
                        ),
                    );
                }
                other => {
                    if let Some(e) = self.continued_expression("syntax-expected-section") {
                        return e;
                    }
                    return self.err(
                        "syntax-expected-section",
                        format!("expected a section, found {}", describe(&other)),
                    );
                }
            }
        }
        Ok(c)
    }

    /// One member of a component's body, added to `c`.
    fn member(&mut self, c: &mut Component, w: &str) -> R<()> {
        match w {
            "state" | "derive" => {
                let t = self.next();
                let name = self.named_ident(t.span)?;
                // @ref LLP 1116.015 D2 — `state x: T = …` declares the type.
                let declared = if w == "state" && self.eat_punct(":") {
                    Some(self.type_expr()?)
                } else {
                    self.type_annotation(w, &name, ":")?;
                    None
                };
                self.expect_punct("=")?;
                self.on_its_line(w)?;
                let expr = self.expr()?;
                let none = matches!(expr, Expr::None(_));
                self.type_annotation(w, &name, if none { "as none" } else { "as" })?;
                let persist = self.persist_word(w, &name)?;
                self.newline()?;
                if let Some(span) = persist {
                    c.persist.push(Persist {
                        name: name.clone(),
                        span,
                    });
                }
                let expr = match declared {
                    Some(ty) => Expr::Typed(Box::new(expr), ty, t.span),
                    None => expr,
                };
                let b = Binding {
                    name,
                    expr,
                    span: t.span,
                };
                if w == "state" {
                    c.states.push(b)
                } else {
                    c.derives.push(b)
                }
            }
            "resource" => c.resources.push(self.resource()?),
            "mutation" => c.mutations.push(self.mutation()?),
            "action" => c.actions.push(self.action()?),
            "task" => c.tasks.push(self.task()?),
            "tool" => self.tool(c)?,
            _ => c.sequences.push(self.sequence()?),
        }
        Ok(())
    }

    /// Past a refused member that began at token `start`: to the next token
    /// that begins a line at the member's own depth, or to the `Dedent` that
    /// closes the component, never before where the refusal stopped.
    fn skip_member(&mut self, start: usize) {
        let failed = self.pos;
        let mut depth = 0usize;
        let mut i = start;
        while i < self.tokens.len() {
            match self.tokens[i].kind {
                TokenKind::Eof => break,
                TokenKind::Indent => depth += 1,
                TokenKind::Dedent if depth == 0 => break,
                TokenKind::Dedent => depth -= 1,
                TokenKind::Newline => {}
                _ if depth == 0
                    && i > start
                    && i >= failed
                    && matches!(
                        self.tokens[i - 1].kind,
                        TokenKind::Newline | TokenKind::Dedent
                    ) =>
                {
                    break
                }
                _ => {}
            }
            i += 1;
        }
        self.pos = i;
    }
}
