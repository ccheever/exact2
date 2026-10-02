//! A recursive-descent parser over the lexer's tokens.
//!
//! Every rejection carries a stable id (`syntax-…`), a message, and a span.
//! A refused top-level declaration is skipped to the next line that starts
//! in the first column, so one run reports each declaration's first error.

use crate::ast::*;
use crate::lexer::{template_expr_end, LexError, Lexer, Token, TokenKind};
use crate::Span;

mod expr;
mod keyframes;
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

/// At most this many refusals from one file's lexing or parsing.
pub const MAX_REFUSALS: usize = 20;

/// Parse one file.
pub fn parse(src: &str) -> Result<File, SyntaxError> {
    parse_source(src, 0)
}

/// Parse a file with the loader's source identity on every token and error.
pub fn parse_source(src: &str, source_id: u32) -> Result<File, SyntaxError> {
    parse_source_all(src, source_id).map_err(|mut all| all.swap_remove(0))
}

/// Parse a file and report every refusal (at most [`MAX_REFUSALS`]): each
/// line that does not lex, else each top-level declaration that does not
/// parse — a refused declaration is skipped to the next line that starts in
/// the first column, and parsing goes on from there.
pub fn parse_source_all(src: &str, source_id: u32) -> Result<File, Vec<SyntaxError>> {
    let (tokens, lexed) = Lexer::tokenize_recovering(
        src,
        Span {
            source_id,
            ..Span::point(1, 1)
        },
    );
    let mut p = Parser {
        tokens,
        pos: 0,
        names: NameSpans::default(),
        depth: 0,
        view_depth: 0,
        last: 0,
    };
    let parsed = p.file_all();
    if lexed.is_empty() {
        return parsed;
    }
    // A line the lexer refused parses as whatever it managed: not news.
    let bad: Vec<u32> = lexed.iter().map(|e| e.span.line).collect();
    let mut all: Vec<SyntaxError> = lexed.into_iter().map(SyntaxError::from).collect();
    all.extend(
        parsed
            .err()
            .unwrap_or_default()
            .into_iter()
            .filter(|e| !bad.contains(&e.span.line)),
    );
    all.sort_by_key(|e| (e.span.line, e.span.col));
    all.truncate(MAX_REFUSALS);
    Err(all)
}

