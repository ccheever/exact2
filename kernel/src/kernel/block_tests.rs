//! A host's containing blocks (LLP 1115.002 D8.2): what a block that
//! changes costs, and what the fixtures in Chrome
//! (`tests/it/browser_host_block.rs`) cannot show.
use super::*;
use crate::{BlockAxis, HostBlock, StyleValue, TextMeasureRequest, TextMetrics};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

const OFFER: Offer = Offer::definite(800.0, 600.0);

/// Measurements by the first letter of the text measured: each subtree's
/// rows begin with its own.
struct Count(Rc<RefCell<BTreeMap<char, usize>>>);
impl TextMeasurer for Count {
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        let first = request.runs[0].text.chars().next().unwrap_or(' ');
        *self.0.borrow_mut().entry(first).or_default() += 1;
        MonospaceMeasurer::default().measure(request)
    }
}

fn t(s: &str) -> StyleValue<'_> {
    StyleValue::Text(s.into())
}
fn n(x: f64) -> StyleValue<'static> {
    StyleValue::Number(x)
}
fn style(id: ViewId, rows: &[(StyleId, StyleValue)]) -> Op {
    let mut patch = StyleProps::default();
    for (row, value) in rows {
        patch.set_dynamic(*row, value).unwrap();
    }
    Op::SetStyle {
        id,
        patch: Box::new(patch),
    }
}
fn view(id: ViewId, node_type: NodeType, rows: &[(StyleId, StyleValue)]) -> [Op; 2] {
    [Op::CreateView { id, node_type }, style(id, rows)]
}
fn text(id: ViewId, value: &str) -> [Op; 2] {
    let prop = Op::SetProp {
        id,
        prop: PropId::Text,
        value: value.into(),
    };
    [
        Op::CreateView {
            id,
            node_type: NodeType::Text,
        },
        prop,
    ]
}
fn children(id: ViewId, children: impl IntoIterator<Item = ViewId>) -> Op {
    Op::SetChildren {
        id,
        children: children.into_iter().collect(),
    }
}
fn is(width: f32, height: f32) -> Option<HostBlock> {
    Some(HostBlock {
        width: BlockAxis::Is(width),
        height: BlockAxis::Is(height),
    })
}
fn at_most(width: f32, height: f32) -> Option<HostBlock> {
    Some(HostBlock {
        width: BlockAxis::AtMost(width),
        height: BlockAxis::AtMost(height),
    })
}
const FILL: [(StyleId, StyleValue); 7] = [
    (
        StyleId::PositionType,
        StyleValue::Text(std::borrow::Cow::Borrowed("absolute")),
    ),
    (StyleId::Left, StyleValue::Number(0.0)),
    (StyleId::Right, StyleValue::Number(0.0)),
    (StyleId::Top, StyleValue::Number(0.0)),
    (StyleId::Bottom, StyleValue::Number(0.0)),
    (
        StyleId::Display,
        StyleValue::Text(std::borrow::Cow::Borrowed("flex")),
    ),
    (
        StyleId::FlexDirection,
        StyleValue::Text(std::borrow::Cow::Borrowed("column")),
    ),
];

/// A page of a title, a stack holding three routes and a footer; each
/// route a column of wrapping paragraphs: two panes' and a sheet's.
const ROWS: u32 = 30;
const ROUTES: [(ViewId, char); 3] = [(10, 'A'), (20, 'B'), (30, 'S')];
fn rows_of(route: ViewId) -> std::ops::Range<ViewId> {
    route * 100..route * 100 + ROWS
}
fn page(counts: &Rc<RefCell<BTreeMap<char, usize>>>) -> Kernel {
    let mut k = Kernel::new(Box::new(Count(counts.clone())));
    let mut ops = Vec::new();
    let column = [
        (StyleId::Display, t("flex")),
        (StyleId::FlexDirection, t("column")),
    ];
    ops.extend(view(1, NodeType::View, &column));
    ops.push(style(1, &[(StyleId::Height, StyleValue::Percent(100.0))]));
    ops.extend(text(2, "Page title, the page's own"));
    let stack = [
        (StyleId::FlexGrow, n(1.0)),
        (StyleId::PositionType, t("relative")),
    ];
    ops.extend(view(3, NodeType::View, &stack));
    ops.extend(text(4, "Page footer"));
    for (route, letter) in ROUTES {
        ops.extend(view(route, NodeType::View, &FILL));
        for row in rows_of(route) {
            let words = "words that wrap at a pane's width and not at a wide one";
            ops.extend(text(row, &format!("{letter}{row} {words}")));
        }
        ops.push(children(route, rows_of(route)));
    }
    ops.extend([
        children(3, ROUTES.map(|r| r.0)),
        children(1, [2, 3, 4]),
        Op::AttachRoot { id: 1 },
    ]);
    k.apply(0, 1, &ops).unwrap();
    k
}

