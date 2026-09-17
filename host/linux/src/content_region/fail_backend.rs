use crate::{
    image::Bitmap,
    paint::{Backend, Rect4, Shape},
    text::{Paragraph, RunPaint, TextEngine},
};
use std::sync::Arc;
use tiny_skia::{Pixmap, Transform};
pub struct Failure;
impl Backend for Failure {
    fn name(&self) -> &'static str {
        "injected-failure"
    }
    fn begin(&mut self, _: f32, _: f32, _: f32) {}
    fn fill(&mut self, _: &Shape, _: [u8; 4], _: Transform) {}
    fn stroke(&mut self, _: &Shape, _: f32, _: [u8; 4], _: Transform) {}
    fn image(&mut self, _: &Arc<Bitmap>, _: Rect4, _: &[Shape], _: Transform) {}
    fn text(
        &mut self,
        _: &mut TextEngine,
        _: &Paragraph,
        _: &[RunPaint],
        _: (f32, f32),
        _: Transform,
    ) {
    }
    fn push_clip(&mut self, _: &Shape, _: Transform) {}
    fn pop_clip(&mut self) {}
    fn push_opacity(&mut self, _: f32) {}
    fn pop_opacity(&mut self) {}
    fn pointer(&mut self, _: f32, _: f32) {}
    fn finish(&mut self) -> Result<Pixmap, String> {
        Err("injected native finish failure".into())
    }
}
