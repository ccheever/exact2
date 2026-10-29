//! Shelf pages stay valid while a frame or kept row references them. @ref LLP 1149 D4, D8.
use crate::device::{Device, Error, Format, Result, Submission, TextureDesc};
use crate::instance::TextureId;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub texture: TextureId,
    pub uv: [f32; 4],
    pub row: u32,
}

struct Page {
    id: TextureId,
    x: u32,
    y: u32,
    shelf: u32,
    used: u64,
    upload: Submission,
}

pub struct Atlas {
    size: [u32; 2],
    format: Format,
    padding: u32,
    ceiling: usize,
    frame: u64,
    pages: Vec<Page>,
    entries: HashMap<Vec<u32>, Entry>,
    overflow: Vec<TextureId>,
    overflow_sizes: HashMap<TextureId, u64>,
    large_used: HashMap<TextureId, u64>,
    pins: HashMap<TextureId, usize>,
    stats: Stats,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub entries: usize,
    pub bytes: u64,
    pub misses: u64,
    pub hits: u64,
    pub uploaded_bytes: u64,
    pub overflow_bytes: u64,
    pub overflow_textures: usize,
    pub pinned_textures: usize,
}

impl Atlas {
    pub fn glyphs() -> Self {
        Self::new([512, 512], Format::Alpha, 1, 16)
    }

    pub fn colors() -> Self {
        Self::new([256, 256], Format::Rgba, 1, 16)
    }

    pub fn stats(&self) -> Stats {
        Stats {
            entries: self.entries.len(),
            bytes: self.pages.len() as u64
                * u64::from(self.size[0])
                * u64::from(self.size[1])
                * u64::from(self.format.channels()),
            overflow_textures: self.overflow.len(),
            pinned_textures: self.pins.len(),
            overflow_bytes: self.overflow_sizes.values().sum(),
            ..self.stats
        }
    }

    pub fn contains(&self, key: &[u32]) -> bool {
        self.entries.contains_key(key)
    }

    pub fn paths() -> Self {
        Self::new([512, 512], Format::Alpha, 1, 16)
    }

    pub fn ramps() -> Self {
        Self::new([256, 64], Format::Rgba, 0, 16)
    }

    /// The usable extent after reserving the filtering border.
    pub fn max_entry_dimension(&self, device: &impl Device) -> u32 {
        device
            .max_texture_dimension()
            .saturating_sub(2 * self.padding)
    }

    pub(crate) fn new(size: [u32; 2], format: Format, padding: u32, ceiling: usize) -> Self {
        Self {
            size,
            format,
            padding,
            ceiling,
            frame: 0,
            pages: Vec::new(),
            entries: HashMap::new(),
            overflow: Vec::new(),
            overflow_sizes: HashMap::new(),
            large_used: HashMap::new(),
            pins: HashMap::new(),
            stats: Stats::default(),
        }
    }

    pub fn begin(&mut self, device: &mut impl Device) -> Result<()> {
        self.frame += 1;
        self.stats = Stats::default();
        let retired: Vec<_> = self
            .overflow
            .extract_if(.., |id| {
                !self.pins.contains_key(id)
                    && self
                        .large_used
                        .get(id)
                        .is_none_or(|used| self.frame.saturating_sub(*used) > 120)
            })
            .collect();
        for id in retired {
            self.overflow_sizes.remove(&id);
            self.large_used.remove(&id);
            self.entries.retain(|_, e| e.texture != id);
            device.retire_texture(id)?;
        }
        Ok(())
    }

    /// Page-level pinning also protects neighbouring entries from page eviction.
    pub fn pin(&mut self, texture: TextureId) {
        if self.pages.iter().any(|p| p.id == texture) || self.overflow.contains(&texture) {
            *self.pins.entry(texture).or_default() += 1;
        }
    }

    pub fn unpin(&mut self, texture: TextureId) {
        if let Some(count) = self.pins.get_mut(&texture) {
            *count -= 1;
            if *count == 0 {
                self.pins.remove(&texture);
            }
        }
    }

    pub fn touch(&mut self, texture: TextureId) {
        if let Some(used) = self.large_used.get_mut(&texture) {
            *used = self.frame;
        }
        if let Some(page) = self.pages.iter_mut().find(|p| p.id == texture) {
            page.used = self.frame;
        }
    }

    pub fn get(&mut self, key: &[u32]) -> Option<Entry> {
        let Some(&entry) = self.entries.get(key) else {
            self.stats.misses += 1;
            return None;
        };
        self.stats.hits += 1;
        self.touch(entry.texture);
        Some(entry)
    }

