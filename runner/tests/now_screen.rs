//! A hand-built plan of the Caltrain "Now" screen's shape, run end to end:
//! boot → kernel tree, press → state → resource re-request → keyed rows,
//! text change → `when` flip, a timer under the seekable clock, `match` on an
//! option, and the refusals that leave the kernel untouched.
//!
//! This plan is also the first expected output of the compiler corpus
//! (LLP 1004 D6): compiling the equivalent `.contract` must yield these tables.

use exact_kernel::{Kernel, NodeType, PropId, StyleId};
use exact_plan::asm::Asm;
use exact_plan::builder::PlanBuilder;
use exact_plan::{
    BindingKind, BindingsRow, Code, EventKind, Opcode, Plan, RegionKind, Stdlib, TypeKind, TypesId,
    Value,
};
use exact_runner::{DataError, DataSource, Event, Runner, RunnerError, Trap};

/// The app's data crate, in miniature: a schedule and the queries over it.
#[derive(Default)]
struct Schedule {
    queries: Vec<(String, Vec<Value>)>,
    wrong_shape: bool,
}

fn station(id: &str, name: &str) -> Value {
    Value::record(vec![Value::str(id), Value::str(name)])
}

fn departure(id: &str, train: f64, at_ms: f64) -> Value {
    Value::record(vec![
        Value::str(id),
        Value::Number(train),
        Value::Number(at_ms),
    ])
}

