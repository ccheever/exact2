//! Atomic terminal reorder; no per-pointer app data or synthetic geometry.
use exact_kernel::Kernel;
use exact_plan::{Items, Value};
use exact_runner::{DataError, DataSource, Runner};
use interaction_gallery_data::Gallery;
use std::rc::Rc;

fn fields(value: &Value) -> &[Value] {
    let Value::Record(fields) = value else {
        panic!("gallery metadata")
    };
    assert_eq!(
        fields.len(),
        26,
        "reorder must preserve existing metadata wire"
    );
    fields
}

fn snapshot(source: &mut Gallery) -> Value {
    source.query("gallery", &[]).unwrap()
}
fn revision(value: &Value) -> u32 {
    fields(value)[7].as_number().unwrap() as u32
}

fn action(source: &mut Gallery, op: &str, id: &str, n: u32) -> Value {
    source
        .query(
            "galleryAction",
            &[Value::str(op), Value::str(id), Value::Number(n as f64)],
        )
        .unwrap()
}

fn terminal(source: &mut Gallery, item: &str, before: Option<&str>, revision: u32) -> Value {
    source
        .query(
            "galleryReorder",
            &[
                Value::str(item),
                Value::Option(before.map(|key| Rc::new(Value::str(key)))),
                Value::Number(revision as f64),
            ],
        )
        .expect("typed domain refusal must return a GalleryState, not DataError")
}

fn rows(source: &mut Gallery, revision: u32) -> Items {
    let Value::List(rows) = source
        .query(
            "galleryRows",
            &[
                Value::Number(revision as f64),
                Value::Number(0.),
                Value::Bool(true),
            ],
        )
        .unwrap()
    else {
        panic!("rows")
    };
    rows
}

fn ids(rows: &[Value]) -> Vec<String> {
    rows.iter()
        .map(|row| {
            let Value::Record(fields) = row else {
                panic!("photo")
            };
            assert_eq!(fields.len(), 8);
            fields[0].as_str().unwrap().to_owned()
        })
        .collect()
}

fn refusal(value: &Value, reason: &str) {
    let notice = fields(value)[19].as_str().unwrap();
    assert!(notice.starts_with("Move refused: "), "{notice}");
    assert!(notice.contains(reason), "{notice} must explain {reason}");
}

#[test]
fn atomic_reorder_moves_up_down_and_to_end_once_by_current_identity() {
    let mut source = Gallery::default();
    let initial = snapshot(&mut source);
    let old_rows = rows(&mut source, revision(&initial));
    let first = terminal(&mut source, "photo-00004", Some("photo-00001"), 0);
    assert_eq!(revision(&first), 1);
    let first_rows = rows(&mut source, 1);
    assert!(!Items::ptr_eq(&old_rows, &first_rows));
    assert_eq!(
        &ids(&first_rows)[..6],
        [
            "photo-00000",
            "photo-00004",
            "photo-00001",
            "photo-00002",
            "photo-00003",
            "photo-00005"
        ]
    );
    let second = terminal(&mut source, "photo-00000", Some("photo-00005"), 1);
    assert_eq!(revision(&second), 2);
    assert_eq!(
        &ids(&rows(&mut source, 2))[..6],
        [
            "photo-00004",
            "photo-00001",
            "photo-00002",
            "photo-00003",
            "photo-00000",
            "photo-00005"
        ]
    );
    let third = terminal(&mut source, "photo-00004", None, 2);
    assert_eq!(revision(&third), 3);
    let order = ids(&rows(&mut source, 3));
    assert_eq!(order.last().unwrap(), "photo-00004");
    assert_eq!(order.len(), old_rows.len());
    assert_eq!(
        ids(&old_rows)[4],
        "photo-00004",
        "prior snapshot remains immutable"
    );
}

#[test]
fn stale_duplicate_returns_current_metadata_without_invalidating_rows() {
    let mut source = Gallery::default();
    let moved = terminal(&mut source, "photo-00004", Some("photo-00001"), 0);
    assert_eq!(revision(&moved), 1);
    action(&mut source, "page", "", 3);
    action(&mut source, "select", "photo-00070", 0);
    let before = snapshot(&mut source);
    let cached = rows(&mut source, 1);
    let refused = terminal(&mut source, "photo-00004", Some("photo-00001"), 0);
    refusal(&refused, "revision");
    for (index, value) in fields(&before).iter().enumerate() {
        if index != 19 {
            assert_eq!(&fields(&refused)[index], value);
        }
    }
    assert_eq!(
        snapshot(&mut source),
        before,
        "refusal notice is returned, not model state"
    );
    assert!(Items::ptr_eq(&cached, &rows(&mut source, 1)));
}

