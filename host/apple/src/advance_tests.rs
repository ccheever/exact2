//! An advance whose commits change no node does no layout (`inert_layout`):
//! a `sent` reader answered at each MIDI message's time (LLP 1125 D6) with
//! nothing on screen reading it, as euxy's activity log is while the
//! sequencer plays. What layout keeps besides boxes still follows.
use super::*;
use crate::content_region::{ContentRegionRegistration, NativeProjectionLimits};
use exact_kernel::MonospaceMeasurer;
use exact_plan::Value;
use exact_runner::{DataError, Event};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const MIDI: &str = r#"shape Message
  output: string
  data: number[]
  timeStamp: number

shape MIDI
  state: string
  sent: Message[]

component A
  resource midi = exactMIDI() as shape MIDI
  state shown = false
  action send
    sendMIDI("", [144, 60, 100], at=performanceNow() + 100)
    sendMIDI("", [128, 60, 0], at=performanceNow() + 200)
  action show
    shown = true
  view
    column
      button press=send testId="send"
        text "Send"
      button press=show testId="show"
        text "Show"
      text (shown ? `${midi.sent.length} sent` : "") testId="log"
"#;

/// A timer that changes a field of every item, which no row shows, and
/// swaps the last two items, past the window: no node changes.
const LIST: &str = r#"shape Item
  id: number
  seen: number

component A
  state items = range(100).map(i => Item(id=i, seen=0))
  action see
    items = items.map(i => Item(id=(i.id >= 98 ? 197 - i.id : i.id), seen=i.seen + 1))
  task seer mount
    every(100, see)
  view
    column
      list virtualized=true height=160 width=240
        for item in items key=item.id
          text `${item.id}`
"#;

/// The same list as a native content region's content.
const REGION: &str = r#"shape Item
  id: number
  seen: number

component A
  state items = range(100).map(i => Item(id=i, seen=0))
  action see
    items = items.map(i => Item(id=i.id, seen=i.seen + 1))
  task seer mount
    every(100, see)
  view
    column width="100%" height="100%"
      view id="owner" width="100%" height=400 flex-shrink=0 overflow-x="hidden" overflow-y="hidden"
        list id="content" virtualized=true height=160 width=240
          for item in items key=item.id
            text `${item.id}`
        text "Preparing" id="pending"
"#;