impl DataSource for Schedule {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.queries.push((source.to_string(), args.to_vec()));
        match source {
            "stations" => Ok(Value::list(vec![
                station("mv", "Mountain View"),
                station("pa", "Palo Alto"),
            ])),
            "departures" => {
                if self.wrong_shape {
                    return Ok(Value::list(vec![Value::str("not a departure")]));
                }
                let id = args
                    .first()
                    .and_then(Value::as_str)
                    .ok_or(DataError::BadArguments("station".into()))?;
                Ok(match id {
                    "mv" => Value::list(vec![
                        departure("d1", 101.0, 600_000.0),
                        departure("d2", 103.0, 1_500_000.0),
                    ]),
                    "pa" => Value::list(vec![
                        departure("d2", 103.0, 1_200_000.0),
                        departure("d1", 101.0, 300_000.0),
                        departure("d9", 109.0, 9_000_000.0),
                    ]),
                    other => return Err(DataError::Unavailable(other.into())),
                })
            }
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}

fn style_id(name: &str) -> u16 {
    StyleId::from_name(name).unwrap() as u16
}

fn prop_id(name: &str) -> u16 {
    PropId::from_name(name).unwrap() as u16
}

fn style(id: &str, expr: Code) -> BindingsRow {
    BindingsRow {
        kind: BindingKind::Style,
        id: style_id(id),
        expr,
    }
}

fn prop(id: &str, expr: Code) -> BindingsRow {
    BindingsRow {
        kind: BindingKind::Prop,
        id: prop_id(id),
        expr,
    }
}

/// The plan. Layout, in Contract terms:
///
/// ```text
/// state stationId = none          state query = ""        state nowMs = 0
/// derive selected = match stationId { some(id) => id, none => "mv" }
/// resource stations = stations()               (compiled: constant args)
/// resource board = departures(selected)        (requested when `selected` changes)
/// derive count = length(board)
/// action selectStation(id) writes stationId    action setQuery(q) writes query
/// action tick writes nowMs                     task ticker mount: every(1000, tick)
/// view
///   column testId="main"
///     text `${count} trains` size=24 weight=700 testId="count"
///     when query == ""
///       each d in board key=d.id
///         button press=selectStation(d.id) label=`Train ${d.train}` testId=`dep-${d.id}`
///           text formatCountdownMinutes(d.at, nowMs)
///     else
///       text "searching" testId="searching"
///     input value=query change=setQuery testId="search"
///     match stationId
///       case some(id) => text `at ${id}` testId="selected"
///       case none => text "nearest" testId="nearest"
/// ```
fn now_screen() -> (Plan, Vec<TypesId>) {
    let mut b = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let number = b.primitive(TypeKind::Number);
    let string = b.primitive(TypeKind::String);
    let opt_string = b.option(string);
    let station_ty = b.record("Station", &[("id", string), ("name", string)]);
    let stations_ty = b.list(station_ty);
    let dep_ty = b.record(
        "Departure",
        &[("id", string), ("train", number), ("at", number)],
    );
    let deps_ty = b.list(dep_ty);

    let none = b.constant(&Value::NONE);
    let station_id = b.slot("stationId", opt_string, none);
    let empty = b.constant(&Value::str(""));
    let query = b.slot("query", string, empty);
    let zero = b.constant(&Value::Number(0.0));
    let now_ms = b.slot("nowMs", number, zero);

    // derive selected = match stationId { some(id) => id, none => "mv" }
    let mv = b.str("mv");
    let mut asm = Asm::new();
    let is_none = asm.label();
    let end = asm.label();
    asm.load_slot(station_id)
        .jump_if_none(is_none)
        .simple(Opcode::Unwrap)
        .jump(end)
        .place(is_none)
        .simple(Opcode::Pop)
        .str(mv)
        .place(end);
    let selected_body = b.code(asm);
    let selected = b.derive("selected", string, selected_body);

    let initial_stations = Value::list(vec![
        station("mv", "Mountain View"),
        station("pa", "Palo Alto"),
    ]);
    let _stations = b.resource(
        "stations",
        "stations",
        &[],
        stations_ty,
        Some(&initial_stations),
    );
    let mut arg = Asm::new();
    arg.load_derive(selected);
    let arg = b.code(arg);
    let board = b.resource("board", "departures", &[arg], deps_ty, None);

    let mut count_body = Asm::new();
    count_body.load_resource(board).call(Stdlib::Length);
    let count_body = b.code(count_body);
    let count = b.derive("count", number, count_body);

    // actions
    let mut sel = Asm::new();
    sel.load_param(0)
        .simple(Opcode::Some)
        .store_slot(station_id);
    let sel = b.code(sel);
    let select_station = b.action("selectStation", &[("id", string)], &[station_id], sel);
    let mut setq = Asm::new();
    setq.load_param(0).store_slot(query);
    let setq = b.code(setq);
    let set_query = b.action("setQuery", &[("q", string)], &[query], setq);
    let mut tick = Asm::new();
    tick.call(Stdlib::Now).store_slot(now_ms);
    let tick = b.code(tick);
    let tick = b.action("tick", &[], &[now_ms], tick);
    b.timer(1000, tick);
    // A bad action: writes a slot it did not declare.
    let mut rogue = Asm::new();
    rogue.number(1.0).store_slot(now_ms);
    let rogue = b.code(rogue);
    b.action("rogue", &[], &[query], rogue);
    // A command-emitting action.
    let set_scheme = b.str("setScheme");
    let mut cmd = Asm::new();
    cmd.load_param(0).command(set_scheme, 1);
    let cmd = b.code(cmd);
    b.action("setDark", &[("s", string)], &[], cmd);

    // view
    let main_id = b.constant(&Value::str("main"));
    let column = b.constant(&Value::str("column"));
    let root = b.node(
        NodeType::View as u8,
        None,
        None,
        0,
        &[style("flex_direction", column), prop("testId", main_id)],
        &[],
        None,
    );
    let trains = b.str(" trains");
    let mut count_text = Asm::new();
    count_text
        .load_derive(count)
        .call(Stdlib::ToString)
        .str(trains)
        .simple(Opcode::Concat);
    let count_text = b.code(count_text);
    let count_tid = b.constant(&Value::str("count"));
    let size24 = b.constant(&Value::Number(24.0));
    let w700 = b.constant(&Value::Number(700.0));
    b.node(
        NodeType::Text as u8,
        Some(root),
        None,
        0,
        &[
            prop("text", count_text),
            prop("testId", count_tid),
            style("font_size", size24),
            style("font_weight", w700),
        ],
        &[],
        None,
    );

    // when query == "" ... else ...
    let mut cond = Asm::new();
    let empty_s = b.str("");
    cond.load_slot(query).str(empty_s).simple(Opcode::Eq);
    let cond = b.code(cond);
    let unit = b.constant(&Value::Unit);
    let (_when, when_arms) = b.region(RegionKind::When, Some(root), None, 1, cond, unit, 2);
    // then-arm: each d in board key=d.id
    let mut subject = Asm::new();
    subject.load_resource(board);
    let subject = b.code(subject);
    let mut key = Asm::new();
    key.load_item(0).field(0);
    let key = b.code(key);
    let (_each, each_arms) = b.region(
        RegionKind::Each,
        None,
        Some(when_arms[0]),
        0,
        subject,
        key,
        1,
    );
    // Inside the row, frames are [when-arm, row], innermost last: the item is depth 0.
    let train = b.str("Train ");
    let dep_prefix = b.str("dep-");
    let mut label = Asm::new();
    label
        .str(train)
        .load_item(0)
        .field(1)
        .call(Stdlib::ToString)
        .simple(Opcode::Concat);
    let label = b.code(label);
    let mut tid = Asm::new();
    tid.str(dep_prefix)
        .load_item(0)
        .field(0)
        .simple(Opcode::Concat);
    let tid = b.code(tid);
    let mut press_arg = Asm::new();
    press_arg.load_item(0).field(0);
    let press_arg = b.code(press_arg);
    let button = b.node(
        NodeType::Pressable as u8,
        None,
        Some(each_arms[0]),
        0,
        &[prop("accessibilityLabel", label), prop("testId", tid)],
        &[(EventKind::Press, select_station, &[press_arg])],
        None,
    );
    let mut countdown = Asm::new();
    countdown
        .load_item(0)
        .field(2)
        .load_slot(now_ms)
        .call(Stdlib::FormatCountdownMinutes);
    let countdown = b.code(countdown);
    b.node(
        NodeType::Text as u8,
        Some(button),
        Some(each_arms[0]),
        0,
        &[prop("text", countdown)],
        &[],
        None,
    );
    // else-arm
    let searching = b.constant(&Value::str("searching"));
    b.node(
        NodeType::Text as u8,
        None,
        Some(when_arms[1]),
        0,
        &[prop("text", searching), prop("testId", searching)],
        &[],
        None,
    );

    // input
    let mut value = Asm::new();
    value.load_slot(query);
    let value = b.code(value);
    let search_tid = b.constant(&Value::str("search"));
    b.node(
        NodeType::TextInput as u8,
        Some(root),
        None,
        2,
        &[prop("value", value), prop("testId", search_tid)],
        &[(EventKind::Change, set_query, &[])],
        None,
    );

    // match stationId
    let mut scrutinee = Asm::new();
    scrutinee.load_slot(station_id);
    let scrutinee = b.code(scrutinee);
    let (_m, match_arms) = b.region(RegionKind::Match, Some(root), None, 3, scrutinee, unit, 2);
    let at = b.str("at ");
    let mut at_text = Asm::new();
    at_text.str(at).load_bound(0).simple(Opcode::Concat);
    let at_text = b.code(at_text);
    let selected_tid = b.constant(&Value::str("selected"));
    b.node(
        NodeType::Text as u8,
        None,
        Some(match_arms[0]),
        0,
        &[prop("text", at_text), prop("testId", selected_tid)],
        &[],
        None,
    );
    let nearest = b.constant(&Value::str("nearest"));
    b.node(
        NodeType::Text as u8,
        None,
        Some(match_arms[1]),
        0,
        &[prop("text", nearest), prop("testId", nearest)],
        &[],
        None,
    );

    (
        b.finish().unwrap(),
        vec![
            number,
            string,
            opt_string,
            station_ty,
            stations_ty,
            dep_ty,
            deps_ty,
        ],
    )
}

fn boot() -> Runner<Schedule> {
    let (plan, _) = now_screen();
    Runner::boot(plan, Schedule::default(), Kernel::with_monospace()).unwrap()
}

fn text_by_test_id(r: &Runner<Schedule>, test_id: &str) -> Option<String> {
    let k = r.kernel();
    let key = k.find_by_test_id(test_id).into_iter().next()?;
    let node = k.node_by_key(key)?;
    node.props.str(PropId::Text).map(str::to_string)
}

fn test_ids(r: &Runner<Schedule>, prefix: &str) -> Vec<(String, u32)> {
    let k = r.kernel();
    let mut out = Vec::new();
    for root in k.roots() {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let n = k.node(id).unwrap();
            if let Some(t) = n.props.str(PropId::TestId) {
                if t.starts_with(prefix) {
                    out.push((t.to_string(), id));
                }
            }
            let mut children = n.children();
            children.reverse();
            stack.extend(children);
        }
    }
    out
}

