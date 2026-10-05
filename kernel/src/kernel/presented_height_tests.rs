use super::*;
use crate::{Dimension, PresentedHeight};

#[test]
fn engine_fault_rebuild_reapplies_projection_and_equal_sample_stays_clean() {
    let mut k = Kernel::with_monospace();
    let mut style = StyleProps::default();
    style.height = Dimension::Points(180.0);
    style.mask.set(StyleId::Height);
    style.box_sizing = BoxSizing::BorderBox;
    style.mask.set(StyleId::BoxSizing);
    k.apply(
        0,
        1,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::SetStyle {
                id: 1,
                patch: Box::new(style.clone()),
            },
            Op::CreateView {
                id: 2,
                node_type: NodeType::View,
            },
            Op::SetStyle {
                id: 2,
                patch: Box::new(style),
            },
            Op::SetChildren {
                id: 1,
                children: vec![2],
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    let p = PresentedHeight {
        node: k.node(1).unwrap().key,
        epoch: k.epoch(),
        px: 320.0,
    };
    let samples = [
        p,
        PresentedHeight {
            node: k.node(2).unwrap().key,
            px: 90.0,
            ..p
        },
    ];
    let offer = Offer::definite(400.0, 600.0);
    k.compute_layout_presented(1, offer, &samples).unwrap();
    // Missing derived state exercises the real Engine error/rebuild path,
    // without changing the authored columns or a production test hook.
    k.arena.set_taffy(p.node.index, None);
    k.compute_layout_presented(1, offer, &samples).unwrap();
    assert!(!k.tree().faulted());
    assert_eq!(k.node(1).unwrap().frame.height, 320.0);
    assert_eq!(k.node(2).unwrap().frame.height, 90.0);
    let node = k.arena.taffy(p.node.index).unwrap();
    assert!(!k.tree().is_dirty(node));
    let arena = k.arena.clone();
    k.tree_mut().present_heights(&arena, &samples);
    assert!(
        !k.tree().is_dirty(node),
        "equal projection must not call Taffy set_style"
    );
    // Target measurement recovers the derived tree without publishing the
    // authored endpoint, and restores every active sample after rebuilding.
    let before = k.export(None).unwrap();
    k.arena.set_taffy(p.node.index, None);
    let owners = samples.map(|p| p.node);
    let targets = k.measure_height_targets(1, offer, &owners).unwrap();
    assert_eq!(
        targets.iter().map(|p| p.px).collect::<Vec<_>>(),
        vec![180.0, 180.0]
    );
    assert_eq!(k.export(None).unwrap(), before);
    assert_eq!(k.tree().height_samples(k.epoch()), samples);
    k.arena.set_taffy(samples[1].node.index, None);
    assert_eq!(
        k.measure_height_targets(1, offer, &owners).unwrap(),
        targets
    );
    assert_eq!(k.export(None).unwrap(), before);
    assert_eq!(k.tree().height_samples(k.epoch()), samples);
    assert!(k
        .compute_layout_presented(1, offer, &samples)
        .unwrap()
        .changed
        .is_empty());
    let node = k.arena.taffy(p.node.index).unwrap();
    let mut style = k.arena.style(p.node.index).clone();
    style.height = Dimension::Points(400.0);
    k.apply(
        0,
        2,
        &[Op::SetStyle {
            id: 1,
            patch: Box::new(style),
        }],
    )
    .unwrap();
    assert!(
        !k.tree().is_dirty(node),
        "central authored write preserves identical derived height"
    );
    k.compute_layout(1, offer).unwrap();
    assert_eq!(k.node(1).unwrap().frame.height, 400.0);
    assert_eq!(k.node(2).unwrap().frame.height, 180.0);
}