/// D8.3: when one block's constraints change, the other subtrees and the
/// page around them are not laid out again. Counted four ways: paragraphs
/// measured (by subtree), leaves the engine laid out, nodes publication
/// visited, and the nodes the receipt names (what `perf` counts as moved).
#[test]
fn a_block_that_changes_lays_out_its_subtree_alone() {
    let counts = Rc::new(RefCell::new(BTreeMap::new()));
    let mut k = page(&counts);
    k.set_host_block(10, is(300.0, 500.0)).unwrap();
    k.set_host_block(20, is(480.0, 500.0)).unwrap();
    k.set_host_block(30, at_most(400.0, 300.0)).unwrap();
    let step = |k: &mut Kernel, what: &str, act: &mut dyn FnMut(&mut Kernel)| {
        counts.borrow_mut().clear();
        let before = k.tree().work;
        act(k);
        let receipt = k.compute_layout(1, OFFER).unwrap();
        let after = k.tree().work;
        let measured = counts.borrow().clone();
        // Which route each named node is under ('P': the page's own).
        let mut named = BTreeMap::<char, usize>::new();
        for key in &receipt.updated {
            let id = k.node_by_key(*key).unwrap().id;
            let route = ROUTES
                .iter()
                .find(|(r, _)| id == *r || rows_of(*r).contains(&id));
            *named.entry(route.map_or('P', |r| r.1)).or_default() += 1;
        }
        let (visited, leaves) = (after.0 - before.0, after.1 - before.1);
        println!("{what}: measured {measured:?}, leaves laid out {leaves}, published {visited}, named {named:?}");
        (measured, leaves, visited, named)
    };
    let only = |map: &BTreeMap<char, usize>, letter: char| {
        map.keys().copied().collect::<Vec<_>>() == vec![letter]
    };
    let nodes = k.live_count();
    let (measured, _, visited, _) = step(&mut k, "first layout", &mut |_| {});
    assert_eq!(
        measured.len(),
        4,
        "every subtree's text is measured once: {measured:?}"
    );
    assert_eq!(visited, nodes);
    let still = step(&mut k, "nothing changed", &mut |_| {});
    assert_eq!(still, (BTreeMap::new(), 0, 0, BTreeMap::new()));
    // A divider moves: pane A is narrower, pane B is as it was.
    let (measured, leaves, visited, named) = step(&mut k, "pane A 300 to 240", &mut |k| {
        k.set_host_block(10, is(240.0, 500.0)).unwrap();
    });
    assert!(
        only(&measured, 'A') && only(&named, 'A'),
        "{measured:?} {named:?}"
    );
    assert!(
        leaves >= ROWS as usize && leaves <= 6 * ROWS as usize,
        "{leaves}"
    );
    assert_eq!(visited, ROWS as usize + 1, "pane A's rows and its root");
    assert_eq!(k.host_block_size(10), Some((240.0, 500.0)));
    // The same size again is no change at all.
    let same = step(&mut k, "pane A 240 again", &mut |k| {
        k.set_host_block(10, is(240.0, 500.0)).unwrap();
    });
    assert_eq!(same, (BTreeMap::new(), 0, 0, BTreeMap::new()));
    // The sheet's content grows: the sheet's subtree, and its used size.
    let before = k.host_block_size(30).unwrap();
    let (measured, leaves, _, named) = step(&mut k, "a sheet row's text grows", &mut |k| {
        let longer = "S and now three lines of it, ".repeat(6);
        let grow = Op::SetProp {
            id: 3003,
            prop: PropId::Text,
            value: longer.into(),
        };
        k.apply(0, 2, &[grow]).unwrap();
    });
    assert!(
        only(&measured, 'S') && only(&named, 'S'),
        "{measured:?} {named:?}"
    );
    assert!(leaves > 0 && leaves <= 13 * ROWS as usize, "{leaves}");
    let after = k.host_block_size(30).unwrap();
    assert!(
        after.1 > before.1 || after.1 == 300.0,
        "{before:?} {after:?}"
    );
    // The page's own text changes: no block's subtree is touched.
    let (measured, _, _, named) = step(&mut k, "the page's title changes", &mut |k| {
        let title = Op::SetProp {
            id: 2,
            prop: PropId::Text,
            value: "Page title, changed and longer than it was".into(),
        };
        k.apply(0, 3, &[title]).unwrap();
    });
    assert!(
        only(&measured, 'P') && named.keys().all(|l| *l == 'P'),
        "{measured:?} {named:?}"
    );
    // The window resizes: the page, and no pane.
    let (measured, _, _, named) = {
        counts.borrow_mut().clear();
        let receipt = k.compute_layout(1, Offer::definite(900.0, 700.0)).unwrap();
        let in_route = |key: &NodeKey| {
            let id = k.node_by_key(*key).unwrap().id;
            ROUTES
                .iter()
                .any(|(r, _)| id == *r || rows_of(*r).contains(&id))
        };
        let named = receipt.updated.iter().filter(|key| in_route(key)).count();
        println!(
            "window 800x600 to 900x700: measured {:?}, block nodes named {named}",
            counts.borrow()
        );
        (counts.borrow().clone(), 0, 0, named)
    };
    assert!(
        measured.keys().all(|l| *l == 'P') && named == 0,
        "{measured:?} {named}"
    );
}

