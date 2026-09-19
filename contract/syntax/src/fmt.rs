//! The printer: a parsed file in canonical form (LLP 1035.005 D1).
//!
//! Two-space indentation; one space between attributes; attribute and
//! argument order exactly as written (order is meaning for duplicate rows);
//! string literals re-emitted with the same value; expressions re-spaced by
//! precedence, a computed attribute value in parentheses as authors write
//! it; comments and blank-line groups put back by position — a comment
//! before the declaration or node that follows it, a trailing comment after
//! its line. An element whose line would pass [`WIDTH`] columns breaks to
//! one attribute per continuation line (the grammar's continuation rule); a
//! component use whose arguments would, to one argument per line inside its
//! parentheses.

use crate::ast::*;
use crate::parser::{parse, SyntaxError};
use crate::Span;

/// The column past which an element's attributes and a use's arguments
/// break one per line.
pub const WIDTH: usize = 100;

/// Parse `src` and print it in canonical form.
pub fn format(src: &str) -> Result<String, SyntaxError> {
    print(&parse(src)?)
}

/// Print a parsed file in canonical form. A `contract` block is refused:
/// the parser skips its lines, so nothing could print them back.
pub fn print(file: &File) -> Result<String, SyntaxError> {
    for c in &file.components {
        if let Some((_, span)) = c.sections.iter().find(|(name, _)| name == "contract") {
            return Err(SyntaxError {
                id: "fmt-contract-block",
                message: format!(
                    "`component {}` has a `contract` block, which the parser skips and the printer cannot put back",
                    c.name
                ),
                span: *span,
            });
        }
    }
    let mut p = Printer {
        out: String::new(),
        trivia: &file.trivia,
        used: vec![false; file.trivia.len()],
        next: 0,
    };
    p.file(file);
    p.finish();
    Ok(p.out)
}

struct Printer<'a> {
    out: String,
    trivia: &'a [Trivia],
    used: Vec<bool>,
    next: usize,
}

