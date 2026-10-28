//! The instanced painter's adapter. @ref LLP 1149 D1.2, D2, D3.1.
use crate::image::Bitmap;
use crate::paint::border::{BorderFill, PathOp};
use crate::paint::{Backend, FrameTarget, GradientPaint, Rect4, Shape, POINTER};
use crate::text::{Paragraph, RunPaint, TextEngine};
use exact_ink::atlas::Atlas;
use exact_ink::device::{Device, Error, Info, Result};
use exact_ink::instance::{flags, Bindings, RoundedClip, TextureId};
use exact_ink::{Brush, Encoder, Instance, Kind, Wgpu};
use std::sync::Arc;
use std::time::Instant;
use tiny_skia::{FillRule, Path, PathBuilder, Pixmap, Transform};

mod clips;
mod effects;
mod stats;
pub use stats::{Statistics, StatisticsReader};
mod gradient;
mod images;
mod rows;
mod svg;
#[cfg(test)]
mod tests;
mod text;

/// Headless wgpu painter; its premultiplied pixels use the presenter's ordinary path.
pub struct Ink {
    device: Wgpu,
    encoder: Encoder,
    paths: Atlas,
    glyphs: exact_ink::glyph::Glyphs,
    text_sigma: f32,
    ramps: Atlas,
    images: images::Images,
    pool: exact_ink::layer::Pool,
    groups: Vec<clips::Group>,
    clips: Vec<bool>,
    target: Option<TextureId>,
    draw_target: Option<TextureId>,
    draw_clear: Option<[f64; 4]>,
    size: [u32; 2],
    scale: f32,
    page: [u8; 4],
    clear: [f64; 4],
    error: Option<String>,
    layer_ms: f64,
    last_ms: (f64, f64),
    statistics: StatisticsReader,
    rows: rows::Rows,
}

