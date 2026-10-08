//! Real Host first-layout cut, stale completion and native artifact ownership.
use exact_apple::{content_region::ContentRegionRegistration, Host};
use exact_kernel::{MonospaceMeasurer, TextMeasureRequest, TextMeasurer, TextMetrics};
use exact_runner::{DataError, DataSource, Event, Value};
use std::{cell::Cell, rc::Rc};

struct Data;
impl DataSource for Data {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        if name == "blob" {
            Ok(Value::Record(
                vec![Value::str(&"giant α body ".repeat(8192))].into(),
            ))
        } else if name == "numbers" {
            Ok(Value::List(
                vec![Value::Number(1.), Value::Number(2.)].into(),
            ))
        } else {
            Err(DataError::UnknownSource(name.into()))
        }
    }
}
struct ShellOnly(Rc<Cell<usize>>);
impl TextMeasurer for ShellOnly {
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        if request.runs.iter().map(|r| r.text.len()).sum::<usize>() > 1024 {
            self.0.set(self.0.get() + 1);
            panic!("registered giant escaped into foreign measurer");
        }
        MonospaceMeasurer::default().measure(request)
    }
}
fn registration() -> ContentRegionRegistration {
    ContentRegionRegistration {
        activate: None,
        owner: "owner",
        content: "content",
        pending: "pending",
    }
}
type Fixture = (Host<Data>, String, Rc<Cell<usize>>);
fn fixture(ids: ContentRegionRegistration) -> Result<Fixture, exact_apple::HostError> {
    let source = r#"shape Blob
  text: string

component App
  state draft = ""
  action edit(value: string)
    draft = value
  resource doc = blob() as shape Blob
  view
    column width="100%" height="100%"
      view id="owner" width="100%" height=400 flex-shrink=0 overflow-x="hidden" overflow-y="hidden"
        view id="content" width="100%" height="100%"
          scroll testId="document" width="100%" height="100%"
            text doc.text font-size=16
        text "Preparing exact content…" id="pending"
      input value=draft change=edit testId="input"
      text draft testId="echo"
"#;
    let plan = contract::compile(source).unwrap().encode();
    let count = Rc::new(Cell::new(0));
    Host::boot_region(
        &plan,
        Data,
        Box::new(ShellOnly(count.clone())),
        600.,
        800.,
        ids,
    )
    .map(|(host, batch)| (host, batch, count))
}
struct DropProbe(Rc<Cell<usize>>);
impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
fn settle(h: &mut Host<Data>, drops: &Rc<Cell<usize>>) -> usize {
    let mut turns = 0;
    while let Some((id, request)) = h.pending_region_request() {
        let metrics = request.with_request(|r| MonospaceMeasurer::default().measure(r));
        let batch = h.complete_region_text(id, metrics, Rc::new(DropProbe(drops.clone())));
        assert!(!batch.contains("\"error\":\""), "{batch}");
        turns += 1;
        assert!(turns <= 64, "unbounded offer discovery");
    }
    turns
}
#[test]
fn first_layout_uses_real_pending_and_never_calls_foreign_giant() {
    let (mut h, batch, calls) = fixture(registration()).unwrap();
    assert!(batch.contains("\"op\":\"region\""), "{batch}");
    assert!(batch.contains("\"selection\":\"pending\""), "{batch}");
    let (id, _) = h.pending_region_request().unwrap();
    let snapshot = h.region_request_json(id).unwrap();
    assert!(snapshot.contains("giant α body"));
    let input = h.runner().kernel().find_by_test_id("input")[0];
    let view = h.runner().kernel().node_by_key(input).unwrap().id;
    let typed = h.dispatch_at(view, Event::Change("Aα exact input".into()), 10.);
    assert!(typed.contains("Aα exact input"));
    assert_eq!(
        h.pending_region_request().unwrap().0,
        id,
        "unrelated typing must keep request identity"
    );
    h.resize(601., 800.);
    assert_eq!(calls.get(), 0);
}
#[test]
fn stale_bad_metrics_drop_only_their_owner_and_cannot_publish() {
    let (mut h, _, _) = fixture(registration()).unwrap();
    let old = h.pending_region_request().unwrap().0;
    h.resize(611., 800.);
    let current = h.pending_region_request().unwrap().0;
    assert_ne!(old, current);
    let drops = Rc::new(Cell::new(0));
    let before = h.engine().now();
    let batch = h.complete_region_text(
        old,
        TextMetrics {
            width: f32::NAN,
            height: -1.,
            first_baseline: None,
        },
        Rc::new(DropProbe(drops.clone())),
    );
    assert!(!batch.contains("\"error\":\""), "{batch}");
    assert_eq!(drops.get(), 1);
    assert_eq!(h.engine().now(), before);
    assert_eq!(h.pending_region_request().unwrap().0, current);
    assert!(settle(&mut h, &drops) > 0);
    let batch = h.resize(611., 800.);
    assert!(batch.contains("\"selection\":\"accepted\""), "{batch}");
    let prior = drops.get();
    drop(h);
    assert!(
        drops.get() > prior,
        "native artifact survives until accepted publication drops"
    );
}
#[test]
fn accepted_source_and_extent_survive_pending_width_replacement() {
    let (mut h, _, calls) = fixture(registration()).unwrap();
    let drops = Rc::new(Cell::new(0));
    settle(&mut h, &drops);
    let old = h.region_publication_id().unwrap();
    let before = drops.get();
    let batch = h.resize(450., 800.);
    assert!(batch.contains("\"current\":false"), "{batch}");
    assert_eq!(h.region_publication_id(), Some(old));
    assert_eq!(
        drops.get(),
        before,
        "old accepted artifact must remain pinned"
    );
    settle(&mut h, &drops);
    assert_ne!(h.region_publication_id(), Some(old));
    assert_eq!(calls.get(), 0);
}
#[test]
fn invalid_registration_refuses_before_any_layout() {
    let mut ids = registration();
    ids.owner = "absent";
    assert!(fixture(ids).is_err());
}

