//! Unrelated input must not revisit immutable rows; every relevant input still invalidates.
use exact_kernel::{Kernel, PropValue};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Event, Runner};

struct Rows;
impl DataSource for Rows {
    fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
        let revision = args[0].as_number().unwrap();
        Ok(Value::list(
            (0..1000)
                .map(|i| {
                    Value::record(vec![
                        Value::Number(i as f64),
                        Value::str(&format!(
                            "row {i} revision {}",
                            if i == 999 { revision } else { 0.0 }
                        )),
                    ])
                })
                .collect(),
        ))
    }
}
const APP: &str = r#"
shape Row
  id: number
  label: string
component App
  state draft = ""
  state revision = 0
  state suffix = "a"
  resource rows = rows(revision) as shape list<Row>
  action edit(value)
    draft = value
  action revise
    revision = revision + 1
  action rename
    suffix = "b"
  view
    column
      input value=draft change=edit testId="input"
      each r in rows key=r.id
        text `${r.label} ${suffix}` testId=`row-${r.id}`
"#;
fn boot(source: &str) -> Runner<Rows> {
    Runner::boot(
        contract::compile(source).unwrap(),
        Rows,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}
fn text(r: &Runner<Rows>, id: &str) -> String {
    r.kernel()
        .node_by_key(r.kernel().find_by_test_id(id)[0])
        .unwrap()
        .props
        .iter()
        .find_map(|(p, v)| match v {
            PropValue::Str(s) if p.name() == "text" => Some(s.clone()),
            _ => None,
        })
        .unwrap()
}
#[test]
fn unrelated_input_skips_all_row_keying_but_global_and_resource_changes_do_not() {
    let mut r = boot(APP);
    assert_eq!(r.last_instance_work().rows_keyed, 1000);
    let key = r.kernel().find_by_test_id("row-999")[0];
    r.act("edit", vec![Value::str("typing")]).unwrap();
    assert_eq!(r.last_instance_work().rows_keyed, 0);
    assert_eq!(r.last_instance_work().regions_skipped, 1);
    assert!(r.last_instance_work().nodes_visited < 10);
    assert_eq!(r.kernel().find_by_test_id("row-999"), [key]);
    r.act("rename", vec![]).unwrap();
    assert_eq!(text(&r, "row-999"), "row 999 revision 0 b");
    // Every row body reads the suffix, so every row is revisited; the keys
    // read nothing that changed, so none is evaluated again.
    assert_eq!(r.last_instance_work().rows_keyed, 0);
    assert_eq!(r.last_instance_work().rows_reused, 0);
    r.act("revise", vec![]).unwrap();
    assert_eq!(text(&r, "row-999"), "row 999 revision 1 b");
    assert_eq!(r.kernel().find_by_test_id("row-999"), [key]);
}
#[test]
fn row_owned_state_is_never_hidden_by_the_region_cache() {
    let app = APP.replace(
        "        text `${r.label} ${suffix}` testId=`row-${r.id}`",
        "        Counter(id=r.id)",
    ) + r#"
component Counter
  props
    id: number
  state n = 0
  action increment
    n = n + 1
  view
    button press=increment testId=`button-${id}`
      text `${n}` testId=`row-${id}`
"#;
    let mut r = boot(&app);
    let id = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("button-500")[0])
        .unwrap()
        .id;
    r.dispatch(id, Event::Press).unwrap();
    assert_eq!(text(&r, "row-500"), "1");
    assert_eq!(text(&r, "row-501"), "0");
    assert_eq!(r.last_instance_work().regions_skipped, 0);
}

#[test]
fn clock_and_inactive_branch_dependencies_are_not_lost() {
    let source=APP.replace("  action rename", "  task producer mount\n    every(10, revise)\n  action rename")
        .replace("        text `${r.label} ${suffix}` testId=`row-${r.id}`",
        "        when revision == 0\n          text `${now()}` testId=`row-${r.id}`\n        else\n          text suffix testId=`row-${r.id}`");
    let mut r = boot(&source);
    assert_eq!(text(&r, "row-0"), "0");
    r.advance(10.0).unwrap();
    assert_eq!(text(&r, "row-0"), "a");
    r.act("rename", vec![]).unwrap();
    assert_eq!(text(&r, "row-0"), "b");
}

