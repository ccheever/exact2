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
            PropValue::Str(s) if p.name() == name => Some(s.to_string()),
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
            "      for c in coins key=c.id\n",
            "      list virtualized=true estimated-item-height=32 height=400\n        for c in coins key=c.id\n",
        )
        .replace("\n        row testId", "\n          row testId")
        .replace("\n          text c.id", "\n            text c.id")
        .replace("\n          text (c.flash", "\n            text (c.flash")
        .replace("\n          text c.tags", "\n            text c.tags")
        .replace("\n          svg ", "\n            svg ")
        .replace("\n            polyline", "\n              polyline"),
    ] {
        assert!(!src.contains("virtualized") || src.contains("list virtualized=true estimated-item-height=32 height=400\n        for c in coins key=c.id\n          row"));
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
fn scaled(xs: number[], k: number): number[] = xs.map(x => x * k)
component A
  resource rows = rows() as shape Row[]
  state k = 10
  view
    column
      text rows.map((r, i) => `${i}:${r.id}:${r.n * k}`).join(" ") testId="a"
      text scaled(rows.map(r => r.n), 2).join(",") testId="b"
      text rows.map(k => k.id).join("") testId="c"
      text rows.map(() => ".").join("") testId="d"
      text rows.map(r => r.n).map((n, i) => rows.map(s => (s.n + n + i).toString()).join("+")).join(" ") testId="e"
      text rows.map(r => r.n).filter((n, i) => i > 0 and n > 1).join(",") testId="f"
      text rows.filter(r => r.n > 99).map(r => r.id).join(",") testId="g"
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
                PropValue::Str(s) if p.name() == "text" => Some(s.to_string()),
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
            "shape S\n  n: number\ncomponent A\n  resource xs = xs() as shape S[]\n  state ns = 0\n  view\n    text {body}\n"
        );
        let Err(e) = contract::compile(&src) else {
            panic!("{body} compiled")
        };
        assert_eq!(e.id, id, "{body}: {e}");
    };
    refused("x => 1", "syntax-expected-expression");
    refused("round(x => 1).toString()", "type-arrow-position");
    refused("xs.map((a, a) => \"\").join(\"\")", "type-arrow-parameters");
    refused(
        "xs.map((a, b, c) => \"\").join(\"\")",
        "type-arrow-parameters",
    );
    refused("xs.map(1).join(\"\")", "type-argument");
    refused("ns.map(x => x).join(\"\")", "type-argument");
    refused("xs.join(\",\")", "type-argument");
    refused("xs.map(x => x.n).join(1)", "type-argument");
    refused("xs.filter(x => x.n).length.toString()", "type-argument");
    refused("xs.map().join(\"\")", "type-arity");
    refused("xs.map(x => x.missing).join(\"\")", "type-unknown-field");
    // A `fn` may not take a callback, and the index is a number.
    let e = contract::compile(
        "fn f(n: number): number = n\ncomponent A\n  view\n    text `${f(x => 1)}`\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-arrow-position", "{e}");
    // An app's `fn map` shadows the roster's, callback and all.
    contract::compile("fn map(n: number): number = n\ncomponent A\n  view\n    text `${map(2)}`\n")
        .unwrap();
    // A method call is the built-in, so a `fn`, shape or action of its name is
    // refused rather than called (LLP 1116.016 D2), in a template too.
    for src in [
        "fn trim(s: string): string = s\ncomponent A\n  state s = \" a \"\n  view\n    text s.trim()\n",
        "fn length(s: string): number = 0\ncomponent A\n  state s = \"a\"\n  view\n    text `${s.length}`\n",
        "component A\n  state xs = [1]\n  state n = 0\n  action filter(k: number)\n    n = k\n  view\n    text xs.filter(x => x > 0).length.toString()\n",
    ] {
        let e = contract::compile(src).unwrap_err();
        assert_eq!(e.id, "syntax-method-shadowed", "{src}: {e}");
        assert!(e.message.contains("is the built-in method"), "{e}");
    }
}

