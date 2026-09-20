//! Source-preserving formatting. @ref LLP 1035.005 D1.
//!
//! The parser identifies safe attribute/argument breaks. The lexer's exact
//! ranges preserve literal spelling, comments and opaque `contract` bodies;
//! no second literal scanner or expression printer is involved. Existing
//! physical breaks remain, including comments and blank groups at file edges.

use crate::{parse, Attr, File, Lexer, Node, Span, SyntaxError, Token, TokenKind, TypeExpr};
use std::collections::{BTreeMap, BTreeSet};

/// The preferred width; indivisible literals and comments may exceed it.
pub const WIDTH: usize = 100;

/// Format valid Contract source without changing literal spelling or order.
/// Formatting is explicit: no compiler or development loop calls this.
pub fn format(src: &str) -> Result<String, SyntaxError> {
    let file = parse(src)?;
    let tokens = Lexer::tokenize(src, 1)?;
    let lines: Vec<&str> = src.lines().collect();
    let mut layout = Layout::new(&tokens, &lines);
    layout.file(&file);
    let out = layout.render();
    // Spacing must never turn subtraction into a hyphenated name, join
    // punctuation, or alter a literal. Reparse the newly introduced breaks.
    let after = Lexer::tokenize(&out, 1)?;
    let after_lines: Vec<&str> = out.lines().collect();
    let original = tokens
        .iter()
        .filter(|t| ordinary(t))
        .map(|t| text(t, &lines));
    let formatted = after
        .iter()
        .filter(|t| ordinary(t))
        .map(|t| text(t, &after_lines));
    if !original.eq(formatted) {
        return Err(SyntaxError {
            id: "fmt-token-change",
            message: "formatting would change a source token; source was not written".into(),
            span: Span::point(1, 1),
        });
    }
    parse(&out)?;
    Ok(out)
}

fn ordinary(t: &Token) -> bool {
    !matches!(
        t.kind,
        TokenKind::Indent | TokenKind::Dedent | TokenKind::Newline | TokenKind::Eof
    )
}

fn text<'a>(t: &Token, lines: &[&'a str]) -> &'a str {
    &lines[t.span.line as usize - 1][t.span.col as usize - 1..t.span.end_col as usize - 1]
}

struct Layout<'a> {
    tokens: &'a [Token],
    lines: &'a [&'a str],
    by_line: Vec<Vec<usize>>,
    indents: Vec<usize>,
    levels: Vec<usize>,
    positions: BTreeMap<Span, usize>,
    attributes: BTreeSet<Span>,
    type_angles: BTreeSet<Span>,
    breaks: BTreeMap<Span, usize>,
    opaque: BTreeMap<usize, usize>,
}

