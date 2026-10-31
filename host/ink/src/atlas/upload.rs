//! The CPU page preserves occupied pixels between disjoint dirty rectangles.
use super::*;
use crate::device::TextureUpload;

pub(super) struct PagePixels {
    size: [u32; 2],
    channels: usize,
    bytes: Vec<u8>,
    dirty: Option<[u32; 4]>,
}
impl PagePixels {
    pub fn new(size: [u32; 2], channels: usize) -> Self {
        Self {
            size,
            channels,
            bytes: vec![0; size[0] as usize * size[1] as usize * channels],
            dirty: None,
        }
    }
    pub fn dirty(&self) -> bool {
        self.dirty.is_some()
    }
    pub fn write(&mut self, origin: [u32; 2], size: [u32; 2], bytes: &[u8]) {
        let stride = self.size[0] as usize * self.channels;
        let row = size[0] as usize * self.channels;
        for (y, bytes) in bytes.chunks_exact(row).enumerate() {
            let at = (origin[1] as usize + y) * stride + origin[0] as usize * self.channels;
            self.bytes[at..at + row].copy_from_slice(bytes);
        }
        let rect = [
            origin[0],
            origin[1],
            origin[0] + size[0],
            origin[1] + size[1],
        ];
        self.dirty = Some(match self.dirty {
            None => rect,
            Some(old) => [
                old[0].min(rect[0]),
                old[1].min(rect[1]),
                old[2].max(rect[2]),
                old[3].max(rect[3]),
            ],
        });
    }
    fn upload(&self, texture: TextureId) -> Option<TextureUpload> {
        let [x, y, right, bottom] = self.dirty?;
        let row = (right - x) as usize * self.channels;
        let stride = self.size[0] as usize * self.channels;
        let mut data = Vec::with_capacity(row * (bottom - y) as usize);
        for y in y..bottom {
            let at = y as usize * stride + x as usize * self.channels;
            data.extend_from_slice(&self.bytes[at..at + row]);
        }
        Some(TextureUpload {
            texture,
            origin: [x, y],
            size: [right - x, bottom - y],
            data,
        })
    }
}
impl Atlas {
    pub(super) fn batched(mut self) -> Self {
        self.pixels = Some(HashMap::new());
        self
    }

