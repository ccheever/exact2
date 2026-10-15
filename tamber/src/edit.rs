//! Edit operations (LLP 1121 D13, stage 3): each takes a design's text and
//! gives the new text and its **inverse**. An edit splices the lines it
//! touches, in `fmt`'s layout, and leaves every other line as it was, so on a
//! formatted design it is a one-line diff (a token's `when` lines with it
//! when it comes or goes whole). The inverse is a preimage restoration: a
//! `restore` of exactly the lines the edit changed, as they were (comments,
//! blank lines, line endings, a block it created), so it gives the bytes back
//! on any design.
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
    /// An inverse: lines `line` (1-based) to `line + remove - 1` replaced by
    /// `lines`, as they were, and whether the text ends with a newline. Only
    /// [`undo`] takes it, and a server takes it only as an inverse it issued
    /// for the file as it is: [`apply`] refuses it.
    Restore {
        /// The first line replaced.
        line: u32,
        /// How many lines are replaced.
        remove: u32,
        /// The lines put back, each without its `\n` (a `\r` kept).
        lines: Vec<String>,
        /// Whether the text ends with a newline.
        newline: bool,
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
    apply_all(src, std::slice::from_ref(op), vocab)
}

/// Apply an inverse an edit returned (its `restore`s) to the text that edit
/// left, checked against `vocab`: the text before it, and its own inverse.
pub fn undo(src: &str, inverse: &[Op], vocab: &Vocab) -> Result<Edited, Vec<Diagnostic>> {
    if inverse.iter().any(|op| !matches!(op, Op::Restore { .. })) {
        return Err(refuse("an inverse is the restores an edit returned"));
    }
    // The preimage as it was, even a design that did not check.
    let mut text = src.to_string();
    let mut undone = Vec::new();
    for op in inverse {
        let (next, back) = step(&text, op, vocab)?;
        text = next;
        undone.insert(0, back);
    }
    Ok(Edited {
        text,
        inverse: undone,
    })
}

/// One edit, unchecked: the new text and the `restore` that undoes it.
fn step(src: &str, op: &Op, vocab: &Vocab) -> Result<(String, Op), Vec<Diagnostic>> {
    valid(op)?;
    let design = crate::parse(src)?;
    if let Op::RenameToken { from, to } = op {
        // A word colour syntax reads (a channel, a space, a hue method) is
        // never renamed from or to, whatever it folds to now.
        for n in [from, to] {
            if KEYWORDS.contains(&n.to_ascii_lowercase().as_str()) {
                return Err(refuse(format!(
                    "`{n}` is a word colour syntax reads: rename it by hand"
                )));
            }
        }
        // A role override keeps its role's name, which rows spell three ways:
        // a name the schema's roles have, whether or not the design checks.
        let roles: Vec<(String, bool)> = vocab
            .roles
            .iter()
            .map(|r| (r.name.clone(), r.system))
            .collect();
        if role_key(&roles, from).is_some() {
            let line = token_index(&design, &roles, from).map_or(1, |i| design.tokens[i].name.line);
            return Err(refuse_at(
                line,
                format!("`{from}` overrides a role: a role keeps its name"),
            ));
        }
    }
    let before = Doc::new(src);
    let mut doc = Doc::new(src);
    doc.roles = vocab
        .roles
        .iter()
        .map(|r| (r.name.clone(), r.system))
        .collect();
    doc.apply(&design, op)?;
    let (a, b) = (&before.lines, &doc.lines);
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    // A rename names, and folds nothing else differently: a word it changed
    // that was no use (a channel, a colour space, a platform selector) would.
    if let Op::RenameToken { .. } = op {
        let folded = |text: &str| {
            crate::check(text, vocab)
                .ok()
                .map(|f| f.tokens.into_iter().map(|t| t.values).collect::<Vec<_>>())
        };
        // Proved only where both sides check and fold the same.
        match (folded(src), folded(&doc.text())) {
            (Some(was), Some(now)) if was == now => {}
            (Some(_), Some(_)) => {
                return Err(refuse("the rename would change what a value folds to (a word it reads that is no token): rename it by hand"))
            }
            _ => return Err(refuse("a rename needs a design that checks before and after it")),
        }
    }
    let undo = Op::Restore {
        line: prefix as u32 + 1,
        remove: (b.len() - prefix - suffix) as u32,
        lines: a[prefix..a.len() - suffix].to_vec(),
        newline: before.newline,
    };
    Ok((doc.text(), undo))
}

