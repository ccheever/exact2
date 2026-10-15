//! Edit operations (LLP 1121 D13, stage 3): each takes a design's text and
//! gives the new text and its **inverse**, the operations that give the
//! old text back. An edit splices the lines it touches, in `fmt`'s layout,
//! and leaves every other line as it was, so on a formatted design it is a
//! one-line diff (two for a token with `when` lines removed or added whole)
//! and its inverse restores the bytes. Where an edit creates or removes
//! something, the inverse restores the preimage: removing a `when` line
//! returns a `set-token` that puts it back where it was, with its trailing
//! comment.
//!
//! An edit that leaves the design refused by the checker is refused, with
//! the checker's diagnostics; so is one naming a dial, token, `when` or axis
//! value the design does not have. Stage 3 has the operations that touch only
//! `.tamber` files: params (stage 7), state parts (stage 8) and rows (LLP
//! 1122 T5) come later.

use crate::lex::{self, Line};
use crate::syntax::Design;
use crate::{Diagnostic, Vocab};

/// A `when` line an `add-token` writes: its conditions (`axis`, `value`),
/// its value, and its trailing comment.
pub type WhenLine = (Vec<(String, String)>, String, Option<String>);

/// One operation (D13's table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// `set-dial name value`.
    SetDial {
        /// The dial.
        name: String,
        /// Its new value, CSS text.
        value: String,
    },
    /// `set-token name [when …] value`: the default, or one `when` line,
    /// added where there is none.
    SetToken {
        /// The token.
        name: String,
        /// The `when` line's conditions (`axis`, `value`); none is the default.
        when: Vec<(String, String)>,
        /// The value, CSS text.
        value: String,
        /// Where a new `when` line goes among the token's (by index); none
        /// is after the last. An inverse names it.
        at: Option<usize>,
        /// A new `when` line's trailing comment, as written (`// …`).
        comment: Option<String>,
    },
    /// `unset-token-when name when…`: remove one `when` line.
    UnsetTokenWhen {
        /// The token.
        name: String,
        /// The line's conditions.
        when: Vec<(String, String)>,
    },
    /// `add-token`: a new token, before `before` (else after the last).
    AddToken {
        /// Its name.
        name: String,
        /// Its kind, when written (`color`).
        kind: Option<String>,
        /// Its default value.
        value: String,
        /// Its metadata (`scheme=fixed`), as written.
        meta: Vec<String>,
        /// Its `when` lines: conditions, value, trailing comment.
        whens: Vec<WhenLine>,
        /// Its line's trailing comment.
        comment: Option<String>,
        /// The token it goes before; none is after the last.
        before: Option<String>,
    },
    /// `rename-token from to`: the declaration and every use in the design.
    RenameToken {
        /// The name now.
        from: String,
        /// The new name.
        to: String,
    },
    /// `remove-token name`, with its `when` lines.
    RemoveToken {
        /// The token.
        name: String,
    },
    /// `add-axis-value axis value`, before `before` (else last).
    AddAxisValue {
        /// The axis.
        axis: String,
        /// The value.
        value: String,
        /// The value it goes before; none is last.
        before: Option<String>,
    },
    /// `remove-axis-value axis value`.
    RemoveAxisValue {
        /// The axis.
        axis: String,
        /// The value.
        value: String,
    },
}

/// An edit's result: the new text, and the operations that undo it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edited {
    /// The design after the edit.
    pub text: String,
    /// Applied in order to `text`, they give the text before.
    pub inverse: Vec<Op>,
}

/// Apply `op` to the design `src`, checked against `vocab`.
pub fn apply(src: &str, op: &Op, vocab: &Vocab) -> Result<Edited, Vec<Diagnostic>> {
    let design = crate::parse(src)?;
    let mut doc = Doc::new(src);
    let inverse = doc.apply(&design, op)?;
    let text = doc.text();
    crate::check(&text, vocab)?;
    Ok(Edited { text, inverse })
}

/// Apply `ops` in order: a transaction's edits to one file. Its inverse is
/// theirs, last first.
pub fn apply_all(src: &str, ops: &[Op], vocab: &Vocab) -> Result<Edited, Vec<Diagnostic>> {
    let mut text = src.to_string();
    let mut inverse = Vec::new();
    for op in ops {
        let design = crate::parse(&text)?;
        let mut doc = Doc::new(&text);
        let mut undo = doc.apply(&design, op)?;
        text = doc.text();
        undo.extend(inverse);
        inverse = undo;
    }
    crate::check(&text, vocab)?;
    Ok(Edited { text, inverse })
}

