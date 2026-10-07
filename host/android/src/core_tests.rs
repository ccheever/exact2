use super::*;
use android_core_data::Core as Data;
use exact_kernel::MonospaceMeasurer;

fn baked() -> Vec<u8> {
    let plan = contract::compile_path(std::path::Path::new("../../apps/android-core/app.contract"))
        .unwrap();
    contract::bake(plan, Data).unwrap().encode()
}

fn assert_kernel_parity(core: &Core<Data>, reference: &exact_apple::Host<Data>) {
    let actual = core.runner.kernel();
    let expected = reference.runner().kernel();
    let rows = actual.rows(None).unwrap();
    let baseline = expected.rows(None).unwrap();
    assert_eq!(rows.len(), baseline.len());
    for (a, b) in rows.iter().zip(&baseline) {
        assert_eq!((a.id, a.parent, a.node_type), (b.id, b.parent, b.node_type));
        assert!(
            a.frame.bits_eq(b.frame),
            "{}: {:?} != {:?}",
            a.id,
            a.frame,
            b.frame
        );
        let a = actual.node(a.id).unwrap();
        let b = expected.node(b.id).unwrap();
        assert_eq!(a.props, b.props);
        assert_eq!(a.style, b.style);
        assert_eq!(a.content, b.content);
    }
}

#[test]
fn receipt_adapter_keeps_shared_native_layout_state_and_css_geometry() {
    let bytes = baked();
    let (mut core, _) = Core::boot(
        Plan::decode(&bytes).unwrap(),
        Data,
        Box::new(MonospaceMeasurer::default()),
        390.,
        844.,
        None,
    )
    .unwrap();
    let (mut reference, _) = exact_apple::Host::boot(
        &bytes,
        Data,
        Box::new(MonospaceMeasurer::default()),
        390.,
        844.,
    )
    .unwrap();
    assert_kernel_parity(&core, &reference);
    for (index, target) in [
        "increment",
        "increment",
        "toggle-batch",
        "rows-1",
        "rows-1000",
        "toggle-move",
        "toggle-batch",
    ]
    .iter()
    .enumerate()
    {
        let key = core.runner.kernel().find_first_by_test_id(target).unwrap();
        let id = core.runner.kernel().node_by_key(key).unwrap().id;
        core.dispatch(id, Event::Press, index as f64 + 1.);
        reference.dispatch_at(id, Event::Press, index as f64 + 1.);
        assert_kernel_parity(&core, &reference);
    }
    let id = core
        .runner
        .kernel()
        .node_by_key(core.runner.kernel().find_first_by_test_id("draft").unwrap())
        .unwrap()
        .id;
    core.dispatch(id, Event::Input("Zażółć 🦀".into()), 10.);
    reference.dispatch_at(id, Event::Input("Zażółć 🦀".into()), 10.);
    assert_kernel_parity(&core, &reference);
    core.resize(420., 860.);
    reference.resize(420., 860.);
    assert_kernel_parity(&core, &reference);
    core.insets(Env::new(24., 0., 16., 0.));
    reference.set_insets(24., 0., 16., 0.);
    assert_kernel_parity(&core, &reference);
}

#[test]
fn unsupported_plan_chooses_general_owner_before_core_initialization() {
    let mut plan = Plan::decode(&baked()).unwrap();
    assert!(eligible(&plan, &Data));
    plan.nodes[0].node_type = NodeType::Canvas as u8;
    assert!(!eligible(&plan, &Data));
    plan.nodes[0].node_type = NodeType::View as u8;
    plan.nodes[1].parent = Some(exact_plan::NodesId(2));
    plan.nodes[2].node_type = NodeType::Text as u8;
    assert!(!eligible(&plan, &Data));
}