fn laid_out(ops: Vec<Op>) -> Kernel {
    let mut k = Kernel::with_monospace();
    k.apply(0, 1, &ops).unwrap();
    k.compute_layout(1, OFFER).unwrap();
    k
}

/// A root, a holder and a route with one child, 150 by 40.
fn small() -> Vec<Op> {
    let half = [(StyleId::Width, n(150.0)), (StyleId::Height, n(40.0))];
    let mut ops = Vec::new();
    ops.extend(view(1, NodeType::View, &[(StyleId::Height, n(600.0))]));
    ops.extend(view(2, NodeType::View, &[(StyleId::Height, n(100.0))]));
    ops.extend(view(3, NodeType::View, &[]));
    ops.extend(view(4, NodeType::View, &half));
    ops.extend([
        children(3, [4]),
        children(2, [3]),
        children(1, [2]),
        Op::AttachRoot { id: 1 },
    ]);
    ops
}

#[test]
fn a_hidden_root_lays_nothing_out_and_its_used_size_is_zero() {
    let mut k = laid_out(small());
    k.set_host_block(3, at_most(300.0, 200.0)).unwrap();
    k.compute_layout(1, OFFER).unwrap();
    let shown = k.node(4).unwrap().frame;
    assert_eq!(k.host_block_size(3), Some((150.0, 40.0)));
    assert_eq!((shown.width, shown.height), (150.0, 40.0));
    let hidden = |k: &Kernel, id| {
        k.arena
            .flags(k.arena.slot_of(id).unwrap())
            .has(NodeFlags::HIDDEN)
    };
    // Its own `display: none`, then an ancestor's, which touches no node of
    // the subtree: both hide it, and showing again lays it out as it was.
    for owner in [3, 2] {
        k.apply(0, 2, &[style(owner, &[(StyleId::Display, t("none"))])])
            .unwrap();
        let receipt = k.compute_layout(1, OFFER).unwrap();
        assert!(hidden(&k, 3) && hidden(&k, 4), "hidden by {owner}");
        assert_eq!(k.host_block_size(3), Some((0.0, 0.0)));
        assert_eq!(k.node(4).unwrap().frame.width, 0.0);
        assert!(receipt.updated.contains(&k.node(3).unwrap().key));
        assert_eq!(k.laid_out_frame(k.node(4).unwrap().key), None);
        k.apply(0, 3, &[style(owner, &[(StyleId::Display, t("block"))])])
            .unwrap();
        let receipt = k.compute_layout(1, OFFER).unwrap();
        assert!(!hidden(&k, 3) && !hidden(&k, 4));
        assert_eq!(k.host_block_size(3), Some((150.0, 40.0)));
        assert!(k.node(4).unwrap().frame.bits_eq(shown));
        assert!(receipt.updated.contains(&k.node(3).unwrap().key));
    }
}

