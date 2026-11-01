//! Composition stays on the device; only finish reads pixels. @ref LLP 1149 D2.5, D2.7.
use super::*;

impl Ink {
    pub(super) fn flush(&mut self) -> Result<TextureId> {
        if let Some(e) = &self.error {
            return Err(Error::Device(e.clone()));
        }
        let started = Instant::now();
        let target = match self.draw_target {
            Some(t) => t,
            None => {
                let t = self.pool.take(&mut self.device)?;
                self.draw_target = Some(t);
                t
            }
        };
        self.glyphs.flush(&mut self.device)?;
        let frame = self.encoder.take_frame();
        self.device.render(&frame, target, self.draw_clear.take())?;
        self.layer_ms += started.elapsed().as_secs_f64() * 1000.;
        Ok(target)
    }

    pub(super) fn masked(
        &mut self,
        shape: &Shape,
        mask: &std::result::Result<GradientPaint, [u8; 4]>,
        ts: Transform,
    ) -> Result<()> {
        let saved = std::mem::replace(&mut self.encoder, Encoder::new(self.size[0], self.size[1]));
        match mask {
            Ok(g) => self.fill_gradient(shape, g, ts),
            Err(c) => self.fill(shape, *c, ts),
        }
        let frame = std::mem::replace(&mut self.encoder, saved).finish();
        let started = Instant::now();
        let target = self.pool.take(&mut self.device)?;
        self.device.render(&frame, target, Some([0.; 4]))?;
        self.layer_ms += started.elapsed().as_secs_f64() * 1000.;
        self.close_masked_group(Some(target))
    }

    pub(super) fn destination(&mut self) -> Result<TextureId> {
        let target = self.flush()?;
        let started = Instant::now();
        let copy = self.pool.take(&mut self.device)?;
        self.device
            .copy_region(target, [0, 0], copy, [0, 0], self.size)?;
        self.layer_ms += started.elapsed().as_secs_f64() * 1000.;
        Ok(copy)
    }

    pub(super) fn blended_image(
        &mut self,
        pixels: &Arc<Pixmap>,
        dst: Rect4,
        ts: Transform,
        mode: u8,
    ) -> Result<()> {
        if dst.2 <= 0. || dst.3 <= 0. {
            return Ok(());
        }
        let texture = self.images.pixmap(&mut self.device, pixels)?;
        let destination = if mode == 0 || !self.rows.capture.is_empty() {
            TextureId(0)
        } else {
            self.destination()?
        };
        let mut i = Instance::new(Kind::RoundedRect, Brush::Image, rect(dst));
        i.xform = affine(self.device_transform(ts));
        i.flags |= u32::from(mode.min(15)) << flags::BLEND_SHIFT;
        self.emit(
            i,
            Bindings {
                image: texture,
                destination,
                ..Default::default()
            },
        )?;
        // A following blend must see this result, including within an opacity root.
        if mode != 0 && self.rows.capture.is_empty() {
            self.flush()?;
        }
        Ok(())
    }

    fn effect(
        &mut self,
        source: TextureId,
        region: [f32; 4],
        kind: Kind,
        param: [f32; 4],
    ) -> Result<TextureId> {
        self.effect_sized(source, region, kind, param, self.size)
    }

    fn effect_sized(
        &mut self,
        source: TextureId,
        region: [f32; 4],
        kind: Kind,
        param: [f32; 4],
        size: [u32; 2],
    ) -> Result<TextureId> {
        let started = Instant::now();
        let target = self.pool.take_sized(&mut self.device, size)?;
        let mut e = Encoder::new(size[0], size[1]);
        let mut i = Instance::new(kind, Brush::Solid, region);
        i.param = param;
        e.push(
            i,
            Bindings {
                image: source,
                ..Default::default()
            },
        )
        .map_err(|_| Error::Invalid("effect clip".into()))?;
        self.device.render(&e.finish(), target, Some([0.; 4]))?;
        self.layer_ms += started.elapsed().as_secs_f64() * 1000.;
        Ok(target)
    }

