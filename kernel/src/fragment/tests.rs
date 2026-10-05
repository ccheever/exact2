//! The cut's rules on hand-built flows: Multicol §3.4's arithmetic, Chrome's
//! break appeal, and its balancer (LLP 1093 D1, D4, D5). Chrome's own
//! results, on real trees, are `kernel/tests/it/browser_columns.rs`.

use super::build::{Atom, Flow, Para};
use super::cut::{Cutter, Pos};
use crate::arena::NodeArena;
use crate::text::MonospaceMeasurer;

fn columns(count: Option<u16>, width: Option<f32>, gap: f32, available: f32) -> (u32, f32) {
    taffy::Multicol {
        count,
        width,
        gap,
        used_height: None,
        overflow: None,
    }
    .columns(available)
}

#[test]
fn the_used_count_and_width_are_multicol_3_4s() {
    // A count alone divides the width less its gaps.
    assert_eq!(columns(Some(3), None, 10.0, 320.0), (3, 100.0));
    // A width alone fits as many as it can, and stretches them.
    assert_eq!(columns(None, Some(80.0), 10.0, 260.0), (3, 80.0));
    assert_eq!(
        columns(None, Some(80.0), 10.0, 300.0),
        (3, 310.0 / 3.0 - 10.0)
    );
    // Both: the count caps what the width fits.
    assert_eq!(columns(Some(3), Some(50.0), 10.0, 320.0), (3, 100.0));
    // A width wider than the box is one column of the box's width.
    assert_eq!(columns(None, Some(300.0), 10.0, 260.0), (1, 260.0));
    // Never a negative width.
    assert_eq!(columns(Some(4), None, 40.0, 60.0), (4, 0.0));
}

fn atom(top: f32, bottom: f32, para: Option<usize>) -> Atom {
    Atom {
        top,
        bottom,
        para,
        margin: 0.0,
        forced: false,
        avoid: false,
        refusal: None,
    }
}

// A paragraph of `lines` 20px lines at `top`, its lines already known.
fn para(flow: &mut Flow, top: f32, lines: usize, orphans: u32, widows: u32) {
    let index = flow.paras.len();
    flow.paras.push(Para {
        slot: 0,
        content_top: top,
        width: 100.0,
        orphans,
        widows,
        avoid: false,
        lines: Some((1..=lines).map(|i| i as f32 * 20.0).collect()),
    });
    flow.atoms
        .push(atom(top, top + 20.0 * lines as f32, Some(index)));
}

fn starts(flow: Flow, h: f32) -> Vec<(usize, usize)> {
    let arena = NodeArena::default();
    let mut measurer = MonospaceMeasurer::default();
    let mut cutter = Cutter::new(flow, &arena, &mut measurer, 0.0);
    let walk = cutter.walk(h);
    walk.starts.iter().map(|(p, _)| (p.atom, p.line)).collect()
}

#[test]
fn widows_move_the_break_to_the_latest_perfect_one() {
    // Six lines, `widows: 4`, in 80px: lines 1–2, then 3–6 (Chrome 154).
    let mut flow = Flow::default();
    para(&mut flow, 0.0, 6, 2, 4);
    assert_eq!(starts(flow, 80.0), vec![(0, 0), (0, 2)]);
}

#[test]
fn a_short_paragraph_splits_at_the_last_line_that_fits() {
    // A 40px box, then three lines in 80px: `widows` does not score when the
    // paragraph is shorter than orphans + widows, so 2 + 1.
    let mut flow = Flow::default();
    flow.atoms.push(atom(0.0, 40.0, None));
    para(&mut flow, 40.0, 3, 2, 2);
    assert_eq!(starts(flow, 80.0), vec![(0, 0), (1, 2)]);
}

#[test]
fn fewer_than_orphans_lines_move_the_paragraph() {
    let mut flow = Flow::default();
    flow.atoms.push(atom(0.0, 80.0, None));
    para(&mut flow, 80.0, 3, 2, 2);
    assert_eq!(starts(flow, 100.0), vec![(0, 0), (1, 0)]);
}