#[test]
fn callback_runs_are_bounded_per_evaluation_and_the_trap_is_named() {
    let src = r#"component A
  resource xs = xs() as shape number[]
  view
    text xs.map(x => xs.map(y => y).length).length.toString() testId="n"
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

/// `sum` (the euxy port's sheet height over its lanes) is
/// `xs.reduce((a, x) => a + x, 0)`: left to right from +0, a list step an
/// item on the evaluation's budget. `sin` and `cos` are `Math`'s (its ring of
/// LEDs), through the pinned `libm`.
#[test]
fn sum_totals_a_list_on_the_budget_and_sin_and_cos_are_maths() {
    let src = r#"shape Lane
  rows: number
component A
  resource xs = xs() as shape number[]
  state lanes = [Lane(rows=2), Lane(rows=3)]
  view
    column
      text sum([1, 2, 3]).toString() testId="lit"
      text sum([1].filter(x => x > 5)).toString() testId="empty"
      text sum(lanes.map(l => l.rows * 44 + 8)).toString() testId="field"
      text sum([0.1, 0.2, 0.3]).toString() testId="order"
      text sum([-0]).toString() testId="zero"
      text sum(xs).toString() testId="big"
      text `${sin(0)} ${cos(0)} ${round(sin(3.141592653589793 / 2) * 1000)} ${round(cos(3.141592653589793) * 1000)} ${sin(1)} ${cos(1)}` testId="trig"
"#;
    struct Xs(usize);
    impl DataSource for Xs {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(numbers(&vec![1.0; self.0]))
        }
    }
    let plan = contract::compile(src).unwrap();
    let r = Runner::boot(
        contract::bake(plan.clone(), Xs(65_536)).unwrap(),
        Xs(65_536),
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
                PropValue::Str(s) if p.name() == "text" => Some(s.to_string()),
                _ => None,
            })
            .unwrap_or_default()
    };
    assert_eq!(text("lit"), "6");
    assert_eq!(text("empty"), "0");
    assert_eq!(text("field"), "236");
    // ((0 + 0.1) + 0.2) + 0.3, as `reduce` adds; not `Math.sumPrecise`'s 0.6.
    assert_eq!(text("order"), "0.6000000000000001");
    assert_eq!(text("zero"), "0");
    assert_eq!(text("big"), "65536");
    // `Math.sin(1)` and `Math.cos(1)` as JavaScriptCore and V8 print them.
    assert_eq!(
        text("trig"),
        "0 1 1000 -1000 0.8414709848078965 0.5403023058681398"
    );
    // One more item than the bound is refused whole, as `map` is.
    let error = contract::bake(plan, Xs(65_537)).unwrap_err();
    assert!(format!("{error:?}").contains("IterationLimit"), "{error:?}");
    // `sum` takes numbers only.
    let e = contract::compile(
        "component A\n  state xs = [\"a\"]\n  view\n    text sum(xs).toString()\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-argument", "{e}");
}

/// `sortBy` (LLP 1116.016 §5): a stable sort by one number or string key,
/// equal keys in input order in both directions; NaN, -0 and +0 tie;
/// strings by UTF-16 code unit. Its list steps are its `Map`'s, one an
/// item, so 5,000 rows filtered then sorted take 10,000 of the 65,536.
#[test]
fn sort_by_is_stable_by_one_key_and_steps_once_an_item() {
    let src = r#"shape Row
  name: string
  n: number
component A
  resource xs = xs() as shape number[]
  state rows = [Row(name="b", n=2), Row(name="a", n=1), Row(name="c", n=2), Row(name="d", n=1)]
  view
    column
      text sortBy(rows, r => r.n).map(r => r.name).join("") testId="asc"
      text sortBy(rows, r => r.n, "desc").map(r => r.name).join("") testId="desc"
      text sortBy(sortBy(rows, r => r.name, "desc"), r => r.n).map(r => r.name).join("") testId="two"
      text sortBy(rows, (r, i) => 0 - i).map(r => r.name).join("") testId="index"
      text sortBy([3, 0 / 0, -0, 1, 0, -1], x => x).map(x => `${x}`).join(" ") testId="nan"
      text sortBy(["～", "😀", "a", "ab", "", "a"], s => s).join(",") testId="units"
      text sortBy(xs.filter(x => x > 0), x => 0 - x).length.toString() testId="big"
      text sortBy(rows.filter(r => r.n > 5), r => r.name).length.toString() testId="empty"
"#;
    struct Xs(usize);
    impl DataSource for Xs {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(numbers(&vec![1.0; self.0]))
        }
    }
    let plan = contract::compile(src).unwrap();
    let r = Runner::boot(
        contract::bake(plan.clone(), Xs(5_000)).unwrap(),
        Xs(5_000),
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
                PropValue::Str(s) if p.name() == "text" => Some(s.to_string()),
                _ => None,
            })
            .unwrap_or_default()
    };
    // Expected values from Bun: a stable `toSorted` comparing the keys.
    assert_eq!(text("asc"), "adbc");
    assert_eq!(text("desc"), "bcad");
    // A second key is an earlier sort: by n, ties by name descending.
    assert_eq!(text("two"), "dacb");
    assert_eq!(text("index"), "dcab");
    assert_eq!(text("nan"), "-1 NaN 0 0 1 3");
    assert_eq!(text("units"), ",a,a,ab,😀,～");
    assert_eq!(text("big"), "5000");
    assert_eq!(text("empty"), "0");
    // 32,768 filtered and sorted is 65,536 steps; one more is refused.
    contract::bake(plan.clone(), Xs(32_768)).unwrap();
    let error = contract::bake(plan, Xs(32_769)).unwrap_err();
    assert!(format!("{error:?}").contains("IterationLimit"), "{error:?}");
    for (body, id) in [
        ("sortBy(rows, r => r)", "type-argument"),
        ("sortBy(rows, r => r.n > 1)", "type-argument"),
        ("sortBy(rows, r => r.n, \"up\")", "type-argument"),
        ("sortBy(rows, r => r.n, dir)", "type-argument"),
        ("sortBy(rows)", "type-arity"),
        ("sortBy(rows, rows)", "type-argument"),
        ("rows.sortBy(r => r.n)", "syntax-method-call"),
    ] {
        let src = format!(
            "shape Row\n  n: number\ncomponent A\n  state rows = [Row(n=1)]\n  state dir = \"asc\"\n  view\n    text {body}.length.toString()\n"
        );
        let e = contract::compile(&src).unwrap_err();
        assert_eq!(e.id, id, "{body}: {e}");
    }
}