#[test]
fn a_cover_applies_inside_a_block() {
    // What a bar covers of a route is padding inside the route's block, as
    // an iPad column's bar will be: the same boxes as that padding written.
    let mut covered = laid_out(small());
    covered.set_host_block(3, is(300.0, 200.0)).unwrap();
    covered
        .set_host_cover(3, Some(HostCover::Edges([44.0, 0.0, 0.0, 10.0])))
        .unwrap();
    covered.compute_layout(1, OFFER).unwrap();
    let mut written = small();
    written.push(style(
        3,
        &[
            (StyleId::PaddingTop, n(44.0)),
            (StyleId::PaddingLeft, n(10.0)),
        ],
    ));
    let mut written = laid_out(written);
    written.set_host_block(3, is(300.0, 200.0)).unwrap();
    written.compute_layout(1, OFFER).unwrap();
    for id in [3, 4] {
        let (a, b) = (
            covered.node(id).unwrap().frame,
            written.node(id).unwrap().frame,
        );
        assert!(a.bits_eq(b), "{id}: {a:?} {b:?}");
    }
    assert_eq!(covered.node(4).unwrap().frame.y, 44.0);
}

#[test]
fn refusals_and_a_destroyed_root() {
    let mut k = laid_out(small());
    let refused = |r: Result<(), KernelError>| matches!(r, Err(KernelError::Layout(_)));
    assert!(
        refused(k.set_host_block(1, is(10.0, 10.0))),
        "a root has its offer"
    );
    assert!(refused(k.set_host_block(99, is(10.0, 10.0))));
    assert!(refused(k.set_host_block(3, is(f32::NAN, 10.0))));
    assert!(refused(k.set_host_block(3, at_most(10.0, -1.0))));
    assert_eq!(k.host_block(3), None);
    k.set_host_block(3, is(300.0, 200.0)).unwrap();
    assert_eq!(k.host_block(3), is(300.0, 200.0));
    // A viewport unit is the window's whatever block the node is in.
    k.apply(0, 2, &[style(4, &[(StyleId::Width, t("50vw"))])])
        .unwrap();
    k.compute_layout(1, OFFER).unwrap();
    assert_eq!(k.node(4).unwrap().frame.width, 400.0);
    assert_eq!(k.env().viewport_width, 800.0);
    // Reads of what layout did are in the block's space, from its root.
    let key = k.node(4).unwrap().key;
    let (frame, _) = k.measure_auto_height(key).unwrap();
    assert_eq!((frame.x, frame.y, frame.width), (0.0, 0.0, 400.0));
    assert_eq!(k.fit_content_height(k.node(3).unwrap().key), None);
    // A node that is destroyed takes its block with it.
    k.apply(0, 3, &[children(2, []), Op::DestroyView { id: 3 }])
        .unwrap();
    assert_eq!(k.host_block(3), None);
    assert!(!k.arena.blocks.any());
    k.compute_layout(1, OFFER).unwrap();
}

#[test]
fn an_authored_height_is_measured_inside_a_block_while_another_is_presented() {
    let mut ops = small();
    ops.push(style(4, &[(StyleId::BoxSizing, t("border-box"))]));
    let mut k = laid_out(ops);
    k.set_host_block(3, at_most(300.0, 200.0)).unwrap();
    let key = k.node(4).unwrap().key;
    let shown = PresentedHeight {
        node: key,
        epoch: k.epoch(),
        px: 90.0,
    };
    k.compute_layout_presented(1, OFFER, &[shown]).unwrap();
    assert_eq!(k.node(4).unwrap().frame.height, 90.0);
    assert_eq!(k.host_block_size(3), Some((150.0, 90.0)));
    let targets = k.measure_height_targets(1, OFFER, &[key]).unwrap();
    assert_eq!(
        targets[0].px, 40.0,
        "the authored height, read in its block's layout"
    );
    assert_eq!(
        k.node(4).unwrap().frame.height,
        90.0,
        "and nothing published"
    );
}