/// An operation's fields, each its own kind: a value on one line, names and
/// conditions as the grammar has them, a comment that is one, so a field
/// cannot write a second line or name.
fn valid(op: &Op) -> Result<(), Vec<Diagnostic>> {
    let one_line = |field: &str, v: &str| match v.contains(['\n', '\r']) {
        true => Err(refuse(format!("`{field}` is one line"))),
        false => Ok(()),
    };
    // A value is a value: no comment after it, no metadata or range clause,
    // which the line keeps from before.
    let value = |v: &str| {
        one_line("value", v)?;
        let words = lex::words(v.trim());
        let clause = words.last().is_some_and(|(_, w)| {
            w.split_once('=')
                .is_some_and(|(k, x)| lex::is_ident(k) && !x.is_empty())
        }) || words.iter().any(|(_, w)| *w == "in");
        if lex::split_comment(v).1.is_some() || clause || v.trim().is_empty() {
            return Err(refuse(format!(
                "`{v}`: a value alone, with no comment, metadata or range after it"
            )));
        }
        Ok(())
    };
    let name = |v: &str| match lex::is_name(v) || crate::syntax::role_spelling(v).is_some() {
        true => Ok(()),
        false => Err(refuse(format!(
            "`{v}`: a token's name is identifiers joined by `.`, or a role's"
        ))),
    };
    let ident = |v: &str| match lex::is_ident(v) {
        true => Ok(()),
        false => Err(refuse(format!(
            "`{v}`: an axis and its values are identifiers"
        ))),
    };
    let conds = |c: &[(String, String)]| {
        if c.iter()
            .enumerate()
            .any(|(i, (a, _))| c[..i].iter().any(|(b, _)| b == a))
        {
            return Err(refuse("a `when` names each axis once"));
        }
        c.iter()
            .try_for_each(|(a, v)| ident(a).and_then(|_| ident(v)))
    };
    let comment = |c: &Option<String>| match c {
        Some(c) if !c.starts_with("//") || c.contains(['\n', '\r']) => {
            Err(refuse("a comment is one line starting `//`"))
        }
        _ => Ok(()),
    };
    match op {
        Op::SetDial { name: n, value: v } => ident(n).and_then(|_| value(v)),
        Op::SetToken {
            name: n,
            when,
            value: v,
            comment: c,
            ..
        } => {
            name(n)?;
            conds(when)?;
            comment(c)?;
            value(v)
        }
        Op::UnsetTokenWhen { name: n, when } => name(n).and_then(|_| conds(when)),
        Op::AddToken {
            name: n,
            kind,
            value: v,
            meta,
            whens,
            comment: c,
            before,
        } => {
            name(n)?;
            if let Some(k) = kind {
                ident(k)?;
            }
            value(v)?;
            comment(c)?;
            // One metadata word each: `name=value`, a quoted value whole.
            for m in meta {
                one_line("meta", m)?;
                let one = lex::words(m).len() == 1;
                let shaped = m.split_once('=').is_some_and(|(k, x)| {
                    lex::is_ident(k)
                        && !x.is_empty()
                        && (!x.starts_with('"') || lex::unquote(x).is_ok())
                });
                if !one || !shaped || lex::split_comment(m).1.is_some() {
                    return Err(refuse(format!("`{m}`: metadata is one `name=value`")));
                }
            }
            for (w, wv, wc) in whens {
                conds(w)?;
                value(wv)?;
                comment(wc)?;
            }
            before.as_deref().map_or(Ok(()), name)
        }
        Op::RenameToken { from, to } => name(from).and_then(|_| name(to)),
        Op::RemoveToken { name: n } => name(n),
        Op::AddAxisValue {
            axis,
            value,
            before,
        } => {
            ident(axis)?;
            ident(value)?;
            before.as_deref().map_or(Ok(()), ident)
        }
        Op::RemoveAxisValue { axis, value } => ident(axis).and_then(|_| ident(value)),
        // A restored line keeps its `\r`.
        Op::Restore { lines, .. } => lines
            .iter()
            .try_for_each(|l| one_line("lines", l.strip_suffix('\r').unwrap_or(l))),
    }
}

