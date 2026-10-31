//! One mapped staging allocation, with a copy for each dirty glyph page.
use super::*;
use crate::device::TextureUpload;

impl Wgpu {
    pub(super) fn upload_atlas(&mut self, uploads: &[TextureUpload]) -> Result<Submission> {
        if uploads.is_empty() {
            return Ok(Submission(0));
        }
        let mut layout = Vec::with_capacity(uploads.len());
        let mut length = 0u64;
        for u in uploads {
            let desc = self.texture(u.texture)?.desc;
            check_region(desc, u.origin, u.size)?;
            let row = u.size[0] * desc.format.channels();
            if u.data.len() as u64 != u64::from(row) * u64::from(u.size[1]) {
                return Err(Error::Invalid("atlas upload length".into()));
            }
            let stride = row.div_ceil(256) * 256;
            layout.push((length, row, stride));
            length += u64::from(stride) * u64::from(u.size[1]);
        }
        let buffer = self.device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("ink glyph atlas staging"),
            size: length,
            usage: ::wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: true,
        });
        {
            let mut mapped = buffer
                .slice(..)
                .get_mapped_range_mut()
                .map_err(|e| Error::Device(format!("atlas staging map: {e}")))?;
            for (u, &(offset, row, stride)) in uploads.iter().zip(&layout) {
                for (y, bytes) in u.data.chunks_exact(row as usize).enumerate() {
                    let at = offset as usize + y * stride as usize;
                    mapped.slice(at..at + bytes.len()).copy_from_slice(bytes);
                }
            }
        }
        buffer.unmap();
        let mut encoder = self.device.create_command_encoder(&Default::default());
        for (u, &(offset, _, stride)) in uploads.iter().zip(&layout) {
            encoder.copy_buffer_to_texture(
                ::wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: ::wgpu::TexelCopyBufferLayout {
                        offset,
                        bytes_per_row: Some(stride),
                        rows_per_image: None,
                    },
                },
                ::wgpu::TexelCopyTextureInfo {
                    texture: &self.texture(u.texture)?.raw,
                    mip_level: 0,
                    origin: ::wgpu::Origin3d {
                        x: u.origin[0],
                        y: u.origin[1],
                        z: 0,
                    },
                    aspect: ::wgpu::TextureAspect::All,
                },
                extent(u.size),
            );
        }
        let s = self.submit(encoder.finish())?;
        // The submission owns the staging buffer until its copies complete.
        for u in uploads {
            self.textures.get_mut(&u.texture).unwrap().last = s;
        }
        Ok(s)
    }
}