#[test]
fn all_compositor_records_keep_the_existing_two_coordinate_wire_contract() {
    let bytes = baked();
    let (core, _) = Core::boot(
        Plan::decode(&bytes).unwrap(),
        Data,
        Box::new(MonospaceMeasurer::default()),
        390.,
        844.,
        None,
    )
    .unwrap();
    let key = core
        .runner
        .kernel()
        .find_first_by_test_id("rows-container")
        .unwrap();
    let node = core.runner.kernel().node_by_key(key).unwrap();
    let mut value = resolved(node);
    value.transform = [[12., 3.], [0.5, 0.], [45., 0.], [0.2, 0.]];
    let mut p = Publication::default();
    p.compositor(node.id, None, &value);
    assert_eq!(p.records, 4);
    for record in p.hot.chunks_exact(27) {
        assert_eq!(record[0], PRESENT);
        assert_eq!(u32::from_le_bytes(record[1..5].try_into().unwrap()), 22);
        assert_eq!(record[10], 2);
    }
}

#[test]
fn padded_block_scroll_keeps_css_end_padding_in_the_published_content_extent() {
    let path = std::path::Path::new("../../apps/android-core/app.contract");
    let source = std::fs::read_to_string(path).unwrap().replace(
        "scroll testId=\"core-scroll\"",
        "scroll testId=\"core-scroll\" display=\"block\" padding-right=20 padding-bottom=30",
    );
    let plan = contract::bake(contract::compile_path_source(path, &source).unwrap(), Data).unwrap();
    let bytes = plan.encode();
    let (core, _) = Core::boot(
        plan,
        Data,
        Box::new(MonospaceMeasurer::default()),
        390.,
        844.,
        None,
    )
    .unwrap();
    let (reference, publication) = exact_apple::Host::boot(
        &bytes,
        Data,
        Box::new(MonospaceMeasurer::default()),
        390.,
        844.,
    )
    .unwrap();
    assert_kernel_parity(&core, &reference);
    let key = core
        .runner
        .kernel()
        .find_first_by_test_id("core-scroll")
        .unwrap();
    let size = core.mirror[&key].content.unwrap();
    let id = core.mirror[&key].id;
    let marker = format!(
        "\"op\":\"content\",\"id\":{id},\"w\":{},\"h\":{}",
        style::num(size.0),
        style::num(size.1)
    );
    assert!(publication.contains(&marker), "{marker}");
    let node = core.runner.kernel().node_by_key(key).unwrap();
    let child = core.runner.kernel().node(node.children()[0]).unwrap();
    assert!(size.1 >= child.frame.y - node.frame.y + child.frame.height + 30.);
}

#[test]
fn initial_language_and_direction_follow_the_shared_runner() {
    let mut plan = Plan::decode(&baked()).unwrap();
    let name = exact_plan::StrId(plan.strings.len() as u32);
    plan.strings.push("ar".into());
    plan.locales.push(exact_plan::LocalesRow {
        name,
        rtl: true,
        texts: exact_plan::TextsRange::default(),
    });
    let (_, publication) = Core::boot(
        plan,
        Data,
        Box::new(MonospaceMeasurer::default()),
        390.,
        844.,
        None,
    )
    .unwrap();
    assert!(String::from_utf8_lossy(&publication)
        .contains("\"op\":\"language\",\"lang\":\"ar\",\"dir\":\"rtl\""));
}

