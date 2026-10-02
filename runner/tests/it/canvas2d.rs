//! Canvas 2D's protocol in the runner (LLP 1056 D4, D5): 2D surfaces never
//! reach the GPU side-output; a draw needs geometry; a new size is a new
//! generation with a fresh recorder; a throw keeps what it drew before;
//! a stale reply is discarded; frames are drawn once per clock value.
use exact_canvas::{list, Context2d, DrawError, Frame};
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    agent, DataError, DataSource, DrawReply, DrawRequest, Drawn, Geometry, Limits, Runner,
};
use std::cell::RefCell;
use std::rc::Rc;

const SOURCE: &str = "component App
  state n = 1
  state show = true
  action bump
    n = n + 1
  action hide
    show = false
  view
    column
      when show
        canvas surface=spark(n) width=100 height=50 testId=\"c\"
      canvas surface=globe(n) testId=\"g\"
";

#[derive(Default)]
struct Log {
    frames: Vec<Frame>,
    later: bool,
    /// Draw images and text instead.
    pictures: bool,
    parked: Vec<(u64, u32, u64)>,
    retired: Vec<(u64, u32)>,
}

struct Spark(Rc<RefCell<Log>>);

impl DataSource for Spark {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("unexpected {name}")
    }
    fn canvas_surfaces(&self) -> Vec<(String, usize)> {
        vec![("spark".into(), 1)]
    }
    fn draw_2d(
        &mut self,
        surface: &str,
        args: &[Value],
        ctx: &Context2d,
        frame: &Frame,
    ) -> Result<bool, DrawError> {
        assert_eq!(surface, "spark");
        self.0.borrow_mut().frames.push(*frame);
        let n = match args[0] {
            Value::Number(n) => n,
            _ => panic!(),
        };
        if self.0.borrow().pictures {
            ctx.draw_image_with_image_handle("photo.png", 0.0, 0.0)?;
            ctx.draw_image_with_image_handle("gone.png", 0.0, 0.0)?;
            ctx.fill_text("hi", 1.0, 9.0)?;
            return Ok(false);
        }
        ctx.translate(1.0, 0.0)?;
        ctx.fill_rect(0.0, 0.0, n, n);
        if n == 3.0 {
            ctx.arc(0.0, 0.0, -1.0, 0.0, 1.0)?;
        }
        Ok(n == 4.0)
    }
    fn draw(&mut self, request: &DrawRequest<'_>, ctx: &Context2d) -> Drawn {
        if self.0.borrow().later {
            self.0
                .borrow_mut()
                .parked
                .push((request.canvas, request.generation, request.seq));
            return Drawn::Later;
        }
        let result = self.draw_2d(request.surface, request.args, ctx, &request.frame);
        Drawn::Now(DrawReply {
            lists: ctx.take_lists(),
            wants_frame: matches!(result, Ok(true)),
            error: result.err().map(|e| e.to_string()),
            notes: ctx.take_notes(),
        })
    }
    fn canvases_retired(&mut self, retired: &[(u64, u32)]) {
        self.0.borrow_mut().retired.extend_from_slice(retired);
    }
}

