//! @ref LLP 1038 D3–D5, D7, D11 — the boundary, not another router corpus.
#[path = "support/router.rs"]
mod fixture;
use exact_kernel::Kernel;
use exact_plan::{asm::Asm, Plan, Stdlib, Value};
use exact_route::{Route, Router, Table};
use exact_runner::{DataError, DataSource, Runner, RunnerError};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
struct Data(Rc<RefCell<Vec<Vec<Value>>>>);
impl DataSource for Data {
    fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
        self.0.borrow_mut().push(args.to_vec());
        Ok(args.first().cloned().unwrap_or(Value::str("asked")))
    }
}
fn table() -> Table {
    Table {
        routes: [
            ("home", "/", None, true),
            ("thread", "/t/:thread", Some(0), false),
            ("details", "/t/:thread/details", Some(1), false),
            ("search", "/search", None, true),
            ("person", "/p/:person", Some(3), false),
        ]
        .into_iter()
        .map(|(n, p, parent, tab)| Route {
            name: n.into(),
            pattern: p.into(),
            parent,
            tab,
            notfound: false,
        })
        .collect(),
    }
}
fn boot(plan: Plan, launch: &str) -> Runner<Data> {
    Runner::boot(
        plan,
        Data::default(),
        Kernel::with_monospace(),
        Default::default(),
        launch,
    )
    .unwrap()
}
fn nav(r: &Runner<Data>) -> Router {
    r.carry().router.unwrap().1
}
fn act(r: &mut Runner<Data>, verb: &str, arg: &str) -> Option<exact_runner::RouterChange> {
    r.take_router_change();
    r.act(
        verb,
        if verb == "back" {
            vec![]
        } else {
            vec![Value::str(arg)]
        },
    )
    .unwrap();
    r.take_router_change()
}