#[test]
fn a_widows_violation_beats_an_avoid_violation() {
    // Three lines with `break-after: avoid`, then a 50px box, in 100px: the
    // box stays whole, after the paragraph's last line (Chrome 154).
    let mut flow = Flow::default();
    para(&mut flow, 0.0, 3, 2, 2);
    let mut figure = atom(60.0, 110.0, None);
    figure.avoid = true;
    flow.atoms.push(figure);
    assert_eq!(starts(flow, 100.0), vec![(0, 0), (0, 2)]);
}

#[test]
fn a_forced_break_ends_the_column_and_keeps_the_next_margin() {
    let mut flow = Flow::default();
    flow.atoms.push(atom(0.0, 20.0, None));
    let mut next = atom(70.0, 90.0, None);
    next.forced = true;
    next.margin = 10.0;
    flow.atoms.push(next);
    let arena = NodeArena::default();
    let mut measurer = MonospaceMeasurer::default();
    let mut cutter = Cutter::new(flow, &arena, &mut measurer, 0.0);
    let walk = cutter.walk(100.0);
    assert_eq!(walk.starts[1], (Pos { atom: 1, line: 0 }, 60.0));
    assert_eq!(walk.shortage, None);
}

#[test]
fn the_balancer_stretches_by_the_shortage_the_early_break_makes() {
    // Six lines, `widows: 4`, two columns: 60px would need a third column,
    // so the balancer stretches to 80px (2 + 4), as Chrome does.
    let mut flow = Flow::default();
    para(&mut flow, 0.0, 6, 2, 4);
    let arena = NodeArena::default();
    let mut measurer = MonospaceMeasurer::default();
    let mut cutter = Cutter::new(flow, &arena, &mut measurer, 0.0);
    let (h, walk) = cutter.balance(2, 120.0);
    assert_eq!(h, 80.0);
    assert_eq!(walk.starts.len(), 2);
    // Eight lines, `orphans: 3; widows: 3`, three columns stay at 60px
    // (3 + 3 + 2): that split fits, and none is perfect.
    let mut flow = Flow::default();
    para(&mut flow, 0.0, 8, 3, 3);
    let mut cutter = Cutter::new(flow, &arena, &mut measurer, 0.0);
    assert_eq!(cutter.balance(3, 160.0).0, 60.0);
}

#[test]
fn forced_breaks_set_the_first_guess() {
    // Four one-line runs between forced breaks in two columns: each run is a
    // column, and the guess is the tallest run, not the content over two.
    let mut flow = Flow::default();
    for i in 0..4 {
        let mut a = atom(i as f32 * 20.0, i as f32 * 20.0 + 20.0, None);
        a.forced = i > 0;
        flow.atoms.push(a);
    }
    let arena = NodeArena::default();
    let mut measurer = MonospaceMeasurer::default();
    let mut cutter = Cutter::new(flow, &arena, &mut measurer, 0.0);
    let (h, walk) = cutter.balance(2, 80.0);
    assert_eq!((h, walk.starts.len()), (20.0, 4));
}

// A chapter of `n` paragraphs of two to five lines under a container with
// `rows`, in a 400px-wide page.
fn chapter(n: usize, rows: &[(crate::StyleId, crate::StyleValue)]) -> crate::Kernel {
    use crate::{NodeType, Offer, Op, PropId, StyleId, StyleProps, StyleValue};
    let mut root = StyleProps::default();
    root.set_dynamic(StyleId::Width, &StyleValue::Number(400.0))
        .unwrap();
    let mut flow = StyleProps::default();
    for (row, value) in rows {
        flow.set_dynamic(*row, value).unwrap();
    }
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(root),
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(flow),
        },
    ];
    let mut children = Vec::new();
    for i in 0..n {
        let id = 3 + i as u32;
        let words = "lorem ipsum dolor sit amet ".repeat(2 + i % 4 * 2);
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::Text,
        });
        ops.push(Op::SetProp {
            id,
            prop: PropId::Text,
            value: words.trim().into(),
        });
        children.push(id);
    }
    ops.push(Op::SetChildren { id: 2, children });
    ops.push(Op::SetChildren {
        id: 1,
        children: vec![2],
    });
    ops.push(Op::AttachRoot { id: 1 });
    let mut k = crate::Kernel::with_monospace();
    k.apply(0, 1, &ops).unwrap();
    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    k
}

