//! Which words a name may be (@ref LLP 1088 D5).
//!
//! Sixteen words are reserved, and only where a name is bound: a later
//! expression reads or calls what a binder names, and each of these words
//! heads an expression, a statement or a view node there (`none` is the
//! literal, so `shape none` could never be built). Every other keyword is
//! contextual: a keyword where its construct starts (`state` at the head of
//! a section line, `refresh` before a name in a body) and a name everywhere
//! else. A shape field, a named argument, a member after `.` and an
//! attribute name can never head an expression, so they admit every word.
//! A state's or derive's name takes no type annotation: its type is inferred
//! (`type_annotation`).

use super::*;

/// The words no binder may be.
pub(super) const RESERVED: [&str; 16] = [
    "when", "if", "else", "each", "in", "match", "case", "as", "fn", "and", "or", "not", "true",
    "false", "none", "some",
];

/// Whether `w` is one of the sixteen reserved words.
pub(super) fn is_reserved(w: &str) -> bool {
    RESERVED.contains(&w)
}

/// The refusal of a reserved word where a name goes.
pub(super) fn reserved_message(w: &str) -> String {
    let why = if matches!(w, "true" | "false" | "none" | "some") {
        "it is a literal"
    } else {
        "it shapes an expression"
    };
    format!("`{w}` is reserved in Contract ({why}); choose another name")
}

/// The section a mistaken section word means, as a refusal's tail (authoring
/// bench: `prop pct: number` in a child component).
pub(super) fn section_hint(word: &str) -> String {
    let section = match word {
        "prop" => "props",
        "states" => "state",
        "derives" => "derive",
        "resources" => "resource",
        "mutations" => "mutation",
        "actions" => "action",
        "tasks" => "task",
        "views" => "view",
        "slots" => "slot",
        "provides" => "provide",
        "injects" => "inject",
        _ => return String::new(),
    };
    let place = match section {
        "props" => " (any component but the first in the file declares them)",
        "resource" | "mutation" | "task" => " (only the root component declares one)",
        _ => "",
    };
    format!(": did you mean `{section}`{place}?")
}

/// A view's `else` or `case` with nothing to belong to, and where it goes (authoring
/// bench: an `else` indented one level under its `when`).
pub(super) fn stray(word: &str) -> String {
    let goes = if word == "else" {
        "an `else` sits at its `when`'s indentation, on the line after the `when`'s block"
    } else {
        "a `case` arm sits indented under its `match`"
    };
    format!("`{word}` without a matching construct: {goes}")
}

impl Parser {
    /// An expression continued on an indented line (`? …`, `: …`, `+ …`, `and …`):
    /// a declaration is one line unless parentheses hold it (authoring bench: a
    /// multi-line ternary in a `fn` or a `derive`).
    pub(super) fn continued_expression<T>(&self, id: &'static str) -> Option<R<T>> {
        let continues = matches!(self.peek_kind(), TokenKind::Indent)
            && match self.peek2() {
                TokenKind::Punct(p) => matches!(
                    *p,
                    "?" | ":"
                        | "+"
                        | "-"
                        | "*"
                        | "/"
                        | "%"
                        | "&&"
                        | "||"
                        | "=="
                        | "!="
                        | "<"
                        | "<="
                        | ">"
                        | ">="
                ),
                TokenKind::Ident(w) => matches!(w.as_str(), "and" | "or"),
                _ => false,
            };
        continues.then(|| {
            self.err(
                id,
                "an indented line that starts with an operator continues the line above, and \
                 a declaration is one line: if the line above is an expression, wrap the whole \
                 expression in parentheses, as in `derive label = (done\n    ? \"Done\"\n    : \"Open\")`",
            )
        })
    }

    /// The end of an `else` line: there is no `else if` or `else when` (authoring
    /// bench), so the next choice goes on its own line under the `else`.
    pub(super) fn after_else(&mut self, choice: &str) -> R<()> {
        if self.at_ident("if") || self.at_ident("when") {
            return self.err(
                "syntax-expected-newline",
                format!(
                    "there is no `else {choice}`: end the line at `else` and write the `{choice}` \
                     indented under it"
                ),
            );
        }
        self.newline()
    }

    /// A declaration whose expression starts on the line after its `=` (authoring bench).
    pub(super) fn on_its_line(&self, what: &str) -> R<()> {
        if matches!(self.peek_kind(), TokenKind::Newline | TokenKind::Indent) {
            return self.err(
                "syntax-expected-expression",
                format!(
                    "a {what}'s expression starts on its `=` line; a long one is wrapped in \
                     parentheses opened there, `= (…`, with the rest of it on the lines under it"
                ),
            );
        }
        Ok(())
    }

    /// A TypeScript-style `: T` before a state's or derive's `=`, or `as T` after its
    /// initializer (`form` "as none" when that initializer is `none`; authoring bench):
    /// its type is inferred.
    pub(super) fn type_annotation(&self, w: &str, name: &str, form: &str) -> R<()> {
        let state = w == "state";
        if form == ":" && self.at_punct(":") {
            let from = if state {
                "; an empty start is `none` or `[]`, and the writes give it its type"
            } else {
                ", which is its expression's"
            };
            return self.err(
                "syntax-expected",
                format!(
                    "a {w}'s type is inferred, so `{w} {name}` takes no `: type`: \
                     write `{w} {name} = …`{from}"
                ),
            );
        }
        if form.starts_with("as") && self.at_ident("as") {
            let from = match (state, form == "as none") {
                (true, true) => format!(
                    "its initializer and the writes to it (a `none` takes its type from a write \
                     such as `{name} = some(…)`)"
                ),
                (true, false) => "its initializer and the writes to it".to_string(),
                _ => "its expression".to_string(),
            };
            return self.err(
                "syntax-expected-newline",
                format!("a {w} takes no `as`: its type is inferred from {from}"),
            );
        }
        Ok(())
    }

    /// A name a binder introduces, or one that refers to a binder's name: any
    /// word but the sixteen reserved ones.
    pub(super) fn ident(&mut self) -> R<(String, Span)> {
        match self.peek_kind().clone() {
            TokenKind::Ident(w) if !is_reserved(&w) => {
                let t = self.next();
                Ok((w, t.span))
            }
            TokenKind::Ident(w) => self.err("syntax-expected-name", reserved_message(&w)),
            other => self.err(
                "syntax-expected-name",
                format!("expected a name, found {}", describe(&other)),
            ),
        }
    }

    /// A name where the grammar can never read an expression: a shape field,
    /// a named argument, a member after `.`. Every word is one.
    pub(super) fn field_name(&mut self) -> R<(String, Span)> {
        match self.peek_kind().clone() {
            TokenKind::Ident(w) => {
                let t = self.next();
                Ok((w, t.span))
            }
            _ => self.ident(),
        }
    }
}
