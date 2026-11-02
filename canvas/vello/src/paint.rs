//! How every paint reaches the scene (`Canvas2DPaint.swift`): the style as
//! a vello brush, global alpha, the operator and the shadow.
//!
//! - A paint under an operator other than source-over is a layer with that
//!   blend. The five that reach outside the shape (source-in, source-out,
//!   destination-in, destination-atop, copy) take the whole canvas as the
//!   layer, so where the shape is not, the layer is transparent and the
//!   operator clears (within the clip layers, which bound it), as WebKit's
//!   offscreen does; the others take the shape's bounds.
//! - A shadow is the paint drawn offscreen at its alpha, blurred (σ =
//!   shadowBlur / 2 device pixels: canvas units × the scale, never the
//!   author matrix), coloured, and drawn beneath the shape at the offset
//!   (also canvas units × the scale), inside one layer composited with the
//!   operator and global alpha: Core Graphics' transparency layer.
//! - Gradients interpolate unpremultiplied, as Core Graphics' do; one stop
//!   fills with its colour, none (or a degenerate gradient) paints nothing.
//! - Patterns tile in pattern space (author ∘ the pattern's transform); a
//!   single row, column or tile is a clip to that band.

use crate::replay::{Frame, Gradient, Host, Replayer, ShadowJob};
use vello::kurbo::{Affine, BezPath, Join, Point, Rect, Shape, Stroke};
use vello::peniko::{
    BlendMode, Blob, Brush, Color, ColorStop, Compose, Extend, Fill, Gradient as Grad,
    ImageAlphaType, ImageBrush, ImageData, ImageFormat, ImageQuality, InterpolationAlphaSpace, Mix,
};

/// A fill or stroke style.
#[derive(Clone, Debug, PartialEq)]
pub enum Style {
    /// `r g b` 0–255 and alpha 0–1, non-premultiplied.
    Color([f64; 4]),
    Gradient(u32),
    Pattern(u32),
}

