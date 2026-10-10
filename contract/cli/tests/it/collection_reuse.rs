//! LLP 1078: a retiring list row rebound to the item the window needs shows
//! exactly what a row built for that item shows, under its new key, epoch and
//! place, and every view of it is renewed for motion and the host.
use exact_kernel::{Kernel, NodeKey, PropId, ViewId};
use exact_plan::Value;
use exact_runner::{CollectionFeedback, DataError, DataSource, Runner};

struct Rows(usize);
impl DataSource for Rows {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::list(
            (0..self.0)
                .map(|i| {
                    Value::record(vec![
                        Value::Number(i as f64),
                        Value::str(&format!("title {i}")),
                        Value::Bool(i % 2 == 0),
                        Value::list((0..i % 3).map(|t| Value::str(&format!("tag{t}"))).collect()),
                    ])
                })
                .collect(),
        ))
    }
}

/// Rows with fields, a `when` on one, a nested `each`, a row slot seeded from
/// the item, a transition, an animation, a picture and a scroller.
const SOURCE: &str = r##"shape Item
  id: number
  title: string
  even: bool
  tags: list<string>
keyframes fade
  from opacity=0
  to opacity=1
component App
  resource rows = rows() as shape list<Item>
  view
    list virtualized=true height=320 estimated-item-height=64 testId="list"
      each x in rows key=x.id
        Row(x=x)
component Row
  props
    x: Item
  state open = x.even
  action toggle
    open = not open
  view
    column testId=`row-${x.id}` height=64 press=toggle transition="opacity 200ms" opacity=(x.even ? 1 : 0.5)
      text x.title animation="fade 400ms"
      when x.even
        text "even" color="#ff0000"
      else
        image `assets/${x.id}.png` width=10 height=10
      each t in x.tags key=t
        text t
      text (open ? "open" : "closed")
      scroll overflow-x="scroll" overflow-y="hidden" width=100 height=20
        row width=300
          text "wide"
"##;

fn boot(source: &str, reuse: bool) -> Runner<Rows> {
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        Rows(400),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.set_row_reuse(reuse);
    r
}

fn facts(r: &Runner<Rows>, top: f64) -> CollectionFeedback {
    let c = &r.collections()[0];
    CollectionFeedback {
        view: c.view,
        revision: c.revision,
        scroll_sequence: c.scroll_sequence + 1,
        offset: top,
        port_cross: 640.,
        port_main: 320.,
        cross: 640.,
        measurements: vec![],
        focus_view: None,
        interaction_view: None,
    }
}

/// A node and its subtree as a reader sees it: type, style, props, children;
/// never a view id.
fn dump(k: &Kernel, id: ViewId, out: &mut String) {
    let n = k.node(id).unwrap();
    out.push_str(&format!("{:?} {:?} {:?} [", n.node_type, n.style, n.props));
    for c in n.children() {
        dump(k, c, out);
    }
    out.push(']');
}

/// One report at `top`: its receipt and how many rows it rebound.
fn report(r: &mut Runner<Rows>, top: f64) -> (Vec<exact_kernel::CommitReceipt>, usize) {
    let result = r.collection_feedback(facts(r, top)).unwrap();
    assert!(result.error.is_none(), "{:?}", result.error);
    let rebound = r.last_instance_work().rows_rebound;
    (
        result.receipts.into_iter().map(|t| t.receipt).collect(),
        rebound,
    )
}

