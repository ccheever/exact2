use super::*;

fn list(
    b: &mut PlanBuilder,
    parent: Option<NodesId>,
    arm: Option<ArmsId>,
    order: u32,
    enabled: exact_plan::Code,
    count: usize,
) -> NodesId {
    let height = b.constant(&Value::Number(320.0));
    let root = b.node(
        NodeType::List as u8,
        parent,
        arm,
        order,
        &[
            binding(BindingKind::Prop, PropId::Virtualized as u16, enabled),
            binding(
                BindingKind::Style,
                exact_kernel::StyleId::Height as u16,
                height,
            ),
        ],
        &[],
        None,
    );
    let subject = b.constant(&values(count));
    let key = code(b, |a| {
        a.load_item(0);
    });
    let (region, arms) = b.region(RegionKind::Each, Some(root), arm, 0, subject, key, 1);
    let num = b.primitive(TypeKind::Number);
    let zero = b.constant(&Value::Number(0.0));
    let slot = b.slot(&format!("row_counter_{}", region.0), num, zero);
    b.set_slot_owner(slot, region);
    b.node(NodeType::View as u8, None, Some(arms[0]), 0, &[], &[], None)
}

fn from_plan(plan: Plan) -> Result<Harness, InstanceError> {
    let slots = vec![Value::Number(0.0); plan.slots.len()];
    let mut ids = Ids::default();
    let mut kernel = Kernel::with_monospace();
    let mut u = Update {
        env: env(&plan, &slots),
        ids: &mut ids,
        ops: vec![],
        surfaces: vec![],
        work: Default::default(),
    };
    let tree = Tree::create(&mut u)?;
    kernel.apply(0, 1, &u.ops).unwrap();
    Ok(Harness {
        plan,
        slots,
        tree,
        ids,
        kernel,
        batch: 1,
    })
}

fn siblings() -> Harness {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let root = b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    let enabled = b.constant(&Value::Bool(true));
    list(&mut b, Some(root), None, 0, enabled, 1000);
    list(&mut b, Some(root), None, 1, enabled, 1000);
    from_plan(b.finish().unwrap()).unwrap()
}

fn collections(h: &Harness) -> Vec<&Collection> {
    let Child::Node(root) = &h.tree.children[0] else {
        panic!()
    };
    root.children
        .iter()
        .map(|child| {
            let Child::Node(node) = child else { panic!() };
            node.collection.as_deref().unwrap()
        })
        .collect()
}

fn feedback(h: &Harness, owner: usize, top: f64) -> CollectionFeedback {
    let snapshot = collections(h)[owner].snapshot();
    CollectionFeedback {
        view: snapshot.view,
        revision: snapshot.revision,
        scroll_sequence: snapshot.scroll_sequence + 1,
        scroll_top: top,
        port_width: 640.0,
        port_height: 320.0,
        row_width: 640.0,
        measurements: vec![],
        focus_view: None,
        interaction_view: None,
    }
}

fn assert_budget(h: &Harness) {
    let mut categories = [0, 0];
    let mut outside = 0;
    for c in collections(h) {
        if let Some(g) = &c.geometry {
            categories[0] += usize::from(g.focus_view.is_some());
            categories[1] += usize::from(g.interaction_view.is_some());
            let band = c
                .index
                .window(g.scroll_top, g.port_height, [None; 2])
                .unwrap();
            outside += c
                .mounted
                .iter()
                .filter(|row| !band.overscan.contains(&row.position))
                .count();
        }
    }
    assert!(
        categories.into_iter().all(|count| count <= 1),
        "duplicate category owners: {categories:?}"
    );
    assert!(
        outside <= 2,
        "{outside} off-window rows exceed the session pin budget"
    );
}

fn send(h: &mut Harness, owner: usize, top: f64, pins: [Option<ViewId>; 2]) {
    let mut f = feedback(h, owner, top);
    f.focus_view = pins[0];
    f.interaction_view = pins[1];
    h.send(f);
    assert_budget(h);
}

#[test]
fn sibling_pin_categories_transfer_independently_and_release_old_row_state() {
    let mut h = siblings();
    send(&mut h, 0, 0.0, [None; 2]);
    send(&mut h, 1, 0.0, [None; 2]);
    let left = collections(&h)[0].snapshot();
    let right = collections(&h)[1].snapshot();
    let old_focus = left.rows[0].root;
    let old_interaction = left.rows[1].root;
    let focus_slots = Rc::downgrade(&collections(&h)[0].mounted[0].row.slots);
    let interaction_slots = Rc::downgrade(&collections(&h)[0].mounted[1].row.slots);
    send(
        &mut h,
        0,
        16_000.0,
        [Some(old_focus), Some(old_interaction)],
    );
    let old_revision = collections(&h)[0].revision;
    send(&mut h, 1, 16_000.0, [Some(right.rows[0].root), None]);
    assert!(collections(&h)[0].revision > old_revision);
    assert!(h.kernel.node(old_focus).is_none());
    assert!(focus_slots.upgrade().is_none());
    assert!(h.kernel.node(old_interaction).is_some());
    let right_middle = collections(&h)[1]
        .snapshot()
        .rows
        .iter()
        .find(|r| r.index == 500)
        .unwrap()
        .root;
    send(
        &mut h,
        1,
        24_000.0,
        [Some(right.rows[0].root), Some(right_middle)],
    );
    assert!(h.kernel.node(old_interaction).is_none());
    assert!(interaction_slots.upgrade().is_none());
    // None only clears the addressed owner, preserving both right-hand pins.
    send(&mut h, 0, 16_000.0, [None; 2]);
    assert!(h.kernel.node(right.rows[0].root).is_some());
    assert!(h.kernel.node(right_middle).is_some());
    let left_middle = collections(&h)[0]
        .snapshot()
        .rows
        .iter()
        .find(|r| r.index == 500)
        .unwrap()
        .root;
    send(&mut h, 0, 24_000.0, [Some(left_middle), None]);
    assert!(h.kernel.node(right.rows[0].root).is_none());
    assert!(h.kernel.node(right_middle).is_some());
    send(&mut h, 0, 24_000.0, [None; 2]);
    assert!(h.kernel.node(left_middle).is_none());
    assert!(h.kernel.node(right_middle).is_some());
    send(&mut h, 1, 24_000.0, [None; 2]);
    assert!(h.kernel.node(right_middle).is_none());
}

