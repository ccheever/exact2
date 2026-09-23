//! Real compiled app and common Arrange; no physical input/presentation claim.
use exact_kernel::{Kernel, NodeKey, NodeType, Offer, PropId};
use exact_live_data::Live;
use exact_plan::Value;
use exact_runner::{
    Answer, CollectionFeedback, DataError, DataSource, Event, ReorderProgress, RowMeasurement,
    Runner, Store, Viewport,
};
use std::rc::Rc;

#[derive(Default)]
struct Counted {
    live: Live,
    calls: Vec<(String, Vec<Value>)>,
}
impl DataSource for Counted {
    fn app_id(&self) -> &str {
        self.live.app_id()
    }
    fn grants(&self) -> &str {
        self.live.grants()
    }
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.calls.push((source.into(), args.to_vec()));
        self.live.query(source, args)
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.calls.push((source.into(), args.to_vec()));
        self.live.answer(store, source, args)
    }
}
fn boot(width: f64) -> Runner<Counted> {
    let plan = contract::compile(include_str!("../../../app.contract")).unwrap();
    let baked = contract::bake(plan, Live::default()).unwrap();
    Runner::boot(
        baked,
        Counted::default(),
        Kernel::with_monospace(),
        Viewport {
            width,
            height: 844.,
        },
        "/",
    )
    .unwrap()
}
fn key(r: &Runner<Counted>, name: &str) -> NodeKey {
    let keys = r.kernel().find_by_test_id(name);
    assert_eq!(keys.len(), 1, "missing or ambiguous {name}");
    keys[0]
}
fn press(r: &mut Runner<Counted>, name: &str) {
    let id = r.kernel().node_by_key(key(r, name)).unwrap().id;
    r.dispatch(id, Event::Press).unwrap();
}
fn rows(r: &Runner<Counted>) -> Rc<Vec<Value>> {
    let Value::List(rows) = r.resource("rows").unwrap() else {
        panic!("rows")
    };
    rows.clone()
}
fn ids(rows: &[Value]) -> Vec<&str> {
    rows.iter()
        .map(|row| {
            let Value::Record(fields) = row else {
                panic!("photo")
            };
            fields[0].as_str().unwrap()
        })
        .collect()
}
fn revision(r: &Runner<Counted>) -> f64 {
    let Value::Record(fields) = r.derive("gallery").unwrap() else {
        panic!("gallery")
    };
    fields[7].as_number().unwrap()
}
fn terminal(r: &mut Runner<Counted>, item: &str, before: Option<&str>) {
    let id = r.kernel().node_by_key(key(r, "rundown-list")).unwrap().id;
    r.dispatch(
        id,
        Event::ReorderDrop {
            item: item.into(),
            before: before.map(str::to_owned),
        },
    )
    .unwrap();
    assert!(!r.has_pending());
    assert!(r.take_requests().is_empty());
}
fn reorder_calls(r: &Runner<Counted>) -> Vec<&[Value]> {
    r.data_ref()
        .calls
        .iter()
        .filter(|(name, _)| name == "galleryReorder")
        .map(|(_, args)| args.as_slice())
        .collect()
}

#[test]
fn authored_grips_bind_stable_list_and_manual_move_excludes_physical_drop() {
    for width in [390., 1280.] {
        let mut r = boot(width);
        let list = r.kernel().node_by_key(key(&r, "rundown-list")).unwrap();
        assert_eq!(list.props.str(PropId::Id), Some("rundown-arrange"));
        assert_eq!(list.props.bool(PropId::Virtualized), Some(true));
        let handle = key(&r, "arrange-grip-photo-00000");
        let grip = r.kernel().node_by_key(handle).unwrap();
        assert_eq!(grip.node_type, NodeType::View);
        assert_eq!(grip.props.str(PropId::ReorderFor), Some("rundown-arrange"));
        assert_ne!(grip.props.bool(PropId::Disabled), Some(true));
        let original = rows(&r);
        press(&mut r, "lift-photo-00000");
        let grip = r
            .kernel()
            .node_by_key(key(&r, "arrange-grip-photo-00000"))
            .unwrap();
        assert_eq!(grip.props.bool(PropId::Disabled), Some(true));
        assert!(r.reorder_binding(grip.key).is_none());
        terminal(&mut r, "photo-00001", None);
        assert_eq!(reorder_calls(&r).len(), 1);
        assert!(
            Rc::ptr_eq(&original, &rows(&r)),
            "manual move rejects synthesized physical drop too"
        );
        assert!(r.kernel().find_by_test_id("manual-move").len() == 1);
        press(&mut r, "cancel-move");
        let grip = r
            .kernel()
            .node_by_key(key(&r, "arrange-grip-photo-00000"))
            .unwrap();
        assert_ne!(grip.props.bool(PropId::Disabled), Some(true));
    }
}