#[test]
fn clock_changes_in_an_otherwise_unchanged_list_update_the_rows() {
    let source = APP
        .replace(
            "  action rename",
            "  task producer mount\n    every(10, rename)\n  action rename",
        )
        .replace("`${r.label} ${suffix}`", "`${now()}`");
    let mut r = boot(&source);
    r.advance(10.0).unwrap();
    assert_eq!(text(&r, "row-500"), "10");
    r.advance(20.0).unwrap();
    assert_eq!(text(&r, "row-500"), "20");
}

#[test]
fn changed_list_only_revisits_rows_whose_values_or_global_inputs_changed() {
    let mut r = boot(APP);
    r.act("revise", vec![]).unwrap();
    assert_eq!(r.last_instance_work().rows_keyed, 1000);
    assert_eq!(r.last_instance_work().rows_reused, 999);
    assert!(r.last_instance_work().nodes_visited < 10);
    assert_eq!(text(&r, "row-999"), "row 999 revision 1 a");
    r.act("rename", vec![]).unwrap();
    assert_eq!(r.last_instance_work().rows_reused, 0);
    assert_eq!(text(&r, "row-0"), "row 0 revision 0 b");
}

#[test]
fn pending_mutation_flags_invalidate_rows_without_changing_their_data() {
    use exact_runner::{Answer, Outcome, Request, Store};
    struct PendingRows;
    impl DataSource for PendingRows {
        fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
            Rows.query(source, args)
        }
        fn answer(
            &mut self,
            _: &mut Store,
            source: &str,
            args: &[Value],
        ) -> Result<Answer, DataError> {
            if source == "submit" {
                Ok(Answer::Later(Request::get("https://fixture.invalid/")))
            } else {
                self.query(source, args).map(Answer::Now)
            }
        }
        fn parse(
            &mut self,
            _: &mut Store,
            _: &str,
            _: &[Value],
            _: Outcome,
        ) -> Result<Answer, DataError> {
            Ok(Answer::Now(Value::Number(1.0)))
        }
    }
    let source=APP.replace("  state draft", "  mutation result as shape number\n  action submit\n    send result = submit()\n  state draft")
        .replace("`${r.label} ${suffix}`", "`${pending(result)}`");
    let mut r = Runner::boot(
        contract::compile(&source).unwrap(),
        PendingRows,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let text = |r: &Runner<PendingRows>| {
        r.kernel()
            .node_by_key(r.kernel().find_by_test_id("row-500")[0])
            .unwrap()
            .props
            .iter()
            .find_map(|(p, v)| match v {
                PropValue::Str(s) if p.name() == "text" => Some(s.clone()),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(text(&r), "false");
    r.act("submit", vec![]).unwrap();
    assert_eq!(text(&r), "true");
    let request = r.take_requests().pop().unwrap();
    r.fulfill(
        request.ticket,
        Outcome::Failed {
            kind: exact_runner::FailureKind::Network,
            message: "synthetic".into(),
        },
    )
    .unwrap();
    assert_eq!(text(&r), "false");
}

#[test]
fn signed_zero_changes_are_observable_inside_an_unchanged_row_key() {
    struct Zeros;
    impl DataSource for Zeros {
        fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
            Ok(Value::list(vec![Value::record(vec![
                Value::Number(1.0),
                Value::str("zero"),
                Value::Number(if args[0].as_number() == Some(0.0) {
                    0.0
                } else {
                    -0.0
                }),
            ])]))
        }
    }
    let source = APP
        .replace("  label: string", "  label: string\n  n: number")
        .replace("`${r.label} ${suffix}`", "`${1 / r.n > 0}`");
    let mut r = Runner::boot(
        contract::compile(&source).unwrap(),
        Zeros,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let text = |r: &Runner<Zeros>| {
        r.kernel()
            .node_by_key(r.kernel().find_by_test_id("row-1")[0])
            .unwrap()
            .props
            .iter()
            .find_map(|(p, v)| match v {
                PropValue::Str(s) if p.name() == "text" => Some(s.clone()),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(text(&r), "true");
    r.act("revise", vec![]).unwrap();
    assert_eq!(text(&r), "false");
    assert_eq!(r.last_instance_work().rows_reused, 0);
}

/// `perf <target>` (LLP 1079 D1–D2): site totals across lifetimes, the
/// hygiene number, and receipt membership split by who changed the node.
#[test]
fn perf_counts_each_sites_work_across_its_instances_lifetimes() {
    struct Three;
    impl DataSource for Three {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(Value::list(
                (0..3).map(|i| Value::Number(i as f64)).collect(),
            ))
        }
    }
    const PERF: &str = r##"
component App
  resource ids = three() as shape list<number>
  state hot = -1
  state tick = 0
  state tint = "#000000"
  action hover(id: number)
    hot = id
  action step
    tick = tick + 1
  action recolor
    tint = "#ff0000"
  view
    column testId="list" color=tint
      each r in ids key=r
        row testId=`row-${r}`
          when hot == r
            text `hot ${r}` testId=`hot-${r}`
          else
            text `cold ${r}` testId=`cold-${r}`
          text `${r} ${tick > 100}` font-size=12 testId=`tick-${r}`
"##;
    let mut r = Runner::boot(
        contract::compile(PERF).unwrap(),
        Three,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let ask = |r: &Runner<Three>, target: &str| -> serde_json::Value {
        serde_json::from_str(&exact_runner::agent::handle(
            r,
            &serde_json::json!({"op":"perf", "target":target}).to_string(),
        ))
        .unwrap()
    };
    // Production trust: the host never measures, and the read says so.
    assert!(ask(&r, "list")["error"]
        .as_str()
        .unwrap()
        .contains("production trust"));
    r.measure(true, false);
    let site = |reply: &serde_json::Value, test_id: &str, r: &Runner<Three>| {
        let key = r.kernel().find_by_test_id(test_id)[0];
        let view = r.kernel().node_by_key(key).unwrap().id;
        let site = r.site_of(view).unwrap().0 .0 as u64;
        reply["sites"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["site"] == site)
            .unwrap()
            .clone()
    };
    let before = ask(&r, "list");
    let seq = before["seq"].as_u64().unwrap();
    // Live instances seed `created` when measuring begins.
    assert_eq!(site(&before, "cold-1", &r)["created"], 3);
    assert_eq!(site(&before, "cold-1", &r)["live"], 3);
    assert!(before.get("moved").is_none() && site(&before, "row-1", &r).get("moved").is_none());

    // Hovering row 1 retires its cold text and realizes a hot one; leaving
    // it does the reverse: churn the retired count keeps after the views go.
    r.act("hover", vec![Value::Number(1.0)]).unwrap();
    r.act("hover", vec![Value::Number(-1.0)]).unwrap();
    let after = ask(&r, "list");
    assert!(after["seq"].as_u64().unwrap() >= seq + 2);
    let cold = site(&after, "cold-1", &r);
    assert_eq!(
        (cold["created"].as_u64(), cold["retired"].as_u64()),
        (Some(4), Some(1))
    );
    assert_eq!(cold["live"], 3);
    // The hot site has no live instance now, and still shows its totals.
    let hot = after["sites"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["created"] == 1 && s["retired"] == 1)
        .expect("the hot text's site, statically under the list")
        .clone();
    assert_eq!(hot["instances"], 0);
    // A read of every root keeps it too: its churn is not lost to a trace.
    let every: serde_json::Value =
        serde_json::from_str(&exact_runner::agent::handle(&r, r#"{"op":"perf"}"#)).unwrap();
    assert!(every["sites"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["site"] == hot["site"]));

    // Every tick re-evaluates the three tick texts, and each comes out as
    // before: the hygiene number, with nothing authored.
    let ticked = site(&after, "tick-0", &r);
    for _ in 0..5 {
        r.act("step", vec![]).unwrap();
    }
    let later = ask(&r, "list");
    let tick = site(&later, "tick-0", &r);
    let delta = |k: &str| tick[k].as_u64().unwrap() - ticked[k].as_u64().unwrap();
    assert_eq!((delta("evaluated"), delta("unchanged")), (15, 15));
    assert_eq!(delta("authored"), 0);

    // A color on the list is inherited by every text under it: their own
    // ops did not touch them.
    r.act("recolor", vec![]).unwrap();
    let tinted = ask(&r, "list");
    let text = site(&tinted, "tick-0", &r);
    assert_eq!(
        text["inherited"].as_u64().unwrap() - tick["inherited"].as_u64().unwrap(),
        3
    );
    assert_eq!(text["authored"], tick["authored"]);
    assert_eq!(
        site(&tinted, "list", &r)["authored"].as_u64().unwrap()
            - site(&later, "list", &r)["authored"].as_u64().unwrap(),
        1
    );
    assert_eq!(tinted["walked"], 10);
    assert_eq!(tinted["truncated"], false);
}
