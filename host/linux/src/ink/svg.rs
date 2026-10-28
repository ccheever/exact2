use super::*;
use crate::paint::{Ink as Paint, SvgPaint};
use exact_ink::instance::{Clip, CoverageClip};

pub(super) fn svg_path(path: &exact_kernel::svg::Path) -> Option<Path> {
    let mut b = PathBuilder::new();
    for seg in &path.0 {
        match *seg {
            exact_kernel::svg::Seg::Move(x, y) => b.move_to(x, y),
            exact_kernel::svg::Seg::Line(x, y) => b.line_to(x, y),
            exact_kernel::svg::Seg::Cubic(a, c, d, e, x, y) => b.cubic_to(a, c, d, e, x, y),
            exact_kernel::svg::Seg::Close => b.close(),
        }
    }
    b.finish()
}

impl Ink {
    pub(super) fn svg(&mut self, s: &SvgPaint<'_>, ts: Transform) -> Result<()> {
        let Some(path) = svg_path(s.path) else {
            return Ok(());
        };
        for part in s.order {
            let (region, ink, rule) = match part {
                0 => (
                    path.clone(),
                    s.fill.as_ref(),
                    if s.even_odd {
                        FillRule::EvenOdd
                    } else {
                        FillRule::Winding
                    },
                ),
                1 if s.width > 0. => {
                    let stroke = tiny_skia::Stroke {
                        width: s.width,
                        miter_limit: s.miter,
                        line_cap: [
                            tiny_skia::LineCap::Butt,
                            tiny_skia::LineCap::Round,
                            tiny_skia::LineCap::Square,
                        ][s.cap.min(2) as usize],
                        line_join: [
                            tiny_skia::LineJoin::Miter,
                            tiny_skia::LineJoin::Round,
                            tiny_skia::LineJoin::Bevel,
                        ][s.join.min(2) as usize],
                        dash: (!s.dash.is_empty())
                            .then(|| tiny_skia::StrokeDash::new(s.dash.clone(), s.phase))
                            .flatten(),
                    };
                    let dev = self.device_transform(ts);
                    let resolution = dev.sx.hypot(dev.ky).max(dev.kx.hypot(dev.sy));
                    let dashed = match &stroke.dash {
                        Some(dash) => path.dash(dash, resolution),
                        None => Some(path.clone()),
                    };
                    let Some(p) = dashed.and_then(|p| p.stroke(&stroke, resolution)) else {
                        continue;
                    };
                    (p, s.stroke.as_ref(), FillRule::Winding)
                }
                _ => continue,
            };
            let Some(ink) = ink else {
                continue;
            };
            let Some((mut i, mut b)) = self.path_instance(&region, rule, &[], ts)? else {
                continue;
            };
            match ink {
                Paint::Solid(c) => i.color = color(*c),
                Paint::Pattern {
                    tile,
                    transform,
                    opacity,
                } => {
                    i.kind_brush = Kind::Coverage as u32 | ((Brush::Pattern as u32) << 16);
                    i.xform = affine(
                        self.device_transform(ts)
                            .pre_concat(from_affine(*transform)),
                    );
                    i.param = [0., 0., tile.width() as f32, tile.height() as f32];
                    i.color = [*opacity; 4];
                    b.image = self.images.pixmap(&mut self.device, tile)?;
                }
                Paint::Gradient {
                    server,
                    stops,
                    transform,
                } => {
                    use exact_kernel::svg::server::{ServerKind, Spread};
                    let stops: Vec<_> = stops
                        .iter()
                        .map(|(p, c)| (*p, exact_kernel::Color::rgba(c[0], c[1], c[2], c[3])))
                        .collect();
                    let Some(ramp) = self.ramp(&stops)? else {
                        continue;
                    };
                    i.kind_brush = Kind::Coverage as u32 | ((Brush::Gradient as u32) << 16);
                    i.flags |= ramp.row << flags::RAMP_SHIFT;
                    i.flags |= match server.spread {
                        Spread::Pad => 0,
                        Spread::Reflect => flags::REFLECT,
                        Spread::Repeat => flags::REPEAT,
                    };
                    i.xform = affine(
                        self.device_transform(ts)
                            .pre_concat(from_affine(*transform)),
                    );
                    match server.kind {
                        ServerKind::Linear { x1, y1, x2, y2 } => i.param = [x1, y1, x2, y2],
                        ServerKind::Radial {
                            cx,
                            cy,
                            r,
                            fx,
                            fy,
                            fr,
                        } => {
                            i.flags |= flags::SVG_RADIAL;
                            i.param = [cx, cy, r, fr];
                            i.radii = [fx, fy, 0., 0.];
                        }
                    }
                    b.ramp = ramp.texture;
                }
            }
            if from_affine(i.xform).invert().is_some() {
                self.emit(i, b)?;
            }
        }
        Ok(())
    }