static RELEASES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
extern "C" fn release(_: *mut std::ffi::c_void) {
    RELEASES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}
#[test]
fn bridge_completion_owns_payload_even_when_runtime_not_booted() {
    let mut bridge = exact_apple::abi::Bridge::<Data>::new();
    RELEASES.store(0, std::sync::atomic::Ordering::SeqCst);
    let owner = exact_apple::content_region::NativeRegionOwner::new(std::ptr::null_mut(), release);
    let n = bridge.region_complete(
        99,
        exact_apple::measure::CMetrics {
            width: f32::NAN,
            height: -1.,
            baseline: -1.,
        },
        Rc::new(owner),
    );
    assert!(String::from_utf8_lossy(bridge.output_bytes(n as usize)).contains("not booted"));
    assert_eq!(RELEASES.load(std::sync::atomic::Ordering::SeqCst), 1);
}

/// Producer-side companion to CollectionMacTests' complete native-subtree
/// boundary. This fixed envelope is an admission control, not the full 10k
/// Messages consumer or a replacement for its preserved 64-offer overflow.
#[test]
fn selected_native_subtree_waits_for_b_while_outside_composer_commits() {
    let source = r##"component App
  state mounted = true
  action unmount
    mounted = false
  state body = "A body 👩🏽‍💻 العربية"
  state sender = true
  state draft = ""
  action revise
    body = "B body not selected yet"
    sender = false
  action edit(value: string)
    draft = value
  action reply
    draft = "Reply selected row"
  view
    column width="100%" height="100%"
      when mounted
        view id="owner" width="100%" height=400 flex-shrink=0 overflow-x="hidden" overflow-y="hidden"
          view id="content" width="100%" height="100%"
            column testId="bubble" padding=10 border-radius=12 background-color="#e6f0ff"
              when sender
                text "Sender" testId="sender"
              text body testId="body" font-size=14 line-height=1.45
              text "12:34" testId="meta"
              button press=reply testId="reply"
                text "Reply"
          text "Preparing" id="pending"
      button press=revise testId="revise"
        text "Revise"
      button press=unmount testId="unmount"
        text "Unmount"
      input value=draft change=edit testId="input"
      text draft testId="echo"
"##;
    let plan = contract::compile(source).unwrap().encode();
    let (mut host, _) = Host::boot_native_region(
        &plan,
        Data,
        Box::new(MonospaceMeasurer::default()),
        600.,
        800.,
        registration(),
        exact_apple::content_region::NativeProjectionLimits::default(),
    )
    .unwrap();
    let drops = Rc::new(Cell::new(0));
    assert!(settle(&mut host, &drops) > 0);
    let selected = host.region_publication_id().expect("A complete before B");
    let view = |host: &Host<Data>, name: &str| {
        let key = host.runner().kernel().find_by_test_id(name)[0];
        host.runner().kernel().node_by_key(key).unwrap().id
    };
    let body = view(&host, "body");
    let bubble = view(&host, "bubble");
    let sender = view(&host, "sender");
    let input = view(&host, "input");
    let revise = view(&host, "revise");
    host.resize(550., 800.);
    let (blocked, request) = host
        .pending_region_request()
        .expect("real B request withheld");
    let blocked_metrics = request.with_request(|r| MonospaceMeasurer::default().measure(r));
    let pending = host.dispatch_at(revise, Event::Press, 10.);
    let typed = host.dispatch_at(input, Event::Change("composer progresses".into()), 11.);
    let newer = host
        .pending_region_request()
        .expect("new source requires its own B")
        .0;
    assert_ne!(blocked, newer, "the control actually superseded B");
    let retained = host.region_publication_id() == Some(selected);
    let leaks_body = pending.contains(&format!("{{\"op\":\"props\",\"id\":{body},"));
    let leaks_children = pending.contains(&format!("{{\"op\":\"children\",\"id\":{bubble},"));
    let leaks_destroy = pending.contains(&format!("{{\"op\":\"destroy\",\"id\":{sender}}}"));
    assert!(
        typed.contains("composer progresses"),
        "outside shell remains live: {typed}"
    );
    let before_stale = host.region_publication_id();
    let before_clock = host.engine().now();
    let stale =
        host.complete_region_text(blocked, blocked_metrics, Rc::new(DropProbe(drops.clone())));
    assert!(!stale.contains("\"error\":\""), "{stale}");
    assert_eq!(host.pending_region_request().unwrap().0, newer);
    assert_eq!(
        host.region_publication_id(),
        before_stale,
        "stale completion cannot select anything"
    );
    assert_eq!(
        host.engine().now(),
        before_clock,
        "completion has no invented input clock"
    );
    // Print every boundary observation before the one expected behavioral RED.
    eprintln!("selected={selected} retained={retained} body_leak={leaks_body} children_leak={leaks_children} destroy_leak={leaks_destroy}");
    assert!(
        retained && !leaks_body && !leaks_children && !leaks_destroy,
        "selected native A must remain complete while latest B is pending: {pending}"
    );
    let mut completed = String::new();
    let mut turns = 0;
    while let Some((id, request)) = host.pending_region_request() {
        let metrics = request.with_request(|r| MonospaceMeasurer::default().measure(r));
        let batch = host.complete_region_text(id, metrics, Rc::new(DropProbe(drops.clone())));
        assert!(!batch.contains("\"error\":\""), "{batch}");
        completed.push_str(&batch);
        turns += 1;
        assert!(turns <= 64);
    }
    assert_ne!(host.region_publication_id(), Some(selected));
    assert!(
        completed.contains("B body not selected yet"),
        "complete B owns its body"
    );
    assert!(completed.contains(&format!("{{\"op\":\"destroy\",\"id\":{sender}}}")));
    assert!(completed.contains(&format!("{{\"op\":\"children\",\"id\":{bubble},")));
    assert!(
        host.runner().kernel().find_by_test_id("sender").is_empty(),
        "selection never resurrects a live key"
    );
    let unmount = view(&host, "unmount");
    let _terminal = host.dispatch_at(unmount, Event::Press, 12.);
    assert!(
        host.region_publication_id().is_none(),
        "removed owner cannot retain native selection"
    );
    assert!(
        host.pending_region_request().is_none(),
        "removed owner cannot keep a worker request"
    );
    assert!(
        drops.get() > 0,
        "native and kernel artifact aliases release on owner removal"
    );
}

