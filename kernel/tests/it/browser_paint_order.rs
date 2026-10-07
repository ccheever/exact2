//! The order siblings paint in (LLP 1083.000), against literal Chrome.
//!
//! Each case is plain HTML/CSS laid out as `browser_cases.rs` lays its cases
//! out, every box an opaque colour of its own. The fixture records, at each
//! probe point, the topmost box Chrome paints twice:
//!
//! - **bare**: the markup as written, plain CSS;
//! - **exact**: the same markup with `isolation: isolate` written on exactly
//!   the boxes the kernel isolates or stacks by policy, which is what every
//!   web path writes (D1).
//!
//! The test holds the kernel to both:
//!
//! - **exact** must equal a painter that paints each child's whole subtree
//!   in turn, children by the kernel's rank then tree order, which is how
//!   every native host paints. So the web and the native hosts agree.
//! - where **bare** differs from **exact**, the case's name must declare a
//!   deviation (`§3.n`); where it does not, it must not.
//!
//! The isolation the fixture was measured with is recorded too, so a change
//! to the rule fails here until the fixture is measured again:
//! `bun kernel/tests/it/fixtures/browser_paint_order.mjs` (it runs this test
//! with `EXACT_PAINT_ORDER_DUMP` set to learn the kernel's decisions, then
//! drives Chrome).
//!
//! A line: name, the root's declarations, the nodes (`id>parent>css`, joined
//! by ` & `), the probes (`x,y`, space-separated), bare's top ids, exact's
//! top ids, and the isolated ids (`-` for none), tab-separated.
use crate::browser_cases::{css_rows, lay_out_with, props, Rows};
use exact_kernel::paint_order::Placed;
use exact_kernel::StyleId::*;
use exact_kernel::{Kernel, StyleValue};
use std::collections::BTreeMap;

/// Rows for declarations `css_rows` leaves out (it maps transforms and
/// isolation to nothing for its own fixtures).
fn rows(css: &str) -> Rows {
    let mut out = Rows::new();
    for decl in css.split(';').filter(|d| !d.is_empty()) {
        let (name, value) = decl.split_once(':').unwrap();
        let number = || StyleValue::Number(value.trim_end_matches("px").parse().unwrap());
        let text = || StyleValue::Text(value.into());
        match name {
            "scale" => out.push((Scale, number())),
            "opacity" => out.push((Opacity, number())),
            "z-index" => out.push((ZIndex, number())),
            "isolation" => out.push((Isolation, text())),
            "transition" => out.push((Transition, text())),
            "background" => {}
            _ => out.extend(css_rows(decl)),
        }
    }
    out
}

struct Case {
    name: String,
    root: String,
    nodes: Vec<(u32, u32, String)>,
    probes: Vec<(f32, f32)>,
    bare: Vec<u32>,
    exact: Vec<u32>,
    isolated: Vec<u32>,
}

fn ids(field: &str) -> Vec<u32> {
    if field == "-" {
        return Vec::new();
    }
    field.split(' ').map(|i| i.parse().unwrap()).collect()
}

fn cases() -> Vec<Case> {
    include_str!("fixtures/browser_paint_order.tsv")
        .lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let f: Vec<_> = line.split('\t').collect();
            Case {
                name: f[0].into(),
                root: f[1].into(),
                nodes: f[2]
                    .split(" & ")
                    .map(|n| {
                        let p: Vec<_> = n.splitn(3, '>').collect();
                        (p[0].parse().unwrap(), p[1].parse().unwrap(), p[2].into())
                    })
                    .collect(),
                probes: f[3]
                    .split(' ')
                    .map(|p| {
                        let (x, y) = p.split_once(',').unwrap();
                        (x.parse().unwrap(), y.parse().unwrap())
                    })
                    .collect(),
                // A case not measured yet has only its first four fields.
                bare: f.get(4).map_or_else(Vec::new, |f| ids(f)),
                exact: f.get(5).map_or_else(Vec::new, |f| ids(f)),
                isolated: f.get(6).map_or_else(Vec::new, |f| ids(f)),
            }
        })
        .collect()
}

fn lay_out(case: &Case) -> Kernel {
    let nodes = case
        .nodes
        .iter()
        .map(|(id, parent, css)| (*id, *parent, rows(css)))
        .collect();
    lay_out_with(
        props(&rows(&case.root.replace("flow-root", "block"))),
        nodes,
        &[],
        &[],
    )
}

/// The topmost box at `(x, y)` when each child's whole subtree paints in
/// turn, children by rank then tree order: the native hosts' painting.
fn native_top(k: &Kernel, placed: &BTreeMap<u32, Placed>, at: (f32, f32)) -> Option<u32> {
    fn paint(
        k: &Kernel,
        placed: &BTreeMap<u32, Placed>,
        id: u32,
        at: (f32, f32),
        top: &mut Option<u32>,
    ) {
        let node = k.node(id).unwrap();
        let f = node.frame;
        if at.0 >= f.x && at.0 < f.x + f.width && at.1 >= f.y && at.1 < f.y + f.height {
            *top = Some(id);
        }
        let mut children: Vec<(usize, u32)> = node.children().into_iter().enumerate().collect();
        children.sort_by_key(|&(i, c)| (placed[&c].rank, i));
        for (_, c) in children {
            paint(k, placed, c, at, top);
        }
    }
    let mut top = None;
    paint(k, placed, 1, at, &mut top);
    top
}

#[test]
fn the_web_with_the_kernels_isolation_paints_what_the_native_hosts_paint() {
    let dump = std::env::var("EXACT_PAINT_ORDER_DUMP").ok();
    let mut out = String::new();
    let mut wrong = Vec::new();
    for case in cases() {
        let k = lay_out(&case);
        let placed: BTreeMap<u32, Placed> = k.paint_order().into_iter().collect();
        let mut isolated: Vec<u32> = Vec::new();
        let mut policy: Vec<u32> = Vec::new();
        for (id, p) in &placed {
            if p.isolated {
                isolated.push(*id);
            }
            if p.policy {
                policy.push(*id);
            }
        }
        if dump.is_some() {
            out.push_str(&format!(
                "{{\"name\":{:?},\"isolate\":{:?},\"policy\":{:?}}}\n",
                case.name, isolated, policy
            ));
            continue;
        }
        if isolated != case.isolated {
            wrong.push(format!("{}: the kernel isolates {isolated:?}, the fixture was measured with {:?}; measure it again", case.name, case.isolated));
            continue;
        }
        for (i, &at) in case.probes.iter().enumerate() {
            let native = native_top(&k, &placed, at);
            if native != Some(case.exact[i]) {
                wrong.push(format!(
                    "{} at {at:?}: native paints {native:?} on top, the web {}",
                    case.name, case.exact[i]
                ));
            }
        }
        let differs = case.bare != case.exact;
        if differs != case.name.contains('§') {
            wrong.push(format!(
                "{}: bare CSS {:?}, Exact {:?}; a difference must be declared (§3.n) and only then",
                case.name, case.bare, case.exact
            ));
        }
    }
    if let Some(path) = dump {
        std::fs::write(path, out).unwrap();
        return;
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