#[test]
fn a_rebound_row_shows_what_a_built_one_does() {
    let (mut fresh, mut reused) = (boot(SOURCE, false), boot(SOURCE, true));
    let mut rebound = 0;
    for top in [
        0.,
        200.,
        640.,
        1300.,
        5000.,
        4800.,
        300.,
        0.,
        20000.,
        64. * 399.,
    ] {
        report(&mut fresh, top);
        rebound += report(&mut reused, top).1;
        let (a, b) = (&fresh.collections()[0], &reused.collections()[0]);
        // Every row of the window, as a built one: its subtree and its place.
        for row in &a.rows {
            let same = b
                .rows
                .iter()
                .find(|r| r.index == row.index)
                .unwrap_or_else(|| panic!("row {} is not mounted at {top}", row.index));
            assert_eq!(row.start, same.start, "row {} at {top}", row.index);
            let (mut built, mut bound) = (String::new(), String::new());
            dump(fresh.kernel(), row.view, &mut built);
            dump(reused.kernel(), same.view, &mut bound);
            assert_eq!(built, bound, "row {} at {top}", row.index);
        }
        // What it holds past the window for the rows it needs next: few,
        // and whole rows of their own items.
        let held: Vec<_> = b
            .rows
            .iter()
            .filter(|r| a.rows.iter().all(|w| w.index != r.index))
            .collect();
        assert!(held.len() <= 4, "{} rows held at {top}", held.len());
        // Nothing a rebound row showed is left in the kernel's other indexes.
        for row in &b.rows {
            let key = reused
                .kernel()
                .node(row.root)
                .unwrap()
                .props
                .str(PropId::TestId);
            assert_eq!(key, Some(format!("row-{}", row.index).as_str()));
            assert_eq!(
                reused
                    .kernel()
                    .find_by_test_id(&format!("row-{}", row.index))
                    .len(),
                1
            );
        }
    }
    assert!(rebound > 0, "the scroll rebound no row");
    // The kernel holds the window's rows, the held ones and nothing else of
    // a row: no node of an item a rebind replaced.
    let (a, b) = (&fresh.collections()[0], &reused.collections()[0]);
    let nodes = |k: &Kernel, rows: &[exact_runner::CollectionRow]| -> usize {
        fn count(k: &Kernel, id: ViewId) -> usize {
            1 + k
                .node(id)
                .unwrap()
                .children()
                .into_iter()
                .map(|c| count(k, c))
                .sum::<usize>()
        }
        let list = k.node(rows[0].view).unwrap().parent.unwrap();
        k.live_count() - count(k, list)
    };
    assert_eq!(
        nodes(fresh.kernel(), &a.rows),
        nodes(reused.kernel(), &b.rows)
    );
}

#[test]
fn a_rebound_row_is_renewed_and_built_from_nothing() {
    let mut r = boot(SOURCE, true);
    report(&mut r, 0.);
    let before: Vec<(ViewId, u64)> = r.collections()[0]
        .rows
        .iter()
        .map(|row| (row.view, row.epoch))
        .collect();
    let (receipts, rebound) = report(&mut r, 2000.);
    assert!(rebound > 0);
    let receipt = &receipts[0];
    // Every row mounted now that was mounted before was rebound: a new
    // epoch, a new key, every view of it renewed, no node created for it.
    let rows = &r.collections()[0].rows;
    let wrappers: Vec<NodeKey> = rows
        .iter()
        .filter(|row| before.iter().any(|(v, _)| *v == row.view))
        .map(|row| r.kernel().node(row.view).unwrap().key)
        .collect();
    assert_eq!(wrappers.len(), rebound);
    for row in rows
        .iter()
        .filter(|row| before.iter().any(|(v, _)| *v == row.view))
    {
        let (_, epoch) = before.iter().find(|(v, _)| *v == row.view).unwrap();
        assert_ne!(*epoch, row.epoch);
        let wrapper = r.kernel().node(row.view).unwrap();
        assert_eq!(
            wrapper.props.str(PropId::ListItemKey),
            Some(format!("n:{}", row.index).as_str())
        );
        assert!(receipt.renewed.contains(&wrapper.key));
        assert!(receipt
            .renewed
            .contains(&r.kernel().node(row.root).unwrap().key));
        assert!(!receipt.created.contains(&wrapper.key));
    }
    // Motion forgets each renewed node and hears it again as new: its
    // animation starts over, no transition runs from the old item's values.
    let sync = r.kernel().motion_sync(receipt);
    for key in &receipt.renewed {
        let node = exact_kernel::motion_node(*key);
        assert!(sync.removed.contains(&node));
        assert!(sync.transitions.iter().any(|(n, _)| *n == node));
        assert_eq!(
            sync.animations.iter().filter(|(n, _)| *n == node).count(),
            1
        );
    }
    // A slot seeded from the item is seeded from the new one.
    for row in rows {
        let root = r.kernel().node(row.root).unwrap();
        let open = root.children()[root.children().len() - 2];
        let text = r
            .kernel()
            .node(open)
            .unwrap()
            .props
            .str(PropId::Text)
            .map(str::to_owned);
        let expected = if row.index % 2 == 0 { "open" } else { "closed" };
        assert_eq!(text.as_deref(), Some(expected), "row {}", row.index);
    }
}

