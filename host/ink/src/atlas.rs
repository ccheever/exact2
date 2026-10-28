//! Shelf pages stay pinned until the frame ends. @ref LLP 1149 D2.3, D8.
use crate::device::{Device, Format, Result, TextureDesc};
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
    stats: Stats,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub entries: usize,
    pub bytes: u64,
    pub misses: u64,
    pub uploaded_bytes: u64,
    pub overflow_bytes: u64,
    pub overflow_textures: usize,
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

    fn new(size: [u32; 2], format: Format, padding: u32, ceiling: usize) -> Self {
        Self {
            size,
            format,
            padding,
            ceiling,
            frame: 0,
            pages: Vec::new(),
            entries: HashMap::new(),
            overflow: Vec::new(),
            stats: Stats::default(),
        }
    }

    pub fn begin(&mut self, device: &mut impl Device) -> Result<()> {
        self.frame += 1;
        self.stats = Stats::default();
        for id in self.overflow.drain(..) {
            self.entries.retain(|_, e| e.texture != id);
            device.retire_texture(id)?;
        }
        Ok(())
    }

    pub fn get(&mut self, key: &[u32]) -> Option<Entry> {
        let Some(&entry) = self.entries.get(key) else {
            self.stats.misses += 1;
            return None;
        };
        if let Some(page) = self.pages.iter_mut().find(|p| p.id == entry.texture) {
            page.used = self.frame;
        }
        Some(entry)
    }

    pub fn insert(
        &mut self,
        device: &mut impl Device,
        key: Vec<u32>,
        size: [u32; 2],
        pixels: &[u8],
    ) -> Result<Entry> {
        let padded = size.map(|s| s + 2 * self.padding);
        let mut slot = None;
        if padded[0] <= self.size[0] && padded[1] <= self.size[1] {
            for page in &mut self.pages {
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
                    .filter(|(_, p)| p.used != self.frame)
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
            self.stats.overflow_bytes +=
                u64::from(padded[0]) * u64::from(padded[1]) * u64::from(self.format.channels());
            (id, [0, 0], padded)
        };
        let channels = self.format.channels() as usize;
        let mut upload = vec![0; padded[0] as usize * padded[1] as usize * channels];
        for (y, row) in pixels.chunks_exact(size[0] as usize * channels).enumerate() {
            let start = ((y + self.padding as usize) * padded[0] as usize + self.padding as usize)
                * channels;
            upload[start..start + row.len()].copy_from_slice(row);
        }
        device.upload_texture(texture, origin, padded, &upload)?;
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