#[test]
fn typed_list_drop_uses_current_revision_and_photos_still_open_after_drop() {
    let mut r = boot(390.);
    let original = rows(&r);
    let initial_revision = revision(&r);
    r.act("editDraft", vec![Value::str("Keep the coast first")])
        .unwrap();
    terminal(&mut r, "photo-00002", Some("photo-00000"));
    assert_eq!(
        reorder_calls(&r).len(),
        1,
        "actual List handler must reach galleryReorder"
    );
    assert_eq!(reorder_calls(&r)[0][2], Value::Number(initial_revision));
    assert_eq!(
        &ids(&rows(&r))[..3],
        ["photo-00002", "photo-00000", "photo-00001"]
    );
    assert_eq!(
        ids(&original)[0],
        "photo-00000",
        "old source remains immutable"
    );
    assert_eq!(revision(&r), initial_revision + 1.);
    terminal(&mut r, "photo-00002", Some("photo-00001"));
    assert_eq!(
        reorder_calls(&r)[1][2],
        Value::Number(initial_revision + 1.)
    );
    assert_eq!(revision(&r), initial_revision + 2.);
    assert_eq!(r.slot("draft"), Some(&Value::str("Keep the coast first")));
    let Value::Record(gallery) = r.derive("gallery").unwrap() else {
        panic!("gallery")
    };
    assert_eq!(gallery[0], Value::str("photos"));
    press(&mut r, "open-photo-00002");
    let handle = key(&r, "viewer-handle");
    let binding = r.kernel().transform_drag_binding(handle).unwrap();
    assert_eq!(binding.target, key(&r, "viewer-transform"));
    assert_eq!(r.kernel().find_by_test_id("close-viewer").len(), 1);
}

fn measured(r: &mut Runner<Counted>, pin: NodeKey) {
    // Feed the real monospace Kernel frames through the existing host seam.
    // This proves common state/data ownership, not browser geometry/pointer capture.
    for _ in 0..3 {
        let viewport = r.viewport();
        for root in r.roots() {
            r.kernel_mut()
                .compute_layout(
                    root,
                    Offer::definite(viewport.width as f32, viewport.height as f32),
                )
                .unwrap();
        }
        let list = r.kernel().node_by_key(key(r, "rundown-list")).unwrap();
        let id = list.id;
        let frame = list.frame;
        assert!(frame.width > 0. && frame.height > 0.);
        let c = r.collections().into_iter().find(|c| c.view == id).unwrap();
        let facts = CollectionFeedback {
            view: id,
            revision: c.revision,
            scroll_sequence: c.scroll_sequence + 1,
            scroll_top: 0.,
            port_width: f64::from(frame.width),
            port_height: f64::from(frame.height),
            row_width: f64::from(frame.width),
            measurements: c
                .rows
                .iter()
                .map(|row| {
                    let height = f64::from(r.kernel().node(row.view).unwrap().frame.height);
                    assert!(height > 0., "layout must precede measured-row admission");
                    RowMeasurement {
                        view: row.view,
                        epoch: row.epoch,
                        height,
                    }
                })
                .collect(),
            focus_view: None,
            interaction_view: Some(r.kernel().node_by_key(pin).unwrap().id),
        };
        r.collection_feedback(facts).unwrap();
    }
}

#[test]
fn common_preview_never_calls_data_and_terminal_queries_once_before_pin_finish() {
    let mut r = boot(1280.);
    let handle = key(&r, "arrange-grip-photo-00001");
    measured(&mut r, handle);
    let binding = r.reorder_binding(handle).expect("measured real card grip");
    let g = r.reorder_geometry(binding.list).unwrap();
    let token = r.begin_reorder(binding, g).unwrap().unwrap().token;
    let before = rows(&r);
    let query_count = r.data_ref().calls.len();
    let list = r.kernel().node_by_key(binding.list).unwrap().id;
    let c = r
        .collections()
        .into_iter()
        .find(|c| c.view == list)
        .unwrap();
    let first = c.rows.iter().find(|row| row.index == 0).unwrap();
    let y = first.top + first.height * 0.25;
    let g = r.reorder_geometry(binding.list).unwrap();
    assert!(first.top >= g.scroll_top && first.top <= g.scroll_top + g.port_height);
    assert!(
        matches!(
            r.preview_reorder(token, g, c.total_extent + 1.).unwrap(),
            ReorderProgress::NeedsMeasurement
        ),
        "a gap outside the actual port remains refused"
    );
    assert_eq!(r.data_ref().calls.len(), query_count);
    for _ in 0..32 {
        let g = r.reorder_geometry(binding.list).unwrap();
        assert!(matches!(
            r.preview_reorder(token, g, y).unwrap(),
            ReorderProgress::Accepted { .. }
        ));
        assert_eq!(r.data_ref().calls.len(), query_count);
        assert!(Rc::ptr_eq(&before, &rows(&r)));
    }
    let g = r.reorder_geometry(binding.list).unwrap();
    assert!(r.drop_reorder(token, g.clone()).unwrap().is_some());
    assert_eq!(reorder_calls(&r).len(), 1);
    assert_eq!(ids(&rows(&r))[0], "photo-00001");
    assert!(r.reorder_frame(token).unwrap().terminal);
    assert!(!r.has_reorder(token));
    assert!(r.drop_reorder(token, g).unwrap().is_none());
    assert_eq!(
        reorder_calls(&r).len(),
        1,
        "duplicate terminal must not query"
    );
    assert!(r.finish_reorder(token).unwrap().is_some());
    assert!(r.reorder_frame(token).is_none());
}
