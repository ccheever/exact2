use super::*;
use exact_ink::device::TextureDesc;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Images {
    bitmaps: HashMap<(usize, [u32; 2]), (Arc<Bitmap>, TextureId)>,
    pixmaps: HashMap<usize, (Arc<Pixmap>, TextureId)>,
    pins: HashMap<TextureId, usize>,
}

impl Images {
    pub fn pin(&mut self, texture: TextureId) {
        if self.bitmaps.values().any(|(_, t)| *t == texture)
            || self.pixmaps.values().any(|(_, t)| *t == texture)
        {
            *self.pins.entry(texture).or_default() += 1;
        }
    }

    pub fn unpin(&mut self, texture: TextureId) {
        if let Some(n) = self.pins.get_mut(&texture) {
            *n -= 1;
            if *n == 0 {
                self.pins.remove(&texture);
            }
        }
    }

    pub fn begin(&mut self, device: &mut impl Device) -> Result<()> {
        let mut counts = HashMap::new();
        for &(address, _) in self.bitmaps.keys() {
            *counts.entry(address).or_insert(0) += 1;
        }
        let dead: std::collections::HashSet<_> = self
            .bitmaps
            .iter()
            .filter_map(|(&(address, _), (p, _))| {
                (Arc::strong_count(p) == counts[&address]).then_some(address)
            })
            .collect();
        for (_, (_, texture)) in self
            .bitmaps
            .extract_if(|(address, _), (_, t)| dead.contains(address) && !self.pins.contains_key(t))
        {
            device.retire_texture(texture)?;
        }
        for (_, (_, texture)) in self
            .pixmaps
            .extract_if(|_, (p, t)| Arc::strong_count(p) == 1 && !self.pins.contains_key(t))
        {
            device.retire_texture(texture)?;
        }
        Ok(())
    }

    pub fn bitmap(
        &mut self,
        device: &mut impl Device,
        image: &Arc<Bitmap>,
        size: [u32; 2],
    ) -> Result<TextureId> {
        let key = (Arc::as_ptr(image) as usize, size);
        if let Some((_, texture)) = self.bitmaps.get(&key) {
            return Ok(*texture);
        }
        let texture = if size == [image.width(), image.height()] {
            upload(device, image.pixels())?
        } else {
            let pixels = exact_ink::image::reduce(
                image.pixels().data(),
                [image.width(), image.height()],
                size,
            );
            let pixels = tiny_skia::PixmapRef::from_bytes(&pixels, size[0], size[1]).unwrap();
            upload(device, pixels)?
        };
        self.bitmaps.insert(key, (image.clone(), texture));
        Ok(texture)
    }

    pub fn pixmap(&mut self, device: &mut impl Device, image: &Arc<Pixmap>) -> Result<TextureId> {
        let key = Arc::as_ptr(image) as usize;
        if let Some((_, texture)) = self.pixmaps.get(&key) {
            return Ok(*texture);
        }
        let texture = upload(device, image.as_ref().as_ref())?;
        self.pixmaps.insert(key, (image.clone(), texture));
        Ok(texture)
    }
}

fn upload(device: &mut impl Device, pixels: tiny_skia::PixmapRef<'_>) -> Result<TextureId> {
    #[cfg(target_os = "macos")]
    let _trace = exact_ink::device::apple::Trace::new(c"image upload");
    let texture = device.create_texture(TextureDesc {
        mipmapped: true,
        ..TextureDesc::rgba(pixels.width(), pixels.height())
    })?;
    device.upload_texture(
        texture,
        [0, 0],
        [pixels.width(), pixels.height()],
        pixels.data(),
    )?;
    device.generate_mips(texture)?;
    Ok(texture)
}

impl Ink {
    pub(super) fn picture(
        &mut self,
        texture: TextureId,
        dst: Rect4,
        clips: &[Shape],
        ts: Transform,
        tint: Option<[u8; 4]>,
    ) {
        if dst.2 <= 0. || dst.3 <= 0. {
            return;
        }
        for shape in clips {
            self.push_clip(shape, ts);
        }
        let mut instance = Instance::new(Kind::RoundedRect, Brush::Image, rect(dst));
        instance.xform = affine(self.device_transform(ts));
        if let Some(tint) = tint {
            instance.flags |= flags::TINT;
            instance.color = color(tint);
        }
        let result = self.emit(
            instance,
            Bindings {
                image: texture,
                ..Default::default()
            },
        );
        self.accept(result);
        for _ in clips {
            self.pop_clip();
        }
    }
}

/// A mip chain cannot reduce one axis while retaining detail in the other.
pub(super) fn sampling_size(size: [u32; 2], dst: Rect4, dev: Transform) -> [u32; 2] {
    let pixels = [
        dst.2.abs() * dev.sx.hypot(dev.ky),
        dst.3.abs() * dev.kx.hypot(dev.sy),
    ];
    let ratios = [pixels[0] / size[0] as f32, pixels[1] / size[1] as f32];
    if ratios[0].max(ratios[1]) > 2. * ratios[0].min(ratios[1]) {
        std::array::from_fn(|i| {
            if ratios[i] < 0.5 {
                (pixels[i].round() as u32).clamp(1, size[i])
            } else {
                size[i]
            }
        })
    } else {
        size
    }
}
