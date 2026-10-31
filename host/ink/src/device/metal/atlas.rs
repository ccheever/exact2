//! Glyph pages share one staging buffer and command. @ref LLP 1149.001 M10.
use super::*;
use crate::device::TextureUpload;

impl Metal {
    pub(super) fn upload_atlas(&mut self, uploads: &[TextureUpload]) -> Result<Submission> {
        if uploads.is_empty() {
            return Ok(Submission(0));
        }
        autoreleasepool(|_| {
            self.completed()?;
            let mut layout = Vec::with_capacity(uploads.len());
            let mut textures = Vec::with_capacity(uploads.len());
            let mut length = 0;
            for u in uploads {
                let texture = self.texture(u.texture)?;
                let desc = texture.desc;
                textures.push(texture.raw.clone());
                check_region(desc, u.origin, u.size)?;
                let row = u.size[0] as usize * desc.format.channels() as usize;
                if u.data.len() != row * u.size[1] as usize {
                    return Err(Error::Invalid("atlas upload length".into()));
                }
                let stride = row.div_ceil(256) * 256;
                layout.push((length, row, stride));
                length += stride * u.size[1] as usize;
            }
            #[cfg(target_os = "macos")]
            let staging = Trace::new(c"atlas staging");
            let slot = self.next_atlas_buffer;
            self.next_atlas_buffer = (slot + 1) % self.atlas_buffers.len();
            // Three lazily allocated slots cover the previous, submitted and
            // encoding frames. A cold fill sizes one slot to all dirty pages,
            // independently of image uploads. A busy slot retires without a wait.
            let buffer = match self.atlas_buffers[slot].take() {
                Some(b)
                    if b.last.get() <= self.done
                        && b.raw.length() >= length
                        && b.raw.length() <= length.next_power_of_two().max(65536) =>
                {
                    b
                }
                old => {
                    if let Some(b) = old {
                        self.retired_buffers.push(b);
                    }
                    self.create_buffer(length.max(65536) as u64)?
                }
            };
            for (u, &(offset, row, stride)) in uploads.iter().zip(&layout) {
                for (y, bytes) in u.data.chunks_exact(row).enumerate() {
                    // SAFETY: the slot completed or is fresh; layout bounds
                    // every row, and the command retains storage through its copy.
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            bytes.as_ptr(),
                            buffer
                                .raw
                                .contents()
                                .as_ptr()
                                .cast::<u8>()
                                .add(offset + y * stride),
                            row,
                        );
                    }
                }
            }
            #[cfg(target_os = "macos")]
            {
                super::super::apple::trace_mark(c"atlas staging bytes", || length as f64);
                super::super::apple::trace_mark(c"atlas copies", || uploads.len() as f64);
                drop(staging);
            }
            let command = self.commands()?;
            #[cfg(target_os = "macos")]
            let encoding = Trace::new(c"atlas copy encode");
            let encoder = command
                .blitCommandEncoder()
                .ok_or_else(|| Error::Device("atlas blit encoder".into()))?;
            for ((u, &(offset, _, stride)), texture) in uploads.iter().zip(&layout).zip(&textures) {
                // SAFETY: checked regions and aligned offsets fit the staging
                // allocation; Metal retains both resources through completion.
                unsafe {
                    encoder.copyFromBuffer_sourceOffset_sourceBytesPerRow_sourceBytesPerImage_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin_options(
                        &buffer.raw, offset, stride, 0,
                        MTLSize { width: u.size[0] as usize, height: u.size[1] as usize, depth: 1 },
                        texture, 0, 0, point(u.origin), MTLBlitOption::None);
                }
            }
            encoder.endEncoding();
            #[cfg(target_os = "macos")]
            drop(encoding);
            let s = self.submit(Some(command));
            #[cfg(target_os = "macos")]
            super::super::apple::trace_mark(c"atlas submission", || s.0 as f64);
            buffer.last.set(s);
            self.atlas_buffers[slot] = Some(buffer);
            for u in uploads {
                self.textures.get_mut(&u.texture).unwrap().last = s;
            }
            if let Some(error) = &self.error {
                return Err(error.clone());
            }
            Ok(s)
        })
    }
}