fn refuse(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::new("tamber-edit", message, 1, 1)]
}

/// The design's lines, which an edit splices.
struct Doc {
    lines: Vec<String>,
    /// Whether the text ended with a newline.
    newline: bool,
}

impl Doc {
    fn new(src: &str) -> Doc {
        let newline = src.ends_with('\n');
        let body = src.strip_suffix('\n').unwrap_or(src);
        Doc {
            lines: body.split('\n').map(str::to_string).collect(),
            newline,
        }
    }

    fn text(&self) -> String {
        let mut out = self.lines.join("\n");
        if self.newline {
            out.push('\n');
        }
        out
    }

    /// The line `n` (1-based) split into its indent, code and comment.
    fn parts(&self, n: u32) -> (usize, String, Option<String>) {
        let raw = self.lines[n as usize - 1].trim_end_matches('\r');
        let (body, comment) = lex::split_comment(raw);
        let indent = body.len() - body.trim_start_matches(' ').len();
        (
            indent,
            body.trim().to_string(),
            comment.map(|c| c.trim_end().to_string()),
        )
    }

    fn set(&mut self, n: u32, indent: usize, code: &str, comment: Option<&str>) {
        self.lines[n as usize - 1] = line(indent, code, comment);
    }

    fn apply(&mut self, design: &Design, op: &Op) -> Result<Vec<Op>, Vec<Diagnostic>> {
        match op {
            Op::SetDial { name, value } => {
                let d = design
                    .dials
                    .iter()
                    .find(|d| d.name.value == *name)
                    .ok_or_else(|| refuse(format!("no dial `{name}`")))?;
                let (indent, code, comment) = self.parts(d.name.line);
                let line = Line {
                    indent,
                    text: code.clone(),
                    line: d.name.line,
                    col: 1,
                };
                let old = d.value.value.clone();
                let new = crate::fmt::dial_with(&line, value.trim());
                self.set(d.name.line, indent, &new, comment.as_deref());
                Ok(vec![Op::SetDial {
                    name: name.clone(),
                    value: old,
                }])
            }
            Op::SetToken {
                name,
                when,
                value,
                at,
                comment,
            } => {
                let i = token_index(design, name)?;
                let t = &design.tokens[i];
                if when.is_empty() {
                    let (indent, code, trailing) = self.parts(t.name.line);
                    let line = Line {
                        indent,
                        text: code,
                        line: t.name.line,
                        col: 1,
                    };
                    let new = crate::fmt::token_with(&line, value.trim());
                    self.set(t.name.line, indent, &new, trailing.as_deref());
                    return Ok(vec![Op::SetToken {
                        name: name.clone(),
                        when: Vec::new(),
                        value: t.value.value.clone(),
                        at: None,
                        comment: None,
                    }]);
                }
                let code = when_code(when, value.trim());
                if let Some(w) = t.whens.iter().find(|w| same_conds(&w.conds, when)) {
                    let (indent, _, trailing) = self.parts(w.line);
                    self.set(w.line, indent, &code, trailing.as_deref());
                    return Ok(vec![Op::SetToken {
                        name: name.clone(),
                        when: when.clone(),
                        value: w.value.value.clone(),
                        at: None,
                        comment: None,
                    }]);
                }
                // A new `when` line: among the token's, at its siblings' indent.
                let indent = match t.whens.first() {
                    Some(w) => self.parts(w.line).0,
                    None => self.parts(t.name.line).0 + 2,
                };
                let index = at.unwrap_or(t.whens.len()).min(t.whens.len());
                let after = match index {
                    0 => t.name.line,
                    k => t.whens[k - 1].line,
                };
                self.lines
                    .insert(after as usize, line(indent, &code, comment.as_deref()));
                Ok(vec![Op::UnsetTokenWhen {
                    name: name.clone(),
                    when: when.clone(),
                }])
            }
            Op::UnsetTokenWhen { name, when } => {
                let t = &design.tokens[token_index(design, name)?];
                let (k, w) = t
                    .whens
                    .iter()
                    .enumerate()
                    .find(|(_, w)| same_conds(&w.conds, when))
                    .ok_or_else(|| refuse(format!("`{name}` has no such `when` line")))?;
                let (_, _, trailing) = self.parts(w.line);
                self.lines.remove(w.line as usize - 1);
                Ok(vec![Op::SetToken {
                    name: name.clone(),
                    when: w.conds.clone(),
                    value: w.value.value.clone(),
                    at: Some(k),
                    comment: trailing,
                }])
            }
            Op::AddToken {
                name,
                kind,
                value,
                meta,
                whens,
                comment,
                before,
            } => {
                if design.tokens.iter().any(|t| t.name.value == *name) {
                    return Err(refuse(format!("a token `{name}` is declared already")));
                }
                let indent = design
                    .tokens
                    .first()
                    .map_or(2, |t| self.parts(t.name.line).0);
                let when_indent = design
                    .tokens
                    .iter()
                    .find_map(|t| t.whens.first())
                    .map_or(indent + 2, |w| self.parts(w.line).0);
                let mut code = match kind {
                    Some(k) => format!("{name}: {k} = {}", value.trim()),
                    None => format!("{name} = {}", value.trim()),
                };
                if !meta.is_empty() {
                    code.push_str("  ");
                    code.push_str(&meta.join(" "));
                }
                let mut new = vec![line(indent, &code, comment.as_deref())];
                for (conds, v, c) in whens {
                    new.push(line(when_indent, &when_code(conds, v.trim()), c.as_deref()));
                }
                let at = match before {
                    Some(b) => {
                        let t = &design.tokens[token_index(design, b)?];
                        t.name.line as usize - 1
                    }
                    // After the block's last line, a comment closing it too;
                    // a design with no `theme` gains one, as fmt lays it out.
                    None => match self.block_end("theme") {
                        Some(end) => end,
                        None => {
                            while self.lines.last().is_some_and(|l| l.trim().is_empty()) {
                                self.lines.pop();
                            }
                            self.lines.push(String::new());
                            self.lines.push("theme".into());
                            self.newline = true;
                            self.lines.len()
                        }
                    },
                };
                for (k, l) in new.into_iter().enumerate() {
                    self.lines.insert(at + k, l);
                }
                Ok(vec![Op::RemoveToken { name: name.clone() }])
            }
            Op::RemoveToken { name } => {
                let i = token_index(design, name)?;
                let t = &design.tokens[i];
                let (_, _, comment) = self.parts(t.name.line);
                let whens = t
                    .whens
                    .iter()
                    .map(|w| (w.conds.clone(), w.value.value.clone(), self.parts(w.line).2))
                    .collect();
                let last = t.whens.last().map_or(t.name.line, |w| w.line);
                self.lines.drain(t.name.line as usize - 1..last as usize);
                Ok(vec![Op::AddToken {
                    name: t.name.value.clone(),
                    kind: t.kind.clone(),
                    value: t.value.value.clone(),
                    meta: t.meta.iter().map(|(k, v)| meta_word(k, v)).collect(),
                    whens,
                    comment,
                    before: design.tokens.get(i + 1).map(|n| n.name.value.clone()),
                }])
            }
            Op::RenameToken { from, to } => {
                token_index(design, from)?;
                if !lex::is_name(to) {
                    return Err(refuse(format!(
                        "`{to}`: a token's name is identifiers joined by `.`"
                    )));
                }
                if design.tokens.iter().any(|t| t.name.value == *to) {
                    return Err(refuse(format!("a token `{to}` is declared already")));
                }
                // A token is named in the theme's values and the exports.
                let mut block = String::new();
                for n in 0..self.lines.len() {
                    let raw = self.lines[n].clone();
                    let (body, comment) = lex::split_comment(&raw);
                    if !body.starts_with(' ') && !body.trim().is_empty() {
                        block = body.trim().to_string();
                        continue;
                    }
                    if block != "theme" && block != "exports" {
                        continue;
                    }
                    let renamed = rename_words(body, from, to);
                    if renamed != body {
                        self.lines[n] = format!("{renamed}{}", comment.unwrap_or(""));
                    }
                }
                Ok(vec![Op::RenameToken {
                    from: to.clone(),
                    to: from.clone(),
                }])
            }
            Op::AddAxisValue {
                axis,
                value,
                before,
            } => {
                let a = design
                    .axes
                    .iter()
                    .find(|a| a.name.value == *axis)
                    .ok_or_else(|| refuse(format!("no axis `{axis}`")))?;
                if a.values.contains(value) {
                    return Err(refuse(format!("`{axis}` has a value `{value}` already")));
                }
                let mut values = a.values.clone();
                let at = match before {
                    Some(b) => values
                        .iter()
                        .position(|v| v == b)
                        .ok_or_else(|| refuse(format!("`{axis}` has no value `{b}`")))?,
                    None => values.len(),
                };
                values.insert(at, value.clone());
                self.set_axis(a, &values);
                Ok(vec![Op::RemoveAxisValue {
                    axis: axis.clone(),
                    value: value.clone(),
                }])
            }
            Op::RemoveAxisValue { axis, value } => {
                let a = design
                    .axes
                    .iter()
                    .find(|a| a.name.value == *axis)
                    .ok_or_else(|| refuse(format!("no axis `{axis}`")))?;
                let at = a
                    .values
                    .iter()
                    .position(|v| v == value)
                    .ok_or_else(|| refuse(format!("`{axis}` has no value `{value}`")))?;
                let mut values = a.values.clone();
                values.remove(at);
                self.set_axis(a, &values);
                Ok(vec![Op::AddAxisValue {
                    axis: axis.clone(),
                    value: value.clone(),
                    before: a.values.get(at + 1).cloned(),
                }])
            }
        }
    }

