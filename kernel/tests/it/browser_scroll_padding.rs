//! A scroll container's end padding against literal Chrome.
//!
//! Chrome 155, 2026-10-24: each scroller is `width: 200px; height: 100px;
//! overflow: auto; padding: 10px`, and the expected size is its
//! `scrollWidth` × `scrollHeight` (the padding box is 220 × 120). The end
//! padding extends the margin boxes of the scroller's in-flow children, not
//! the overflow of their descendants (CSS Overflow 3 §2.2, "scrollable
//! overflow"): a descendant reaching the padding edge, as a full-bleed
//! hairline in a padded list does, leaves nothing to scroll.
use crate::browser_cases::{css_rows, lay_out_with, props};
use crate::support::reader::text as t;
use exact_kernel::StyleId::{OverflowX, OverflowY};

// `overflow: auto` is added as rows: `css_rows` reads `auto` as a length.
const SCROLLER: &str = "width:200px;height:100px;padding:10px;";

/// A case: its name, the scroller's own declarations after [`SCROLLER`],
/// its descendants as `(id, parent, css)` (the scroller is 2), and Chrome's
/// `scrollWidth` × `scrollHeight`.
type Case = (
    &'static str,
    &'static str,
    &'static [(u32, u32, &'static str)],
    (f32, f32),
);

const CASES: &[Case] = &[
    (
        "block: a child bleeding into the side padding",
        "",
        &[(3, 2, "margin-left:-10px;margin-right:-10px;height:20px")],
        (220.0, 120.0),
    ),
    (
        "block: a grandchild bleeding to the padding edge",
        "",
        &[
            (3, 2, "height:20px"),
            (4, 3, "margin-left:-10px;width:220px;height:20px"),
        ],
        (220.0, 120.0),
    ),
    (
        "block: the last child's bottom margin, then the padding",
        "",
        &[(3, 2, "height:150px;margin-bottom:20px")],
        (220.0, 190.0),
    ),
    (
        "block: a grandchild overflowing its parent, with no padding after it",
        "",
        &[(3, 2, "height:50px"), (4, 3, "height:200px")],
        (220.0, 210.0),
    ),
    (
        "block: a wide child's right margin, then the padding",
        "",
        &[(3, 2, "width:300px;margin-right:15px;height:10px")],
        (335.0, 120.0),
    ),
    (
        "flex column: the last item's bottom margin, then the padding",
        "display:flex;flex-direction:column",
        &[(3, 2, "height:150px;flex-shrink:0;margin-bottom:20px")],
        (220.0, 190.0),
    ),
    (
        "flex column: a grandchild overflowing its item, with no padding after it",
        "display:flex;flex-direction:column",
        &[(3, 2, "height:50px;flex-shrink:0"), (4, 3, "height:200px")],
        (220.0, 210.0),
    ),
    (
        "flex column: a full-bleed hairline in an item",
        "display:flex;flex-direction:column",
        &[
            (3, 2, ""),
            (4, 3, "margin-left:-10px;margin-right:-10px;height:1px"),
        ],
        (220.0, 120.0),
    ),
    (
        "flex column, rtl: a full-bleed hairline in an item",
        "display:flex;flex-direction:column;direction:rtl",
        &[
            (3, 2, ""),
            (4, 3, "margin-left:-10px;margin-right:-10px;height:1px"),
        ],
        (220.0, 120.0),
    ),
    (
        "flex column: a grandchild 30 past the padding edge",
        "display:flex;flex-direction:column",
        &[(3, 2, ""), (4, 3, "margin-right:-40px;height:1px")],
        (250.0, 120.0),
    ),
    (
        "flex row: a wide item's right margin, then the padding",
        "display:flex",
        &[(
            3,
            2,
            "width:300px;flex-shrink:0;margin-right:15px;height:10px",
        )],
        (335.0, 120.0),
    ),
    (
        "grid: a full-bleed hairline in an item",
        "display:grid",
        &[
            (3, 2, ""),
            (4, 3, "margin-left:-10px;margin-right:-10px;height:1px"),
        ],
        (220.0, 120.0),
    ),
    (
        "grid: an item's right margin in a 300px column, then the padding",
        "display:grid;grid-template-columns:300px",
        &[(3, 2, "margin-right:15px;height:10px")],
        (320.0, 120.0),
    ),
    // A relative offset moves the box, not where the padding is added.
    (
        "block: a child moved by a relative offset",
        "",
        &[(3, 2, RELATIVE)],
        (240.0, 120.0),
    ),
    (
        "flex column: an item moved by a relative offset",
        "display:flex;flex-direction:column",
        &[(
            3,
            2,
            "flex-shrink:0;position:relative;left:30px;top:-5px;width:200px;height:100px",
        )],
        (240.0, 120.0),
    ),
    (
        "grid: an item moved by a relative offset",
        "display:grid",
        &[(3, 2, RELATIVE)],
        (240.0, 120.0),
    ),
    (
        "flex column, rtl: an item moved by a relative offset",
        "display:flex;flex-direction:column;direction:rtl",
        &[(
            3,
            2,
            "flex-shrink:0;position:relative;right:30px;top:-5px;width:200px;height:100px",
        )],
        (240.0, 120.0),
    ),
];

const RELATIVE: &str = "position:relative;left:30px;top:-5px;width:200px;height:100px";

#[test]
fn end_padding_extends_children_not_descendants() {
    let mut failures = Vec::new();
    for (name, own, nodes, (width, height)) in CASES {
        let mut scroller = css_rows(&format!("{SCROLLER}{own}"));
        scroller.extend([(OverflowX, t("auto")), (OverflowY, t("auto"))]);
        let mut all = vec![(2, 1, scroller)];
        all.extend(
            nodes
                .iter()
                .map(|(id, parent, css)| (*id, *parent, css_rows(css))),
        );
        let k = lay_out_with(props(&css_rows("width:800px")), all, &[], &[]);
        let (w, h) = k.node(2).unwrap().content;
        // scrollWidth and scrollHeight are at least the padding box.
        let got = (w.max(220.0), h.max(120.0));
        if (got.0 - width).abs() > 0.01 || (got.1 - height).abs() > 0.01 {
            failures.push(format!("{name}: {got:?}, Chrome {width} × {height}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