#[test]
fn the_plan_round_trips_through_bytes_and_boots_to_the_expected_tree() {
    let (plan, _) = now_screen();
    let bytes = plan.encode();
    let decoded = Plan::decode(&bytes).unwrap();
    assert_eq!(decoded, plan);
    let r = Runner::boot(decoded, Schedule::default(), Kernel::with_monospace()).unwrap();
    assert_eq!(r.roots().len(), 1);
    // Compiled data needs no query; the state-argument resource is requested once at boot.
    assert_eq!(
        r.kernel().live_count(),
        1 + 1 + 2 * 2 + 1 + 1,
        "root, count, two rows of two, input, nearest"
    );
    assert_eq!(text_by_test_id(&r, "count").as_deref(), Some("2 trains"));
    assert_eq!(text_by_test_id(&r, "nearest").as_deref(), Some("nearest"));
    let rows = test_ids(&r, "dep-");
    assert_eq!(
        rows.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(),
        ["dep-d1", "dep-d2"]
    );
    let main = r.kernel().find_by_test_id("main")[0];
    let main = r.kernel().node_by_key(main).unwrap();
    assert_eq!(
        main.style.flex_direction,
        exact_kernel::FlexDirection::Column
    );
    let count = r.kernel().find_by_test_id("count")[0];
    let count = r.kernel().node_by_key(count).unwrap();
    assert_eq!(
        (count.style.font_size, count.style.font_weight),
        (24.0, 700)
    );
    assert!(count.style.mask.has(StyleId::FontSize));
}

