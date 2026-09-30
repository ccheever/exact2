//! Absolutely positioned boxes, a root's own margins and the `aspect-ratio`
//! paths LLP 1053 left (LLP 1074), against literal Chrome.
//!
//! Chrome 154, 2026-09-30, by the method of `browser_cases.rs`: each case is
//! plain HTML/CSS inside an 800px-wide box (the kernel's offer); the case's
//! outer box is the kernel root, id 1, `display: flow-root` unless the case
//! names a display; descendants are `box-sizing: content-box` with `font:
//! 16px/18px monospace`. Frames are measured from the 800px box. Text cases
//! use one letter per line, so the measurer's advance does not matter.
//!
//! A fixture line is `name`, the root's declarations, the nodes (`id>parent>
//! declarations>text`, joined by ` & `) and Chrome's frames (`id=x,y,w,h`),
//! separated by tabs.
use crate::browser_cases::{css_rows, lay_out_with, mismatches, props, Rows};

struct Case<'a> {
    name: &'a str,
    root: &'a str,
    nodes: Vec<(u32, u32, &'a str, &'a str)>,
    want: Vec<(u32, [f32; 4])>,
}

fn cases(fixture: &str) -> Vec<Case<'_>> {
    fixture
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let field: Vec<_> = line.split('\t').collect();
            let nodes = field[2]
                .split(" & ")
                .map(|node| {
                    let part: Vec<_> = node.splitn(4, '>').collect();
                    let id = |s: &str| s.parse().unwrap();
                    (id(part[0]), id(part[1]), part[2], part[3])
                })
                .collect();
            let want = field[3]
                .split(' ')
                .map(|frame| {
                    let (id, rect) = frame.split_once('=').unwrap();
                    let v: Vec<f32> = rect.split(',').map(|x| x.parse().unwrap()).collect();
                    (id.parse().unwrap(), [v[0], v[1], v[2], v[3]])
                })
                .collect();
            Case {
                name: field[0],
                root: field[1],
                nodes,
                want,
            }
        })
        .collect()
}

/// Every case's mismatches. The static-root fixtures use the production
/// default; the kernel makes that root the containing block. With
/// `relative`, so is every box that names no position: the cases of the
/// first fixture were measured in Chrome that way.
fn failures(fixture: &str, relative: bool) -> Vec<(String, Vec<String>)> {
    let positioned = |css: &str, relative: bool| {
        let mut rows = css_rows(css);
        if relative && !css.split(';').any(|d| d.starts_with("position:")) {
            rows.extend(css_rows("position:relative"));
        }
        rows
    };
    cases(fixture)
        .into_iter()
        .map(|case| {
            let nodes: Vec<(u32, u32, Rows)> = case
                .nodes
                .iter()
                .map(|(id, parent, css, _)| (*id, *parent, positioned(css, relative)))
                .collect();
            let texts: Vec<(u32, String)> = case
                .nodes
                .iter()
                .filter(|node| !node.3.is_empty())
                .map(|node| (node.0, node.3.to_string()))
                .collect();
            let texts: Vec<(u32, &str)> = texts.iter().map(|(id, s)| (*id, s.as_str())).collect();
            let k = lay_out_with(props(&positioned(case.root, relative)), nodes, &texts, &[]);
            (case.name.to_string(), mismatches(case.name, &k, &case.want))
        })
        .collect()
}

/// Cases about what else makes a containing block in a browser: a
/// transform, a filter. The kernel's rule is position alone; the Contract
/// compiler lowers `position: relative` onto such a box (LLP 1074 T1), so a
/// plan never holds one that is `static`.
const NOT_IN_THE_KERNEL: &[&str] = &[
    "inset 0 under translate: the box",
    "inset 0 under translate 0: the box",
    "inset 0 under scale: the box",
    "inset 0 under rotate: the box",
    "inset 0 under transform: the box",
    "inset 0 under filter: the box",
    "inset 0 under backdrop-filter: the box",
];

fn check(all: Vec<(String, Vec<String>)>, owed: &[&str]) {
    let mut wrong = Vec::new();
    let mut fixed = Vec::new();
    for (name, found) in &all {
        let is_owed = owed.contains(&name.as_str());
        if is_owed && found.is_empty() {
            fixed.push(name.clone());
        } else if !is_owed {
            wrong.extend(found.iter().cloned());
        }
    }
    for name in owed {
        assert!(all.iter().any(|(n, _)| n == name), "no case named {name}");
    }
    assert!(
        wrong.is_empty() && fixed.is_empty(),
        "{}\n{} mismatches; owed cases that now match: {fixed:?}",
        wrong.join("\n"),
        wrong.len()
    );
}

/// Absolute boxes in their containing block, roots and ratios, where every
/// box is positioned.
#[test]
fn positioned_boxes_roots_and_ratios_match_chrome() {
    let fixture = include_str!("fixtures/browser_position.tsv");
    check(failures(fixture, true), &[]);
}

/// LLP 1074 T1: `position: static` is the default, an absolute box's
/// containing block is its nearest positioned ancestor, and with no inset on
/// an axis it sits at its static position.
#[test]
fn containing_blocks_and_static_positions_match_chrome() {
    let fixture = include_str!("fixtures/browser_containing_block.tsv");
    check(failures(fixture, false), NOT_IN_THE_KERNEL);
}