impl Ink {
    /// Open a native wgpu device and record its startup phases.
    pub fn new() -> std::result::Result<Self, String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Wgpu::new()
                .map(Self::from_device)
                .map_err(|e| e.to_string())
        }
        #[cfg(target_arch = "wasm32")]
        {
            Err("Ink's synchronous presenter requires a native device".into())
        }
    }

    /// Adapt an already opened device, including one opened asynchronously.
    pub fn from_device(device: Wgpu) -> Self {
        Self {
            device,
            encoder: Encoder::new(1, 1),
            paths: Atlas::paths(),
            glyphs: Default::default(),
            text_sigma: 0.,
            ramps: Atlas::ramps(),
            images: images::Images::default(),
            pool: exact_ink::layer::Pool::default(),
            groups: Vec::new(),
            clips: Vec::new(),
            target: None,
            draw_target: None,
            draw_clear: Some([1.; 4]),
            size: [1, 1],
            scale: 1.,
            page: [255; 4],
            clear: [1.; 4],
            error: None,
            layer_ms: 0.,
            last_ms: (0., 0.),
            statistics: StatisticsReader::default(),
            rows: rows::Rows::default(),
        }
    }

    /// Device and shader timings reported by the presenter.
    pub fn info(&self) -> &Info {
        self.device.info()
    }

    fn accept<T>(&mut self, result: Result<T>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(e) => {
                self.error.get_or_insert_with(|| e.to_string());
                None
            }
        }
    }

    fn device_transform(&self, ts: Transform) -> Transform {
        Transform::from_scale(self.scale, self.scale).pre_concat(ts)
    }

    fn emit(&mut self, instance: Instance, bindings: Bindings) -> Result<()> {
        if self.error.is_some() {
            return Ok(());
        }
        if let Some(capture) = self.rows.capture.last_mut() {
            capture
                .drawing
                .push(instance.translated(capture.offset(self.scale)), bindings);
            return Ok(());
        }
        if self.encoder.push(instance, bindings).is_err() {
            // A shadow uses its second clip slot for the excluded border box.
            self.open_group(1., None);
            self.encoder
                .push(instance, bindings)
                .map_err(|_| Error::Invalid("instance clips".into()))?;
            self.close_group()?;
        }
        Ok(())
    }

    fn shape_instance(
        &mut self,
        shape: &Shape,
        ts: Transform,
    ) -> Result<Option<(Instance, Bindings)>> {
        if shape.rect.2 <= 0. || shape.rect.3 <= 0. {
            return Ok(None);
        }
        let dev = self.device_transform(ts);
        if !dev.is_finite() || dev.invert().is_none() {
            return Ok(None);
        }
        if circular(shape) {
            let mut instance = Instance::new(Kind::RoundedRect, Brush::Solid, rect(shape.rect));
            instance.radii = shape.radii.map(|r| r.0);
            instance.xform = affine(dev);
            Ok(Some((instance, Bindings::default())))
        } else {
            let Some(path) = shape_path(shape) else {
                return Ok(None);
            };
            self.path_instance(&path, FillRule::Winding, &[], ts)
        }
    }

    fn path_instance(
        &mut self,
        path: &Path,
        rule: FillRule,
        clips: &[(&Path, FillRule)],
        ts: Transform,
    ) -> Result<Option<(Instance, Bindings)>> {
        let dev = self.device_transform(ts);
        let coverage = exact_ink::path::coverage(
            &mut self.paths,
            &mut self.device,
            path,
            rule,
            clips,
            dev,
            self.rows.capture.is_empty().then_some(self.size),
        )?;
        Ok(coverage.map(|c| {
            let mut instance = Instance::new(Kind::Coverage, Brush::Solid, c.rect);
            instance.xform = affine(dev);
            instance.uv = c.entry.uv;
            (
                instance,
                Bindings {
                    path: c.entry.texture,
                    ..Default::default()
                },
            )
        }))
    }

    fn border(&mut self, part: &BorderFill, ts: Transform) -> Result<()> {
        if let Some(((outer, outer_radii), (inner, inner_radii))) = part.ring {
            if part.clip.is_none()
                && part.ring_clip.is_none()
                && outer_radii
                    .iter()
                    .chain(inner_radii.iter())
                    .all(|r| r.0 == r.1)
            {
                let mut instance = Instance::new(Kind::Ring, Brush::Solid, rect(outer));
                instance.radii = outer_radii.map(|r| r.0);
                instance.uv = rect(inner);
                instance.param = inner_radii.map(|r| r.0);
                instance.xform = affine(self.device_transform(ts));
                instance.color = color(part.color);
                return self.emit(instance, Bindings::default());
            }
        }
        let Some(region) = path(&part.region) else {
            return Ok(());
        };
        let side = part.clip.as_deref().and_then(path);
        let ring = part.ring_clip.as_deref().and_then(path);
        let clips: Vec<_> = side
            .as_ref()
            .map(|p| (p, FillRule::Winding))
            .into_iter()
            .chain(ring.as_ref().map(|p| (p, FillRule::EvenOdd)))
            .collect();
        if let Some((mut instance, bindings)) =
            self.path_instance(&region, FillRule::EvenOdd, &clips, ts)?
        {
            instance.color = color(part.color);
            self.emit(instance, bindings)?;
        }
        Ok(())
    }
}