/// LLP 1093 §3, owed by stage 1: the cut of a 40-column chapter, that
/// chapter's layout against the same chapter as one column, a balanced
/// three-column container's walk count, and a one-word edit in the last
/// column. Printed, never checked.
#[test]
#[ignore = "async lane: LLP 1093 §3's multicol probe, ~2 s; bun scripts/async.mjs runs it"]
fn multicol_probe() {
    use crate::{StyleId, StyleValue};
    use std::time::Instant;
    let n = 400;
    let paged = [
        (StyleId::Height, StyleValue::Number(800.0)),
        (StyleId::ColumnWidth, StyleValue::Number(400.0)),
        (StyleId::ColumnGap, StyleValue::Number(40.0)),
        (StyleId::ColumnFill, StyleValue::Text("auto".into())),
    ];
    let best = |f: &mut dyn FnMut() -> f64| (0..7).map(|_| f()).fold(f64::INFINITY, f64::min);
    let single = best(&mut || {
        let t = Instant::now();
        chapter(n, &[]);
        t.elapsed().as_secs_f64() * 1e3
    });
    let columns = best(&mut || {
        let t = Instant::now();
        chapter(n, &paged);
        t.elapsed().as_secs_f64() * 1e3
    });
    let mut k = chapter(n, &paged);
    let count = k.columns(k.node(2).unwrap().key).unwrap().columns.len();
    let cut = best(&mut || {
        let (arena, tree, measurer) = k.parts();
        let t = Instant::now();
        super::cut::container(arena, tree, measurer, arena.slot_of(2).unwrap());
        t.elapsed().as_secs_f64() * 1e3
    });
    let last = 2 + n as u32;
    let edit = best(&mut || {
        let ops = [crate::Op::SetProp {
            id: last,
            prop: crate::PropId::Text,
            value: format!("word {}", Instant::now().elapsed().as_nanos()).into(),
        }];
        k.apply(0, 2, &ops).unwrap();
        let t = Instant::now();
        k.compute_layout(1, crate::Offer::definite(400.0, 400.0))
            .unwrap();
        t.elapsed().as_secs_f64() * 1e3
    });
    let mut balanced = chapter(
        n,
        &[
            (StyleId::ColumnCount, StyleValue::Number(3.0)),
            (StyleId::ColumnGap, StyleValue::Number(20.0)),
        ],
    );
    let (arena, tree, measurer) = balanced.parts();
    let walks = super::cut::container(arena, tree, measurer, arena.slot_of(2).unwrap()).walks;
    println!("multicol probe: {n} paragraphs, {count} columns");
    println!(
        "  layout as one column {single:.2} ms, as columns {columns:.2} ms (+{:.0}%)",
        (columns / single - 1.0) * 100.0
    );
    println!("  one cut {cut:.3} ms; a one-word edit in the last column {edit:.3} ms");
    println!("  balanced into 3 columns: {walks} walks");
}

#[test]
fn a_paragraph_the_host_cannot_break_is_never_cut_shorter() {
    // A host with no line boxes (Apple, before its hook) keeps a paragraph
    // whole: the balanced height is at least that paragraph's, as Chrome's
    // is never below its tallest unbreakable piece.
    let mut flow = Flow::default();
    flow.paras.push(Para {
        slot: 0,
        content_top: 0.0,
        width: 100.0,
        orphans: 2,
        widows: 2,
        avoid: false,
        lines: Some(Vec::new()),
    });
    flow.atoms.push(atom(0.0, 160.0, Some(0)));
    flow.atoms.push(atom(160.0, 220.0, None));
    let arena = NodeArena::default();
    let mut measurer = MonospaceMeasurer::default();
    let mut cutter = Cutter::new(flow, &arena, &mut measurer, 0.0);
    let (h, walk) = cutter.balance(3, 220.0);
    assert_eq!((h, walk.starts.len()), (160.0, 2));
}
