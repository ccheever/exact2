//! @ref LLP 1115.000 D6 (amended 2026-10-13, Charlie's ruling) — a default
//! yields to the author's version of the same state: a disabled custom
//! pressable dims on the hosts that dim it, unless its author styles its
//! own disabled state. Decided here, once, from the element as written:
//! Contract marks the node (`disabledStyled`), and every host that dims
//! reads the mark.
//!
//! A node styles its own disabled state when it has `disabled` and
//!
//! - writes `opacity`, whatever its value: the author said how faint it is;
//! - or writes a colour, a background, a border (or its colour) or a filter
//!   whose expression reads a name its `disabled` expression reads from
//!   outside it (a slot, a derive, a prop; not a callback's parameter or a
//!   `match` binding): the look changes with what disables it.
//!
//! A literal `disabled` reads nothing, so only an `opacity` marks it.

use contract_syntax::{Attr, Expr, TemplatePart};

/// The paint a disabled look is drawn in.
const PAINT: &[&str] = &[
    "color",
    "background",
    "background-color",
    "background-image",
    "border",
    "border-top",
    "border-right",
    "border-bottom",
    "border-left",
    "border-color",
    "border-top-color",
    "border-right-color",
    "border-bottom-color",
    "border-left-color",
    "filter",
];

/// Whether the element, as its classes expanded it, styles its own disabled state.
pub(crate) fn styled(attrs: &[Attr]) -> bool {
    let Some(disabled) = attrs.iter().rev().find(|a| a.name == "disabled") else {
        return false;
    };
    if attrs.iter().any(|a| a.name == "opacity") {
        return true;
    }
    let mut reads = Vec::new();
    free_reads(&disabled.value, &mut Vec::new(), &mut reads);
    !reads.is_empty()
        && attrs
            .iter()
            .filter(|a| PAINT.contains(&a.name.as_str()))
            .any(|a| {
                let mut paint = Vec::new();
                free_reads(&a.value, &mut Vec::new(), &mut paint);
                paint.iter().any(|n| reads.contains(n))
            })
}

/// The names an expression reads from outside it: not a callback's
/// parameters, a `match` arm's binding or a `let`'s name in its own scope
/// (`bound`); a call reads its arguments, not its name.
fn free_reads(e: &Expr, bound: &mut Vec<String>, out: &mut Vec<String>) {
    fn under(names: &[&str], body: &Expr, bound: &mut Vec<String>, out: &mut Vec<String>) {
        let depth = bound.len();
        bound.extend(names.iter().map(|n| n.to_string()));
        free_reads(body, bound, out);
        bound.truncate(depth);
    }
    match e {
        Expr::Ident(n, _) => {
            if !bound.contains(n) {
                out.push(n.clone());
            }
        }
        Expr::Call(_, args, _) | Expr::List(args, _) => {
            args.iter().for_each(|a| free_reads(a, bound, out))
        }
        Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Typed(x, _, _)
        | Expr::Some(x, _)
        | Expr::Unary(_, x, _) => free_reads(x, bound, out),
        Expr::Arrow { params, body, .. } => {
            let names: Vec<&str> = params.iter().map(String::as_str).collect();
            under(&names, body, bound, out);
        }
        Expr::Binary(_, a, b, _) => {
            free_reads(a, bound, out);
            free_reads(b, bound, out);
        }
        Expr::Ternary(a, b, c, _) => {
            free_reads(a, bound, out);
            free_reads(b, bound, out);
            free_reads(c, bound, out);
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => {
            free_reads(subject, bound, out);
            under(&[var.as_str()], some, bound, out);
            free_reads(none, bound, out);
        }
        Expr::Let {
            name, value, body, ..
        } => {
            free_reads(value, bound, out);
            under(&[name.as_str()], body, bound, out);
        }
        Expr::Template(parts, _) => parts.iter().for_each(|p| {
            if let TemplatePart::Expr(x) = p {
                free_reads(x, bound, out);
            }
        }),
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => {}
    }
}
