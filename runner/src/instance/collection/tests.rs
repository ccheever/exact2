use super::*;
use exact_kernel::{Kernel, PropValue};
use exact_plan::{asm::Asm, builder::PlanBuilder, BindingsRow, Opcode, TypeKind};

#[path = "ownership_tests.rs"]
mod ownership;

fn code(b: &mut PlanBuilder, emit: impl FnOnce(&mut Asm)) -> exact_plan::Code {
    let mut a = Asm::new();
    emit(&mut a);
    b.code(a)
}
fn binding(kind: BindingKind, id: u16, expr: exact_plan::Code) -> BindingsRow {
    BindingsRow { kind, id, expr }
}
fn plan(n: usize, row_state: bool, follow: bool) -> Plan {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let num = b.primitive(TypeKind::Number);
    let list = b.list(num);
    let zero = b.constant(&Value::Number(0.0));
    let initial = b.constant(&values(n));
    let data = b.slot("items", list, initial);
    let body = b.slot("body", num, zero);
    let key_dep = b.slot("key_dep", num, zero);
    let font = b.constant(&Value::Number(16.0));
    let font = b.slot("font", num, font);
    let font = code(&mut b, |a| {
        a.load_slot(font);
    });
    let enabled = b.constant(&Value::Bool(true));
    let follow = b.constant(&Value::Bool(follow));
    let height = b.constant(&Value::Number(320.0));
    let root = b.node(
        NodeType::List as u8,
        None,
        None,
        0,
        &[
            binding(BindingKind::Prop, PropId::Virtualized as u16, enabled),
            binding(BindingKind::Prop, PropId::ScrollFollowEnd as u16, follow),
            binding(
                BindingKind::Style,
                exact_kernel::StyleId::FontSize as u16,
                font,
            ),
            binding(
                BindingKind::Style,
                exact_kernel::StyleId::Height as u16,
                height,
            ),
        ],
        &[],
        None,
    );
    let subject = code(&mut b, |a| {
        a.load_slot(data);
    });
    let key = code(&mut b, |a| {
        a.load_item(0).load_slot(key_dep).op(Opcode::Add, &[]);
    });
    let (region, arms) = b.region(RegionKind::Each, Some(root), None, 0, subject, key, 1);
    let state = if row_state {
        let slot = b.slot("counter", num, zero);
        b.set_slot_owner(slot, region);
        slot
    } else {
        body
    };
    let text = code(&mut b, |a| {
        a.load_slot(state).call(exact_plan::Stdlib::ToString);
    });
    b.node(
        NodeType::Text as u8,
        None,
        Some(arms[0]),
        0,
        &[binding(BindingKind::Prop, PropId::Text as u16, text)],
        &[],
        None,
    );
    b.finish().unwrap()
}
struct Harness {
    plan: Plan,
    slots: Vec<Value>,
    tree: Tree,
    ids: Ids,
    kernel: Kernel,
    batch: u64,
}
fn env<'a>(plan: &'a Plan, slots: &'a [Value]) -> Env<'a> {
    Env {
        plan,
        // Test harnesses outlive every Env they build; leak one table per call.
        strings: Box::leak(crate::vm::intern(plan).into_boxed_slice()),
        slots,
        router: None,
        derives: &[],
        resources: &[],
        params: &[],
        frames: &[],
        now_ms: 0.0,
        pending_resources: &[],
        pending_mutations: &[],
        store_dependent_derives: &[],
        store_dependent_resources: &[],
    }
}
fn values(n: usize) -> Value {
    Value::list((0..n).map(|i| Value::Number(i as f64)).collect())
}
impl Harness {
    fn new(n: usize, row_state: bool, follow: bool) -> Self {
        let plan = plan(n, row_state, follow);
        let slots = vec![
            values(n),
            Value::Number(0.0),
            Value::Number(0.0),
            Value::Number(16.0),
            Value::Unit,
        ];
        Self::from_parts(plan, slots)
    }
    fn from_parts(plan: Plan, slots: Vec<Value>) -> Self {
        let mut ids = Ids::default();
        let mut kernel = Kernel::with_monospace();
        let mut u = Update {
            env: env(&plan, &slots),
            sites: &crate::instance::SiteIndex::new(&plan),
            ids: &mut ids,
            ops: vec![],
            surfaces: vec![],
            work: Default::default(),
        };
        let tree = Tree::create(&mut u).unwrap();
        kernel.apply(0, 1, &u.ops).unwrap();
        Self {
            plan,
            slots,
            ids,
            tree,
            kernel,
            batch: 1,
        }
    }
    fn snapshot(&self) -> CollectionSnapshot {
        self.tree.collections().pop().unwrap()
    }
    fn feedback(&self, top: f64) -> CollectionFeedback {
        let s = self.snapshot();
        CollectionFeedback {
            view: s.view,
            revision: s.revision,
            scroll_sequence: s.scroll_sequence + 1,
            scroll_top: top,
            port_width: 640.0,
            port_height: 320.0,
            row_width: 640.0,
            measurements: vec![],
            focus_view: None,
            interaction_view: None,
        }
    }
    fn send(&mut self, feedback: CollectionFeedback) -> bool {
        let mut u = Update {
            env: env(&self.plan, &self.slots),
            sites: &crate::instance::SiteIndex::new(&self.plan),
            ids: &mut self.ids,
            ops: vec![],
            surfaces: vec![],
            work: Default::default(),
        };
        let changed = self.tree.update_collection(&mut u, feedback).unwrap().0;
        self.batch += 1;
        self.kernel.apply(0, self.batch, &u.ops).unwrap();
        assert_eq!(u.work.rows_keyed, 0, "geometry never keys records");
        changed
    }
    fn update(&mut self) -> Result<(), InstanceError> {
        let mut u = Update {
            env: env(&self.plan, &self.slots),
            sites: &crate::instance::SiteIndex::new(&self.plan),
            ids: &mut self.ids,
            ops: vec![],
            surfaces: vec![],
            work: Default::default(),
        };
        self.tree.update(&mut u)?;
        self.batch += 1;
        self.kernel.apply(0, self.batch, &u.ops).unwrap();
        Ok(())
    }
    fn collection(&self) -> &Collection {
        let Child::Node(root) = &self.tree.children[0] else {
            panic!()
        };
        root.collection.as_ref().unwrap()
    }
}
#[test]
fn bounded_bootstrap_twenty_traversals_release_row_slots_and_kernel_views() {
    for n in [1_000, 25_000] {
        let mut h = Harness::new(n, true, false);
        assert_eq!(h.snapshot().rows.len(), BOOTSTRAP_ROWS);
        assert_eq!(h.tree.last_work.rows_keyed, n);
        let weak = Rc::downgrade(&h.collection().mounted[0].row.slots);
        let original = h.snapshot().rows[0].root;
        for traversal in 0..20 {
            for step in 0..20 {
                let offset = (if traversal % 2 == 0 { step } else { 19 - step }) as f64
                    * (n - 20) as f64
                    * 32.0
                    / 19.0;
                let feedback = h.feedback(offset);
                h.send(feedback);
                assert!(h.snapshot().rows.len() <= 31);
                assert!(h.collection().mounted.len() <= 31);
                assert!(h.collection().spacers.len() <= 2);
                assert!(h.kernel.arena().live_count() <= 1 + 31 * 2 + 2);
                assert!(h.kernel.arena().slot_count() <= 1 + 62 * 2 + 4);
            }
        }
        assert!(weak.upgrade().is_none(), "no historical row-slot cache");
        assert!(
            h.kernel.node(original).is_none(),
            "retired kernel root is destroyed"
        );
        assert_eq!(h.collection().keys.len(), n);
        assert_eq!(h.collection().index.len(), n);
    }
}
#[test]
fn offwindow_duplicate_rejected_before_mutating_existing_rows() {
    let mut h = Harness::new(25_000, false, false);
    let before = h.snapshot();
    let mut data: Vec<_> = (0..25_000).map(|i| Value::Number(i as f64)).collect();
    data[24_999] = Value::Number(24_998.0);
    h.slots[0] = Value::list(data);
    assert!(matches!(
        h.update(),
        Err(InstanceError::DuplicateKey { .. })
    ));
    assert_eq!(h.snapshot(), before);
}
#[test]
fn body_and_key_dependencies_are_independent_and_surviving_keys_reuse_views() {
    let mut h = Harness::new(1_000, false, false);
    h.send(h.feedback(640.0));
    let before = h.snapshot();
    h.slots[1] = Value::Number(9.0);
    h.update().unwrap();
    assert_eq!(h.tree.last_work.rows_keyed, 0);
    for row in &h.snapshot().rows {
        assert_eq!(
            h.kernel.node(row.root).unwrap().props.str(PropId::Text),
            Some("9")
        );
        assert_eq!(
            row.root,
            before
                .rows
                .iter()
                .find(|r| r.index == row.index)
                .unwrap()
                .root
        );
    }
    h.update().unwrap();
    assert_eq!(h.tree.last_work.rows_keyed, 0);
    assert!(h.tree.last_work.regions_skipped > 0);
    h.slots[2] = Value::Number(1.0);
    h.update().unwrap();
    assert_eq!(h.tree.last_work.rows_keyed, 1_000);
}
#[test]
fn variable_heights_stale_epochs_width_changes_and_noop_feedback() {
    let mut h = Harness::new(1_000, false, false);
    h.send(h.feedback(640.0));
    let s = h.snapshot();
    let mut feedback = h.feedback(640.0);
    feedback.measurements = s
        .rows
        .iter()
        .map(|r| RowMeasurement {
            view: r.view,
            epoch: r.epoch,
            height: if r.index == 10 { 96.0 } else { 32.0 },
        })
        .collect();
    h.send(feedback);
    let measured = h.snapshot();
    assert_eq!(measured.total_extent, 32_064.0);
    assert_eq!(measured.correction.unwrap().scroll_top, 704.0);
    let mut noop = h.feedback(704.0);
    h.send(noop.clone()); // correction acknowledgement
    noop = h.feedback(704.0);
    assert!(!h.send(noop));
    let stale = h.feedback(704.0);
    let before_width = h.snapshot();
    let mut width = h.feedback(704.0);
    width.row_width = 320.0;
    width.port_width = 320.0;
    h.send(width);
    let after_width = h.snapshot();
    assert_eq!(after_width.total_extent, measured.total_extent);
    assert!(after_width.rows.iter().all(|r| !r.measured));
    assert!(!h.send(stale));
    let mut old_epoch = h.feedback(704.0);
    old_epoch.row_width = 320.0;
    old_epoch.measurements = vec![RowMeasurement {
        view: before_width.rows[0].view,
        epoch: before_width.rows[0].epoch,
        height: 900.0,
    }];
    assert!(!h.send(old_epoch));
    assert_eq!(h.snapshot(), after_width);
}
#[test]
fn prepend_reorder_delete_and_end_follow_preserve_the_right_anchor() {
    let mut h = Harness::new(100, false, false);
    h.send(h.feedback(642.0));
    let before = h.snapshot();
    h.slots[0] = Value::list(
        std::iter::once(Value::Number(-1.0))
            .chain((0..100).map(|i| Value::Number(i as f64)))
            .collect(),
    );
    h.update().unwrap();
    let after = h.snapshot();
    assert_eq!(after.correction.unwrap().scroll_top, 674.0);
    assert_eq!(
        after.rows.iter().find(|r| r.index == 21).unwrap().root,
        before.rows.iter().find(|r| r.index == 20).unwrap().root
    );
    h.send(h.feedback(674.0));
    h.slots[0] = Value::list((0..100).rev().map(|i| Value::Number(i as f64)).collect());
    h.update().unwrap();
    assert_eq!(
        h.snapshot().correction.unwrap().scroll_top,
        79.0 * 32.0 + 2.0
    );
    h.send(h.feedback(79.0 * 32.0 + 2.0));
    h.slots[0] = Value::list(
        (0..100)
            .rev()
            .filter(|i| *i != 20)
            .map(|i| Value::Number(i as f64))
            .collect(),
    );
    h.update().unwrap();
    assert_eq!(h.snapshot().correction, None); // successor 19 occupies deleted 20's old position
    let mut end = Harness::new(100, false, true);
    end.send(end.feedback(2880.0));
    end.slots[0] = values(200);
    end.update().unwrap();
    assert_eq!(end.snapshot().correction.unwrap().scroll_top, 6080.0);
    end.send(end.feedback(640.0));
    end.slots[0] = values(300);
    end.update().unwrap();
    assert!(end.snapshot().correction.is_none());
}
#[test]
fn dom_integer_end_append_follows_but_half_pixel_reader_does_not() {
    for (at_end, follow) in [(true, true), (false, true), (true, false)] {
        let mut h = Harness::new(2, false, follow);
        let mut initial = h.feedback(0.0);
        initial.port_height = 519.0;
        h.send(initial); // establish width before accepting measurements
        let mut measured = h.feedback(0.0);
        measured.port_height = 519.0;
        measured.measurements = h
            .snapshot()
            .rows
            .iter()
            .zip([335_080.0, 297.078_125])
            .map(|(r, height)| RowMeasurement {
                view: r.view,
                epoch: r.epoch,
                height,
            })
            .collect();
        h.send(measured);
        assert_eq!(h.snapshot().total_extent, 335_377.078_125);
        let top = if at_end {
            334_858.0
        } else {
            334_858.078_125 - 0.500_001
        };
        let mut tail = h.feedback(top);
        tail.port_height = 519.0;
        let sequence = tail.scroll_sequence;
        h.send(tail.clone());
        h.slots[0] = values(3);
        h.update().unwrap();
        let appended = h.snapshot();
        if at_end && follow {
            let correction = appended.correction.expect("DOM end must follow append");
            assert_eq!(correction.scroll_sequence, sequence);
            assert_eq!(correction.scroll_top, appended.total_extent - 519.0);
        } else {
            assert!(appended.correction.is_none());
        }
        assert!(
            !h.send(tail),
            "old revision must not replace the accepted append"
        );
        assert_eq!(h.snapshot(), appended);
        let row = appended.rows.iter().find(|r| r.index == 2).unwrap();
        let mut growth = h.feedback(if at_end && follow {
            (appended.total_extent - 519.0).round()
        } else {
            top
        });
        growth.port_height = 519.0;
        growth.measurements = vec![RowMeasurement {
            view: row.view,
            epoch: row.epoch,
            height: 134.593_75,
        }];
        h.send(growth);
        let grown = h.snapshot();
        assert_eq!(grown.total_extent, 335_511.671_875);
        if at_end && follow {
            assert_eq!(
                grown.correction.unwrap().scroll_top,
                grown.total_extent - 519.0
            );
        } else {
            assert!(grown.correction.is_none());
        }
    }
}

