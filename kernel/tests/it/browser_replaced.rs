//! Replaced elements without a natural size from content, against literal
//! Chrome: `canvas` (natural size 300×150, so a natural 2:1 ratio), and
//! `iframe`, `video` before metadata and `svg` without a view box, which
//! have only CSS Images 3 §5's default object size, 300×150, and no ratio.
//!
//! Chrome 154, 2026-09-27, by the method of `browser_cases.rs`: each case is
//! a 400px-wide `display: flow-root` box (the kernel root, id 1) with these
//! rows, holding elements `display: block` with these rows (ids 2, 3).
use crate::browser_cases::{css_rows as rows, lay_out_as, mismatches, props};
use crate::support::reader::number as n;
use exact_kernel::StyleId::*;
use exact_kernel::{GridTrack, GridTracks, NodeType, Op, PropId};

type Frames = &'static [[f32; 4]];

/// (name, root rows, element rows, element count, Chrome's frames for
/// canvas, iframe, video, svg).
const CASES: &[(&str, &str, &str, u32, [Frames; 4])] = &[
    (
        "bare",
        "",
        "",
        1,
        [
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
        ],
    ),
    (
        "width 200",
        "",
        "width:200px",
        1,
        [
            &[[0.0, 0.0, 200.0, 100.0]],
            &[[0.0, 0.0, 200.0, 150.0]],
            &[[0.0, 0.0, 200.0, 150.0]],
            &[[0.0, 0.0, 200.0, 150.0]],
        ],
    ),
    (
        "width 100%",
        "",
        "width:100%",
        1,
        [
            &[[0.0, 0.0, 400.0, 200.0]],
            &[[0.0, 0.0, 400.0, 150.0]],
            &[[0.0, 0.0, 400.0, 150.0]],
            &[[0.0, 0.0, 400.0, 150.0]],
        ],
    ),
    (
        "height 100",
        "",
        "height:100px",
        1,
        [
            &[[0.0, 0.0, 200.0, 100.0]],
            &[[0.0, 0.0, 300.0, 100.0]],
            &[[0.0, 0.0, 300.0, 100.0]],
            &[[0.0, 0.0, 300.0, 100.0]],
        ],
    ),
    (
        "ratio 16/9",
        "",
        "aspect-ratio:16/9",
        1,
        [
            &[[0.0, 0.0, 300.0, 168.75]],
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 400.0, 225.0]],
        ],
    ),
    (
        "width 200 ratio 16/9",
        "",
        "width:200px;aspect-ratio:16/9",
        1,
        [
            &[[0.0, 0.0, 200.0, 112.5]],
            &[[0.0, 0.0, 200.0, 112.5]],
            &[[0.0, 0.0, 200.0, 112.5]],
            &[[0.0, 0.0, 200.0, 112.5]],
        ],
    ),
    (
        "width 100% ratio 16/9",
        "",
        "width:100%;aspect-ratio:16/9",
        1,
        [
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 400.0, 225.0]],
        ],
    ),
    (
        "height 90 ratio 16/9",
        "",
        "height:90px;aspect-ratio:16/9",
        1,
        [
            &[[0.0, 0.0, 160.0, 90.0]],
            &[[0.0, 0.0, 160.0, 90.0]],
            &[[0.0, 0.0, 160.0, 90.0]],
            &[[0.0, 0.0, 160.0, 90.0]],
        ],
    ),
    (
        "width 200 ratio auto 16/9",
        "",
        "width:200px;aspect-ratio:auto 16/9",
        1,
        [
            &[[0.0, 0.0, 200.0, 100.0]],
            &[[0.0, 0.0, 200.0, 112.5]],
            &[[0.0, 0.0, 200.0, 112.5]],
            &[[0.0, 0.0, 200.0, 112.5]],
        ],
    ),
    (
        "width 200 height 50 ratio 16/9",
        "",
        "width:200px;height:50px;aspect-ratio:16/9",
        1,
        [
            &[[0.0, 0.0, 200.0, 50.0]],
            &[[0.0, 0.0, 200.0, 50.0]],
            &[[0.0, 0.0, 200.0, 50.0]],
            &[[0.0, 0.0, 200.0, 50.0]],
        ],
    ),
    (
        "max-width 100",
        "",
        "max-width:100px",
        1,
        [
            &[[0.0, 0.0, 100.0, 50.0]],
            &[[0.0, 0.0, 100.0, 150.0]],
            &[[0.0, 0.0, 100.0, 150.0]],
            &[[0.0, 0.0, 100.0, 150.0]],
        ],
    ),
    (
        "min-width 360",
        "",
        "min-width:360px",
        1,
        [
            &[[0.0, 0.0, 360.0, 180.0]],
            &[[0.0, 0.0, 360.0, 150.0]],
            &[[0.0, 0.0, 360.0, 150.0]],
            &[[0.0, 0.0, 360.0, 150.0]],
        ],
    ),
    (
        "max-height 100",
        "",
        "max-height:100px",
        1,
        [
            &[[0.0, 0.0, 200.0, 100.0]],
            &[[0.0, 0.0, 300.0, 100.0]],
            &[[0.0, 0.0, 300.0, 100.0]],
            &[[0.0, 0.0, 300.0, 100.0]],
        ],
    ),
    (
        "min-height 200",
        "",
        "min-height:200px",
        1,
        [
            &[[0.0, 0.0, 400.0, 200.0]],
            &[[0.0, 0.0, 300.0, 200.0]],
            &[[0.0, 0.0, 300.0, 200.0]],
            &[[0.0, 0.0, 300.0, 200.0]],
        ],
    ),
    (
        "width 200 max-height 50",
        "",
        "width:200px;max-height:50px",
        1,
        [
            &[[0.0, 0.0, 200.0, 50.0]],
            &[[0.0, 0.0, 200.0, 50.0]],
            &[[0.0, 0.0, 200.0, 50.0]],
            &[[0.0, 0.0, 200.0, 50.0]],
        ],
    ),
    (
        "width 100% max-height 120",
        "",
        "width:100%;max-height:120px",
        1,
        [
            &[[0.0, 0.0, 400.0, 120.0]],
            &[[0.0, 0.0, 400.0, 120.0]],
            &[[0.0, 0.0, 400.0, 120.0]],
            &[[0.0, 0.0, 400.0, 120.0]],
        ],
    ),
    (
        "width 200 padding 10",
        "",
        "width:200px;padding:10px",
        1,
        [
            &[[0.0, 0.0, 220.0, 120.0]],
            &[[0.0, 0.0, 220.0, 170.0]],
            &[[0.0, 0.0, 220.0, 170.0]],
            &[[0.0, 0.0, 220.0, 170.0]],
        ],
    ),
    (
        "col bare",
        "display:flex;flex-direction:column",
        "",
        1,
        [
            &[[0.0, 0.0, 400.0, 200.0]],
            &[[0.0, 0.0, 400.0, 150.0]],
            &[[0.0, 0.0, 400.0, 150.0]],
            &[[0.0, 0.0, 400.0, 150.0]],
        ],
    ),
    (
        "col ratio 16/9",
        "display:flex;flex-direction:column",
        "aspect-ratio:16/9",
        1,
        [
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 400.0, 225.0]],
        ],
    ),
    (
        "col flex-start bare",
        "display:flex;flex-direction:column;align-items:flex-start",
        "",
        1,
        [
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
        ],
    ),
    (
        "col width 200",
        "display:flex;flex-direction:column",
        "width:200px",
        1,
        [
            &[[0.0, 0.0, 200.0, 100.0]],
            &[[0.0, 0.0, 200.0, 150.0]],
            &[[0.0, 0.0, 200.0, 150.0]],
            &[[0.0, 0.0, 200.0, 150.0]],
        ],
    ),
    (
        "col height 100",
        "display:flex;flex-direction:column",
        "height:100px",
        1,
        [
            &[[0.0, 0.0, 400.0, 100.0]],
            &[[0.0, 0.0, 400.0, 100.0]],
            &[[0.0, 0.0, 400.0, 100.0]],
            &[[0.0, 0.0, 400.0, 100.0]],
        ],
    ),
    (
        "col h300 grow",
        "display:flex;flex-direction:column;height:300px",
        "flex-grow:1",
        1,
        [
            &[[0.0, 0.0, 400.0, 300.0]],
            &[[0.0, 0.0, 400.0, 300.0]],
            &[[0.0, 0.0, 400.0, 300.0]],
            &[[0.0, 0.0, 400.0, 300.0]],
        ],
    ),
    (
        "row bare",
        "display:flex",
        "",
        1,
        [
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
        ],
    ),
    (
        "row grow",
        "display:flex",
        "flex-grow:1",
        1,
        [
            &[[0.0, 0.0, 400.0, 200.0]],
            &[[0.0, 0.0, 400.0, 150.0]],
            &[[0.0, 0.0, 400.0, 150.0]],
            &[[0.0, 0.0, 400.0, 150.0]],
        ],
    ),
    (
        "row grow ratio 16/9",
        "display:flex",
        "flex-grow:1;aspect-ratio:16/9",
        1,
        [
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 400.0, 225.0]],
        ],
    ),
    (
        "row h300 bare",
        "display:flex;height:300px",
        "",
        1,
        [
            &[[0.0, 0.0, 600.0, 300.0]],
            &[[0.0, 0.0, 300.0, 300.0]],
            &[[0.0, 0.0, 300.0, 300.0]],
            &[[0.0, 0.0, 300.0, 300.0]],
        ],
    ),
    (
        "row h300 flex-start",
        "display:flex;height:300px;align-items:flex-start",
        "",
        1,
        [
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
        ],
    ),
    (
        "row width 100%",
        "display:flex",
        "width:100%",
        1,
        [
            &[[0.0, 0.0, 400.0, 200.0]],
            &[[0.0, 0.0, 400.0, 150.0]],
            &[[0.0, 0.0, 400.0, 150.0]],
            &[[0.0, 0.0, 400.0, 150.0]],
        ],
    ),
    (
        "row width 200",
        "display:flex",
        "width:200px",
        1,
        [
            &[[0.0, 0.0, 200.0, 100.0]],
            &[[0.0, 0.0, 200.0, 150.0]],
            &[[0.0, 0.0, 200.0, 150.0]],
            &[[0.0, 0.0, 200.0, 150.0]],
        ],
    ),
    (
        "row height 100",
        "display:flex",
        "height:100px",
        1,
        [
            &[[0.0, 0.0, 200.0, 100.0]],
            &[[0.0, 0.0, 300.0, 100.0]],
            &[[0.0, 0.0, 300.0, 100.0]],
            &[[0.0, 0.0, 300.0, 100.0]],
        ],
    ),
    (
        "row two bare",
        "display:flex",
        "",
        2,
        [
            &[[0.0, 0.0, 300.0, 150.0], [300.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0], [300.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0], [300.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 200.0, 150.0], [200.0, 0.0, 200.0, 150.0]],
        ],
    ),
];

