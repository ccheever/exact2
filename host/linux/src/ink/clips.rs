use super::*;
use exact_ink::instance::{Clip, CoverageClip, Scissor};
use exact_ink::path::Coverage;

pub(super) struct Group {
    parent: Encoder,
    opacity: f32,
    mask: Option<Coverage>,
}

impl Ink {
    pub(super) fn open_group(&mut self, opacity: f32, mask: Option<Coverage>) {
        let next = Encoder::new(self.size[0], self.size[1]);
        let parent = std::mem::replace(&mut self.encoder, next);
        self.groups.push(Group {
            parent,
            opacity,
            mask,
        });
    }

    pub(super) fn close_group(&mut self) -> Result<()> {
        let Some(group) = self.groups.pop() else {
            return Ok(());
        };
        let child = std::mem::replace(&mut self.encoder, group.parent).finish();
        if self.error.is_some() {
            return Ok(());
        }
        let started = Instant::now();
        let target = self.pool.take(&mut self.device)?;
        self.device.render(&child, target, Some([0.; 4]))?;
        self.layer_ms += started.elapsed().as_secs_f64() * 1000.;
        let mut instance = Instance::new(
            Kind::Layer,
            Brush::Image,
            [0., 0., self.size[0] as f32, self.size[1] as f32],
        );
        instance.param[0] = group.opacity;
        let mut bindings = Bindings {
            image: target,
            ..Default::default()
        };
        if let Some(mask) = group.mask {
            instance.set_mask(CoverageClip {
                rect: mask.rect,
                uv: mask.entry.uv,
            });
            bindings.path = mask.entry.texture;
        }
        self.emit(instance, bindings)
    }

    pub(super) fn clip(&mut self, shape: &Shape, ts: Transform) -> Result<()> {
        if shape.rect.2 <= 0. || shape.rect.3 <= 0. {
            self.encoder
                .push_clip(Clip::Scissor(Scissor {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                }))
                .map_err(|_| Error::Invalid("empty clip".into()))?;
            self.clips.push(false);
            return Ok(());
        }
        let dev = self.device_transform(ts);
        if let Some(clip) = simple(shape, dev, self.size) {
            if self.encoder.push_clip(clip).is_ok() {
                self.clips.push(false);
                return Ok(());
            }
            self.encoder.pop_clip();
        }
        let coverage = match shape_path(shape) {
            Some(path) => exact_ink::path::coverage(
                &mut self.paths,
                &mut self.device,
                &path,
                FillRule::Winding,
                &[],
                dev,
                self.size,
            )?,
            None => None,
        };
        if let Some(mask) = coverage {
            self.open_group(1., Some(mask));
            self.clips.push(true);
        } else {
            self.encoder
                .push_clip(Clip::Scissor(Scissor {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                }))
                .map_err(|_| Error::Invalid("empty clip".into()))?;
            self.clips.push(false);
        }
        Ok(())
    }
}

fn simple(shape: &Shape, dev: Transform, size: [u32; 2]) -> Option<Clip> {
    if !circular(shape)
        || !dev.is_finite()
        || dev.kx != 0.
        || dev.ky != 0.
        || dev.sx == 0.
        || dev.sy == 0.
    {
        return None;
    }
    let (x, y, w, h) = shape.rect;
    let (x0, y0, x1, y1) = (
        x * dev.sx + dev.tx,
        y * dev.sy + dev.ty,
        (x + w) * dev.sx + dev.tx,
        (y + h) * dev.sy + dev.ty,
    );
    let edges = [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)];
    if !shape.rounded() && edges.iter().all(|e| e.fract() == 0.) {
        let x = edges[0].clamp(0., size[0] as f32) as u32;
        let y = edges[1].clamp(0., size[1] as f32) as u32;
        return Some(Clip::Scissor(Scissor {
            x,
            y,
            width: (edges[2].clamp(0., size[0] as f32) as u32).saturating_sub(x),
            height: (edges[3].clamp(0., size[1] as f32) as u32).saturating_sub(y),
        }));
    }
    if shape.rounded() && dev.sx.abs() != dev.sy.abs() {
        return None;
    }
    let mut radii = shape.radii.map(|r| r.0 * dev.sx.abs());
    if dev.sx < 0. {
        radii.swap(0, 1);
        radii.swap(3, 2);
    }
    if dev.sy < 0. {
        radii.swap(0, 3);
        radii.swap(1, 2);
    }
    Some(Clip::Rounded(RoundedClip {
        rect: [edges[0], edges[1], edges[2] - edges[0], edges[3] - edges[1]],
        radii,
    }))
}
