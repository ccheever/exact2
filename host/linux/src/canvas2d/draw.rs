//! One paint pipeline for every drawing operation (LLP 1056 §1, §3 stage
//! 2): the style's source (a colour, a gradient, a conic gradient, a
//! pattern, an image), the shadow, and the compositing operator — the five
//! clip-extent operators by a layer composited over the whole clip.
//!
//! - **Shadows.** The shape is drawn into a transparent layer; its alpha is
//!   blurred with σ = `shadowBlur`/2 in backing pixels, tinted with
//!   `shadowColor`, offset by `shadowOffset` × the base scale (never the
//!   author's matrix), and composited with the operator under the clip,
//!   before the shape.
//! - **The clip-extent operators** (`source-in`, `source-out`,
//!   `destination-in`, `destination-atop`, `copy`): the shape is drawn
//!   source-over into a layer the bitmap's size, and the whole layer is
//!   composited with the operator under the clip, so pixels outside the
//!   shape but inside the clip change.
//! - **Conic gradients** have no tiny-skia shader: the gradient is evaluated
//!   per backing pixel and drawn as an image.
//! - **Patterns** repeat both ways in tiny-skia; `repeat-x`, `repeat-y` and
//!   `no-repeat` mask the shape to the tiled band.

use super::{blend, color, Env, Object, Replayer, Style};
use exact_canvas::list::Record;
use std::sync::Arc;
use tiny_skia::{
    BlendMode, Color, FillRule, FilterQuality, GradientStop, IntRect, LinearGradient, Mask, Paint,
    Path, PathBuilder, Pattern, Pixmap, PixmapPaint, Point, PremultipliedColorU8, RadialGradient,
    Rect, Shader, SpreadMode, Stroke, Transform,
};

/// What is painted, in the space `ts` maps to device pixels.
pub(super) enum Geom<'a> {
    Fill(&'a Path, FillRule, Transform),
    Stroke(&'a Path, Stroke, Transform),
}

impl Geom<'_> {
    fn ts(&self) -> Transform {
        match self {
            Geom::Fill(_, _, ts) | Geom::Stroke(_, _, ts) => *ts,
        }
    }
}

/// A style's source, with `globalAlpha` applied.
pub(super) enum Source {
    Nothing,
    Color(Color),
    /// A linear or radial gradient in user space; `device` maps user space
    /// to device pixels.
    Gradient {
        stops: Vec<GradientStop>,
        kind: [f32; 6],
        radial: bool,
        device: Transform,
    },
    /// An image: `space` maps its pixels to device pixels.
    Pixmap {
        pixmap: Arc<Pixmap>,
        space: Transform,
        spread: SpreadMode,
        quality: FilterQuality,
        opacity: f32,
        /// A pattern's repetition when it is not `repeat`.
        band: Option<u8>,
    },
}

fn paint<'a>(src: &'a Source, ts: Transform, mode: BlendMode) -> Option<Paint<'a>> {
    let inv = ts.invert()?;
    let shader = match src {
        Source::Nothing => return None,
        Source::Color(c) => Shader::SolidColor(*c),
        Source::Gradient {
            stops,
            kind,
            radial,
            device,
        } => {
            let local = inv.pre_concat(*device);
            let k = kind;
            if *radial {
                RadialGradient::new(
                    Point::from_xy(k[0], k[1]),
                    k[2],
                    Point::from_xy(k[3], k[4]),
                    k[5],
                    stops.clone(),
                    SpreadMode::Pad,
                    local,
                )?
            } else {
                LinearGradient::new(
                    Point::from_xy(k[0], k[1]),
                    Point::from_xy(k[2], k[3]),
                    stops.clone(),
                    SpreadMode::Pad,
                    local,
                )?
            }
        }
        Source::Pixmap {
            pixmap,
            space,
            spread,
            quality,
            opacity,
            ..
        } => Pattern::new(
            pixmap.as_ref().as_ref(),
            *spread,
            *quality,
            *opacity,
            inv.pre_concat(*space),
        ),
    };
    Some(Paint {
        shader,
        blend_mode: mode,
        anti_alias: true,
        ..Default::default()
    })
}

fn render(
    target: &mut Pixmap,
    geom: &Geom<'_>,
    src: &Source,
    mode: BlendMode,
    mask: Option<&Mask>,
) {
    let Some(p) = paint(src, geom.ts(), mode) else {
        return;
    };
    match geom {
        Geom::Fill(path, rule, ts) => target.fill_path(path, &p, *rule, *ts, mask),
        Geom::Stroke(path, stroke, ts) => target.stroke_path(path, &p, stroke, *ts, mask),
    }
}