#[test]
fn a_press_selects_a_station_re_requests_the_board_and_keeps_rows_by_key() {
    let mut r = boot();
    let rows = test_ids(&r, "dep-");
    let d1_view = rows[0].1;
    let d2_view = rows[1].1;
    // Press the second row's button: selectStation(d.id) — the curried
    // argument is evaluated in the row's scope at dispatch time, so the
    // source is asked for departures("d2"). It refuses: the refusal is
    // typed, the reducer's write rolls back, and the kernel is untouched.
    let before = r.kernel().export(None).unwrap();
    let err = r.dispatch(d2_view, Event::Press).unwrap_err();
    assert!(
        matches!(err, RunnerError::Data { ref resource, error: DataError::Unavailable(ref id) } if resource == "board" && id == "d2")
    );
    assert_eq!(
        r.data().queries.last().unwrap().1,
        vec![Value::str("d2")],
        "the curried argument came from the row's scope"
    );
    assert_eq!(
        r.slot("stationId"),
        Some(&Value::NONE),
        "the write rolled back"
    );
    assert_eq!(r.kernel().export(None).unwrap(), before);
    assert!(!r.is_poisoned());
    r.act("selectStation", vec![Value::str("pa")]).unwrap();
    assert_eq!(
        r.data()
            .queries
            .iter()
            .rfind(|(s, _)| s == "departures")
            .unwrap()
            .1,
        vec![Value::str("pa")]
    );
    assert_eq!(text_by_test_id(&r, "count").as_deref(), Some("3 trains"));
    assert_eq!(text_by_test_id(&r, "selected").as_deref(), Some("at pa"));
    assert_eq!(
        text_by_test_id(&r, "nearest"),
        None,
        "the none arm was torn down"
    );
    let rows = test_ids(&r, "dep-");
    assert_eq!(
        rows.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(),
        ["dep-d2", "dep-d1", "dep-d9"]
    );
    assert_eq!(rows[0].1, d2_view, "d2 kept its view across the reorder");
    assert_eq!(rows[1].1, d1_view, "d1 kept its view across the reorder");
    assert_ne!(rows[2].1, d1_view);
}

