//! CSS Values 4 §10.10.1, "simplify a calculation tree", with the
//! information a specified value has: absolute lengths are px already, and
//! percentages, viewport units and `env()` wait for layout. Sums and products
//! hold their children in §10.13's order ([`sort`]), so equal values are
//! equal trees.

use super::{pick, Node, Unit};

/// The tree `root` simplified, every NaN the one NaN: CSS writes a NaN as
/// `NaN`, whatever its sign or payload (§10.13), so the tree holds no other.
pub(super) fn simplify(root: Node) -> Node {
    canonical(fold(root))
}

fn canonical(n: Node) -> Node {
    let all = |c: Vec<Node>| c.into_iter().map(canonical).collect();
    match n {
        Node::Value(v, unit) if v.is_nan() => Node::Value(f64::NAN, unit),
        Node::Value(..) | Node::Env(..) => n,
        Node::Sum(c) => Node::Sum(all(c)),
        Node::Product(c) => Node::Product(all(c)),
        Node::Min(c) => Node::Min(all(c)),
        Node::Max(c) => Node::Max(all(c)),
        Node::Negate(c) => Node::Negate(Box::new(canonical(*c))),
        Node::Invert(c) => Node::Invert(Box::new(canonical(*c))),
        Node::Clamp(c) => Node::Clamp(Box::new(c.map(|a| a.map(canonical)))),
    }
}

fn fold(root: Node) -> Node {
    match root {
        Node::Value(..) | Node::Env(..) => root,
        Node::Min(c) => min_max(c.into_iter().map(fold).collect(), false),
        Node::Max(c) => min_max(c.into_iter().map(fold).collect(), true),
        Node::Clamp(c) => clamp(c.map(|a| a.map(fold))),
        Node::Negate(c) => negate(fold(*c)),
        Node::Invert(c) => match fold(*c) {
            // The reciprocal of a number; dividing by zero is an infinity
            Node::Value(v, Unit::Number) => Node::Value(1.0 / v, Unit::Number),
            Node::Invert(c) => *c,
            c => Node::Invert(Box::new(c)),
        },
        Node::Sum(c) => sum(c.into_iter().map(fold).collect()),
        Node::Product(c) => product(c.into_iter().map(fold).collect()),
    }
}

/// A numeric value that compares with another of its unit: a percentage
/// does not, since its basis might be negative.
fn comparable(n: &Node) -> Option<(f64, Unit)> {
    match n {
        Node::Value(v, unit) if *unit != Unit::Percent => Some((*v, *unit)),
        _ => None,
    }
}

/// A Min or Max node: its children of one comparable unit combined into the
/// first of them; one child left is the node.
fn min_max(children: Vec<Node>, max: bool) -> Node {
    let mut out: Vec<Node> = Vec::with_capacity(children.len());
    for child in children {
        let merged = comparable(&child).and_then(|(v, unit)| {
            out.iter_mut()
                .find(|o| comparable(o).is_some_and(|(_, u)| u == unit))
                .map(|o| {
                    let (w, _) = comparable(o).expect("found comparable");
                    *o = Node::Value(pick(w, v, max), unit);
                })
        });
        if merged.is_none() {
            out.push(child);
        }
    }
    if out.len() == 1 {
        return out.pop().expect("one child");
    }
    if max {
        Node::Max(out)
    } else {
        Node::Min(out)
    }
}

/// A Clamp node: its result where its children are values of one
/// comparable unit (`none` taking no part), else the node.
fn clamp(c: [Option<Node>; 3]) -> Node {
    let units: Vec<_> = c.iter().flatten().map(comparable).collect();
    let one_unit = units
        .iter()
        .all(|u| u.is_some_and(|(_, unit)| Some(unit) == units[0].map(|(_, u)| u)));
    if let ([lo, Some(v), hi], true) = (&c, one_unit) {
        let (mut x, unit) = comparable(v).expect("comparable");
        if let Some((h, _)) = hi.as_ref().and_then(comparable) {
            x = pick(x, h, false);
        }
        if let Some((l, _)) = lo.as_ref().and_then(comparable) {
            x = pick(l, x, true);
        }
        return Node::Value(x, unit);
    }
    Node::Clamp(Box::new(c))
}

