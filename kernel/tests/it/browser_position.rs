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
//! Cases prefixed `Chrome 151 absolute review`, `Chrome 151 wrapped main size`
//! or `Chrome 151 content minimum` were measured on 2026-10-18 with Chrome
//! 151.0.7922.34, including full container and item frames for wrapping
//! columns, automatic item minima and row controls.
//! `Chrome 151 shrink-to-fit row max` uses the same version and date for
//! capped absolute rows, rows inside capped absolute blocks, and controls
//! for normal flow, no maximum, nowrap, widths below the maximum and columns.
//! `Chrome 151 wrapping min-content` uses that version and date for contents
//! wider and narrower than the flex basis, zero and automatic minima, hidden
//! overflow, preferred widths, item bounds, margins, padding and nowrap.
//! The same frames were measured with float:left wrappers in an 80px offer.
//! `Chrome 151 wrapping content bases` uses that version and date for nested
//! wrapping items after a wider item. The absolute frames also agree inside
//! width:min-content wrappers, used for the direct intrinsic requests below.
//! `Chrome 151 stretched ratio` uses that version and date for percentage
//! children of stretched ratio items, including unchanged content-grown heights.
//! Its fractional cases cover item margins, container padding and borders,
//! both box-sizing modes, nested containers, columns and clamping. Actual
//! stretch is checked down to one 1/64px layout unit.
//! `Chrome 151 stretched ratio snapped` compares frames in layout units:
//! these constraints differ within a unit, while kernel geometry stays f32.
//! `Chrome 151 ratio minimum` uses that version and date for widths derived
//! from definite heights in shrinking rows and an absolute block. Controls
//! cover explicit minima, overflow, stretch, columns, wider content, height
//! bounds, and padding and borders under both box-sizing modes.
//! `Chrome 151 ratio column stretch` uses that version and date for narrow
//! columns, reversed columns, alignment overrides, auto and fixed margins,
//! width and height bounds, padding, wider content, intrinsic widths,
//! wrapping, block and grid.
//! `Chrome 151 ratio column shrink` was measured on 2026-10-19 with Chrome
//! 151.0.7922.34. It covers preferred heights in shorter columns, definite
//! stretch versus wrapping, alignment and auto margins, content minima,
//! authored widths, shrink opt-outs, padding, borders and row controls.
//! `Chrome 151 ratio container minimum` uses that version and date for flex,
//! grid and block items in short columns, including ratio floors, taller
//! content, percentage children, shrink opt-outs, box sizing and row controls.
//! `Chrome 151 ratio container content floor` uses that version and date for
//! content wider or taller than the transfer, specified-size caps, insets,
//! shrink opt-outs, ratio-2 floors and a flex column inside a row.
//! `Chrome 151 ratio definite cross` uses that version and date for auto-width
//! ratio containers in a 50px block and as roots offered 50px. Controls cover
//! a second ratio, height bounds, wrapping, alignment, column direction,
//! and flex and grid items.
//! `Chrome 151 column stretch floor` uses that version and date for RTL
//! columns with items wider than the line's stretch size. Controls cover
//! ratios, LTR, fitting items, alignment, explicit widths, padding, borders
//! and width bounds under both box-sizing modes.
//! `Chrome 151 ratio max-height minimum` uses that version and date for
//! wide content in ratio items capped by max-height. Controls cover block,
//! flex and grid items, columns, definite and automatic heights, specified
//! widths, bounds, explicit minima, overflow, box sizing and reversed rows.
//! `Chrome 151 ratio max-height growth` uses that version and date for
//! growing auto-height items, with block, flex and grid contents and parents.
//! Controls include explicit and stretched heights, equal height bounds,
//! minima, overflow, padding, flex bases and width caps.
//! `Chrome 151 ratio inline probe` uses that version and date for direct
//! text leaves and ratio boxes in block, absolute, root and column layout.
//! The text's monospace advance was normalized to 10px in Chrome to match
//! the reference measurer: twelve `a`s have a measured width of 120px.
//! `Chrome 151 ratio text growth` was measured on 2026-10-24 with Chrome
//! 151.0.7922.34, using the same 10px advance. It covers growing auto-height
//! text leaves with height caps, flex bases, explicit minima, overflow,
//! width caps, ratios, box sizing, definite heights and stretch. Controls
//! include equal height bounds, a growing sibling and leaves without a ratio
//! or height cap.
//! `Chrome 151 ratio text wrapping` uses the same version and date. Letter
//! and word spacing were normalized separately: `aaaaaa aaaaaa` measures
//! 130px at max-content and 60px at min-content. Cases cover column cross
//! sizing with definite, flexed and percentage heights, height caps, stretch,
//! box sizing, ratios and inline bounds, with row, block and absolute controls.
//! `Chrome 151 grid ratio maximum` was measured on 2026-10-19 with Chrome
//! 151.0.7922.34. It covers grid item content minima and track contributions
//! under maximum sizes, implicit and explicit auto columns, alignment,
//! authored bounds, overflow, box sizing, and the max-width counterpart.
//! `Chrome 151 grid ratio stretch` uses the same version and date for explicit
//! stretch in fixed, auto, intrinsic and fractional tracks. Controls cover
//! normal alignment, margins, box sizing, authored limits, overflow and
//! siblings that expose the auto column's content minimum.
//! `Chrome 151 grid ratio span` and `Chrome 151 grid ratio minimum floor` were
//! measured on 2026-10-19 with Chrome 151.0.7922.34. They cover spanning items
//! beside fractional tracks on both axes, gaps, normal and stretch alignment,
//! zero minima, and ratio floors in max-content and shrink-to-fit grids.
//! `Chrome 151 ratio max-width height` uses that version and date for tall
//! content under a transferred height maximum in block, absolute and grid
//! layout. Controls cover short content, flex rows and columns, authored
//! height bounds, explicit minima, overflow, ratios, insets and nested layouts.
//! `Chrome 151 ratio column conflicting widths` uses that version and date
//! for column content caps when min-width exceeds max-width, including
//! preferred heights, height bounds, reversed columns and non-binding maxima.
//!
//! `Chrome 151 intrinsic flex basis` uses that version and date for shrink-to-fit
//! rows, a block wrapper, growth and shrink limits, automatic minima, bounds,
//! indefinite bases, padding, borders, margins and overflow, with column and wrap controls.
//! The zero-inner-basis cases include negative margins on padding/border floors,
//! growable zero-width items, negative left margins and a zero-height column.
//!
//! `Chrome 151 ratio root overflow` was measured on 2026-10-24 with Chrome
//! 151.0.7922.34, in a 50px block and as direct roots offered 50×600 and
//! 50×1000. It covers overflowing children, stretch, wrapping, automatic and
//! explicit root widths, item minima, overflow and both box-sizing modes,
//! with block, grid, column and explicit-height controls.
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
            let wrong = if case.name.starts_with("Chrome 151 stretched ratio snapped ") {
                case.want
                    .iter()
                    .filter_map(|(id, want)| {
                        let f = k.node(*id).unwrap().frame;
                        let got = [f.x, f.y, f.width, f.height];
                        got.iter()
                            .zip(want)
                            .any(|(g, w)| (g * 64.0).floor() != (w * 64.0).floor())
                            .then(|| {
                                format!("{} #{id}: kernel {got:?}, Chrome {want:?}", case.name)
                            })
                    })
                    .collect()
            } else {
                mismatches(case.name, &k, &case.want)
            };
            (case.name.to_string(), wrong)
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
    // QUEUE: the auto-height column shrinks to 60px instead of its 100px cap.
    check(
        failures(fixture, true),
        &["Chrome 151 ratio text wrapping column height cap"],
    );
}