fn intersect(a: Option<&Mask>, b: Option<Mask>) -> Option<Mask> {
    match (a, b) {
        (None, b) => b,
        (Some(a), None) => Some(a.clone()),
        (Some(a), Some(mut b)) => {
            for (x, y) in b.data_mut().iter_mut().zip(a.data()) {
                *x = ((*x as u16 * *y as u16 + 127) / 255) as u8;
            }
            Some(b)
        }
    }
}

/// The five clip-extent operators, composited by hand over every pixel,
/// weighted by the clip's coverage (premultiplied Porter-Duff).
fn composite_extent(dst: &mut Pixmap, src: &Pixmap, op: usize, clip: Option<&Mask>) {
    let cover = clip.map(|m| m.data());
    for (i, (d, s)) in dst.pixels_mut().iter_mut().zip(src.pixels()).enumerate() {
        let m = cover.map_or(1.0, |c| c[i] as f32 / 255.0);
        if m == 0.0 {
            continue;
        }
        let (sa, da) = (s.alpha() as f32 / 255.0, d.alpha() as f32 / 255.0);
        let sv = [s.red(), s.green(), s.blue(), s.alpha()].map(|v| v as f32);
        let dv = [d.red(), d.green(), d.blue(), d.alpha()].map(|v| v as f32);
        let r = [0, 1, 2, 3].map(|c| {
            let v = match op {
                1 => sv[c] * da,
                2 => sv[c] * (1.0 - da),
                5 => dv[c] * sa,
                7 => dv[c] * sa + sv[c] * (1.0 - da),
                _ => sv[c],
            };
            dv[c] + (v - dv[c]) * m
        });
        let a = r[3].round().clamp(0.0, 255.0) as u8;
        let ch = |v: f32| (v.round().clamp(0.0, 255.0) as u8).min(a);
        *d = PremultipliedColorU8::from_rgba(ch(r[0]), ch(r[1]), ch(r[2]), a)
            .unwrap_or(PremultipliedColorU8::TRANSPARENT);
    }
}

/// Three box blurs of an alpha plane, approximating a Gaussian of `sigma`.
fn blur(alpha: &mut [f32], w: usize, h: usize, sigma: f32) {
    if sigma < 0.3 {
        return;
    }
    // Box sizes for three passes (Kovesi's `boxesForGauss`).
    let n = 3.0;
    let ideal = (12.0 * sigma * sigma / n + 1.0).sqrt();
    let mut lo = ideal.floor() as i32;
    if lo % 2 == 0 {
        lo -= 1;
    }
    let hi = lo + 2;
    let m = ((12.0 * sigma * sigma - n * (lo * lo) as f32 - 4.0 * n * lo as f32 - 3.0 * n)
        / (-4.0 * lo as f32 - 4.0))
        .round() as i32;
    let mut tmp = vec![0.0f32; alpha.len()];
    for i in 0..3 {
        let r = ((if i < m { lo } else { hi }) - 1) / 2;
        if r <= 0 {
            continue;
        }
        box_pass(alpha, &mut tmp, w, h, r as usize, true);
        box_pass(&tmp, alpha, w, h, r as usize, false);
    }
}

fn box_pass(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize, horizontal: bool) {
    let (outer, inner) = if horizontal { (h, w) } else { (w, h) };
    let at = |o: usize, i: usize| if horizontal { o * w + i } else { i * w + o };
    let k = 1.0 / (2 * r + 1) as f32;
    for o in 0..outer {
        let mut acc = 0.0f32;
        for i in 0..=r.min(inner.saturating_sub(1)) {
            acc += src[at(o, i)];
        }
        for i in 0..inner {
            dst[at(o, i)] = acc * k;
            if i + r + 1 < inner {
                acc += src[at(o, i + r + 1)];
            }
            if i >= r {
                acc -= src[at(o, i - r)];
            }
        }
    }
}

fn premultiply(c: [f64; 4], a: f32) -> PremultipliedColorU8 {
    let a = (c[3] as f32 * a).clamp(0.0, 1.0);
    let ch = |v: f64| ((v as f32 / 255.0 * a) * 255.0).round() as u8;
    let a8 = (a * 255.0).round() as u8;
    PremultipliedColorU8::from_rgba(ch(c[0]).min(a8), ch(c[1]).min(a8), ch(c[2]).min(a8), a8)
        .unwrap_or(PremultipliedColorU8::TRANSPARENT)
}

fn stop_color(stops: &[(f32, [f64; 4])], t: f32) -> [f64; 4] {
    let first = stops[0];
    if t <= first.0 {
        return first.1;
    }
    for w in stops.windows(2) {
        let (a, b) = (w[0], w[1]);
        if t <= b.0 {
            let f = if b.0 > a.0 {
                ((t - a.0) / (b.0 - a.0)) as f64
            } else {
                1.0
            };
            return [0, 1, 2, 3].map(|i| a.1[i] + (b.1[i] - a.1[i]) * f);
        }
    }
    stops[stops.len() - 1].1
}

