use super::*;
use exact_ink::device::TextureDesc;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Images {
    bitmaps: HashMap<usize, (Arc<Bitmap>, TextureId)>,
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
        for (_, (_, texture)) in self
            .bitmaps
            .extract_if(|_, (p, t)| Arc::strong_count(p) == 1 && !self.pins.contains_key(t))
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

    pub fn bitmap(&mut self, device: &mut impl Device, image: &Arc<Bitmap>) -> Result<TextureId> {
        let key = Arc::as_ptr(image) as usize;
        if let Some((_, texture)) = self.bitmaps.get(&key) {
            return Ok(*texture);
        }
        let texture = upload(device, image.pixels())?;
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