#[test]
fn replaced_elements_size_from_their_natural_or_default_size() {
    let tags = [
        ("canvas", NodeType::Canvas),
        ("iframe", NodeType::WebView),
        ("video", NodeType::Video),
        ("svg", NodeType::Svg),
    ];
    let mut failures = Vec::new();
    for (name, root, item, count, frames) in CASES {
        for ((tag, kind), want) in tags.iter().zip(frames) {
            let root = props(&[vec![(Width, n(400.0))], rows(root)].concat());
            let nodes = (0..*count).map(|i| (2 + i, 1, rows(item))).collect();
            let k = lay_out_as(root, nodes, &[], &[], *kind);
            let want: Vec<_> = want
                .iter()
                .enumerate()
                .map(|(i, f)| (2 + i as u32, *f))
                .collect();
            failures.extend(mismatches(&format!("{tag} {name}"), &k, &want));
        }
    }
    assert!(
        failures.is_empty(),
        "{}\n{} failures",
        failures.join("\n"),
        failures.len()
    );
}

/// (name, root rows, element rows, Chrome's frame for canvas, iframe,
/// video, svg, an `svg` with `viewBox="0 0 100 50"`, and an `img` whose
/// natural size is 100×50). Chrome 154, 2026-09-27, as above.
const PLACED: &[(&str, &str, &str, [Frames; 6])] = &[
    (
        "grid bare",
        "display:grid",
        "",
        [
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 400.0, 200.0]],
            &[[0.0, 0.0, 100.0, 50.0]],
        ],
    ),
    (
        "grid ratio 16/9",
        "display:grid",
        "aspect-ratio:16/9",
        [
            &[[0.0, 0.0, 300.0, 168.75]],
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 100.0, 56.25]],
        ],
    ),
    (
        "grid width 100%",
        "display:grid",
        "width:100%",
        [
            &[[0.0, 0.0, 400.0, 200.0]],
            &[[0.0, 0.0, 400.0, 150.0]],
            &[[0.0, 0.0, 400.0, 150.0]],
            &[[0.0, 0.0, 400.0, 150.0]],
            &[[0.0, 0.0, 400.0, 200.0]],
            &[[0.0, 0.0, 400.0, 200.0]],
        ],
    ),
    (
        "grid h300 bare",
        "display:grid;height:300px",
        "",
        [
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 400.0, 200.0]],
            &[[0.0, 0.0, 100.0, 50.0]],
        ],
    ),
    (
        "grid 2col bare",
        "display:grid;grid-template-columns:200px 200px",
        "",
        [
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 200.0, 100.0]],
            &[[0.0, 0.0, 100.0, 50.0]],
        ],
    ),
    (
        "abs bare",
        "position:relative",
        "position:absolute",
        [
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 400.0, 200.0]],
            &[[0.0, 0.0, 100.0, 50.0]],
        ],
    ),
    (
        "abs inset 0",
        "position:relative;height:300px",
        "position:absolute;left:0;right:0;top:0;bottom:0",
        [
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 400.0, 200.0]],
            &[[0.0, 0.0, 100.0, 50.0]],
        ],
    ),
    (
        "abs left right",
        "position:relative",
        "position:absolute;left:0;right:0",
        [
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 400.0, 200.0]],
            &[[0.0, 0.0, 100.0, 50.0]],
        ],
    ),
    (
        "abs top bottom",
        "position:relative;height:300px",
        "position:absolute;top:0;bottom:0",
        [
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 300.0, 150.0]],
            &[[0.0, 0.0, 400.0, 200.0]],
            &[[0.0, 0.0, 100.0, 50.0]],
        ],
    ),
    (
        "abs inset 0 ratio 16/9",
        "position:relative;height:300px",
        "position:absolute;left:0;right:0;top:0;bottom:0;aspect-ratio:16/9",
        [
            &[[0.0, 0.0, 300.0, 168.75]],
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 400.0, 225.0]],
            &[[0.0, 0.0, 100.0, 56.25]],
        ],
    ),
    (
        "abs left right width 100",
        "position:relative",
        "position:absolute;left:0;right:0;width:100px",
        [
            &[[0.0, 0.0, 100.0, 50.0]],
            &[[0.0, 0.0, 100.0, 150.0]],
            &[[0.0, 0.0, 100.0, 150.0]],
            &[[0.0, 0.0, 100.0, 150.0]],
            &[[0.0, 0.0, 100.0, 50.0]],
            &[[0.0, 0.0, 100.0, 50.0]],
        ],
    ),
];