#[test]
fn display_frame_tasks_do_not_also_schedule_virtual_timer_frames() {
    let plan = Plan::decode(&baked()).unwrap();
    let index = plan
        .actions
        .iter()
        .position(|a| plan.str(a.name) == "increment")
        .unwrap();
    let mut builder = exact_plan::builder::PlanBuilder::from_plan(plan);
    builder.frame_timer(exact_plan::ActionsId(index as u32));
    let plan = builder.finish().unwrap();
    let bytes = plan.encode();
    let (mut reference, _) = exact_apple::Host::boot(
        &bytes,
        Data,
        Box::new(MonospaceMeasurer::default()),
        390.,
        844.,
    )
    .unwrap();
    let (mut core, _) = Core::boot(
        plan,
        Data,
        Box::new(MonospaceMeasurer::default()),
        390.,
        844.,
        None,
    )
    .unwrap();
    core.advance(16., true);
    reference.frame(16.);
    assert_eq!(
        core.runner.timer_due_ms(),
        reference.runner().timer_due_ms()
    );
    assert_eq!(core.runner.timer_due_ms(), None);
    core.advance(100., false);
    reference.advance(100.);
    assert_kernel_parity(&core, &reference);
    assert_eq!(
        core.agent(r#"{"op":"state"}"#),
        reference.agent(r#"{"op":"state"}"#)
    );
}

fn records(wire: &[u8]) -> Vec<(u8, &[u8])> {
    let count = u32::from_le_bytes(wire[8..12].try_into().unwrap());
    let mut at = HEADER_BYTES;
    (0..count)
        .map(|_| {
            let opcode = wire[at];
            let length = u32::from_le_bytes(wire[at + 1..at + 5].try_into().unwrap()) as usize;
            at += 5;
            let payload = &wire[at..at + length];
            at += length;
            (opcode, payload)
        })
        .collect()
}

#[test]
fn new_leaves_need_no_child_list_but_existing_parents_can_be_cleared() {
    let (mut core, initial) = Core::boot(
        Plan::decode(&baked()).unwrap(),
        Data,
        Box::new(MonospaceMeasurer::default()),
        390.,
        844.,
        None,
    )
    .unwrap();
    let initial_lists: Vec<_> = records(&initial)
        .into_iter()
        .filter(|(opcode, payload)| {
            *opcode == crate::wire::JSON && payload.starts_with(br#"{"op":"children""#)
        })
        .collect();
    assert!(!initial_lists.is_empty());
    for (_, list) in initial_lists {
        assert!(!std::str::from_utf8(list).unwrap().contains(r#""ids":[]"#));
    }
    let row = core.runner.kernel().find_first_by_test_id("row-0").unwrap();
    let parent = core
        .runner
        .kernel()
        .node_by_key(row)
        .unwrap()
        .parent
        .unwrap();
    let root = core.runner.roots()[0];
    let receipt = core
        .runner
        .kernel_mut()
        .apply(
            root,
            1,
            &[exact_kernel::Op::SetChildren {
                id: parent,
                children: vec![],
            }],
        )
        .unwrap();
    let wire = core.publish(&[receipt], false, None);
    let expected = format!(r#"{{"op":"children","id":{parent},"ids":[]}}"#);
    assert!(records(&wire).iter().any(|(opcode, payload)| {
        *opcode == crate::wire::JSON && *payload == expected.as_bytes()
    }));
}

#[test]
fn bulk_paint_keeps_typed_row_props_and_publishes_only_color_pairs() {
    let (mut core, _) = Core::boot(
        Plan::decode(&baked()).unwrap(),
        Data,
        Box::new(MonospaceMeasurer::default()),
        390.,
        844.,
        None,
    )
    .unwrap();
    let rows: Vec<_> = (0..100)
        .map(|index| {
            let key = core
                .runner
                .kernel()
                .find_first_by_test_id(&format!("row-{index}"))
                .unwrap();
            let props = &core.mirror[&key].props;
            (key, props.str(PropId::Text).unwrap().as_ptr())
        })
        .collect();
    let key = core
        .runner
        .kernel()
        .find_first_by_test_id("toggle-batch")
        .unwrap();
    let id = core.runner.kernel().node_by_key(key).unwrap().id;
    let wire = core.dispatch(id, Event::Press, 1.);
    let operations = records(&wire);
    assert_eq!(operations.len(), rows.len());
    for (opcode, payload) in operations {
        assert_eq!(opcode, PAINT);
        assert_eq!(payload.len(), 16);
        assert_eq!(u32::from_le_bytes(payload[4..8].try_into().unwrap()), 1);
        for pair in payload[8..].chunks_exact(4) {
            assert_eq!(u32::from_le_bytes(pair.try_into().unwrap()), 0xffd1e4ff);
        }
    }
    for (key, pointer) in &rows {
        assert_eq!(
            core.mirror[key].props.str(PropId::Text).unwrap().as_ptr(),
            *pointer
        );
    }
    let first = core.mirror[&rows[0].0].style.as_ref().unwrap();
    assert!(rows
        .iter()
        .all(|(key, _)| Rc::ptr_eq(first, core.mirror[key].style.as_ref().unwrap())));
}

#[test]
fn typed_props_keep_scalar_values_clears_and_dynamic_spellcheck_inheritance() {
    use exact_kernel::wire::Op;

    let (mut core, _) = Core::boot(
        Plan::decode(&baked()).unwrap(),
        Data,
        Box::new(MonospaceMeasurer::default()),
        390.,
        844.,
        None,
    )
    .unwrap();
    let key = core.runner.kernel().find_first_by_test_id("draft").unwrap();
    let id = core.runner.kernel().node_by_key(key).unwrap().id;
    let root = core.roots[0];
    let values = [
        (PropId::Placeholder, PropValue::Str("東京 🌍".into())),
        (PropId::Disabled, PropValue::Bool(true)),
        (PropId::TabIndex, PropValue::Int(-3)),
        (PropId::HitSlop, PropValue::Float(3.75)),
    ];
    let ops: Vec<_> = values
        .iter()
        .map(|(prop, value)| Op::SetProp {
            id,
            prop: *prop,
            value: value.clone(),
        })
        .collect();
    let receipt = core.runner.kernel_mut().apply(root, 1, &ops).unwrap();
    let wire = core.publish(&[receipt], false, None);
    let operations = records(&wire);
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].0, crate::wire::JSON);
    let text = std::str::from_utf8(operations[0].1).unwrap();
    for pair in [
        r#""placeholder":"東京 🌍""#,
        r#""disabled":"true""#,
        r#""tabIndex":"-3""#,
        r#""hitSlop":"3.75""#,
    ] {
        assert!(text.contains(pair), "{text}");
    }
    let ops: Vec<_> = values
        .iter()
        .map(|(prop, _)| Op::ClearProp { id, prop: *prop })
        .collect();
    let receipt = core.runner.kernel_mut().apply(root, 2, &ops).unwrap();
    let wire = core.publish(&[receipt], false, None);
    let text = std::str::from_utf8(records(&wire)[0].1).unwrap();
    assert!(
        text.contains(r#""clear":["disabled","hitSlop","placeholder","tabIndex"]"#),
        "{text}"
    );

    for (batch, target, value, expected) in [
        (3, root, Some("false"), Some(false)),
        (4, id, Some(""), Some(true)),
        (5, root, None, Some(true)),
        (6, id, None, None),
    ] {
        let op = match value {
            Some(value) => Op::SetProp {
                id: target,
                prop: PropId::Spellcheck,
                value: PropValue::Str(value.into()),
            },
            None => Op::ClearProp {
                id: target,
                prop: PropId::Spellcheck,
            },
        };
        let receipt = core.runner.kernel_mut().apply(root, batch, &[op]).unwrap();
        let wire = core.publish(&[receipt], false, None);
        assert_eq!(core.mirror[&key].spellcheck, expected);
        let input_marker = format!(r#""id":{id},"#);
        let input = records(&wire).into_iter().find_map(|(opcode, payload)| {
            if opcode != crate::wire::JSON {
                return None;
            }
            let text = std::str::from_utf8(payload).unwrap();
            text.contains(&input_marker).then_some(text)
        });
        if batch == 5 {
            assert!(
                input.is_none(),
                "own spelling hint overrides the removed ancestor hint"
            );
        } else {
            let text = input.unwrap();
            let pair = match expected {
                Some(value) => format!(r#""spellcheck":"{value}""#),
                None => r#""clear":["spellcheck"]"#.to_owned(),
            };
            assert!(text.contains(&pair), "{text}");
        }
    }
}

#[test]
fn stack_paint_payload_keeps_every_light_dark_color_pair_and_noop_silence() {
    let (core, _) = Core::boot(
        Plan::decode(&baked()).unwrap(),
        Data,
        Box::new(MonospaceMeasurer::default()),
        390.,
        844.,
        None,
    )
    .unwrap();
    let node = core.runner.kernel().node(core.roots[0]).unwrap();
    let before = resolved(node);
    let mut after = before.clone();
    after.colors = std::array::from_fn(|index| [index as u32 + 1, index as u32 + 101]);
    let mut publication = Publication::default();
    publication.paint(node.id, &before, &before);
    assert!(publication.hot.is_empty());
    publication.paint(node.id, &before, &after);
    assert_eq!(publication.records, 1);
    assert_eq!(publication.hot[0], PAINT);
    assert_eq!(
        u32::from_le_bytes(publication.hot[1..5].try_into().unwrap()),
        56
    );
    assert_eq!(
        u32::from_le_bytes(publication.hot[9..13].try_into().unwrap()),
        63
    );
    for (bytes, color) in publication.hot[13..]
        .chunks_exact(4)
        .zip(after.colors.into_iter().flatten())
    {
        assert_eq!(u32::from_le_bytes(bytes.try_into().unwrap()), color);
    }
}

#[test]
fn new_shared_style_leaf_keeps_full_colors_after_an_existing_hot_touch() {
    use crate::wire::{STYLE_DEFINE, STYLE_REFERENCE};
    use exact_kernel::{
        style::{Color, ColorValue},
        wire::Op,
    };
    let (mut core, _) = Core::boot(
        Plan::decode(&baked()).unwrap(),
        Data,
        Box::new(MonospaceMeasurer::default()),
        390.,
        844.,
        None,
    )
    .unwrap();
    let row = core.runner.kernel().find_first_by_test_id("row-0").unwrap();
    let node = core.runner.kernel().node_by_key(row).unwrap();
    let (id, parent, mut full) = (node.id, node.parent.unwrap(), node.style.clone());
    let color = ColorValue::LightDark(Color(0x991122ff), Color(0x223344ff));
    full.background_color = Some(color);
    full.mask.set(StyleId::BackgroundColor);
    let mut hot = StyleProps {
        background_color: Some(color),
        ..StyleProps::default()
    };
    hot.mask.set(StyleId::BackgroundColor);
    let added = 10001;
    let mut children = core.runner.kernel().node(parent).unwrap().children();
    children.push(added);
    let receipt = core
        .runner
        .kernel_mut()
        .apply(
            core.roots[0],
            1000,
            &[
                Op::SetStyle {
                    id,
                    patch: Box::new(hot),
                },
                Op::CreateView {
                    id: added,
                    node_type: NodeType::Text,
                },
                Op::SetStyle {
                    id: added,
                    patch: Box::new(full),
                },
                Op::SetProp {
                    id: added,
                    prop: PropId::Text,
                    value: "Added".into(),
                },
                Op::SetChildren {
                    id: parent,
                    children,
                },
            ],
        )
        .unwrap();
    assert!(
        Rc::ptr_eq(
            &core.runner.kernel().node(id).unwrap().shared_style(),
            &core.runner.kernel().node(added).unwrap().shared_style(),
        ),
        "fixture must exercise the same authored-style memo key"
    );
    let first = CommitReceipt {
        touched: vec![row],
        ..CommitReceipt::default()
    };
    let wire = core.publish(&[first, receipt], false, None);
    let operations = records(&wire);
    let paint = operations
        .iter()
        .find_map(|(opcode, payload)| {
            (*opcode == PAINT && u32::from_le_bytes(payload[..4].try_into().unwrap()) == id)
                .then_some(*payload)
        })
        .unwrap();
    assert_eq!(u32::from_le_bytes(paint[4..8].try_into().unwrap()), 1);
    assert_eq!(
        u32::from_le_bytes(paint[8..12].try_into().unwrap()),
        0xff991122
    );
    assert_eq!(
        u32::from_le_bytes(paint[12..16].try_into().unwrap()),
        0xff223344
    );
    let create = format!(r#"{{"op":"create","id":{added},"#);
    let reference = operations
        .iter()
        .find_map(|(opcode, payload)| {
            (*opcode == STYLE_REFERENCE && payload[4..].starts_with(create.as_bytes()))
                .then_some(*payload)
        })
        .expect("new leaf needs a complete style definition");
    let json = operations
        .iter()
        .find_map(|(opcode, payload)| {
            (*opcode == STYLE_DEFINE && payload[..4] == reference[..4])
                .then(|| std::str::from_utf8(&payload[4..]).unwrap())
        })
        .unwrap();
    assert!(
        json.contains(r#""background_color":[[153,17,34,255],[34,51,68,255]]"#),
        "{json}"
    );
    assert!(json.contains(r#""text_color":[26,28,30,255]"#), "{json}");
    assert!(json.contains(r#""font_size":16"#), "{json}");
}
#[test]
fn explicit_color_mask_survives_equal_inherited_publication_then_clear() {
    use exact_kernel::{
        style::{Color, ColorValue},
        wire::Op,
    };
    let plan = contract::compile(
        r##"component A
  view
    column testId="parent" color="#112233" font-size=14
      text "Inherited" testId="inherited"
      text "Own" testId="own" color="#112233"
"##,
    )
    .unwrap();
    let (mut core, _) = Core::boot(
        plan,
        (),
        Box::new(MonospaceMeasurer::default()),
        320.,
        640.,
        None,
    )
    .unwrap();
    let id = |core: &Core<()>, name: &str| {
        let key = core.runner.kernel().find_first_by_test_id(name).unwrap();
        core.runner.kernel().node_by_key(key).unwrap().id
    };
    let (parent, inherited, own) = (
        id(&core, "parent"),
        id(&core, "inherited"),
        id(&core, "own"),
    );
    let mut patch = StyleProps {
        text_color: ColorValue::Fixed(Color(0x445566ff)),
        ..StyleProps::default()
    };
    patch.mask.set(StyleId::TextColor);
    for (batch, op, changed, unchanged) in [
        (
            1,
            Op::SetStyle {
                id: parent,
                patch: Box::new(patch),
            },
            inherited,
            own,
        ),
        (
            2,
            Op::ClearStyle {
                id: own,
                mask: StyleMask::of(StyleId::TextColor),
            },
            own,
            inherited,
        ),
    ] {
        let receipt = core
            .runner
            .kernel_mut()
            .apply(core.roots[0], batch, &[op])
            .unwrap();
        let wire = core.publish(&[receipt], false, None);
        let paints: Vec<_> = records(&wire)
            .into_iter()
            .filter_map(|(opcode, payload)| (opcode == PAINT).then_some(payload))
            .collect();
        assert!(paints
            .iter()
            .all(|payload| u32::from_le_bytes(payload[..4].try_into().unwrap()) != unchanged));
        let paint = paints
            .into_iter()
            .find(|payload| u32::from_le_bytes(payload[..4].try_into().unwrap()) == changed)
            .expect("only the leaf affected by inheritance/clear must repaint");
        assert_ne!(u32::from_le_bytes(paint[4..8].try_into().unwrap()) & 2, 0);
        assert_eq!(
            u32::from_le_bytes(paint[8..12].try_into().unwrap()),
            0xff445566
        );
        assert_eq!(
            u32::from_le_bytes(paint[12..16].try_into().unwrap()),
            0xff445566
        );
    }
}
#[test]
fn cold_layout_change_keeps_color_delta_after_a_direct_paint_turn() {
    use exact_kernel::{
        style::{Color, ColorValue},
        wire::Op,
    };

    let (mut core, _) = Core::boot(
        Plan::decode(&baked()).unwrap(),
        Data,
        Box::new(MonospaceMeasurer::default()),
        390.,
        844.,
        None,
    )
    .unwrap();
    let action = core
        .runner
        .kernel()
        .find_first_by_test_id("toggle-batch")
        .unwrap();
    let action = core.runner.kernel().node_by_key(action).unwrap().id;
    core.dispatch(action, Event::Press, 1.);
    let row = core.runner.kernel().find_first_by_test_id("row-0").unwrap();
    let id = core.runner.kernel().node_by_key(row).unwrap().id;
    let mut patch = StyleProps {
        width: Dimension::Points(200.),
        background_color: Some(ColorValue::Fixed(Color::WHITE)),
        ..StyleProps::default()
    };
    patch.mask.set(StyleId::Width);
    patch.mask.set(StyleId::BackgroundColor);
    let root = core.roots[0];
    let receipt = core
        .runner
        .kernel_mut()
        .apply(
            root,
            1000,
            &[Op::SetStyle {
                id,
                patch: Box::new(patch),
            }],
        )
        .unwrap();
    let wire = core.publish(&[receipt], false, None);
    let paint = records(&wire)
        .into_iter()
        .find_map(|(opcode, payload)| {
            (opcode == PAINT && u32::from_le_bytes(payload[..4].try_into().unwrap()) == id)
                .then_some(payload)
        })
        .expect("layout-only cold JSON must not suppress the changed background");
    assert_eq!(u32::from_le_bytes(paint[4..8].try_into().unwrap()), 1);
    assert_eq!(paint[8..], [255; 8]);
    assert_eq!(
        core.mirror[&row].style.as_ref().unwrap().colors[0],
        [u32::MAX; 2]
    );
}

fn viewport_plan() -> Plan {
    contract::compile(
        r##"component Viewport
  state highlighted = false
  action paint
    highlighted = not highlighted
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding-left="10vw" padding-top="10vh" padding-right="env(safe-area-inset-right)" padding-bottom="env(safe-area-inset-bottom)" background-color=(highlighted ? "#445566" : "#112233")
      button testId="paint" press=paint
        text "Paint"
"##,
    )
    .unwrap()
}

fn published_style(wire: &[u8], id: u32) -> &str {
    use crate::wire::{JSON, STYLE_DEFINE, STYLE_REFERENCE};
    let operations = records(wire);
    let marker = format!(r#""id":{id},"#);
    let (opcode, reference) = operations
        .iter()
        .copied()
        .find(|(opcode, payload)| {
            let json = match *opcode {
                STYLE_REFERENCE => &payload[4..],
                JSON => payload,
                _ => return false,
            };
            let json = std::str::from_utf8(json).unwrap();
            (json.starts_with(r#"{"op":"create""#) || json.starts_with(r#"{"op":"style""#))
                && json.contains(&marker)
        })
        .expect("the native node needs an updated dictionary");
    if opcode == JSON {
        return std::str::from_utf8(reference).unwrap();
    }
    operations
        .into_iter()
        .find_map(|(opcode, payload)| {
            (opcode == STYLE_DEFINE && payload[..4] == reference[..4])
                .then(|| std::str::from_utf8(&payload[4..]).unwrap())
        })
        .unwrap()
}

fn id_named(core: &Core<()>, name: &str) -> u32 {
    let key = core.runner.kernel().find_first_by_test_id(name).unwrap();
    core.runner.kernel().node_by_key(key).unwrap().id
}

fn assert_viewport_parity(core: &Core<()>, reference: &exact_apple::Host<()>) {
    assert_eq!(core.env, core.runner.kernel().env());
    assert_eq!(core.env, reference.runner().kernel().env());
    for actual in core.runner.kernel().rows(None).unwrap() {
        let expected = reference.runner().kernel().node(actual.id).unwrap();
        assert!(actual.frame.bits_eq(expected.frame));
        let actual = core.runner.kernel().node(actual.id).unwrap();
        assert_eq!(actual.props, expected.props);
        assert_eq!(actual.style, expected.style);
    }
}

#[test]
fn initial_viewport_styles_are_resolved_before_create_and_frames_follow_creates() {
    let plan = viewport_plan();
    let (reference, _) = exact_apple::Host::boot(
        &plan.encode(),
        (),
        Box::new(MonospaceMeasurer::default()),
        320.,
        640.,
    )
    .unwrap();
    let (mut core, wire) = Core::boot(
        plan,
        (),
        Box::new(MonospaceMeasurer::default()),
        320.,
        640.,
        None,
    )
    .unwrap();
    let root = id_named(&core, "root");
    let style = published_style(&wire, root);
    assert!(style.contains(r#""padding_left":32"#), "{style}");
    assert!(style.contains(r#""padding_top":64"#), "{style}");
    let paint = id_named(&core, "paint");
    let frame = core.runner.kernel().node(paint).unwrap().frame;
    assert_eq!((frame.x, frame.y), (32., 64.));
    let operations = records(&wire);
    let mut created = std::collections::BTreeSet::new();
    for (opcode, payload) in operations {
        let json = match opcode {
            crate::wire::STYLE_REFERENCE => Some(&payload[4..]),
            crate::wire::JSON => Some(payload),
            _ => None,
        };
        if let Some(json) = json {
            if let Some(json) = std::str::from_utf8(json)
                .unwrap()
                .strip_prefix(r#"{"op":"create","id":"#)
            {
                let id = json.split(',').next().unwrap().parse::<u32>().unwrap();
                created.insert(id);
            }
        }
        if opcode == FRAME {
            let id = u32::from_le_bytes(payload[..4].try_into().unwrap());
            assert!(
                created.contains(&id),
                "FRAME arrived before CREATE for {id}"
            );
        }
    }
    assert_eq!(created.len(), core.mirror.len());
    assert_viewport_parity(&core, &reference);
    assert!(records(&core.quiet()).is_empty());
}

#[test]
fn resize_and_insets_publish_current_styles_then_leave_paint_and_noops_sparse() {
    let plan = viewport_plan();
    let (mut reference, _) = exact_apple::Host::boot(
        &plan.encode(),
        (),
        Box::new(MonospaceMeasurer::default()),
        320.,
        640.,
    )
    .unwrap();
    let (mut core, _) = Core::boot(
        plan,
        (),
        Box::new(MonospaceMeasurer::default()),
        320.,
        640.,
        None,
    )
    .unwrap();
    let root = id_named(&core, "root");
    let wire = core.resize(480., 800.);
    reference.resize(480., 800.);
    let style = published_style(&wire, root);
    assert!(style.contains(r#""padding_left":48"#), "{style}");
    assert!(style.contains(r#""padding_top":80"#), "{style}");
    assert_viewport_parity(&core, &reference);
    assert!(records(&core.resize(480., 800.)).is_empty());
    assert!(records(&core.quiet()).is_empty());

    let insets = Env::new(24., 8., 12., 16.);
    let wire = core.insets(insets.clone());
    reference.set_insets(24., 8., 12., 16.);
    let style = published_style(&wire, root);
    assert!(style.contains(r#""padding_left":48"#), "{style}");
    assert!(style.contains(r#""padding_top":80"#), "{style}");
    assert!(style.contains(r#""padding_right":8"#), "{style}");
    assert!(style.contains(r#""padding_bottom":12"#), "{style}");
    assert_viewport_parity(&core, &reference);
    assert!(records(&core.insets(insets)).is_empty());
    assert!(records(&core.quiet()).is_empty());

    let paint = id_named(&core, "paint");
    let wire = core.dispatch(paint, Event::Press, 1.);
    reference.dispatch_at(paint, Event::Press, 1.);
    let operations = records(&wire);
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].0, PAINT);
    assert_eq!(
        u32::from_le_bytes(operations[0].1[..4].try_into().unwrap()),
        root
    );
    assert_eq!(
        u32::from_le_bytes(operations[0].1[4..8].try_into().unwrap()),
        1
    );
    assert_viewport_parity(&core, &reference);
    assert!(records(&core.quiet()).is_empty());
}