/// What a paint draws: a shape under a transform (to device pixels).
pub enum Geom<'a> {
    Fill(&'a BezPath, Fill, Affine),
    Stroke(&'a BezPath, Stroke, Affine),
    /// A rectangle (`shape`, in user space) filled with `image` placed by
    /// `to_user` (image pixels to user space).
    Image(&'a BezPath, Affine, ImageData, Affine),
}

/// A style resolved for vello: the brush, where its space lands in device
/// pixels, and the band a single-row, -column or -tile pattern is clipped to
/// (in the brush's space).
struct Resolved {
    brush: Brush,
    to_device: Affine,
    band: Option<Rect>,
}

pub fn color(c: [f64; 4], alpha: f64) -> Color {
    Color::new([
        (c[0] / 255.0) as f32,
        (c[1] / 255.0) as f32,
        (c[2] / 255.0) as f32,
        (c[3] * alpha).clamp(0.0, 1.0) as f32,
    ])
}

/// `globalCompositeOperation` by index.
pub fn blend(k: usize) -> BlendMode {
    let compose = |c| BlendMode::new(Mix::Normal, c);
    let mix = |m| BlendMode::new(m, Compose::SrcOver);
    match k {
        1 => compose(Compose::SrcIn),
        2 => compose(Compose::SrcOut),
        3 => compose(Compose::SrcAtop),
        4 => compose(Compose::DestOver),
        5 => compose(Compose::DestIn),
        6 => compose(Compose::DestOut),
        7 => compose(Compose::DestAtop),
        8 => compose(Compose::Plus),
        9 => compose(Compose::Copy),
        10 => compose(Compose::Xor),
        11 => mix(Mix::Multiply),
        12 => mix(Mix::Screen),
        13 => mix(Mix::Overlay),
        14 => mix(Mix::Darken),
        15 => mix(Mix::Lighten),
        16 => mix(Mix::ColorDodge),
        17 => mix(Mix::ColorBurn),
        18 => mix(Mix::HardLight),
        19 => mix(Mix::SoftLight),
        20 => mix(Mix::Difference),
        21 => mix(Mix::Exclusion),
        22 => mix(Mix::Hue),
        23 => mix(Mix::Saturation),
        24 => mix(Mix::Color),
        25 => mix(Mix::Luminosity),
        _ => BlendMode::new(Mix::Normal, Compose::SrcOver),
    }
}

/// An image's sampling for the state's smoothing: disabled is nearest,
/// `low` and `medium` bilinear, `high` bicubic.
fn quality(smoothing: bool, q: u8) -> ImageQuality {
    match (smoothing, q) {
        (false, _) => ImageQuality::Low,
        (true, 2) => ImageQuality::High,
        _ => ImageQuality::Medium,
    }
}

/// A shape's bounds in device pixels (a stroke's with its reach).
fn bounds(geom: &Geom<'_>) -> Rect {
    match geom {
        Geom::Fill(p, _, t) => t.transform_rect_bbox(p.bounding_box()),
        Geom::Stroke(p, s, t) => {
            let mut reach = s.width / 2.0;
            if s.join == Join::Miter {
                reach *= s.miter_limit.max(1.0);
            }
            reach *= std::f64::consts::SQRT_2;
            t.transform_rect_bbox(p.bounding_box().inflate(reach, reach))
        }
        Geom::Image(p, t, _, _) => t.transform_rect_bbox(p.bounding_box()),
    }
}

fn stops(s: &[(f64, [f64; 4])]) -> Vec<ColorStop> {
    s.iter()
        .map(|(o, c)| ColorStop::from((*o as f32, color(*c, 1.0))))
        .collect()
}

impl Replayer {
    /// A style as a brush, or none when it paints nothing.
    fn resolve(&self, style: &Style, host: &mut dyn Host) -> Option<Resolved> {
        let user = self.device();
        match style {
            Style::Color(c) => Some(Resolved {
                brush: Brush::Solid(color(*c, 1.0)),
                to_device: user,
                band: None,
            }),
            Style::Gradient(id) => {
                let (g, s) = match self.gradients.get(id)? {
                    Gradient::Linear(p, s) => {
                        if p[0] == p[2] && p[1] == p[3] {
                            return None;
                        }
                        (Grad::new_linear((p[0], p[1]), (p[2], p[3])), s)
                    }
                    Gradient::Radial(p, s) => {
                        if p[0] == p[3] && p[1] == p[4] && p[2] == p[5] {
                            return None;
                        }
                        (
                            Grad::new_two_point_radial(
                                (p[0], p[1]),
                                p[2] as f32,
                                (p[3], p[4]),
                                p[5] as f32,
                            ),
                            s,
                        )
                    }
                    Gradient::Conic(p, s) => {
                        let start = p[0].rem_euclid(std::f64::consts::TAU) as f32;
                        // One turn from the start angle; vello measures the
                        // sweep from +x in [0, 1), so the turn wraps.
                        let g = Grad::new_sweep((p[1], p[2]), start, start + std::f32::consts::TAU)
                            .with_extend(Extend::Repeat);
                        (g, s)
                    }
                };
                match s.len() {
                    0 => None,
                    1 => Some(Resolved {
                        brush: Brush::Solid(color(s[0].1, 1.0)),
                        to_device: user,
                        band: None,
                    }),
                    _ => {
                        let g = g
                            .with_stops(stops(s).as_slice())
                            .with_interpolation_alpha_space(
                                InterpolationAlphaSpace::Unpremultiplied,
                            );
                        Some(Resolved {
                            brush: Brush::Gradient(g),
                            to_device: user,
                            band: None,
                        })
                    }
                }
            }
            Style::Pattern(id) => {
                let p = self.patterns.get(id)?;
                let src = self.images.get(&p.image)?;
                let image = host.image(src)?;
                let (w, h) = (f64::from(image.width), f64::from(image.height));
                let to_device = user * p.transform;
                // The band, from the canvas's extent in pattern space.
                let reach = to_device
                    .inverse()
                    .transform_rect_bbox(self.whole())
                    .inflate(w, h);
                let band = match p.repetition {
                    1 => Some(Rect::new(reach.x0, 0.0, reach.x1, h)),
                    2 => Some(Rect::new(0.0, reach.y0, w, reach.y1)),
                    3 => Some(Rect::new(0.0, 0.0, w, h)),
                    _ => None,
                };
                let brush = ImageBrush::new(image)
                    .with_extend(Extend::Repeat)
                    .with_quality(quality(self.state.smoothing, self.state.quality));
                Some(Resolved {
                    brush: Brush::Image(brush),
                    to_device,
                    band,
                })
            }
        }
    }

    /// Draw `geom` with `r` at `alpha` into `scene`, everything first
    /// through `pre` (device pixels to the scene's pixels).
    fn draw(scene: &mut vello::Scene, pre: Affine, geom: &Geom<'_>, r: &Resolved, alpha: f64) {
        let a = alpha as f32;
        let brush = match &r.brush {
            Brush::Solid(c) => Brush::Solid(c.multiply_alpha(a)),
            Brush::Gradient(g) => Brush::Gradient(g.clone().multiply_alpha(a)),
            Brush::Image(i) => Brush::Image(i.clone().multiply_alpha(a)),
        };
        if let Some(band) = r.band {
            scene.push_clip_layer(Fill::NonZero, pre * r.to_device, &band);
        }
        match geom {
            Geom::Fill(p, fill, t) => {
                let t = pre * *t;
                scene.fill(*fill, t, &brush, Some(t.inverse() * pre * r.to_device), *p);
            }
            Geom::Stroke(p, s, t) => {
                let t = pre * *t;
                scene.stroke(s, t, &brush, Some(t.inverse() * pre * r.to_device), *p);
            }
            Geom::Image(p, t, _, _) => {
                let t = pre * *t;
                scene.fill(
                    Fill::NonZero,
                    t,
                    &brush,
                    Some(t.inverse() * pre * r.to_device),
                    *p,
                );
            }
        }
        if r.band.is_some() {
            scene.pop_layer();
        }
    }

    /// Paint `geom` with `style` under the state's alpha, operator and
    /// shadow.
    pub fn paint(&mut self, frame: &mut Frame, host: &mut dyn Host, geom: Geom<'_>, style: &Style) {
        let resolved = match &geom {
            Geom::Image(_, _, image, to_user) => {
                let brush = ImageBrush::new(image.clone())
                    .with_quality(quality(self.state.smoothing, self.state.quality));
                Some(Resolved {
                    brush: Brush::Image(brush),
                    to_device: self.device() * *to_user,
                    band: None,
                })
            }
            _ => self.resolve(style, host),
        };
        let Some(r) = resolved else { return };
        let s = &self.state;
        let op = s.composite;
        let extent = exact_canvas::composite_clips_extent(op);
        let shadow =
            s.shadow_color[3] > 0.0 && (s.shadow_blur > 0.0 || s.shadow_offset != (0.0, 0.0));
        // Core Graphics: a shadow is a transparency layer at the alpha; the
        // clip-extent operators draw the shape at the alpha offscreen and
        // composite it at 1.
        let (layer_alpha, brush_alpha) = if shadow && !extent {
            (s.alpha, 1.0)
        } else {
            (1.0, s.alpha)
        };
        let whole = self.whole();
        let mut clip = bounds(&geom);
        let shadow_at = if shadow {
            self.shadow(frame, &geom, &r, brush_alpha)
        } else {
            None
        };
        if let Some((_, at)) = &shadow_at {
            clip = clip.union(*at);
        }
        let layer = op != 0 || shadow;
        if layer {
            let region = if extent { whole } else { clip.intersect(whole) };
            if !extent && region.is_zero_area() {
                return;
            }
            frame.scene.push_layer(
                Fill::NonZero,
                blend(op),
                layer_alpha as f32,
                Affine::IDENTITY,
                &region,
            );
        }
        if let Some((image, at)) = shadow_at {
            let brush = ImageBrush::new(image).with_quality(ImageQuality::Medium);
            frame
                .scene
                .draw_image(&brush, Affine::translate((at.x0, at.y0)));
        }
        Self::draw(&mut frame.scene, Affine::IDENTITY, &geom, &r, brush_alpha);
        if layer {
            frame.scene.pop_layer();
        }
    }

    /// The shadow of `geom`: an offscreen job and where its image lands
    /// (device pixels), or none when it cannot reach the canvas.
    fn shadow(
        &self,
        frame: &mut Frame,
        geom: &Geom<'_>,
        r: &Resolved,
        alpha: f64,
    ) -> Option<(ImageData, Rect)> {
        let s = &self.state;
        let sigma = (s.shadow_blur * self.scale / 2.0).max(0.0);
        let margin = (3.0 * sigma).ceil() + 1.0;
        let off = (
            s.shadow_offset.0 * self.scale,
            s.shadow_offset.1 * self.scale,
        );
        let limit = self
            .whole()
            .with_origin(Point::new(-off.0, -off.1))
            .inflate(margin, margin);
        let region = bounds(geom)
            .inflate(margin, margin)
            .intersect(limit)
            .expand();
        if region.width() < 1.0
            || region.height() < 1.0
            || region.width() > 16384.0
            || region.height() > 16384.0
        {
            return None;
        }
        let (w, h) = (region.width() as u32, region.height() as u32);
        let mut scene = vello::Scene::new();
        Self::draw(
            &mut scene,
            Affine::translate((-region.x0, -region.y0)),
            geom,
            r,
            alpha,
        );
        let image = ImageData {
            data: Blob::new(std::sync::Arc::new([0u8; 0])),
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::AlphaPremultiplied,
            width: w,
            height: h,
        };
        let c = s.shadow_color;
        frame.shadows.push(ShadowJob {
            scene,
            width: w,
            height: h,
            sigma,
            color: [
                (c[0] / 255.0) as f32,
                (c[1] / 255.0) as f32,
                (c[2] / 255.0) as f32,
                c[3] as f32,
            ],
            image: image.clone(),
        });
        Some((
            image,
            region.with_origin(Point::new(region.x0 + off.0, region.y0 + off.1)),
        ))
    }
}