enum Item<'a> {
    Routes(&'a RoutesDecl),
    Use(&'a UseDecl),
    Font(&'a FontDecl),
    Shape(&'a ShapeDecl),
    Style(&'a StyleDecl),
    Fn(&'a FnDecl),
    Test(&'a TestDecl),
    Component(&'a Component),
}

enum Member<'a> {
    Section(&'a str),
    State(&'a Binding),
    Derive(&'a Binding),
    Resource(&'a ResourceDecl),
    Mutation(&'a MutationDecl),
    Action(&'a Action),
    Task(&'a Task),
}

impl Printer<'_> {
    // ---- trivia -----------------------------------------------------------

    /// Emit every comment and blank group that precedes `line`, at `indent`.
    fn flush(&mut self, line: u32, indent: usize) {
        while self.next < self.trivia.len() && self.trivia[self.next].line() < line {
            let i = self.next;
            self.next += 1;
            if self.used[i] {
                continue;
            }
            self.used[i] = true;
            match &self.trivia[i] {
                Trivia::Comment { text, .. } | Trivia::Trailing { text, .. } => {
                    self.push_indent(indent);
                    self.out.push_str("//");
                    self.out.push_str(text);
                    self.out.push('\n');
                }
                Trivia::Blank { count, .. } => {
                    if !self.out.is_empty() {
                        for _ in 0..*count {
                            self.out.push('\n');
                        }
                    }
                }
            }
        }
    }

    /// What remains after the last construct: comments, never blank lines.
    fn finish(&mut self) {
        let mut trailing_blank = 0;
        for i in self.next..self.trivia.len() {
            if self.used[i] {
                continue;
            }
            match &self.trivia[i] {
                Trivia::Comment { text, .. } | Trivia::Trailing { text, .. } => {
                    for _ in 0..trailing_blank {
                        self.out.push('\n');
                    }
                    trailing_blank = 0;
                    self.out.push_str("//");
                    self.out.push_str(text);
                    self.out.push('\n');
                }
                Trivia::Blank { count, .. } => trailing_blank = *count,
            }
        }
        self.next = self.trivia.len();
    }

    fn push_indent(&mut self, indent: usize) {
        for _ in 0..indent {
            self.out.push_str("  ");
        }
    }

    /// A line that stands for source line `at.line`: the trivia before it,
    /// the text, and the trailing comment that line carried.
    fn line(&mut self, at: Span, indent: usize, text: &str) {
        self.flush(at.line, indent);
        self.push_indent(indent);
        self.out.push_str(text);
        for (i, t) in self.trivia.iter().enumerate() {
            if let Trivia::Trailing { text, span } = t {
                if span.line == at.line && !self.used[i] {
                    self.used[i] = true;
                    self.out.push_str(" //");
                    self.out.push_str(text);
                    break;
                }
            }
        }
        self.out.push('\n');
    }

    /// A line with no source line of its own (`else`, `case none`, a
    /// continuation).
    fn raw(&mut self, indent: usize, text: &str) {
        self.push_indent(indent);
        self.out.push_str(text);
        self.out.push('\n');
    }

    // ---- declarations -----------------------------------------------------

    fn file(&mut self, file: &File) {
        let mut items: Vec<(u32, Item)> = Vec::new();
        items.extend(file.routes.iter().map(|d| (d.span.line, Item::Routes(d))));
        items.extend(file.uses.iter().map(|d| (d.span.line, Item::Use(d))));
        items.extend(file.fonts.iter().map(|d| (d.span.line, Item::Font(d))));
        items.extend(file.shapes.iter().map(|d| (d.span.line, Item::Shape(d))));
        items.extend(file.styles.iter().map(|d| (d.span.line, Item::Style(d))));
        items.extend(file.fns.iter().map(|d| (d.span.line, Item::Fn(d))));
        items.extend(file.tests.iter().map(|d| (d.span.line, Item::Test(d))));
        items.extend(
            file.components
                .iter()
                .map(|d| (d.span.line, Item::Component(d))),
        );
        items.sort_by_key(|(line, _)| *line);
        for (_, item) in items {
            match item {
                Item::Routes(d) => {
                    self.line(d.span, 0, &format!("routes {}", d.slot));
                    let mut depths = Vec::new();
                    for row in &d.rows {
                        let depth = row.parent.map_or(1, |parent| depths[parent] + 1);
                        depths.push(depth);
                        let text = if row.notfound {
                            "notfound".to_string()
                        } else {
                            format!(
                                "{}{} {}",
                                if row.tab { "tab " } else { "" },
                                row.name,
                                string(&row.pattern)
                            )
                        };
                        self.line(row.span, depth, &text);
                    }
                }
                Item::Use(d) => {
                    let text = format!("use {} from {}", d.name, string(&d.path));
                    self.line(d.span, 0, &text);
                }
                Item::Font(d) => self.font(d),
                Item::Shape(d) => {
                    self.line(d.span, 0, &format!("shape {}", d.name));
                    for f in &d.fields {
                        self.line(f.span, 1, &format!("{}: {}", f.name, type_expr(&f.ty)));
                    }
                }
                Item::Style(d) => {
                    self.line(d.span, 0, &format!("style {}", d.name));
                    for a in &d.attrs {
                        self.line(a.span, 1, &format!("{}={}", a.name, expr(&a.value)));
                    }
                }
                Item::Fn(d) => {
                    let params: Vec<String> = d.params.iter().map(param).collect();
                    let text = format!(
                        "fn {}({}): {} = {}",
                        d.name,
                        params.join(", "),
                        type_expr(&d.ret),
                        expr(&d.body)
                    );
                    self.line(d.span, 0, &text);
                }
                Item::Test(d) => {
                    self.line(d.span, 0, &format!("test {}", string(&d.name)));
                    for s in &d.steps {
                        self.step(s);
                    }
                }
                Item::Component(c) => self.component(c),
            }
        }
    }

    fn font(&mut self, d: &FontDecl) {
        if let [face] = d.faces.as_slice() {
            if face.weight == 400 && !face.italic {
                let text = format!("font {} = {}", string(&d.name), string(&face.source));
                self.line(d.span, 0, &text);
                return;
            }
        }
        self.line(d.span, 0, &format!("font {}", string(&d.name)));
        for face in &d.faces {
            let italic = if face.italic { " italic" } else { "" };
            let text = format!("{}{italic} = {}", face.weight, string(&face.source));
            self.line(face.span, 1, &text);
        }
    }

    fn step(&mut self, s: &Step) {
        let (span, text) = match s {
            Step::Tap {
                target,
                hover,
                span,
            } => (
                *span,
                format!(
                    "tap {}{}",
                    string(target),
                    if *hover { " hover" } else { "" }
                ),
            ),
            Step::Type { target, text, span } => {
                (*span, format!("type {} {}", string(target), string(text)))
            }
            Step::Key { target, key, span } => (
                *span,
                format!("type {} key {}", string(target), string(key)),
            ),
            Step::Clock { arg, span } => (*span, format!("clock {arg}")),
            Step::Screenshot { path, span } => (*span, format!("screenshot {}", string(path))),
            Step::ExpectTree {
                target,
                present,
                span,
            } => (
                *span,
                format!(
                    "expect tree {} {}",
                    if *present { "has" } else { "missing" },
                    string(target)
                ),
            ),
            Step::ExpectText {
                target,
                value,
                span,
            } => (
                *span,
                format!("expect text {} == {}", string(target), string(value)),
            ),
            Step::ExpectState { name, value, span } => {
                (*span, format!("expect state {name} == {}", expr(value)))
            }
        };
        self.line(span, 1, &text);
    }

    fn component(&mut self, c: &Component) {
        self.line(c.span, 0, &format!("component {}", c.name));
        let mut members: Vec<(u32, Member)> = Vec::new();
        members.extend(
            c.sections
                .iter()
                .map(|(name, span)| (span.line, Member::Section(name))),
        );
        members.extend(c.states.iter().map(|b| (b.span.line, Member::State(b))));
        members.extend(c.derives.iter().map(|b| (b.span.line, Member::Derive(b))));
        members.extend(
            c.resources
                .iter()
                .map(|r| (r.span.line, Member::Resource(r))),
        );
        members.extend(
            c.mutations
                .iter()
                .map(|m| (m.span.line, Member::Mutation(m))),
        );
        members.extend(c.actions.iter().map(|a| (a.span.line, Member::Action(a))));
        members.extend(c.tasks.iter().map(|t| (t.span.line, Member::Task(t))));
        members.sort_by_key(|(line, _)| *line);
        let section_span = |name: &str| {
            c.sections
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, s)| *s)
                .unwrap_or_default()
        };
        for (_, member) in members {
            match member {
                Member::Section("props") => {
                    self.line(section_span("props"), 1, "props");
                    for p in &c.props {
                        self.line(p.span, 2, &param(p));
                    }
                }
                Member::Section("inject") => {
                    self.line(section_span("inject"), 1, "inject");
                    for p in &c.injects {
                        self.line(p.span, 2, &param(p));
                    }
                }
                Member::Section("slot") => self.line(section_span("slot"), 1, "slot"),
                Member::Section("view") => {
                    self.line(section_span("view"), 1, "view");
                    self.nodes(&c.view, 2);
                }
                Member::Section(_) => {}
                Member::State(b) => {
                    self.line(b.span, 1, &format!("state {} = {}", b.name, expr(&b.expr)))
                }
                Member::Derive(b) => {
                    self.line(b.span, 1, &format!("derive {} = {}", b.name, expr(&b.expr)))
                }
                Member::Resource(r) => {
                    let text = format!(
                        "resource {} = {}({}) as shape {}",
                        r.name,
                        r.source,
                        args(&r.args),
                        type_expr(&r.shape)
                    );
                    self.line(r.span, 1, &text);
                }
                Member::Mutation(m) => {
                    let text = format!("mutation {} as shape {}", m.name, type_expr(&m.shape));
                    self.line(m.span, 1, &text);
                }
                Member::Action(a) => {
                    let mut text = format!("action {}", a.name);
                    if !a.params.is_empty() {
                        let params: Vec<String> = a.params.iter().map(param).collect();
                        text.push_str(&format!("({})", params.join(", ")));
                    }
                    if !a.writes.is_empty() {
                        let writes: Vec<&str> = a.writes.iter().map(|(w, _)| w.as_str()).collect();
                        text.push_str(&format!(" writes {}", writes.join(", ")));
                    }
                    self.line(a.span, 1, &text);
                    self.stmts(&a.body, 2);
                }
                Member::Task(t) => {
                    self.line(t.span, 1, &format!("task {} mount", t.name));
                    let text = format!("every({}, {})", expr(&t.every.0), t.every.1);
                    self.line(t.every.2, 2, &text);
                }
            }
        }
    }

    // ---- statements -------------------------------------------------------

    fn stmts(&mut self, stmts: &[Stmt], indent: usize) {
        for s in stmts {
            match s {
                Stmt::Assign {
                    target,
                    expr: e,
                    span,
                } => self.line(*span, indent, &format!("{target} = {}", expr(e))),
                Stmt::Command {
                    name,
                    args: a,
                    span,
                } => self.line(*span, indent, &format!("{name}({})", args(a))),
                Stmt::Send {
                    target,
                    source,
                    args: a,
                    span,
                    ..
                } => {
                    let text = format!("send {target} = {source}({})", args(a));
                    self.line(*span, indent, &text);
                }
                Stmt::Refresh { target, span } => {
                    self.line(*span, indent, &format!("refresh {target}"))
                }
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                    span,
                } => {
                    self.line(*span, indent, &format!("if {}", expr(cond)));
                    self.stmts(then, indent + 1);
                    if !otherwise.is_empty() {
                        self.raw(indent, "else");
                        self.stmts(otherwise, indent + 1);
                    }
                }
                Stmt::Match {
                    subject,
                    some,
                    none,
                    span,
                } => {
                    self.line(*span, indent, &format!("match {}", expr(subject)));
                    self.raw(indent + 1, &format!("case some({})", some.0));
                    self.stmts(&some.1, indent + 2);
                    self.raw(indent + 1, "case none");
                    self.stmts(none, indent + 2);
                }
            }
        }
    }

    // ---- view -------------------------------------------------------------

    fn nodes(&mut self, nodes: &[Node], indent: usize) {
        for n in nodes {
            self.node(n, indent);
        }
    }

    fn node(&mut self, n: &Node, indent: usize) {
        match n {
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                span,
            } => self.element(tag, positional, attrs, children, *span, indent),
            Node::Use {
                name,
                args,
                children,
                span,
            } => {
                let parts: Vec<String> = args
                    .iter()
                    .map(|a| format!("{}={}", a.name, value(&a.value)))
                    .collect();
                let one = format!("{name}({})", parts.join(", "));
                if fits(indent, &one) || parts.len() < 2 {
                    self.line(*span, indent, &one);
                } else {
                    self.line(*span, indent, &format!("{name}("));
                    for part in &parts {
                        self.raw(indent + 1, &format!("{part},"));
                    }
                    self.raw(indent, ")");
                }
                self.nodes(children, indent + 1);
            }
            Node::Provide {
                name,
                expr: e,
                body,
                span,
            } => {
                self.line(*span, indent, &format!("provide {name} = {}", expr(e)));
                self.nodes(body, indent + 1);
            }
            Node::Children { span } => self.line(*span, indent, "children"),
            Node::When {
                cond,
                then,
                otherwise,
                span,
            } => {
                self.line(*span, indent, &format!("when {}", expr(cond)));
                self.nodes(then, indent + 1);
                if !otherwise.is_empty() {
                    self.raw(indent, "else");
                    self.nodes(otherwise, indent + 1);
                }
            }
            Node::Each {
                var,
                list,
                key,
                body,
                span,
                ..
            } => {
                let text = format!("each {var} in {} key={}", expr(list), expr(key));
                self.line(*span, indent, &text);
                self.nodes(body, indent + 1);
            }
            Node::Match {
                subject,
                some,
                none,
                span,
            } => {
                self.line(*span, indent, &format!("match {}", expr(subject)));
                self.raw(indent + 1, &format!("case some({})", some.0));
                self.nodes(&some.1, indent + 2);
                self.raw(indent + 1, "case none");
                self.nodes(none, indent + 2);
            }
        }
    }

    fn element(
        &mut self,
        tag: &str,
        positional: &[Expr],
        attrs: &[Attr],
        children: &[Node],
        span: Span,
        indent: usize,
    ) {
        let mut head = tag.to_string();
        for p in positional {
            head.push(' ');
            head.push_str(&value(p));
        }
        // `button "Text"` parses to a `text` child carrying the button's own
        // span; print it back as the primary argument it was.
        let mut rest = children;
        if tag == "button" && positional.is_empty() {
            if let Some(Node::Element {
                tag: child_tag,
                positional: child_positional,
                attrs: child_attrs,
                children: child_children,
                span: child_span,
            }) = children.first()
            {
                if child_tag == "text"
                    && *child_span == span
                    && child_positional.len() == 1
                    && child_attrs.is_empty()
                    && child_children.is_empty()
                {
                    head.push(' ');
                    head.push_str(&value(&child_positional[0]));
                    rest = &children[1..];
                }
            }
        }
        let parts: Vec<String> = attrs
            .iter()
            .map(|a| format!("{}={}", a.name, value(&a.value)))
            .collect();
        let mut one = head.clone();
        for part in &parts {
            one.push(' ');
            one.push_str(part);
        }
        if fits(indent, &one) || parts.len() < 2 {
            self.line(span, indent, &one);
        } else {
            self.line(span, indent, &format!("{head} {}", parts[0]));
            for part in &parts[1..] {
                self.raw(indent + 1, part);
            }
        }
        self.nodes(rest, indent + 1);
    }
}

/// Whether `text` at `indent` stays within [`WIDTH`] columns.
fn fits(indent: usize, text: &str) -> bool {
    indent * 2 + text.chars().count() <= WIDTH
}

fn param(p: &Param) -> String {
    match &p.ty {
        Some(t) => format!("{}: {}", p.name, type_expr(t)),
        None => p.name.clone(),
    }
}

fn args(a: &[Expr]) -> String {
    a.iter().map(expr).collect::<Vec<_>>().join(", ")
}

/// A written type.
pub fn type_expr(t: &TypeExpr) -> String {
    match t {
        TypeExpr::Named(n, _) => n.clone(),
        TypeExpr::Option(inner, _) => format!("option<{}>", type_expr(inner)),
        TypeExpr::List(inner, _) => format!("list<{}>", type_expr(inner)),
    }
}

/// A string literal with the same value: `"`, `\`, newline, and tab escaped.
pub fn string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// An attribute value or argument: a computed value in parentheses, so the
/// spaces inside it never read as attribute boundaries.
fn value(e: &Expr) -> String {
    let text = expr(e);
    if matches!(
        e,
        Expr::Binary(..) | Expr::Ternary(..) | Expr::Unary(UnOp::Not, ..) | Expr::Match { .. }
    ) {
        format!("({text})")
    } else {
        text
    }
}

/// Precedence, as the parser binds: the conditional lowest, then `or`,
/// `and`, equality, comparison, additive, multiplicative, unary, postfix.
fn prec(e: &Expr) -> u8 {
    match e {
        Expr::Ternary(..) => 0,
        Expr::Binary(op, ..) => match op {
            BinOp::Or => 1,
            BinOp::And => 2,
            BinOp::Eq | BinOp::Ne => 3,
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 4,
            BinOp::Add | BinOp::Sub => 5,
            BinOp::Mul | BinOp::Div | BinOp::Rem => 6,
        },
        Expr::Unary(..) => 7,
        Expr::Member(..) => 8,
        _ => 9,
    }
}

fn op(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Rem => "%",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "and",
        BinOp::Or => "or",
    }
}

/// `e` printed, in parentheses when it binds looser than `min`.
fn sub(e: &Expr, min: u8) -> String {
    let text = expr(e);
    if prec(e) < min {
        format!("({text})")
    } else {
        text
    }
}

/// An expression, re-spaced by precedence with the parentheses it needs.
pub fn expr(e: &Expr) -> String {
    match e {
        Expr::Number(n, _) => format!("{n}"),
        Expr::Str(s, _) => string(s),
        Expr::Template(parts, _) => {
            let mut out = String::from("`");
            for part in parts {
                match part {
                    TemplatePart::Text(t) => out.push_str(t),
                    TemplatePart::Expr(e) => {
                        out.push_str("${");
                        out.push_str(&expr(e));
                        out.push('}');
                    }
                }
            }
            out.push('`');
            out
        }
        Expr::Bool(b, _) => format!("{b}"),
        Expr::None(_) => "none".into(),
        Expr::Some(inner, _) => format!("some({})", expr(inner)),
        Expr::Ident(n, _) => n.clone(),
        Expr::Member(base, field, _) => format!("{}.{field}", sub(base, 8)),
        Expr::NamedArg(name, value, _) => format!("{name}={}", expr(value)),
        Expr::Call(name, a, _) => format!("{name}({})", args(a)),
        Expr::Unary(UnOp::Neg, inner, _) => {
            let text = sub(inner, 7);
            if text.starts_with('-') {
                format!("- {text}")
            } else {
                format!("-{text}")
            }
        }
        Expr::Unary(UnOp::Not, inner, _) => format!("not {}", sub(inner, 7)),
        Expr::Binary(o, l, r, _) => {
            let p = prec(e);
            format!("{} {} {}", sub(l, p), op(*o), sub(r, p + 1))
        }
        Expr::Ternary(c, a, b, _) => format!("{} ? {} : {}", sub(c, 1), sub(a, 1), sub(b, 1)),
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => format!(
            "match {} {{ case some({var}) => {}, case none => {} }}",
            expr(subject),
            expr(some),
            expr(none)
        ),
    }
}