    fn resample(
        &mut self,
        source: TextureId,
        region: [f32; 4],
        uv: [f32; 4],
        size: [u32; 2],
        area: bool,
    ) -> Result<TextureId> {
        let target = self.pool.take_sized(&mut self.device, size)?;
        let mut encoder = Encoder::new(size[0], size[1]);
        let mut i = Instance::new(
            if area { Kind::Downsample } else { Kind::Layer },
            Brush::Image,
            region,
        );
        i.uv = uv;
        encoder
            .push(
                i,
                Bindings {
                    image: source,
                    ..Default::default()
                },
            )
            .map_err(|_| Error::Invalid("resample clip".into()))?;
        self.device
            .render(&encoder.finish(), target, Some([0.; 4]))?;
        Ok(target)
    }

    pub(super) fn backdrop(
        &mut self,
        shape: &Shape,
        filter: &exact_kernel::style::BackdropFilter,
        ts: Transform,
    ) -> Result<()> {
        let dev = self.device_transform(ts);
        let Some(path) = shape_path(shape).and_then(|p| p.transform(dev)) else {
            return Ok(());
        };
        if dev.invert().is_none() {
            return Ok(());
        }
        let b = path.bounds();
        let x = b.left().floor().max(0.);
        let y = b.top().floor().max(0.);
        let right = b.right().ceil().min(self.size[0] as f32);
        let bottom = b.bottom().ceil().min(self.size[1] as f32);
        if right <= x || bottom <= y {
            return Ok(());
        }
        let region = [x, y, right - x, bottom - y];
        let original = self.destination()?;
        let mut source = original;
        let scale = (dev.sx * dev.sy - dev.kx * dev.ky).abs().sqrt();
        for op in &filter.0 {
            match *op {
                exact_kernel::style::BackdropOp::Saturate(amount) => {
                    source = self.effect(source, region, Kind::ColorPass, [amount, 0., 0., 0.])?;
                }
                exact_kernel::style::BackdropOp::Blur(sigma) => {
                    let mut sigma = sigma * scale;
                    if !sigma.is_finite() || sigma <= 0. {
                        continue;
                    }
                    let mut size = self.size;
                    let mut area = region;
                    let mut uv = [
                        region[0] / size[0] as f32,
                        region[1] / size[1] as f32,
                        region[2] / size[0] as f32,
                        region[3] / size[1] as f32,
                    ];
                    let reduced = sigma > 12.;
                    // Halving bounds each area pass to at most nine source pixels.
                    while sigma > 12. {
                        size = [
                            (area[2] / 2.).ceil().max(1.) as u32,
                            (area[3] / 2.).ceil().max(1.) as u32,
                        ];
                        let next = [0., 0., size[0] as f32, size[1] as f32];
                        source = self.resample(source, next, uv, size, true)?;
                        sigma *= 0.5;
                        area = next;
                        uv = [0., 0., 1., 1.];
                    }
                    for vertical in [0., 1.] {
                        if sigma < 2. {
                            let r = (3. * sigma).ceil();
                            source = self.effect_sized(
                                source,
                                area,
                                Kind::Blur,
                                [r, r, vertical, sigma],
                                size,
                            )?;
                        } else {
                            let d = (sigma * 3. * (2. * std::f32::consts::PI).sqrt() / 4. + 0.5)
                                .floor()
                                .max(1.) as u32;
                            let r = (d / 2) as f32;
                            let passes = if d % 2 == 1 {
                                [(r, r); 3]
                            } else {
                                [(r, r - 1.), (r - 1., r), (r, r)]
                            };
                            for (left, right) in passes {
                                source = self.effect_sized(
                                    source,
                                    area,
                                    Kind::Blur,
                                    [left, right, vertical, 0.],
                                    size,
                                )?;
                            }
                        }
                    }
                    if reduced {
                        source =
                            self.resample(source, region, [0., 0., 1., 1.], self.size, false)?;
                    }
                }
            }
        }
        if let Some((mut i, mut bindings, geometry)) = self.shape_instance(shape, ts)? {
            i.kind_brush = (i.kind_brush & 65535) | ((Brush::Image as u32) << 16);
            i.flags |= flags::REPLACE;
            bindings.image = source;
            bindings.destination = original;
            self.emit_geometry(i, bindings, geometry)?;
        }
        self.flush()?;
        Ok(())
    }
}