/// The same Chrome frames hold when the ratio container is the root in a
/// 50px offer, including children that overflow its ratio-derived height.
#[test]
fn ratio_roots_in_a_definite_offer_match_chrome() {
    let mut wrong = Vec::new();
    for case in cases(include_str!("fixtures/browser_position.tsv"))
        .into_iter()
        .filter(|case| {
            case.name.starts_with("Chrome 151 ratio definite cross ")
                || case.name.starts_with("Chrome 151 ratio root overflow ")
        })
    {
        let root = case.nodes[0].2;
        let nodes = case.nodes[1..]
            .iter()
            .map(|(id, parent, css, _)| (id - 1, parent - 1, css_rows(css)))
            .collect();
        let want: Vec<_> = case.want[1..]
            .iter()
            .map(|(id, frame)| (id - 1, *frame))
            .collect();
        let mut k = lay_out_with(props(&css_rows(root)), nodes, &[], &[]);
        for height in [600.0, 1000.0] {
            k.compute_layout(1, exact_kernel::Offer::definite(50.0, height))
                .unwrap();
            wrong.extend(mismatches(case.name, &k, &want));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Chrome 151.0.7922.34, 2026-10-18: cross-axis auto margins with negative,
/// zero and positive free space, in rows, columns and reversed directions.
/// RTL columns sit in an LTR block; wrapping and fixed margins are controls.
#[test]
fn cross_axis_auto_margins_match_chrome() {
    check(
        failures(
            include_str!("fixtures/browser_flex_auto_margins.tsv"),
            false,
        ),
        &[],
    );
}

/// Chrome 151.0.7922.34, 2026-10-18: overflowing and fitting flex lines,
/// reversed main axes, RTL, and grid/block controls.
#[test]
fn flex_distribution_fallbacks_match_chrome() {
    check(
        failures(
            include_str!("fixtures/browser_flex_wrap_reverse.tsv"),
            false,
        ),
        &[],
    );
}

/// The row itself receives MinContent, without an absolute solver's later
/// definite-width pass. Chrome's width:min-content wrapper gives these frames.
#[test]
fn min_content_rows_remeasure_content_dependent_bases() {
    use exact_kernel::{AxisOffer, NodeType, Offer, Op};
    use std::collections::BTreeMap;

    let mut wrong = Vec::new();
    for case in cases(include_str!("fixtures/browser_position.tsv"))
        .into_iter()
        .filter(|case| case.name.starts_with("Chrome 151 wrapping content bases"))
    {
        let root = case.nodes[0].0;
        let mut ops = Vec::new();
        let mut children: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for (id, parent, css, text) in case.nodes {
            assert!(text.is_empty());
            let css = css.replace("position:absolute;", "");
            ops.extend([
                Op::CreateView {
                    id,
                    node_type: NodeType::View,
                },
                Op::SetStyle {
                    id,
                    patch: Box::new(props(&css_rows(&css))),
                },
            ]);
            if id != root {
                children.entry(parent).or_default().push(id);
            }
        }
        ops.extend(
            children
                .into_iter()
                .map(|(id, children)| Op::SetChildren { id, children }),
        );
        ops.push(Op::AttachRoot { id: root });
        let mut k = crate::support::reader::kernel();
        k.apply(0, 1, &ops).unwrap();
        k.compute_layout(
            root,
            Offer {
                width: AxisOffer::MinContent,
                height: AxisOffer::MaxContent,
            },
        )
        .unwrap();
        let want: Vec<_> = case.want.into_iter().filter(|(id, _)| *id != 1).collect();
        wrong.extend(mismatches(case.name, &k, &want));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Chrome 151.0.7922.34, 2026-10-18: a width:max-content wrapper around each
/// case. The eight basic row cases also agree inside float:left wrappers.
/// `Chrome 151 intrinsic wrap floor` also checks the min-content floor,
/// including reversed rows, item and container padding, margins and a cap.
/// `Chrome 151 intrinsic ratio floor` (2026-10-19) checks the ratio height
/// after intrinsic width sizing, including line alignment and content overflow.
/// `Chrome 151 intrinsic ratio percentages` (151.0.7922.34, 2026-10-23)
/// checks item heights and bounds against the ratio basis, including when
/// content grows beyond it, wrapping, box sizing and a definite-width control.
/// `Chrome 151 intrinsic ratio column percentages` (151.0.7922.34, 2026-10-24)
/// checks the same basis after column cross sizing, including tall content,
/// reverse directions, caps, box sizing, and definite-size and no-ratio controls.
/// A max-content root offer must publish wrapped frames without a parent's
/// definite-width pass.
#[test]
fn max_content_roots_wrap_at_their_resolved_width() {
    use exact_kernel::{NodeType, Offer, Op};

    let mut wrong = Vec::new();
    for case in cases(include_str!("fixtures/browser_max_content.tsv")) {
        let mut ops = Vec::new();
        for (id, css) in std::iter::once((1, case.root)).chain(case.nodes.iter().map(
            |(id, parent, css, text)| {
                assert_eq!(*parent, 1);
                assert!(text.is_empty());
                (*id, *css)
            },
        )) {
            ops.push(Op::CreateView {
                id,
                node_type: NodeType::View,
            });
            ops.push(Op::SetStyle {
                id,
                patch: Box::new(props(&css_rows(css))),
            });
        }
        ops.extend([
            Op::SetChildren {
                id: 1,
                children: case.nodes.iter().map(|node| node.0).collect(),
            },
            Op::AttachRoot { id: 1 },
        ]);
        let mut k = crate::support::reader::kernel();
        k.apply(0, 1, &ops).unwrap();
        k.compute_layout(1, Offer::MAX_CONTENT).unwrap();
        // Chrome quantizes each length to 1/64px. These fractional cases
        // accumulate up to five such lengths; their measured drift is <0.05px.
        let tolerance = if case.name.starts_with("fractional ") {
            0.05
        } else {
            0.01
        };
        for (id, want) in &case.want {
            let f = k.node(*id).unwrap().frame;
            let got = [f.x, f.y, f.width, f.height];
            if got
                .iter()
                .zip(want)
                .any(|(g, w)| !g.is_finite() || !w.is_finite() || (g - w).abs() > tolerance)
            {
                wrong.push(format!(
                    "{} #{id}: kernel {got:?}, Chrome {want:?}",
                    case.name
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
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

/// Chrome 154, 2026-10-02: this fixture uses 16px/18px monospace with
/// `letter-spacing: calc(10px - 1ch)` so each glyph matches the kernel's
/// 10px test advance. Insets, margins, wrapping and used widths are measured.
#[test]
fn available_space_with_margins_matches_chrome() {
    check(
        failures(include_str!("fixtures/browser_available_space.tsv"), false),
        &[],
    );
}