/// Apply `ops` in order: a transaction's edits to one file. Its inverse is
/// theirs, last first.
pub fn apply_all(src: &str, ops: &[Op], vocab: &Vocab) -> Result<Edited, Vec<Diagnostic>> {
    if ops.iter().any(|op| matches!(op, Op::Restore { .. })) {
        return Err(refuse("a restore is an inverse: undo applies it"));
    }
    run(src, ops, vocab)
}

fn run(src: &str, ops: &[Op], vocab: &Vocab) -> Result<Edited, Vec<Diagnostic>> {
    let mut text = src.to_string();
    let mut inverse = Vec::new();
    for op in ops {
        let (next, undo) = step(&text, op, vocab)?;
        text = next;
        inverse.insert(0, undo);
    }
    crate::check(&text, vocab)?;
    Ok(Edited { text, inverse })
}

/// A refusal with nowhere in the design to point (a name it lacks).
fn refuse(message: impl Into<String>) -> Vec<Diagnostic> {
    refuse_at(1, message)
}

/// A refusal at line `line`, which names what the operation clashes with.
fn refuse_at(line: u32, message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::new("tamber-edit", message, line, 1)]
}

/// Words relative colours, `color()` and `color-mix()` read (channels,
/// spaces, hue methods), which a rename never touches.
const KEYWORDS: &[&str] = &[
    "r",
    "g",
    "b",
    "h",
    "s",
    "l",
    "w",
    "c",
    "a",
    "x",
    "y",
    "z",
    "alpha",
    "from",
    "in",
    "srgb",
    "srgb-linear",
    "display-p3",
    "a98-rgb",
    "prophoto-rgb",
    "rec2020",
    "xyz",
    "xyz-d50",
    "xyz-d65",
    "lab",
    "lch",
    "oklab",
    "oklch",
    "hsl",
    "hwb",
    "shorter",
    "longer",
    "increasing",
    "decreasing",
    "transparent",
    "currentcolor",
];

/// The design's lines, which an edit splices.
struct Doc {
    lines: Vec<String>,
    /// Whether the text ended with a newline.
    newline: bool,
    /// The schema's roles, by their row spelling, which a role override
    /// answers to in any case.
    roles: Vec<(String, bool)>,
}