/// A Negate node: a value negated, a negation cancelled, a sum's children
/// each negated; else the node.
pub(super) fn negate(child: Node) -> Node {
    match child {
        Node::Value(v, unit) => Node::Value(-v, unit),
        Node::Negate(c) => *c,
        Node::Sum(c) => sum(c.into_iter().map(negate).collect()),
        c => Node::Negate(Box::new(c)),
    }
}

/// A Sum node: its sums' children taken in, its values of one unit added
/// into one (a zero kept: it is never simply removed), one child left the
/// child.
fn sum(children: Vec<Node>) -> Node {
    let mut flat = Vec::with_capacity(children.len());
    for child in children {
        match child {
            Node::Sum(c) => flat.extend(c),
            c => flat.push(c),
        }
    }
    let mut out: Vec<Node> = Vec::with_capacity(flat.len());
    for child in flat {
        if let Node::Value(v, unit) = child {
            if let Some(Node::Value(w, _)) = out
                .iter_mut()
                .find(|o| matches!(o, Node::Value(_, u) if *u == unit))
            {
                *w += v;
                continue;
            }
        }
        out.push(child);
    }
    if out.len() == 1 {
        return out.pop().expect("one child");
    }
    Node::Sum(sort(out))
}

/// A Product node: its products' children taken in and its numbers
/// multiplied into one; a number times a sum of values spread over it;
/// values alone multiplied out; one child left the child.
fn product(children: Vec<Node>) -> Node {
    let mut flat = Vec::with_capacity(children.len());
    for child in children {
        match child {
            Node::Product(c) => flat.extend(c),
            c => flat.push(c),
        }
    }
    let mut number: Option<f64> = None;
    let mut rest = Vec::with_capacity(flat.len());
    for child in flat {
        match child {
            Node::Value(v, Unit::Number) => number = Some(number.unwrap_or(1.0) * v),
            c => rest.push(c),
        }
    }
    match (number, &rest[..]) {
        (Some(k), []) => return Node::Value(k, Unit::Number),
        // A number times a sum whose children are all values
        (Some(k), [Node::Sum(c)]) if c.iter().all(|n| matches!(n, Node::Value(..))) => {
            let Some(Node::Sum(c)) = rest.pop() else {
                unreachable!("matched a sum");
            };
            return sum(c.into_iter().map(|n| scale(n, k)).collect());
        }
        // Values alone: the type check leaves one unit at most among them
        (k, [Node::Value(v, unit)]) => return Node::Value(k.unwrap_or(1.0) * v, *unit),
        (None, [_]) => return rest.pop().expect("one child"),
        _ => {}
    }
    let mut children: Vec<Node> = number
        .map(|k| Node::Value(k, Unit::Number))
        .into_iter()
        .collect();
    children.extend(rest);
    Node::Product(sort(children))
}

fn scale(n: Node, k: f64) -> Node {
    match n {
        Node::Value(v, unit) => Node::Value(k * v, unit),
        n => n,
    }
}

/// §10.13's order: the number, the percentage, the dimensions by unit
/// (ASCII, case-insensitively), then every other child as it was.
pub(super) fn sort(children: Vec<Node>) -> Vec<Node> {
    let rank = |n: &Node| match n {
        Node::Value(_, Unit::Number) => 0,
        Node::Value(_, Unit::Percent) => 1,
        Node::Value(..) => 2,
        _ => 3,
    };
    let unit = |n: &Node| match n {
        Node::Value(_, u) => u.name(),
        _ => "",
    };
    let mut children = children;
    // A stable sort keeps the others in their order
    children.sort_by(|a, b| {
        rank(a).cmp(&rank(b)).then_with(|| {
            unit(a)
                .to_ascii_lowercase()
                .cmp(&unit(b).to_ascii_lowercase())
        })
    });
    children
}