/// A replaced element never stretches: not to its grid area (CSS Grid 1
/// §6.2, `normal` as `start` for an item with a ratio or a natural size),
/// not between the insets of an absolute box (CSS 2.1 §10.3.8, §10.6.5).
/// Its measure sizes it; a ratio without a natural size fills what is
/// offered, as in block flow.
#[test]
fn replaced_elements_do_not_stretch_in_a_grid_area_or_between_insets() {
    let mut failures = Vec::new();
    for (name, root, item, frames) in PLACED {
        for (i, want) in frames.iter().enumerate() {
            // The one track list, `200px 200px`, is set as tracks.
            let (root, tracks) = match root.split_once(";grid-template-columns:") {
                Some((root, _)) => (root, true),
                None => (*root, false),
            };
            let mut root = props(&[vec![(Width, n(400.0))], rows(root)].concat());
            if tracks {
                root.grid_template_columns =
                    GridTracks::from_tracks(vec![GridTrack::Points(200.0); 2]);
                root.mask.set(GridTemplateColumns);
            }
            let nodes = vec![(2, 1, rows(item))];
            let (tag, kind) = [
                ("canvas", NodeType::Canvas),
                ("iframe", NodeType::WebView),
                ("video", NodeType::Video),
                ("svg", NodeType::Svg),
                ("svg viewBox", NodeType::Svg),
                ("img", NodeType::Image),
            ][i];
            let image = [(2, Some((100.0, 50.0)))];
            let images: &[_] = if kind == NodeType::Image { &image } else { &[] };
            let mut k = lay_out_as(root, nodes, &[], images, kind);
            if tag == "svg viewBox" {
                let op = Op::SetProp {
                    id: 2,
                    prop: PropId::ViewBox,
                    value: "0 0 100 50".into(),
                };
                k.apply(0, 2, &[op]).unwrap();
                k.compute_layout(1, exact_kernel::Offer::definite(800.0, 600.0))
                    .unwrap();
            }
            let want = [(2, want[0])];
            failures.extend(mismatches(&format!("{tag} {name}"), &k, &want));
        }
    }
    assert!(
        failures.is_empty(),
        "{}\n{} failures",
        failures.join("\n"),
        failures.len()
    );
}