/// `range(n)` (the euxy port's `range64().slice(0, n)`) is 0 to n - 1, a list
/// step an item before it is built; a count that is not a whole number at
/// least 0 does not fit: refused where it is a literal, a trap where it is
/// computed, which refuses the commit that would make it.
#[test]
fn range_counts_from_zero_on_the_budget_and_refuses_what_is_no_count() {
    let src = r#"component A
  state n = 3
  derive xs = range(n)
  action set(v: number)
    n = v
  view
    column
      text xs.join(",") testId="xs"
      text `${range(0).length} ${range(-0).length} ${sum(range(4))}` testId="edges"
      for i in range(2) key=i
        text `row ${i}` testId=`row-${i}`
"#;
    struct Nothing;
    impl DataSource for Nothing {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(Value::list(Vec::new()))
        }
    }
    let plan = contract::compile(src).unwrap();
    let mut r = Runner::boot(
        plan,
        Nothing,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let text = |r: &Runner<Nothing>, id: &str| {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .props
            .iter()
            .find_map(|(p, v)| match v {
                PropValue::Str(s) if p.name() == "text" => Some(s.to_string()),
                _ => None,
            })
            .unwrap_or_default()
    };
    assert_eq!(text(&r, "xs"), "0,1,2");
    assert_eq!(text(&r, "edges"), "0 0 6");
    assert_eq!(text(&r, "row-1"), "row 1");
    for bad in [2.5, -1.0] {
        let e = r.act("set", vec![Value::Number(bad)]).unwrap_err();
        assert!(format!("{e:?}").contains("TypeMismatch"), "{bad}: {e:?}");
        assert_eq!(text(&r, "xs"), "0,1,2", "{bad} refused whole");
    }
    // NaN and the infinities live only inside an evaluation.
    for bad in ["0 / 0", "1 / 0", "-1 / 0"] {
        let plan = contract::compile(&format!(
            "component A\n  state z = 0\n  derive xs = range({bad} + z)\n  view\n    text xs.join(\",\")\n"
        ))
        .unwrap();
        let e = Runner::boot(
            plan,
            Nothing,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .err()
        .unwrap_or_else(|| panic!("{bad} booted"));
        assert!(format!("{e:?}").contains("TypeMismatch"), "{bad}: {e:?}");
    }
    r.act("set", vec![Value::Number(65_536.0)]).unwrap();
    let e = r.act("set", vec![Value::Number(65_537.0)]).unwrap_err();
    assert!(format!("{e:?}").contains("IterationLimit"), "{e:?}");
    let e = r.act("set", vec![Value::Number(1e300)]).unwrap_err();
    assert!(format!("{e:?}").contains("IterationLimit"), "{e:?}");
    // Literals are refused where they are written; `range(a, b)` names the slice.
    for (call, id, says) in [
        (
            "range(2.5)",
            "type-argument",
            "a whole number at least 0; given `2.5`",
        ),
        ("range(-1)", "type-argument", "given `-1`"),
        ("range(\"3\")", "type-argument", "expects `number`"),
        ("range(1, 4)", "type-arity", "`range(n).slice(a, b)`"),
    ] {
        let e = contract::compile(&format!(
            "component A\n  view\n    text {call}.join(\",\")\n"
        ))
        .unwrap_err();
        assert_eq!(e.id, id, "{call}: {e}");
        assert!(e.message.contains(says), "{call}: {e}");
    }
}

/// `xs.flat()` (the euxy port joined lists of lines into a string and split
/// it again) is `Array.prototype.flat()` at depth 1: a step an outer item and
/// one an item it keeps, taken before the list is built; no depth is taken.
#[test]
fn flat_joins_one_level_on_the_budget_and_takes_no_depth() {
    let src = r#"shape Lane
  hits: number[]
component A
  resource xss = xss() as shape number[][]
  state lanes = [Lane(hits=[0, 4]), Lane(hits=[]), Lane(hits=[2])]
  view
    column
      text lanes.map(l => l.hits).flat().join(",") testId="hits"
      text [["a"], ["b", "c"]].flat().join("") testId="lit"
      text `${[[[1]], [[2, 3]]].flat().flat().length}` testId="deep"
      text `${xss.flat().length}` testId="big"
"#;
    // `n` lists of one item: `2n` steps.
    struct Xss(usize);
    impl DataSource for Xss {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(Value::list(vec![numbers(&[1.0]); self.0]))
        }
    }
    let plan = contract::compile(src).unwrap();
    let r = Runner::boot(
        contract::bake(plan.clone(), Xss(32_768)).unwrap(),
        Xss(32_768),
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
                PropValue::Str(s) if p.name() == "text" => Some(s.to_string()),
                _ => None,
            })
            .unwrap_or_default()
    };
    assert_eq!(text("hits"), "0,4,2");
    assert_eq!(text("lit"), "abc");
    assert_eq!(text("deep"), "3");
    assert_eq!(text("big"), "32768");
    let error = contract::bake(plan, Xss(32_769)).unwrap_err();
    assert!(format!("{error:?}").contains("IterationLimit"), "{error:?}");
    for (call, id, says) in [
        (
            "[[1]].flat(2)",
            "type-arity",
            "two levels are `xs.flat().flat()`",
        ),
        (
            "[1, 2].flat()",
            "type-argument",
            "flattens a list of lists, given `number[]`",
        ),
        ("flat([[1]])", "syntax-method-form", "write `x.flat()`"),
    ] {
        let e = contract::compile(&format!(
            "component A\n  view\n    text {call}.join(\",\")\n"
        ))
        .unwrap_err();
        assert_eq!(e.id, id, "{call}: {e}");
        assert!(e.message.contains(says), "{call}: {e}");
    }
}

