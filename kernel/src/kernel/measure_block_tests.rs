//! A trial layout of a block's root (`Kernel::measure_block`): the used size
//! it would have in another block, by the same engine path as a block's
//! root, changing nothing the kernel holds.
use super::*;
use crate::{BlockAxis, BlockMeasure, HostBlock, StyleValue, TextMeasureRequest, TextMetrics};

const OFFER: Offer = Offer::definite(800.0, 600.0);

fn style(id: ViewId, rows: &[(StyleId, StyleValue)]) -> Op {
    let mut patch = StyleProps::default();
    for (row, value) in rows {
        patch.set_dynamic(*row, value).unwrap();
        patch.written.set(*row);
    }
    Op::SetStyle {
        id,
        patch: Box::new(patch),
    }
}
fn t(s: &str) -> StyleValue<'_> {
    StyleValue::Text(s.into())
}
fn n(x: f64) -> StyleValue<'static> {
    StyleValue::Number(x)
}

/// A root (1) holding a positioned box (2) holding the popover (3, with
/// `rows`) holding a paragraph (4, `text`) and a 120-point box (5).
fn page(rows: &[(StyleId, StyleValue)], text: &str) -> Kernel {
    let mut k = Kernel::with_monospace();
    let mut ops = vec![];
    for id in 1..=5 {
        let node_type = if id == 4 {
            NodeType::Text
        } else {
            NodeType::View
        };
        ops.push(Op::CreateView { id, node_type });
    }
    ops.push(style(
        1,
        &[
            (StyleId::Display, t("flex")),
            (StyleId::FlexDirection, t("column")),
        ],
    ));
    ops.push(style(
        2,
        &[
            (StyleId::PositionType, t("relative")),
            (StyleId::Width, n(100.0)),
            (StyleId::Height, n(100.0)),
        ],
    ));
    ops.push(style(3, rows));
    ops.push(Op::SetProp {
        id: 3,
        prop: PropId::Popover,
        value: "auto".into(),
    });
    ops.push(style(
        5,
        &[(StyleId::Width, n(120.0)), (StyleId::Height, n(10.0))],
    ));
    ops.push(Op::SetProp {
        id: 4,
        prop: PropId::Text,
        value: text.into(),
    });
    ops.push(Op::SetChildren {
        id: 3,
        children: vec![4, 5],
    });
    ops.push(Op::SetChildren {
        id: 2,
        children: vec![3],
    });
    ops.push(Op::SetChildren {
        id: 1,
        children: vec![2],
    });
    ops.push(Op::AttachRoot { id: 1 });
    k.apply(0, 1, &ops).unwrap();
    k.compute_layout(1, OFFER).unwrap();
    k
}

fn fit(w: f32, h: f32) -> HostBlock {
    HostBlock {
        width: BlockAxis::Fit(w),
        height: BlockAxis::Fit(h),
    }
}
fn region(w: f32, h: f32) -> HostBlock {
    HostBlock {
        width: BlockAxis::Region(w),
        height: BlockAxis::Region(h),
    }
}

/// Everything a layout leaves: each node's frame, the blocks with their used
/// sizes, and the engine's own state; with `engine`, its whole tree too, its
/// caches with it, which compares only against the same kernel.
fn digest(k: &Kernel, engine: bool) -> String {
    let frames: Vec<_> = (1..=5)
        .map(|id| (id, k.node(id).map(|n| n.frame)))
        .collect();
    let tree = k
        .layout
        .as_deref()
        .and_then(LayoutMirror::tree_ref)
        .unwrap();
    let whole = if engine {
        tree.engine_digest()
    } else {
        String::new()
    };
    format!(
        "{frames:?} {:?} {} {whole}",
        k.arena.blocks,
        tree.state_digest()
    )
}

/// What `measure_block` answers is what giving the block and laying out
/// reads (theme2's condition b), for each kind of axis and a percentage.
#[test]
fn a_measure_is_the_used_size_the_block_would_give() {
    let text = "one two three four five six seven eight nine ten";
    for rows in [
        vec![],
        vec![(StyleId::Width, t("50%"))],
        vec![
            (StyleId::PaddingLeft, n(7.0)),
            (StyleId::PaddingTop, n(5.0)),
        ],
        vec![
            (StyleId::MarginLeft, t("10%")),
            (StyleId::MarginTop, n(4.0)),
        ],
        // Columns: their height is fragmentation's, settled after the engine.
        vec![
            (StyleId::ColumnCount, n(2.0)),
            (StyleId::ColumnGap, n(10.0)),
        ],
    ] {
        for block in [
            fit(250.0, 600.0),
            fit(90.0, 600.0),
            region(90.0, 20.0),
            region(250.0, 600.0),
            HostBlock {
                width: BlockAxis::AtMost(200.0),
                height: BlockAxis::AtMost(40.0),
            },
            HostBlock {
                width: BlockAxis::Is(300.0),
                height: BlockAxis::Is(500.0),
            },
        ] {
            let mut k = page(&rows, text);
            let measured = k.measure_block(3, block).unwrap().used;
            k.set_host_block(3, Some(block)).unwrap();
            k.compute_layout(1, OFFER).unwrap();
            assert_eq!(Some(measured), k.host_block_size(3), "{rows:?} {block:?}");
        }
    }
}

