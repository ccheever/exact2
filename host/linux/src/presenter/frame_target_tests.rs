use super::*;
use crate::image::Bitmap;
use crate::paint::{FrameTarget, Shape};
use crate::text::{Paragraph, RunPaint};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tiny_skia::Transform;

struct PresentPaint {
    inner: Raster,
    targets: Rc<RefCell<Vec<FrameTarget>>>,
    omit_capture: Rc<Cell<bool>>,
}
impl Backend for PresentPaint {
    fn name(&self) -> &'static str {
        "test-presenter"
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
    let frame = p.frame_for(FrameTarget::Window).unwrap();
    assert_eq!((frame.width(), frame.height()), (320, 240));
    assert!(p.last_frame_succeeded());
}