/// A shape may declare a field named `length` (LLP 1116.015 D2, amended
/// 2026-10-17): `x.length` reads it on such a record, built and copied with
/// it as any field, and still counts a list or text; on a record without one
/// it is refused, saying what `.length` counts.
#[test]
fn a_field_named_length_is_read_and_written_and_lists_still_count() {
    let src = r#"shape Lane
  name: string
  length: number
fn steps(l: Lane): number = l.length
component A
  state lane = Lane(name="kick", length=16)
  state lanes = [Lane(name="a", length=8), Lane(name="bb", length=12)]
  action grow
    lane = Lane(lane, length=lane.length + 1)
  view
    column
      text `${lane.length} ${steps(lane)} ${lane.name.length}` testId="lane"
      text `${lanes.length} ${lanes.filter(l => l.length > 10).length} ${sum(lanes.map(l => l.length))}` testId="lanes"
"#;
    struct Nothing;
    impl DataSource for Nothing {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(Value::list(Vec::new()))
        }
    }
    let plan = contract::compile(src).unwrap();
    let mut r = Runner::boot(
        plan,
        Nothing,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let text = |r: &Runner<Nothing>, id: &str| {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .props
            .iter()
            .find_map(|(p, v)| match v {
                PropValue::Str(s) if p.name() == "text" => Some(s.to_string()),
                _ => None,
            })
            .unwrap_or_default()
    };
    assert_eq!(text(&r, "lane"), "16 16 4");
    assert_eq!(text(&r, "lanes"), "2 1 20");
    r.act("grow", vec![]).unwrap();
    assert_eq!(text(&r, "lane"), "17 17 4");
    // The Lean embedding reads the field as a field, and a list's count as a call.
    let lean = contract::lean::lean(src, "lengths").unwrap();
    assert!(
        lean.contains("(.member (.var \"lane\") \"length\")"),
        "{lean}"
    );
    assert!(
        lean.contains("(.call \"length\" [(.var \"lanes\")])"),
        "{lean}"
    );
    let e = contract::compile(
        "shape Item\n  name: string\ncomponent A\n  state it = Item(name=\"a\")\n  view\n    text `${it.length}`\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-unknown-field", "{e}");
    assert!(
        e.message.contains("`.length` counts a list or text, and `Item` is a record: `Item` has no field `length`; available fields: `name`"),
        "{e}"
    );
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
fn sum2(xs: number[]): string = xs.map(x => twice(x) + twice(x)).join(",")
component A
  resource ks = ks() as shape number[]
  state k = 100
  view
    column
      Child(xs=ks, n=k)
      text sum2(ks) testId="shared"
      text sum2(ks.filter(x => x > 5)) testId="empty"
component Child
  props
    xs: number[]
    n: number
  view
    text xs.map(k => k + n).join(",") testId="captured"
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
                PropValue::Str(s) if p.name() == "text" => Some(s.to_string()),
                _ => None,
            })
            .unwrap_or_default()
    };
    // `n` is the parent's `k` (100), not the callback's item.
    assert_eq!(text("captured"), "101,102");
    assert_eq!(text("shared"), "4,8");
    assert_eq!(text("empty"), "");
    let e = contract::compile(
        "fn f(xs: number[]): number = xs.map(x => f(xs)).length\ncomponent A\n  view\n    text \"a\"\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-fn-recursive", "{e}");
    let share = contract_syntax::share_calls;
    let file = contract_syntax::parse(
        "fn g(xs: number[]): string = xs.map(x => f(x) + f(x)).join(\",\")\ncomponent A\n  view\n    text \"a\"\n",
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
            "shape S\n  name: string\n  n: number\ncomponent A\n  resource xs = xs() as shape S[]\n  view\n    column\n      {line}\n"
        );
        let Err(e) = contract::compile(&src) else {
            panic!("{line} compiled")
        };
        assert_eq!(e.id, id, "{line}: {e}");
        assert!(e.message.contains(fix), "{line}: {e}");
    };
    // 1. The web's methods are Contract's (LLP 1116.015 D2), and the free
    // spellings name the method form.
    for line in [
        "text xs.map(x => x.name).join(\",\")",
        "text xs.filter(x => x.n > 0).map(x => x.name).join(\",\")",
        "text \"a,b\".split(\",\").join(\",\")",
        "text xs.map(x => x.name).indexOf(\"a\").toString()",
        "text xs.length.toString()",
        "text (1.5).toFixed(2)",
    ] {
        let src = format!(
            "shape S\n  name: string\n  n: number\ncomponent A\n  resource xs = xs() as shape S[]\n  view\n    column\n      {line}\n"
        );
        if let Err(e) = contract::compile(&src) {
            panic!("{line}: {e}")
        }
    }
    for (line, fix) in [
        (
            "text (map(xs, x => x.name)).join(\",\")",
            "write `xs.map(…)`",
        ),
        (
            "text filter(xs, x => x.n > 0).length.toString()",
            "write `xs.filter(…)`",
        ),
        ("text join(xs.map(x => x.name), \",\")", ".join(…)`"),
        ("text split(\"a,b\", \",\").join(\",\")", ".split(…)`"),
        (
            "text indexOf(xs, \"a\").toString()",
            "write `xs.indexOf(…)`",
        ),
        ("text length(xs).toString()", "write `xs.length`"),
        ("text toFixed(1.5, 2)", ".toFixed(…)`"),
    ] {
        refused(line, "syntax-method-form", fix);
    }
    refused(
        "text xs.join(\",\")",
        "type-argument",
        "`map` each item to a string first",
    );
    refused(
        "text xs.map.toString()",
        "type-not-a-record",
        "call it: `.map(…)`",
    );
    // 2. Refused neighbours, as functions and as methods.
    refused(
        "text (some(xs, x => x.n)).length.toString()",
        "syntax-refused-idiom",
        "compute it in the data source",
    );
    refused(
        "text xs.some(x => x.n).toString()",
        "syntax-method-call",
        "compute it in the data source",
    );
    for f in [
        // `push` names `concat` (`diagnostics.rs`).
        "reduce", "find", "every", "sort",
    ] {
        refused(
            &format!("text ({f}(xs, x => x.n)).toString()"),
            "type-refused-idiom",
            "in the data source",
        );
        refused(
            &format!("text (xs.{f}(x => x.n)).toString()"),
            "syntax-method-call",
            "in the data source",
        );
    }
    // A comparator sort names the key sort (LLP 1116.016 §5).
    for line in [
        "text xs.sort((a, b) => a.n - b.n).length.toString()",
        "text xs.toSorted((a, b) => a.n - b.n).length.toString()",
    ] {
        refused(line, "syntax-method-call", "`sortBy(xs, x => x.name)`");
    }
    // The four with a composition in the roster name it.
    // (`some(…)` is the parser's: `some(x)` also makes an option.)
    for (f, id, instead) in [
        (
            "findIndex",
            "type-refused-idiom",
            "xs.map(x => …).indexOf(true)",
        ),
        ("find", "type-refused-idiom", "first(xs.filter(x => …))"),
        (
            "some",
            "syntax-refused-idiom",
            "xs.map(x => …).includes(true)",
        ),
        (
            "every",
            "type-refused-idiom",
            "not xs.map(x => …).includes(false)",
        ),
        ("flatMap", "type-refused-idiom", "xs.map(x => …).flat()"),
    ] {
        refused(&format!("text ({f}(xs, x => x.n)).toString()"), id, instead);
    }
    refused(
        "text (xs.flatMap(x => x.n)).toString()",
        "syntax-method-call",
        "xs.map(x => …).flat()",
    );
    // Each repair it names compiles for its callback: a list, or one value.
    refused(
        "text [1, 2].flatMap(x => x * 2).join(\",\")",
        "syntax-method-call",
        "when it returns one value, `xs.map(x => …)` is the same list",
    );
    for repair in [
        "[1, 2].map(x => [x, x * 2]).flat()",
        "[1, 2].map(x => x * 2)",
    ] {
        contract::compile(&format!(
            "component A\n  view\n    text {repair}.join(\",\")\n"
        ))
        .unwrap_or_else(|e| panic!("{repair}: {e}"));
    }
    refused(
        "text Math.min(1, 2).toString()",
        "syntax-method-call",
        "`min(a, b)` for two numbers",
    );
    refused(
        "text Math.max(1, 2).toString()",
        "syntax-method-call",
        "`lo` and `hi`",
    );
    refused(
        "text toPrecision(1.5, 2)",
        "type-refused-idiom",
        "`x.toFixed(digits)`",
    );
    // 3. Mapping to view nodes.
    refused(
        "map(xs, x => text x.name)",
        "syntax-map-view",
        "`for x in xs key=x.id`",
    );
    refused(
        "text xs.map(x => text x.name).join(\"\")",
        "syntax-callback-view",
        "`for x in xs key=x.id`",
    );
    refused(
        "text xs.map(x => Row(s=x)).join(\"\")",
        "type-callback-view",
        "`for x in xs key=x.id`",
    );
    // 4. Truthiness.
    refused(
        "text xs.filter(x => x.name).length.toString()",
        "type-argument",
        "`x.name != \"\"`",
    );
    refused(
        "text xs.filter(x => x.n).length.toString()",
        "type-argument",
        "`x.count != 0`",
    );
    // A field named like a method is still a field, and a spaced `(` is not a call.
    let ok = "shape S\n  map: string\ncomponent A\n  resource xs = xs() as shape S[]\n  view\n    text xs.map(x => x.map).join(\",\")\n";
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
/// declared `T[]`, or a write into the state it initializes (LLP
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
                PropValue::Str(s) if p.name() == "text" => Some(s.to_string()),
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
    // `picked` was `[]`; the write typed it `string[]`.
    r.act("pick", vec![Value::str("b")]).unwrap();
    assert_eq!(text(&r, "picked"), "b");
    r.act("pick", vec![Value::str("zzz")]).unwrap();
    assert_eq!(text(&r, "picked"), "");
}