fn boot() -> (Runner<Spark>, Rc<RefCell<Log>>) {
    let log = Rc::new(RefCell::new(Log::default()));
    let plan = contract::compile(SOURCE).unwrap();
    let r = Runner::boot(
        plan,
        Spark(log.clone()),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    (r, log)
}

fn geometry(w: f64, h: f64, scale: f64) -> Geometry {
    Geometry {
        width: w,
        height: h,
        scale,
        bitmap: None,
    }
}

fn ops(bytes: &[u8]) -> Vec<String> {
    list::records(bytes)
        .unwrap()
        .iter()
        .map(|r| r.op.name().to_string())
        .collect()
}

#[test]
fn a_2d_surface_draws_by_generation_and_never_reaches_the_gpu_side_output() {
    let (mut r, log) = boot();
    let gpu = r.take_surface_updates();
    assert_eq!(gpu.len(), 1, "only globe is the GPU module's");
    assert_eq!(gpu[0].name, "globe");
    let views = r.canvas_views();
    assert_eq!(views.len(), 1);
    let view = views[0];

    // No geometry, no draw.
    r.draw_canvases(&|_| true);
    assert!(r.take_canvas_lists().is_empty());

    r.set_canvas_geometry(view, geometry(100.0, 50.0, 2.0));
    r.draw_canvases(&|_| true);
    let lists = r.take_canvas_lists();
    assert_eq!(lists.len(), 2, "the fresh generation, then the mount draw");
    assert!(lists[0].fresh && lists[0].lists.is_empty());
    assert_eq!(
        (lists[0].pixel_width, lists[0].pixel_height, lists[0].scale),
        (200, 100, 2.0)
    );
    assert_eq!(ops(&lists[1].lists[0]), ["SetTransform", "FillRect"]);
    let f = log.borrow().frames[0];
    assert_eq!(
        (f.cause.primary(), f.width, f.pixel_width),
        ("mount", 100.0, 200)
    );

    // Args: the recorder persists — the transform is cumulative.
    r.act("bump", vec![]).unwrap();
    r.draw_canvases(&|_| true);
    let lists = r.take_canvas_lists();
    let recs = list::records(&lists[0].lists[0]).unwrap();
    assert_eq!(
        recs[0].at(4),
        2.0,
        "translate(1, 0) twice in one generation"
    );
    assert_eq!(log.borrow().frames[1].cause.primary(), "args");

    // A size change: a new generation, a fresh recorder, the old retired.
    r.set_canvas_geometry(view, geometry(100.0, 50.0, 3.0));
    r.draw_canvases(&|_| true);
    let lists = r.take_canvas_lists();
    assert!(lists[0].fresh && lists[0].generation == 1);
    let recs = list::records(&lists[1].lists[0]).unwrap();
    assert_eq!(recs[0].at(4), 1.0, "a new generation starts at identity");
    assert_eq!(log.borrow().retired.len(), 1);
    // Rounding to the same store is not a new generation.
    r.set_canvas_geometry(view, geometry(100.1, 50.0, 3.0));
    r.draw_canvases(&|_| true);
    assert!(r.take_canvas_lists().is_empty());

    // A throw keeps what was drawn before it (r3: not atomic).
    r.act("bump", vec![]).unwrap();
    r.draw_canvases(&|_| true);
    let lists = r.take_canvas_lists();
    assert_eq!(ops(&lists[0].lists[0]), ["SetTransform", "FillRect"]);
    let state = agent::state(&r);
    assert!(state.contains("IndexSizeError"), "{state}");

    // n = 4 asks for frames: one per clock value, only while on screen.
    r.act("bump", vec![]).unwrap();
    r.draw_canvases(&|_| true);
    r.take_canvas_lists();
    assert!(r.canvas_wants_frame());
    r.canvas_frame();
    r.draw_canvases(&|_| false);
    assert!(r.take_canvas_lists().is_empty(), "held off screen");
    r.draw_canvases(&|_| true);
    assert_eq!(r.take_canvas_lists().len(), 1);
    r.canvas_frame();
    r.draw_canvases(&|_| true);
    assert!(r.take_canvas_lists().is_empty(), "once per clock value");
    r.advance(16.0).unwrap();
    r.canvas_frame();
    r.draw_canvases(&|_| true);
    assert_eq!(r.take_canvas_lists().len(), 1);
    assert_eq!(log.borrow().frames.last().unwrap().cause.primary(), "frame");

    // Unmounting drops the record and retires its generation.
    r.act("hide", vec![]).unwrap();
    assert!(r.canvas_views().is_empty());
    r.draw_canvases(&|_| true);
    assert_eq!(log.borrow().retired.len(), 2);
}

#[test]
fn a_later_reply_applies_only_with_live_stamps() {
    let (mut r, log) = boot();
    log.borrow_mut().later = true;
    let view = r.canvas_views()[0];
    r.set_canvas_geometry(view, geometry(10.0, 10.0, 1.0));
    r.draw_canvases(&|_| true);
    assert!(r.take_canvas_lists()[0].fresh);
    let (canvas, generation, seq) = log.borrow().parked[0];
    // A resize before the reply lands: the reply is discarded.
    r.set_canvas_geometry(view, geometry(20.0, 10.0, 1.0));
    let ctx = Context2d::new();
    ctx.fill_rect(0.0, 0.0, 1.0, 1.0);
    let reply = DrawReply {
        lists: ctx.take_lists(),
        ..Default::default()
    };
    assert!(r
        .canvas_reply(canvas, generation, seq, reply.clone())
        .is_err());
    r.draw_canvases(&|_| true);
    let (c2, g2, s2) = log.borrow().parked[1];
    assert_eq!((c2, g2), (canvas, generation + 1));
    assert!(r.canvas_reply(c2, g2, s2, reply.clone()).is_ok());
    let lists = r.take_canvas_lists();
    assert!(lists[0].fresh && !lists[1].fresh);
    // A draw that asked for no frame does not animate (LLP 1056 §8.4).
    assert!(!lists[0].animating && !lists[1].animating);
    // The same stamps twice: the second is stale.
    assert!(r.canvas_reply(c2, g2, s2, reply.clone()).is_err());
    // One that asks for the next frame does, on the list it returns.
    r.set_canvas_geometry(view, geometry(40.0, 20.0, 2.0));
    r.draw_canvases(&|_| true);
    let (c3, g3, s3) = *log.borrow().parked.last().unwrap();
    let asks = DrawReply {
        wants_frame: true,
        ..reply
    };
    assert!(r.canvas_reply(c3, g3, s3, asks).is_ok());
    assert!(r
        .take_canvas_lists()
        .iter()
        .any(|l| !l.fresh && l.animating));
}

#[test]
fn sizes_past_the_limits_are_refused_through_state() {
    let (mut r, _) = boot();
    r.set_canvas_limits(Limits {
        max_side: 65_535,
        max_area: 67_108_864,
        budget: 1 << 20,
    });
    let view = r.canvas_views()[0];
    r.set_canvas_geometry(view, geometry(1000.0, 1000.0, 1.0));
    r.draw_canvases(&|_| true);
    let lists = r.take_canvas_lists();
    assert_eq!(
        (lists.len(), lists[0].pixel_width),
        (1, 0),
        "no bitmap, no draw"
    );
    assert!(agent::state(&r).contains("MiB budget"));
    r.set_canvas_geometry(view, geometry(9000.0, 9000.0, 1.0));
    assert!(agent::state(&r).contains("area limit"));
}

#[test]
fn an_image_a_draw_asks_for_redraws_it_when_it_decodes() {
    let (mut r, log) = boot();
    log.borrow_mut().pictures = true;
    let view = r.canvas_views()[0];
    r.set_canvas_geometry(view, geometry(10.0, 10.0, 1.0));
    r.draw_canvases(&|_| true);
    let lists = r.take_canvas_lists();
    assert_eq!(
        ops(&lists[1].lists[0]),
        ["Font", "FillText"],
        "no image yet"
    );
    let mut asked = r.take_canvas_image_requests();
    asked.sort();
    assert_eq!(asked, ["gone.png", "photo.png"]);
    assert!(r.canvas_images_pending());
    r.canvas_image("photo.png", Ok((4, 2)), &[]);
    r.canvas_image("gone.png", Err("404".into()), &[]);
    assert!(!r.canvas_images_pending());
    r.draw_canvases(&|_| true);
    let lists = r.take_canvas_lists();
    assert_eq!(ops(&lists[0].lists[0]), ["Image", "DrawImage", "FillText"]);
    assert_eq!(log.borrow().frames.last().unwrap().cause.primary(), "image");
    assert!(r.take_canvas_image_requests().is_empty(), "asked once");
    assert!(agent::state(&r).contains("gone.png: 404"));
    // A font that loads redraws a canvas that drew text.
    r.canvas_fonts_loaded();
    r.draw_canvases(&|_| true);
    assert_eq!(r.take_canvas_lists().len(), 1);
    assert_eq!(log.borrow().frames.last().unwrap().cause.primary(), "font");
}