impl Replayer {
    /// A style's source for this paint.
    pub(super) fn source(&self, style: Style, env: &Env<'_>) -> Source {
        let alpha = self.state.alpha;
        let id = match style {
            Style::Color(c) => return Source::Color(color(c, alpha)),
            Style::Object(id) => id,
        };
        let gradient = |kind: [f32; 6], radial: bool, stops: &Vec<(f32, [f64; 4])>| {
            if stops.is_empty() {
                return Source::Nothing;
            }
            let stops = stops
                .iter()
                .map(|(o, c)| GradientStop::new(*o, color(*c, alpha)))
                .collect();
            Source::Gradient {
                stops,
                kind,
                radial,
                device: self.device(),
            }
        };
        match self.objects.get(&id) {
            Some(Object::Linear(k, stops)) => {
                if k[0] == k[2] && k[1] == k[3] {
                    return Source::Nothing;
                }
                gradient([k[0], k[1], k[2], k[3], 0.0, 0.0], false, stops)
            }
            Some(Object::Radial(k, stops)) => {
                if k[0] == k[3] && k[1] == k[4] && k[2] == k[5] {
                    return Source::Nothing;
                }
                gradient(*k, true, stops)
            }
            Some(Object::Conic(k, stops)) => self.conic(*k, stops),
            Some(Object::Pattern {
                image,
                repetition,
                transform,
            }) => {
                let Some(pixmap) = self.image(*image, env) else {
                    return Source::Nothing;
                };
                Source::Pixmap {
                    pixmap,
                    space: self.device().pre_concat(*transform),
                    spread: SpreadMode::Repeat,
                    quality: self.quality(),
                    opacity: alpha,
                    band: (*repetition != 0).then_some(*repetition),
                }
            }
            _ => Source::Nothing,
        }
    }

    fn image(&self, id: u32, env: &Env<'_>) -> Option<Arc<Pixmap>> {
        match self.objects.get(&id)? {
            Object::Image(src) => env.images.get(src).cloned(),
            _ => None,
        }
    }

    fn quality(&self) -> FilterQuality {
        match self.state.smoothing {
            (false, _) => FilterQuality::Nearest,
            (true, 0) => FilterQuality::Bilinear,
            _ => FilterQuality::Bicubic,
        }
    }

    /// A conic gradient evaluated at every backing pixel's centre.
    fn conic(&self, k: [f32; 3], stops: &[(f32, [f64; 4])]) -> Source {
        if stops.is_empty() {
            return Source::Nothing;
        }
        let Some(inv) = self.device().invert() else {
            return Source::Nothing;
        };
        let (w, h) = (self.pixmap.width(), self.pixmap.height());
        let Some(mut pm) = Pixmap::new(w, h) else {
            return Source::Nothing;
        };
        let alpha = self.state.alpha;
        let tau = std::f32::consts::TAU;
        let px = pm.pixels_mut();
        for y in 0..h {
            for x in 0..w {
                let mut p = Point::from_xy(x as f32 + 0.5, y as f32 + 0.5);
                inv.map_point(&mut p);
                let a = (p.y - k[2]).atan2(p.x - k[1]) - k[0];
                let t = (a / tau).rem_euclid(1.0);
                px[(y * w + x) as usize] = premultiply(stop_color(stops, t), alpha);
            }
        }
        Source::Pixmap {
            pixmap: Arc::new(pm),
            space: Transform::identity(),
            spread: SpreadMode::Pad,
            quality: FilterQuality::Nearest,
            opacity: 1.0,
            band: None,
        }
    }

    /// The tiled band a pattern's repetition allows, as a device mask.
    fn band(&self, src: &Source) -> Option<Mask> {
        let Source::Pixmap {
            pixmap,
            space,
            band: Some(rep),
            ..
        } = src
        else {
            return None;
        };
        let (w, h) = (pixmap.width() as f32, pixmap.height() as f32);
        let big = 1e6;
        let rect = match rep {
            1 => Rect::from_ltrb(-big, 0.0, big, h),
            2 => Rect::from_ltrb(0.0, -big, w, big),
            _ => Rect::from_ltrb(0.0, 0.0, w, h),
        }?;
        let mut mask = Mask::new(self.pixmap.width(), self.pixmap.height())?;
        mask.fill_path(
            &PathBuilder::from_rect(rect),
            FillRule::Winding,
            true,
            *space,
        );
        Some(mask)
    }

    fn shadowed(&self) -> bool {
        let s = &self.state;
        s.shadow_color[3] > 0.0 && (s.shadow_blur > 0.0 || s.shadow_offset != (0.0, 0.0))
    }