#[test]
fn a_measurement_for_the_old_item_does_not_reach_the_new_one() {
    let mut r = boot(SOURCE, true);
    report(&mut r, 0.);
    let old = r.collections()[0].rows[0].clone();
    report(&mut r, 3000.);
    let now = &r.collections()[0];
    let row = now
        .rows
        .iter()
        .find(|row| row.view == old.view)
        .expect("rebound");
    assert!(!row.measured);
    let mut f = facts(&r, 3000.);
    f.measurements.push(exact_runner::RowMeasurement {
        view: old.view,
        epoch: old.epoch,
        size: 999.,
    });
    r.collection_feedback(f).unwrap();
    let row = r.collections()[0]
        .rows
        .iter()
        .find(|row| row.view == old.view)
        .cloned()
        .unwrap();
    assert!(!row.measured);
    assert_ne!(row.size, 999.);
}

#[test]
fn a_pinned_row_keeps_its_item() {
    let mut r = boot(SOURCE, true);
    report(&mut r, 0.);
    let pinned = r.collections()[0].rows[1].clone();
    let mut f = facts(&r, 4000.);
    f.focus_view = Some(pinned.root);
    r.collection_feedback(f).unwrap();
    let row = r.collections()[0]
        .rows
        .iter()
        .find(|row| row.view == pinned.view)
        .cloned()
        .expect("a focused row stays");
    assert_eq!(row.index, pinned.index);
    assert_eq!(row.epoch, pinned.epoch);
}

#[test]
fn what_a_rebind_cannot_make_fresh_refuses_it() {
    for (name, source) in [
        (
            "an editor",
            SOURCE.replace(
                "      text (open ? \"open\" : \"closed\")",
                "      input value=x.title",
            ),
        ),
        (
            "a layout transition",
            SOURCE.replace(
                "height=64 press=toggle",
                "height=64 press=toggle -exact-layout-transition=\"all 200ms\"",
            ),
        ),
        (
            "slots below the row's",
            SOURCE.replace(
                "      each t in x.tags key=t\n        text t",
                "      each t in x.tags key=t\n        Tag(t=t)",
            ) + "component Tag\n  props\n    t: string\n  state on = false\n  view\n    text t\n",
        ),
    ] {
        let mut r = boot(&source, true);
        report(&mut r, 0.);
        let (_, rebound) = report(&mut r, 3000.);
        assert_eq!(rebound, 0, "{name}");
    }
    let mut r = boot(SOURCE, false);
    report(&mut r, 0.);
    let (receipts, rebound) = report(&mut r, 3000.);
    assert_eq!(rebound, 0, "off by default");
    assert!(receipts[0].renewed.is_empty());
}

/// Each mounted row's subtree, by item position.
fn rows(r: &Runner<Rows>) -> std::collections::BTreeMap<usize, String> {
    r.collections()[0]
        .rows
        .iter()
        .map(|row| {
            let mut out = String::new();
            dump(r.kernel(), row.view, &mut out);
            (row.index, out)
        })
        .collect()
}

