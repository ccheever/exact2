use super::*;
use crate::image::Bitmap;
use crate::paint::{FrameTarget, Shape};
use crate::text::{Paragraph, RunPaint};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tiny_skia::Transform;

struct PresentPaint {
    name: &'static str,
    lost: bool,
    inner: Raster,
    targets: Rc<RefCell<Vec<FrameTarget>>>,
    omit_capture: Rc<Cell<bool>>,
}
impl Backend for PresentPaint {
    fn lost_device_needs_cpu(&self) -> bool {
        self.lost
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn begin(&mut self, w: f32, h: f32, s: f32) {
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
        panic!("the consumer must select a target")
    }
    fn finish_for(&mut self, target: FrameTarget) -> Result<Option<Pixmap>, String> {
        self.targets.borrow_mut().push(target);
        if target == FrameTarget::Pixels && !self.omit_capture.get() {
            self.inner.finish().map(Some)
        } else {
            Ok(None)
        }
    }
}

#[test]
fn window_geometry_and_capture_have_distinct_consumers() {
    let source = r##"component App
  view
    box width=320 height=240 background-color="#cc3300" testId="target"
"##;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (320., 240.),
        2.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    let targets = Rc::new(RefCell::new(Vec::new()));
    let omit_capture = Rc::new(Cell::new(false));
    p.brush.replace_backend(Box::new(PresentPaint {
        lost: false,
        name: "test-presenter",
        inner: Raster::new(),
        targets: targets.clone(),
        omit_capture: omit_capture.clone(),
    }));

    // The same public query used by hit testing must never present a window.
    assert!(!p.boxes().is_empty());
    assert_eq!(*targets.borrow(), [FrameTarget::Geometry]);
    let key = p.host.kernel().find_by_test_id("target")[0];
    let id = p.host.kernel().node_by_key(key).unwrap().id;
    assert_eq!(p.hit(100., 100.), Some(id));
    assert_eq!(targets.borrow().len(), 1);

    p.repaint();
    assert!(p.frame_for(FrameTarget::Window).is_none());
    assert!(p.last_frame_succeeded());
    assert_eq!(targets.borrow().last(), Some(&FrameTarget::Window));
    assert_eq!(p.hit(100., 100.), Some(id));

    let pixels = p.frame();
    assert_eq!((pixels.width(), pixels.height()), (640, 480));
    assert_eq!(pixels.pixel(200, 200).unwrap().red(), 204);
    assert_eq!(pixels.pixel(200, 200).unwrap().alpha(), 255);
    assert_eq!(targets.borrow().last(), Some(&FrameTarget::Pixels));

    omit_capture.set(true);
    let fallback = p.frame();
    assert!(
        !p.last_frame_succeeded(),
        "missing capture pixels are a paint failure"
    );
    assert_eq!((fallback.width(), fallback.height()), (640, 480));
    assert_eq!(
        p.hit(100., 100.),
        Some(id),
        "failed capture preserves accepted hits"
    );
}

#[test]
fn cpu_windows_still_return_complete_pixels() {
    let (mut p, _) = Presenter::boot_with(
        &crate::test_plan("component App\n  view\n    text \"CPU frame\"\n"),
        (),
        (320., 240.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(p.frame_for(FrameTarget::Geometry).is_none());
    assert!(p.last_frame_succeeded());
    let frame = p.frame_for(FrameTarget::Window).unwrap();
    assert_eq!((frame.width(), frame.height()), (320, 240));
    assert!(p.last_frame_succeeded());
}

#[test]
fn geometry_without_pixels_preserves_hits_fields_and_the_next_image() {
    let source = r##"component App
  view
    column width="100%" height="100%" background-color="#f2f2f2"
      box testId="list" width="100%" height=120 flex-shrink=0 overflow-y="auto" border-radius=12
        column height=600 opacity=0.65 background-color="#773399"
          text "Scrollable content" font-size=18
          box height=200 background-color="#22aa88"
          text "Further down" font-size=22
      textarea testId="field" value="Draft text" width=260 height=60 appearance="none"
"##;
    for scale in [1., 1.25, 1.5, 2.] {
        let boot = || {
            let (mut p, error) = Presenter::boot_with(
                &crate::test_plan(source),
                (),
                (320., 240.),
                scale,
                PathBuf::new(),
                PainterChoice::Cpu,
            )
            .unwrap();
            assert!(error.is_none(), "{error:?}");
            p.brush.scrollbars.enabled = true;
            p
        };
        let mut geometry = boot();
        let mut painted = boot();
        for phase in 0..4 {
            for p in [&mut geometry, &mut painted] {
                let kernel = p.host.kernel();
                let id = |name| {
                    kernel
                        .node_by_key(kernel.find_by_test_id(name)[0])
                        .unwrap()
                        .id
                };
                let (list, field) = (id("list"), id("field"));
                match phase {
                    1 => {
                        p.scroll.insert(list, (0., 175.));
                        p.set_focus(Some(field), 0.);
                    }
                    2 => {
                        assert!(p.ime_begin(p.ime_target().unwrap()));
                        p.ime_preedit("日本", Some((6, 6)));
                    }
                    3 => {
                        p.ime_commit("日本");
                        assert!(p.resize_scaled(360., 280., scale).is_none());
                    }
                    _ => {}
                }
                p.repaint();
            }
            assert!(geometry.frame_for(FrameTarget::Geometry).is_none());
            assert!(geometry.last_frame_succeeded());
            let expected = painted.frame();
            let boxes = |p: &Presenter<()>| {
                p.boxes
                    .iter()
                    .map(|b| (b.id, b.rect, b.clip, b.scroll, b.pointer_hit))
                    .collect::<Vec<_>>()
            };
            assert_eq!(boxes(&geometry), boxes(&painted));
            assert!(!geometry.brush.scrollbars.bars.is_empty());
            let bars = |p: &Presenter<()>| {
                p.brush
                    .scrollbars
                    .bars
                    .iter()
                    .map(|b| (b.id, b.track, b.thumb, b.offset, b.max))
                    .collect::<Vec<_>>()
            };
            assert_eq!(bars(&geometry), bars(&painted));
            assert_eq!(geometry.caret_frame(), painted.caret_frame());
            assert_eq!(geometry.ime_cursor_area(), painted.ime_cursor_area());
            if phase > 0 {
                assert!(geometry.caret_frame().is_some());
            }
            for (x, y) in [(20., 20.), (150., 80.), (100., 140.), (310., 210.)] {
                assert_eq!(geometry.hit(x, y), painted.hit(x, y));
            }
            let actual = geometry.frame_for(FrameTarget::Window).unwrap();
            assert_eq!(
                actual.data(),
                expected.data(),
                "scale {scale}, phase {phase}"
            );
            assert_eq!(geometry.frame().data(), expected.data());
        }
    }
}

#[test]
fn explicit_ink_refuses_paint_errors_but_a_lost_readback_device_repaints_on_cpu() {
    for (choice, lost) in [
        (PainterChoice::Ink, false),
        (PainterChoice::Auto, false),
        (PainterChoice::Ink, true),
    ] {
        let (mut p, _) = Presenter::boot_with(
            &crate::test_plan("component App\n  view\n    text \"Frame\"\n"),
            (),
            (32., 32.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        p.choice = choice;
        p.brush.replace_backend(Box::new(PresentPaint {
            lost,
            name: "ink",
            inner: Raster::new(),
            targets: Default::default(),
            omit_capture: Rc::new(Cell::new(true)),
        }));
        p.frame();
        assert_eq!(
            p.last_frame_succeeded(),
            choice == PainterChoice::Auto || lost
        );
        assert_eq!(
            p.brush.backend(),
            if choice == PainterChoice::Auto || lost {
                "cpu"
            } else {
                "ink"
            }
        );
    }
}