impl Backend for Ink {
    fn name(&self) -> &'static str {
        "ink"
    }
    fn needs_geometry_paint(&self) -> bool {
        true
    }
    fn page(&mut self, color: [u8; 4]) {
        self.page = color;
    }

    fn begin(&mut self, width: f32, height: f32, scale: f32) {
        self.layer_ms = 0.;
        self.text_sigma = 0.;
        self.error = None;
        self.scale = scale;
        self.size = [width, height].map(|n| (n * scale).round().max(1.) as u32);
        self.encoder = Encoder::new(self.size[0], self.size[1]);
        self.groups.clear();
        self.clips.clear();
        self.clear = color(self.page).map(f64::from);
        let result = (|| {
            if !scale.is_finite() || scale <= 0. || self.size.iter().any(|s| *s > 16384) {
                return Err(Error::Invalid("frame dimensions or scale".into()));
            }
            self.paths.begin(&mut self.device)?;
            self.glyphs.begin(&mut self.device)?;
            self.ramps.begin(&mut self.device)?;
            self.images.begin(&mut self.device)?;
            self.pool.begin(&mut self.device, self.size)?;
            self.target = Some(match self.target {
                Some(t) => self.device.resize_target(t, self.size[0], self.size[1])?,
                None => self.device.create_target(self.size[0], self.size[1])?,
            });
            self.draw_target = self.target;
            self.draw_clear = Some(self.clear);
            Ok(())
        })();
        self.accept(result);
    }

    fn fill(&mut self, shape: &Shape, c: [u8; 4], ts: Transform) {
        let dev = self.device_transform(ts);
        if c[3] == 255
            && self.rows.capture.is_empty()
            && !shape.rounded()
            && self.clips.is_empty()
            && self.groups.is_empty()
            && dev.kx == 0.
            && dev.ky == 0.
            && dev.sx > 0.
            && dev.sy > 0.
            && shape.rect.0 * dev.sx + dev.tx <= 0.
            && shape.rect.1 * dev.sy + dev.ty <= 0.
            && (shape.rect.0 + shape.rect.2) * dev.sx + dev.tx >= self.size[0] as f32
            && (shape.rect.1 + shape.rect.3) * dev.sy + dev.ty >= self.size[1] as f32
        {
            self.encoder = Encoder::new(self.size[0], self.size[1]);
            self.clear = color(c).map(f64::from);
            self.draw_clear = Some(self.clear);
            return;
        }
        let result = self.shape_instance(shape, ts);
        if let Some(Some((mut i, b))) = self.accept(result) {
            i.color = color(c);
            let result = self.emit(i, b);
            self.accept(result);
        }
    }

    fn fill_gradient(&mut self, shape: &Shape, gradient: &GradientPaint, ts: Transform) {
        let result = self.gradient(shape, gradient, ts);
        self.accept(result);
    }

    fn fill_border(&mut self, part: &BorderFill, ts: Transform) {
        let result = self.border(part, ts);
        self.accept(result);
    }

    fn blurred_shadow(
        &mut self,
        shape: &Shape,
        c: [u8; 4],
        sigma: f32,
        outer: &Shape,
        ts: Transform,
    ) -> bool {
        if !circular(shape) || !circular(outer) {
            return false;
        }
        let reach = 3. * sigma.max(0.) + 1.;
        let (x, y, w, h) = shape.rect;
        let mut i = Instance::new(
            Kind::Shadow,
            Brush::Solid,
            [x - reach, y - reach, w + 2. * reach, h + 2. * reach],
        );
        i.xform = affine(self.device_transform(ts));
        i.uv = rect(shape.rect);
        i.radii = shape.radii.map(|r| r.0);
        i.param[0] = sigma;
        i.color = color(c);
        if i.set_clip(
            1,
            RoundedClip {
                rect: rect(outer.rect),
                radii: outer.radii.map(|r| r.0),
            },
        )
        .is_err()
        {
            return false;
        }
        i.flags |= flags::SHADOW_LOCAL;
        let result = self.emit(i, Bindings::default());
        self.accept(result);
        true
    }

    fn image(
        &mut self,
        image: &Arc<Bitmap>,
        dst: Rect4,
        clips: &[Shape],
        ts: Transform,
        tint: Option<[u8; 4]>,
    ) {
        let result = self.images.bitmap(&mut self.device, image);
        if let Some(texture) = self.accept(result) {
            self.picture(texture, dst, clips, ts, tint);
        }
    }

    fn surface_image(&mut self, pixels: Arc<Pixmap>, dst: Rect4) {
        self.canvas(&pixels, dst, &[], Transform::identity());
    }
    fn island_image(&mut self, pixels: Arc<Pixmap>, dst: Rect4, ts: Transform, mode: u8) {
        let result = self.blended_image(&pixels, dst, ts, mode);
        self.accept(result);
    }
    fn canvas(&mut self, pixels: &Arc<Pixmap>, dst: Rect4, clips: &[Shape], ts: Transform) {
        let result = self.images.pixmap(&mut self.device, pixels);
        if let Some(texture) = self.accept(result) {
            self.picture(texture, dst, clips, ts, None);
        }
    }

    fn text_blur(&mut self, sigma: f32) {
        self.text_sigma = sigma.max(0.);
    }

    fn text(
        &mut self,
        text: &mut TextEngine,
        paragraph: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        ts: Transform,
    ) {
        let result = self.glyph_runs(text, paragraph, palette, origin, ts);
        self.accept(result);
    }

    fn svg_path(&mut self, s: &crate::paint::SvgPaint<'_>, ts: Transform) {
        let result = self.svg(s, ts);
        self.accept(result);
    }
    fn push_css_clip(&mut self, path: &exact_kernel::clip::ClipPath, ts: Transform) -> bool {
        let result = self.css_clip(path, ts);
        self.accept(result).unwrap_or(false)
    }
    fn push_svg_clip(&mut self, clip: &exact_kernel::svg::scene::Clip, ts: Transform) -> usize {
        let result = self.svg_clip(clip, ts);
        self.accept(result).unwrap_or(0)
    }
    fn push_clip(&mut self, shape: &Shape, ts: Transform) {
        let result = self.clip(shape, ts);
        self.accept(result);
    }
    fn pop_clip(&mut self) {
        if self.record(rows::Marker::PopClip) {
            return;
        }
        if let Some(layer) = self.clips.pop() {
            if layer {
                let r = self.close_group();
                self.accept(r);
            } else {
                self.encoder.pop_clip();
            }
        }
    }
    fn push_opacity(&mut self, alpha: f32) {
        if self.record(rows::Marker::Opacity(alpha)) {
            return;
        }
        self.open_group(alpha.clamp(0., 1.), None);
    }
    fn pop_opacity(&mut self) {
        if self.record(rows::Marker::PopOpacity) {
            return;
        }
        let r = self.close_group();
        self.accept(r);
    }

    fn push_mask(&mut self, _: &Shape, _: Transform) {
        if self.record(rows::Marker::Opacity(1.)) {
            return;
        }
        self.open_group(1., None);
    }
    fn pop_mask(
        &mut self,
        shape: &Shape,
        mask: &std::result::Result<GradientPaint, [u8; 4]>,
        ts: Transform,
    ) {
        let ts = self.record_transform(ts);
        if self.record(rows::Marker::Mask(*shape, mask.clone(), ts)) {
            return;
        }
        let result = self.masked(shape, mask, ts);
        self.accept(result);
    }
    fn backdrop_filter(
        &mut self,
        shape: &Shape,
        filter: &exact_kernel::style::BackdropFilter,
        ts: Transform,
    ) {
        let ts = self.record_transform(ts);
        if self.record(rows::Marker::Backdrop(*shape, filter.clone(), ts)) {
            return;
        }
        let result = self.backdrop(shape, filter, ts);
        self.accept(result);
    }

    fn pointer(&mut self, x: f32, y: f32) {
        let mut builder = PathBuilder::new();
        for (i, &(x, y)) in POINTER.iter().enumerate() {
            if i == 0 {
                builder.move_to(x, y);
            } else {
                builder.line_to(x, y);
            }
        }
        builder.close();
        let Some(path) = builder.finish() else {
            return;
        };
        let ts = Transform::from_translate(x, y);
        let result = (|| {
            if let Some((mut i, b)) = self.path_instance(&path, FillRule::Winding, &[], ts)? {
                i.color = [1.; 4];
                self.emit(i, b)?;
            }
            if let Some(stroke) = path.stroke(
                &tiny_skia::Stroke {
                    width: 1.,
                    ..Default::default()
                },
                self.scale,
            ) {
                if let Some((mut i, b)) = self.path_instance(&stroke, FillRule::Winding, &[], ts)? {
                    i.color = [0., 0., 0., 1.];
                    self.emit(i, b)?;
                }
            }
            Ok(())
        })();
        self.accept(result);
    }

    fn finish(&mut self) -> std::result::Result<Pixmap, String> {
        while !self.groups.is_empty() {
            let r = self.close_group();
            self.accept(r);
        }
        if let Some(e) = self.error.take() {
            return Err(e);
        }
        let target = self.flush().map_err(|e| e.to_string())?;
        let render_ms = self.layer_ms;
        let readback = Instant::now();
        let bytes = self.device.readback(target).map_err(|e| e.to_string())?;
        let pixels = Pixmap::from_vec(
            bytes,
            tiny_skia::IntSize::from_wh(self.size[0], self.size[1]).unwrap(),
        )
        .ok_or("invalid ink readback")?;
        self.last_ms = (render_ms, readback.elapsed().as_secs_f64() * 1000.);
        self.update_statistics();
        Ok(pixels)
    }
    fn finish_for(&mut self, target: FrameTarget) -> std::result::Result<Option<Pixmap>, String> {
        if target == FrameTarget::Geometry {
            Ok(None)
        } else {
            self.finish().map(Some)
        }
    }
    fn last_frame_ms(&self) -> Option<(f64, f64)> {
        Some(self.last_ms)
    }

    fn rows(&self) -> bool {
        true
    }
    fn row_begin(&mut self, id: u32, origin: (f32, f32), _: Option<u32>) {
        self.record_row(id, origin);
    }
    fn row_end(&mut self, _: Rect4) -> u32 {
        self.keep_row()
    }
    fn row_draw(&mut self, id: u32, origin: (f32, f32), _: Rect4) {
        let r = self.draw_row(id, origin);
        self.accept(r);
    }
    fn row_free(&mut self, id: u32) {
        self.free_row(id);
    }
    fn row_culled(&self, bounds: Rect4) -> bool {
        self.culled(bounds)
    }
    fn group_begin(&mut self, id: exact_kernel::ViewId, scroll: (f32, f32)) {
        self.rows.groups.push((id, scroll));
    }
    fn group_end(&mut self) {
        self.rows.groups.pop();
    }
    fn slot_begin(&mut self, id: exact_kernel::ViewId) {
        self.record_slot(id);
    }
    fn slot_end(&mut self) {
        self.keep_slot();
    }
    fn slot_again(&mut self, row: u32, id: exact_kernel::ViewId) -> bool {
        self.replace_slot(row, id)
    }
    fn layer_begin(&mut self, key: u64, ts: Transform, pivot: (f32, f32), base: [f32; 7]) -> bool {
        self.record_layer(key, ts, pivot, base);
        true
    }
    fn layer_recolored(&mut self) { /* Each layer_begin records the current colour. */
    }
    fn layer_end(&mut self) {
        let r = self.keep_layer();
        self.accept(r);
    }
}

