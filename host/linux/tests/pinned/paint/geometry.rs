//! C3 removes this adapter when Ink can draw text; every other CPU operation stays.
use exact_linux::image::Bitmap;
use exact_linux::paint::{border::BorderFill, Backend, GradientPaint, Rect4, Shape, SvgPaint};
use exact_linux::raster::Raster;
use exact_linux::text::{Paragraph, RunPaint, TextEngine};
use std::sync::Arc;
use tiny_skia::{Pixmap, Transform};

pub(super) struct WithoutText(pub Raster);
impl Backend for WithoutText {
    fn name(&self) -> &'static str {
        "cpu"
    }
    fn page(&mut self, c: [u8; 4]) {
        self.0.page(c);
    }
    fn begin(&mut self, w: f32, h: f32, s: f32) {
        self.0.begin(w, h, s);
    }
    fn fill(&mut self, shape: &Shape, c: [u8; 4], ts: Transform) {
        self.0.fill(shape, c, ts);
    }
    fn fill_gradient(&mut self, shape: &Shape, g: &GradientPaint, ts: Transform) {
        self.0.fill_gradient(shape, g, ts);
    }
    fn fill_border(&mut self, p: &BorderFill, ts: Transform) {
        self.0.fill_border(p, ts);
    }
    fn image(
        &mut self,
        image: &Arc<Bitmap>,
        dst: Rect4,
        clips: &[Shape],
        ts: Transform,
        tint: Option<[u8; 4]>,
    ) {
        self.0.image(image, dst, clips, ts, tint);
    }
    fn island_image(&mut self, image: Arc<Pixmap>, dst: Rect4, ts: Transform, mode: u8) {
        self.0.island_image(image, dst, ts, mode);
    }
    fn surface_image(&mut self, image: Arc<Pixmap>, dst: Rect4) {
        self.0.surface_image(image, dst);
    }
    fn canvas(&mut self, image: &Arc<Pixmap>, dst: Rect4, clips: &[Shape], ts: Transform) {
        self.0.canvas(image, dst, clips, ts);
    }
    fn text(
        &mut self,
        _: &mut TextEngine,
        _: &Paragraph,
        _: &[RunPaint],
        _: (f32, f32),
        _: Transform,
    ) {
    }
    fn svg_path(&mut self, shape: &SvgPaint<'_>, ts: Transform) {
        self.0.svg_path(shape, ts);
    }
    fn push_svg_clip(&mut self, clip: &exact_kernel::svg::scene::Clip, ts: Transform) -> usize {
        self.0.push_svg_clip(clip, ts)
    }
    fn push_css_clip(&mut self, path: &exact_kernel::clip::ClipPath, ts: Transform) -> bool {
        self.0.push_css_clip(path, ts)
    }
    fn push_clip(&mut self, shape: &Shape, ts: Transform) {
        self.0.push_clip(shape, ts);
    }
    fn pop_clip(&mut self) {
        self.0.pop_clip();
    }
    fn push_opacity(&mut self, alpha: f32) {
        self.0.push_opacity(alpha);
    }
    fn pop_opacity(&mut self) {
        self.0.pop_opacity();
    }
    fn push_mask(&mut self, shape: &Shape, ts: Transform) {
        self.0.push_mask(shape, ts);
    }
    fn pop_mask(&mut self, shape: &Shape, mask: &Result<GradientPaint, [u8; 4]>, ts: Transform) {
        self.0.pop_mask(shape, mask, ts);
    }
    fn backdrop_filter(
        &mut self,
        shape: &Shape,
        filter: &exact_kernel::style::BackdropFilter,
        ts: Transform,
    ) {
        self.0.backdrop_filter(shape, filter, ts);
    }
    fn pointer(&mut self, x: f32, y: f32) {
        self.0.pointer(x, y);
    }
    fn finish(&mut self) -> Result<Pixmap, String> {
        self.0.finish()
    }
}