    /// The index after block `name`'s last line that is not blank (its
    /// entries and the comments inside it), or `None` without the block.
    fn block_end(&self, name: &str) -> Option<usize> {
        let head = self
            .lines
            .iter()
            .position(|l| lex::split_comment(l).0.trim_end() == name)?;
        let mut end = head + 1;
        for (i, l) in self.lines.iter().enumerate().skip(head + 1) {
            // The next block, or a top-level comment opening what follows.
            if !l.trim().is_empty() && !l.starts_with(' ') {
                break;
            }
            if !l.trim().is_empty() {
                end = i + 1;
            }
        }
        Some(end)
    }

    fn set_axis(&mut self, a: &crate::syntax::Axis, values: &[String]) {
        let (indent, _, comment) = self.parts(a.name.line);
        let mut code = format!("{} = {}", a.name.value, values.join(" | "));
        if a.under_dark {
            code.push_str("  under scheme=dark");
        }
        self.set(a.name.line, indent, &code, comment.as_deref());
    }
}

/// A line: its indent, its code and its trailing comment, as fmt writes them.
fn line(indent: usize, code: &str, comment: Option<&str>) -> String {
    let mut out = " ".repeat(indent);
    out.push_str(code);
    if let Some(c) = comment {
        out.push_str("  ");
        out.push_str(c);
    }
    out
}