    pub fn insert(
        &mut self,
        device: &mut impl Device,
        key: Vec<u32>,
        size: [u32; 2],
        pixels: &[u8],
    ) -> Result<Entry> {
        if size.iter().any(|s| *s > self.max_entry_dimension(device)) {
            return Err(Error::Invalid(
                "atlas entry exceeds device texture limit".into(),
            ));
        }
        let completed = device.completed()?;
        let mut busy = std::collections::HashSet::new();
        for p in &self.pages {
            let last = device.last_use(p.id)?;
            // The device orders consecutive uploads; only an unfinished draw
            // prevents appending to the page. @ref LLP 1149 D11, S2.5.
            if last > completed && last != p.upload {
                busy.insert(p.id);
            }
        }
        let padded = size.map(|s| s + 2 * self.padding);
        let mut slot = None;
        if padded[0] <= self.size[0] && padded[1] <= self.size[1] {
            for page in &mut self.pages {
                if busy.contains(&page.id) {
                    continue;
                }
                if let Some(origin) = place(page, self.size, padded) {
                    slot = Some((page.id, origin));
                    page.used = self.frame;
                    break;
                }
            }
            if slot.is_none() {
                let oldest = self
                    .pages
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| {
                        p.used != self.frame
                            && !self.pins.contains_key(&p.id)
                            && !busy.contains(&p.id)
                    })
                    .min_by_key(|(_, p)| p.used)
                    .map(|(i, _)| i);
                if self.pages.len() < self.ceiling || oldest.is_some() {
                    let mut page = if self.pages.len() < self.ceiling {
                        let id = device.create_texture(TextureDesc {
                            width: self.size[0],
                            height: self.size[1],
                            format: self.format,
                            mipmapped: false,
                        })?;
                        Page {
                            id,
                            x: 0,
                            y: 0,
                            shelf: 0,
                            used: self.frame,
                            upload: Submission(0),
                        }
                    } else {
                        let old = self.pages.swap_remove(oldest.unwrap());
                        self.entries.retain(|_, e| e.texture != old.id);
                        Page {
                            x: 0,
                            y: 0,
                            shelf: 0,
                            used: self.frame,
                            ..old
                        }
                    };
                    slot = Some((page.id, place(&mut page, self.size, padded).unwrap()));
                    self.pages.push(page);
                }
            }
        }
        let (texture, origin, extent) = if let Some((id, origin)) = slot {
            (id, origin, self.size)
        } else {
            let id = device.create_texture(TextureDesc {
                width: padded[0],
                height: padded[1],
                format: self.format,
                mipmapped: false,
            })?;
            self.overflow.push(id);
            if padded[0] > self.size[0] || padded[1] > self.size[1] {
                self.large_used.insert(id, self.frame);
            }
            self.overflow_sizes.insert(
                id,
                u64::from(padded[0]) * u64::from(padded[1]) * u64::from(self.format.channels()),
            );
            (id, [0, 0], padded)
        };
        let channels = self.format.channels() as usize;
        let mut upload = vec![0; padded[0] as usize * padded[1] as usize * channels];
        for (y, row) in pixels.chunks_exact(size[0] as usize * channels).enumerate() {
            let start = ((y + self.padding as usize) * padded[0] as usize + self.padding as usize)
                * channels;
            upload[start..start + row.len()].copy_from_slice(row);
        }
        let submitted = device.upload_texture(texture, origin, padded, &upload)?;
        if let Some(page) = self.pages.iter_mut().find(|p| p.id == texture) {
            page.upload = submitted;
        }
        self.stats.uploaded_bytes += upload.len() as u64;
        let entry = Entry {
            texture,
            uv: [
                (origin[0] + self.padding) as f32 / extent[0] as f32,
                (origin[1] + self.padding) as f32 / extent[1] as f32,
                size[0] as f32 / extent[0] as f32,
                size[1] as f32 / extent[1] as f32,
            ],
            row: origin[1] + self.padding,
        };
        self.entries.insert(key, entry);
        Ok(entry)
    }
}