impl Doc {
    fn new(src: &str) -> Doc {
        let newline = src.ends_with('\n');
        let body = src.strip_suffix('\n').unwrap_or(src);
        Doc {
            lines: body.split('\n').map(str::to_string).collect(),
            newline,
            roles: Vec::new(),
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

    /// A new line, ending as the design's lines do (`\r\n` where they do).
    fn ended(&self, mut l: String) -> String {
        if self.lines.first().is_some_and(|f| f.ends_with('\r')) {
            l.push('\r');
        }
        l
    }

    /// Line `n` rewritten, keeping its `\r` if it had one.
    fn set(&mut self, n: u32, indent: usize, code: &str, comment: Option<&str>) {
        let cr = self.lines[n as usize - 1].ends_with('\r');
        let mut l = line(indent, code, comment);
        if cr {
            l.push('\r');
        }
        self.lines[n as usize - 1] = l;
    }

    fn apply(&mut self, design: &Design, op: &Op) -> Result<Vec<Op>, Vec<Diagnostic>> {
        match op {
            Op::Restore {
                line,
                remove,
                lines,
                newline,
            } => {
                let start = (*line as usize).saturating_sub(1);
                let end = start.checked_add(*remove as usize);
                if *line == 0 || end.is_none_or(|e| e > self.lines.len()) {
                    return Err(refuse("a restore names lines the design does not have"));
                }
                let end = end.unwrap_or(start);
                self.lines.splice(start..end, lines.iter().cloned());
                self.newline = *newline;
                Ok(Vec::new())
            }
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
                let i = token_index(design, &self.roles, name)?;
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
                    // The line's own order of its conditions.
                    let code = when_code(&w.conds, value.trim());
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
                let new = self.ended(line(indent, &code, comment.as_deref()));
                self.lines.insert(after as usize, new);
                Ok(vec![Op::UnsetTokenWhen {
                    name: name.clone(),
                    when: when.clone(),
                }])
            }
            Op::UnsetTokenWhen { name, when } => {
                let t = &design.tokens[token_index(design, &self.roles, name)?];
                let (k, w) = t
                    .whens
                    .iter()
                    .enumerate()
                    .find(|(_, w)| same_conds(&w.conds, when))
                    .ok_or_else(|| {
                        refuse_at(t.name.line, format!("`{name}` has no such `when` line"))
                    })?;
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
                if let Some(t) = design.tokens.iter().find(|t| t.name.value == *name) {
                    return Err(refuse_at(
                        t.name.line,
                        format!("a token `{name}` is declared already"),
                    ));
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
                        let t = &design.tokens[token_index(design, &self.roles, b)?];
                        t.name.line as usize - 1
                    }
                    // After the block's last line, a comment closing it too;
                    // a design with no `theme` gains one, as fmt lays it out.
                    None => match self.block_end("theme").zip(design.tokens.last()) {
                        Some((_, t)) => {
                            // After the last token's last line, and the
                            // indented comments and blanks that close the block.
                            let mut end = t.whens.last().map_or(t.name.line, |w| w.line) as usize;
                            for (i, l) in self.lines.iter().enumerate().skip(end) {
                                if !l.trim().is_empty() && !l.starts_with(' ') {
                                    break;
                                }
                                if !l.trim().is_empty() {
                                    end = i + 1;
                                }
                            }
                            end
                        }
                        None if self.block_end("theme").is_some() => {
                            self.block_end("theme").unwrap_or(0)
                        }
                        None => {
                            while self.lines.last().is_some_and(|l| l.trim().is_empty()) {
                                self.lines.pop();
                            }
                            let (blank, head) =
                                (self.ended(String::new()), self.ended("theme".into()));
                            self.lines.push(blank);
                            self.lines.push(head);
                            self.newline = true;
                            self.lines.len()
                        }
                    },
                };
                for (k, l) in new.into_iter().enumerate() {
                    let l = self.ended(l);
                    self.lines.insert(at + k, l);
                }
                Ok(vec![Op::RemoveToken { name: name.clone() }])
            }
            Op::RemoveToken { name } => {
                let i = token_index(design, &self.roles, name)?;
                let t = &design.tokens[i];
                let (_, _, comment) = self.parts(t.name.line);
                let whens = t
                    .whens
                    .iter()
                    .map(|w| (w.conds.clone(), w.value.value.clone(), self.parts(w.line).2))
                    .collect();
                let last = t.whens.last().map_or(t.name.line, |w| w.line);
                let at = t.name.line as usize - 1;
                self.lines.drain(at..last as usize);
                // fmt's blanks: none opening a block, never two together.
                let blank = |l: Option<&String>| l.is_some_and(|l| l.trim().is_empty());
                let opens = at == 0
                    || blank(self.lines.get(at - 1))
                    || !self.lines[at - 1].starts_with(' ');
                if blank(self.lines.get(at)) && opens && at < self.lines.len() {
                    self.lines.remove(at);
                }
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
                token_index(design, &self.roles, from)?;
                if !lex::is_name(to) {
                    return Err(refuse(format!(
                        "`{to}`: a token's name is identifiers joined by `.`"
                    )));
                }
                if let Some(t) = design.tokens.iter().find(|t| t.name.value == *to) {
                    return Err(refuse_at(
                        t.name.line,
                        format!("a token `{to}` is declared already"),
                    ));
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
                    // A token line's name and value; a when line's value
                    // only (its conditions are axes); an export whole.
                    let code = body.trim_start();
                    let keep = match block.as_str() {
                        "theme"
                            if code.starts_with("when ")
                                && !code[4..].trim_start().starts_with('=') =>
                        {
                            lex::split_top(body, ':').map_or(body.len(), |(c, _)| c.len() + 1)
                        }
                        _ => 0,
                    };
                    let (kept, body) = body.split_at(keep);
                    // A token line's metadata (`scheme=fixed`) is no use.
                    let words = lex::words(body.trim_end());
                    let meta_from = (1..words.len())
                        .rev()
                        .take_while(|&i| {
                            words[i]
                                .1
                                .split_once('=')
                                .is_some_and(|(k, v)| lex::is_ident(k) && !v.is_empty())
                        })
                        .last()
                        .map_or(body.len(), |i| words[i].0);
                    let (body, meta) = body.split_at(meta_from);
                    let renamed = rename_words(body, from, to);
                    if renamed != body {
                        self.lines[n] = format!("{kept}{renamed}{meta}{}", comment.unwrap_or(""));
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
                    return Err(refuse_at(
                        a.name.line,
                        format!("`{axis}` has a value `{value}` already"),
                    ));
                }
                let mut values = a.values.clone();
                let at = match before {
                    Some(b) => values.iter().position(|v| v == b).ok_or_else(|| {
                        refuse_at(a.name.line, format!("`{axis}` has no value `{b}`"))
                    })?,
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
                let at = a.values.iter().position(|v| v == value).ok_or_else(|| {
                    refuse_at(a.name.line, format!("`{axis}` has no value `{value}`"))
                })?;
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

/// A token by its name as declared, or a role override by any spelling of
/// its role, as the checker reads one: a system role's name in any case
/// (`Canvas`, `canvas`), an Exact role's `-exact-` spelling or bare name in
/// any case (the IR's `-exact-label` for a declared `label`).
fn token_index(
    design: &Design,
    roles: &[(String, bool)],
    name: &str,
) -> Result<usize, Vec<Diagnostic>> {
    let key = |n: &str| role_key(roles, n);
    design
        .tokens
        .iter()
        .position(|t| t.name.value == name)
        .or_else(|| {
            let k = key(name)?;
            design
                .tokens
                .iter()
                .position(|t| key(&t.name.value).as_ref() == Some(&k))
        })
        .ok_or_else(|| refuse(format!("no token `{name}`")))
}

/// The role a name spells, by its schema name, or `None` for a plain name.
fn role_key(roles: &[(String, bool)], n: &str) -> Option<String> {
    let lower = n.to_ascii_lowercase();
    let found = match lower.strip_prefix("-exact-") {
        Some(bare) => roles
            .iter()
            .find(|(r, system)| !system && r.eq_ignore_ascii_case(bare)),
        None => roles
            .iter()
            .find(|(r, system)| *system && r.eq_ignore_ascii_case(&lower))
            .or_else(|| {
                roles
                    .iter()
                    .find(|(r, system)| !system && r.eq_ignore_ascii_case(&lower))
            }),
    };
    found.map(|(r, _)| r.clone())
}

/// `when a=b c=d: value`.
fn when_code(conds: &[(String, String)], value: &str) -> String {
    let conds: Vec<String> = conds.iter().map(|(a, v)| format!("{a}={v}")).collect();
    format!("when {}: {value}", conds.join(" "))
}

/// The same conditions, in any order.
fn same_conds(a: &[(String, String)], b: &[(String, String)]) -> bool {
    a.len() == b.len() && a.iter().all(|c| b.contains(c)) && b.iter().all(|c| a.contains(c))
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
            if c == '\\' {
                // An escape takes the next character with it.
                if let Some(n) = rest.chars().next() {
                    out.push(n);
                    rest = &rest[n.len_utf8()..];
                }
            } else if c == q {
                quote = None;
            }
            continue;
        }
        // A `'` opens a string at a token's start, as the lexer reads it.
        let starts = out
            .chars()
            .next_back()
            .is_none_or(|p| p.is_whitespace() || matches!(p, '=' | '(' | ',' | ':'));
        if c == '"' || c == '\'' && starts {
            quote = Some(c);
            out.push(c);
            rest = &rest[1..];
            continue;
        }
        // A hex colour's digits are no name.
        let before_ok = out
            .chars()
            .next_back()
            .is_none_or(|p| !name_char(p) && p != '#');
        if before_ok
            && rest.starts_with(from)
            && rest[from.len()..]
                .chars()
                .next()
                // A function's name (`rgb(`) is none either.
                .is_none_or(|n| !name_char(n) && n != '(')
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