fn token_index(design: &Design, name: &str) -> Result<usize, Vec<Diagnostic>> {
    design
        .tokens
        .iter()
        .position(|t| t.name.value == name)
        .ok_or_else(|| refuse(format!("no token `{name}`")))
}

/// `when a=b c=d: value`.
fn when_code(conds: &[(String, String)], value: &str) -> String {
    let conds: Vec<String> = conds.iter().map(|(a, v)| format!("{a}={v}")).collect();
    format!("when {}: {value}", conds.join(" "))
}

/// The same conditions, in any order.
fn same_conds(a: &[(String, String)], b: &[(String, String)]) -> bool {
    a.len() == b.len() && a.iter().all(|c| b.contains(c))
}

/// A metadata word as a token line writes it, quoting a value that needs it.
fn meta_word(k: &str, v: &str) -> String {
    match v
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_.%#".contains(c))
    {
        true => format!("{k}={v}"),
        false => format!("{k}=\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"")),
    }
}

/// `text` with each whole use of the name `from` as `to`: a name is whole
/// where no name character (a letter, digit, `-`, `_` or `.`) touches it,
/// and a quoted string is left alone.
fn rename_words(text: &str, from: &str, to: &str) -> String {
    let name_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
    let mut out = String::new();
    let mut rest = text;
    let mut quote: Option<char> = None;
    while let Some(c) = rest.chars().next() {
        if let Some(q) = quote {
            out.push(c);
            rest = &rest[c.len_utf8()..];
            if c == q {
                quote = None;
            }
            continue;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
            out.push(c);
            rest = &rest[1..];
            continue;
        }
        let before_ok = out.chars().next_back().is_none_or(|p| !name_char(p));
        if before_ok
            && rest.starts_with(from)
            && rest[from.len()..]
                .chars()
                .next()
                .is_none_or(|n| !name_char(n))
        {
            out.push_str(to);
            rest = &rest[from.len()..];
            continue;
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}
