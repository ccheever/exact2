//! A recursive-descent parser over the lexer's tokens.
//!
//! Every rejection carries a stable id (`syntax-…`), a message, and a span.
//! The parser stops at the first error: a syntax error is a single defect to
//! show, not a list to guess through.

use crate::ast::*;
use crate::lexer::{template_expr_end, LexError, Lexer, Token, TokenKind};
use crate::Span;

#[path = "routes.rs"]
mod routes;
mod steps;

/// A parse failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    /// Stable id.
    pub id: &'static str,
    /// What went wrong.
    pub message: String,
    /// Where.
    pub span: Span,
}

impl From<LexError> for SyntaxError {
    fn from(e: LexError) -> Self {
        SyntaxError {
            id: e.id,
            message: e.message,
            span: e.span,
        }
    }
}

impl std::fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [{}] {}", self.span, self.id, self.message)
    }
}

/// Parse one file.
pub fn parse(src: &str) -> Result<File, SyntaxError> {
    parse_source(src, 0)
}

/// Parse a file with the loader's source identity on every token and error.
pub fn parse_source(src: &str, source_id: u32) -> Result<File, SyntaxError> {
    let tokens = Lexer::tokenize_at(
        src,
        Span {
            source_id,
            ..Span::point(1, 1)
        },
    )?;
    parse_tokens(tokens).map(|(file, _)| file)
}

/// Parse lexer output and return its unchanged tokens for source-preserving tools.
pub(crate) fn parse_tokens(tokens: Vec<Token>) -> Result<(File, Vec<Token>), SyntaxError> {
    let mut p = Parser {
        tokens,
        pos: 0,
        names: NameSpans::default(),
    };
    let file = p.file()?;
    Ok((file, p.tokens))
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    names: NameSpans,
}

type R<T> = Result<T, SyntaxError>;

fn duplicate<T>(what: &str, name: &str, span: Span, first: Span) -> R<T> {
    Err(SyntaxError {
        id: "syntax-duplicate-declaration",
        message: format!("{what} `{name}` is declared twice; first declared at {first}"),
        span,
    })
}

impl Parser {
    fn named_ident(&mut self, owner: Span) -> R<String> {
        let (name, span) = self.ident()?;
        self.names.names.insert(owner, span);
        Ok(name)
    }