fn color(c: [u8; 4]) -> [f32; 4] {
    let a = c[3] as f32 / 255.;
    [
        c[0] as f32 / 255. * a,
        c[1] as f32 / 255. * a,
        c[2] as f32 / 255. * a,
        a,
    ]
}
fn rect((x, y, w, h): Rect4) -> [f32; 4] {
    [x, y, w, h]
}
fn affine(t: Transform) -> [f32; 6] {
    [t.sx, t.ky, t.kx, t.sy, t.tx, t.ty]
}
fn circular(shape: &Shape) -> bool {
    shape.corners.is_none_or(|c| c.is_round())
        && shape
            .radii
            .iter()
            .all(|r| r.0 == r.1 && r.0.is_finite() && r.0 >= 0.)
}
fn shape_path(shape: &Shape) -> Option<Path> {
    let mut ops = Vec::new();
    crate::paint::border::shape_path(&mut ops, shape);
    path(&ops)
}
fn path(ops: &[PathOp]) -> Option<Path> {
    let mut b = PathBuilder::new();
    for op in ops {
        match *op {
            PathOp::Move(x, y) => b.move_to(x, y),
            PathOp::Line(x, y) => b.line_to(x, y),
            PathOp::Cubic(a, b0, c, d, x, y) => b.cubic_to(a, b0, c, d, x, y),
            PathOp::Close => b.close(),
        }
    }
    b.finish()
}
