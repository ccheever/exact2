//! A virtualized list's window changes lay out inside the list: every report
//! the host sends (rows built, measured, released; rows inserted and removed)
//! leaves a layout bit-equal to a fresh one from the columns (LLP 1044 §4.5,
//! `kernel/tests/it/layout_equality.rs`'s rule). The shape is List Bench's:
//! a header over a `flex=1` list whose rows are nested flex boxes, a
//! horizontal swipe scroller and text.
use exact_kernel::{Kernel, MonospaceMeasurer, Offer};
use exact_plan::Value;
use exact_runner::{CollectionFeedback, DataError, DataSource, RowMeasurement, Runner};
use std::collections::BTreeSet;
use std::time::Instant;

const SOURCE: &str = r##"shape Message
  id: string
  title: string
  body: string
  attachment: bool
component App
  state saved = ""
  state revision = 0
  resource rows = rows(revision) as shape list<Message>
  action save(id: string) writes saved
    saved = id
  action bump writes revision
    revision = revision + 1
  view
    column testId="main" width="100%" height="100%" overflow="hidden"
      column flex-shrink=0 padding=12 gap=12
        row align-items="center" gap=12
          text "Messages" font-size=17
          box flex=1
          text `saved ${saved}` font-size=17
      list testId="messages" virtualized=true estimated-item-height=130 flex=1 min-height=0 width="100%" overflow-x="hidden"
        each m in rows key=m.id
          Row(m=m, saved=saved == m.id, save=save)
component Row
  props
    m: Message
    saved: bool
    save: action
  view
    column width="100%" box-sizing="border-box" padding-left=16 padding-right=16
      scroll width="100%" overflow-x="scroll" overflow-y="hidden"
        row width="100%"
          column width="100%" flex-shrink=0 background-color="#ffffff"
            row width="100%" align-items="center"
              column flex=1 min-width=0 padding-left=16 padding-right=16 padding-top=21 padding-bottom=18 gap=10
                row align-items="center" gap=8
                  row align-items="center" gap=12 flex=1 min-width=0
                    box width=28 height=24
                    text m.title font-size=17 line-height="22px"
                  button press=save(m.id) testId=`save-${m.id}` min-width=44 min-height=44
                text m.body font-size=17 line-height="22px" white-space="pre-wrap"
                when saved
                  text "Saved for later" font-size=12 line-height="16px"
                when m.attachment
                  row align-self="flex-start" padding=8
                    text "Weekend itinerary.pdf" font-size=12 line-height="14px"
          button width=90 flex-shrink=0
"##;

const COUNT: usize = 10_000;
const BODIES: [&str; 4] = [
    "Are we still meeting for coffee?",
    "Yes! I found a place near the park. We can walk over afterward if the weather holds.",
    "Things to bring:\nCamera\nA warm jacket\nSomething for the picnic",
    "Sounds good. See you there",
];

/// Ten thousand messages; the test removes some to insert and remove rows.
#[derive(Default)]
struct Messages {
    removed: BTreeSet<usize>,
}
impl DataSource for Messages {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::list(
            (0..COUNT)
                .filter(|i| !self.removed.contains(i))
                .map(|i| {
                    Value::record(vec![
                        Value::str(&format!("m{i}")),
                        Value::str(&format!("Message {}", i + 1)),
                        Value::str(BODIES[i % 4]),
                        Value::Bool(i % 5 == 0),
                    ])
                })
                .collect(),
        ))
    }
}

const WIDTH: f32 = 402.0;
const HEIGHT: f32 = 874.0;

