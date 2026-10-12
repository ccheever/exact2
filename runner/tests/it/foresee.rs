//! What a press would mount is found without changing the runner.

use exact_kernel::{Kernel, Op, ViewId};
use exact_plan::Value;
use exact_runner::{agent, DataError, DataSource, Event, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("a foreseen press queried {name}")
    }
}

const SOURCE: &str = "component App
  state shown = none
  state open = false
  state n = 0
  action show(t: string)
    shown = some(t)
    open = true
  action bump
    n = n + 1
  view
    column
      button press=show(\"hello\") testId=\"a\"
        text \"A\"
      button press=bump testId=\"b\"
        text `${n}`
      text \"plain\" testId=\"c\"
      column testId=\"host\"
        match shown
          case some(t)
            column testId=\"detail\"
              text t testId=\"body\"
          case none
      row testId=\"bar\"
        when open
          text \"open\" testId=\"flag\"
";

fn boot() -> Runner<NoData> {
    let plan = contract::compile(SOURCE).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn view(r: &Runner<NoData>, test_id: &str) -> ViewId {
    let key = r.kernel().find_first_by_test_id(test_id).unwrap();
    r.kernel().node_by_key(key).unwrap().id
}

#[test]
fn a_press_that_opens_a_screen_is_foreseen_and_the_runner_is_as_it_was() {
    let mut r = boot();
    let (tree, state) = (agent::tree(&r), agent::state(&r));
    let (live, journal) = (r.kernel().live_count(), r.journal().count());

    let seen = r.foresee(view(&r, "a")).expect("the press mounts two arms");
    let mut parents: Vec<ViewId> = seen.mounts.iter().map(|m| m.parent).collect();
    parents.sort_unstable();
    let mut expected = vec![view(&r, "host"), view(&r, "bar")];
    expected.sort_unstable();
    assert_eq!(parents, expected);
    assert!(seen.mounts.iter().all(|m| m.roots.len() == 1));
    // The ops make the arms, with the press's own argument, and name no live view.
    assert!(format!("{:?}", seen.ops).contains("hello"));
    assert!(seen
        .ops
        .iter()
        .any(|op| matches!(op, Op::CreateView { .. })));
    for op in &seen.ops {
        assert!(r.kernel().node(op.target()).is_none(), "{op:?}");
    }
    let created = |id: &ViewId| {
        seen.ops
            .iter()
            .any(|op| matches!(op, Op::CreateView { id: made, .. } if made == id))
    };
    assert!(seen.mounts.iter().all(|m| m.roots.iter().all(created)));

    assert_eq!(agent::tree(&r), tree);
    assert_eq!(agent::state(&r), state);
    assert_eq!(r.kernel().live_count(), live);
    assert_eq!(r.journal().count(), journal);
    assert!(r.take_commands().is_empty());

    // The press itself is what it would have been: a twin never asked agrees.
    let mut twin = boot();
    r.dispatch(view(&r, "a"), Event::Press).unwrap();
    twin.dispatch(view(&twin, "a"), Event::Press).unwrap();
    assert_eq!(agent::tree(&r), agent::tree(&twin));
    assert!(agent::tree(&r).contains("hello"));
    // Shown now: the same press mounts nothing more.
    assert!(r.foresee(view(&r, "a")).is_none());
}

#[test]
fn a_press_that_mounts_nothing_or_has_no_handler_foresees_nothing() {
    let r = boot();
    assert!(r.foresee(view(&r, "b")).is_none());
    assert!(r.foresee(view(&r, "c")).is_none());
    assert!(r.foresee(ViewId::MAX).is_none());
}