#[test]
fn a_limited_report_rebinds_the_rows_it_would_keep() {
    let (mut fresh, mut reused) = (boot(SOURCE, false), boot(SOURCE, true));
    let mut rebound = 0;
    for (i, top) in [0., 300., 900., 1500., 2100., 2700., 2500., 1900.]
        .into_iter()
        .enumerate()
    {
        let fill = exact_runner::CollectionFill {
            velocity: if i < 6 { 6000. } else { -6000. },
            limit: Some(2),
            ..Default::default()
        };
        fresh
            .collection_feedback_filled(facts(&fresh, top), fill)
            .unwrap();
        reused
            .collection_feedback_filled(facts(&reused, top), fill)
            .unwrap();
        rebound += reused.last_instance_work().rows_rebound;
        let (a, b) = (rows(&fresh), rows(&reused));
        // The visible rows are the same rows; a row both mount is the same.
        for (index, row) in &b {
            if let Some(built) = a.get(index) {
                assert_eq!(built, row, "row {index} at {top}");
            }
        }
        let shown = |r: &Runner<Rows>| {
            r.collections()[0]
                .rows
                .iter()
                .filter(|row| row.start + row.size > top && row.start < top + 320.)
                .map(|row| row.index)
                .collect::<Vec<_>>()
        };
        assert_eq!(shown(&fresh), shown(&reused), "at {top}");
        assert!(b.len() <= a.len(), "at {top}");
    }
    assert!(rebound > 0);
}

/// Travel of two viewports between passes, under a lead (a host that waits
/// for its pass, LLP 1078): the rows behind are all two viewports past the
/// port when a pass sees them, and its first slice builds only its limit.
/// The rest wait for the slices after it, which rebind them: once the window
/// has grown to its lead, no row is built from nothing.
#[test]
fn a_slice_holds_the_rows_its_window_still_needs() {
    let (mut fresh, mut reused) = (boot(SOURCE, false), boot(SOURCE, true));
    let views = |r: &Runner<Rows>| -> std::collections::BTreeSet<ViewId> {
        r.collections()[0].rows.iter().map(|row| row.view).collect()
    };
    let (mut built, mut rebound, mut most) = (0, 0, 0);
    for (pass, top) in (0..12).map(|i| (i, 700. * i as f64)) {
        // The reader asks again while a slice leaves rows.
        for slice in 0..32 {
            let fill = exact_runner::CollectionFill {
                velocity: 1e6,
                limit: Some(2),
                ..Default::default()
            };
            let before = views(&reused);
            fresh
                .collection_feedback_filled(facts(&fresh, top), fill)
                .unwrap();
            reused
                .collection_feedback_filled(facts(&reused, top), fill)
                .unwrap();
            let bound = reused.last_instance_work().rows_rebound;
            let new = views(&reused).difference(&before).count();
            rebound += bound;
            // The first passes grow the window to its lead.
            if pass >= 3 {
                built += new;
            }
            let (a, b) = (rows(&fresh), rows(&reused));
            for (index, row) in &b {
                if let Some(row_built) = a.get(index) {
                    assert_eq!(row_built, row, "row {index} at {top}, slice {slice}");
                }
            }
            let shown = |r: &Runner<Rows>| {
                r.collections()[0]
                    .rows
                    .iter()
                    .filter(|row| row.start + row.size > top && row.start < top + 320.)
                    .map(|row| row.index)
                    .collect::<Vec<_>>()
            };
            assert_eq!(shown(&fresh), shown(&reused), "at {top}, slice {slice}");
            most = most.max(b.len());
            if !reused.collections()[0].pending && !fresh.collections()[0].pending {
                break;
            }
            assert!(slice < 31, "the slices at {top} never ended");
        }
    }
    assert!(rebound > 50, "{rebound}");
    assert_eq!(built, 0, "rows built from nothing under a lead");
    // A viewport before, the view, three after (64 px rows in 320), and
    // what one pass's travel leaves behind for the next.
    assert!(most <= 25 + 12, "{most} rows mounted");
}
