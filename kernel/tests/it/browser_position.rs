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

/// An absolutely positioned root is sized and placed in its offer by the
/// solver every absolute box uses. Chrome 154, 2026-09-30: a `position:
/// absolute` box in a `position: relative` 800×600 box.
#[test]
fn an_absolute_root_is_placed_in_its_offer_as_chrome_places_it() {
    let cases: &[(&str, [f32; 4])] = &[
        ("left:10px;top:20px;width:30px;height:40px", [10., 20., 30., 40.]),
        ("left:10px;right:30px;top:5px;height:40px", [10., 5., 760., 40.]),
        ("left:10px;right:30px;top:5px;bottom:15px", [10., 5., 760., 580.]),
        ("right:10px;bottom:20px;width:30px;height:40px", [760., 540., 30., 40.]),
        ("margin-left:7px;margin-top:9px;width:30px;height:40px", [7., 9., 30., 40.]),
        (
            "left:0px;right:0px;top:0px;bottom:0px;width:100px;height:50px;margin-left:auto;margin-right:auto;margin-top:auto;margin-bottom:auto",
            [350., 275., 100., 50.],
        ),
        ("left:10%;top:10%;width:50%;height:50%", [80., 60., 400., 300.]),
        ("left:100px;right:100px;top:0px;aspect-ratio:2", [100., 0., 600., 300.]),
        ("direction:rtl;width:30px;height:40px", [0., 0., 30., 40.]),
    ];
    let mut wrong = Vec::new();
    for (css, want) in cases {
        let root = props(&css_rows(&format!("position:absolute;{css}")));
        let k = lay_out_with(root, vec![], &[], &[]);
        wrong.extend(mismatches(css, &k, &[(1, *want)]));
    }
    // With no insets and no size it is its content's size at the origin, and a
    // child's percentages are of the root.
    let k = lay_out_with(
        props(&css_rows("position:absolute")),
        vec![(2, 1, css_rows("width:60px;height:25px"))],
        &[],
        &[],
    );
    wrong.extend(mismatches("content size", &k, &[(1, [0., 0., 60., 25.])]));
    let k = lay_out_with(
        props(&css_rows(
            "position:absolute;left:10px;top:20px;width:200px;height:100px",
        )),
        vec![(2, 1, css_rows("width:50%;height:50%"))],
        &[],
        &[],
    );
    wrong.extend(mismatches(
        "a child that fills",
        &k,
        &[(1, [10., 20., 200., 100.]), (2, [10., 20., 100., 50.])],
    ));
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The containing-block fixture again with `position: static` authored on
/// the root: the kernel positions a static root itself (the containing block
/// of last resort), so every frame is the same.
#[test]
fn an_authored_static_root_is_the_last_containing_block() {
    let fixture = include_str!("fixtures/browser_containing_block.tsv");
    let rooted: String = fixture
        .lines()
        .filter(|line| !line.is_empty() && !line.split('\t').nth(1).unwrap().contains("position:"))
        .map(|line| {
            let mut field: Vec<String> = line.split('\t').map(str::to_string).collect();
            field[1] = format!("position:static;{}", field[1]);
            field.join("\t") + "\n"
        })
        .collect();
    check(failures(&rooted, false), NOT_IN_THE_KERNEL);
}
