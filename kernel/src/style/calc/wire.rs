//! A calculation tree on the wire (dimension kind 24): each node a tag and
//! its payload, children in order. The decoder admits only a tree the
//! parser makes, one whose own text (§10.13) parses back to it, so the wire,
//! the text and the parser hold one form of every value.

use super::super::env::{Edge, Region, SegmentVar, TitleBarVar};
use super::super::ViewportUnit;
use super::{Comparison, Entry, EnvTerm, Node, Unit};
use crate::error::DecodeError;
use crate::wire::codec::{Reader, Writer};
use crate::StyleId;

/// Nodes and levels a tree on the wire may hold while it is read, bounding
/// the work before it is checked: past what any text within §10.8's limits
/// makes, a level's `clamp(none, …, none)` and the parentheses the kernel
/// writes included.
pub(super) const MAX_NODES: usize = 2 * super::MAX_VALUES * (super::MAX_DEPTH + 1);
pub(super) const MAX_LEVELS: usize = 2 * super::MAX_DEPTH + 2;

/// Append an entry: a text whose `env()` has a fallback as tag 9 and the
/// codec's string (a `u32` length and UTF-8 bytes); a tree node by node,
/// each a tag (0 a value, 1 `env()`, 2 a sum, 3 a product, 4 a negation, 5
/// an inversion, 6 min, 7 max, 8 clamp), then a value's `f64` and unit (0 a
/// number, 1 a percentage, 2 px, 3–12 a viewport unit); an `env()`'s tag
/// (0–3 an inset's edge, 8–13 a segment's variable, then its two indices,
/// 16–35 a layout region's inset, 36–39 a title-bar area variable);
/// a list's `u8` count; clamp's three presence bytes, each before its
/// argument.
pub(super) fn encode(entry: &Entry, w: &mut Writer) {
    match entry {
        Entry::Tree(n) => node_out(n, w),
        // A text whose `env()` has a fallback, as written
        Entry::Text { text, .. } => {
            w.u8(9);
            w.string(text);
        }
    }
}

fn node_out(n: &Node, w: &mut Writer) {
    match n {
        Node::Value(v, unit) => {
            w.u8(0);
            w.f64(*v);
            w.u8(match unit {
                Unit::Number => 0,
                Unit::Percent => 1,
                Unit::Px => 2,
                Unit::Viewport(u) => 3 + *u as u8,
            });
        }
        Node::Env(term) => {
            w.u8(1);
            match term {
                EnvTerm::Inset(edge) => w.u8(*edge as u8),
                EnvTerm::Segment(var, x, y) => {
                    w.u8(8 + *var as u8);
                    w.u8(*x);
                    w.u8(*y);
                }
                // A layout region's inset (LLP 1123 D10): 16 + region × 4 + edge
                EnvTerm::Region(region, edge) => w.u8(16 + *region as u8 * 4 + *edge as u8),
                EnvTerm::TitleBar(var) => w.u8(36 + *var as u8),
                // A content owner's value (LLP 1123 D9): 40, then its four bytes.
                EnvTerm::Owner(var) => {
                    w.u8(40);
                    for b in var.bytes() {
                        w.u8(b);
                    }
                }
            }
        }
        Node::Sum(c) | Node::Product(c) | Node::Min(c) | Node::Max(c) => {
            w.u8(match n {
                Node::Sum(_) => 2,
                Node::Product(_) => 3,
                Node::Min(_) => 6,
                _ => 7,
            });
            w.u8(c.len() as u8);
            c.iter().for_each(|c| node_out(c, w));
        }
        Node::Negate(c) | Node::Invert(c) => {
            w.u8(if matches!(n, Node::Negate(_)) { 4 } else { 5 });
            node_out(c, w);
        }
        Node::Clamp(c) => {
            w.u8(8);
            c.iter().for_each(|a| optional(a.as_ref(), w));
        }
    }
}

fn optional(n: Option<&Node>, w: &mut Writer) {
    match n {
        Some(n) => {
            w.u8(1);
            node_out(n, w);
        }
        None => w.u8(0),
    }
}