#[test]
fn focus_and_interaction_pins_are_disjoint_and_wrappers_have_no_sites() {
    let mut h = Harness::new(25_000, true, false);
    h.send(h.feedback(0.0));
    let first = h.snapshot().rows[0].clone();
    let mut feedback = h.feedback(25_000.0 * 16.0);
    feedback.focus_view = Some(first.root);
    h.send(feedback);
    let middle = h.snapshot().rows[2].clone();
    let mut feedback = h.feedback(25_000.0 * 32.0);
    feedback.focus_view = Some(first.root);
    feedback.interaction_view = Some(middle.root);
    h.send(feedback);
    let s = h.snapshot();
    assert!(s.rows.len() <= 32);
    assert!(h.collection().spacers.len() <= 3);
    assert!(s.rows.iter().any(|r| r.root == first.root));
    assert!(s.rows.iter().any(|r| r.root == middle.root));
    assert!(h.tree.find(first.view).is_none());
    assert!(h.tree.site(first.view).is_none());
    let (_, path) = h.tree.site(first.root).unwrap();
    assert!(matches!(path[0], InstanceStep::Row { .. }));
    let (_, frames) = h.tree.find(first.root).unwrap();
    frames[0]
        .row
        .as_ref()
        .unwrap()
        .borrow_mut()
        .insert(4, Value::Number(7.0));
    h.update().unwrap();
    assert_eq!(
        h.kernel.node(first.root).unwrap().props.str(PropId::Text),
        Some("7")
    );
    assert!(!h.tree.handlers(&h.plan).contains_key(&first.view));
    for (view, _) in &h.collection().spacers {
        assert_eq!(
            h.kernel
                .node(*view)
                .unwrap()
                .props
                .get(PropId::AccessibilityElementsHidden),
            Some(&PropValue::Bool(true))
        );
    }
}
#[test]
fn binary_feedback_roundtrips_and_rejects_malformed_reports() {
    let h = Harness::new(10, false, false);
    let mut f = h.feedback(12.0);
    let row = &h.snapshot().rows[0];
    f.measurements.push(RowMeasurement {
        view: row.view,
        epoch: row.epoch,
        height: 20.5,
    });
    let bytes = f.encode().unwrap();
    assert_eq!(bytes.len(), 88);
    assert_eq!(CollectionFeedback::decode(&bytes).unwrap(), f);
    for length in 0..bytes.len() {
        assert!(CollectionFeedback::decode(&bytes[..length]).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(CollectionFeedback::decode(&trailing).is_err());
    let mut bad = bytes.clone();
    bad[64..68].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(CollectionFeedback::decode(&bad).is_err());
    f.measurements.push(f.measurements[0]);
    assert!(f.encode().is_err());
    f.measurements.pop();
    f.row_width = f64::NAN;
    assert!(f.encode().is_err());
    assert!(h.snapshot().json().contains("\"totalExtent\""));
}

#[test]
fn snapshot_json_preserves_u64_metadata_as_decimal_strings() {
    let snapshot = CollectionSnapshot {
        view: 1,
        revision: (1_u64 << 53) + 1,
        scroll_sequence: u64::MAX - 1,
        count: 3,
        total_extent: 96.0,
        rows: vec![CollectionRow {
            view: 2,
            root: 3,
            index: 1,
            top: 32.0,
            height: 32.0,
            epoch: u64::MAX,
            measured: true,
        }],
        correction: Some(AnchorCorrection {
            scroll_sequence: (1_u64 << 53) + 3,
            scroll_top: 16.5,
        }),
    };
    let expected = concat!(
        r#"{"view":1,"revision":"9007199254740993","scrollSequence":"18446744073709551614","count":3,"totalExtent":96,"rows":["#,
        r#"{"view":2,"root":3,"index":1,"top":32,"height":32,"epoch":"18446744073709551615","measured":true}],"#,
        r#""correction":{"scrollSequence":"9007199254740995","scrollTop":16.5}}"#,
    );
    assert_eq!(snapshot.json(), expected);
    assert_eq!(
        snapshots_json(std::slice::from_ref(&snapshot)),
        format!("[{expected}]")
    );
    let mut batch = String::from("prefix:");
    snapshot.write_json(&mut batch);
    assert_eq!(batch, format!("prefix:{expected}"));

    let mut initial = snapshot;
    initial.revision = 1;
    initial.scroll_sequence = 0;
    initial.rows[0].epoch = 1;
    initial.correction.as_mut().unwrap().scroll_sequence = 0;
    let json = initial.json();
    assert!(json.contains(r#""revision":"1","scrollSequence":"0""#));
    assert!(json.contains(r#""epoch":"1""#));
    assert!(json.contains(r#""correction":{"scrollSequence":"0""#));
}

#[test]
fn pending_correction_survives_another_commit_until_host_acknowledges_it() {
    let mut h = Harness::new(100, false, false);
    h.send(h.feedback(640.0));
    h.slots[0] = Value::list(
        std::iter::once(Value::Number(-1.0))
            .chain((0..100).map(|i| Value::Number(i as f64)))
            .collect(),
    );
    h.update().unwrap();
    let correction = h.snapshot().correction;
    assert_eq!(correction.unwrap().scroll_top, 672.0);
    h.slots[1] = Value::Number(4.0);
    h.update().unwrap();
    assert_eq!(h.snapshot().correction, correction);
    h.send(h.feedback(672.0));
    assert!(h.snapshot().correction.is_none());
}

#[test]
fn height_resize_preserves_end_only_when_previously_following_and_new_sequence_wins() {
    let mut h = Harness::new(100, false, true);
    h.send(h.feedback(2880.0));
    let mut resized = h.feedback(2880.0);
    resized.port_height = 160.0;
    h.send(resized);
    assert_eq!(h.snapshot().correction.unwrap().scroll_top, 3040.0);
    let mut reading = h.feedback(640.0);
    reading.port_height = 160.0;
    h.send(reading);
    assert!(h.snapshot().correction.is_none());
    let before = h.snapshot();
    let mut stale = h.feedback(0.0);
    stale.scroll_sequence = before.scroll_sequence - 1;
    assert!(!h.send(stale));
    assert_eq!(h.snapshot(), before);
}

#[test]
fn invalidated_zero_heights_do_not_hide_newly_nonempty_offwindow_rows() {
    let mut h = Harness::new(1000, false, false);
    h.send(h.feedback(0.0));
    let Child::Node(root) = &mut h.tree.children[0] else {
        panic!()
    };
    let c = root.collection.as_mut().unwrap();
    // Compact metadata after previously traversing a document of empty rows.
    for position in 0..c.index.len() {
        let key = c.index.key(position).unwrap().to_owned();
        let token = c.index.measurement_token(&key).unwrap();
        c.index.set_measured_height(&key, token, 0.0).unwrap();
        c.zero_heights.insert(key);
    }
    h.slots[1] = Value::Number(1.0);
    h.update().unwrap();
    assert_eq!(h.snapshot().total_extent, 32_000.0);
    assert!(h.snapshot().rows.len() <= 31);
    h.send(h.feedback(900.0 * 32.0));
    let snapshot = h.snapshot();
    assert!(snapshot.rows.iter().any(|r| r.index == 900));
    assert!(snapshot.rows.iter().all(|r| !r.measured));
    assert_eq!(h.tree.last_work.rows_keyed, 0);
}

#[test]
fn typography_change_remounts_a_fully_zero_measured_collection() {
    let mut h = Harness::new(1000, false, false);
    h.send(h.feedback(0.0));
    for _ in 0..100 {
        let snapshot = h.snapshot();
        if snapshot.rows.is_empty() {
            break;
        }
        let mut feedback = h.feedback(0.0);
        feedback.measurements = snapshot
            .rows
            .iter()
            .map(|r| RowMeasurement {
                view: r.view,
                epoch: r.epoch,
                height: 0.0,
            })
            .collect();
        h.send(feedback);
    }
    assert_eq!(h.snapshot().total_extent, 0.0);
    assert!(h.snapshot().rows.is_empty());
    let stale = h.feedback(0.0);
    // Only inherited typography changes; data/key/body dependencies do not.
    h.slots[3] = Value::Number(24.0);
    h.update().unwrap();
    let snapshot = h.snapshot();
    assert_eq!(snapshot.total_extent, 32_000.0);
    assert!(
        !snapshot.rows.is_empty(),
        "typography must remount a measurable window"
    );
    assert!(snapshot.rows.len() <= 31);
    assert!(snapshot.rows.iter().all(|r| !r.measured));
    assert_eq!(h.tree.last_work.rows_keyed, 0);
    assert!(!h.send(stale));
    h.kernel
        .compute_layout(snapshot.view, exact_kernel::Offer::definite(640.0, 320.0))
        .unwrap();
    let mut feedback = h.feedback(0.0);
    feedback.measurements = snapshot
        .rows
        .iter()
        .map(|r| RowMeasurement {
            view: r.view,
            epoch: r.epoch,
            height: h.kernel.node(r.view).unwrap().frame.height as f64,
        })
        .collect();
    h.send(feedback);
    assert!(h
        .snapshot()
        .rows
        .iter()
        .any(|r| r.measured && r.height > 0.0));
    h.send(h.feedback(900.0 * 32.0));
    assert!(h.snapshot().rows.iter().any(|r| r.index >= 890));
}

struct NoData;
impl crate::DataSource for NoData {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, crate::DataError> {
        panic!("collection geometry must not query resources")
    }
}
fn runner_feedback(r: &crate::Runner<NoData>) -> CollectionFeedback {
    let c = &r.collections()[0];
    CollectionFeedback {
        view: c.view,
        revision: c.revision,
        scroll_sequence: c.scroll_sequence + 1,
        scroll_top: 0.0,
        port_width: 640.0,
        port_height: 320.0,
        row_width: 640.0,
        measurements: vec![],
        focus_view: None,
        interaction_view: None,
    }
}

#[test]
fn typography_restores_zero_prefix_without_moving_the_reading_anchor() {
    let mut h = Harness::new(1000, false, false);
    h.send(h.feedback(0.0));
    let mut measured = h.feedback(0.0);
    measured.measurements = h
        .snapshot()
        .rows
        .iter()
        .map(|row| RowMeasurement {
            view: row.view,
            epoch: row.epoch,
            height: if row.index < 10 { 0.0 } else { 32.0 },
        })
        .collect();
    h.send(measured);
    h.send(h.feedback(640.0));
    let before = h.snapshot();
    let anchor = before.rows.iter().find(|row| row.index == 30).unwrap();
    let old_epoch = anchor.epoch;
    let old_root = anchor.root;
    h.slots[3] = Value::Number(24.0);
    h.update().unwrap();
    let after = h.snapshot();
    assert_eq!(after.correction.unwrap().scroll_top, 960.0);
    assert_eq!(
        after.correction.unwrap().scroll_sequence,
        before.scroll_sequence
    );
    let anchor = after.rows.iter().find(|row| row.index == 30).unwrap();
    assert_eq!(anchor.root, old_root);
    assert_ne!(anchor.epoch, old_epoch);
    assert!(!anchor.measured);
    assert_eq!(h.tree.last_work.rows_keyed, 0);
    h.kernel
        .compute_layout(after.view, exact_kernel::Offer::definite(640.0, 320.0))
        .unwrap();
    for (view, height) in &h.collection().spacers {
        assert_eq!(
            h.kernel.node(*view).unwrap().frame.height as f64,
            *height,
            "the same typography commit must publish restored spacer heights"
        );
    }
}

#[test]
fn aggregate_measurement_overflow_is_atomic_and_does_not_poison_runner() {
    for across_batches in [false, true] {
        let mut r = crate::Runner::boot(
            plan(5, false, false),
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        r.collection_feedback(runner_feedback(&r)).unwrap();
        let rows = r.collections()[0].rows.clone();
        if across_batches {
            let mut feedback = runner_feedback(&r);
            feedback.focus_view = Some(rows[1].root);
            feedback.measurements.push(RowMeasurement {
                view: rows[0].view,
                epoch: rows[0].epoch,
                height: f32::MAX as f64 * 0.75,
            });
            r.collection_feedback(feedback).unwrap();
        }
        let before = r.collections();
        let live = r.kernel().live_count();
        let mut feedback = runner_feedback(&r);
        feedback.focus_view = Some(rows[1].root);
        feedback.measurements = if across_batches {
            vec![RowMeasurement {
                view: rows[1].view,
                epoch: rows[1].epoch,
                height: f32::MAX as f64 * 0.75,
            }]
        } else {
            rows.iter()
                .map(|row| RowMeasurement {
                    view: row.view,
                    epoch: row.epoch,
                    height: f32::MAX as f64 * 0.75,
                })
                .collect()
        };
        // Every field separately fits f32. The combined extent cannot.
        let wire = feedback.encode().unwrap();
        assert!(
            matches!(
                r.collection_feedback_bytes(&wire),
                Err(crate::RunnerError::Instance(
                    InstanceError::InvalidCollectionFeedback
                ))
            ),
            "reject the aggregate extent before mutation"
        );
        assert!(
            !r.is_poisoned(),
            "bad host geometry must not poison the runner"
        );
        assert_eq!(
            r.collections(),
            before,
            "reject before mutating epochs, geometry or heights"
        );
        assert_eq!(r.kernel().live_count(), live);
        let mut valid = runner_feedback(&r);
        valid.measurements = r.collections()[0]
            .rows
            .iter()
            .map(|row| RowMeasurement {
                view: row.view,
                epoch: row.epoch,
                height: 24.0,
            })
            .collect();
        r.collection_feedback(valid).unwrap();
        assert!(!r.is_poisoned());
        assert_eq!(r.last_instance_work().rows_keyed, 0);
    }
}

#[test]
fn measured_interior_zero_run_bounds_25k_realized_views_and_row_slots() {
    const COUNT: usize = 25_000;
    const MAX_MOUNTED: usize = 33; // Visible + overscan + at most two pins.
    let mut h = Harness::new(COUNT, true, false);
    let bootstrap = h.snapshot();
    let focus = bootstrap.rows[1].root;
    let interaction = bootstrap.rows[2].root;
    let retired_root = bootstrap.rows[3].root;
    let retired_slots = Rc::downgrade(&h.collection().mounted[3].row.slots);
    let pinned_slots = Rc::downgrade(&h.collection().mounted[1].row.slots);
    let mut first = h.feedback(0.0);
    first.focus_view = Some(focus);
    first.interaction_view = Some(interaction);
    h.send(first);
    let mut measured = std::collections::BTreeSet::new();
    // Discover every zero through actual mounted-row feedback, retaining only
    // the positive endpoints and the two explicitly pinned zero-height rows.
    for _ in 0..COUNT {
        let snapshot = h.snapshot();
        let pending: Vec<_> = snapshot.rows.iter().filter(|row| !row.measured).collect();
        if pending.is_empty() {
            break;
        }
        let mut feedback = h.feedback(0.0);
        feedback.focus_view = Some(focus);
        feedback.interaction_view = Some(interaction);
        feedback.measurements = pending
            .into_iter()
            .map(|row| {
                assert!(measured.insert(row.index), "row unexpectedly remounted");
                RowMeasurement {
                    view: row.view,
                    epoch: row.epoch,
                    height: if row.index == 0 || row.index == COUNT - 1 {
                        32.0
                    } else {
                        0.0
                    },
                }
            })
            .collect();
        h.send(feedback);
        assert!(h.snapshot().rows.len() <= MAX_MOUNTED);
        assert!(h.collection().mounted.len() <= MAX_MOUNTED);
        assert!(h.collection().spacers.len() <= 3);
        assert!(h.kernel.arena().live_count() <= 1 + MAX_MOUNTED * 2 + 3);
        assert!(h.kernel.arena().slot_count() <= 1 + MAX_MOUNTED * 4 + 6);
    }
    assert_eq!(measured.len(), COUNT, "all 25k rows were actually sampled");
    assert_eq!(h.snapshot().total_extent, 64.0);
    assert_eq!(
        h.snapshot()
            .rows
            .iter()
            .map(|row| row.index)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, COUNT - 1]
    );
    assert!(retired_slots.upgrade().is_none());
    assert!(h.kernel.node(retired_root).is_none());
    assert!(pinned_slots.upgrade().is_some());
    // Releasing the pins also releases the last zero-height UI and row state.
    h.send(h.feedback(0.0));
    assert_eq!(h.snapshot().rows.len(), 2);
    assert!(pinned_slots.upgrade().is_none());
    assert!(h.kernel.node(focus).is_none());
    assert!(h.kernel.node(interaction).is_none());
    assert!(h.kernel.arena().live_count() <= 5);
    assert_eq!(h.collection().keys.len(), COUNT);
    assert_eq!(h.collection().index.len(), COUNT);
}

#[test]
fn identical_order_with_new_content_still_invalidates_rows_and_updates_layout() {
    let mut h = Harness::new(100, false, false);
    h.send(h.feedback(0.)); // establish width before reporting measurements
    let before = h.snapshot();
    let mut measured = h.feedback(0.);
    measured.measurements = before
        .rows
        .iter()
        .map(|row| RowMeasurement {
            view: row.view,
            epoch: row.epoch,
            height: 20.,
        })
        .collect();
    h.send(measured);
    let before = h.snapshot();
    let old_row = before.rows[0].clone();
    assert!(old_row.measured);
    let key = h.collection().index.key(old_row.index).unwrap().to_owned();
    let old_token = h.collection().index.measurement_token(&key).unwrap();
    // Fresh list with unchanged scalar items can reuse certified keys. Changed
    // text still needs body and layout invalidation in this same update.
    h.slots[0] = values(100);
    h.slots[1] = Value::Number(9.);
    h.update().unwrap();
    assert_eq!(
        h.tree.last_work.rows_keyed, 0,
        "key reuse must not suppress body or measurement invalidation"
    );
    assert!(h.tree.last_work.nodes_visited > 0);
    let after = h.snapshot();
    assert!(after.revision > before.revision);
    let row = after
        .rows
        .iter()
        .find(|row| row.index == old_row.index)
        .unwrap();
    assert_eq!(row.view, old_row.view);
    assert_eq!(row.root, old_row.root);
    assert_ne!(row.epoch, old_row.epoch);
    assert!(!row.measured);
    assert_ne!(
        h.collection().index.measurement_token(&key),
        Some(old_token)
    );
    assert_eq!(
        h.kernel.node(row.root).unwrap().props.str(PropId::Text),
        Some("9")
    );
    h.kernel
        .compute_layout(after.view, exact_kernel::Offer::definite(640., 320.))
        .unwrap();
    let frame = h.kernel.node(row.root).unwrap().frame;
    assert!(frame.height.is_finite() && frame.height > 0.);
    let mut stale = h.feedback(0.);
    stale.measurements = vec![RowMeasurement {
        view: old_row.view,
        epoch: old_row.epoch,
        height: 99.,
    }];
    assert!(!h.send(stale));
    let fresh = h.snapshot();
    let mut next = h.feedback(0.);
    next.measurements = fresh
        .rows
        .iter()
        .map(|row| RowMeasurement {
            view: row.view,
            epoch: row.epoch,
            height: 24.,
        })
        .collect();
    assert!(h.send(next));
    assert!(h
        .snapshot()
        .rows
        .iter()
        .any(|row| row.measured && row.height == 24.));
}

// Key reuse must be earned by immutable item identity AND key-environment
// equality, not by the fresh outer list or by unchanged rendered row bodies.
fn key_reuse_row(id: f64, text: &str) -> Value {
    Value::record(vec![
        Value::Number(id),
        Value::some(Value::Number(0.)),
        Value::str(text),
    ])
}

fn key_reuse_harness(clock_key: bool) -> Harness {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let number = b.primitive(TypeKind::Number);
    let string = b.primitive(TypeKind::String);
    let optional = b.option(number);
    let row = b.record(
        "KeyReuseRow",
        &[("id", number), ("gate", optional), ("text", string)],
    );
    let list = b.list(row);
    let items = Value::list((0..128).map(|i| key_reuse_row(i as f64, "old")).collect());
    // Harness supplies the live records directly; the typed slot's unused
    // initializer is empty (record constants require explicit Asm::record).
    let initial = b.constant(&Value::list(Vec::new()));
    let zero = b.constant(&Value::Number(0.));
    let data = b.slot("items", list, initial);
    let dep = b.slot("keyDependency", number, zero);
    let enabled = b.constant(&Value::Bool(true));
    let root = b.node(
        NodeType::List as u8,
        None,
        None,
        0,
        &[binding(
            BindingKind::Prop,
            PropId::Virtualized as u16,
            enabled,
        )],
        &[],
        None,
    );
    let subject = code(&mut b, |a| {
        a.load_slot(data);
    });
    let key = code(&mut b, |a| {
        // A later row can trap, so duplicate-vs-trap precedence is observable.
        a.load_item(0)
            .field(1)
            .simple(Opcode::Unwrap)
            .simple(Opcode::Pop);
        a.load_item(0).field(0).load_slot(dep).simple(Opcode::Add);
        if clock_key {
            a.call(exact_plan::Stdlib::Now).simple(Opcode::Add);
        }
    });
    let (_, arms) = b.region(RegionKind::Each, Some(root), None, 0, subject, key, 1);
    let body = code(&mut b, |a| {
        a.load_item(0).field(2);
    });
    b.node(
        NodeType::Text as u8,
        None,
        Some(arms[0]),
        0,
        &[binding(BindingKind::Prop, PropId::Text as u16, body)],
        &[],
        None,
    );
    Harness::from_parts(b.finish().unwrap(), vec![items, Value::Number(0.)])
}

fn key_reuse_items(h: &Harness) -> Vec<Value> {
    let Value::List(items) = &h.slots[0] else {
        panic!("list fixture")
    };
    items.as_ref().clone()
}

#[test]
fn key_reuse_fresh_outer_list_retains_keys_without_key_vm_calls() {
    let mut h = key_reuse_harness(false);
    let keys = h.collection().keys.as_ptr();
    let index_key = h.collection().index.key(0).unwrap().as_ptr();
    let revision = h.snapshot().revision;
    h.slots[0] = Value::list(key_reuse_items(&h));
    h.update().unwrap();
    assert_eq!(h.tree.last_work.rows_keyed, 0);
    assert_eq!(h.collection().keys.as_ptr(), keys);
    assert_eq!(h.collection().index.key(0).unwrap().as_ptr(), index_key);
    assert!(
        h.snapshot().revision > revision,
        "ordinary refinement still runs"
    );
}

#[test]
fn key_reuse_one_changed_record_evaluates_once_and_updates_measured_body() {
    let mut h = key_reuse_harness(false);
    h.send(h.feedback(0.));
    let mut measured = h.feedback(0.);
    measured.measurements = h
        .snapshot()
        .rows
        .iter()
        .map(|r| RowMeasurement {
            view: r.view,
            epoch: r.epoch,
            height: 20.,
        })
        .collect();
    h.send(measured);
    let old = h.snapshot().rows[0].clone();
    let keys = h.collection().keys.as_ptr();
    let mut items = key_reuse_items(&h);
    items[0] = key_reuse_row(0., "new body");
    h.slots[0] = Value::list(items);
    h.update().unwrap();
    assert_eq!(h.tree.last_work.rows_keyed, 1);
    assert_eq!(h.collection().keys.as_ptr(), keys);
    let new = h.snapshot().rows[0].clone();
    assert_eq!(new.root, old.root);
    assert_ne!(new.epoch, old.epoch);
    assert!(!new.measured);
    assert_eq!(
        h.kernel.node(new.root).unwrap().props.str(PropId::Text),
        Some("new body")
    );
    h.kernel
        .compute_layout(h.snapshot().view, exact_kernel::Offer::definite(640., 320.))
        .unwrap();
    assert!(h.kernel.node(new.root).unwrap().frame.height > 0.);
    h.send(h.feedback(0.));
    assert_eq!(
        h.tree.last_work.rows_keyed, 0,
        "feedback has its own zero-key Update"
    );
}

#[test]
fn key_reuse_equal_content_fresh_records_still_evaluate_every_key() {
    let mut h = key_reuse_harness(false);
    let keys = h.collection().keys.as_ptr();
    h.slots[0] = Value::list((0..128).map(|i| key_reuse_row(i as f64, "old")).collect());
    h.update().unwrap();
    assert_eq!(h.tree.last_work.rows_keyed, 128);
    assert_eq!(
        h.collection().keys.as_ptr(),
        keys,
        "ordered uniqueness proof remains valid"
    );
}

#[test]
fn key_reuse_external_slot_change_invalidates_all_identical_items() {
    let mut h = key_reuse_harness(false);
    h.slots[0] = Value::list(key_reuse_items(&h));
    h.slots[1] = Value::Number(500.);
    h.update().unwrap();
    assert_eq!(h.tree.last_work.rows_keyed, 128);
    assert_eq!(h.collection().index.key(0), Some("n:500"));
    assert_eq!(h.collection().index.key(127), Some("n:627"));
}

#[test]
fn key_reuse_clock_change_invalidates_all_identical_items() {
    let mut h = key_reuse_harness(true);
    h.slots[0] = Value::list(key_reuse_items(&h));
    let mut input = env(&h.plan, &h.slots);
    input.now_ms = 500.;
    let mut u = Update {
        env: input,
        sites: &crate::instance::SiteIndex::new(&h.plan),
        ids: &mut h.ids,
        ops: vec![],
        surfaces: vec![],
        work: Default::default(),
    };
    h.tree.update(&mut u).unwrap();
    assert_eq!(u.work.rows_keyed, 128);
    assert_eq!(h.collection().index.key(0), Some("n:500"));
}

#[test]
fn key_reuse_is_positional_not_a_cross_position_identity_cache() {
    let mut h = key_reuse_harness(false);
    let old = h.snapshot();
    let mut items = key_reuse_items(&h);
    items.swap(0, 1);
    h.slots[0] = Value::list(items);
    h.update().unwrap();
    assert_eq!(h.tree.last_work.rows_keyed, 2);
    assert_eq!(h.collection().index.key(0), Some("n:1"));
    assert_eq!(h.collection().index.key(1), Some("n:0"));
    assert_eq!(h.snapshot().rows[0].root, old.rows[1].root);
    let mut items = key_reuse_items(&h);
    items.push(key_reuse_row(500., "inserted"));
    h.slots[0] = Value::list(items);
    h.update().unwrap();
    assert_eq!(h.collection().index.len(), 129);
    let mut items = key_reuse_items(&h);
    items.remove(0);
    h.slots[0] = Value::list(items);
    h.update().unwrap();
    assert_eq!(h.collection().index.len(), 128);
    assert_eq!(h.collection().index.position("n:1"), None);
}

#[test]
fn key_reuse_duplicate_prefix_precedes_later_vm_trap_without_publication() {
    let mut h = key_reuse_harness(false);
    let before = h.snapshot();
    let keys = h.collection().keys.as_ptr();
    let items = Rc::clone(&h.collection().items);
    let mut next = key_reuse_items(&h);
    next[2] = key_reuse_row(0., "duplicate reused prefix");
    next[3] = Value::record(vec![Value::Number(3.), Value::NONE, Value::str("trap")]);
    h.slots[0] = Value::list(next);
    assert!(matches!(
        h.update(),
        Err(InstanceError::DuplicateKey { .. })
    ));
    assert_eq!(h.snapshot(), before);
    assert_eq!(h.collection().keys.as_ptr(), keys);
    assert!(Rc::ptr_eq(&h.collection().items, &items));
}

#[test]
fn key_reuse_duplicate_reused_suffix_precedes_later_vm_trap() {
    let mut h = key_reuse_harness(false);
    let before = h.snapshot();
    let mut next = key_reuse_items(&h);
    next[0] = key_reuse_row(2., "collides with unchanged row two");
    next[3] = Value::record(vec![Value::Number(3.), Value::NONE, Value::str("trap")]);
    h.slots[0] = Value::list(next);
    assert!(matches!(
        h.update(),
        Err(InstanceError::DuplicateKey { .. })
    ));
    assert_eq!(h.snapshot(), before);
}

#[test]
fn key_reuse_changed_row_trap_is_not_hidden_by_unchanged_key_field() {
    let mut h = key_reuse_harness(false);
    let before = h.snapshot();
    let mut next = key_reuse_items(&h);
    next[127] = Value::record(vec![Value::Number(127.), Value::NONE, Value::str("trap")]);
    h.slots[0] = Value::list(next);
    assert!(matches!(
        h.update(),
        Err(InstanceError::Trap(Trap::UnwrapNone { .. }))
    ));
    assert_eq!(h.snapshot(), before);
}

#[test]
fn key_reuse_repeated_answers_do_not_retain_historical_records() {
    let mut h = key_reuse_harness(false);
    h.send(h.feedback(0.));
    for turn in 0..16 {
        let old = Rc::downgrade(&h.collection().items);
        let Value::Record(row) = &h.collection().items[100] else {
            panic!()
        };
        let old_row = Rc::downgrade(row);
        let mut next = key_reuse_items(&h);
        next[100] = key_reuse_row(100., &format!("answer {turn}"));
        h.slots[0] = Value::list(next);
        h.update().unwrap();
        assert_eq!(h.tree.last_work.rows_keyed, 1);
        assert!(old.upgrade().is_none());
        assert!(old_row.upgrade().is_none());
        assert_eq!(h.collection().keys.len(), 128);
        assert!(h.collection().mounted.len() <= 31);
    }
}

#[test]
fn key_reuse_environment_distinguishes_absent_item_and_bound_from_unit() {
    let p = plan(4, false, false);
    let slots = [
        values(4),
        Value::Number(0.),
        Value::Number(0.),
        Value::Number(16.),
    ];
    let input = env(&p, &slots);
    for field in [0, 1] {
        let mut memo = KeyMemo::new(&p, RegionsId(0)).unwrap();
        let absent = Frame::default();
        memo.remember(&input, std::slice::from_ref(&absent));
        assert!(memo.unchanged(&input, std::slice::from_ref(&absent)));
        let mut present = absent;
        if field == 0 {
            present.item = Some(Value::Unit);
        } else {
            present.bound = Some(Value::Unit);
        }
        assert!(!memo.unchanged(&input, &[present]));
    }
}

#[test]
fn key_reuse_environment_preserves_frame_scope_and_resolved_mutable_slot() {
    let mut p = plan(4, false, false);
    p.slots[2].owner = Some(RegionsId(0));
    let slots = [
        values(4),
        Value::Number(0.),
        Value::Number(0.),
        Value::Number(16.),
    ];
    let input = env(&p, &slots);
    let local = Rc::new(std::cell::RefCell::new(BTreeMap::from([(
        2,
        Value::Number(7.),
    )])));
    let frame = Frame {
        region: Some(0),
        row: Some(Rc::clone(&local)),
        ..Frame::default()
    };
    let mut memo = KeyMemo::new(&p, RegionsId(0)).unwrap();
    memo.remember(&input, std::slice::from_ref(&frame));
    assert!(memo.unchanged(&input, std::slice::from_ref(&frame)));
    local.borrow_mut().insert(2, Value::Number(8.));
    assert!(!memo.unchanged(&input, std::slice::from_ref(&frame)));
    local.borrow_mut().insert(2, Value::Number(7.));
    let mut changed = frame.clone();
    changed.region = Some(1);
    assert!(!memo.unchanged(&input, &[changed]));
    assert!(!memo.unchanged(&input, &[Frame::default(), frame]));
}

fn key_reuse_dependency_plan(which: u8) -> Plan {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let number = b.primitive(TypeKind::Number);
    let zero = b.constant(&Value::Number(0.));
    b.slot("dep", number, zero);
    let derive = b.derive("derived", number, zero);
    let resource = b.resource("answer", "answer", &[], number, Some(&Value::Number(0.)));
    let optional = b.option(number);
    let none = b.constant(&Value::NONE);
    let reply = b.slot("reply", optional, none);
    let mutation = b.mutation("write", reply, number);
    let subject = b.constant(&values(1));
    let key = code(&mut b, |a| match which {
        0 => {
            a.load_derive(derive);
        }
        1 => {
            a.load_resource(resource);
        }
        2 => {
            a.pending_resource(resource);
        }
        3 => {
            a.pending_mutation(mutation);
        }
        4 => {
            a.simple(Opcode::Unit).call(exact_plan::Stdlib::Depth);
        }
        5 => {
            a.load_param(0);
        }
        _ => unreachable!(),
    });
    b.region(RegionKind::Each, None, None, 0, subject, key, 1);
    b.finish().unwrap()
}

#[test]
fn key_reuse_certificate_tracks_derive_resource_and_pending_variants() {
    for kind in 0..4 {
        let p = key_reuse_dependency_plan(kind);
        let slots = [Value::Number(0.)];
        let before = [Some(Value::Number(1.))];
        let after = [Some(Value::Number(2.))];
        let pending = [false];
        let changed_pending = [true];
        let mut input = env(&p, &slots);
        input.derives = &before;
        input.resources = &before;
        input.pending_resources = &pending;
        input.pending_mutations = &pending;
        let mut memo = KeyMemo::new(&p, RegionsId(0)).unwrap();
        memo.remember(&input, &[]);
        assert!(memo.unchanged(&input, &[]));
        match kind {
            0 => input.derives = &after,
            1 => input.resources = &after,
            2 => input.pending_resources = &changed_pending,
            3 => input.pending_mutations = &changed_pending,
            _ => unreachable!(),
        }
        assert!(!memo.unchanged(&input, &[]), "dependency {kind}");
    }
}

#[test]
fn key_reuse_unsupported_router_call_and_action_param_decline_certificate() {
    for kind in [4, 5] {
        let p = key_reuse_dependency_plan(kind);
        assert!(KeyMemo::new(&p, RegionsId(0)).is_none());
    }
}

#[test]
fn key_reuse_certificate_tracks_outer_values_and_unreferenced_scope_structure() {
    let p = plan(4, false, false);
    let slots = [
        values(4),
        Value::Number(0.),
        Value::Number(0.),
        Value::Number(16.),
    ];
    let input = env(&p, &slots);
    let frame = Frame {
        item: Some(Value::str("outer")),
        bound: Some(Value::str("bound")),
        ..Frame::default()
    };
    let mut memo = KeyMemo::new(&p, RegionsId(0)).unwrap();
    memo.remember(&input, std::slice::from_ref(&frame));
    assert!(memo.unchanged(&input, std::slice::from_ref(&frame)));
    let mut changed = frame.clone();
    changed.item = Some(Value::str("different"));
    assert!(!memo.unchanged(&input, &[changed]));
    let mut changed = frame.clone();
    changed.bound = Some(Value::str("different"));
    assert!(!memo.unchanged(&input, &[changed]));
    let mut changed = frame.clone();
    changed.region = Some(0);
    assert!(!memo.unchanged(&input, &[changed]));
    let mut changed = frame;
    changed.row = Some(Rc::new(std::cell::RefCell::new(BTreeMap::new())));
    assert!(!memo.unchanged(&input, &[changed]));
}

#[test]
fn bounded_snapshots_equal_ordinary_and_refuse_before_owned_rows() {
    let mut h = Harness::new(10_000, false, false);
    let ordinary = h.tree.collections();
    let rows = ordinary.iter().map(|c| c.rows.len()).sum::<usize>();
    assert_eq!(
        h.tree.collections_bounded(1, rows, 4096, 1 << 20).unwrap(),
        ordinary
    );
    assert!(h.tree.collections_bounded(0, rows, 4096, 1 << 20).is_err());
    assert!(h
        .tree
        .collections_bounded(1, rows - 1, 4096, 1 << 20)
        .is_err());
    assert!(h.tree.collections_bounded(1, rows, 0, 1 << 20).is_err());
    assert!(h.tree.collections_bounded(1, rows, 4096, 0).is_err());
    assert_eq!(
        h.tree.collections(),
        ordinary,
        "refusal never mutates epochs or revisions"
    );
    h.send(h.feedback(640.0));
    let current = h.tree.collections();
    assert_eq!(
        h.tree.collections_bounded(2, 4096, 4096, 1 << 20).unwrap(),
        current
    );
    assert_ne!(current, ordinary);
}