#[test]
fn self_adjacent_and_already_at_end_are_normalized_noops() {
    let mut source = Gallery::default();
    let before = snapshot(&mut source);
    let cached = rows(&mut source, 0);
    for (item, destination) in [
        ("photo-00001", Some("photo-00001")),
        ("photo-00001", Some("photo-00002")),
        ("photo-00099", None),
    ] {
        assert_eq!(terminal(&mut source, item, destination, 0), before);
        assert_eq!(snapshot(&mut source), before);
        assert!(Items::ptr_eq(&cached, &rows(&mut source, 0)));
    }
}

#[test]
fn missing_source_destination_and_empty_some_never_fall_back_to_end() {
    let mut source = Gallery::default();
    let before = snapshot(&mut source);
    let cached = rows(&mut source, 0);
    for (item, destination) in [
        ("photo-99999", Some("photo-00001")),
        ("photo-00000", Some("photo-99999")),
        ("photo-00000", Some("")),
        ("not-a-photo", None),
    ] {
        refusal(&terminal(&mut source, item, destination, 0), "");
        assert_eq!(snapshot(&mut source), before);
        assert!(Items::ptr_eq(&cached, &rows(&mut source, 0)));
    }
}

#[test]
fn deleted_source_or_destination_cannot_reappear_or_use_a_rank_fallback() {
    for removed in ["photo-00003", "photo-00007"] {
        let mut source = Gallery::default();
        action(&mut source, "delete", removed, 0);
        let before = snapshot(&mut source);
        let current = revision(&before);
        let cached = rows(&mut source, current);
        refusal(
            &terminal(&mut source, "photo-00003", Some("photo-00007"), 0),
            "revision",
        );
        refusal(
            &terminal(&mut source, "photo-00003", Some("photo-00007"), current),
            "",
        );
        assert_eq!(snapshot(&mut source), before);
        assert!(Items::ptr_eq(&cached, &rows(&mut source, current)));
        assert!(!ids(&cached).contains(&removed.to_owned()));
    }
}

#[test]
fn manual_move_blocks_atomic_drop_without_consuming_its_preview_or_token() {
    let mut source = Gallery::default();
    action(&mut source, "lift", "photo-00003", 0);
    let token = fields(&snapshot(&mut source))[15].as_number().unwrap() as u32;
    action(&mut source, "before", "photo-00007", token);
    let before = snapshot(&mut source);
    let cached = rows(&mut source, 0);
    refusal(&terminal(&mut source, "photo-00005", None, 0), "manual");
    assert_eq!(snapshot(&mut source), before);
    assert!(Items::ptr_eq(&cached, &rows(&mut source, 0)));
    let placed = action(&mut source, "place", "", token);
    assert_eq!(revision(&placed), 1);
    let order = ids(&rows(&mut source, 1));
    let position = order.iter().position(|key| key == "photo-00003").unwrap();
    assert_eq!(order[position + 1], "photo-00007");
}

#[test]
fn malformed_reorder_wire_refuses_without_changing_model_or_rows() {
    let mut source = Gallery::default();
    let before = snapshot(&mut source);
    let cached = rows(&mut source, 0);
    for args in [
        vec![],
        vec![Value::str("photo-00001"), Value::str(""), Value::Number(0.)],
        vec![
            Value::str("photo-00001"),
            Value::Option(Some(Rc::new(Value::Number(1.)))),
            Value::Number(0.),
        ],
    ] {
        assert!(source.query("galleryReorder", &args).is_err());
    }
    for revision in [f64::NAN, f64::INFINITY, -1., 0.5, u32::MAX as f64 + 1.] {
        assert!(source
            .query(
                "galleryReorder",
                &[
                    Value::str("photo-00001"),
                    Value::Option(None),
                    Value::Number(revision)
                ]
            )
            .is_err());
    }
    assert_eq!(snapshot(&mut source), before);
    assert!(Items::ptr_eq(&cached, &rows(&mut source, 0)));
}

#[derive(Default)]
struct Counted {
    data: Gallery,
    reorder_calls: usize,
    row_calls: usize,
}

impl DataSource for Counted {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.reorder_calls += usize::from(source == "galleryReorder");
        self.row_calls += usize::from(source == "galleryRows");
        self.data.query(source, args)
    }
}