    /// Paint `geom` with `src` through the shadow and the operator.
    pub(super) fn draw(&mut self, geom: Geom<'_>, src: Source) {
        if matches!(src, Source::Nothing) {
            return;
        }
        let mode = blend(self.state.composite);
        let extent = exact_canvas::composite_clips_extent(self.state.composite);
        let band = self.band(&src);
        let (w, h) = (self.pixmap.width(), self.pixmap.height());
        let clip = self.state.clip.clone();
        if self.shadowed() || extent {
            let Some(mut layer) = Pixmap::new(w, h) else {
                return;
            };
            render(
                &mut layer,
                &geom,
                &src,
                BlendMode::SourceOver,
                band.as_ref(),
            );
            if self.shadowed() {
                if let Some(shadow) = self.shadow(&layer) {
                    let (ox, oy) = self.state.shadow_offset;
                    let paint = PixmapPaint {
                        opacity: 1.0,
                        blend_mode: mode,
                        quality: FilterQuality::Bilinear,
                    };
                    let ts = Transform::from_translate(ox * self.scale, oy * self.scale);
                    self.pixmap
                        .draw_pixmap(0, 0, shadow.as_ref(), &paint, ts, clip.as_ref());
                }
            }
            if extent {
                composite_extent(
                    &mut self.pixmap,
                    &layer,
                    self.state.composite,
                    clip.as_ref(),
                );
                return;
            }
        }
        let mask = intersect(clip.as_ref(), band);
        render(&mut self.pixmap, &geom, &src, mode, mask.as_ref());
    }

    /// The shadow of a layer: its alpha blurred and tinted.
    fn shadow(&self, layer: &Pixmap) -> Option<Pixmap> {
        let (w, h) = (layer.width() as usize, layer.height() as usize);
        let mut alpha: Vec<f32> = layer.pixels().iter().map(|p| p.alpha() as f32).collect();
        blur(&mut alpha, w, h, self.state.shadow_blur / 2.0 * self.scale);
        let mut out = Pixmap::new(w as u32, h as u32)?;
        let c = self.state.shadow_color;
        for (px, a) in out.pixels_mut().iter_mut().zip(alpha) {
            *px = premultiply(c, a / 255.0);
        }
        Some(out)
    }

    /// `drawImage`: the source rectangle of a decoded handle into the
    /// destination under the author matrix.
    pub(super) fn draw_image(&mut self, r: &Record<'_>, env: &Env<'_>) {
        let Some(image) = self.image(r.at(0) as u32, env) else {
            return;
        };
        let f = |i: usize| r.at(i) as f32;
        let (sx, sy, sw, sh) = (f(1), f(2), f(3), f(4));
        let (dx, dy, dw, dh) = (f(5), f(6), f(7), f(8));
        // Sample only the source rectangle: crop to the whole pixels it
        // covers, so filtering never reaches past it.
        let crop = IntRect::from_ltrb(
            sx.floor() as i32,
            sy.floor() as i32,
            (sx + sw).ceil() as i32,
            (sy + sh).ceil() as i32,
        );
        let (pixmap, ox, oy) = match crop {
            Some(c) if (c.width(), c.height()) != (image.width(), image.height()) => {
                match image.clone_rect(c) {
                    Some(p) => (Arc::new(p), c.x() as f32, c.y() as f32),
                    None => return,
                }
            }
            _ => (image, 0.0, 0.0),
        };
        let space = self
            .device()
            .pre_translate(dx, dy)
            .pre_scale(dw / sw, dh / sh)
            .pre_translate(ox - sx, oy - sy);
        let Some(rect) = Rect::from_xywh(dx, dy, dw, dh) else {
            return;
        };
        let path = PathBuilder::from_rect(rect);
        let ts = self.device();
        let src = Source::Pixmap {
            pixmap,
            space,
            spread: SpreadMode::Pad,
            quality: self.quality(),
            opacity: self.state.alpha,
            band: None,
        };
        self.draw(Geom::Fill(&path, FillRule::Winding, ts), src);
    }

    /// `putImageData`: raw backing pixels, premultiplied; nothing else
    /// applies.
    pub(super) fn put_image_data(&mut self, r: &Record<'_>) {
        let (x0, y0) = (r.at(0) as i64, r.at(1) as i64);
        let (w, h) = (r.at(2) as i64, r.at(3) as i64);
        let (pw, ph) = (self.pixmap.width() as i64, self.pixmap.height() as i64);
        let px = self.pixmap.pixels_mut();
        for y in 0..h {
            for x in 0..w {
                let (tx, ty) = (x0 + x, y0 + y);
                if tx < 0 || ty < 0 || tx >= pw || ty >= ph {
                    continue;
                }
                let v = r.at(4 + (y * w + x) as usize) as u32;
                let [cr, cg, cb, ca] = v.to_be_bytes();
                let c = [cr as f64, cg as f64, cb as f64, ca as f64 / 255.0];
                px[(ty * pw + tx) as usize] = premultiply(c, 1.0);
            }
        }
    }
}