/// In a region the box is `fit-content` (Chrome's `position-area` box): the
/// 120-point box is its min-content and wins over a narrower region, a
/// long paragraph wraps at the region's width, its height overflows a
/// short region rather than being held in it, and a percentage is the
/// region's. A `Fit` block caps an automatic width instead (unchanged).
#[test]
fn a_region_is_fit_content_with_its_minimum_winning() {
    let mut k = page(&[], "a b");
    assert_eq!(
        k.measure_block(3, region(40.0, 600.0)).unwrap().used.0,
        120.0,
        "min-content wins"
    );
    assert_eq!(
        k.measure_block(3, region(700.0, 600.0)).unwrap().used.0,
        120.0,
        "as wide as its widest line"
    );
    assert_eq!(
        k.measure_block(3, fit(40.0, 600.0)).unwrap().used.0,
        40.0,
        "a fit caps it, as before"
    );
    let mut k = page(&[], &"word ".repeat(100));
    let (w, h) = k.measure_block(3, region(250.0, 20.0)).unwrap().used;
    assert_eq!(w, 250.0, "wraps at the region's width");
    assert!(h > 100.0, "and its height overflows a short region: {h}");
    let mut k = page(&[(StyleId::Width, t("50%"))], "a b");
    assert_eq!(
        k.measure_block(3, region(300.0, 600.0)).unwrap().used.0,
        150.0,
        "a percentage is the region's"
    );
}

/// Nothing the kernel holds changes (theme2's condition a): frames, blocks
/// and the engine's state are the same by digest, its whole tree with its
/// caches among it, with columns (fragmentation's records) and without, and
/// the next layout changes nothing.
#[test]
fn a_measure_changes_nothing() {
    let text = "one two three four five six seven eight nine ten";
    let columns = [
        (StyleId::ColumnCount, n(2.0)),
        (StyleId::ColumnGap, n(10.0)),
    ];
    for rows in [&[][..], &columns[..]] {
        let mut k = page(rows, text);
        let before = digest(&k, true);
        for block in [
            fit(90.0, 600.0),
            region(60.0, 50.0),
            HostBlock {
                width: BlockAxis::Is(300.0),
                height: BlockAxis::AtMost(10.0),
            },
        ] {
            k.measure_block(3, block).unwrap();
            assert_eq!(digest(&k, true), before, "{rows:?} {block:?}");
        }
        // And the next layout is the one a kernel never asked lays out.
        let mut twin = page(rows, text);
        let receipt = k.compute_layout(1, OFFER).unwrap();
        let theirs = twin.compute_layout(1, OFFER).unwrap();
        assert!(
            receipt.changed.is_empty() && receipt.updated.is_empty(),
            "{receipt:?}"
        );
        assert_eq!(
            (receipt.changed, receipt.updated),
            (theirs.changed, theirs.updated)
        );
        assert_eq!(digest(&k, false), digest(&twin, false));
    }
}

/// The margins come back as the layout resolved them: a percentage is the
/// block's, so the border box is the used size less them (Astra round 1).
#[test]
fn a_measure_answers_its_resolved_margins() {
    let rows = [
        (StyleId::Width, n(100.0)),
        (StyleId::MarginLeft, t("10%")),
        (StyleId::MarginTop, n(6.0)),
    ];
    let mut k = page(&rows, "a");
    let m = k.measure_block(3, region(250.0, 600.0)).unwrap();
    assert_eq!(m.margin, [6.0, 0.0, 0.0, 25.0]);
    assert_eq!(m.used.0, 125.0, "the border box and its margins");
    assert_eq!(m.border.0, 100.0);
}

/// Columns are balanced after the engine lays the box out (fragment
/// settling): a trial measures the height the columns settle at, the
/// height the block gives once told (Astra round 1).
#[test]
fn a_measure_settles_columns() {
    let rows = [
        (StyleId::ColumnCount, n(2.0)),
        (StyleId::ColumnGap, n(10.0)),
    ];
    let text = "word ".repeat(60);
    let mut k = page(&rows, &text);
    for block in [region(150.0, 600.0), region(300.0, 600.0), fit(90.0, 600.0)] {
        let measured = k.measure_block(3, block).unwrap().used;
        let mut told = page(&rows, &text);
        told.set_host_block(3, Some(block)).unwrap();
        told.compute_layout(1, OFFER).unwrap();
        assert_eq!(Some(measured), told.host_block_size(3), "{block:?}");
    }
}

