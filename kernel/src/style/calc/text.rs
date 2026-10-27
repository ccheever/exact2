//! CSS Values 4 §10.13: serialize a math function, and a calculation tree,
//! as a specified value. Numbers are written in their shortest form that
//! reads back as the same `f64`, never in scientific notation, rather than
//! rounded to CSSOM's six decimals, so the text is the tree.

use super::{EnvTerm, Node, Unit};

/// "Serialize a math function" whose tree is `root`.
pub(super) fn math_function(root: &Node, out: &mut String) {
    match root {
        Node::Min(c) | Node::Max(c) => {
            out.push_str(if matches!(root, Node::Min(_)) {
                "min("
            } else {
                "max("
            });
            arguments(c.iter().map(Some), out);
            out.push(')');
        }
        Node::Clamp(c) => {
            out.push_str("clamp(");
            arguments(c.iter().map(Option::as_ref), out);
            out.push(')');
        }
        // A non-math function as normal
        Node::Env(..) => tree(root, out),
        _ => {
            out.push_str("calc(");
            out.push_str(stripped(&serialized(root)));
            out.push(')');
        }
    }
}

/// A function's arguments, each without the parentheses around it, `none`
/// for an absent one.
fn arguments<'a>(args: impl Iterator<Item = Option<&'a Node>>, out: &mut String) {
    for (i, a) in args.enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        match a {
            Some(a) => out.push_str(stripped(&serialized(a))),
            None => out.push_str("none"),
        }
    }
}

/// `text` without one pair of parentheses around the whole of it.
fn stripped(text: &str) -> &str {
    text.strip_prefix('(')
        .and_then(|t| t.strip_suffix(')'))
        .unwrap_or(text)
}

fn serialized(n: &Node) -> String {
    let mut out = String::new();
    tree(n, &mut out);
    out
}

/// "Serialize a calculation tree".
fn tree(n: &Node, out: &mut String) {
    match n {
        Node::Value(v, unit) => value(*v, *unit, out),
        Node::Env(term) => env(*term, out),
        Node::Min(_) | Node::Max(_) | Node::Clamp(_) => math_function(n, out),
        Node::Negate(c) => {
            out.push_str("(-1 * ");
            tree(c, out);
            out.push(')');
        }
        Node::Invert(c) => {
            out.push_str("(1 / ");
            tree(c, out);
            out.push(')');
        }
        Node::Sum(c) => {
            out.push('(');
            for (i, child) in c.iter().enumerate() {
                match child {
                    _ if i == 0 => tree(child, out),
                    Node::Negate(g) => {
                        out.push_str(" - ");
                        tree(g, out);
                    }
                    Node::Value(v, unit) if v.is_sign_negative() && !v.is_nan() && *v != 0.0 => {
                        out.push_str(" - ");
                        value(-v, *unit, out);
                    }
                    _ => {
                        out.push_str(" + ");
                        tree(child, out);
                    }
                }
            }
            out.push(')');
        }
        Node::Product(c) => {
            out.push('(');
            for (i, child) in c.iter().enumerate() {
                match child {
                    _ if i == 0 => tree(child, out),
                    Node::Invert(g) => {
                        out.push_str(" / ");
                        tree(g, out);
                    }
                    _ => {
                        out.push_str(" * ");
                        tree(child, out);
                    }
                }
            }
            out.push(')');
        }
    }
}

/// `env()` without a fallback, as CSS writes it.
pub(super) fn env(term: EnvTerm, out: &mut String) {
    if let EnvTerm::Owner(var) = term {
        out.push_str("var(");
        out.push_str(&var.name());
        out.push(')');
        return;
    }
    out.push_str("env(");
    match term {
        EnvTerm::Inset(edge) => {
            out.push_str("safe-area-inset-");
            out.push_str(edge.name());
        }
        EnvTerm::Segment(var, x, y) => {
            use std::fmt::Write as _;
            let _ = write!(out, "viewport-segment-{} {x} {y}", var.name());
        }
        EnvTerm::Region(region, edge) => {
            out.push_str(region.prefix());
            out.push_str(edge.name());
        }
        EnvTerm::TitleBar(var) => {
            out.push_str(super::super::env::TitleBarVar::PREFIX);
            out.push_str(var.name());
        }
        EnvTerm::Owner(_) => unreachable!("written above"),
    }
    out.push(')');
}

/// A numeric value as CSS writes it; an infinity or a NaN, which no
/// number token spells, as the product §10.13 writes for it.
fn value(v: f64, unit: Unit, out: &mut String) {
    let keyword = if v.is_nan() {
        "NaN"
    } else if v == f64::INFINITY {
        "infinity"
    } else if v == f64::NEG_INFINITY {
        "-infinity"
    } else if v == 0.0 && v.is_sign_negative() {
        // A zero as written is unsigned; one arithmetic made negative is
        // written as the product that makes it
        out.push_str("(-1 * 0");
        out.push_str(unit.name());
        out.push(')');
        return;
    } else {
        out.push_str(&number(v));
        out.push_str(unit.name());
        return;
    };
    if unit == Unit::Number {
        out.push_str(keyword);
        return;
    }
    out.push('(');
    out.push_str(keyword);
    out.push_str(" * 1");
    out.push_str(unit.name());
    out.push(')');
}

/// A finite number in its shortest form that reads back as itself.
pub(super) fn number(v: f64) -> String {
    exact_num::Shortest(v).to_string()
}
