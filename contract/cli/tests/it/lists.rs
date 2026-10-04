//! LLP 1017.003: `map`, `filter` and `join` in Contract expressions, proven
//! on the runner — what they compute, what they refuse, their bound, and
//! that a row's derived string is formatted again only when what it reads
//! changes.

use exact_kernel::{Kernel, PropValue};
use exact_plan::{Items, Plan, Value};
use exact_runner::instance::InstanceError;
use exact_runner::{DataError, DataSource, Runner, RunnerError, Trap};
use std::path::Path;

fn corpus(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn numbers(values: &[f64]) -> Value {
    Value::list(values.iter().map(|v| Value::Number(*v)).collect())
}

/// Field order is `shape Coin` in `lists.contract`.
fn coin(id: &str, series: Value, flash: bool, tags: &[&str]) -> Value {
    let Value::List(items) = &series else {
        unreachable!("a series is a list")
    };
    let nums: Vec<f64> = items.iter().filter_map(Value::as_number).collect();
    let lo = nums.iter().copied().fold(f64::MAX, f64::min);
    let hi = nums.iter().copied().fold(f64::MIN, f64::max);
    Value::record(vec![
        Value::str(id),
        series,
        Value::Number(lo),
        Value::Number(hi),
        Value::Bool(flash),
        Value::list(tags.iter().map(|t| Value::str(t)).collect()),
    ])
}

/// Three coins. Tick 1 turns coin `b`'s flash on and shares its series;
/// tick 2 gives it a new series; tick 3 a bigger list of tags only.
struct Coins {
    ticks: u32,
    rows: Vec<Value>,
}

impl Default for Coins {
    fn default() -> Self {
        Coins {
            ticks: 0,
            rows: vec![
                coin("a", numbers(&[1.0, 2.0, 3.0]), false, &["x", ""]),
                coin("b", numbers(&[5.0, 5.0]), false, &[]),
                coin("c", numbers(&[3.0, 1.0, 2.0, 1.0]), false, &["", "y", "z"]),
            ],
        }
    }
}

impl DataSource for Coins {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "coins" => Ok(Value::list(self.rows.clone())),
            "tick" => {
                self.ticks += 1;
                let Value::Record(b) = &self.rows[1] else {
                    unreachable!()
                };
                let mut b = b.to_vec();
                match self.ticks {
                    1 => b[4] = Value::Bool(true),
                    2 => {
                        let series = numbers(&[5.0, 6.0, 4.0]);
                        self.rows[1] = coin("b", series, true, &[]);
                        return Ok(Value::list(self.rows.clone()));
                    }
                    _ => b[5] = Value::list(vec![Value::str("w")]),
                }
                self.rows[1] = Value::Record(Items::from(b));
                Ok(Value::list(self.rows.clone()))
            }
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

fn prop(r: &Runner<Coins>, id: &str, name: &str) -> String {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(id)[0])
        .unwrap()
        .props
        .iter()
        .find_map(|(p, v)| match v {
            PropValue::Str(s) if p.name() == name => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn boot(src: &str) -> Runner<Coins> {
    let plan = contract::compile(src).unwrap();
    let plan = contract::bake(plan, Coins::default()).unwrap();
    Runner::boot(
        plan,
        Coins::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

#[test]
fn a_view_maps_filters_and_joins_a_list_it_holds_once() {
    let r = boot(&corpus("lists.contract"));
    assert_eq!(prop(&r, "tagged", "text"), "2 tagged");
    assert_eq!(prop(&r, "ids", "text"), "1. a, 2. b, 3. c");
    assert_eq!(prop(&r, "tags-a", "text"), "x");
    assert_eq!(prop(&r, "tags-c", "text"), "y·z");
    assert_eq!(prop(&r, "tags-b", "text"), "");
    // x = i * 96 / (n - 1), y = 32 - (p - lo) / (hi - lo) * 32, as the web
    // prints them after rounding to hundredths; a flat series sits at 16.
    assert_eq!(prop(&r, "chart-a", "points"), "0,32 48,16 96,0");
    assert_eq!(prop(&r, "chart-b", "points"), "0,16 96,16");
    assert_eq!(prop(&r, "chart-c", "points"), "0,0 32,32 64,16 96,32");
}

#[test]
fn a_rows_points_are_formatted_again_only_when_what_they_read_changes() {
    for src in [
        corpus("lists.contract"),
        // The same row in a virtualized list: the collection's path.
        corpus("lists.contract").replace(
            "      each c in coins key=c.id\n",
            "      list virtualized=true estimated-item-height=32 height=400\n        each c in coins key=c.id\n",
        )
        .replace("\n        row testId", "\n          row testId")
        .replace("\n          text c.id", "\n            text c.id")
        .replace("\n          text (c.flash", "\n            text (c.flash")
        .replace("\n          text join(filter", "\n            text join(filter")
        .replace("\n          svg ", "\n            svg ")
        .replace("\n            polyline", "\n              polyline"),
    ] {
        assert!(!src.contains("virtualized") || src.contains("list virtualized=true estimated-item-height=32 height=400\n        each c in coins key=c.id\n          row"));
        let mut r = boot(&src);
        let before = prop(&r, "chart-b", "points");
        // Each tick gives a new list, so the one root binding that reads it
        // (`ids`) runs every time; `tagged` keeps an equivalent value.
        // Tick 1: only `b.flash` changes: its text runs, and not the chart,
        // which reads `series`, `lo` and `hi` (nor the row's `testId`s,
        // which read `id`, nor its tags).
        r.act("tick", vec![]).unwrap();
        assert_eq!(prop(&r, "flash-b", "text"), "*");
        assert_eq!(r.last_instance_work().bindings_evaluated, 2, "{src}");
        // Tick 2: `b.series`, `lo` and `hi` change: the chart runs, alone.
        r.act("tick", vec![]).unwrap();
        assert_ne!(prop(&r, "chart-b", "points"), before);
        assert_eq!(prop(&r, "chart-b", "points"), "0,16 48,0 96,32");
        assert_eq!(r.last_instance_work().bindings_evaluated, 2, "{src}");
        // Tick 3: only `b.tags`: its tags text and, now that `b` is
        // tagged, the count; not the chart.
        r.act("tick", vec![]).unwrap();
        assert_eq!(prop(&r, "tags-b", "text"), "w");
        assert_eq!(prop(&r, "tagged", "text"), "3 tagged");
        assert_eq!(r.last_instance_work().bindings_evaluated, 3, "{src}");
        // And evaluating everything agrees with what the skips left.
        r.set_full_evaluation(true);
        let shown = prop(&r, "chart-b", "points");
        r.act("tick", vec![]).unwrap();
        assert_eq!(prop(&r, "chart-b", "points"), shown);
        assert!(r.last_instance_work().bindings_evaluated > 8);
    }
}

#[test]
fn a_callback_sees_its_item_index_and_scope_and_shadows_outer_names() {
    let src = r#"shape Row
  id: string
  n: number
fn scaled(xs: list<number>, k: number): list<number> = map(xs, x => x * k)
component A
  resource rows = rows() as shape list<Row>
  state k = 10
  view
    column
      text join(map(rows, (r, i) => `${i}:${r.id}:${r.n * k}`), " ") testId="a"
      text join(scaled(map(rows, r => r.n), 2), ",") testId="b"
      text join(map(rows, k => k.id), "") testId="c"
      text join(map(rows, () => "."), "") testId="d"
      text join(map(map(rows, r => r.n), (n, i) => join(map(rows, s => toString(s.n + n + i)), "+")), " ") testId="e"
      text join(filter(map(rows, r => r.n), (n, i) => i > 0 and n > 1), ",") testId="f"
      text join(map(filter(rows, r => r.n > 99), r => r.id), ",") testId="g"
"#;
    struct Rows;
    impl DataSource for Rows {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(Value::list(
                [("p", 1.0), ("q", 2.0), ("r", 3.0)]
                    .iter()
                    .map(|(id, n)| Value::record(vec![Value::str(id), Value::Number(*n)]))
                    .collect(),
            ))
        }
    }
    let plan = contract::bake(contract::compile(src).unwrap(), Rows).unwrap();
    let r = Runner::boot(
        plan,
        Rows,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let text = |id: &str| {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .props
            .iter()
            .find_map(|(p, v)| match v {
                PropValue::Str(s) if p.name() == "text" => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default()
    };
    assert_eq!(text("a"), "0:p:10 1:q:20 2:r:30");
    assert_eq!(text("b"), "2,4,6");
    assert_eq!(text("c"), "pqr");
    assert_eq!(text("d"), "...");
    assert_eq!(text("e"), "2+3+4 4+5+6 6+7+8");
    assert_eq!(text("f"), "2,3");
    assert_eq!(text("g"), "");
}

#[test]
fn an_arrow_is_only_a_callback_and_the_types_hold() {
    let refused = |body: &str, id: &str| {
        let src = format!(
            "shape S\n  n: number\ncomponent A\n  resource xs = xs() as shape list<S>\n  state ns = 0\n  view\n    text {body}\n"
        );
        let Err(e) = contract::compile(&src) else {
            panic!("{body} compiled")
        };
        assert_eq!(e.id, id, "{body}: {e}");
    };
    refused("x => 1", "syntax-expected-expression");
    refused("toString(x => 1)", "type-arrow-position");
    refused(
        "join(map(xs, (a, a) => \"\"), \"\")",
        "type-arrow-parameters",
    );
    refused(
        "join(map(xs, (a, b, c) => \"\"), \"\")",
        "type-arrow-parameters",
    );
    refused("join(map(xs, 1), \"\")", "type-argument");
    refused("join(map(ns, x => x), \"\")", "type-argument");
    refused("join(xs, \",\")", "type-argument");
    refused("join(map(xs, x => x.n), 1)", "type-argument");
    refused("toString(length(filter(xs, x => x.n)))", "type-argument");
    refused("join(map(xs), \"\")", "type-arity");
    refused("join(map(xs, x => x.missing), \"\")", "type-unknown-field");
    // A `fn` may not take a callback, and the index is a number.
    let e = contract::compile(
        "fn f(n: number): number = n\ncomponent A\n  view\n    text `${f(x => 1)}`\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-arrow-position", "{e}");
    let e =
        contract::compile("fn map(n: number): number = n\ncomponent A\n  view\n    text \"a\"\n")
            .unwrap_err();
    assert_eq!(e.id, "contract-fn-shadows-roster", "{e}");
}

#[test]
fn callback_runs_are_bounded_per_evaluation_and_the_trap_is_named() {
    let src = r#"component A
  resource xs = xs() as shape list<number>
  view
    text toString(length(map(xs, x => length(map(xs, y => y))))) testId="n"
"#;
    struct Xs(usize);
    impl DataSource for Xs {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(numbers(&vec![1.0; self.0]))
        }
    }
    // 255 outer runs, each 255 inner: 255 * 256 = 65,280 runs fit;
    // 256 * 257 = 65,792 do not.
    let plan = contract::compile(src).unwrap();
    contract::bake(plan.clone(), Xs(255)).unwrap();
    let error = contract::bake(plan.clone(), Xs(256)).unwrap_err();
    assert!(format!("{error:?}").contains("IterationLimit"), "{error:?}");
    // At run time the commit is refused whole, and the journal says why.
    let r = Runner::boot(
        plan,
        Xs(256),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    );
    match r {
        Err(RunnerError::Instance(InstanceError::Trap(Trap::IterationLimit { .. }))) => {}
        Err(e) => panic!("{e:?}"),
        Ok(_) => panic!("booted past the bound"),
    }
}

/// A callback's parameters are binders: a prop passed in that mentions the
/// same name is not captured by them; a `fn` that reaches itself through a
/// callback is recursive; a call repeated inside a callback is bound inside
/// it, never hoisted ahead of a list that may be empty.
#[test]
fn a_callback_parameter_is_a_binder_through_inlining_and_sharing() {
    struct Ks;
    impl DataSource for Ks {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(numbers(&[1.0, 2.0]))
        }
    }
    let src = r#"fn twice(n: number): number = n * 2
fn sum2(xs: list<number>): string = join(map(xs, x => twice(x) + twice(x)), ",")
component A
  resource ks = ks() as shape list<number>
  state k = 100
  view
    column
      Child(xs=ks, n=k)
      text sum2(ks) testId="shared"
      text sum2(filter(ks, x => x > 5)) testId="empty"
component Child
  props
    xs: list<number>
    n: number
  view
    text join(map(xs, k => k + n), ",") testId="captured"
"#;
    let plan = contract::bake(contract::compile(src).unwrap(), Ks).unwrap();
    let r = Runner::boot(plan, Ks, Kernel::with_monospace(), Default::default(), "/").unwrap();
    let text = |id: &str| {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .props
            .iter()
            .find_map(|(p, v)| match v {
                PropValue::Str(s) if p.name() == "text" => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default()
    };
    // `n` is the parent's `k` (100), not the callback's item.
    assert_eq!(text("captured"), "101,102");
    assert_eq!(text("shared"), "4,8");
    assert_eq!(text("empty"), "");
    let e = contract::compile(
        "fn f(xs: list<number>): number = length(map(xs, x => f(xs)))\ncomponent A\n  view\n    text \"a\"\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-fn-recursive", "{e}");
    let share = contract_syntax::share_calls;
    let file = contract_syntax::parse(
        "fn g(xs: list<number>): string = join(map(xs, x => f(x) + f(x)), \",\")\ncomponent A\n  view\n    text \"a\"\n",
    )
    .unwrap();
    let shared = share(&file.fns[0].body, &|n| n == "f");
    let contract_syntax::Expr::Call(_, args, _) = &shared else {
        panic!("{shared:?}")
    };
    let contract_syntax::Expr::Call(_, inner, _) = &args[0] else {
        panic!("{shared:?}")
    };
    assert!(
        matches!(&inner[1], contract_syntax::Expr::Arrow { body, .. } if matches!(**body, contract_syntax::Expr::Let { .. })),
        "bound inside the callback: {shared:?}"
    );
}

/// LLP 1017.003 §Diagnostics: the web's list idioms, as an agent or a web
/// developer writes them first, are each refused with the spelling that works
/// or what to do instead.
#[test]
fn the_webs_list_idioms_are_refused_with_their_fix() {
    let refused = |line: &str, id: &str, fix: &str| {
        let src = format!(
            "shape S\n  name: string\n  n: number\ncomponent A\n  resource xs = xs() as shape list<S>\n  view\n    column\n      {line}\n"
        );
        let Err(e) = contract::compile(&src) else {
            panic!("{line} compiled")
        };
        assert_eq!(e.id, id, "{line}: {e}");
        assert!(e.message.contains(fix), "{line}: {e}");
    };
    // 1. Methods.
    for m in ["map", "filter"] {
        refused(
            &format!("text join(xs.{m}(x => x.name), \",\")"),
            "syntax-method-call",
            &format!("`{m}(xs, (x, i) => …)`"),
        );
    }
    refused(
        "text xs.join(\",\")",
        "syntax-method-call",
        "`join(xs, \", \")`",
    );
    refused(
        "text toString(xs.length)",
        "type-not-a-record",
        "`length(xs)`",
    );
    refused(
        "text toString(xs.map)",
        "type-not-a-record",
        "`map(xs, (x, i) => …)`",
    );
    // 2. Refused neighbours, as functions and as methods.
    refused(
        "text toString(length(some(xs, x => x.n)))",
        "syntax-refused-idiom",
        "compute it in the data source",
    );
    refused(
        "text toString(xs.some(x => x.n))",
        "syntax-method-call",
        "compute it in the data source",
    );
    for f in [
        // `slice` and `concat` name LLP 1086 §9's follow-up (`diagnostics.rs`).
        "reduce", "find", "every", "sort", "flatMap",
    ] {
        refused(
            &format!("text toString({f}(xs, x => x.n))"),
            "type-refused-idiom",
            "compute it in the data source",
        );
        refused(
            &format!("text toString(xs.{f}(x => x.n))"),
            "syntax-method-call",
            "compute it in the data source",
        );
    }
    refused(
        "text toString(Math.min(1, 2))",
        "syntax-method-call",
        "`min(a, b)` for two numbers",
    );
    refused(
        "text toString(Math.max(1, 2))",
        "syntax-method-call",
        "`lo` and `hi`",
    );
    refused(
        "text toFixed(1.5, 2)",
        "type-refused-idiom",
        "floor(v * 100 + 0.5) / 100",
    );
    refused(
        "text (1.5).toFixed(2)",
        "syntax-method-call",
        "formatNumber",
    );
    // 3. Mapping to view nodes.
    refused(
        "map(xs, x => text x.name)",
        "syntax-map-view",
        "`each x in xs key=x.id`",
    );
    refused(
        "text join(map(xs, x => text x.name), \"\")",
        "syntax-callback-view",
        "`each x in xs key=x.id`",
    );
    refused(
        "text join(map(xs, x => Row(s=x)), \"\")",
        "type-callback-view",
        "`each x in xs key=x.id`",
    );
    // 4. Truthiness.
    refused(
        "text toString(length(filter(xs, x => x.name)))",
        "type-argument",
        "`x.name != \"\"`",
    );
    refused(
        "text toString(length(filter(xs, x => x.n)))",
        "type-argument",
        "`x.count != 0`",
    );
    // A field named like a method is still a field, and a spaced `(` is not a call.
    let ok = "shape S\n  map: string\ncomponent A\n  resource xs = xs() as shape list<S>\n  view\n    text join(map(xs, x => x.map), \",\")\n";
    contract::compile(ok).unwrap();
}

/// Two coins from `tick()`, as `empty-list.contract` shapes them.
struct Tick;

impl DataSource for Tick {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "tick" => Ok(Value::list(vec![
                Value::record(vec![Value::str("a"), Value::list(vec![Value::str("x")])]),
                Value::record(vec![Value::str("b"), Value::list(vec![])]),
            ])),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

/// `[]` is the empty list, typed by the other arm of a `match` or `?:`, a
/// declared `list<T>`, or a write into the state it initializes (LLP
/// 1017.003 D4), and it runs as the empty list it names.
#[test]
fn an_empty_list_is_typed_from_its_context_and_runs() {
    let plan = contract::bake(
        contract::compile(&corpus("empty-list.contract")).unwrap(),
        Tick,
    )
    .unwrap();
    let mut r = Runner::boot(
        Plan::decode(&plan.encode()).unwrap(),
        Tick,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let text = |r: &Runner<Tick>, id: &str| {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .props
            .iter()
            .find_map(|(p, v)| match v {
                PropValue::Str(s) if p.name() == "text" => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default()
    };
    // Before the mutation answers, every list is the empty one.
    assert_eq!(text(&r, "count"), "0 coins");
    assert_eq!(text(&r, "shown"), "");
    assert_eq!(text(&r, "picked"), "");
    assert_eq!(text(&r, "fn"), "");
    assert_eq!(text(&r, "tags"), "0 tags");
    r.act("tick", vec![]).unwrap();
    assert_eq!(text(&r, "count"), "2 coins");
    // `shown` is `[]` until toggled, then the coins.
    assert_eq!(text(&r, "shown"), "");
    r.act("toggle", vec![]).unwrap();
    assert_eq!(text(&r, "shown"), "a,b");
    r.act("toggle", vec![]).unwrap();
    assert_eq!(text(&r, "shown"), "");
    // `picked` was `[]`; the write typed it `list<string>`.
    r.act("pick", vec![Value::str("b")]).unwrap();
    assert_eq!(text(&r, "picked"), "b");
    r.act("pick", vec![Value::str("zzz")]).unwrap();
    assert_eq!(text(&r, "picked"), "");
}

/// An `[]` nothing types is refused where it is written, not as a cycle;
/// a state only `[]` initializes is refused as a state nothing writes; a
/// list literal with items is not Contract.
#[test]
fn an_empty_list_nothing_types_is_refused_where_it_is_written() {
    let e = contract::compile("component A\n  derive xs = []\n  view\n    text join(xs, \",\")\n")
        .unwrap_err();
    assert_eq!(e.id, "type-cannot-infer", "{e}");
    assert!(e.message.contains("what `[]` holds in `xs`"), "{e}");
    assert_eq!((e.span.line, e.span.col), (2, 15), "{e}");
    let e = contract::compile(
        "component A\n  state flag = true\n  derive xs = flag ? [] : []\n  view\n    text join(xs, \",\")\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-cannot-infer", "{e}");
    assert_eq!((e.span.line, e.span.col), (3, 22), "{e}");
    let e = contract::compile(
        "component A\n  state picked = []\n  view\n    text join(picked, \",\")\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-cannot-infer", "{e}");
    assert!(e.message.contains("nothing writes a value into it"), "{e}");
    let e = contract::compile(
        "component A\n  state on = true\n  derive xs = on ? \"a\" : []\n  view\n    text xs\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-branches", "{e}");
    assert_eq!(e.message, "branches disagree: `string` and `list<?>`");
    let e = contract::compile("component A\n  derive xs = [1, 2]\n  view\n    text \"a\"\n")
        .unwrap_err();
    assert_eq!(e.id, "syntax-expected", "{e}");
    assert!(e.message.contains("`[]` is the empty list"), "{e}");
    assert_eq!((e.span.line, e.span.col), (2, 16), "{e}");
    // A `fn` returning a declared `list<T>` and a `list<T>` prop type it.
    contract::compile(
        "fn nothing(): list<number> = []\ncomponent A\n  view\n    column\n      B(xs=[])\n      text `${length(nothing())}`\ncomponent B\n  props\n    xs: list<string>\n  view\n    text join(xs, \",\")\n",
    )
    .unwrap();
    let e = contract::compile("fn nothing(): number = []\ncomponent A\n  view\n    text \"a\"\n")
        .unwrap_err();
    assert_eq!(e.id, "type-fn-return", "{e}");
    // A state a later action writes is a typed source argument, whatever
    // the declaration order (the second review's two Contracts).
    for src in [
        "shape Coin\n  id: string\ncomponent A\n  state q = none\n  action set(s: string)\n    q = some(s)\n  resource xs = src(q) as shape list<Coin>\n  view\n    text `${length(xs)}`\n",
        "shape Coin\n  id: string\ncomponent A\n  mutation changed as shape list<Coin>\n  state q = none\n  action tick\n    send changed = tick(q)\n  action set(s: string)\n    q = some(s)\n  view\n    text \"a\"\n",
        "shape Coin\n  id: string\ncomponent A\n  mutation changed as shape list<Coin>\n  state ids = []\n  resource coins = coins() as shape list<Coin>\n  action tick\n    send changed = tick(ids)\n  action pick\n    ids = map(coins, c => c.id)\n  view\n    text \"a\"\n",
    ] {
        contract::compile(src).unwrap_or_else(|e| panic!("{src}: {e}"));
    }
    // A literal `[]` (or `none`) passed to a source has nothing to type it:
    // refused at the argument, not left for lowering to find at 0:0.
    for (src, line, col) in [
        (
            "shape C\n  id: string\ncomponent A\n  mutation changed as shape list<C>\n  action tick\n    send changed = tick([])\n  view\n    text \"a\"\n",
            6,
            25,
        ),
        (
            "shape C\n  id: string\ncomponent A\n  resource xs = src([]) as shape list<C>\n  view\n    text `${length(xs)}`\n",
            4,
            21,
        ),
        (
            "shape C\n  id: string\ncomponent A\n  resource xs = src(none) as shape list<C>\n  view\n    text `${length(xs)}`\n",
            4,
            21,
        ),
        (
            "shape C\n  id: string\ncomponent A\n  resource xs = src(1) as shape list<C> else src([])\n  view\n    text `${length(xs)}`\n",
            4,
            50,
        ),
        // `each` over `[]` has nothing to type its item.
        (
            "component A\n  view\n    column\n      each x in [] key=\"k\"\n        text x\n",
            4,
            17,
        ),
    ] {
        let e = contract::compile(src).unwrap_err();
        assert_eq!(e.id, "type-cannot-infer", "{src}: {e}");
        assert_eq!((e.span.line, e.span.col), (line, col), "{src}: {e}");
    }
}