fn boot(source: &str) -> Host<NoData> {
    let plan = contract::compile(source).unwrap().encode();
    let (host, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    host
}

fn press(host: &mut Host<NoData>, test_id: &str, now: f64) -> String {
    let k = host.runner.kernel();
    let id = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id;
    host.dispatch_at(id, Event::Press, now)
}

fn sent(host: &Host<NoData>) -> usize {
    let Some(Value::Record(f)) = host.runner.resource("midi") else {
        panic!()
    };
    let Value::List(items) = &f[1] else {
        panic!("{:?}", f[1])
    };
    items.len()
}

#[test]
fn an_answer_nothing_shows_skips_layout_and_one_shown_lays_out() {
    let mut host = boot(MIDI);
    press(&mut host, "send", 0.0);
    let layouts = host.layout_calls;
    for (now, count) in [(100.0, 1), (200.0, 2)] {
        let batch = host.advance(now);
        assert_eq!(sent(&host), count, "the reader was answered at {now}");
        assert!(!batch.contains("\"op\":\""), "nothing to present: {batch}");
    }
    assert_eq!(host.layout_calls, layouts, "and nothing laid out");
    // Shown, an answer lays out and carries the text it changed.
    press(&mut host, "show", 250.0);
    press(&mut host, "send", 300.0);
    let layouts = host.layout_calls;
    let batch = host.advance(400.0);
    assert!(batch.contains("3 sent"), "{batch}");
    assert!(host.layout_calls > layouts);
}

/// The list's metadata is published, which the presenter checks its scroll
/// and measurement feedback against, and its generation is the one the next
/// layout judges row moves from.
#[test]
fn an_unchanged_advance_still_publishes_a_list_it_changed() {
    let mut host = boot(LIST);
    let revision = |host: &Host<NoData>| host.runner.collections()[0].revision;
    let before = revision(&host);
    let generations = host.presence.generations.clone();
    let layouts = host.layout_calls;
    let batch = host.advance(100.0);
    assert!(revision(&host) > before, "the list's data and keys changed");
    assert_eq!(host.layout_calls, layouts, "and no node: {batch}");
    assert!(
        batch.contains(&format!("\"revision\":\"{}\"", revision(&host))),
        "its metadata is published: {batch}"
    );
    assert_ne!(host.presence.generations, generations);
    assert_eq!(host.presence.generations, host.runner.collection_data());
}

/// Boxes a failed layout queued are emitted by the next layout, so an
/// unchanged advance lays out while any wait.
#[test]
fn an_unchanged_advance_lays_out_while_boxes_wait() {
    let mut host = boot(LIST);
    let root = host.runner.roots()[0];
    let key = host.runner.kernel().node(root).unwrap().key;
    // As a layout that failed after laying the root out leaves it: queued,
    // its frame not yet the presenter's.
    host.pending_layout.insert(key);
    host.mirror.get_mut(&root).unwrap().frame = None;
    let layouts = host.layout_calls;
    let batch = host.advance(100.0);
    assert!(host.layout_calls > layouts, "a whole layout ran");
    assert!(host.pending_layout.is_empty());
    assert!(
        batch.contains(&format!("\"op\":\"frame\",\"id\":{root}")),
        "and emitted the root's frame: {batch}"
    );
}

/// A native content region's next snapshot of its lists is prepared only in
/// layout (`native_prepare_candidate`): there an unchanged commit lays out.
#[test]
fn an_unchanged_advance_in_a_native_region_lays_out() {
    let plan = contract::compile(REGION).unwrap().encode();
    let registration = ContentRegionRegistration {
        activate: None,
        owner: "owner",
        content: "content",
        pending: "pending",
    };
    let (mut host, _) = Host::boot_native_region(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
        registration,
        NativeProjectionLimits::default(),
    )
    .unwrap();
    assert!(host.native_mode());
    let layouts = host.layout_calls;
    let batch = host.advance(100.0);
    assert!(!batch.contains("\"error\":\""), "{batch}");
    assert!(host.layout_calls > layouts);
}

/// A layout that failed (a resize whose text measured nothing) is owed: the
/// next advance lays out once measurement recovers, though its commits
/// change no node.
#[test]
fn an_unchanged_advance_lays_out_after_a_failed_layout() {
    use exact_kernel::{TextMeasureRequest, TextMeasurer, TextMetrics};
    use std::{cell::Cell, rc::Rc};
    struct Fallible(Rc<Cell<bool>>);
    impl TextMeasurer for Fallible {
        fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
            if self.0.get() {
                TextMetrics {
                    width: f32::NAN,
                    height: 0.,
                    first_baseline: None,
                }
            } else {
                MonospaceMeasurer::default().measure(request)
            }
        }
    }
    let failing = Rc::new(Cell::new(false));
    let plan = contract::compile(MIDI).unwrap().encode();
    let (mut host, _) = Host::boot(
        &plan,
        NoData,
        Box::new(Fallible(failing.clone())),
        390.0,
        844.0,
    )
    .unwrap();
    press(&mut host, "send", 0.0);
    failing.set(true);
    let rejected = host.resize(500.0, 900.0);
    assert!(rejected.contains("InvalidTextMetrics"), "{rejected}");
    failing.set(false);
    let layouts = host.layout_calls;
    let batch = host.advance(100.0);
    assert_eq!(sent(&host), 1);
    assert!(host.layout_calls > layouts, "the owed layout ran");
    assert!(
        batch.contains("\"op\":\"frame\""),
        "and placed the boxes at the new size: {batch}"
    );
}

/// Answers `number(n)` with `n`, its first turn for 2 over its budget.
struct Busy(bool);
impl DataSource for Busy {
    fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
        if args[0] == Value::Number(2.) && std::mem::take(&mut self.0) {
            return Err(DataError::Overrun(
                "took 180.0 ms, over the 100 ms budget for one turn".into(),
            ));
        }
        Ok(args[0].clone())
    }
}

/// A resource a turn over its budget refused is asked again at 250 ms
/// (exact-runner's `overrun.rs`): the agent's `settle` is that re-ask's time,
/// so a frozen clock's `clock settle` goes there rather than stopping with the
/// resource pending.
#[test]
fn settle_is_a_reasks_time() {
    let plan = contract::compile(
        "component App\n  state n = 1\n  resource value = number(n) as shape number\n  action next\n    n = n + 1\n  view\n    text value.toString()\n",
    )
    .unwrap()
    .encode();
    let (mut host, _) = Host::boot(
        &plan,
        Busy(true),
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    host.runner_mut().act("next", vec![]).unwrap();
    assert_eq!(host.runner().resource("value"), Some(&Value::Number(1.)));
    assert_eq!(host.agent("{\"op\":\"settle\"}"), "{\"settle\":250}");
}