#[test]
fn native_projection_budget_is_explicit_and_does_not_select_opaque_mode() {
    let source = r#"component App
  view
    column width="100%" height="100%"
      view id="owner" width="100%" height=400 flex-shrink=0 overflow-x="hidden" overflow-y="hidden"
        view id="content" width="100%" height="100%"
          text "bounded body"
        text "Preparing" id="pending"
"#;
    let bytes = contract::compile(source).unwrap().encode();
    let limits = exact_apple::content_region::NativeProjectionLimits {
        nodes: 0,
        ..Default::default()
    };
    let refused = Host::boot_native_region(
        &bytes,
        Data,
        Box::new(MonospaceMeasurer::default()),
        600.,
        800.,
        registration(),
        limits,
    );
    assert!(
        refused.is_err(),
        "no partial selected packet on structural refusal"
    );
}

#[test]
fn native_collection_epochs_stay_with_selected_rows_until_complete() {
    let source = r#"component App
  resource numbers = numbers() as shape list<number>
  state textValue = "A body"
  state draft = ""
  action revise
    textValue = "B longer body still pending"
  action edit(value: string)
    draft = value
  view
    column width="100%" height="100%"
      view id="owner" width="100%" height=400 flex-shrink=0 overflow-x="hidden" overflow-y="hidden"
        view id="content" width="100%" height="100%"
          list virtualized=true testId="list" width="100%" height="100%"
            each item in numbers key=item
              column width="100%" padding=4
                text textValue
                button press=revise
                  text "Reply"
        text "Preparing" id="pending"
      button press=revise testId="revise"
        text "Revise"
      input value=draft change=edit testId="input"
"#;
    let plan = contract::compile(source).unwrap().encode();
    let (mut host, _) = Host::boot_native_region(
        &plan,
        Data,
        Box::new(MonospaceMeasurer::default()),
        600.,
        800.,
        registration(),
        Default::default(),
    )
    .unwrap();
    let drops = Rc::new(Cell::new(0));
    settle(&mut host, &drops);
    let selected = host.region_publication_id().unwrap();
    let a = host.runner().collections().pop().unwrap();
    let revise = host.runner().kernel().find_by_test_id("revise")[0];
    let revise = host.runner().kernel().node_by_key(revise).unwrap().id;
    let pending = host.dispatch_at(revise, Event::Press, 10.);
    assert!(!pending.contains("\"error\":\""), "{pending}");
    assert!(host.pending_region_request().is_some());
    assert_eq!(host.region_publication_id(), Some(selected));
    let b = host.runner().collections().pop().unwrap();
    assert_ne!(
        a.revision, b.revision,
        "the fixture really changed row metadata"
    );
    assert!(
        !pending.contains("\"op\":\"collections\""),
        "live B collection metadata leaked: {pending}"
    );
    let feedback = exact_runner::CollectionFeedback {
        view: a.view,
        revision: a.revision,
        scroll_sequence: a.scroll_sequence + 1,
        offset: 0.,
        port_cross: 600.,
        port_main: 400.,
        cross: 600.,
        measurements: a
            .rows
            .iter()
            .map(|r| exact_runner::RowMeasurement {
                view: r.view,
                epoch: r.epoch,
                size: 120.,
            })
            .collect(),
        focus_view: None,
        interaction_view: None,
    };
    let stale = host.collection_feedback(&feedback.encode().unwrap(), 11.);
    assert!(!stale.contains("\"error\":\""), "{stale}");
    assert_eq!(
        host.runner().collections()[0],
        b,
        "A heights must not be credited to B epochs"
    );
    settle(&mut host, &drops);
    assert_ne!(host.region_publication_id(), Some(selected));
}