#[test]
fn a_text_change_flips_the_when_region_and_back() {
    let mut r = boot();
    let search = r.kernel().find_by_test_id("search")[0];
    let search_view = r.kernel().node_by_key(search).unwrap().id;
    r.dispatch(search_view, Event::Change("pal".into()))
        .unwrap();
    assert_eq!(r.slot("query"), Some(&Value::str("pal")));
    assert_eq!(
        text_by_test_id(&r, "searching").as_deref(),
        Some("searching")
    );
    assert!(test_ids(&r, "dep-").is_empty(), "the rows were torn down");
    let input = r.kernel().node_by_key(search).unwrap();
    assert_eq!(input.props.str(PropId::Value), Some("pal"));
    r.dispatch(search_view, Event::Change(String::new()))
        .unwrap();
    assert_eq!(text_by_test_id(&r, "searching"), None);
    assert_eq!(test_ids(&r, "dep-").len(), 2, "fresh rows");
    // Unchanged rows are not re-sent: a no-op change produces no ops.
    let before = r.kernel().epoch();
    r.dispatch(search_view, Event::Change(String::new()))
        .unwrap();
    assert_eq!(
        r.kernel().epoch(),
        before,
        "an update that changes nothing bumps no epoch"
    );
}

#[test]
fn the_timer_fires_under_the_seekable_clock() {
    let mut r = boot();
    let d1 = test_ids(&r, "dep-")[0].1;
    let countdown = |r: &Runner<Schedule>| {
        let n = r.kernel().node(d1).unwrap();
        let child = n.children()[0];
        r.kernel()
            .node(child)
            .unwrap()
            .props
            .str(PropId::Text)
            .unwrap()
            .to_string()
    };
    assert_eq!(countdown(&r), "10", "600000 ms away at t=0");
    let receipts = r.advance(2_500.0).unwrap();
    assert_eq!(receipts.len(), 2, "two ticks fired, at 1000 and 2000");
    assert_eq!(
        r.slot("nowMs"),
        Some(&Value::Number(2_000.0)),
        "each tick reads the clock at its own time"
    );
    assert_eq!(r.now_ms(), 2_500.0);
    let receipts = r.advance(300_000.0).unwrap();
    assert_eq!(receipts.len(), 298);
    assert_eq!(countdown(&r), "5");
    assert_eq!(
        r.advance(100.0).unwrap().len(),
        0,
        "the clock does not run backwards"
    );
}

#[test]
fn commands_exit_the_side_and_refusals_leave_the_kernel_untouched() {
    let mut r = boot();
    r.act("setDark", vec![Value::str("dark")]).unwrap();
    let commands = r.take_commands();
    assert_eq!(commands.len(), 1);
    assert_eq!(
        (commands[0].name.as_str(), &commands[0].args[..]),
        ("setScheme", &[Value::str("dark")][..])
    );
    assert!(r.take_commands().is_empty());

    let before = r.kernel().export(None).unwrap();
    let err = r.act("rogue", vec![]).unwrap_err();
    assert!(matches!(
        err,
        RunnerError::Trap(Trap::WriteNotDeclared { slot: 2, .. })
    ));
    assert_eq!(r.slot("nowMs"), Some(&Value::Number(0.0)));
    assert_eq!(r.kernel().export(None).unwrap(), before);

    let err = r.act("selectStation", vec![]).unwrap_err();
    assert!(matches!(
        err,
        RunnerError::Arity {
            expected: 1,
            actual: 0,
            ..
        }
    ));

    r.data().wrong_shape = true;
    let err = r.act("selectStation", vec![Value::str("pa")]).unwrap_err();
    assert!(matches!(err, RunnerError::Shape { ref resource } if resource == "board"));
    assert_eq!(
        r.kernel().export(None).unwrap(),
        before,
        "a shape refusal applies nothing"
    );
    assert_eq!(
        r.slot("stationId"),
        Some(&Value::NONE),
        "and the reducer's write rolled back"
    );
    assert!(!r.is_poisoned());

    let (mut plan, _) = now_screen();
    plan.kernel_schema_digest ^= 1;
    assert!(matches!(
        Runner::boot(plan, Schedule::default(), Kernel::with_monospace()),
        Err(RunnerError::KernelSchemaMismatch { .. })
    ));
}

/// A source whose second query fails: settlement must publish nothing.
struct Flaky {
    calls: u32,
}

