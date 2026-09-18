//! Full parsed documents with only viewport-sized rich-text block realization.
use exact_kernel::{Kernel, Offer};
use exact_plan::Value;
use exact_runner::{CollectionFeedback, RowMeasurement, Runner};
use markdown_stress_data::MarkdownStress;

fn boot() -> Runner<MarkdownStress> {
    Runner::boot(
        contract::compile(include_str!("../../app.contract")).unwrap(),
        MarkdownStress::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn feedback(runner: &mut Runner<MarkdownStress>, top: f64, width: f64) {
    // A first width change invalidates old measurements; a following layout
    // supplies measurements carrying the newly published epochs.
    let sequence = runner.collections()[0].scroll_sequence + 1;
    let mut top = top;
    for _ in 0..12 {
        let root = runner.roots()[0];
        runner
            .kernel_mut()
            .compute_layout(root, Offer::definite(width as f32, 800.0))
            .unwrap();
        let snapshot = runner.collections().remove(0);
        let port = runner.kernel().node(snapshot.view).unwrap().frame;
        let row_width = snapshot.rows.first().map_or(port.width, |row| {
            runner.kernel().node(row.view).unwrap().frame.width
        });
        if let Some(correction) = snapshot.correction {
            if correction.scroll_sequence == sequence {
                top = correction.scroll_top;
            }
        }
        top = top.clamp(0.0, (snapshot.total_extent - port.height as f64).max(0.0));
        let measurements = snapshot
            .rows
            .iter()
            .map(|row| RowMeasurement {
                view: row.view,
                epoch: row.epoch,
                height: runner.kernel().node(row.view).unwrap().frame.height as f64,
            })
            .collect();
        let receipt = runner
            .collection_feedback(CollectionFeedback {
                view: snapshot.view,
                revision: snapshot.revision,
                scroll_sequence: sequence,
                scroll_top: top,
                port_width: port.width as f64,
                port_height: port.height as f64,
                row_width: row_width as f64,
                measurements,
                focus_view: None,
                interaction_view: None,
            })
            .unwrap();
        if receipt.is_none() {
            let visible: Vec<_> = snapshot
                .rows
                .iter()
                .filter(|row| row.top + row.height > top && row.top < top + port.height as f64)
                .collect();
            assert!(!visible.is_empty(), "no block covers the settled viewport");
            assert!(visible[0].top <= top + 0.01);
            assert!(
                visible.last().unwrap().top + visible.last().unwrap().height
                    >= (top + port.height as f64).min(snapshot.total_extent) - 0.01
            );
            assert!(visible
                .windows(2)
                .all(|rows| rows[0].index + 1 == rows[1].index));
            return;
        }
    }
    panic!("collection feedback did not settle within twelve passes");
}

#[test]
fn full_megabyte_document_keeps_bounded_blocks_through_scroll_resize_and_reparse() {
    let mut runner = boot();
    runner
        .act("chooseProfile", vec![Value::str("blocks")])
        .unwrap();
    runner
        .act("chooseSize", vec![Value::Number(1_048_576.0)])
        .unwrap();
    runner.act("toggleWindowed", vec![]).unwrap();
    let count = runner.collections()[0].count;
    assert!(count > 25_000);
    assert!(runner.collections()[0].rows.len() <= 16);
    for (top, width) in [
        (0.0, 640.0),
        (160_000.0, 640.0),
        (480_000.0, 390.0),
        (0.0, 900.0),
    ] {
        feedback(&mut runner, top, width);
        let snapshot = &runner.collections()[0];
        assert_eq!(snapshot.count, count);
        assert!(
            snapshot.rows.len() < 100,
            "{} live blocks",
            snapshot.rows.len()
        );
        assert!(runner.kernel().arena().live_count() < 1_500);
    }
    runner
        .act(
            "editDraft",
            vec![Value::str("Still reading after resize 🦀")],
        )
        .unwrap();
    runner.act("reparse", vec![]).unwrap();
    feedback(&mut runner, 0.0, 900.0);
    assert_eq!(runner.collections()[0].count, count);
    assert_eq!(
        runner.slot("draft"),
        Some(&Value::str("Still reading after resize 🦀"))
    );
    assert_eq!(runner.slot("revision"), Some(&Value::Number(1.0)));
    runner.act("reset", vec![]).unwrap();
    assert!(runner.collections().is_empty());
}

#[test]
fn windowing_preserves_giant_single_blocks_and_manual_and_eager_controls() {
    let mut runner = boot();
    runner
        .act("chooseProfile", vec![Value::str("paragraph")])
        .unwrap();
    runner
        .act("chooseSize", vec![Value::Number(262_144.0)])
        .unwrap();
    runner.act("toggleWindowed", vec![]).unwrap();
    // The complete paragraph stays one block: no truncation or hidden paging.
    assert_eq!(runner.collections()[0].count, 2);
    assert_eq!(runner.collections()[0].rows.len(), 2);
    let key = runner.kernel().find_by_test_id("block-1")[0];
    let root = runner.kernel().node_by_key(key).unwrap();
    assert!(root.text_runs().iter().map(|r| r.text.len()).sum::<usize>() > 250_000);
    runner.act("toggleEager", vec![]).unwrap();
    assert!(runner.collections().is_empty());
    assert_eq!(runner.slot("windowed"), Some(&Value::Bool(false)));
    assert_eq!(runner.slot("eager"), Some(&Value::Bool(true)));
    runner.act("toggleWindowed", vec![]).unwrap();
    assert_eq!(runner.slot("eager"), Some(&Value::Bool(false)));
    runner.act("toggleWindowed", vec![]).unwrap();
    assert!(runner.collections().is_empty());
    assert_eq!(runner.kernel().find_by_test_id("next-page").len(), 1);
}