fn boot() -> Runner<Messages> {
    let plan = contract::compile(SOURCE).unwrap();
    let mut r = Runner::boot(
        plan,
        Messages::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    layout(&mut r);
    r
}

fn layout(r: &mut Runner<Messages>) {
    let root = r.roots()[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(WIDTH, HEIGHT))
        .unwrap();
}

/// What one host report cost.
#[derive(Default, Clone, Copy)]
struct Cost {
    reports: usize,
    feedback_ms: f64,
    layout_ms: f64,
}

/// Report the list's measured rows at `top` until the window settles, laying
/// out after each report as a host does.
fn settle(r: &mut Runner<Messages>, sequence: &mut u64, top: f64, check: bool) -> Cost {
    let mut cost = Cost::default();
    for _ in 0..16 {
        let snapshot = r.collections().pop().unwrap();
        let port = r.kernel().node(snapshot.view).unwrap().frame;
        let top = top.clamp(0.0, (snapshot.total_extent - port.height as f64).max(0.0));
        let measurements = snapshot
            .rows
            .iter()
            .map(|row| RowMeasurement {
                view: row.view,
                epoch: row.epoch,
                height: r.kernel().node(row.view).unwrap().frame.height as f64,
            })
            .collect();
        *sequence += 1;
        let facts = CollectionFeedback {
            view: snapshot.view,
            revision: snapshot.revision,
            scroll_sequence: *sequence,
            scroll_top: top,
            port_width: port.width as f64,
            port_height: port.height as f64,
            row_width: port.width as f64,
            measurements,
            focus_view: None,
            interaction_view: None,
        };
        let at = Instant::now();
        let advanced = r.collection_feedback(facts).unwrap();
        cost.feedback_ms += at.elapsed().as_secs_f64() * 1000.0;
        if advanced.receipts.is_empty() {
            return cost;
        }
        cost.reports += 1;
        let at = Instant::now();
        layout(r);
        cost.layout_ms += at.elapsed().as_secs_f64() * 1000.0;
        if check {
            equal_fresh(r);
        }
    }
    panic!("the window did not settle");
}

/// Every live frame and scroll extent equals a fresh layout's, bit for bit.
fn equal_fresh(r: &Runner<Messages>) {
    let k = r.kernel();
    let mut fresh = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    fresh
        .compute_layout(r.roots()[0], Offer::definite(WIDTH, HEIGHT))
        .unwrap();
    let (a, b) = (k.arena(), fresh.arena());
    for slot in a.iter_live() {
        assert!(
            a.frame(slot).bits_eq(b.frame(slot)),
            "frame of {}: {:?} != {:?}",
            a.local_id(slot),
            a.frame(slot),
            b.frame(slot)
        );
        let bits = |(w, h): (f32, f32)| (w.to_bits(), h.to_bits());
        assert_eq!(
            bits(a.content(slot)),
            bits(b.content(slot)),
            "overflow of {}",
            a.local_id(slot)
        );
    }
}

fn save(r: &mut Runner<Messages>, id: &str) {
    let key = r
        .kernel()
        .find_first_by_test_id(&format!("save-{id}"))
        .unwrap_or_else(|| panic!("save-{id} is not mounted"));
    let view = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(view, exact_runner::Event::Press).unwrap();
    layout(r);
}

fn mounted(r: &Runner<Messages>) -> Vec<usize> {
    r.collections()[0]
        .rows
        .iter()
        .map(|row| row.index)
        .collect()
}

#[test]
fn window_changes_equal_a_fresh_layout() {
    let mut r = boot();
    let mut sequence = 0;
    // Build the first window, then scroll through it, as a fling does.
    for step in 0..24 {
        settle(&mut r, &mut sequence, step as f64 * 240.0, true);
    }
    // A mounted row's height changes from inside it.
    let row = mounted(&r)[3];
    save(&mut r, &format!("m{row}"));
    equal_fresh(&r);
    settle(&mut r, &mut sequence, 23.0 * 240.0, true);
    // Rows around the window are removed, then come back.
    let at = mounted(&r)[2];
    r.data().removed.extend([at, at + 1, at + 7, at - 30]);
    r.act("bump", vec![]).unwrap();
    layout(&mut r);
    equal_fresh(&r);
    settle(&mut r, &mut sequence, 23.0 * 240.0, true);
    r.data().removed.clear();
    r.act("bump", vec![]).unwrap();
    layout(&mut r);
    equal_fresh(&r);
    settle(&mut r, &mut sequence, 23.0 * 240.0, true);
    // A jump far down, then back up.
    settle(&mut r, &mut sequence, 600_000.0, true);
    settle(&mut r, &mut sequence, 1_000.0, true);
}