#[test]
fn rejected_or_stale_feedback_never_transfers_session_pins() {
    let mut h = siblings();
    send(&mut h, 0, 0.0, [None; 2]);
    send(&mut h, 1, 0.0, [None; 2]);
    let left = collections(&h)[0].snapshot();
    let right = collections(&h)[1].snapshot();
    send(
        &mut h,
        0,
        16_000.0,
        [Some(left.rows[0].root), Some(left.rows[1].root)],
    );
    let before = h.tree.collections();
    let live = h.kernel.arena().live_count();
    for variant in 0..8 {
        let mut f = feedback(&h, 1, 16_000.0);
        f.focus_view = Some(right.rows[0].root);
        f.interaction_view = Some(right.rows[1].root);
        match variant {
            0 => f.revision -= 1,
            1 => f.scroll_sequence = 0,
            2 => f.view = u32::MAX,
            3 => f.focus_view = Some(u32::MAX),
            4 => f.interaction_view = Some(left.rows[1].root),
            5 => f.scroll_top = f64::NAN,
            6 => f.measurements.push(RowMeasurement {
                view: right.rows[0].view,
                epoch: right.rows[0].epoch - 1,
                height: 40.0,
            }),
            7 => {
                f.measurements = right.rows[..2]
                    .iter()
                    .map(|row| RowMeasurement {
                        view: row.view,
                        epoch: row.epoch,
                        height: f32::MAX as f64,
                    })
                    .collect()
            }
            _ => unreachable!(),
        }
        let mut u = Update {
            env: env(&h.plan, &h.slots),
            ids: &mut h.ids,
            ops: vec![],
            surfaces: vec![],
            work: Default::default(),
        };
        assert!(
            !matches!(h.tree.update_collection(&mut u, f), Ok((true, _))),
            "variant {variant}"
        );
        assert!(
            u.ops.is_empty(),
            "rejected report emitted operations: {variant}"
        );
        assert_eq!(h.tree.collections(), before, "variant {variant}");
        assert_eq!(h.kernel.arena().live_count(), live);
        assert_eq!(
            collections(&h)[0].geometry.as_ref().unwrap().focus_view,
            Some(left.rows[0].root)
        );
        assert_eq!(
            collections(&h)[0]
                .geometry
                .as_ref()
                .unwrap()
                .interaction_view,
            Some(left.rows[1].root)
        );
        assert_budget(&h);
    }
}

#[test]
fn accepted_pin_transfer_runs_even_when_target_window_does_not_change() {
    let mut h = siblings();
    send(&mut h, 0, 0.0, [None; 2]);
    send(&mut h, 1, 0.0, [None; 2]);
    let left = collections(&h)[0].snapshot().rows[0].root;
    let right = collections(&h)[1].snapshot().rows[0].root;
    send(&mut h, 0, 0.0, [Some(left), None]);
    let before = collections(&h)[1].snapshot();
    send(&mut h, 1, 0.0, [Some(right), None]);
    assert_eq!(collections(&h)[1].snapshot().rows, before.rows);
    assert_eq!(
        collections(&h)[0].geometry.as_ref().unwrap().focus_view,
        None
    );
}

fn nested_plan(
    outer_count: usize,
    outer_enabled: bool,
    inner_enabled: bool,
    conditional: bool,
) -> Plan {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let outer = b.constant(&Value::Bool(outer_enabled));
    let row = list(&mut b, None, None, 0, outer, outer_count);
    let arm = b.plan().node(row).arm;
    let (parent, arm) = if conditional {
        let off = b.constant(&Value::Bool(false));
        let (_, arms) = b.region(RegionKind::When, Some(row), arm, 0, off, off, 2);
        (None, Some(arms[0]))
    } else {
        (Some(row), arm)
    };
    let inner = b.constant(&Value::Bool(inner_enabled));
    list(&mut b, parent, arm, 0, inner, 3);
    b.finish().unwrap()
}

#[test]
fn handbuilt_nested_virtual_collection_is_rejected_even_when_unrealized() {
    for (count, conditional) in [(1, false), (0, false), (1, true)] {
        match from_plan(nested_plan(count, true, true, conditional)) {
            Err(InstanceError::Collection(message)) => {
                assert!(message.contains("nested"), "{message}")
            }
            _ => panic!(
                "nested virtual collection accepted: count={count}, conditional={conditional}"
            ),
        }
    }
}

#[test]
fn ordinary_eager_lists_may_enclose_or_be_inside_a_virtual_collection() {
    let disabled_inner = from_plan(nested_plan(2, true, false, false)).unwrap();
    assert_eq!(disabled_inner.tree.collections().len(), 1);
    let disabled_outer = from_plan(nested_plan(2, false, true, false)).unwrap();
    assert_eq!(disabled_outer.tree.collections().len(), 2);
}