impl DataSource for Flaky {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.calls += 1;
        match source {
            "stations" => Ok(Value::list(vec![station("mv", "Mountain View")])),
            "departures" => {
                // Boot is the first query (`stations` is compiled data); the
                // re-request after `selectStation` is the second, and fails.
                if self.calls > 1 {
                    return Err(DataError::Unavailable("flaky".into()));
                }
                let id = args.first().and_then(Value::as_str).unwrap_or("");
                Ok(Value::list(vec![departure(&format!("{id}-1"), 1.0, 1.0)]))
            }
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}

#[test]
fn a_settlement_refusal_publishes_no_partial_resource_state() {
    let (plan, _) = now_screen();
    let mut r = Runner::boot(plan, Flaky { calls: 0 }, Kernel::with_monospace()).unwrap();
    let before = r.resource("board").cloned();
    // selectStation("pa") re-requests `board`; the source now refuses.
    let err = r.act("selectStation", vec![Value::str("pa")]).unwrap_err();
    assert!(matches!(err, RunnerError::Data { .. }));
    assert_eq!(
        r.resource("board").cloned(),
        before,
        "the cache is untouched"
    );
    assert_eq!(r.slot("stationId"), Some(&Value::NONE));
    assert!(!r.is_poisoned());
}

#[test]
fn values_conform_to_declared_types_at_every_boundary() {
    let mut r = boot();
    // A parameter of the wrong type is refused before the body runs.
    let err = r
        .act("selectStation", vec![Value::Number(7.0)])
        .unwrap_err();
    assert!(matches!(err, RunnerError::ArgumentType { ref param, .. } if param == "id"));
    assert_eq!(r.slot("stationId"), Some(&Value::NONE));
    // A non-finite clock is refused.
    assert!(matches!(
        r.advance(f64::INFINITY),
        Err(RunnerError::NonFiniteClock)
    ));
    // A plan whose slot initializer does not match its declared type is refused at boot.
    let (mut plan, _) = now_screen();
    let bad = exact_plan::builder::PlanBuilder::from_plan(plan.clone())
        .plan()
        .clone();
    let _ = bad;
    let mut b = exact_plan::builder::PlanBuilder::from_plan(plan.clone());
    let text = b.constant(&Value::str("not a number"));
    b.set_slot_init(exact_plan::SlotsId(2), text); // nowMs: number
    plan = b.finish().unwrap();
    assert!(matches!(
        Runner::boot(plan, Schedule::default(), Kernel::with_monospace()),
        Err(RunnerError::SlotType { ref slot }) if slot == "nowMs"
    ));
}

#[test]
fn a_region_at_the_plan_root_is_refused() {
    let (mut plan, _) = now_screen();
    // Detach the `when` region from its parent: it becomes a root site.
    plan.regions[0].parent = None;
    // Remove the root node so the site count stays one.
    let root_children: Vec<usize> = (0..plan.nodes.len())
        .filter(|i| plan.nodes[*i].parent == Some(exact_plan::NodesId(0)))
        .collect();
    for i in root_children {
        plan.nodes[i].parent = None;
    }
    plan.nodes.remove(0);
    plan.validate().unwrap_or_else(|e| panic!("{e:?}"));
    let err = Runner::boot(plan, Schedule::default(), Kernel::with_monospace())
        .err()
        .unwrap();
    assert!(matches!(
        err,
        RunnerError::RootRegion | RunnerError::NotOneRoot(_)
    ));
}

/// Duplicate keys poison the runner; commands must not leak out.
struct Duplicates;

impl DataSource for Duplicates {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        match source {
            "stations" => Ok(Value::list(vec![])),
            "departures" => Ok(Value::list(vec![
                departure("same", 1.0, 1.0),
                departure("same", 2.0, 2.0),
            ])),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}

#[test]
fn keys_follow_one_rule_and_a_poisoned_runner_leaks_no_commands() {
    let (plan, _) = now_screen();
    let err = Runner::boot(plan.clone(), Duplicates, Kernel::with_monospace())
        .err()
        .unwrap();
    assert!(matches!(
        err,
        RunnerError::Instance(exact_runner::instance::InstanceError::DuplicateKey { .. })
    ));
    // A runner poisoned after commit leaks no commands, even ones queued earlier.
    struct LateDuplicates {
        calls: u32,
    }
    impl DataSource for LateDuplicates {
        fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
            self.calls += 1;
            match source {
                "stations" => Ok(Value::list(vec![])),
                "departures" if self.calls == 1 => Ok(Value::list(vec![departure("d1", 1.0, 1.0)])),
                "departures" => {
                    let _ = args;
                    Ok(Value::list(vec![
                        departure("same", 1.0, 1.0),
                        departure("same", 2.0, 2.0),
                    ]))
                }
                other => Err(DataError::UnknownSource(other.into())),
            }
        }
    }
    let (plan, _) = now_screen();
    let mut r = Runner::boot(plan, LateDuplicates { calls: 0 }, Kernel::with_monospace()).unwrap();
    r.act("setDark", vec![Value::str("dark")]).unwrap();
    let err = r
        .act("selectStation", vec![Value::str("pa")])
        .err()
        .unwrap();
    assert!(matches!(
        err,
        RunnerError::Instance(exact_runner::instance::InstanceError::DuplicateKey { .. })
    ));
    assert!(r.is_poisoned());
    assert!(r.take_commands().is_empty(), "no command survives a poison");
    assert!(matches!(
        r.act("setDark", vec![Value::str("x")]),
        Err(RunnerError::Poisoned)
    ));
}

#[test]
fn the_journal_is_a_ring_and_logs_reports_where_its_window_starts() {
    use exact_runner::JOURNAL_RING;
    let mut r = boot();
    // The boot line, then more lines than the ring holds: the oldest go.
    for i in 0..JOURNAL_RING + 10 {
        r.log(format!("line {i}"));
    }
    assert_eq!(
        r.journal_start(),
        11,
        "the boot line and ten more were dropped"
    );
    assert_eq!(r.journal().count(), JOURNAL_RING);
    assert_eq!(r.journal().next(), Some("t=0 line 10"));
    let logs = exact_runner::agent::logs(&r, 0);
    let total = JOURNAL_RING + 11;
    assert!(
        logs.starts_with(&format!(
            "{{\"next\":{total},\"from\":11,\"lines\":[\"t=0 line 10\""
        )),
        "{}",
        &logs[..80]
    );
    let tail = exact_runner::agent::logs(&r, total - 2);
    assert_eq!(
        tail,
        format!(
            "{{\"next\":{total},\"from\":{},\"lines\":[\"t=0 line {}\",\"t=0 line {}\"]}}",
            total - 2,
            JOURNAL_RING + 8,
            JOURNAL_RING + 9
        )
    );
}

#[test]
fn an_advance_stops_at_a_refusing_timer_with_the_refusal_and_the_clock() {
    // A poisoned runner refuses every action: the first timer of a seek
    // refuses, the advance stops at that timer's due time with no commits,
    // and the refusal rides along — the kernel is exactly as it was.
    struct Dupes {
        calls: u32,
    }
    impl DataSource for Dupes {
        fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
            self.calls += 1;
            match source {
                "stations" => Ok(Value::list(vec![])),
                "departures" if self.calls == 1 => Ok(Value::list(vec![departure("d1", 1.0, 1.0)])),
                "departures" => Ok(Value::list(vec![
                    departure("same", 1.0, 1.0),
                    departure("same", 2.0, 2.0),
                ])),
                other => Err(DataError::UnknownSource(other.into())),
            }
        }
    }
    let (plan, _) = now_screen();
    let mut r = Runner::boot(plan, Dupes { calls: 0 }, Kernel::with_monospace()).unwrap();
    // One timer fires cleanly first: its commit is kept and timed.
    let ok = r.advance_timed(1_500.0);
    assert_eq!(ok.receipts.len(), 1);
    assert_eq!(ok.receipts[0].at_ms, 1_000.0);
    assert_eq!(ok.now_ms, 1_500.0);
    assert!(ok.error.is_none());
    let _ = r.act("selectStation", vec![Value::str("pa")]);
    assert!(r.is_poisoned());
    let a = r.advance_timed(5_000.0);
    assert!(a.receipts.is_empty());
    assert_eq!(
        a.now_ms, 2_000.0,
        "the clock stays at the refusing timer's due time"
    );
    assert!(matches!(a.error, Some(RunnerError::Poisoned)));
    assert_eq!(r.now_ms(), 2_000.0);
    let logs = exact_runner::agent::logs(&r, 0);
    assert!(logs.contains("timer 0 (tick) refused: Poisoned"), "{logs}");
}