#[test]
fn real_runner_terminal_is_synchronous_and_stale_refusal_does_not_poison_actions() {
    // Uses only today's mutation API; no pending common reorder schema required.
    let shapes = include_str!("../../../app.contract")
        .split("component InteractionGallery")
        .next()
        .unwrap();
    let plan = contract::compile(&format!("{shapes}\ncomponent ReorderProof\n  resource initial = gallery() as shape GalleryState\n  mutation changed as shape GalleryState\n  derive gallery = match changed {{ case some(value) => value, case none => initial }}\n  resource rows = galleryRows(gallery.revision, 0, true) as shape list<Photo>\n  state draft = \"\"\n  action place(item: string, before: option<string>, revision: number)\n    send changed = galleryReorder(item, before, revision)\n  action edit(value: string)\n    draft = value\n  view\n    column\n      text gallery.notice testId=\"notice\"\n      text draft testId=\"draft\"\n")).unwrap();
    let mut runner = Runner::boot(
        plan,
        Counted::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let tuple = vec![
        Value::str("photo-00003"),
        Value::Option(Some(Rc::new(Value::str("photo-00001")))),
        Value::Number(0.),
    ];
    runner.act("place", tuple.clone()).unwrap();
    assert!(!runner.has_pending());
    assert!(runner.take_requests().is_empty());
    assert_eq!(runner.data_ref().reorder_calls, 1);
    assert_eq!(runner.data_ref().row_calls, 2);
    let Value::List(rows) = runner.resource("rows").unwrap() else {
        panic!("rows")
    };
    let current = rows.clone();
    assert_eq!(
        &ids(&current)[..4],
        ["photo-00000", "photo-00003", "photo-00001", "photo-00002"]
    );
    runner
        .act("place", tuple)
        .expect("stale terminal is ordinary data, never Runner poison");
    assert_eq!(runner.data_ref().reorder_calls, 2);
    assert_eq!(runner.data_ref().row_calls, 2);
    let Value::List(rows) = runner.resource("rows").unwrap() else {
        panic!("rows")
    };
    assert!(Items::ptr_eq(&current, rows));
    runner
        .act("edit", vec![Value::str("still usable")])
        .unwrap();
    assert_eq!(runner.slot("draft"), Some(&Value::str("still usable")));
}

// App/common validation below waits for the exact frozen common overlay. These
// are synthesized typed terminals, not evidence of pointer recognition or pins.
fn boot_app() -> Runner<Counted> {
    let plan = contract::compile(include_str!("../../../app.contract")).unwrap();
    let baked = contract::bake(plan, Gallery::default()).unwrap();
    Runner::boot(
        baked,
        Counted::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn app_key(runner: &Runner<Counted>, name: &str) -> exact_kernel::NodeKey {
    let keys = runner.kernel().find_by_test_id(name);
    assert_eq!(keys.len(), 1, "missing or ambiguous {name}");
    keys[0]
}

fn app_press(runner: &mut Runner<Counted>, name: &str) {
    let id = runner
        .kernel()
        .node_by_key(app_key(runner, name))
        .unwrap()
        .id;
    runner.dispatch(id, exact_runner::Event::Press).unwrap();
}

fn app_rows(runner: &Runner<Counted>) -> Items {
    let Value::List(rows) = runner.resource("rows").unwrap() else {
        panic!("rows")
    };
    rows.clone()
}

fn app_drop(runner: &mut Runner<Counted>, item: &str, before: Option<&str>) {
    let list = runner
        .kernel()
        .node_by_key(app_key(runner, "reorder-scroll"))
        .unwrap()
        .id;
    runner
        .dispatch(
            list,
            exact_runner::Event::ReorderDrop {
                item: item.into(),
                before: before.map(str::to_owned),
            },
        )
        .unwrap();
    assert!(!runner.has_pending());
    assert!(runner.take_requests().is_empty());
}

#[test]
fn authored_windowed_grips_are_nonbuttons_and_disabled_during_manual_move() {
    use exact_kernel::{NodeType, PropId};
    let mut runner = boot_app();
    app_press(&mut runner, "mode-reorder");
    let list = runner
        .kernel()
        .node_by_key(app_key(&runner, "reorder-scroll"))
        .unwrap();
    assert_eq!(list.props.str(PropId::Id), Some("arrange-list"));
    assert_eq!(list.props.bool(PropId::Virtualized), Some(true));
    let handle = app_key(&runner, "arrange-grip-photo-00000");
    let grip = runner.kernel().node_by_key(handle).unwrap();
    assert_eq!(grip.node_type, NodeType::View);
    assert_eq!(grip.props.str(PropId::ReorderFor), Some("arrange-list"));
    assert_ne!(grip.props.bool(PropId::Disabled), Some(true));
    let original = app_rows(&runner);
    app_press(&mut runner, "lift-photo-00000");
    for index in 0..3 {
        let name = format!("arrange-grip-photo-{index:05}");
        let grip = runner
            .kernel()
            .node_by_key(app_key(&runner, &name))
            .unwrap();
        assert_eq!(grip.props.bool(PropId::Disabled), Some(true));
    }
    assert!(Items::ptr_eq(&original, &app_rows(&runner)));
    let queries = runner.data_ref().reorder_calls;
    app_drop(&mut runner, "photo-00003", None);
    assert_eq!(runner.data_ref().reorder_calls, queries + 1);
    assert!(
        Items::ptr_eq(&original, &app_rows(&runner)),
        "manual exclusion applies even to synthesized delivery"
    );
    app_press(&mut runner, "cancel");
    assert_ne!(
        runner
            .kernel()
            .node_by_key(app_key(&runner, "arrange-grip-photo-00000"))
            .unwrap()
            .props
            .bool(PropId::Disabled),
        Some(true)
    );
    for mode in ["render-manual", "render-eager"] {
        app_press(&mut runner, mode);
        assert!(runner.collections().is_empty());
        assert!(runner
            .kernel()
            .find_by_test_id("arrange-grip-photo-00000")
            .is_empty());
        assert_eq!(runner.kernel().find_by_test_id("lift-photo-00000").len(), 1);
    }
    app_press(&mut runner, "render-windowed");
    assert_eq!(
        runner
            .kernel()
            .find_by_test_id("arrange-grip-photo-00000")
            .len(),
        1
    );
}

#[test]
fn actual_list_terminal_uses_latest_revision_and_keeps_orthogonal_local_state() {
    let mut runner = boot_app();
    app_press(&mut runner, "count-25000");
    app_press(&mut runner, "mode-reorder");
    runner
        .act("chooseSheet", vec![Value::Number(640.)])
        .unwrap();
    let original = app_rows(&runner);
    let queries = (runner.data_ref().reorder_calls, runner.data_ref().row_calls);
    runner
        .act("edit", vec![Value::str("still typing")])
        .unwrap();
    assert_eq!(
        (runner.data_ref().reorder_calls, runner.data_ref().row_calls),
        queries
    );
    assert!(Items::ptr_eq(&original, &app_rows(&runner)));
    assert_eq!(runner.last_instance_work().rows_keyed, 0);
    app_drop(&mut runner, "photo-00000", Some("photo-24999"));
    assert_eq!(runner.data_ref().reorder_calls, queries.0 + 1);
    let moved = app_rows(&runner);
    assert_eq!(ids(&moved)[24_998], "photo-00000");
    assert_eq!(ids(&moved)[24_999], "photo-24999");
    assert_eq!(
        ids(&original)[0],
        "photo-00000",
        "accepted old snapshot is immutable"
    );
    let row_queries = runner.data_ref().row_calls;
    app_drop(&mut runner, "photo-00000", Some("photo-24999"));
    assert!(
        Items::ptr_eq(&moved, &app_rows(&runner)),
        "duplicate gap is unchanged"
    );
    assert_eq!(runner.data_ref().row_calls, row_queries);
    app_drop(&mut runner, "photo-00000", None);
    assert_eq!(ids(&app_rows(&runner)).last().unwrap(), "photo-00000");
    assert_eq!(
        runner.data_ref().row_calls,
        row_queries + 1,
        "handler must read the new structural revision"
    );
    assert_eq!(runner.slot("draft"), Some(&Value::str("still typing")));
    assert_eq!(runner.slot("sheetPx"), Some(&Value::Number(640.)));
    assert!(runner.kernel().find_by_test_id("move-preview").is_empty());
    assert_eq!(runner.collections()[0].count, 25_000);
    assert!(runner.collections()[0].rows.len() < 100);
}

#[test]
fn actual_list_missing_destination_refuses_and_following_valid_terminal_still_works() {
    use exact_kernel::PropId;
    let mut runner = boot_app();
    app_press(&mut runner, "mode-reorder");
    app_press(&mut runner, "delete-photo-00003");
    let original = app_rows(&runner);
    let row_queries = runner.data_ref().row_calls;
    for (item, before) in [("photo-00000", Some("photo-00003")), ("photo-00003", None)] {
        app_drop(&mut runner, item, before);
        assert!(Items::ptr_eq(&original, &app_rows(&runner)));
        assert_eq!(runner.data_ref().row_calls, row_queries);
        let status = runner
            .kernel()
            .node_by_key(app_key(&runner, "status"))
            .unwrap();
        assert!(status
            .props
            .str(PropId::Text)
            .unwrap()
            .starts_with("Move refused: "));
    }
    runner
        .act("edit", vec![Value::str("refusal is not failstop")])
        .unwrap();
    app_drop(&mut runner, "photo-00002", Some("photo-00000"));
    assert_eq!(
        &ids(&app_rows(&runner))[..3],
        ["photo-00002", "photo-00000", "photo-00001"]
    );
    assert_eq!(
        runner.slot("draft"),
        Some(&Value::str("refusal is not failstop"))
    );
}