/// Whether the box shows is what the layout after would say, not the last
/// one: hidden since the last layout it measures zero, shown again since
/// then its size (Astra round 1).
#[test]
fn a_measure_reads_whether_it_shows_now() {
    let mut k = page(&[], "one two three");
    let shown = k.measure_block(3, region(200.0, 600.0)).unwrap();
    assert!(shown.used.0 > 0.0);
    k.apply(0, 2, &[style(2, &[(StyleId::Display, t("none"))])])
        .unwrap();
    assert_eq!(
        k.measure_block(3, region(200.0, 600.0)).unwrap(),
        BlockMeasure::default(),
        "an ancestor hid it since the last layout"
    );
    k.compute_layout(1, OFFER).unwrap();
    k.apply(0, 3, &[style(2, &[(StyleId::Display, t("block"))])])
        .unwrap();
    assert_eq!(
        k.measure_block(3, region(200.0, 600.0)).unwrap(),
        shown,
        "shown again since"
    );
}

/// A measurer that keeps paragraphs by stamp, as the Mac's reader does: the
/// stamps it is asked with.
struct Stamps(std::rc::Rc<std::cell::RefCell<Vec<crate::text::ParagraphStamp>>>);
impl TextMeasurer for Stamps {
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        MonospaceMeasurer::default().measure(request)
    }
    fn measure_identified(
        &mut self,
        stamp: &crate::text::ParagraphStamp,
        request: &TextMeasureRequest<'_>,
    ) -> TextMetrics {
        self.0.borrow_mut().push(stamp.clone());
        self.measure(request)
    }
}

/// A trial asks with the stamps the kernel's own layout asks with: the same
/// paragraphs, which a measurer holding them by stamp answers from what it
/// holds, not new text that would replace them (Astra round 2).
#[test]
fn a_measure_asks_with_the_paragraphs_own_stamps() {
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut k = page(&[], &"word ".repeat(40)).rehydrate(Box::new(Stamps(seen.clone())));
    k.compute_layout(1, OFFER).unwrap();
    let live = k.node(4).unwrap().paragraph_stamp().unwrap();
    seen.borrow_mut().clear();
    k.measure_block(3, region(130.0, 600.0)).unwrap();
    let asked = seen.borrow();
    assert!(!asked.is_empty(), "the paragraph measured at a new width");
    assert!(asked.iter().all(|s| *s == live), "{asked:?} {live:?}");
}

/// What only the next layout prepares is refused, not guessed: a box that
/// became a popover since the last layout takes its block from that layout,
/// changing its parent's size, and an edit's text is the layout's to show
/// first. Once laid out, the measure is the size again.
#[test]
fn a_measure_waits_for_a_layout_that_must_prepare_first() {
    let mut k = page(&[], "one two three");
    k.apply(
        0,
        2,
        &[Op::SetProp {
            id: 5,
            prop: PropId::Popover,
            value: "auto".into(),
        }],
    )
    .unwrap();
    assert!(k.measure_block(3, region(200.0, 600.0)).is_err());
    k.compute_layout(1, OFFER).unwrap();
    // A text edit under the root waits for its layout too (Astra round 3).
    k.apply(
        0,
        3,
        &[Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "one two three four".into(),
        }],
    )
    .unwrap();
    assert!(k.measure_block(3, region(200.0, 600.0)).is_err());
    k.compute_layout(1, OFFER).unwrap();
    let measured = k.measure_block(3, region(200.0, 600.0)).unwrap().used;
    k.set_host_block(3, Some(region(200.0, 600.0))).unwrap();
    k.compute_layout(1, OFFER).unwrap();
    assert_eq!(Some(measured), k.host_block_size(3));
}

/// The layout stamp stands across measures and moves with a commit that
/// changes something and with a layout, even one that moves no frame (a
/// `max-width` past the box's size: Astra on the host cache).
#[test]
fn the_layout_stamp_moves_whenever_a_measure_could() {
    let mut k = page(&[(StyleId::Width, t("100%"))], "a b");
    let stamp = k.layout_stamp();
    k.measure_block(3, region(400.0, 600.0)).unwrap();
    assert_eq!(k.layout_stamp(), stamp, "a measure moves nothing");
    k.apply(0, 2, &[style(3, &[(StyleId::MaxWidth, n(300.0))])])
        .unwrap();
    let committed = k.layout_stamp();
    assert_ne!(committed, stamp, "a commit that changed something");
    let receipt = k.compute_layout(1, OFFER).unwrap();
    assert_ne!(k.layout_stamp(), committed, "a layout: {receipt:?}");
}

/// Only a block's root is measured; a bad length and an unknown view are
/// refused.
#[test]
fn a_measure_refuses_what_is_no_blocks_root() {
    let mut k = page(&[], "a");
    assert!(
        k.measure_block(2, fit(100.0, 100.0)).is_err(),
        "no block: in its parent's layout"
    );
    assert!(k.measure_block(3, fit(f32::NAN, 100.0)).is_err());
    assert!(k.measure_block(3, fit(-1.0, 100.0)).is_err());
    assert!(k.measure_block(99, fit(100.0, 100.0)).is_err());
}
