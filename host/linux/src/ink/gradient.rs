use super::*;
use exact_kernel::gradient::{premultiplied_ramp, Geometry};

impl Ink {
    pub(super) fn gradient(
        &mut self,
        shape: &Shape,
        gradient: &GradientPaint,
        ts: Transform,
    ) -> Result<()> {
        let Some((mut instance, mut bindings)) = self.shape_instance(shape, ts)? else {
            return Ok(());
        };
        let stops = premultiplied_ramp(&gradient.stops);
        let Some((ramp, flags)) = self.ramp(&stops)? else {
            return Ok(());
        };
        instance.kind_brush = (instance.kind_brush & 65535) | ((Brush::Gradient as u32) << 16);
        instance.flags |= flags | (ramp.row << flags::RAMP_SHIFT);
        if gradient.repeat {
            instance.flags |= flags::REPEAT;
        }
        instance.param = match gradient.geometry {
            Geometry::Linear { start, end } => [start.0, start.1, end.0, end.1],
            Geometry::Radial { center, radii } => {
                instance.flags |= flags::RADIAL;
                [center.0, center.1, radii.0, radii.1]
            }
            Geometry::Conic { center, from } => {
                instance.flags |= flags::CONIC;
                [center.0, center.1, from.to_radians(), 0.]
            }
        };
        bindings.ramp = ramp.texture;
        self.emit(instance, bindings)
    }
    pub(super) fn ramp(
        &mut self,
        stops: &[(f32, exact_kernel::Color)],
    ) -> Result<Option<(exact_ink::atlas::Entry, u32)>> {
        let key: Vec<u32> = stops
            .iter()
            .flat_map(|(at, c)| {
                [
                    at.to_bits(),
                    u32::from_be_bytes([c.r(), c.g(), c.b(), c.a()]),
                ]
            })
            .collect();
        let exact = stops.windows(2).any(|p| p[1].0 - p[0].0 < 1. / 255.);
        let ramp = if let Some(entry) = self.ramps.get(&key) {
            entry
        } else if exact {
            let width = (1 + 2 * stops.len()).max(256) as u32;
            let mut bytes = vec![0; width as usize * 4];
            bytes[..4].copy_from_slice(&(stops.len() as u32).to_le_bytes());
            for (n, (at, c)) in stops.iter().enumerate() {
                let start = (1 + 2 * n) * 4;
                bytes[start..start + 4].copy_from_slice(&at.to_le_bytes());
                bytes[start + 4..start + 8].copy_from_slice(&[c.r(), c.g(), c.b(), c.a()]);
            }
            self.ramps
                .insert(&mut self.device, key, [width, 1], &bytes)?
        } else {
            // Sample the oracle's expanded, straight-alpha stops at texel centers.
            let stops = stops
                .iter()
                .copied()
                .map(|(at, c)| {
                    tiny_skia::GradientStop::new(
                        at,
                        tiny_skia::Color::from_rgba8(c.r(), c.g(), c.b(), c.a()),
                    )
                })
                .collect();
            let Some(shader) = tiny_skia::LinearGradient::new(
                tiny_skia::Point::from_xy(0.5, 0.),
                tiny_skia::Point::from_xy(255.5, 0.),
                stops,
                tiny_skia::SpreadMode::Pad,
                Transform::identity(),
            ) else {
                return Ok(None);
            };
            let mut pixels =
                Pixmap::new(256, 1).ok_or_else(|| Error::Invalid("ramp allocation".into()))?;
            pixels.fill_rect(
                tiny_skia::Rect::from_xywh(0., 0., 256., 1.).unwrap(),
                &tiny_skia::Paint {
                    shader,
                    ..Default::default()
                },
                Transform::identity(),
                None,
            );
            self.ramps
                .insert(&mut self.device, key, [256, 1], pixels.data())?
        };
        Ok(Some((ramp, if exact { flags::STOP_RAMP } else { 0 })))
    }
}