/// An `[]` nothing types is refused where it is written, not as a cycle;
/// a state only `[]` initializes is refused as a state nothing writes; a
/// list literal's items have one type (LLP 1088 §9.1).
#[test]
fn an_empty_list_nothing_types_is_refused_where_it_is_written() {
    let e = contract::compile("component A\n  derive xs = []\n  view\n    text xs.join(\",\")\n")
        .unwrap_err();
    assert_eq!(e.id, "type-cannot-infer", "{e}");
    assert!(e.message.contains("what `[]` holds in `xs`"), "{e}");
    assert_eq!((e.span.line, e.span.col), (2, 15), "{e}");
    let e = contract::compile(
        "component A\n  state flag = true\n  derive xs = flag ? [] : []\n  view\n    text xs.join(\",\")\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-cannot-infer", "{e}");
    assert_eq!((e.span.line, e.span.col), (3, 22), "{e}");
    let e = contract::compile(
        "component A\n  state picked = []\n  view\n    text picked.join(\",\")\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-cannot-infer", "{e}");
    assert!(e.message.contains("nothing writes a value into it"), "{e}");
    let e = contract::compile(
        "component A\n  state on = true\n  derive xs = on ? \"a\" : []\n  view\n    text xs\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-branches", "{e}");
    assert_eq!(e.message, "branches disagree: `string` and `?[]`");
    let e = contract::compile("component A\n  derive xs = [1, \"2\"]\n  view\n    text \"a\"\n")
        .unwrap_err();
    assert_eq!(e.id, "type-list-item", "{e}");
    assert_eq!(
        e.message,
        "a list's items have one type: item 2 is `string`, the items before it `number`"
    );
    assert_eq!((e.span.line, e.span.col), (2, 19), "{e}");
    // A `fn` returning a declared `T[]` and a `T[]` prop type it.
    contract::compile(
        "fn nothing(): number[] = []\ncomponent A\n  view\n    column\n      B(xs=[])\n      text `${nothing().length}`\ncomponent B\n  props\n    xs: string[]\n  view\n    text xs.join(\",\")\n",
    )
    .unwrap();
    let e = contract::compile("fn nothing(): number = []\ncomponent A\n  view\n    text \"a\"\n")
        .unwrap_err();
    assert_eq!(e.id, "type-fn-return", "{e}");
    // A state a later action writes is a typed source argument, whatever
    // the declaration order (the second review's two Contracts).
    for src in [
        "shape Coin\n  id: string\ncomponent A\n  state q = none\n  action set(s: string)\n    q = some(s)\n  resource xs = src(q) as shape Coin[]\n  view\n    text `${xs.length}`\n",
        "shape Coin\n  id: string\ncomponent A\n  mutation changed as shape Coin[]\n  state q = none\n  action tick\n    send changed = tick(q)\n  action set(s: string)\n    q = some(s)\n  view\n    text \"a\"\n",
        "shape Coin\n  id: string\ncomponent A\n  mutation changed as shape Coin[]\n  state ids = []\n  resource coins = coins() as shape Coin[]\n  action tick\n    send changed = tick(ids)\n  action pick\n    ids = coins.map(c => c.id)\n  view\n    text \"a\"\n",
    ] {
        contract::compile(src).unwrap_or_else(|e| panic!("{src}: {e}"));
    }
    // A literal `[]` (or `none`) passed to a source has nothing to type it:
    // refused at the argument, not left for lowering to find at 0:0.
    for (src, line, col) in [
        (
            "shape C\n  id: string\ncomponent A\n  mutation changed as shape C[]\n  action tick\n    send changed = tick([])\n  view\n    text \"a\"\n",
            6,
            25,
        ),
        (
            "shape C\n  id: string\ncomponent A\n  resource xs = src([]) as shape C[]\n  view\n    text `${xs.length}`\n",
            4,
            21,
        ),
        (
            "shape C\n  id: string\ncomponent A\n  resource xs = src(none) as shape C[]\n  view\n    text `${xs.length}`\n",
            4,
            21,
        ),
        (
            "shape C\n  id: string\ncomponent A\n  resource xs = src(1) as shape C[] else src([])\n  view\n    text `${xs.length}`\n",
            4,
            46,
        ),
        // `for` over `[]` has nothing to type its item.
        (
            "component A\n  view\n    column\n      for x in [] key=\"k\"\n        text x\n",
            4,
            16,
        ),
    ] {
        let e = contract::compile(src).unwrap_err();
        assert_eq!(e.id, "type-cannot-infer", "{src}: {e}");
        assert_eq!((e.span.line, e.span.col), (line, col), "{src}: {e}");
    }
}

