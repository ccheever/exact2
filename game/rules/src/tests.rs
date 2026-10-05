use super::*;

fn program(src: &str) -> Program {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e:?}"));
    Program::new(&plan.encode()).unwrap()
}

const RALLY: &str = r#"
component Rally
  state hits = 0
  state phase = "ready"
  derive long = hits >= 3
  action serve
    if phase == "ready"
      phase = "rally"
      count(1)
  action hit
    if phase == "rally"
      count(hits + 1)
  action count(n: number)
    hits = n
  view
    column
      button press=serve testId="serve"
        text "serve"
      button press=hit testId="hit"
        text "hit"
"#;

#[test]
fn events_run_and_derives_follow() {
    let p = program(RALLY);
    assert_eq!(p.events(), ["serve", "hit"]);
    let mut r = p.boot().unwrap();
    assert_eq!(r.text("phase"), "ready");
    assert!(!r.flag("long"));
    p.act(&mut r, "hit").unwrap();
    assert_eq!(r.num("hits"), 0.0, "hit before the serve is guarded");
    p.act(&mut r, "serve").unwrap();
    p.act(&mut r, "hit").unwrap();
    p.act(&mut r, "hit").unwrap();
    assert_eq!(r.num("hits"), 3.0);
    assert!(r.flag("long"));
}

#[test]
fn only_declared_events_run() {
    let p = program(RALLY);
    let mut r = p.boot().unwrap();
    let before = r.slots.clone();
    let e = p.act(&mut r, "count").unwrap_err();
    assert!(e.contains("not an event"), "{e}");
    assert_eq!(r.slots, before);
}

#[test]
fn a_new_program_adopts_slots_by_name() {
    let old = program(RALLY);
    let mut r = old.boot().unwrap();
    old.act(&mut r, "serve").unwrap();
    old.act(&mut r, "hit").unwrap();
    // The edit: `long` at two hits, `hits` kept, `phase` now a number, a new slot.
    let new = program(
        &RALLY
            .replace("hits >= 3", "hits >= 2")
            .replace(
                "state phase = \"ready\"",
                "state phase = 0\n  state lets = 0",
            )
            .replace("phase == \"ready\"", "phase == 0")
            .replace("phase = \"rally\"", "phase = 1")
            .replace("phase == \"rally\"", "phase == 1"),
    );
    let (r, adopted) = new.adopt(&r).unwrap();
    assert_eq!(r.num("hits"), 2.0);
    assert!(r.flag("long"), "the new derive reads the carried slot");
    assert_eq!(adopted.reset, ["phase"]);
    assert_eq!(adopted.added, ["lets"]);
    assert!(adopted.dropped.is_empty());
}

#[test]
fn a_program_that_asks_a_host_is_refused() {
    let plan = contract::compile(
        "component A\n  state n = 0\n  task tick when n < 3\n  action tick\n    n = n + 1\n  view\n    text \"a\"\n",
    );
    if let Ok(plan) = plan {
        let e = Program::new(&plan.encode()).err().expect("refused");
        assert!(e.contains("tasks"), "{e}");
    }
}
