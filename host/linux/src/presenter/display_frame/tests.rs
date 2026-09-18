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
  action edit(value) writes draft
    draft = value
  action press writes count
    count = count + 1
  action hide writes showing
    showing = false
  action show writes showing
    showing = true
  action tick writes ticks
    ticks = ticks + 1
  action request writes result
    send result = work()
  task timer mount
    every(250, tick)
  view
    column width=320 height=240
      input value=draft change=edit testId="input" height=32
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
    fn stroke(&mut self, s: &Shape, w: f32, c: [u8; 4], t: Transform) {
        self.inner.stroke(s, w, c, t);
    }
    fn image(&mut self, b: &Arc<Bitmap>, r: Rect4, c: &[Shape], t: Transform) {
        self.inner.image(b, r, c, t);
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
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(APP).unwrap().encode(),
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
            Some(Box::new(move || {
                gate.recv_timeout(Duration::from_secs(5)).unwrap();
                Outcome::Response(Response {
                    status: 200,
                    headers: vec![],
                    body: vec![],
                })
            })),
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
    p.layout_json(None);
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