#[test]
fn shapes_round_trip_launch_precedes_initializers_and_state_is_named_json() {
    let table = table();
    let mut r = boot(fixture::plan(&table), "/t/a%2Fb/details?q=hello+world");
    let expected = Router::launch(&table, "/t/a%2Fb/details?q=hello+world").0;
    assert_eq!(nav(&r), expected);
    assert_eq!(
        r.slot("initial_url"),
        Some(&Value::str("/t/a%2Fb/details?q=hello+world"))
    );
    let slot = r.slot("nav").unwrap().clone();
    assert!(slot.conforms(r.plan(), r.plan().slot(r.plan().router.unwrap()).ty));
    r.take_router_change();
    r.act("set", vec![slot.clone()]).unwrap();
    assert!(r.take_router_change().is_none());
    assert_eq!(r.slot("nav"), Some(&slot));
    assert_eq!(nav(&r), expected);
    assert_eq!(r.derive("depth"), Some(&Value::Number(3.0)));
    assert!(matches!(r.derive("stack"), Some(Value::List(v)) if v.len() == 3));
    assert!(matches!(r.derive("top"), Some(Value::Record(v)) if v.len() == 5));
    let state = exact_runner::agent::state(&r);
    assert!(state.contains(r#""nav":{"tab":"home","tabs":[{"name":"home","stack":[{"id":0,"name":"home","url":"/","tab":"home","params":{"thread":"","person":""}}"#), "{state}");
    assert!(state.contains(r#""params":{"thread":"a/b","person":""}"#));
    assert!(act(&mut r, "readParams", "thread").is_none());
    assert_eq!(
        r.slot("read_params"),
        Some(&Value::list(vec![Value::str("a/b"), Value::str("a/b")]))
    );
    act(&mut r, "readSearch", "q");
    assert_eq!(r.slot("read_search"), Some(&Value::str("hello world")));
    act(&mut r, "readSearch", "absent");
    assert_eq!(r.slot("read_search"), Some(&Value::str("")));
    act(&mut r, "encode", "a/b ?é");
    assert_eq!(r.slot("encoded"), Some(&Value::str("a%2Fb%20%3F%C3%A9")));
}

#[test]
fn launch_fallback_is_journaled_and_notfound_preserves_the_location() {
    let mut table = table();
    let mut r = boot(fixture::plan(&table), "/absent");
    assert_eq!(nav(&r), Router::launch(&table, "/").0);
    assert_eq!(r.take_router_change().unwrap().url, "/");
    assert_eq!(
        r.journal()
            .filter(|l| l.contains("router launch refused"))
            .count(),
        1
    );
    table.routes.push(Route {
        name: "notfound".into(),
        pattern: "".into(),
        parent: None,
        tab: false,
        notfound: true,
    });
    let mut r = boot(fixture::plan(&table), "/absent?q=1");
    assert_eq!(r.take_router_change().unwrap().url, "/absent?q=1");
    assert!(!r.journal().any(|l| l.contains("refused")));
    let mut table = self::table();
    table.routes[0].pattern = "/home".into();
    assert!(matches!(
        Runner::boot(
            fixture::plan(&table),
            Data::default(),
            Kernel::with_monospace(),
            Default::default(),
            "/absent"
        ),
        Err(RunnerError::Router(_))
    ));
}

#[test]
fn drained_changes_describe_commits_across_all_tabs_and_refusals_do_not_change_slots() {
    let mut r = boot(fixture::plan(&table()), "/");
    let boot = r.take_router_change().unwrap();
    assert_eq!(boot.top, 0);
    assert!(boot.removed.is_empty());
    assert!(act(&mut r, "back", "").is_none());
    let pushed = act(&mut r, "push", "/t/1").unwrap();
    assert_eq!(pushed.url, "/t/1");
    assert!(pushed.removed.is_empty());
    let replaced = act(&mut r, "replace", "/t/2").unwrap();
    assert_eq!(replaced.top, pushed.top);
    assert_eq!(replaced.url, "/t/2");
    let selected = act(&mut r, "select", "search").unwrap();
    assert!(selected.removed.is_empty());
    act(&mut r, "push", "/p/3");
    let searched = nav(&r).tabs[1].stack.last().unwrap().id;
    let opened = act(&mut r, "open", "/t/4/details").unwrap();
    assert_eq!(opened.removed, vec![pushed.top]);
    assert!(nav(&r).tabs[1].stack.iter().any(|e| e.id == searched));
    let before = nav(&r);
    let gone = act(&mut r, "go", "/").unwrap();
    assert_eq!(
        gone.removed,
        before.tabs[0].stack[1..]
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(gone.top, 0);
    let before = r.slot("nav").cloned();
    for _ in 0..2 {
        assert!(act(&mut r, "twice", "/absent").is_none());
    }
    assert_eq!(r.slot("nav"), before.as_ref());
    assert_eq!(
        r.journal()
            .filter(|l| l.contains("router push refused"))
            .count(),
        2
    );
    assert!(act(&mut r, "select", "absent").is_none());
    assert!(act(&mut r, "replace", "/t/5").is_none());
    assert_eq!(r.slot("nav"), before.as_ref());
}

#[test]
fn carry_checks_every_retained_entry_and_restarts_at_the_old_top() {
    let t = table();
    let mut r = boot(fixture::plan(&t), "/t/1/details");
    act(&mut r, "select", "search");
    act(&mut r, "push", "/p/2");
    let carried = r.carry();
    let reload = |t: &Table| {
        Runner::boot_carrying(
            fixture::plan(t),
            Data::default(),
            Kernel::with_monospace(),
            &carried,
            Default::default(),
            "/",
        )
        .unwrap()
    };
    assert_eq!(nav(&reload(&t)), nav(&r));
    let mut renamed = t.clone();
    renamed.routes[1].name = "conversation".into();
    let again = reload(&renamed);
    assert_eq!(nav(&again), Router::launch(&renamed, "/p/2").0);
    assert_eq!(again.derive("url"), Some(&Value::str("/p/2")));
    let mut missing_top = t.clone();
    missing_top.routes[4].pattern = "/people/:person".into();
    assert_eq!(reload(&missing_top).derive("url"), Some(&Value::str("/")));
    let mut renamed_param = t.clone();
    renamed_param.routes[4].pattern = "/p/:member".into();
    let again = reload(&renamed_param);
    assert_eq!(nav(&again).next, nav(&r).next);
    assert_eq!(
        exact_route::top(&nav(&again)).unwrap().params["member"],
        "2"
    );
}

#[test]
fn compiled_resources_are_keyed_by_the_evaluated_boot_arguments() {
    let mut b = fixture::builder(&table());
    let nav = b.plan().router.unwrap();
    let string = b.plan().slots[0].ty;
    let mut arg = Asm::new();
    arg.load_slot(nav).call(Stdlib::Top).field(2);
    let arg = b.code(arg);
    let cached = b.resource(
        "page",
        "page",
        &[arg],
        string,
        Some(&Value::str("compiled")),
    );
    b.set_resource_initial_args(cached, &[Value::str("/")]);
    b.resource(
        "static",
        "static",
        &[],
        string,
        Some(&Value::str("static cache")),
    );
    let plan = b.finish().unwrap();
    for (launch, value, queries) in [("/", "compiled", 0), ("/t/42", "/t/42", 1)] {
        let data = Data::default();
        let calls = data.0.clone();
        let r = Runner::boot(
            plan.clone(),
            data,
            Kernel::with_monospace(),
            Default::default(),
            launch,
        )
        .unwrap();
        assert_eq!(r.resource("page"), Some(&Value::str(value)));
        assert_eq!(calls.borrow().len(), queries);
        assert_eq!(
            r.resource_args("page"),
            Some([Value::str(launch)].as_slice())
        );
        assert_eq!(r.resource("static"), Some(&Value::str("static cache")));
    }
}

#[test]
fn deferred_deep_launch_uses_a_placeholder_then_asks_the_current_arguments() {
    #[derive(Default)]
    struct Deferred {
        ready: bool,
        asked: Vec<Vec<Value>>,
    }
    impl DataSource for Deferred {
        fn ready(&self) -> bool {
            self.ready
        }
        fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
            assert!(self.ready, "no app source may run before first pixel");
            self.asked.push(args.to_vec());
            Ok(args[0].clone())
        }
    }
    let mut b = fixture::builder(&table());
    let mut arg = Asm::new();
    arg.load_slot(b.plan().router.unwrap())
        .call(Stdlib::Top)
        .field(2);
    let arg = b.code(arg);
    let string = b.plan().slots[0].ty;
    let page = b.resource(
        "page",
        "page",
        &[arg],
        string,
        Some(&Value::str("placeholder")),
    );
    b.set_resource_initial_args(page, &[Value::str("/")]);
    let plan = b.finish().unwrap();
    // The rule applies to ordinary resources as well as store readers.
    for reader in [false, true] {
        let mut plan = plan.clone();
        plan.resources[0].reader = reader;
        let mut r = Runner::boot(
            plan,
            Deferred::default(),
            Kernel::with_monospace(),
            Default::default(),
            "/t/42/details",
        )
        .unwrap();
        assert_eq!(r.resource("page"), Some(&Value::str("placeholder")));
        assert_eq!(r.derive("depth"), Some(&Value::Number(3.0)));
        assert_eq!(
            r.resource_args("page"),
            Some([Value::str("/t/42/details")].as_slice())
        );
        assert!(r.data_ref().asked.is_empty());
        assert!(r.data_ready().unwrap().is_none());
        r.data().ready = true;
        assert!(r.data_ready().unwrap().is_some());
        assert_eq!(r.data_ref().asked, [vec![Value::str("/t/42/details")]]);
        assert_eq!(r.resource("page"), Some(&Value::str("/t/42/details")));
        assert!(r.data_ready().unwrap().is_none());
        assert_eq!(r.data_ref().asked.len(), 1);
    }
}

#[test]
fn malformed_shapes_and_fractional_ids_are_typed_refusals() {
    let mut plan = fixture::plan(&table());
    let router = plan.slot(plan.router.unwrap()).ty;
    plan.types[router.0 as usize].fields.len = 2;
    assert!(matches!(
        Runner::boot(
            plan,
            Data::default(),
            Kernel::with_monospace(),
            Default::default(),
            "/"
        ),
        Err(RunnerError::Router(_))
    ));
    let mut r = boot(fixture::plan(&table()), "/");
    let before = r.slot("nav").unwrap().clone();
    let Value::Record(v) = &before else { panic!() };
    let mut fields = v.to_vec();
    fields[2] = Value::Number(0.5);
    assert!(matches!(
        r.act("set", vec![Value::record(fields)]),
        Err(RunnerError::Router(_))
    ));
    assert_eq!(r.slot("nav"), Some(&before));
    assert!(!r.is_poisoned());
    assert!(act(&mut r, "push", "/t/1").is_some());
}

#[test]
fn a_resource_refusal_rolls_back_navigation_and_the_next_change_uses_the_committed_value() {
    struct Fail;
    impl DataSource for Fail {
        fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
            if args == [Value::str("/t/fail")] {
                Err(DataError::Unavailable("refused".into()))
            } else {
                Ok(args[0].clone())
            }
        }
    }
    let mut b = fixture::builder(&table());
    let mut args = Asm::new();
    args.load_slot(b.plan().router.unwrap())
        .call(Stdlib::Top)
        .field(2);
    let args = b.code(args);
    let string = b.plan().slots[0].ty;
    b.resource("page", "page", &[args], string, None);
    let mut r = Runner::boot(
        b.finish().unwrap(),
        Fail,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.take_router_change();
    let before = r.kernel().epoch();
    let slot = r.slot("nav").cloned();
    assert!(matches!(
        r.act("push", vec![Value::str("/t/fail")]),
        Err(RunnerError::Data { .. })
    ));
    assert_eq!(r.kernel().epoch(), before);
    assert!(r.take_router_change().is_none());
    assert_eq!(r.slot("nav"), slot.as_ref());
    let receipt: exact_kernel::CommitReceipt = r.act("push", vec![Value::str("/t/ok")]).unwrap();
    assert_eq!(
        receipt.epoch, before,
        "navigation alone emits no kernel ops"
    );
    assert_eq!(receipt.batch, 2);
    let change = r.take_router_change().unwrap();
    assert_eq!(change.top, 2);
    assert!(change.removed.is_empty());
}

#[test]
fn a_deep_launch_answering_later_cannot_show_another_locations_compiled_value() {
    struct Later(Rc<RefCell<usize>>);
    impl DataSource for Later {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            unreachable!()
        }
        fn answer(
            &mut self,
            _: &mut exact_runner::Store,
            _: &str,
            _: &[Value],
        ) -> Result<exact_runner::Answer, DataError> {
            *self.0.borrow_mut() += 1;
            Ok(exact_runner::Answer::Later(exact_runner::Request::get(
                "https://example.test/data",
            )))
        }
    }
    let mut b = fixture::builder(&table());
    let mut arg = Asm::new();
    arg.load_slot(b.plan().router.unwrap())
        .call(Stdlib::Top)
        .field(2);
    let arg = b.code(arg);
    let string = b.plan().slots[0].ty;
    let resource = b.resource(
        "page",
        "page",
        &[arg],
        string,
        Some(&Value::str("home only")),
    );
    b.set_resource_initial_args(resource, &[Value::str("/")]);
    let plan = b.finish().unwrap();
    let calls = Rc::new(RefCell::new(0));
    // @ref LLP 1054.000.002 D1 — the deep launch shows the type's zero,
    // pending, never another location's compiled value.
    let mut deep = Runner::boot(
        plan.clone(),
        Later(calls.clone()),
        Kernel::with_monospace(),
        Default::default(),
        "/t/42",
    )
    .unwrap();
    assert_eq!(deep.resource("page"), Some(&Value::str("")));
    assert_eq!(deep.take_requests().len(), 1);
    assert_eq!(*calls.borrow(), 1);
    let mut r = Runner::boot(
        plan,
        Later(calls.clone()),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(*calls.borrow(), 1);
    r.act("push", vec![Value::str("/t/42")]).unwrap();
    assert_eq!(*calls.borrow(), 2);
    assert_eq!(r.take_requests().len(), 1);
    assert_eq!(
        r.resource("page"),
        Some(&Value::str("home only")),
        "an already settled value may remain while pending"
    );
}

#[test]
fn router_changes_drain_once_and_survive_unchanged_or_refused_commits() {
    let mut r = boot(fixture::plan(&table()), "/");
    // Boot's change remains pending even if the first action changes nothing.
    let receipt: exact_kernel::CommitReceipt = r.act("back", vec![]).unwrap();
    assert_eq!(receipt.batch, 2);
    assert!(
        receipt.created.is_empty() && receipt.touched.is_empty() && receipt.destroyed.is_empty()
    );
    assert_eq!(r.take_router_change().unwrap().url, "/");
    assert!(r.take_router_change().is_none());
    r.act("push", vec![Value::str("/t/1")]).unwrap();
    let epoch = r.kernel().epoch();
    assert!(r.act("unknown", vec![]).is_err());
    assert_eq!(r.kernel().epoch(), epoch);
    assert_eq!(r.take_router_change().unwrap().url, "/t/1");
    assert!(r.take_router_change().is_none());
    r.act("go", vec![Value::str("/t/1")]).unwrap();
    assert!(r.take_router_change().is_none());
}

#[test]
fn several_commits_before_a_take_keep_the_latest_top_and_all_removals() {
    let mut b = fixture::builder(&table());
    let back = b
        .plan()
        .actions
        .iter()
        .position(|a| b.plan().str(a.name) == "back")
        .unwrap();
    b.timer(1000, exact_plan::ActionsId(back as u32), false);
    let mut r = boot(b.finish().unwrap(), "/t/1/details");
    let before = nav(&r);
    r.take_router_change();
    // One seek makes two removals and then an unchanged commit before a drain.
    let receipts: Vec<exact_kernel::CommitReceipt> = r.advance(3000.0).unwrap();
    assert_eq!(receipts.len(), 3);
    let change = r.take_router_change().unwrap();
    assert_eq!(change.top, before.tabs[0].stack[0].id);
    assert_eq!(change.url, "/");
    assert_eq!(
        change.removed,
        before.tabs[0].stack[1..]
            .iter()
            .rev()
            .map(|e| e.id)
            .collect::<Vec<_>>()
    );
    assert!(r.take_router_change().is_none());
}

#[test]
fn assigned_router_entries_must_be_canonical_and_match_name_and_params() {
    let mut r = boot(fixture::plan(&table()), "/t/1");
    r.take_router_change();
    let before = r.slot("nav").unwrap().clone();
    let epoch = r.kernel().epoch();
    // Top, root, and an inactive tab all cross the same slot boundary.
    for (tab, entry, field, replacement) in [
        (0, 1, 1, Value::str("home")),
        (0, 1, 2, Value::str("/foo")),
        (0, 1, 2, Value::str("/t/./1")),
        (
            0,
            1,
            4,
            Value::record(vec![Value::str("2"), Value::str("")]),
        ),
        (0, 0, 2, Value::str("/foo")),
        (1, 0, 2, Value::str("/foo")),
    ] {
        let mut path = vec![1, tab, 1, entry, field];
        let mut value = replaced(&before, &path, replacement);
        // The review's exact case: top /foo named home.
        if field == 2 && tab == 0 && entry == 1 && at(&value, &path) == &Value::str("/foo") {
            path[4] = 1;
            value = replaced(&value, &path, Value::str("home"));
        }
        assert!(value.conforms(r.plan(), r.plan().slot(r.plan().router.unwrap()).ty));
        assert!(matches!(
            r.act("set", vec![value]),
            Err(RunnerError::Router(_))
        ));
        assert_eq!(r.slot("nav"), Some(&before));
        assert_eq!(r.kernel().epoch(), epoch);
        assert!(r.take_router_change().is_none());
        assert!(r.journal().last().unwrap().contains("refused: Router"));
        assert!(!r.is_poisoned());
    }
    assert!(act(&mut r, "push", "/t/2").is_some());
}

#[test]
fn carry_rebuilds_when_tab_names_or_order_change() {
    let t = table();
    let r = boot(fixture::plan(&t), "/t/1/details");
    let carried = r.carry();
    let mut added = t.clone();
    added.routes.push(Route {
        name: "settings".into(),
        pattern: "/settings".into(),
        parent: None,
        tab: true,
        notfound: false,
    });
    let mut reordered = t.clone();
    let new_index = [2, 3, 4, 0, 1];
    reordered.routes = [3, 4, 0, 1, 2]
        .into_iter()
        .map(|i| {
            let mut route = t.routes[i].clone();
            route.parent = route.parent.map(|p| new_index[p]);
            route
        })
        .collect();
    let mut removed = t.clone();
    removed.routes[3].tab = false;
    for changed in [&added, &reordered, &removed] {
        let mut next = Runner::boot_carrying(
            fixture::plan(changed),
            Data::default(),
            Kernel::with_monospace(),
            &carried,
            Default::default(),
            "/",
        )
        .unwrap();
        assert_eq!(nav(&next), Router::launch(changed, "/t/1/details").0);
        if changed == &added {
            assert_eq!(
                act(&mut next, "select", "settings").unwrap().url,
                "/settings"
            );
            act(&mut next, "select", "home");
            assert_eq!(
                act(&mut next, "open", "/settings").unwrap().url,
                "/settings"
            );
        }
    }
}

/// Answers `later` with a request; everything else as [`Data`] does.
struct Replies(Data);
impl DataSource for Replies {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.0.query(source, args)
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        source: &str,
        args: &[Value],
    ) -> Result<exact_runner::Answer, DataError> {
        Ok(if source == "later" {
            exact_runner::Answer::Later(exact_runner::Request::get("https://fixture.invalid/"))
        } else {
            exact_runner::Answer::Now(self.query(source, args)?)
        })
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
        _: exact_runner::Outcome,
    ) -> Result<exact_runner::Answer, DataError> {
        Ok(exact_runner::Answer::Now(Value::str("replied")))
    }
}

#[test]
fn a_refused_navigation_does_not_carry_its_refresh_into_the_next_commit() {
    let mut b = fixture::builder(&table());
    let string = b.primitive(exact_plan::TypeKind::String);
    let (index, nav) = b
        .plan()
        .slots
        .iter()
        .enumerate()
        .find(|(_, s)| b.plan().str(s.name) == "nav")
        .map(|(i, s)| (exact_plan::SlotsId(i as u32), s.ty))
        .unwrap();
    let rows = b.resource("rows", "rows", &[], string, None);
    let later = b.resource("later", "later", &[], string, Some(&Value::str("baked")));
    b.set_resource_initial_args(later, &[]);
    for (name, resource, set) in [
        ("refreshLater", later, false),
        ("refreshAndSet", rows, true),
    ] {
        let mut a = Asm::new();
        a.refresh(resource);
        if set {
            a.load_param(0).store_slot(index);
        }
        a.op(exact_plan::Opcode::Unit, &[]);
        let code = b.code(a);
        let (params, writes) = if set {
            (vec![("value", nav)], vec![index])
        } else {
            (vec![], vec![])
        };
        b.action(name, &params, &writes, code);
    }
    let mut r = Runner::boot(
        b.finish().unwrap(),
        Replies(Data::default()),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.act("refreshLater", vec![]).unwrap();
    let ticket = r.take_requests().remove(0).ticket;
    let asked = r.data().0 .0.borrow().len();
    // Shape-correct but not a router: `next` must exceed every visit id.
    let Some(Value::Record(fields)) = r.slot("nav").cloned() else {
        panic!("router record")
    };
    let mut forged = fields.to_vec();
    forged[2] = Value::Number(0.);
    assert!(matches!(
        r.act("refreshAndSet", vec![Value::record(forged)]),
        Err(RunnerError::Router(_))
    ));
    r.fulfill(ticket, exact_runner::Outcome::Storage(Vec::new()))
        .unwrap()
        .unwrap();
    assert_eq!(r.resource("later"), Some(&Value::str("replied")));
    assert_eq!(
        r.data().0 .0.borrow().len(),
        asked,
        "the refused refresh never reaches the source"
    );
}

/// `v` with the item at `path` (positions through records and lists)
/// replaced.
fn replaced(v: &Value, path: &[usize], new: Value) -> Value {
    let Some((&first, rest)) = path.split_first() else {
        return new;
    };
    let (Value::Record(items) | Value::List(items)) = v else {
        panic!("a record or a list")
    };
    let mut items = items.to_vec();
    items[first] = replaced(&items[first], rest, new);
    match v {
        Value::Record(_) => Value::record(items),
        _ => Value::list(items),
    }
}

fn at<'a>(v: &'a Value, path: &[usize]) -> &'a Value {
    path.iter().fold(v, |v, &i| match v {
        Value::Record(items) | Value::List(items) => &items[i],
        _ => panic!("a record or a list"),
    })
}

/// LLP 1115 D5: a host's own Back for a route with no Back control goes to
/// the location of the visit beneath it, on whichever stack holds it.
#[test]
fn the_location_beneath_a_visit_is_its_stacks_previous_entry() {
    let mut r = boot(fixture::plan(&table()), "/t/1/details");
    let stack = nav(&r).tabs[0].stack.clone();
    assert_eq!(stack.len(), 3);
    assert_eq!(r.location_beneath(stack[2].id).as_deref(), Some("/t/1"));
    assert_eq!(r.location_beneath(stack[1].id).as_deref(), Some("/"));
    assert_eq!(r.location_beneath(stack[0].id), None);
    assert_eq!(r.location_beneath(u64::MAX), None);
    // A retained tab's visit answers too; going there pops as `back` would.
    act(&mut r, "select", "search");
    assert_eq!(r.location_beneath(stack[2].id).as_deref(), Some("/t/1"));
    act(&mut r, "select", "home");
    let to = r.location_beneath(stack[2].id).unwrap();
    let change = act(&mut r, "go", &to).unwrap();
    assert_eq!((change.top, change.removed), (stack[1].id, vec![stack[2].id]));
}