    pub(super) fn coverage_clip(
        &mut self,
        coverage: exact_ink::path::Coverage,
        transformed: bool,
    ) -> Result<()> {
        let mut coverage = coverage;
        if let Some(c) = self.rows.capture.last() {
            let offset = c.offset(self.scale);
            coverage.rect[0] += offset[0];
            coverage.rect[1] += offset[1];
        }
        if self.record(rows::Marker::Coverage(coverage, transformed)) {
            return Ok(());
        }
        let clip = CoverageClip {
            texture: coverage.entry.texture,
            rect: coverage.rect,
            uv: coverage.entry.uv,
        };
        if !transformed {
            if self.encoder.push_clip(Clip::Coverage(clip)).is_ok() {
                self.clips.push(false);
                return Ok(());
            }
            self.encoder.pop_clip();
        }
        self.open_clip_group(coverage)?;
        self.clips.push(false);
        Ok(())
    }

    pub(super) fn css_clip(
        &mut self,
        css: &exact_kernel::clip::ClipPath,
        ts: Transform,
    ) -> Result<bool> {
        let mut b = PathBuilder::new();
        for (op, v) in css.commands() {
            match op {
                'M' => b.move_to(v[0], v[1]),
                'L' => b.line_to(v[0], v[1]),
                'Q' => b.quad_to(v[0], v[1], v[2], v[3]),
                'C' => b.cubic_to(v[0], v[1], v[2], v[3], v[4], v[5]),
                'Z' => b.close(),
                _ => unreachable!("validated CSS path"),
            }
        }
        let Some(path) = b.finish() else {
            return Ok(false);
        };
        let rule = if css.rule() == exact_kernel::FillRule::Evenodd {
            FillRule::EvenOdd
        } else {
            FillRule::Winding
        };
        let dev = self.device_transform(ts);
        let c = exact_ink::path::clip_union(
            &mut self.paths,
            &mut self.device,
            &[vec![(path, rule)]],
            dev,
            self.rows.capture.is_empty().then_some(self.size),
        )?;
        self.coverage_clip(c, dev.kx != 0. || dev.ky != 0.)?;
        Ok(true)
    }

    pub(super) fn svg_clip(
        &mut self,
        clip: &exact_kernel::svg::scene::Clip,
        ts: Transform,
    ) -> Result<usize> {
        let mut groups = Vec::new();
        let mut current = Some(clip);
        while let Some(clip) = current {
            groups.push(
                clip.shapes
                    .iter()
                    .filter_map(|s| {
                        svg_path(&s.path).map(|p| {
                            (
                                p,
                                if s.even_odd {
                                    FillRule::EvenOdd
                                } else {
                                    FillRule::Winding
                                },
                            )
                        })
                    })
                    .collect(),
            );
            current = clip.then.as_deref();
        }
        let dev = self.device_transform(ts);
        let c = exact_ink::path::clip_union(
            &mut self.paths,
            &mut self.device,
            &groups,
            dev,
            self.rows.capture.is_empty().then_some(self.size),
        )?;
        self.coverage_clip(c, dev.kx != 0. || dev.ky != 0.)?;
        Ok(1)
    }
}

fn from_affine(m: [f32; 6]) -> Transform {
    Transform::from_row(m[0], m[1], m[2], m[3], m[4], m[5])
}