/// Two rows, as `list-construction.contract` shapes them.
struct Rows;

impl DataSource for Rows {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "rows" => Ok(Value::list(
                ["a", "b"]
                    .iter()
                    .map(|id| Value::record(vec![Value::str(id), Value::str(&id.to_uppercase())]))
                    .collect(),
            )),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

fn shown<D: DataSource>(r: &Runner<D>, id: &str) -> String {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(id)[0])
        .unwrap()
        .props
        .iter()
        .find_map(|(p, v)| match v {
            PropValue::Str(s) if p.name() == "text" => Some(s.to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

/// LLP 1088 §9.1: `[a, b]` is a list of its items, written across lines
/// with a trailing comma, its items' types met as a ternary's arms are; a
/// selection and a set of collapsed ids are kept with `concat`, `slice`,
/// `includes` and `indexOf`, the web's array methods, and recipients are
/// `split`, text's.
#[test]
fn a_list_the_screen_keeps_is_built_in_contract() {
    let plan = contract::bake(
        contract::compile(&corpus("list-construction.contract")).unwrap(),
        Rows,
    )
    .unwrap();
    let mut r = Runner::boot(
        Plan::decode(&plan.encode()).unwrap(),
        Rows,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(shown(&r, "tabs"), "inbox,sent");
    assert_eq!(shown(&r, "scores"), "2");
    assert_eq!(shown(&r, "lengths"), "2,0,2");
    assert_eq!(shown(&r, "chips"), "ann@x.org|bo@y.org");
    assert_eq!(shown(&r, "at"), "-1 11");
    assert_eq!(
        shown(&r, "shown"),
        "a",
        "`slice(rows, 0, -1)` drops the last"
    );
    r.act("pick", vec![Value::str("b")]).unwrap();
    r.act("pick", vec![Value::str("a")]).unwrap();
    assert_eq!(shown(&r, "picked"), "b,a");
    assert_eq!(shown(&r, "at"), "1 11");
    assert_eq!(shown(&r, "lengths"), "2,2,2");
    for id in ["a", "b", "a"] {
        r.act("toggle", vec![Value::str(id)]).unwrap();
    }
    assert_eq!(shown(&r, "collapsed"), "b");
    r.act("clear", vec![]).unwrap();
    assert_eq!(shown(&r, "picked"), "");
}

/// What each takes, refused where the web would answer something else:
/// `concat` joins two lists of one type, `includes` finds a string, number
/// or bool in a list (the web compares an object by identity), and `slice`
/// takes text or a list.
#[test]
fn concat_slice_and_includes_are_typed() {
    let refused = |expr: &str, id: &str, says: &str| {
        let src = format!(
            "shape R\n  id: string\ncomponent A\n  resource rs = rs() as shape R[]\n  state xs = [1, 2]\n  state ws = [\"a\"]\n  view\n    text ({expr}).toString()\n"
        );
        let e = contract::compile(&src).unwrap_err();
        assert_eq!(e.id, id, "{expr}: {e}");
        assert!(e.message.contains(says), "{expr}: {e}");
    };
    refused(
        "xs.concat(ws).length",
        "type-argument",
        "`concat` joins two lists of one item type, given `number[]` and `string[]`",
    );
    refused(
        "\"a\".concat(\"b\").length",
        "type-argument",
        "text joins with `+` or a template",
    );
    refused("xs.concat(1).length", "type-argument", "expects `number[]`");
    refused(
        "rs.includes(first(rs))",
        "type-argument",
        "argument 1 of `.includes(…)` expects `R`, given `option<R>`",
    );
    refused(
        "rs.map(r => r).includes(rs.at(0))",
        "type-argument",
        "expects `R`",
    );
    refused(
        "xs.includes(\"1\")",
        "type-argument",
        "expects `number`, given `string`",
    );
    refused(
        "\"ab\".includes(1)",
        "type-argument",
        "expects `string`, given `number`",
    );
    refused(
        "(1).slice(2).length",
        "type-argument",
        "expects `string | list`",
    );
    refused(
        "xs.slice(\"1\").length",
        "type-argument",
        "expects `number`",
    );
    refused("xs.concat().length", "type-arity", "`list.concat(list)`");
    let e = contract::compile(
        "shape R\n  id: string\ncomponent A\n  resource rs = rs() as shape R[]\n  view\n    text rs.includes(R(id=\"a\")).toString()\n",
    )
    .unwrap_err();
    assert!(
        e.message
            .contains("`includes` finds a string, number or bool in a list, given `R`"),
        "{e}"
    );
    contract::compile("component A\n  state xs = [1, 2]\n  view\n    text xs.concat([]).includes(0 / 0).toString()\n").unwrap();
    // `indexOf` takes what `includes` takes, and `split` text only.
    refused(
        "xs.indexOf(\"1\")",
        "type-argument",
        "argument 1 of `.indexOf(…)` expects `number`, given `string`",
    );
    refused(
        "\"ab\".indexOf(1)",
        "type-argument",
        "expects `string`, given `number`",
    );
    refused(
        "rs.map(r => r).indexOf(rs.at(0))",
        "type-argument",
        "expects `R`",
    );
    refused(
        "ws.split(\",\").length",
        "type-argument",
        "expects `string`",
    );
    let e = contract::compile(
        "shape R\n  id: string\ncomponent A\n  resource rs = rs() as shape R[]\n  view\n    text rs.indexOf(R(id=\"a\")).toString()\n",
    )
    .unwrap_err();
    assert!(
        e.message
            .contains("`indexOf` finds a string, number or bool in a list, given `R`"),
        "{e}"
    );
    contract::compile("component A\n  state s = \"a,b\"\n  view\n    text (s.split(\",\").indexOf(\"b\") + s.indexOf(\"b\")).toString()\n").unwrap();
}

/// `n` rows from `rows(n)`, and a string of `n` bytes from `long(n)`.
struct Many;

impl DataSource for Many {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        let n = args.first().and_then(Value::as_number).unwrap_or(0.0) as usize;
        match source {
            "rows" => Ok(Value::list(
                (0..n)
                    .map(|i| Value::record(vec![Value::str(&i.to_string()), Value::str("t")]))
                    .collect(),
            )),
            "long" => Ok(Value::str(&"a".repeat(n))),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

/// LLP 1088 §9.1 in LLP 1090 D3's terms: `concat`, `slice` and `split` take
/// a list step for each item they keep, before they build, and `includes`
/// and `indexOf` one for each item they scan up to the match, on the
/// evaluation's one budget; a
/// list they build is bounded as `List` is (conformance runs the same on the
/// JS target, `host/web-js/conformance/budget.contract`).
#[test]
fn concat_slice_and_includes_take_list_steps_and_build_bounded_lists() {
    let src = "shape Row\n  id: string\n  title: string\ncomponent A\n  state n = 0\n  state c = 0\n  resource rs = rows(n) as shape Row[]\n  resource s = long(c) as shape string\n  derive both = rs.concat(rs).length\n  derive kept = rs.slice(1).length\n  derive found = rs.map(r => r.id).includes(\"none\")\n  derive big = c > 0 ? rs.map(r => s).concat(rs.map(r => s)).length : 0\n  action go(k: number, b: number)\n    n = k\n    c = b\n  view\n    text `${both} ${kept} ${found} ${big}` testId=\"t\"\n";
    let mut r = Runner::boot(
        contract::compile(src).unwrap(),
        Many,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    // 32,768 rows: `concat` keeps 65,536 items, at the bound; `includes`
    // scans them after `map`'s 32,768 steps, 65,536 in all.
    r.act("go", vec![Value::Number(32768.0), Value::Number(0.0)])
        .unwrap();
    assert_eq!(shown(&r, "t"), "65536 32767 false 0");
    let trap = |e: &RunnerError| format!("{e:?}");
    let e = r
        .act("go", vec![Value::Number(32769.0), Value::Number(0.0)])
        .unwrap_err();
    assert!(trap(&e).contains("IterationLimit"), "{e:?}");
    // 1,024 strings of 32 KiB twice: 64 MiB of bytes is the bound; one
    // byte a string more is past it, a `ValueTooLarge` at the `concat`.
    r.act("go", vec![Value::Number(1024.0), Value::Number(32768.0)])
        .unwrap();
    let e = r
        .act("go", vec![Value::Number(1024.0), Value::Number(32769.0)])
        .unwrap_err();
    assert!(trap(&e).contains("ValueTooLarge"), "{e:?}");
}

/// `indexOf` steps as `includes` does, and `split` a step a piece, counted
/// before the pieces are built: 65,536 pieces of `long(n)`'s text are the
/// bound, one more past it.
#[test]
fn index_of_and_split_take_list_steps() {
    let src = "shape Row\n  id: string\n  title: string\ncomponent A\n  state n = 0\n  state c = 0\n  resource rs = rows(n) as shape Row[]\n  resource s = long(c) as shape string\n  derive at = rs.map(r => r.id).indexOf(\"none\")\n  derive pieces = s.split(\"\").length\n  derive words = s.split(\"a\").length\n  action go(k: number, b: number)\n    n = k\n    c = b\n  view\n    text `${at} ${pieces} ${words}` testId=\"t\"\n";
    let mut r = Runner::boot(
        contract::compile(src).unwrap(),
        Many,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.act("go", vec![Value::Number(32768.0), Value::Number(65535.0)])
        .unwrap();
    assert_eq!(shown(&r, "t"), "-1 65535 65536");
    let trap = |e: &RunnerError| format!("{e:?}");
    for (k, b) in [(32769.0, 0.0), (0.0, 65536.0)] {
        let e = r
            .act("go", vec![Value::Number(k), Value::Number(b)])
            .unwrap_err();
        assert!(trap(&e).contains("IterationLimit"), "{k} {b}: {e:?}");
    }
}