impl<'a> Layout<'a> {
    fn new(tokens: &'a [Token], lines: &'a [&'a str]) -> Self {
        let mut by_line = vec![Vec::new(); lines.len()];
        let mut indents = vec![0; lines.len()];
        let mut levels = vec![0; lines.len()];
        let mut positions = BTreeMap::new();
        let mut depth: usize = 0;
        let mut brackets: usize = 0;
        let mut widths: BTreeMap<usize, Vec<(usize, usize)>> = BTreeMap::new();
        for (i, t) in tokens.iter().enumerate() {
            match &t.kind {
                TokenKind::Indent => depth += 1,
                TokenKind::Dedent => depth = depth.saturating_sub(1),
                _ if ordinary(t) => {
                    let line = t.span.line as usize - 1;
                    let closing = matches!(t.kind, TokenKind::Punct(")" | "]" | "}"));
                    if by_line[line].is_empty() {
                        let leading_closes = tokens[i..]
                            .iter()
                            .take_while(|next| {
                                next.span.line == t.span.line
                                    && matches!(next.kind, TokenKind::Punct(")" | "]" | "}"))
                            })
                            .count();
                        levels[line] = depth;
                        indents[line] = depth + brackets.saturating_sub(leading_closes);
                        let width = lines[line].len() - lines[line].trim_start().len();
                        widths.entry(width).or_default().push((line, indents[line]));
                    }
                    by_line[line].push(i);
                    positions.insert(t.span, i);
                    if matches!(t.kind, TokenKind::Punct("(" | "[" | "{")) {
                        brackets += 1;
                    } else if closing {
                        brackets = brackets.saturating_sub(1);
                    }
                }
                _ => {}
            }
        }
        // Comments do not participate in indentation grammar. Keep them at
        // the level of the nearest code line with their original indentation.
        for (line, raw) in lines.iter().enumerate() {
            if by_line[line].is_empty() && !raw.trim().is_empty() {
                let width = raw.len() - raw.trim_start().len();
                indents[line] = width / 2;
                if let Some(nearby) = widths.get(&width) {
                    let at = nearby.partition_point(|(l, _)| *l < line);
                    let candidates =
                        nearby[at.saturating_sub(1)..(at + 1).min(nearby.len())].iter();
                    if let Some((_, indent)) = candidates.min_by_key(|(l, _)| l.abs_diff(line)) {
                        indents[line] = *indent;
                    }
                }
            }
        }
        let mut opaque = BTreeMap::new();
        let mut contract_indent = None;
        for (line, indices) in by_line.iter().enumerate() {
            if let Some(&first) = indices.first() {
                let source_indent = tokens[first].span.col as usize - 1;
                if contract_indent.is_some_and(|width| source_indent <= width) {
                    contract_indent = None;
                }
                if indents[line] == 1 && text(&tokens[first], lines) == "contract" {
                    contract_indent = Some(source_indent);
                    continue;
                }
            }
            if let Some(width) = contract_indent {
                let leading = lines[line].len() - lines[line].trim_start().len();
                if leading > width {
                    opaque.insert(line, width);
                }
            }
        }
        Self {
            tokens,
            lines,
            by_line,
            indents,
            levels,
            positions,
            attributes: BTreeSet::new(),
            type_angles: BTreeSet::new(),
            breaks: BTreeMap::new(),
            opaque,
        }
    }

    fn file(&mut self, file: &File) {
        for field in file.shapes.iter().flat_map(|s| &s.fields) {
            self.ty(&field.ty);
        }
        for function in &file.fns {
            self.ty(&function.ret);
            for ty in function.params.iter().filter_map(|p| p.ty.as_ref()) {
                self.ty(ty);
            }
        }
        for style in &file.styles {
            self.attributes.extend(style.attrs.iter().map(|a| a.span));
        }
        for component in &file.components {
            for ty in component
                .props
                .iter()
                .chain(&component.injects)
                .chain(component.actions.iter().flat_map(|a| &a.params))
                .filter_map(|p| p.ty.as_ref())
            {
                self.ty(ty);
            }
            for resource in &component.resources {
                self.ty(&resource.shape);
            }
            for mutation in &component.mutations {
                self.ty(&mutation.shape);
            }
            self.nodes(&component.view);
        }
    }

    fn ty(&mut self, ty: &TypeExpr) -> usize {
        let start = self.positions[&ty.span()];
        match ty {
            TypeExpr::Named(..) => start + 1,
            TypeExpr::List(inner, _) | TypeExpr::Option(inner, _) => {
                let end = self.ty(inner);
                self.type_angles.insert(self.tokens[start + 1].span);
                self.type_angles.insert(self.tokens[end].span);
                end + 1
            }
        }
    }

    fn nodes(&mut self, nodes: &[Node]) {
        for node in nodes {
            match node {
                Node::Element {
                    tag,
                    positional,
                    attrs,
                    children,
                    span,
                } => {
                    // The button's normalized text child has its parent's
                    // span, but no corresponding source tag of its own.
                    if self
                        .positions
                        .get(span)
                        .is_some_and(|i| text(&self.tokens[*i], self.lines) == tag)
                    {
                        let mut last_positional = positional.iter().map(|e| e.span()).max();
                        if tag == "button" {
                            if let Some(Node::Element {
                                positional,
                                span: child_span,
                                ..
                            }) = children.first()
                            {
                                if child_span == span {
                                    last_positional = positional.iter().map(|e| e.span()).max();
                                }
                            }
                        }
                        self.header(*span, attrs, false, last_positional);
                    }
                    self.nodes(children);
                }
                Node::Use {
                    args,
                    children,
                    span,
                    ..
                } => {
                    self.header(*span, args, true, None);
                    self.nodes(children);
                }
                Node::Provide { body, .. } | Node::Each { body, .. } => self.nodes(body),
                Node::When {
                    then, otherwise, ..
                } => {
                    self.nodes(then);
                    self.nodes(otherwise);
                }
                Node::Match { some, none, .. } => {
                    self.nodes(&some.1);
                    self.nodes(none);
                }
                Node::Children { .. } => {}
            }
        }
    }

    fn header(
        &mut self,
        span: Span,
        attrs: &[Attr],
        component: bool,
        last_positional: Option<Span>,
    ) {
        self.attributes.extend(attrs.iter().map(|a| a.span));
        let Some(last) = attrs.last() else { return };
        let start = self.positions[&span];
        let mut end = start;
        let mut brackets = 0usize;
        for (i, t) in self.tokens.iter().enumerate().skip(start) {
            if matches!(t.kind, TokenKind::Newline | TokenKind::Eof)
                && t.span.line >= last.span.line
            {
                end = i;
                break;
            }
            if matches!(t.kind, TokenKind::Punct("(" | "[" | "{")) {
                brackets += 1;
            } else if matches!(t.kind, TokenKind::Punct(")" | "]" | "}")) {
                brackets = brackets.saturating_sub(1);
                if component && brackets == 0 {
                    end = i + 1;
                    break;
                }
            }
        }
        let indent = self.indents[span.line as usize - 1];
        let mut one = String::new();
        let mut previous = None;
        for t in self.tokens[start..end].iter().filter(|t| ordinary(t)) {
            self.append(&mut one, previous, t);
            previous = Some(t);
        }
        if indent * 2 + one.chars().count() > WIDTH {
            for (at, a) in attrs.iter().enumerate().skip(usize::from(!component)) {
                // Positional expressions are legal anywhere on the element's
                // head, but not on an attribute-only continuation. Keep that
                // prefix intact, including the normalized button label.
                if last_positional.is_some_and(|last| a.span < last) {
                    continue;
                }
                self.breaks.insert(a.span, indent + 1);
                if !component {
                    // Moving an attribute off the element's head creates a
                    // structural continuation level. Its existing multiline
                    // expression (and comments inside it) moves with it.
                    let line = a.span.line as usize - 1;
                    let delta = (indent + 1).saturating_sub(self.levels[line]);
                    let until = attrs
                        .get(at + 1)
                        .map_or(end, |next| self.positions[&next.span]);
                    let last_line = self.tokens[..until]
                        .iter()
                        .rev()
                        .find(|t| ordinary(t))
                        .unwrap()
                        .span
                        .line as usize
                        - 1;
                    for following in line + 1..=last_line {
                        self.indents[following] += delta;
                    }
                }
            }
            if component {
                self.breaks.insert(self.tokens[end - 1].span, indent);
            }
        }
    }

    fn append(&self, out: &mut String, previous: Option<&Token>, token: &Token) {
        if let Some(previous) = previous {
            let a = text(previous, self.lines);
            let b = text(token, self.lines);
            let attr_equals = b == "=" && self.attributes.contains(&previous.span);
            let after_attr_equals = a == "="
                && self
                    .positions
                    .range(..previous.span)
                    .next_back()
                    .is_some_and(|(span, _)| self.attributes.contains(span));
            let tight = attr_equals
                || after_attr_equals
                || self.type_angles.contains(&token.span)
                || (a == "<" && self.type_angles.contains(&previous.span))
                || matches!(b, ")" | "]" | ",")
                || matches!(a, "(" | "[")
                || a == "."
                || (b == "." && !matches!(previous.kind, TokenKind::Number(_)))
                || (b == "("
                    && matches!(previous.kind, TokenKind::Ident(_))
                    && !matches!(a, "when" | "if" | "match" | "not" | "and" | "or" | "in"))
                || b == ":";
            if !tight {
                out.push(' ');
            }
        }
        out.push_str(text(token, self.lines));
    }

    fn render(&self) -> String {
        let mut out = String::new();
        for (line, raw) in self.lines.iter().enumerate() {
            if raw.trim().is_empty() {
                out.push('\n');
                continue;
            }
            if let Some(width) = self.opaque.get(&line) {
                let leading = raw.len() - raw.trim_start().len();
                out.push_str(&" ".repeat(2 + leading.saturating_sub(*width)));
                out.push_str(raw.trim_start());
                out.push('\n');
                continue;
            }
            let mut printed = String::new();
            let mut previous = None;
            let mut indent = self.indents[line];
            for &i in &self.by_line[line] {
                let t = &self.tokens[i];
                if let Some(&level) = self.breaks.get(&t.span) {
                    if !printed.is_empty() {
                        out.push_str(&"  ".repeat(indent));
                        out.push_str(&printed);
                        out.push('\n');
                        printed.clear();
                    }
                    indent = level;
                    previous = None;
                }
                self.append(&mut printed, previous, t);
                previous = Some(t);
            }
            let suffix = if let Some(last) = previous {
                &raw[last.span.end_col as usize - 1..]
            } else {
                raw
            };
            let comment = suffix.trim_start();
            if !comment.is_empty() {
                if !printed.is_empty() {
                    printed.push(' ');
                }
                printed.push_str(comment);
            }
            out.push_str(&"  ".repeat(indent));
            out.push_str(&printed);
            out.push('\n');
        }
        out
    }
}