/// Read an entry for `row`, refusing one the parser would not make.
pub(super) fn decode(r: &mut Reader<'_>, row: StyleId) -> Result<Comparison, DecodeError> {
    let refused = DecodeError::InvalidComparison;
    // A text whose `env()` has a fallback: admitted where the parser holds
    // it so, for this row
    let start = r.clone();
    if r.u8()? == 9 {
        let text = r.string()?;
        return match crate::StyleValue::Text(text.into()).dimension(row, row.admits_auto()) {
            Ok(crate::Dimension::Compare(c)) if c == Comparison::substitution(text, row) => Ok(c),
            _ => Err(refused),
        };
    }
    *r = start;
    let mut nodes = 0;
    let tree = node(r, 0, &mut nodes).ok_or(refused.clone())?;
    if !within_author_limits(&tree) {
        return Err(refused);
    }
    let mut text = String::new();
    super::text::math_function(&tree, &mut text);
    // A tree a compact form holds travels as that form, not as a tree
    match super::parse::tree(&text)
        .filter(|parsed| *parsed == tree)
        .map(super::dimension)
    {
        Some(crate::Dimension::Compare(c)) => Ok(c),
        _ => Err(refused),
    }
}

/// Check the simplified tree, excluding parentheses added by serialization.
pub(super) fn within_author_limits(tree: &Node) -> bool {
    let (values, levels) = authored(tree, true);
    values <= super::MAX_VALUES && levels <= super::MAX_DEPTH
}

/// The values `n` holds and the levels of parentheses and functions it
/// takes as written; `opened` when parentheses open it: the root, which
/// `calc()` opens unless it is a function, and a sum a product holds.
fn authored(n: &Node, opened: bool) -> (usize, usize) {
    let function = matches!(n, Node::Min(_) | Node::Max(_) | Node::Clamp(_));
    let product = matches!(n, Node::Product(_));
    let mut values = usize::from(matches!(n, Node::Value(..) | Node::Env(..)));
    let mut deepest = 0;
    for c in n.children() {
        let (v, l) = authored(c, product && matches!(c, Node::Sum(_)));
        values += v;
        deepest = deepest.max(l);
    }
    (values, deepest + usize::from(function || opened))
}

fn node(r: &mut Reader<'_>, level: usize, nodes: &mut usize) -> Option<Node> {
    *nodes += 1;
    if level > MAX_LEVELS || *nodes > MAX_NODES {
        return None;
    }
    let list = |r: &mut Reader<'_>, nodes: &mut usize| -> Option<Vec<Node>> {
        let count = usize::from(r.u8().ok()?);
        (0..count).map(|_| node(r, level + 1, nodes)).collect()
    };
    let optional = |r: &mut Reader<'_>, nodes: &mut usize| -> Option<Option<Node>> {
        match r.u8().ok()? {
            0 => Some(None),
            1 => Some(Some(node(r, level + 1, nodes)?)),
            _ => None,
        }
    };
    Some(match r.u8().ok()? {
        0 => {
            let v = r.f64().ok()?;
            let unit = match r.u8().ok()? {
                0 => Unit::Number,
                1 => Unit::Percent,
                2 => Unit::Px,
                u => Unit::Viewport(*ViewportUnit::ALL.get(usize::from(u.checked_sub(3)?))?),
            };
            Node::Value(v, unit)
        }
        1 => {
            let term = match r.u8().ok()? {
                tag @ 0..=3 => EnvTerm::Inset(Edge::from_index(tag)?),
                tag @ 8..=13 => {
                    let var = SegmentVar::from_index(tag - 8)?;
                    EnvTerm::Segment(var, r.u8().ok()?, r.u8().ok()?)
                }
                tag @ 16..=35 => EnvTerm::Region(
                    Region::from_index((tag - 16) / 4)?,
                    Edge::from_index((tag - 16) % 4)?,
                ),
                tag @ 36..=39 => EnvTerm::TitleBar(TitleBarVar::from_index(tag - 36)?),
                40 => EnvTerm::Owner(crate::style::OwnerVar::from_bytes([
                    r.u8().ok()?,
                    r.u8().ok()?,
                    r.u8().ok()?,
                    r.u8().ok()?,
                ])?),
                _ => return None,
            };
            Node::Env(term)
        }
        2 => Node::Sum(list(r, nodes)?),
        3 => Node::Product(list(r, nodes)?),
        4 => Node::Negate(Box::new(node(r, level + 1, nodes)?)),
        5 => Node::Invert(Box::new(node(r, level + 1, nodes)?)),
        6 => Node::Min(list(r, nodes)?),
        7 => Node::Max(list(r, nodes)?),
        8 => Node::Clamp(Box::new([
            optional(r, nodes)?,
            optional(r, nodes)?,
            optional(r, nodes)?,
        ])),
        _ => return None,
    })
}
