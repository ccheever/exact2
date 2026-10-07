//! Real Presenter regressions over a pending display submission.
use super::super::*;
use super::SubmittedFrame;
use crate::image::Bitmap;
use crate::paint::Shape;
use crate::text::{Paragraph, RunPaint};
use exact_runner::{Answer, DataError, Outcome, Request, Response, Store, Value};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use tiny_skia::Transform;

fn submit<D: DataSource>(p: &mut Presenter<D>) -> Option<SubmittedFrame> {
    p.display_frame()
}
fn complete<D: DataSource>(p: &mut Presenter<D>, frame: &SubmittedFrame) -> bool {
    p.display_complete(frame)
}

#[derive(Default)]
struct Empty;
impl DataSource for Empty {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
    fn answer(&mut self, _: &mut Store, name: &str, _: &[Value]) -> Result<Answer, DataError> {
        assert_eq!(name, "work");
        Ok(Answer::Later(Request::continuation(1)))
    }
    fn parse(
        &mut self,
        _: &mut Store,
        name: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        assert_eq!(name, "work");
        assert!(matches!(outcome, Outcome::Response(_)));
        Ok(Answer::Now(Value::Bool(true)))
    }
}
const APP: &str = r##"component App
  state draft = ""
  state count = 0
  state showing = true
  state ticks = 0
  mutation result as shape bool
  action edit(value)
    draft = value
  action press
    count = count + 1
  action hide
    showing = false
  action show
    showing = true
  action tick
    ticks = ticks + 1
  action request
    send result = work()
  task timer mount
    every(250, tick)
  view
    column width=320 height=240
      input value=draft input=edit testId="input" height=32
      when showing
        box press=press testId="target" width=100 height=32 background-color="#cc3300"
      box press=hide testId="hide" height=16
      box press=show testId="show" height=16
      text `${count}` testId="count" height=20
      box press=request testId="request" height=16
      text `${ticks}` testId="ticks" height=16
      match result
        case some(value)
          text "completed" testId="completed" height=16
        case none
          text "idle" height=16
      scroll testId="port" width=300 height=64
        box width=300 height=640 background-color="#225599"
