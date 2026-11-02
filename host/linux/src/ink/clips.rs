use super::*;
use exact_ink::instance::{Clip, CoverageClip, Scissor};
use exact_ink::path::Coverage;

pub(super) struct Group {
    parent: Encoder,
    target: Option<TextureId>,
    clear: Option<[f64; 4]>,
    opacity: f32,
    pub(super) transform: Option<Transform>,
}

impl Ink {
    pub(super) fn open_group(&mut self, opacity: f32) {
        let next = self.encoder.fork();
        let parent = std::mem::replace(&mut self.encoder, next);
        self.groups.push(Group {
            parent,
            target: self.draw_target.take(),
            clear: self.draw_clear.replace([0.; 4]),
            opacity,
            transform: None,
        });
    }

    /// Flatten overflowing clips into one mask, so every child sees their intersection.
    pub(super) fn combined_clip(&mut self, mask: Option<Coverage>) -> Result<CoverageClip> {
        let rect = [0., 0., self.size[0] as f32, self.size[1] as f32];
        let mut encoder = self.encoder.fork();
        encoder
            .push(
                Instance::new(Kind::RoundedRect, Brush::Solid, rect),
                Bindings::default(),
            )
            .map_err(|_| Error::Invalid("parent clip mask".into()))?;
        let parent = self.pool.take(&mut self.device)?;
        self.device
            .render(&encoder.finish(), parent, Some([0.; 4]))?;
        exact_ink::counters::add(exact_ink::counters::Counter::ClipMasks, 1);
        let texture = if let Some(mask) = mask {
            let mut encoder = Encoder::new(self.size[0], self.size[1]);
            let mut instance = Instance::new(Kind::Layer, Brush::Image, rect);
            instance.set_mask(CoverageClip {
                texture: mask.entry.texture,
                rect: mask.rect,
                uv: mask.entry.uv,
            });
            encoder
                .push(
                    instance,
                    Bindings {
                        image: parent,
                        mask: mask.entry.texture,
                        ..Default::default()
                    },
                )
                .map_err(|_| Error::Invalid("clip intersection".into()))?;
            let target = self.pool.take(&mut self.device)?;
            self.device
                .render(&encoder.finish(), target, Some([0.; 4]))?;
            exact_ink::counters::add(exact_ink::counters::Counter::ClipMasks, 1);
            target
        } else {
            parent
        };
        Ok(CoverageClip {
            texture,
            rect,
            uv: [0., 0., 1., 1.],
        })
    }

    pub(super) fn open_clip_group(&mut self, mask: Coverage) -> Result<()> {
        let clip = self.combined_clip(Some(mask))?;
        self.encoder.push_combined_clip(clip);
        Ok(())
    }

    pub(super) fn close_group(&mut self) -> Result<()> {
        self.close_masked_group(None)
    }

    pub(super) fn close_masked_group(&mut self, alpha_mask: Option<TextureId>) -> Result<()> {
        if self.groups.is_empty() {
            return Ok(());
        }
        let target = self.flush()?;
        let group = self.groups.pop().unwrap();
        self.encoder = group.parent;
        self.draw_target = group.target;
        self.draw_clear = group.clear;
        let mut instance = Instance::new(
            Kind::Layer,
            Brush::Image,
            [0., 0., self.size[0] as f32, self.size[1] as f32],
        );
        instance.param[0] = group.opacity;
        if let Some(ts) = group.transform {
            instance.xform = affine(ts);
        }
        let mut bindings = Bindings {
            image: target,
            ..Default::default()
        };
        if let Some(mask) = alpha_mask {
            instance.flags |= flags::LAYER_MASK;
            bindings.color = mask;
        }
        self.encoder
            .push_unclipped(instance, bindings, self.size)
            .map_err(|_| Error::Invalid("group composition".into()))
    }

    pub(super) fn clip(&mut self, shape: &Shape, ts: Transform) -> Result<()> {
        let ts = self.record_transform(ts);
        if self.record(rows::Marker::Clip(*shape, ts)) {
            return Ok(());
        }
        if shape.rect.2 <= 0. || shape.rect.3 <= 0. {
            self.encoder
                .push_clip(Clip::Scissor(Scissor {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                }))
                .map_err(|_| Error::Invalid("empty clip".into()))?;
            self.clips.push(());
            return Ok(());
        }
        let dev = self.device_transform(ts);
        if let Some(clip) = simple(shape, dev, self.size) {
            if self.encoder.push_clip(clip).is_ok() {
                self.clips.push(());
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
                Some(self.size),
            )?,
            None => None,
        };
        if let Some(mask) = coverage {
            self.open_clip_group(mask)?;
            self.clips.push(());
        } else {
            self.encoder
                .push_clip(Clip::Scissor(Scissor {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                }))
                .map_err(|_| Error::Invalid("empty clip".into()))?;
            self.clips.push(());
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