    /// Flush before any pass that samples these atlases. Subsequent passes can
    /// discover more masks; their writes must precede their own draw too.
    pub fn flush(atlases: &mut [&mut Self], device: &mut impl Device) -> Result<()> {
        let mut uploads = Vec::new();
        for atlas in atlases.iter() {
            if let Some(pages) = &atlas.pixels {
                uploads.extend(pages.iter().filter_map(|(&id, p)| p.upload(id)));
            }
        }
        if uploads.is_empty() {
            return Ok(());
        }
        uploads.sort_by_key(|u| u.texture.0);
        #[cfg(target_os = "macos")]
        let _trace = crate::device::apple::Trace::new(c"glyph atlas upload");
        device.upload_textures(&uploads)?;
        crate::counters::add(crate::counters::Counter::GlyphUploadBatches, 1);
        crate::counters::add(
            crate::counters::Counter::GlyphUploadCopies,
            uploads.len() as u64,
        );
        for atlas in atlases {
            for u in &uploads {
                if let Some(pixels) = atlas.pixels.as_mut().and_then(|p| p.get_mut(&u.texture)) {
                    pixels.dirty = None;
                    atlas.stats.uploaded_bytes += u.data.len() as u64;
                    crate::counters::add(
                        crate::counters::Counter::GlyphUploadBytes,
                        u.data.len() as u64,
                    );
                    if let Some(page) = atlas.pages.iter_mut().find(|p| p.id == u.texture) {
                        page.upload = device.last_use(u.texture)?;
                    } else {
                        // Overflow textures cannot be appended to.
                        atlas.pixels.as_mut().unwrap().remove(&u.texture);
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn check(device: &mut impl Device) {
        for format in [Format::Alpha, Format::Rgba] {
            let mut serial = Atlas::new([16, 16], format, 1, 1);
            let mut batch = Atlas::new([16, 16], format, 1, 1).batched();
            let channels = format.channels() as usize;
            let mut entries = Vec::new();
            for frame in 0..3 {
                serial.begin(device).unwrap();
                batch.begin(device).unwrap();
                let count = match frame {
                    0 => 3,
                    1 => 1,
                    _ => 0,
                };
                for n in 0..count {
                    let key = vec![frame, n];
                    let data: Vec<u8> = (0..4 * 4 * channels)
                        .map(|i| (i + 19 * n as usize) as u8)
                        .collect();
                    let a = serial.insert(device, key.clone(), [4, 4], &data).unwrap();
                    let b = batch.insert(device, key, [4, 4], &data).unwrap();
                    assert_eq!((a.uv, a.row), (b.uv, b.row));
                    assert_eq!(batch.stats().uploaded_bytes, 0);
                    entries.push((a, b));
                }
                let pending = batch
                    .pixels
                    .as_ref()
                    .unwrap()
                    .values()
                    .filter(|p| p.dirty())
                    .count();
                assert_eq!(pending, usize::from(count > 0));
                Atlas::flush(&mut [&mut batch], device).unwrap();
                let bytes = batch.stats().uploaded_bytes;
                assert_eq!(
                    bytes,
                    match frame {
                        0 => 12 * 12 * channels as u64,
                        1 => 6 * 6 * channels as u64,
                        _ => 0,
                    }
                );
                Atlas::flush(&mut [&mut batch], device).unwrap();
                assert_eq!(batch.stats().uploaded_bytes, bytes);
                for &(a, b) in &entries {
                    let left = device.readback(a.texture).unwrap();
                    let right = device.readback(b.texture).unwrap();
                    let x = (a.uv[0] * 16.) as usize - 1;
                    let y = (a.uv[1] * 16.) as usize - 1;
                    for y in y..y + 6 {
                        let at = (y * 16 + x) * channels;
                        assert_eq!(&left[at..at + 6 * channels], &right[at..at + 6 * channels]);
                    }
                }
            }
            batch.pin(entries[0].1.texture);
            batch.begin(device).unwrap();
            let spill = batch
                .insert(device, vec![9], [14, 14], &vec![137; 14 * 14 * channels])
                .unwrap();
            assert_ne!(spill.texture, entries[0].1.texture);
            batch.begin(device).unwrap();
            assert!(batch.contains(&[9]));
            Atlas::flush(&mut [&mut batch], device).unwrap();
            let bytes = device.readback(spill.texture).unwrap();
            assert_eq!(
                &bytes[(16 + 1) * channels..(16 + 15) * channels],
                &vec![137; 14 * channels]
            );
            assert!(!batch.pixels.as_ref().unwrap().contains_key(&spill.texture));
        }
    }

    #[test]
    fn wgpu_batches_identical_masks_and_padding_then_only_the_new_rectangle() {
        check(&mut crate::Wgpu::new().unwrap());
    }
    #[cfg(exact_ink_metal)]
    #[test]
    fn metal_batches_identical_masks_and_padding_then_only_the_new_rectangle() {
        check(&mut crate::device::metal::Metal::new().unwrap());
    }
    #[cfg(windows)]
    #[test]
    fn d3d11_batches_identical_masks_and_padding_then_only_the_new_rectangle() {
        check(&mut crate::device::d3d11::D3d11::new().unwrap());
    }

    #[test]
    fn a_dirty_union_keeps_earlier_pixels_between_new_masks() {
        let mut page = PagePixels::new([8, 8], 1);
        page.write([2, 2], [4, 4], &[79; 16]);
        page.dirty = None;
        page.write([1, 1], [1, 1], &[11]);
        page.write([6, 6], [1, 1], &[37]);
        let upload = page.upload(TextureId(1)).unwrap();
        assert_eq!((upload.origin, upload.size), ([1, 1], [6, 6]));
        assert_eq!((upload.data[0], upload.data[35]), (11, 37));
        for y in 1..5 {
            for x in 1..5 {
                assert_eq!(upload.data[y * 6 + x], 79);
            }
        }
    }
}