/// Parse lexer output and return its unchanged tokens for source-preserving tools.
pub(crate) fn parse_tokens(tokens: Vec<Token>) -> Result<(File, Vec<Token>), SyntaxError> {
    let mut p = Parser {
        tokens,
        pos: 0,
        names: NameSpans::default(),
        depth: 0,
        view_depth: 0,
        last: 0,
    };
    let file = p.file()?;
    Ok((file, p.tokens))
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    names: NameSpans,
    /// Expressions (and prefix operators) open around the current token.
    depth: u32,
    view_depth: usize,
    /// The tree depth of the expression last parsed.
    last: usize,
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
            // A keyword where a name goes says so, and how to get out of it
            // (LLP 1054 L2: a shape field `key`, an action `view`).
            TokenKind::Ident(w) => self.err(
                "syntax-expected-name",
                format!("expected a name, found `{w}`, a reserved word: choose another name (`{w}s`, `my{}{}`)", w[..1].to_uppercase(), &w[1..]),
            ),
            other => self.err(
                "syntax-expected-name",
                format!("expected a name, found {}", describe(&other)),
            ),
        }
    }

    /// A name in a place the grammar can't mistake for syntax: a shape field,
    /// a prop, a member after `.`, a named argument. A keyword that only
    /// structures a file (`state`, `key`, `view`, …) is a name here; one that
    /// shapes an expression or a block (`when`, `if`, `match`, …) never is.
    fn field_name(&mut self) -> R<(String, Span)> {
        match self.peek_kind().clone() {
            TokenKind::Ident(w) if is_name_word(&w) => {
                let t = self.next();
                Ok((w, t.span))
            }
            _ => self.ident(),
        }
    }

    fn newline(&mut self) -> R<()> {
        match self.peek_kind() {
            TokenKind::Newline => {
                self.next();
                Ok(())
            }
            TokenKind::Eof => Ok(()),
            other => {
                // `when filter = "all"`: a comparison was meant.
                let hint = if matches!(other, TokenKind::Punct("=")) {
                    "; `==` compares, and only a statement assigns"
                } else {
                    ""
                };
                self.err(
                    "syntax-expected-newline",
                    format!("expected end of line, found {}{hint}", describe(other)),
                )
            }
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
        self.declarations(false)
            .map_err(|mut all| all.swap_remove(0))
    }

    fn file_all(&mut self) -> Result<File, Vec<SyntaxError>> {
        self.declarations(true)
    }

    /// The file's declarations. With `recover`, a refused declaration is
    /// recorded and parsing resumes at the next line starting in column 1.
    fn declarations(&mut self, recover: bool) -> Result<File, Vec<SyntaxError>> {
        let mut file = File::default();
        let mut errors = Vec::new();
        loop {
            let start = self.pos;
            match self.declaration(&mut file) {
                Ok(true) => {}
                Ok(false) if errors.is_empty() => {
                    file.names = std::mem::take(&mut self.names);
                    return Ok(file);
                }
                Ok(false) => return Err(errors),
                Err(e) => {
                    errors.push(e);
                    if !recover || errors.len() >= MAX_REFUSALS {
                        return Err(errors);
                    }
                    // A declaration refused where it began is stepped past.
                    if self.pos == start {
                        self.next();
                    }
                    self.skip_to_declaration();
                }
            }
        }
    }

    /// Past a refused declaration: to the next token that begins a line in
    /// the first column, where every declaration begins (possibly the token
    /// the refusal stopped at), or the end.
    fn skip_to_declaration(&mut self) {
        while !matches!(self.peek_kind(), TokenKind::Eof) {
            let at_line_start = self.pos > 0
                && matches!(
                    self.tokens[self.pos - 1].kind,
                    TokenKind::Newline | TokenKind::Dedent
                );
            if at_line_start
                && self.peek().span.col == 1
                && matches!(self.peek_kind(), TokenKind::Ident(_))
            {
                return;
            }
            self.next();
        }
    }

    /// One top-level declaration into `file`; `false` at the end.
    fn declaration(&mut self, file: &mut File) -> R<bool> {
        {
            match self.peek_kind() {
                TokenKind::Eof => return Ok(false),
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
                TokenKind::Ident(w) if w == "keyframes" => {
                    let decl = self.keyframes_decl()?;
                    if let Some(first) = file.keyframes.iter().find(|k| k.name == decl.name) {
                        return duplicate("keyframes", &decl.name, decl.span, first.span);
                    }
                    file.keyframes.push(decl);
                }
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
                        "expected `routes`, `font`, `shape`, `style`, `keyframes`, `fn`, `use`, or `component`, found {}",
                        describe(other)
                    ),
                    )
                }
            }
        }
        Ok(true)
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
        let attrs = self.literal_attrs(&format!("style {name}"))?;
        Ok(StyleDecl { name, attrs, span })
    }

    /// An indented block of `attr=literal` lines, several to a line, each
    /// name once: the body of a `style` or of a keyframe.
    fn literal_attrs(&mut self, owner: &str) -> R<Vec<Attr>> {
        let lines = self.block(|p| p.literal_line(owner))?;
        let attrs: Vec<Attr> = lines.into_iter().flatten().collect();
        unique_attrs(&attrs, owner)?;
        Ok(attrs)
    }

    /// One line of `attr=literal` pairs, through its newline.
    fn literal_line(&mut self, owner: &str) -> R<Vec<Attr>> {
        let mut attrs = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::Newline | TokenKind::Eof) {
            let (aname, aspan) = match (self.peek_kind().clone(), self.peek2().clone()) {
                (TokenKind::Ident(n), TokenKind::Punct("=")) => (n, self.next().span),
                (other, _) => {
                    return self.err(
                        "syntax-expected-attr",
                        format!(
                            "expected `attr=literal` in `{owner}`, found {}",
                            describe(&other)
                        ),
                    )
                }
            };
            self.next();
            let value = match self.expr()? {
                // `-0.2` is a literal to anyone writing a style: fold it, so
                // `letter-spacing=-0.2` is a constant like `0.2`.
                Expr::Unary(UnOp::Neg, inner, span) if matches!(*inner, Expr::Number(..)) => {
                    let Expr::Number(n, _) = *inner else {
                        unreachable!()
                    };
                    Expr::Number(-n, span)
                }
                other => other,
            };
            if !matches!(value, Expr::Number(..) | Expr::Str(..) | Expr::Bool(..)) {
                return Err(SyntaxError {
                    id: "contract-style-literal",
                    message: format!("`{aname}` in `{owner}` must be a literal: a style is constant, and a node's own attribute may compute"),
                    span: aspan,
                });
            }
            attrs.push(Attr {
                name: aname,
                value,
                span: aspan,
            });
        }
        self.newline()?;
        Ok(attrs)
    }

    fn shape(&mut self) -> R<ShapeDecl> {
        let span = self.expect_word("shape")?;
        let name = self.named_ident(span)?;
        self.newline()?;
        let fields = self.block(|p| {
            let (name, span) = p.field_name()?;
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
                    if matches!(w.as_str(), "props" | "inject" | "slot" | "view") {
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
                                let (name, span) = p.field_name()?;
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
        // @ref LLP 1048.003 D6 — what shows until the source answers.
        let placeholder = if self.at_ident("else") {
            self.next();
            let at = self.peek().span;
            let source = self.source_ident(at)?;
            self.expect_punct("(")?;
            let args = self.call_args()?;
            Some(Placeholder {
                source,
                args,
                span: at,
            })
        } else {
            None
        };
        self.newline()?;
        Ok(ResourceDecl {
            name,
            source,
            args,
            shape,
            placeholder,
            span,
        })
    }

    fn mutation(&mut self) -> R<MutationDecl> {
        let span = self.expect_word("mutation")?;
        let name = self.named_ident(span)?;
        self.expect_word("as")?;
        self.expect_word("shape")?;
        let shape = self.type_expr()?;
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
            refreshes,
            then,
            span,
        })
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
        // `send` is a keyword only where the send statement starts: `send`
        // then a name. Elsewhere (`send = x`, `send(x)`, a prop or state
        // named `send`) it is an ordinary name.
        if self.at_ident("send") && matches!(self.peek2(), TokenKind::Ident(_)) {
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

    // ---- view -------------------------------------------------------------

    fn node(&mut self) -> R<Node> {
        // The plan accepts at most 256 nested sites. Refuse before constructing
        // a deeper AST: all later compiler passes recursively walk this tree.
        if self.view_depth >= 256 {
            return self.err("syntax-view-depth", "views nest more than 256 sites deep");
        }
        self.view_depth += 1;
        let result = self.node_inner();
        self.view_depth -= 1;
        result
    }

    fn node_inner(&mut self) -> R<Node> {
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
                let index = if self.at_punct(",") {
                    self.next();
                    Some(self.named_ident(span)?)
                } else {
                    None
                };
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
                    index,
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
            "map" | "filter" if matches!(self.peek2(), TokenKind::Punct("(")) => self.err(
                "syntax-map-view",
                format!("`{word}` makes a value, not view nodes: repeat children with `each x in xs key=x.id` (LLP 1017.003)"),
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
                self.unquoted_unit(&name, &value)?;
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
            } else if matches!(self.peek_kind(), TokenKind::Ident(name) if name == "autofocus") {
                let aspan = self.next().span;
                if attrs.iter().any(|a| a.name == "autofocus") {
                    return self.err(
                        "syntax-duplicate-attr",
                        "attribute `autofocus` appears twice",
                    );
                }
                attrs.push(Attr {
                    name: "autofocus".into(),
                    value: Expr::Bool(true, aspan),
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

    /// `width=10px` lexes as the number `10` and then a name `px` written
    /// against it: say it is a string rather than let `px` be an unknown name.
    fn unquoted_unit(&self, attr: &str, value: &Expr) -> R<()> {
        let (Expr::Number(n, at), TokenKind::Ident(unit)) = (value, self.peek_kind()) else {
            return Ok(());
        };
        let next = self.peek().span;
        if next.line != at.line || next.col != at.end_col {
            return Ok(());
        }
        Err(SyntaxError {
            id: "syntax-unquoted-length",
            message: format!(
                "`{attr}={n}{unit}` needs quotes: a value with a unit is a string, `{attr}=\"{n}{unit}\"` (a bare number is pixels)"
            ),
            span: Span { end_col: next.end_col, ..*at },
        })
    }

    fn named_args(&mut self) -> R<Vec<Attr>> {
        let mut out = Vec::new();
        while !self.at_punct(")") {
            let (name, span) = self.field_name()?;
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
        let (mut out, mut deepest) = (Vec::new(), 0);
        while !self.at_punct(")") {
            let arg = if matches!(self.peek_kind(), TokenKind::Ident(_))
                && matches!(self.peek2(), TokenKind::Punct("=" | ":"))
            {
                let (name, span) = self.field_name()?;
                if self.at_punct(":") {
                    return self.err(
                        "syntax-named-argument",
                        format!("named arguments use `{name}=value`, not `{name}: value`"),
                    );
                }
                self.expect_punct("=")?;
                let value = self.expr()?;
                self.last += 1;
                Expr::NamedArg(name, Box::new(value), span)
            } else if self.arrow_ahead() {
                self.arrow()?
            } else {
                self.expr()?
            };
            deepest = deepest.max(self.last);
            out.push(arg);
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct(")")?;
        self.last = deepest;
        Ok(out)
    }
}

/// A keyword that may still be a name where one is expected: every keyword but
/// those that begin or join an expression, a region, or a statement.
fn is_name_word(w: &str) -> bool {
    !is_keyword(w)
        || !matches!(
            w,
            "when" | "if" | "else" | "each" | "in" | "match" | "case" | "as" | "fn" | "refresh"
        )
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

/// Each attribute name once in `owner`.
fn unique_attrs(attrs: &[Attr], owner: &str) -> R<()> {
    for (index, attr) in attrs.iter().enumerate() {
        if attrs[..index].iter().any(|prior| prior.name == attr.name) {
            return Err(SyntaxError {
                id: "syntax-duplicate-attr",
                message: format!("attribute `{}` appears twice in `{owner}`", attr.name),
                span: attr.span,
            });
        }
    }
    Ok(())
}