    fn source_ident(&mut self, owner: Span) -> R<String> {
        let (name, span) = self.ident()?;
        self.names.sources.insert(owner, span);
        Ok(name)
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    fn peek2(&self) -> &TokenKind {
        &self.tokens[(self.pos + 1).min(self.tokens.len() - 1)].kind
    }

    fn next(&mut self) -> Token {
        let t = self.peek().clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn at_ident(&self, word: &str) -> bool {
        matches!(self.peek_kind(), TokenKind::Ident(w) if w == word)
    }

    fn at_punct(&self, p: &str) -> bool {
        matches!(self.peek_kind(), TokenKind::Punct(q) if *q == p)
    }

    fn eat_punct(&mut self, p: &str) -> bool {
        if self.at_punct(p) {
            self.next();
            true
        } else {
            false
        }
    }

    fn err<T>(&self, id: &'static str, message: impl Into<String>) -> R<T> {
        Err(SyntaxError {
            id,
            message: message.into(),
            span: self.peek().span,
        })
    }

    fn expect_punct(&mut self, p: &'static str) -> R<Span> {
        if self.at_punct(p) {
            Ok(self.next().span)
        } else {
            self.err(
                "syntax-expected",
                format!("expected `{p}`, found {}", describe(self.peek_kind())),
            )
        }
    }

    fn expect_word(&mut self, w: &'static str) -> R<Span> {
        if self.at_ident(w) {
            Ok(self.next().span)
        } else {
            self.err(
                "syntax-expected",
                format!("expected `{w}`, found {}", describe(self.peek_kind())),
            )
        }
    }

    fn ident(&mut self) -> R<(String, Span)> {
        match self.peek_kind().clone() {
            TokenKind::Ident(w) if !is_keyword(&w) => {
                let t = self.next();
                Ok((w, t.span))
            }
            other => self.err(
                "syntax-expected-name",
                format!("expected a name, found {}", describe(&other)),
            ),
        }
    }

    fn newline(&mut self) -> R<()> {
        match self.peek_kind() {
            TokenKind::Newline => {
                self.next();
                Ok(())
            }
            TokenKind::Eof => Ok(()),
            other => self.err(
                "syntax-expected-newline",
                format!("expected end of line, found {}", describe(other)),
            ),
        }
    }

    /// Consume `Indent`, run `body` until the matching `Dedent`.
    fn block<T>(&mut self, item: impl FnMut(&mut Self) -> R<T>) -> R<Vec<T>> {
        if !matches!(self.peek_kind(), TokenKind::Indent) {
            return Ok(Vec::new());
        }
        self.next();
        self.block_rest(item)
    }

    fn block_rest<T>(&mut self, mut item: impl FnMut(&mut Self) -> R<T>) -> R<Vec<T>> {
        let mut out = Vec::new();
        loop {
            match self.peek_kind() {
                TokenKind::Dedent => {
                    self.next();
                    return Ok(out);
                }
                TokenKind::Eof => return Ok(out),
                TokenKind::Newline => {
                    self.next();
                }
                _ => out.push(item(self)?),
            }
        }
    }

    fn required_block<T>(
        &mut self,
        span: Span,
        construct: &str,
        item: impl FnMut(&mut Self) -> R<T>,
    ) -> R<Vec<T>> {
        let body = self.block(item)?;
        if body.is_empty() {
            return Err(SyntaxError {
                id: "syntax-empty-block",
                message: format!("`{construct}` needs a non-empty indented body"),
                span,
            });
        }
        Ok(body)
    }

    // ---- declarations -----------------------------------------------------

    fn file(&mut self) -> R<File> {
        let mut file = File::default();
        loop {
            match self.peek_kind() {
                TokenKind::Eof => { file.names = std::mem::take(&mut self.names); return Ok(file); },
                TokenKind::Newline => {
                    self.next();
                }
                TokenKind::Ident(w) if w == "font" => file.fonts.push(self.font_decl()?),
                TokenKind::Ident(w) if w == "routes" => {
                    if file.routes.is_some() {
                        return self.err("route-duplicate", "an app declares exactly one `routes` table");
                    }
                    file.routes = Some(self.routes_decl()?);
                }
                TokenKind::Ident(w) if w == "shape" => file.shapes.push(self.shape()?),
                TokenKind::Ident(w) if w == "style" => file.styles.push(self.style()?),
                TokenKind::Ident(w) if w == "fn" => file.fns.push(self.fn_decl()?),
                TokenKind::Ident(w) if w == "test" => file.tests.push(self.test_decl()?),
                TokenKind::Ident(w) if w == "component" => {
                    let component = self.component()?;
                    if let Some(first) = file
                        .components
                        .iter()
                        .find(|prior| prior.name == component.name)
                    {
                        return duplicate("component", &component.name, component.span, first.span);
                    }
                    file.components.push(component);
                }
                TokenKind::Ident(w) if w == "use" => file.uses.push(self.use_decl()?),
                other => {
                    return self.err(
                        "syntax-expected-declaration",
                        format!(
                        "expected `routes`, `font`, `shape`, `style`, `fn`, `use`, or `component`, found {}",
                        describe(other)
                    ),
                    )
                }
            }
        }
    }

    /// `use Name from "./file.contract"` (LLP 1017 P8). Only a `.contract`
    /// file may be used: no TypeScript, no packages, no behaviours — data
    /// comes from the app's Rust data source and formatting from the roster
    /// or a `fn` (LLP 1004 D4).
    fn use_decl(&mut self) -> R<UseDecl> {
        let span = self.expect_word("use")?;
        let name = self.named_ident(span)?;
        self.expect_word("from")?;
        let path = match self.peek_kind().clone() {
            TokenKind::Str(s) => {
                self.next();
                s
            }
            other => {
                return self.err(
                    "syntax-expected-path",
                    format!("expected a file path in quotes, found {}", describe(&other)),
                )
            }
        };
        if !path.ends_with(".contract") {
            return Err(SyntaxError {
                id: "contract-no-imports",
                message: format!("`use … from \"{path}\"` is not admitted: only a `.contract` file may be used — data comes from the app's Rust data source and formatting from the stdlib roster (LLP 1004 D4)"),
                span,
            });
        }
        self.newline()?;
        Ok(UseDecl { name, path, span })
    }

    /// `fn name(param: type, …): type = expr` (LLP 1017 P5).
    fn fn_decl(&mut self) -> R<FnDecl> {
        let span = self.expect_word("fn")?;
        let name = self.named_ident(span)?;
        self.expect_punct("(")?;
        let mut params = Vec::new();
        while !self.at_punct(")") {
            let (pname, pspan) = self.ident()?;
            if let Some(first) = params.iter().find(|p: &&Param| p.name == pname) {
                return duplicate("function parameter", &pname, pspan, first.span);
            }
            self.expect_punct(":")?;
            let ty = self.type_expr()?;
            params.push(Param {
                name: pname,
                ty: Some(ty),
                span: pspan,
            });
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct(")")?;
        self.expect_punct(":")?;
        let ret = self.type_expr()?;
        self.expect_punct("=")?;
        let body = self.expr()?;
        self.newline()?;
        Ok(FnDecl {
            name,
            params,
            ret,
            body,
            span,
        })
    }

    /// `style Name` then lines of `attr=literal` (LLP 1017 P6).
    fn style(&mut self) -> R<StyleDecl> {
        let span = self.expect_word("style")?;
        let name = self.named_ident(span)?;
        self.newline()?;
        let lines = self.block(|p| {
            let mut attrs = Vec::new();
            while !matches!(p.peek_kind(), TokenKind::Newline | TokenKind::Eof) {
                let (aname, aspan) = match (p.peek_kind().clone(), p.peek2().clone()) {
                    (TokenKind::Ident(n), TokenKind::Punct("=")) => (n, p.next().span),
                    (other, _) => {
                        return p.err(
                            "syntax-expected-attr",
                            format!("expected `attr=literal` in a style, found {}", describe(&other)),
                        )
                    }
                };
                p.next();
                let value = p.expr()?;
                if !matches!(value, Expr::Number(..) | Expr::Str(..) | Expr::Bool(..)) {
                    return Err(SyntaxError {
                        id: "contract-style-literal",
                        message: format!("`{aname}` in `style {name}` must be a literal: a style is constant, and a node's own attribute may compute"),
                        span: aspan,
                    });
                }
                attrs.push(Attr {
                    name: aname,
                    value,
                    span: aspan,
                });
            }
            p.newline()?;
            Ok(attrs)
        })?;
        let attrs: Vec<Attr> = lines.into_iter().flatten().collect();
        for (index, attr) in attrs.iter().enumerate() {
            if attrs[..index].iter().any(|prior| prior.name == attr.name) {
                return Err(SyntaxError {
                    id: "syntax-duplicate-attr",
                    message: format!("attribute `{}` appears twice in `style {name}`", attr.name),
                    span: attr.span,
                });
            }
        }
        Ok(StyleDecl { name, attrs, span })
    }

    fn shape(&mut self) -> R<ShapeDecl> {
        let span = self.expect_word("shape")?;
        let name = self.named_ident(span)?;
        self.newline()?;
        let fields = self.block(|p| {
            let (name, span) = p.ident()?;
            p.expect_punct(":")?;
            let ty = p.type_expr()?;
            p.newline()?;
            Ok(Field { name, ty, span })
        })?;
        for (index, field) in fields.iter().enumerate() {
            if let Some(first) = fields[..index]
                .iter()
                .find(|prior| prior.name == field.name)
            {
                return duplicate("shape field", &field.name, field.span, first.span);
            }
        }
        Ok(ShapeDecl { name, fields, span })
    }

    fn type_expr(&mut self) -> R<TypeExpr> {
        // `action` is a keyword everywhere else; as a prop type it names an
        // action reference (LLP 1004 D3: children are views over their props).
        if self.at_ident("action") {
            let span = self.next().span;
            return Ok(TypeExpr::Named("action".into(), span));
        }
        let (name, span) = self.ident()?;
        match name.as_str() {
            "option" | "list" => {
                self.expect_punct("<")?;
                let inner = self.type_expr()?;
                self.expect_punct(">")?;
                Ok(if name == "option" {
                    TypeExpr::Option(Box::new(inner), span)
                } else {
                    TypeExpr::List(Box::new(inner), span)
                })
            }
            _ => Ok(TypeExpr::Named(name, span)),
        }
    }

    fn component(&mut self) -> R<Component> {
        let span = self.expect_word("component")?;
        let name = self.named_ident(span)?;
        self.newline()?;
        let mut c = Component {
            name,
            props: Vec::new(),
            injects: Vec::new(),
            slot: false,
            states: Vec::new(),
            derives: Vec::new(),
            resources: Vec::new(),
            mutations: Vec::new(),
            actions: Vec::new(),
            tasks: Vec::new(),
            view: Vec::new(),
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
                    if matches!(
                        w.as_str(),
                        "props" | "inject" | "slot" | "view" | "contract"
                    ) {
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
                        "slot" => {
                            self.next();
                            self.newline()?;
                            c.slot = true;
                        }
                        "state" | "derive" => {
                            let t = self.next();
                            let name = self.named_ident(t.span)?;
                            self.expect_punct("=")?;
                            let expr = self.expr()?;
                            self.newline()?;
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
                        "view" => {
                            self.next();
                            self.newline()?;
                            c.view = self.block(|p| p.node())?;
                        }
                        "contract" => {
                            self.next();
                            self.newline()?;
                            // Contract blocks are agent assertions; not compiled in v1.
                            self.block(|p| p.skip_line())?;
                        }
                        other => {
                            return self.err(
                                "syntax-unknown-section",
                                format!("unknown section `{other}`"),
                            )
                        }
                    }
                }
                other => {
                    return self.err(
                        "syntax-expected-section",
                        format!("expected a section, found {}", describe(&other)),
                    )
                }
            }
        }
        Ok(c)
    }

    fn skip_line(&mut self) -> R<()> {
        while !matches!(
            self.peek_kind(),
            TokenKind::Newline | TokenKind::Eof | TokenKind::Dedent | TokenKind::Indent
        ) {
            self.next();
        }
        if matches!(self.peek_kind(), TokenKind::Indent) {
            self.block(|p| p.skip_line())?;
        }
        self.newline()
    }

    fn resource(&mut self) -> R<ResourceDecl> {
        let span = self.expect_word("resource")?;
        let name = self.named_ident(span)?;
        self.expect_punct("=")?;
        let source = self.source_ident(span)?;
        self.expect_punct("(")?;
        let args = self.call_args()?;
        self.expect_word("as")?;
        self.expect_word("shape")?;
        let shape = self.type_expr()?;
        self.newline()?;
        Ok(ResourceDecl {
            name,
            source,
            args,
            shape,
            span,
        })
    }

    fn mutation(&mut self) -> R<MutationDecl> {
        let span = self.expect_word("mutation")?;
        let name = self.named_ident(span)?;
        self.expect_word("as")?;
        self.expect_word("shape")?;
        let shape = self.type_expr()?;
        self.newline()?;
        Ok(MutationDecl { name, shape, span })
    }

    fn action(&mut self) -> R<Action> {
        let span = self.expect_word("action")?;
        let name = self.named_ident(span)?;
        let mut params = Vec::new();
        if self.eat_punct("(") {
            while !self.at_punct(")") {
                let (pname, pspan) = self.ident()?;
                if let Some(first) = params.iter().find(|p: &&Param| p.name == pname) {
                    return duplicate("action parameter", &pname, pspan, first.span);
                }
                let ty = if self.eat_punct(":") {
                    Some(self.type_expr()?)
                } else {
                    None
                };
                params.push(Param {
                    name: pname,
                    ty,
                    span: pspan,
                });
                if !self.eat_punct(",") {
                    break;
                }
            }
            self.expect_punct(")")?;
        }
        let mut writes = Vec::new();
        if self.at_ident("writes") {
            self.next();
            loop {
                writes.push(self.ident()?);
                if !self.eat_punct(",") {
                    break;
                }
            }
        }
        self.newline()?;
        let body = self.block(|p| p.stmt())?;
        Ok(Action {
            name,
            params,
            writes,
            body,
            span,
        })
    }

    fn stmt(&mut self) -> R<Stmt> {
        if self.at_ident("if") {
            let span = self.expect_word("if")?;
            let cond = self.expr()?;
            self.newline()?;
            let then = self.required_block(span, "if", |p| p.stmt())?;
            let mut otherwise = Vec::new();
            if self.at_ident("else") {
                self.next();
                self.newline()?;
                otherwise = self.required_block(span, "else", |p| p.stmt())?;
            }
            return Ok(Stmt::If {
                cond,
                then,
                otherwise,
                span,
            });
        }
        if self.at_ident("match") {
            let span = self.expect_word("match")?;
            let subject = self.expr()?;
            self.newline()?;
            let mut some = None;
            let mut none = None;
            self.block(|p| {
                let case_span = p.expect_word("case")?;
                if p.at_ident("some") {
                    if some.is_some() {
                        return duplicate("match arm", "some", case_span, span);
                    }
                    p.next();
                    p.expect_punct("(")?;
                    let var = p.named_ident(span)?;
                    p.expect_punct(")")?;
                    p.newline()?;
                    some = Some((var, p.block(|q| q.stmt())?));
                } else {
                    p.expect_word("none")?;
                    if none.is_some() {
                        return duplicate("match arm", "none", case_span, span);
                    }
                    p.newline()?;
                    none = Some(p.block(|q| q.stmt())?);
                }
                Ok(())
            })?;
            let some = some.ok_or(SyntaxError {
                id: "contract-match-arms",
                message: "`match` needs `case some(x)`".into(),
                span,
            })?;
            let none = none.ok_or(SyntaxError {
                id: "contract-match-arms",
                message: "`match` needs `case none`".into(),
                span,
            })?;
            return Ok(Stmt::Match {
                subject,
                some,
                none,
                span,
            });
        }
        if self.at_ident("send") {
            let span = self.expect_word("send")?;
            let target = self.named_ident(span)?;
            self.expect_punct("=")?;
            let source = self.source_ident(span)?;
            self.expect_punct("(")?;
            let args = self.call_args()?;
            self.newline()?;
            return Ok(Stmt::Send {
                target,
                source,
                args,
                span,
            });
        }
        if self.at_ident("refresh") {
            let span = self.expect_word("refresh")?;
            let target = self.named_ident(span)?;
            self.newline()?;
            return Ok(Stmt::Refresh { target, span });
        }
        let (name, span) = self.ident()?;
        if self.eat_punct("=") {
            let expr = self.expr()?;
            self.newline()?;
            return Ok(Stmt::Assign {
                target: name,
                expr,
                span,
            });
        }
        if self.eat_punct("(") {
            let args = self.call_args()?;
            self.newline()?;
            return Ok(Stmt::Command { name, args, span });
        }
        self.err(
            "syntax-expected-statement",
            "expected `slot = expr`, `command(args)`, `send mutation = source(args)`, `refresh resource`, `if cond`, or `match option`",
        )
    }

    fn task(&mut self) -> R<Task> {
        let span = self.expect_word("task")?;
        let name = self.named_ident(span)?;
        self.expect_word("mount")?;
        self.newline()?;
        let mut every = None;
        self.block(|p| {
            let (f, fspan) = p.ident()?;
            if f != "every" {
                return p.err(
                    "contract-task-body",
                    "a v1 task body is `every(ms, action)`",
                );
            }
            p.expect_punct("(")?;
            let ms = p.expr()?;
            p.expect_punct(",")?;
            let action = p.named_ident(fspan)?;
            p.expect_punct(")")?;
            p.newline()?;
            if every.is_some() {
                return duplicate("task entry", "every", fspan, span);
            }
            every = Some((ms, action, fspan));
            Ok(())
        })?;
        let every = every.ok_or(SyntaxError {
            id: "contract-task-body",
            message: "a task needs `every(ms, action)`".into(),
            span,
        })?;
        Ok(Task { name, every, span })
    }

    // ---- view -------------------------------------------------------------

    fn node(&mut self) -> R<Node> {
        let (word, span) = match self.peek_kind().clone() {
            TokenKind::Ident(w) => (w, self.peek().span),
            other => {
                return self.err(
                    "syntax-expected-node",
                    format!("expected a view node, found {}", describe(&other)),
                )
            }
        };
        if self.at_continuation() {
            return self.err(
                "syntax-continuation-indent",
                format!(
                    "`{word}=` continues an element's attributes only on a line indented deeper than the element's; a child begins with a tag or component name"
                ),
            );
        }
        match word.as_str() {
            "provide" => {
                self.next();
                let name = self.named_ident(span)?;
                self.expect_punct("=")?;
                let expr = self.expr()?;
                self.newline()?;
                let body = self.block(|p| p.node())?;
                Ok(Node::Provide {
                    name,
                    expr,
                    body,
                    span,
                })
            }
            "children" => {
                self.next();
                self.newline()?;
                Ok(Node::Children { span })
            }
            "when" => {
                self.next();
                let cond = self.expr()?;
                self.newline()?;
                let then = self.required_block(span, "when", |p| p.node())?;
                let mut otherwise = Vec::new();
                if self.at_ident("else") {
                    self.next();
                    self.newline()?;
                    otherwise = self.required_block(span, "else", |p| p.node())?;
                }
                Ok(Node::When {
                    cond,
                    then,
                    otherwise,
                    span,
                })
            }
            "each" => {
                self.next();
                let var = self.named_ident(span)?;
                self.expect_word("in")?;
                let list = self.expr()?;
                self.expect_word("key")?;
                self.expect_punct("=")?;
                let key = self.expr()?;
                self.newline()?;
                let body = self.block(|p| p.node())?;
                Ok(Node::Each {
                    tag: 0,
                    var,
                    list,
                    key,
                    body,
                    span,
                })
            }
            "match" => {
                self.next();
                let subject = self.expr()?;
                self.newline()?;
                let mut some = None;
                let mut none = None;
                self.block(|p| {
                    let case_span = p.expect_word("case")?;
                    if p.at_ident("some") {
                        if some.is_some() {
                            return duplicate("match arm", "some", case_span, span);
                        }
                        p.next();
                        p.expect_punct("(")?;
                        let var = p.named_ident(span)?;
                        p.expect_punct(")")?;
                        p.newline()?;
                        some = Some((var, p.block(|q| q.node())?));
                    } else {
                        p.expect_word("none")?;
                        if none.is_some() {
                            return duplicate("match arm", "none", case_span, span);
                        }
                        p.newline()?;
                        none = Some(p.block(|q| q.node())?);
                    }
                    Ok(())
                })?;
                let some = some.ok_or(SyntaxError {
                    id: "contract-match-arms",
                    message: "`match` needs `case some(x)`".into(),
                    span,
                })?;
                let none = none.ok_or(SyntaxError {
                    id: "contract-match-arms",
                    message: "`match` needs `case none`".into(),
                    span,
                })?;
                Ok(Node::Match {
                    subject,
                    some,
                    none,
                    span,
                })
            }
            "else" | "case" => self.err(
                "syntax-stray-keyword",
                format!("`{word}` without a matching construct"),
            ),
            _ if word.chars().next().is_some_and(char::is_uppercase) => {
                self.next();
                self.expect_punct("(")?;
                let args = self.named_args()?;
                self.newline()?;
                let children = self.block(|p| p.node())?;
                Ok(Node::Use {
                    name: word,
                    args,
                    children,
                    span,
                })
            }
            _ => {
                self.next();
                let mut positional = Vec::new();
                let mut attrs = Vec::new();
                self.attr_line(&mut positional, &mut attrs, false)?;
                self.newline()?;
                let mut children = Vec::new();
                if matches!(self.peek_kind(), TokenKind::Indent) {
                    self.next();
                    // The attribute list may continue on deeper lines that
                    // begin with `name=`; a child never does (LLP 1035.005 D1).
                    while self.at_continuation() {
                        self.attr_line(&mut positional, &mut attrs, true)?;
                        self.newline()?;
                    }
                    children = self.block_rest(|p| p.node())?;
                }
                // LLP 1017.001's primary button argument is its visible text
                // child, not a prop on the pressable itself. Keeping it as a
                // real text node gives every host the same visible and
                // accessible subtree.
                if word == "button" && positional.len() == 1 {
                    children.insert(
                        0,
                        Node::Element {
                            tag: "text".into(),
                            positional: vec![positional.remove(0)],
                            attrs: Vec::new(),
                            children: Vec::new(),
                            span,
                            instance: 0,
                        },
                    );
                }
                Ok(Node::Element {
                    tag: word,
                    positional,
                    attrs,
                    children,
                    span,
                    instance: 0,
                })
            }
        }
    }

    /// Whether the line begins `name=`: an element's continued attributes.
    fn at_continuation(&self) -> bool {
        matches!(
            (self.peek_kind(), self.peek2()),
            (TokenKind::Ident(_), TokenKind::Punct("="))
        )
    }

    /// One line of an element: `attr=expr` pairs and, on the element's own
    /// line, positional expressions.
    fn attr_line(
        &mut self,
        positional: &mut Vec<Expr>,
        attrs: &mut Vec<Attr>,
        continuation: bool,
    ) -> R<()> {
        while !matches!(self.peek_kind(), TokenKind::Newline | TokenKind::Eof) {
            if let (TokenKind::Ident(name), TokenKind::Punct("=")) =
                (self.peek_kind().clone(), self.peek2().clone())
            {
                let aspan = self.next().span;
                self.next();
                let value = self.expr()?;
                if attrs.iter().any(|a: &Attr| a.name == name) {
                    return Err(SyntaxError {
                        id: "syntax-duplicate-attr",
                        message: format!("attribute `{name}` appears twice on the same element"),
                        span: aspan,
                    });
                }
                attrs.push(Attr {
                    name,
                    value,
                    span: aspan,
                });
            } else if continuation {
                return self.err(
                    "syntax-expected-attr",
                    format!(
                        "a continued attribute line holds `name=value` pairs only, found {}",
                        describe(self.peek_kind())
                    ),
                );
            } else {
                positional.push(self.expr()?);
            }
        }
        Ok(())
    }

    fn named_args(&mut self) -> R<Vec<Attr>> {
        let mut out = Vec::new();
        while !self.at_punct(")") {
            let (name, span) = self.ident()?;
            self.expect_punct("=")?;
            let value = self.expr()?;
            out.push(Attr { name, value, span });
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct(")")?;
        Ok(out)
    }

    fn call_args(&mut self) -> R<Vec<Expr>> {
        let mut out = Vec::new();
        while !self.at_punct(")") {
            out.push(self.expr()?);
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct(")")?;
        Ok(out)
    }

    // ---- expressions ------------------------------------------------------

    fn expr(&mut self) -> R<Expr> {
        self.ternary()
    }

    fn ternary(&mut self) -> R<Expr> {
        let cond = self.binary(0)?;
        if self.eat_punct("?") {
            let a = self.expr()?;
            self.expect_punct(":")?;
            let b = self.expr()?;
            let span = cond.span();
            return Ok(Expr::Ternary(
                Box::new(cond),
                Box::new(a),
                Box::new(b),
                span,
            ));
        }
        Ok(cond)
    }

    fn binary(&mut self, min_prec: u8) -> R<Expr> {
        let mut left = self.unary()?;
        loop {
            let (op, prec) = match self.peek_kind() {
                TokenKind::Punct("||") => (BinOp::Or, 1),
                TokenKind::Ident(w) if w == "or" => (BinOp::Or, 1),
                TokenKind::Punct("&&") => (BinOp::And, 2),
                TokenKind::Ident(w) if w == "and" => (BinOp::And, 2),
                TokenKind::Punct("==") => (BinOp::Eq, 3),
                TokenKind::Punct("!=") => (BinOp::Ne, 3),
                TokenKind::Punct("<") => (BinOp::Lt, 4),
                TokenKind::Punct("<=") => (BinOp::Le, 4),
                TokenKind::Punct(">") => (BinOp::Gt, 4),
                TokenKind::Punct(">=") => (BinOp::Ge, 4),
                TokenKind::Punct("+") => (BinOp::Add, 5),
                TokenKind::Punct("-") => (BinOp::Sub, 5),
                TokenKind::Punct("*") => (BinOp::Mul, 6),
                TokenKind::Punct("/") => (BinOp::Div, 6),
                TokenKind::Punct("%") => (BinOp::Rem, 6),
                _ => break,
            };
            if prec < min_prec {
                break;
            }
            self.next();
            let right = self.binary(prec + 1)?;
            let span = left.span();
            left = Expr::Binary(op, Box::new(left), Box::new(right), span);
        }
        Ok(left)
    }

    fn unary(&mut self) -> R<Expr> {
        if self.at_punct("-") {
            let span = self.next().span;
            let e = self.unary()?;
            return Ok(Expr::Unary(UnOp::Neg, Box::new(e), span));
        }
        if self.at_punct("!") || self.at_ident("not") {
            let span = self.next().span;
            let e = self.unary()?;
            return Ok(Expr::Unary(UnOp::Not, Box::new(e), span));
        }
        self.postfix()
    }

    fn postfix(&mut self) -> R<Expr> {
        let mut e = self.primary()?;
        while self.at_punct(".") {
            self.next();
            let (field, span) = self.ident()?;
            e = Expr::Member(Box::new(e), field, span);
        }
        Ok(e)
    }

    fn primary(&mut self) -> R<Expr> {
        let t = self.next();
        let span = t.span;
        match t.kind {
            TokenKind::Number(n) => Ok(Expr::Number(n, span)),
            TokenKind::Str(s) => Ok(Expr::Str(s, span)),
            TokenKind::Template(raw) => self.template(&raw, span),
            TokenKind::Punct("(") => {
                let e = self.expr()?;
                self.expect_punct(")")?;
                Ok(e)
            }
            TokenKind::Ident(w) => match w.as_str() {
                "true" => Ok(Expr::Bool(true, span)),
                "false" => Ok(Expr::Bool(false, span)),
                "none" => Ok(Expr::None(span)),
                "some" => {
                    self.expect_punct("(")?;
                    let e = self.expr()?;
                    self.expect_punct(")")?;
                    Ok(Expr::Some(Box::new(e), span))
                }
                "match" => {
                    let subject = self.expr()?;
                    self.expect_punct("{")?;
                    self.expect_word("case")?;
                    self.expect_word("some")?;
                    self.expect_punct("(")?;
                    let var = self.named_ident(span)?;
                    self.expect_punct(")")?;
                    self.expect_punct("=>")?;
                    let some = self.expr()?;
                    self.expect_punct(",")?;
                    self.expect_word("case")?;
                    self.expect_word("none")?;
                    self.expect_punct("=>")?;
                    let none = self.expr()?;
                    self.eat_punct(",");
                    self.expect_punct("}")?;
                    Ok(Expr::Match {
                        subject: Box::new(subject),
                        var,
                        some: Box::new(some),
                        none: Box::new(none),
                        span,
                    })
                }
                _ if is_keyword(&w) => Err(SyntaxError {
                    id: "syntax-keyword-as-value",
                    message: format!("`{w}` is a keyword"),
                    span,
                }),
                _ => {
                    if self.eat_punct("(") {
                        let args = self.call_args()?;
                        Ok(Expr::Call(w, args, span))
                    } else {
                        Ok(Expr::Ident(w, span))
                    }
                }
            },
            other => Err(SyntaxError {
                id: "syntax-expected-expression",
                message: format!("expected an expression, found {}", describe(&other)),
                span,
            }),
        }
    }

    fn template(&mut self, raw: &str, span: Span) -> R<Expr> {
        let mut parts = Vec::new();
        let mut text = String::new();
        let mut rest = raw;
        while let Some(i) = rest.find("${") {
            text.push_str(&rest[..i]);
            let after = &rest[i + 2..];
            let end = template_expr_end(after).ok_or(SyntaxError {
                id: "syntax-unterminated-template-expr",
                message: "`${` never closes".into(),
                span,
            })?;
            if !text.is_empty() {
                parts.push(TemplatePart::Text(std::mem::take(&mut text)));
            }
            let inner = &after[..end];
            // `raw` starts after the backtick. Keep byte offsets through Unicode
            // prefixes, repeated interpolations, and recursively nested templates.
            let col = span.col + 1 + (raw.len() - after.len()) as u32;
            let mut tokens = Lexer::tokenize_at(
                inner,
                Span {
                    source_id: span.source_id,
                    ..Span::point(span.line, col)
                },
            )?;
            // A template fragment ends at its closing brace, not the next file
            // line. In particular, `${}` must report that brace's position.
            tokens.last_mut().unwrap().span = Span {
                source_id: span.source_id,
                ..Span::point(span.line, col + inner.len() as u32)
            };
            let mut sub = Parser {
                tokens,
                pos: 0,
                names: NameSpans::default(),
            };
            let e = sub.expr()?;
            if !matches!(sub.peek_kind(), TokenKind::Newline | TokenKind::Eof) {
                return sub.err("syntax-template-expr", "unexpected token in `${…}`");
            }
            self.names.names.extend(sub.names.names);
            self.names.sources.extend(sub.names.sources);
            parts.push(TemplatePart::Expr(e));
            rest = &after[end + 1..];
        }
        text.push_str(rest);
        if !text.is_empty() {
            parts.push(TemplatePart::Text(text));
        }
        Ok(Expr::Template(parts, span))
    }
}

fn is_keyword(w: &str) -> bool {
    matches!(
        w,
        "component"
            | "font"
            | "shape"
            | "state"
            | "derive"
            | "resource"
            | "mutation"
            | "send"
            | "refresh"
            | "action"
            | "task"
            | "view"
            | "props"
            | "when"
            | "if"
            | "else"
            | "style"
            | "fn"
            | "test"
            | "expect"
            | "from"
            | "provide"
            | "children"
            | "inject"
            | "slot"
            | "each"
            | "in"
            | "key"
            | "match"
            | "case"
            | "writes"
            | "mount"
            | "as"
            | "and"
            | "or"
            | "not"
    )
}

fn describe(k: &TokenKind) -> String {
    match k {
        TokenKind::Ident(w) => format!("`{w}`"),
        TokenKind::Number(n) => format!("number {n}"),
        TokenKind::Str(_) => "a string".into(),
        TokenKind::Template(_) => "a template string".into(),
        TokenKind::Punct(p) => format!("`{p}`"),
        TokenKind::Newline => "end of line".into(),
        TokenKind::Indent => "an indented block".into(),
        TokenKind::Dedent => "the end of a block".into(),
        TokenKind::Eof => "end of file".into(),
    }
}
