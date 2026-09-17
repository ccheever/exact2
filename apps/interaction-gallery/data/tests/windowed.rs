//! Real gallery Contract, complete records and actual nested viewport geometry.
use exact_kernel::{Kernel, Offer, PropId};
use exact_plan::Value;
use exact_runner::{CollectionFeedback, Event, RowMeasurement, Runner};
use interaction_gallery_data::Gallery;
use std::rc::Rc;

fn boot() -> Runner<Gallery> {
    Runner::boot(
        contract::compile(include_str!("../../app.contract")).unwrap(),
        Gallery::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}
fn press(r: &mut Runner<Gallery>, name: &str) {
    let keys = r.kernel().find_by_test_id(name);
    assert_eq!(keys.len(), 1, "missing control {name}");
    let id = r.kernel().node_by_key(keys[0]).unwrap().id;
    r.dispatch(id, Event::Press).unwrap();
}
fn rows(r: &Runner<Gallery>) -> Rc<Vec<Value>> {
    let Value::List(rows) = r.resource("rows").expect("separate stable row resource") else {
        panic!("rows must be a list")
    };
    rows.clone()
}
fn id(row: &Value) -> &str {
    let Value::Record(fields) = row else {
        panic!("record")
    };
    fields[0].as_str().unwrap()
}
fn text(r: &Runner<Gallery>, name: &str) -> String {
    let key = r.kernel().find_by_test_id(name)[0];
    r.kernel()
        .node_by_key(key)
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
        .into()
}

// The same geometry supplied by native/browser adapters: real nested port and
// measured wrappers. A bounded loop accepts anchor corrections after reflow.
fn settle(r: &mut Runner<Gallery>, requested: f64, width: f64) -> (f64, f64) {
    let sequence = r.collections()[0].scroll_sequence + 1;
    let mut top = requested;
    for _ in 0..16 {
        let root = r.roots()[0];
        r.kernel_mut()
            .compute_layout(root, Offer::definite(width as f32, 860.0))
            .unwrap();
        let c = r.collections().remove(0);
        let port = r.kernel().node(c.view).unwrap().frame;
        if let Some(correction) = c.correction.filter(|c| c.scroll_sequence == sequence) {
            top = correction.scroll_top;
        }
        top = top.clamp(0.0, (c.total_extent - port.height as f64).max(0.0));
        let facts = CollectionFeedback {
            view: c.view,
            revision: c.revision,
            scroll_sequence: sequence,
            scroll_top: top,
            port_width: port.width as f64,
            port_height: port.height as f64,
            row_width: c.rows.first().map_or(port.width, |row| {
                r.kernel().node(row.view).unwrap().frame.width
            }) as f64,
            measurements: c
                .rows
                .iter()
                .map(|row| RowMeasurement {
                    view: row.view,
                    epoch: row.epoch,
                    height: r.kernel().node(row.view).unwrap().frame.height as f64,
                })
                .collect(),
            focus_view: None,
            interaction_view: None,
        };
        if r.collection_feedback(facts).unwrap().is_none() {
            let visible: Vec<_> = c
                .rows
                .iter()
                .filter(|row| row.top + row.height > top && row.top < top + port.height as f64)
                .collect();
            assert!(!visible.is_empty());
            assert!(visible[0].top <= top + 0.1);
            assert!(
                visible.last().unwrap().top + visible.last().unwrap().height
                    >= (top + port.height as f64).min(c.total_extent) - 0.5
            );
            assert!(visible.windows(2).all(|w| w[0].index + 1 == w[1].index));
            assert!(
                c.rows.len() < 100,
                "unbounded realization: {}",
                c.rows.len()
            );
            assert!(r.kernel().arena().live_count() < 2_000);
            return (top, port.height as f64);
        }
    }
    panic!("gallery viewport did not settle within sixteen passes");
}

#[test]
fn arrange_defaults_to_full_windowing_and_reaches_last_record_after_resize() {
    let mut r = boot();
    press(&mut r, "count-25000");
    assert_eq!(rows(&r).len(), 12, "Photos remains manual");
    press(&mut r, "mode-reorder");
    assert_eq!(rows(&r).len(), 25_000);
    assert_eq!(r.collections()[0].count, 25_000);
    assert!(r.collections()[0].rows.len() <= 16);
    assert!(text(&r, "supplied-count").contains("25000 supplied"));
    for (top, width) in [
        (0., 1180.),
        (160_000., 1180.),
        (160_000., 420.),
        (1e9, 420.),
    ] {
        settle(&mut r, top, width);
        assert_eq!(r.collections()[0].count, 25_000);
    }
    assert_eq!(r.collections()[0].rows.last().unwrap().index, 24_999);
    assert_eq!(r.kernel().find_by_test_id("card-photo-24999").len(), 1);
    assert!(r.kernel().find_by_test_id("card-photo-00000").is_empty());
}

#[test]
fn draft_preview_and_sheet_only_actions_keep_rows_and_skip_all_record_keying() {
    let mut r = boot();
    press(&mut r, "count-25000");
    press(&mut r, "mode-reorder");
    settle(&mut r, 0., 1180.);
    let original = rows(&r);
    r.act("edit", vec![Value::str("Persistent draft")]).unwrap();
    for control in ["lift-photo-00000", "later", "earlier", "cancel"] {
        assert!(Rc::ptr_eq(&original, &rows(&r)));
        assert_eq!(r.last_instance_work().rows_keyed, 0, "{control}");
        press(&mut r, control);
    }
    assert!(Rc::ptr_eq(&original, &rows(&r)));
    assert_eq!(r.last_instance_work().rows_keyed, 0);
    press(&mut r, "mode-sheet");
    settle(&mut r, 0., 1180.);
    let original = rows(&r);
    for control in ["note-photo-00002", "sheet-peek", "sheet-full", "sheet-read"] {
        press(&mut r, control);
        assert!(
            Rc::ptr_eq(&original, &rows(&r)),
            "{control} regenerated rows"
        );
        assert_eq!(
            r.last_instance_work().rows_keyed,
            0,
            "{control} rekeyed rows"
        );
    }
    assert_eq!(r.slot("draft"), Some(&Value::str("Persistent draft")));
}

#[test]
fn read_uses_the_actual_sheet_scrollport_and_remeasures_variable_height_rows() {
    let mut r = boot();
    press(&mut r, "count-25000");
    press(&mut r, "mode-sheet");
    let (_, wide_port) = settle(&mut r, 0., 1180.);
    let wide = r.collections()[0].rows[0].height;
    let (_, narrow_port) = settle(&mut r, 0., 420.);
    assert!(r.collections()[0].rows[0].height > wide);
    assert!(narrow_port < 860. && wide_port < 860.);
    press(&mut r, "sheet-peek");
    let (_, peek) = settle(&mut r, 0., 420.);
    press(&mut r, "sheet-full");
    let (_, full) = settle(&mut r, 0., 420.);
    assert!(full > peek * 2., "peek {peek}, full {full}");
    assert_eq!(rows(&r).len(), 25_000);
    settle(&mut r, 1e9, 420.);
    assert_eq!(r.collections()[0].rows.last().unwrap().index, 24_999);
    assert_eq!(r.kernel().find_by_test_id("note-photo-24999").len(), 1);
}

#[test]
fn insertion_preserves_the_reading_identity_and_offset_while_rekeying_new_data() {
    let mut r = boot();
    press(&mut r, "count-1000");
    press(&mut r, "mode-reorder");
    let (top, _) = settle(&mut r, 16_000., 1180.);
    let before = r.collections().remove(0);
    let anchor = before
        .rows
        .iter()
        .find(|row| row.top + row.height > top)
        .unwrap();
    let key = r
        .kernel()
        .node(anchor.root)
        .unwrap()
        .props
        .str(PropId::TestId)
        .unwrap()
        .to_owned();
    let offset = anchor.top - top;
    let old = rows(&r);
    press(&mut r, "insert");
    assert!(!Rc::ptr_eq(&old, &rows(&r)));
    assert_eq!(r.last_instance_work().rows_keyed, 1001);
    let next = r.collections()[0].correction.map_or(top, |c| c.scroll_top);
    let (new_top, _) = settle(&mut r, next, 1180.);
    let current = r.collections().remove(0);
    let same = current
        .rows
        .iter()
        .find(|row| {
            r.kernel().node(row.root).unwrap().props.str(PropId::TestId) == Some(key.as_str())
        })
        .unwrap();
    assert_eq!(same.index, anchor.index + 1);
    assert!((same.top - new_top - offset).abs() < 0.5);
}

#[test]
fn full_collection_preview_does_not_change_order_and_drop_applies_once() {
    let mut r = boot();
    press(&mut r, "count-25000");
    press(&mut r, "mode-reorder");
    settle(&mut r, 0., 1180.);
    let before = rows(&r);
    press(&mut r, "lift-photo-00000");
    settle(&mut r, 1e9, 1180.);
    press(&mut r, "before-photo-24999");
    assert!(Rc::ptr_eq(&before, &rows(&r)));
    press(&mut r, "place");
    let after = rows(&r);
    assert_eq!(id(&after[0]), "photo-00001");
    assert_eq!(id(&after[24_998]), "photo-00000");
    assert_eq!(id(&after[24_999]), "photo-24999");
    assert!(r.kernel().find_by_test_id("move-preview").is_empty());
}

#[test]
fn explicit_manual_and_eager_controls_survive_beside_windowing() {
    let mut r = boot();
    press(&mut r, "mode-reorder");
    assert_eq!(r.collections()[0].count, 100);
    press(&mut r, "render-manual");
    assert!(r.collections().is_empty());
    assert_eq!(rows(&r).len(), 12);
    press(&mut r, "next-page");
    assert_eq!(id(&rows(&r)[0]), "photo-00012");
    press(&mut r, "render-eager");
    assert!(r.collections().is_empty());
    assert_eq!(rows(&r).len(), 100);
    assert_eq!(r.kernel().find_by_test_id("card-photo-00099").len(), 1);
    press(&mut r, "render-windowed");
    assert_eq!(r.collections()[0].count, 100);
    press(&mut r, "mode-photos");
    assert!(r.collections().is_empty());
    assert_eq!(rows(&r).len(), 12);
}