"##;
struct CountPaint {
    inner: Raster,
    paints: Rc<Cell<usize>>,
    fail: Rc<Cell<bool>>,
}
impl Backend for CountPaint {
    fn name(&self) -> &'static str {
        "counted-cpu"
    }
    fn begin(&mut self, w: f32, h: f32, s: f32) {
        self.paints.set(self.paints.get() + 1);
        self.inner.begin(w, h, s);
    }
    fn fill(&mut self, s: &Shape, c: [u8; 4], t: Transform) {
        self.inner.fill(s, c, t);
    }
    fn fill_gradient(&mut self, s: &Shape, g: &crate::paint::GradientPaint, t: Transform) {
        self.inner.fill_gradient(s, g, t);
    }
    fn fill_border(&mut self, part: &crate::paint::border::BorderFill, t: Transform) {
        self.inner.fill_border(part, t);
    }
    fn image(
        &mut self,
        b: &Arc<Bitmap>,
        r: Rect4,
        c: &[Shape],
        t: Transform,
        tint: Option<[u8; 4]>,
    ) {
        self.inner.image(b, r, c, t, tint);
    }
    fn text(
        &mut self,
        e: &mut TextEngine,
        p: &Paragraph,
        c: &[RunPaint],
        o: (f32, f32),
        t: Transform,
    ) {
        self.inner.text(e, p, c, o, t);
    }
    fn push_clip(&mut self, s: &Shape, t: Transform) {
        self.inner.push_clip(s, t);
    }
    fn pop_clip(&mut self) {
        self.inner.pop_clip();
    }
    fn push_opacity(&mut self, a: f32) {
        self.inner.push_opacity(a);
    }
    fn pop_opacity(&mut self) {
        self.inner.pop_opacity();
    }
    fn pointer(&mut self, x: f32, y: f32) {
        self.inner.pointer(x, y);
    }
    fn finish(&mut self) -> Result<Pixmap, String> {
        if self.fail.get() {
            Err("injected finish refusal".into())
        } else {
            self.inner.finish()
        }
    }
}
type Fixture = (Presenter<Empty>, Rc<Cell<usize>>, Rc<Cell<bool>>);
fn boot() -> Fixture {
    boot_app(APP)
}
fn boot_app(app: &str) -> Fixture {
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(app).unwrap().encode(),
        Empty,
        (320., 240.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let paints = Rc::new(Cell::new(0));
    let fail = Rc::new(Cell::new(false));
    p.brush.replace_backend(Box::new(CountPaint {
        inner: Raster::new(),
        paints: paints.clone(),
        fail: fail.clone(),
    }));
    (p, paints, fail)
}
fn primed() -> Fixture {
    let (mut p, paints, fail) = boot();
    let a = submit(&mut p).unwrap();
    assert!(complete(&mut p, &a));
    paints.set(0);
    (p, paints, fail)
}
fn id(p: &Presenter<Empty>, name: &str) -> ViewId {
    let k = p.host.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}
fn point(p: &Presenter<Empty>, view: ViewId) -> (f32, f32) {
    let b = p.boxes.iter().find(|b| b.id == view).unwrap();
    (b.rect.0 + 2., b.rect.1 + 2.)
}
fn count(p: &Presenter<Empty>) -> &str {
    p.host
        .kernel()
        .node(id(p, "count"))
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
}

#[test]
fn existing_executor_completion_and_timer_commit_while_display_slot_is_occupied() {
    let (mut p, paints, _) = primed();
    let first = submit(&mut p).unwrap();
    // Dispatch through the actual Host, then give its exact ticket to the
    // existing Executor with controlled work. There is no socket server or
    // transport timing assumption; the UI must pump before any flip release.
    assert!(p
        .host
        .dispatch_at(id(&p, "request"), Event::Press, 1.)
        .is_none());
    let request = p.host.take_requests().pop().unwrap();
    assert!(p.after_commit().is_none());
    let (release, gate) = std::sync::mpsc::channel();
    p.executor
        .run(
            request,
            Some(exact_runner::Work::Now(Box::new(move || {
                gate.recv_timeout(Duration::from_secs(5)).unwrap();
                Outcome::Response(Response {
                    status: 200,
                    headers: vec![],
                    body: vec![],
                })
            }))),
        )
        .unwrap();
    assert!(p.pending());
    let input = id(&p, "input");
    p.type_text(input, "still typing").unwrap();
    assert!(p.advance(250.).is_none());
    assert_eq!(
        p.host
            .kernel()
            .node(id(&p, "ticks"))
            .unwrap()
            .props
            .str(PropId::Text),
        Some("1")
    );
    release.send(()).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while p.pending() {
        assert!(
            std::time::Instant::now() < deadline,
            "owned worker did not deliver"
        );
        assert!(p.pump(251.).is_none());
        std::thread::yield_now();
    }
    assert_eq!(p.host.kernel().find_by_test_id("completed").len(), 1);
    assert_eq!(paints.get(), 1);
    assert!(p.dirty());
    assert!(
        submit(&mut p).is_none(),
        "completion must not overwrite either display buffer"
    );
    assert!(complete(&mut p, &first));
    submit(&mut p).unwrap();
    assert_eq!(paints.get(), 2);
}

#[test]
fn pending_dirty_hit_input_scroll_and_timer_do_not_paint_or_clear_dirty() {
    let (mut p, paints, _) = primed();
    let first = submit(&mut p).unwrap();
    let target = id(&p, "target");
    let hit = point(&p, target);
    let port = id(&p, "port");
    let wheel = point(&p, port);
    let input = id(&p, "input");
    p.type_text(input, "EXACT_🧪漢字").unwrap();
    assert!(p.dirty());
    assert_eq!(p.hit(hit.0, hit.1), Some(target));
    assert_eq!(p.press_at(hit.0, hit.1, 1.), Some(target));
    p.wheel_at(wheel.0, wheel.1, 0., 40.);
    assert!(p.advance(250.).is_none());
    assert!(p.pump(251.).is_none());
    assert_eq!(count(&p), "1");
    assert_eq!(
        p.host
            .kernel()
            .node(input)
            .unwrap()
            .props
            .str(PropId::Value),
        Some("EXACT_🧪漢字")
    );
    assert_eq!(p.scroll_of(port).1, 40.);
    assert_eq!(p.host.now(), 250.);
    assert_eq!(
        paints.get(),
        1,
        "boxes/hit must not paint into an occupied display slot"
    );
    assert!(
        p.dirty(),
        "new work must survive until the back buffer is free"
    );
    assert!(complete(&mut p, &first));
    assert!(p.dirty());
    let next = submit(&mut p).unwrap();
    assert_eq!(paints.get(), 2);
    assert_ne!(first.pixels.data(), next.pixels.data());
}

#[test]
fn one_pending_submission_coalesces_latest_input_without_a_second_pixmap() {
    let (mut p, paints, _) = primed();
    let first = submit(&mut p).unwrap();
    let input = id(&p, "input");
    for n in 0..20 {
        p.type_text(input, &format!("latest {n}")).unwrap();
        assert!(
            submit(&mut p).is_none(),
            "two buffers permit only one pending flip"
        );
    }
    assert_eq!(paints.get(), 1);
    assert!(complete(&mut p, &first));
    let next = submit(&mut p).unwrap();
    assert_eq!(paints.get(), 2);
    assert!(
        !complete(&mut p, &first),
        "duplicate receipt cannot retire the successor"
    );
    assert!(submit(&mut p).is_none());
    assert!(complete(&mut p, &next));
}

#[test]
fn submitted_pixels_and_first_pixel_belong_to_submission_despite_live_dirty_input() {
    let (mut p, paints, _) = boot();
    let first = submit(&mut p).unwrap();
    let hash = first.pixels.data().to_vec();
    let weak = Arc::downgrade(&first.pixels);
    p.set_pointer(Some((200., 200.)));
    assert!(!p.painted);
    p.first_pixel(); // A generic module tick is not a flip acknowledgement.
    assert!(!p.painted);
    assert!(complete(&mut p, &first));
    assert!(
        p.painted,
        "submitted successful frame may activate while later input is dirty"
    );
    assert!(p.dirty());
    assert_eq!(hash, first.pixels.data());
    assert_eq!(paints.get(), 1);
    drop(first);
    assert!(
        weak.upgrade().is_none(),
        "receipt must not become a pixel history"
    );
}

#[test]
fn reload_during_pending_refuses_old_hits_and_old_first_pixel_even_if_ids_alias() {
    let (mut p, paints, _) = primed();
    let first = submit(&mut p).unwrap();
    let target = id(&p, "target");
    let old_key = p.host.kernel().node(target).unwrap().key;
    let hit = point(&p, target);
    p.reload(&contract::compile(APP).unwrap().encode(), Empty)
        .unwrap();
    let new_target = id(&p, "target");
    assert_eq!(
        target, new_target,
        "fixture exercises the same wire id across runtimes"
    );
    assert_eq!(old_key, p.host.kernel().node(new_target).unwrap().key);
    assert_eq!(p.hit(hit.0, hit.1), None);
    assert!(p.press_at(hit.0, hit.1, 2.).is_none());
    assert_eq!(count(&p), "0");
    assert_eq!(paints.get(), 1);
    assert!(
        complete(&mut p, &first),
        "old DRM owner is released, not adopted by new host"
    );
    assert!(!p.painted);
    let second = submit(&mut p).unwrap();
    assert!(complete(&mut p, &second));
    assert!(p.painted);
    assert_eq!(p.hit(hit.0, hit.1), Some(new_target));
}

#[test]
fn rejected_reload_preserves_pending_origin_and_can_acknowledge_it() {
    let (mut p, paints, _) = boot();
    let first = submit(&mut p).unwrap();
    assert!(p.reload(b"invalid plan", Empty).is_err());
    p.set_pointer(Some((200., 200.)));
    assert!(complete(&mut p, &first));
    assert!(p.painted);
    assert!(p.dirty());
    assert_eq!(paints.get(), 1);
}

#[test]
fn destroyed_then_remounted_target_cannot_reuse_the_submitted_hit_identity() {
    let (mut p, paints, _) = primed();
    let first = submit(&mut p).unwrap();
    let old = id(&p, "target");
    let old_key = p.host.kernel().node(old).unwrap().key;
    let hit = point(&p, old);
    for name in ["hide", "show"] {
        assert!(p.host.dispatch_at(id(&p, name), Event::Press, 1.).is_none());
        assert!(p.after_commit().is_none());
    }
    let new = id(&p, "target");
    assert_ne!(old_key, p.host.kernel().node(new).unwrap().key);
    assert_ne!(p.hit(hit.0, hit.1), Some(new));
    assert!(p.press_at(hit.0, hit.1, 3.).is_none());
    assert_eq!(count(&p), "0");
    assert_eq!(paints.get(), 1);
    assert!(complete(&mut p, &first));
    let remounted = submit(&mut p).unwrap();
    assert!(complete(&mut p, &remounted));
    assert_eq!(p.hit(hit.0, hit.1), Some(new));
}

#[test]
fn failed_paint_cannot_acknowledge_first_pixel_or_bind_stale_hit_boxes() {
    let (mut p, _, fail) = boot();
    p.frame();
    let target = id(&p, "target");
    let hit = point(&p, target);
    fail.set(true);
    let failed = submit(&mut p).unwrap();
    assert!(!p.last_frame_succeeded);
    assert_eq!(p.hit(hit.0, hit.1), None);
    assert!(complete(&mut p, &failed));
    assert!(!p.painted);
}

#[test]
fn headless_boxes_and_agent_layout_keep_the_existing_eager_frame_semantics() {
    let (mut p, paints, _) = boot();
    assert!(!p.boxes().is_empty());
    assert_eq!(paints.get(), 1);
    p.set_pointer(Some((12., 12.)));
    p.layout_json(None, false);
    assert_eq!(paints.get(), 2);
    assert!(!p.dirty());
    p.first_pixel();
    assert!(p.painted);
}

#[test]
fn foreign_receipt_does_not_release_or_acknowledge_another_presenter() {
    let (mut a, _, _) = boot();
    let (mut b, paints, _) = boot();
    let first = submit(&mut a).unwrap();
    let second = submit(&mut b).unwrap();
    assert!(!complete(&mut b, &first));
    assert!(!b.painted);
    assert!(submit(&mut b).is_none());
    assert_eq!(paints.get(), 1);
    assert!(complete(&mut b, &second));
    assert!(b.painted);
}

const VIEWPORT_APP: &str = r##"component App
  state extent = 600
  state draft = ""
  state count = 0
  action edit(value)
    draft = value
  action press
    count = count + 1
  action shorten
    extent = 100
  action lengthen
    extent = 700
  view
    column testId="root" width="100%" height=extent background-color="#225599"
      input testId="input" value=draft input=edit height=24
      box testId="shorten" press=shorten height=24
      box testId="lengthen" press=lengthen height=24
      box testId="target" press=press height=32 background-color="#cc3300"
      text `${count}` testId="count" height=20
"##;

fn viewport_action(p: &mut Presenter<Empty>, name: &str) {
    let view = id(p, name);
    assert!(p
        .host
        .dispatch_at(view, Event::Press, p.host.now())
        .is_none());
    assert!(p.after_commit().is_none());
}

fn acknowledged_root_page() -> (Fixture, SubmittedFrame) {
    let (mut p, paints, fail) = boot_app(VIEWPORT_APP);
    p.frame(); // Existing immediate/headless frame for the initial scroll intent.
    p.wheel_at(300., 200., 0., 60.);
    assert_eq!(p.page.1, 60.);
    let a = submit(&mut p).unwrap();
    assert!(complete(&mut p, &a));
    assert_eq!(a.pixels.height(), 240);
    ((p, paints, fail), a)
}

#[test]
fn viewport_resize_during_pending_b_keeps_a_root_scroll_then_b_after_ack() {
    let ((mut p, paints, _), a) = acknowledged_root_page();
    let key = p.host.kernel().node(id(&p, "root")).unwrap().key;
    viewport_action(&mut p, "shorten");
    assert!(p.resize(240., 160.).is_none());
    let b = submit(&mut p).unwrap();
    let b_pixels = b.pixels.data().to_vec();
    let painted = paints.get();
    assert_eq!((b.pixels.width(), b.pixels.height()), (240, 160));
    assert_eq!(p.page.1, 60.);

    // C changes both axes while A is still the acknowledged picture.
    assert!(p.resize(900., 900.).is_none());
    assert_eq!(p.viewport(), (900., 900.));
    assert_eq!(p.host.kernel().node(id(&p, "root")).unwrap().key, key);
    assert_eq!(p.page.1, 60., "C viewport must not clamp A's scroll intent");
    p.type_text(id(&p, "input"), "EXACT_🧪漢字").unwrap();
    p.wheel_at(300., 200., 0., 30.);
    assert_eq!(
        p.page.1, 90.,
        "A's viewport and document jointly bound input"
    );
    assert!(submit(&mut p).is_none());
    assert_eq!(paints.get(), painted);
    assert_eq!(b.pixels.data(), b_pixels);
    assert!(complete(&mut p, &b));
    assert_eq!(p.page.1, 0., "B ACK installs B's captured viewport/extent");
    assert!(p.dirty(), "C remains pending after B acknowledgement");
    assert!(p.box_of(id(&p, "root")).is_some());
    p.wheel_at(20., 120., 0., 30.);
    assert_eq!(p.page.1, 0., "between ACK and next submit still uses B");
    assert_eq!(paints.get(), painted);
    assert!(
        !complete(&mut p, &a),
        "old ACK cannot reinstall A's viewport"
    );
    assert_eq!(b.pixels.data(), b_pixels);
    let c = submit(&mut p).unwrap();
    assert_eq!((c.pixels.width(), c.pixels.height()), (900, 900));
    assert!(complete(&mut p, &c));
}

#[test]
fn viewport_short_a_cannot_gain_root_overflow_from_pending_resize() {
    let (mut p, paints, _) = boot_app(VIEWPORT_APP);
    viewport_action(&mut p, "shorten");
    let a = submit(&mut p).unwrap();
    assert!(complete(&mut p, &a));
    assert_eq!(p.page.1, 0.);
    viewport_action(&mut p, "lengthen");
    assert!(p.resize(300., 400.).is_none());
    let b = submit(&mut p).unwrap();
    assert!(p.resize(80., 80.).is_none());
    let before = paints.get();
    p.wheel_at(10., 10., 0., 30.);
    assert_eq!(p.page.1, 0., "A has no overflow; C cannot manufacture it");
    assert_eq!(paints.get(), before);
    assert!(complete(&mut p, &b));
    p.wheel_at(200., 200., 0., 1000.);
    assert_eq!(
        p.page.1, 300.,
        "700 document minus B's 400 viewport, not C's 80"
    );
    assert!(p.dirty());
}

#[test]
fn viewport_input_outside_acknowledged_surface_cannot_hit_or_scroll() {
    let (mut p, paints, _) = boot_app(VIEWPORT_APP);
    let a = submit(&mut p).unwrap();
    assert!(complete(&mut p, &a));
    assert!(p.resize(100., 100.).is_none());
    let before = paints.get();
    // Root content is 600 tall, but only the acknowledged 240px viewport exists.
    assert_eq!(p.hit(10., 300.), None);
    p.wheel_at(10., 300., 0., 40.);
    assert_eq!(
        p.page.1, 0.,
        "outside the visible surface is not root scrolling"
    );
    for (x, y) in [(320., 20.), (20., 240.), (-1., 10.), (f32::NAN, 10.)] {
        assert_eq!(p.hit(x, y), None);
        p.wheel_at(x, y, 0., 40.);
        assert_eq!(p.page.1, 0.);
    }
    assert_eq!(paints.get(), before);
}

#[test]
fn viewport_failed_b_and_rejected_resize_keep_a_until_valid_ack() {
    let ((mut p, paints, fail), a) = acknowledged_root_page();
    assert!(p.resize(f32::NAN, 160.).is_some());
    assert_eq!(p.viewport(), (320., 240.));
    assert!(p.resize(900., 900.).is_none());
    fail.set(true);
    let b = submit(&mut p).unwrap();
    assert!(!p.last_frame_succeeded);
    assert!(complete(&mut p, &b));
    assert_eq!(p.page.1, 60.);
    let before = paints.get();
    p.wheel_at(300., 200., 0., 30.);
    assert_eq!(p.page.1, 90., "failed B does not install its viewport");
    assert!(p.reload(b"invalid", Empty).is_err());
    assert_eq!(paints.get(), before);
    assert!(!complete(&mut p, &a));
    fail.set(false);
    let c = submit(&mut p).unwrap();
    assert!(complete(&mut p, &c));
    assert_eq!(p.page.1, 0.);
}

#[test]
fn viewport_reload_old_receipt_releases_without_installing_geometry() {
    let ((mut p, _, _), a) = acknowledged_root_page();
    assert!(p.resize(240., 160.).is_none());
    let b = submit(&mut p).unwrap();
    assert!(p.resize(900., 900.).is_none());
    p.reload(&contract::compile(VIEWPORT_APP).unwrap().encode(), Empty)
        .unwrap();
    let before = p.page;
    assert_eq!(p.hit(10., 10.), None);
    p.wheel_at(10., 10., 0., 30.);
    assert_eq!(p.page, before);
    assert!(complete(&mut p, &b));
    assert_eq!(
        p.hit(10., 10.),
        None,
        "old runtime's B is released, not adopted"
    );
    assert!(!complete(&mut p, &a));
    let c = submit(&mut p).unwrap();
    let weak_pixels = Arc::downgrade(&c.pixels);
    let weak_identity = Rc::downgrade(&c.identity);
    assert!(complete(&mut p, &c));
    assert_eq!(p.viewport(), (900., 900.));
    drop(c);
    assert!(weak_pixels.upgrade().is_none());
    assert!(
        weak_identity.upgrade().is_none(),
        "no acknowledged receipt history"
    );
}

#[test]
fn viewport_idle_resize_corrects_clamped_root_pixels_after_ack_without_c() {
    let ((mut p, paints, _), _) = acknowledged_root_page();
    let root = id(&p, "root");
    assert_eq!(p.box_of(root).unwrap().rect.1, -60.);
    assert!(p.resize(900., 900.).is_none());
    let b = submit(&mut p).unwrap();
    let b_pixels = b.pixels.data().to_vec();
    let count = paints.get();
    assert_eq!(p.page.1, 60., "pending resize keeps acknowledged A intent");
    assert_eq!(p.box_of(root).unwrap().rect.1, -60.);
    assert!(
        !p.dirty(),
        "no C, collection, input or timer keeps this alive"
    );
    assert!(complete(&mut p, &b));
    assert_eq!(p.page.1, 0.);
    let b_root = p.box_of(root).unwrap();
    assert!(
        b_root.rect.1 == 0. || p.dirty(),
        "ACK clamp must not leave permanent old-offset pixels in idle B"
    );
    if b_root.rect.1 != 0. {
        let correction = submit(&mut p).expect("dirty correction is paintable");
        assert!(complete(&mut p, &correction));
        assert_eq!(paints.get(), count + 1);
    }
    assert_eq!(p.box_of(root).unwrap().rect.1, 0.);
    assert!(p.hit(10., 10.).is_some());
    assert!(
        !p.dirty(),
        "one correction converges without a repaint loop"
    );
    assert_eq!(b.pixels.data(), b_pixels, "submitted pixels stay immutable");
}

#[test]
fn viewport_headless_resize_keeps_immediate_root_scroll_semantics() {
    let (mut p, _, _) = boot_app(VIEWPORT_APP);
    p.frame();
    p.wheel_at(300., 200., 0., 60.);
    assert_eq!(p.page.1, 60.);
    assert!(p.resize(900., 900.).is_none());
    assert_eq!(p.page.1, 0.);
    let pixels = p.frame();
    assert_eq!((pixels.width(), pixels.height()), (900, 900));
    assert!(!p.dirty());
}

// Timer repaint demand: real runner receipts, display ownership and side effects.
const IDLE_TIMER: &str = r##"component App
  state running = false
  state ticks = 0
  action tick
    if running
      ticks = ticks + 1
  task timer mount
    every(250, tick)
  view
    text `${ticks}` testId="ticks"
"##;
fn timer_primed(source: &str) -> Fixture {
    let (mut p, paints, fail) = boot_app(source);
    let a = submit(&mut p).unwrap();
    assert!(complete(&mut p, &a));
    assert!(!p.dirty());
    paints.set(0);
    (p, paints, fail)
}
#[test]
fn timer_demand_paused_due_receipt_does_not_repaint() {
    let plan = contract::compile(IDLE_TIMER).unwrap();
    let mut runner = exact_runner::Runner::boot(
        plan,
        Empty,
        exact_kernel::Kernel::new(Box::new(exact_kernel::MonospaceMeasurer::default())),
        exact_runner::Viewport::sized(320., 240.),
        "/",
    )
    .unwrap();
    let due = runner.advance_timed(250.);
    assert!(due.error.is_none());
    assert_eq!(due.receipts.len(), 1, "due timer really executes");
    let r = &due.receipts[0].receipt;
    assert!(r.created.is_empty() && r.destroyed.is_empty() && r.touched.is_empty());
    assert!(!r.layout_invalidated);
    let (mut p, paints, _) = timer_primed(IDLE_TIMER);
    let epoch = p.host.kernel().epoch();
    assert!(p.advance(250.).is_none());
    assert_eq!(p.host.now(), 250.);
    assert_eq!(p.host.kernel().epoch(), epoch);
    let submitted = if p.dirty() {
        let frame = submit(&mut p).unwrap();
        assert!(complete(&mut p, &frame));
        true
    } else {
        false
    };
    assert_eq!(
        paints.get(),
        0,
        "paused due timer must not repaint through the real loop gate"
    );
    assert!(!submitted && !p.dirty());
}
#[test]
fn timer_demand_empty_receipts_do_not_repaint_but_explicit_clock_does() {
    let (mut p, paints, _) = timer_primed(IDLE_TIMER);
    assert!(p.advance(100.).is_none());
    assert_eq!(p.host.now(), 100.);
    let submitted = if p.dirty() {
        let frame = submit(&mut p).unwrap();
        assert!(complete(&mut p, &frame));
        true
    } else {
        false
    };
    assert_eq!(
        paints.get(),
        0,
        "empty receipt must not repaint through the real loop gate"
    );
    assert!(!submitted && !p.dirty());
    assert_eq!(p.clock(125.), (125., None));
    assert!(p.dirty(), "explicit clock remains conservative");
}
#[test]
fn timer_demand_changed_timer_and_final_motion_frame_remain_visible() {
    let source = IDLE_TIMER.replace("running = false", "running = true")
        .replace("text `${ticks}` testId=\"ticks\"", "box testId=\"ticks\" scale=(1 + ticks) transition=\"scale 100ms linear\" width=30 height=30");
    let (mut p, _, _) = timer_primed(&source);
    assert!(p.advance(250.).is_none());
    assert!(p.dirty() && p.host.motion());
    let b = submit(&mut p).unwrap();
    assert!(complete(&mut p, &b));
    assert!(!p.dirty());
    assert!(p.advance(400.).is_none());
    assert!(!p.host.motion(), "final sample has settled");
    assert_eq!(p.host.presented(id(&p, "ticks")).scale, 2.);
    assert!(
        p.dirty(),
        "settling sample must still reach a final picture"
    );
}
#[test]
fn timer_demand_pending_b_keeps_dirty_c_and_b_pixels() {
    let (mut p, paints, _) = primed();
    let b = submit(&mut p).unwrap();
    let pixels = b.pixels.data().to_vec();
    p.type_text(id(&p, "input"), "C").unwrap();
    assert!(p.advance(100.).is_none());
    assert!(p.dirty());
    assert!(submit(&mut p).is_none());
    assert_eq!(b.pixels.data(), pixels);
    assert!(complete(&mut p, &b));
    assert!(p.dirty());
    let c = submit(&mut p).unwrap();
    assert!(complete(&mut p, &c));
    assert_eq!(paints.get(), 2);
}
#[test]
#[cfg(unix)]
#[allow(unsafe_code)] // poll borrows this live executor FD with a zero timeout; it takes no ownership.
fn timer_demand_nonvisual_command_wakes_existing_executor_without_paint() {
    let source = "component App\n  action tick\n    copyText(\"timer\")\n  task timer mount\n    every(250, tick)\n  view\n    text \"unchanged\"\n";
    let (mut p, _, _) = timer_primed(source);
    p.executor.begin_pump();
    assert!(p.advance(250.).is_none());
    assert_eq!(p.commands.len(), 1);
    assert_eq!(p.commands[0].name, "copyText");
    let mut fd = libc::pollfd {
        fd: p.executor_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    assert_eq!(
        unsafe { libc::poll(&mut fd, 1, 0) },
        1,
        "queued command must wake the loop before its next timer"
    );
    assert!(!p.dirty(), "nonvisual command alone needs no picture");
    p.run_commands(Empty::default);
    assert!(p.commands.is_empty());
}
#[test]
fn timer_demand_scheme_command_marks_its_visual_effect() {
    let source = "component App\n  action tick\n    setScheme(\"dark\")\n  task timer mount\n    every(250, tick)\n  view\n    text \"unchanged\" color=\"light-dark(#000000,#ffffff)\"\n";
    let (mut p, _, _) = timer_primed(source);
    assert!(p.advance(250.).is_none());
    p.run_commands(Empty::default);
    assert!(p.brush.dark && p.dirty());
}
#[test]
fn timer_root_font_size_command_is_laid_out_by_the_runner() {
    // LLP 1069.000 D3: the app's `:root { font-size }`; this host has nothing to run.
    let source = "component App\n  action tick\n    setRootFontSize(20)\n  task timer mount\n    every(250, tick)\n  view\n    view width=\"10rem\" height=\"2rem\"\n";
    let (mut p, _, _) = timer_primed(source);
    assert!(p.advance(250.).is_none());
    assert_eq!(p.host.runner().root_font_size(), 20.0);
    assert!(p.dirty());
    p.run_commands(Empty::default);
    assert!(p.commands.is_empty());
    // The host's own 16 stays beneath it, for a new runner of this launch.
    assert_eq!(p.host.runner().host_root_font_size(), 16.0);
}
#[test]
fn timer_demand_focus_retirement_and_scroll_clamp_are_independent() {
    for focus_only in [true, false] {
        let (mut p, _, _) = timer_primed(IDLE_TIMER);
        if focus_only {
            p.focus = Some(u32::MAX);
        } else {
            p.page = (0., 60.);
        }
        assert!(p.advance(100.).is_none());
        assert!(p.focus.is_none());
        assert_eq!(p.page, (0., 0.));
        assert!(
            p.dirty(),
            "focus retirement and clamp must each invalidate independently"
        );
    }
}
#[test]
fn timer_demand_router_change_is_consumed_even_without_view_edits() {
    let source = "routes nav\n  home \"/\"\n    post \"/post\"\ncomponent App\n  action tick\n    nav = open(nav, \"/post\")\n  task timer mount\n    every(250, tick)\n  view\n    text \"unchanged shell\"\n";
    let (mut p, _, _) = timer_primed(source);
    assert!(p.advance(250.).is_none());
    assert!(p.host.router_op().is_some());
    assert!(p.dirty(), "consumed navigation remains conservative");
}

struct TimerMeasure {
    fail: Rc<Cell<bool>>,
}
impl exact_kernel::TextMeasurer for TimerMeasure {
    fn measure(&mut self, r: &exact_kernel::TextMeasureRequest<'_>) -> exact_kernel::TextMetrics {
        let mut metrics = exact_kernel::MonospaceMeasurer::default().measure(r);
        if self.fail.get() {
            metrics.height = f32::NAN;
        }
        metrics
    }
}
#[test]
fn timer_demand_failed_layout_recovers_and_marks_actual_geometry() {
    let source =
        "component App\n  view\n    text \"recover geometry\" testId=\"text\" width=\"100%\"\n";
    let (mut p, _, _) = boot_app(source);
    let fail = Rc::new(Cell::new(false));
    p.host = Host::boot(
        &contract::compile(source).unwrap().encode(),
        Empty,
        Box::new(TimerMeasure { fail: fail.clone() }),
        320.,
        240.,
    )
    .unwrap()
    .0;
    let a = submit(&mut p).unwrap();
    assert!(complete(&mut p, &a));
    let text = id(&p, "text");
    assert_eq!(p.host.kernel().node(text).unwrap().frame.width, 320.);
    fail.set(true);
    assert!(p.host.resize(160., 240.).is_some());
    assert!(
        p.advance(50.).is_some(),
        "failed recovery is not silently clean"
    );
    let failed = submit(&mut p).unwrap();
    assert!(complete(&mut p, &failed));
    fail.set(false);
    assert!(p.advance(100.).is_none());
    assert_eq!(p.host.kernel().node(text).unwrap().frame.width, 160.);
    assert!(p.dirty(), "empty receipt recovery still changes geometry");
}
#[test]
fn timer_demand_image_report_stays_dirty_through_empty_advance() {
    let (mut p, _, _) = timer_primed("component App\n  view\n    image testId=\"image\"\n");
    let image = id(&p, "image");
    assert!(p.apply_reports(vec![(image, Some((32., 48.)))]));
    assert!(p.dirty());
    assert!(p.advance(100.).is_none());
    assert!(p.dirty());
    let b = submit(&mut p).unwrap();
    assert!(complete(&mut p, &b));
}
struct TimerRows;
impl DataSource for TimerRows {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        assert_eq!(name, "timerRows");
        Ok(Value::list(
            (0..200).map(|n| Value::Number(n as f64)).collect(),
        ))
    }
}
#[test]
fn timer_demand_collection_feedback_still_refines_after_empty_advance() {
    let source = "component App\n  resource rows = timerRows() as shape list<number>\n  view\n    list virtualized=true width=320 height=180 testId=\"port\"\n      each x in rows key=x\n        text `${x}` height=24\n";
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        TimerRows,
        (320., 240.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    for _ in 0..16 {
        assert!(p.pump(0.).is_none());
        if p.dirty() {
            let f = submit(&mut p).unwrap();
            assert!(complete(&mut p, &f));
        }
    }
    let before = p.host.runner().collections().pop().unwrap();
    assert!(before.rows.iter().all(|r| r.measured));
    let port = before.view;
    let bounds = p.box_of(port).unwrap().rect;
    p.wheel_at(bounds.0 + 2., bounds.1 + 2., 0., 480.);
    assert!(p.advance(100.).is_none());
    assert!(p.dirty());
    for _ in 0..8 {
        if p.dirty() {
            let f = submit(&mut p).unwrap();
            assert!(complete(&mut p, &f));
        }
        assert!(p.advance(100.).is_none());
    }
    let after = p.host.runner().collections().pop().unwrap();
    assert!(after.rows[0].index > before.rows[0].index);
    assert!(after.rows.iter().all(|r| r.measured));
}
#[test]
fn timer_demand_content_region_remains_conservative_for_same_shell() {
    let _service = crate::content_region::test_service();
    let source = "component App\n  view\n    view id=\"owner\" width=320 height=240 overflow-x=\"hidden\" overflow-y=\"hidden\"\n      scroll id=\"content\" width=320 height=240\n        text \"immutable paragraph\" testId=\"paragraph\"\n      text \"Preparing content\" id=\"pending\" position=\"absolute\"\n";
    let (mut p, error) = Presenter::boot_with_content_region(
        &contract::compile(source).unwrap().encode(),
        Empty,
        (320., 240.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
        crate::content_region::ContentRegionRegistration {
            activate: None,
            owner: "owner",
            content: "content",
            pending: "pending",
        },
    )
    .unwrap();
    assert!(error.is_none());
    // The real placeholder ACK starts the region's font/source worker.
    let placeholder = submit(&mut p).unwrap();
    assert!(complete(&mut p, &placeholder));
    let end = std::time::Instant::now() + Duration::from_secs(10);
    while !p
        .host
        .content_region()
        .unwrap()
        .receipt()
        .is_some_and(|r| r.current)
    {
        assert!(std::time::Instant::now() < end);
        assert!(p.poll_content_region().is_none());
        std::thread::yield_now();
    }
    let a = submit(&mut p).unwrap();
    assert!(complete(&mut p, &a));
    assert!(!p.dirty());
    assert!(p.advance(100.).is_none());
    assert!(
        p.dirty(),
        "geometry equality alone cannot certify a region publication"
    );
}

// Real Runner corrections, ordinary feedback and acknowledged-picture bounds.
// No correction, collection sequence, or Presenter scroll offset is injected.
#[path = "follow_end_growth_tests.rs"]
mod follow_end_growth;

// @ref LLP 1073 D2, D4, D5 — a frame task keeps the display pump running and
// fires once per display frame, at the frame's time; once frames are presented
// the timer path fires none and owes no deadline for them; a stall is not
// caught up.
#[test]
fn a_frame_task_fires_once_per_display_frame_not_from_a_timeout() {
    let (mut p, _, _) = boot_app(
        r#"component App
  state frames = 0
  state at = 0
  action step
    frames = frames + 1
    at = now()
  task ticker mount
    every(frame, step)
  view
    text `${frames}` testId="frames" width=100 height=20
"#,
    );
    let a = submit(&mut p).unwrap();
    assert!(complete(&mut p, &a));
    assert!(p.needs_animation_frame());
    let slot = |p: &Presenter<Empty>, name| p.host.runner().slot(name).cloned();
    assert!(p.animation_frame(10.).is_none());
    assert_eq!(slot(&p, "frames"), Some(Value::Number(1.)));
    assert_eq!(p.host.now(), 10.);
    assert!(p.dirty());
    assert_eq!(p.host.timer_due_ms(), None);
    assert!(p.advance(1_000.).is_none());
    assert_eq!(slot(&p, "frames"), Some(Value::Number(1.)));
    let at = slot(&p, "at");
    assert!(p.animation_frame(5_000.).is_none());
    assert_eq!(slot(&p, "frames"), Some(Value::Number(2.)));
    assert_ne!(slot(&p, "at"), at);
}

const SVG_APP: &str = r##"component App
  state count = 0
  action press
    count = count + 1
  view
    column width=320 height=240
      button press=press testId="icon" width=48 height=48
        svg width=48 height=48 viewBox="0 0 24 24"
          path d="M0 0h24v24H0z" fill="#111827"
      text `${count}` testId="count" height=20
"##;

#[test]
fn a_press_on_an_svg_element_reaches_its_button_on_an_attached_display() {
    let (mut p, _, _) = boot_app(SVG_APP);
    let a = submit(&mut p).unwrap();
    assert!(complete(&mut p, &a));
    let icon = id(&p, "icon");
    let (x, y) = point(&p, icon);
    let (x, y) = (x + 20., y + 20.);
    let hit = p.hit(x, y).unwrap();
    assert_eq!(
        p.host.kernel().node(hit).unwrap().node_type,
        NodeType::SvgPath
    );
    assert_eq!(p.press_at(x, y, 1.), Some(icon));
    assert_eq!(count(&p), "1");
}