/// Accumulated actual public frame/content ops, following the existing host
/// test's last-op reader. No reconstructed native batch or epsilon oracle.
fn projection_wire(history: &str, id: u32, op: &str) -> Option<(Vec<u32>, String)> {
    let marker = format!("\"op\":\"{op}\",\"id\":{id},");
    let at = history.rfind(&marker)?;
    let rest = &history[at..];
    let raw = &rest[..=rest.find('}').unwrap()];
    let fields: &[&str] = if op == "frame" {
        &["x", "y", "w", "h"]
    } else {
        &["w", "h"]
    };
    let bits = fields
        .iter()
        .map(|name| {
            let field = format!("\"{name}\":");
            let value = &raw[raw.find(&field).unwrap() + field.len()..];
            let n: f32 = value[..value.find([',', '}']).unwrap()].parse().unwrap();
            assert!(n.is_finite());
            n.to_bits()
        })
        .collect();
    Some((bits, raw.to_owned()))
}

fn projection_id(host: &Host<Data>, name: &str) -> u32 {
    let k = host.runner().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

fn projection_ids(host: &Host<Data>) -> Vec<u32> {
    let mut stack = vec![projection_id(host, "content")];
    let mut ids = Vec::new();
    while let Some(id) = stack.pop() {
        ids.push(id);
        stack.extend(
            host.runner()
                .kernel()
                .node(id)
                .unwrap()
                .children()
                .into_iter()
                .rev(),
        );
    }
    ids
}

fn projection_complete(host: &mut Host<Data>, history: &mut String) -> usize {
    let mut count = 0;
    while let Some((id, request)) = host.pending_region_request() {
        let metrics = request.with_request(|r| MonospaceMeasurer::default().measure(r));
        let batch = host.complete_region_text(id, metrics, Rc::new(()));
        assert!(!batch.contains("\"error\":\""), "{batch}");
        history.push_str(&batch);
        count += 1;
        assert!(count <= 64);
    }
    count
}

fn projection_matches_ordinary(
    ordinary: &Host<Data>,
    native: &Host<Data>,
    ordinary_wire: &str,
    native_wire: &str,
) {
    let ids = projection_ids(native);
    assert_eq!(ids, projection_ids(ordinary));
    let mut inline = 0;
    let mut content_ops = 0;
    for id in ids {
        let expected = ordinary.runner().kernel().node(id).unwrap();
        let actual = native.runner().kernel().node(id).unwrap();
        assert!(
            actual.frame.bits_eq(expected.frame),
            "world frame {id}: {:?} != {:?}",
            actual.frame,
            expected.frame
        );
        if actual.is_inline_run() {
            inline += 1;
            assert!(actual.frame.bits_eq(exact_kernel::Frame::default()));
            for wire in [ordinary_wire, native_wire] {
                assert!(
                    projection_wire(wire, id, "frame").is_none(),
                    "inline frame {id}"
                );
                assert!(
                    !wire.contains(&format!("\"op\":\"create\",\"id\":{id},")),
                    "inline view {id}"
                );
                assert!(
                    wire.contains(&format!("\"id\":{id},\"parent\":")),
                    "missing run {id}"
                );
            }
            continue;
        }
        let parent = expected
            .parent
            .and_then(|p| ordinary.runner().kernel().node(p));
        let f = expected.frame;
        let tuple = parent.map_or([f.x, f.y, f.width, f.height], |p| {
            [f.x - p.frame.x, f.y - p.frame.y, f.width, f.height]
        });
        let expected_wire =
            projection_wire(ordinary_wire, id, "frame").expect("ordinary frame emitted");
        assert_eq!(
            expected_wire.0,
            tuple.map(f32::to_bits),
            "ordinary wire/kernel {id}"
        );
        let actual_wire = projection_wire(native_wire, id, "frame").expect("native frame emitted");
        assert_eq!(
            actual_wire, expected_wire,
            "all f32 bits and exact frame bytes {id}"
        );
        let content = projection_wire(ordinary_wire, id, "content");
        content_ops += usize::from(content.is_some());
        assert_eq!(
            projection_wire(native_wire, id, "content"),
            content,
            "overflow {id}"
        );
    }
    assert!(
        inline >= 2,
        "the inline-zero control must actually participate"
    );
    assert!(
        content_ops > 0,
        "the overflow control must actually participate"
    );
}

#[test]
fn native_bulk_projection_matches_ordinary_through_origin_pending_and_width() {
    let source = r#"component App
  state top = 213.6
  action shift
    top = 213.7
  action shiftAgain
    top = 214.1
  view
    column width="100%" height="100%" padding-top=top
      view id="owner" testId="owner" width="100%" height=400 flex-shrink=0 overflow-x="hidden" overflow-y="hidden"
        view id="content" testId="content" width="100%" height="100%" padding-top=0.1
          view testId="nested" padding-top=0.1
            text testId="paragraph" font-size=14 line-height=1.45
              text "Exact α emoji 👩🏽‍💻 and mixed inline text for a genuinely new width " testId="inline-one"
              text "second run with more words to wrap across the current offered width" testId="inline-two" font-weight=700
            scroll testId="overflow" width="100%" height=60
              view width="100%" height=480
        text "Preparing" id="pending"
      button press=shift testId="shift"
        text "Shift"
      button press=shiftAgain testId="shift-again"
        text "Shift again"
"#;
    let plan = contract::compile(source).unwrap().encode();
    let (mut ordinary, mut ordinary_wire) = Host::boot(
        &plan,
        Data,
        Box::new(MonospaceMeasurer::default()),
        400.,
        900.,
    )
    .unwrap();
    let (mut native, mut native_wire) = Host::boot_native_region(
        &plan,
        Data,
        Box::new(MonospaceMeasurer::default()),
        400.,
        900.,
        registration(),
        Default::default(),
    )
    .unwrap();
    assert!(projection_complete(&mut native, &mut native_wire) > 0);
    projection_matches_ordinary(&ordinary, &native, &ordinary_wire, &native_wire);
    let first = native.region_publication_id().unwrap();
    let owner = native
        .runner()
        .kernel()
        .node(projection_id(&native, "owner"))
        .unwrap()
        .frame;
    let paragraph = native
        .runner()
        .kernel()
        .node(projection_id(&native, "paragraph"))
        .unwrap()
        .frame;
    assert_eq!(owner.y.to_bits(), 213.6_f32.to_bits());
    assert_eq!(
        paragraph.y.to_bits(),
        ((owner.y + 0.1_f32) + 0.1_f32).to_bits()
    );
    assert_ne!(
        paragraph.y.to_bits(),
        (owner.y + (0.1_f32 + 0.1_f32)).to_bits(),
        "real nonassociative f32 control"
    );

    // Same shape width/source, different actual parent origin. No new metadata
    // or width-history identity may be substituted for ordinary f32 residuals.
    let ordinary_shift = projection_id(&ordinary, "shift");
    let native_shift = projection_id(&native, "shift");
    ordinary_wire.push_str(&ordinary.dispatch_at(ordinary_shift, Event::Press, 10.));
    native_wire.push_str(&native.dispatch_at(native_shift, Event::Press, 10.));
    assert!(
        native.pending_region_request().is_none(),
        "origin-only must reuse complete metrics"
    );
    assert_ne!(
        native.region_publication_id(),
        Some(first),
        "new origin owns a new selected packet"
    );
    projection_matches_ordinary(&ordinary, &native, &ordinary_wire, &native_wire);

    let a = native.region_publication_id().unwrap();
    let saved: Vec<_> = projection_ids(&native)
        .into_iter()
        .map(|id| {
            (
                id,
                projection_wire(&native_wire, id, "frame"),
                projection_wire(&native_wire, id, "content"),
            )
        })
        .collect();
    ordinary_wire.push_str(&ordinary.resize(359., 900.));
    let pending = native.resize(359., 900.);
    assert!(!pending.contains("\"error\":\""), "{pending}");
    assert!(pending.contains("\"current\":false"));
    assert!(native.pending_region_request().is_some());
    native_wire.push_str(&pending);
    let os = projection_id(&ordinary, "shift-again");
    let ns = projection_id(&native, "shift-again");
    ordinary_wire.push_str(&ordinary.dispatch_at(os, Event::Press, 11.));
    let moved = native.dispatch_at(ns, Event::Press, 11.);
    assert!(!moved.contains("\"error\":\""), "{moved}");
    native_wire.push_str(&moved);
    assert_eq!(native.region_publication_id(), Some(a));
    for (id, frame, content) in &saved {
        assert_eq!(
            &projection_wire(&native_wire, *id, "frame"),
            frame,
            "pending A frame stays immutable"
        );
        assert_eq!(
            &projection_wire(&native_wire, *id, "content"),
            content,
            "pending A extent stays immutable"
        );
    }
    assert!(projection_complete(&mut native, &mut native_wire) > 0);
    assert_ne!(native.region_publication_id(), Some(a));
    projection_matches_ordinary(&ordinary, &native, &ordinary_wire, &native_wire);

    // Existing public invalid-viewport refusal cannot project/relabel A.
    let before = native.region_publication_id();
    let refused = native.resize(f32::NAN, 900.);
    assert!(refused.contains("\"error\":\""), "{refused}");
    assert_eq!(native.region_publication_id(), before);
    for id in projection_ids(&native) {
        assert!(projection_wire(&refused, id, "frame").is_none());
    }
}

#[test]
fn native_region_promotes_and_clears_the_fields_editor_rect() {
    let plan = contract::compile(
        r#"component App
  view
    column width="100%" height="100%"
      view id="owner" width=300 height=200 overflow-x="hidden" overflow-y="hidden"
        view id="content" width="100%" height="100%"
          input testId="field" padding=10 value="Inset"
        view id="pending"
"#,
    )
    .unwrap()
    .encode();
    let (mut host, batch) = Host::boot_native_region(
        &plan,
        Data,
        Box::new(MonospaceMeasurer::default()),
        400.,
        300.,
        registration(),
        exact_apple::content_region::NativeProjectionLimits::default(),
    )
    .unwrap();
    let mut batches = vec![batch];
    while let Some((id, request)) = host.pending_region_request() {
        let metrics = request.with_request(|r| MonospaceMeasurer::default().measure(r));
        batches.push(host.complete_region_text(id, metrics, Rc::new(())));
        assert!(batches.len() <= 64);
    }
    assert!(host.region_publication_id().is_some());
    let id = host
        .runner()
        .kernel()
        .node_by_key(host.runner().kernel().find_by_test_id("field")[0])
        .unwrap()
        .id;
    let rect = host
        .runner()
        .kernel()
        .node(id)
        .unwrap()
        .field_content_rect()
        .unwrap();
    assert_eq!(rect.x, 10.);
    let ops = |batch: &str| {
        serde_json::from_str::<serde_json::Value>(batch).unwrap()["ops"]
            .as_array()
            .unwrap()
            .clone()
    };
    let editor = batches
        .iter()
        .flat_map(|batch| ops(batch))
        .find(|op| op["op"] == "fieldContent" && op["id"] == id)
        .expect("promotion must publish the editor rect");
    let emitted: Vec<f32> = serde_json::from_value(editor["rect"].clone()).unwrap();
    assert_eq!(emitted, [rect.x, rect.y, rect.width, rect.height]);
    assert!(!ops(&host.resize(400., 300.))
        .iter()
        .any(|op| op["op"] == "fieldContent"));
    // Producer-level appearance change exercises the clearing protocol without
    // introducing a bound appearance into Contract (which refuses it).
    let mut patch = exact_kernel::StyleProps::default();
    patch
        .set_dynamic(
            exact_kernel::StyleId::Appearance,
            &exact_kernel::StyleValue::Text("none".into()),
        )
        .unwrap();
    host.runner_mut()
        .kernel_mut()
        .apply(
            0,
            99,
            &[exact_kernel::Op::SetStyle {
                id,
                patch: Box::new(patch),
            }],
        )
        .unwrap();
    let mut batch = host.resize(401., 300.);
    while let Some((id, request)) = host.pending_region_request() {
        let metrics = request.with_request(|r| MonospaceMeasurer::default().measure(r));
        batch = host.complete_region_text(id, metrics, Rc::new(()));
    }
    assert!(
        ops(&batch)
            .iter()
            .any(|op| op["op"] == "fieldContent" && op["id"] == id && op["rect"].is_null()),
        "{batch}"
    );
}