fn place(page: &mut Page, extent: [u32; 2], size: [u32; 2]) -> Option<[u32; 2]> {
    let (mut x, mut y, mut shelf) = (page.x, page.y, page.shelf);
    if x + size[0] > extent[0] {
        x = 0;
        y += shelf;
        shelf = 0;
    }
    if y + size[1] > extent[1] {
        return None;
    }
    page.x = x + size[0];
    page.y = y;
    page.shelf = shelf.max(size[1]);
    Some([x, y])
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::device::Error;
    use crate::instance::Bindings;
    use crate::{Brush, Encoder, Instance, Kind, Wgpu};

    #[test]
    fn kept_references_pin_pages_and_overflow_until_released() {
        let mut device = Wgpu::new().expect("native adapter");
        let mut atlas = Atlas::new([4, 4], Format::Alpha, 0, 1);
        atlas.begin(&mut device).unwrap();
        let kept = atlas
            .insert(&mut device, vec![1], [4, 4], &[73; 16])
            .unwrap();
        atlas.pin(kept.texture);
        atlas.pin(kept.texture);
        atlas.unpin(kept.texture);
        atlas.begin(&mut device).unwrap();
        let spill = atlas
            .insert(&mut device, vec![2], [4, 4], &[181; 16])
            .unwrap();
        assert_ne!(kept.texture, spill.texture);
        atlas.pin(spill.texture);
        atlas.begin(&mut device).unwrap();
        assert!(atlas.contains(&[1]) && atlas.contains(&[2]));
        assert_eq!(atlas.stats().pinned_textures, 2);
        assert_eq!(atlas.stats().overflow_bytes, 16);
        let mut encoder = Encoder::new(2, 1);
        for (x, entry) in [kept, spill].into_iter().enumerate() {
            let mut i = Instance::new(Kind::Coverage, Brush::Solid, [x as f32, 0., 1., 1.]);
            i.uv = entry.uv;
            encoder
                .push(
                    i,
                    Bindings {
                        path: entry.texture,
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        let target = device.create_target(2, 1).unwrap();
        device
            .render(&encoder.finish(), target, Some([0.; 4]))
            .unwrap();
        assert_eq!(
            device.readback(target).unwrap(),
            [73, 73, 73, 73, 181, 181, 181, 181]
        );
        atlas.unpin(kept.texture);
        atlas.unpin(spill.texture);
        atlas.begin(&mut device).unwrap();
        let replacement = atlas
            .insert(&mut device, vec![3], [4, 4], &[255; 16])
            .unwrap();
        assert_eq!(replacement.texture, kept.texture);
        assert!(!atlas.contains(&[1]) && !atlas.contains(&[2]));
        assert_eq!(atlas.stats().overflow_textures, 0);
    }

    #[test]
    fn a_full_atlas_preserves_frame_references_through_spill_and_eviction() {
        let mut device = match Wgpu::new() {
            Ok(device) => device,
            Err(Error::NoAdapter(e)) => {
                eprintln!("no GPU here: {e}");
                return;
            }
            Err(e) => panic!("ink device: {e}"),
        };
        let mut atlas = Atlas::paths();
        let target = device.create_target(18, 1).unwrap();
        for frame in 0..3 {
            atlas.begin(&mut device).unwrap();
            assert_eq!(atlas.stats().overflow_bytes, 0);
            assert_eq!(atlas.stats().overflow_textures, 0);
            let mut encoder = Encoder::new(18, 1);
            for n in 0..18 {
                assert!(atlas.get(&[frame, n]).is_none());
                let alpha = (n * 11 + frame) as u8;
                let entry = atlas
                    .insert(
                        &mut device,
                        vec![frame, n],
                        [510, 510],
                        &vec![alpha; 510 * 510],
                    )
                    .unwrap();
                let mut instance =
                    Instance::new(Kind::Coverage, Brush::Solid, [n as f32, 0., 1., 1.]);
                instance.uv = entry.uv;
                encoder
                    .push(
                        instance,
                        Bindings {
                            path: entry.texture,
                            ..Default::default()
                        },
                    )
                    .unwrap();
            }
            device
                .render(&encoder.finish(), target, Some([0.; 4]))
                .unwrap();
            let pixels = device.readback(target).unwrap();
            for (n, pixel) in pixels.chunks_exact(4).enumerate() {
                assert_eq!(pixel, &[(n * 11 + frame as usize) as u8; 4]);
            }
            let stats = atlas.stats();
            assert_eq!(stats.entries, 18);
            assert_eq!(stats.bytes, 4 << 20);
            assert_eq!(stats.misses, 18);
            assert_eq!(stats.uploaded_bytes, 18 * 512 * 512);
            assert_eq!(stats.overflow_bytes, 2 * 512 * 512);
            assert_eq!(stats.overflow_textures, 2);
        }
    }
}
