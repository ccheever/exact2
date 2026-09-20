//! Host-neutral output targets; window ownership stays with the native binary.
use crate::{
    Renderer,
    gpu::{Result, texture},
};
impl Renderer {
    pub fn color_view(&self) -> wgpu::TextureView {
        self.color.create_view(&Default::default())
    }
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        if width == 0 || height == 0 || width > 8192 || height > 8192 {
            return Err("size must be 1..8192".into());
        }
        self.static_bytes -= u64::from(self.width) * u64::from(self.height) * 36;
        self.width = width;
        self.height = height;
        self.color = texture(
            &self.device,
            width,
            height,
            1,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        self.msaa = texture(
            &self.device,
            width,
            height,
            4,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
        .create_view(&Default::default());
        self.depth = texture(
            &self.device,
            width,
            height,
            4,
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
        .create_view(&Default::default());
        self.static_bytes += u64::from(width) * u64::from(height) * 36;
        Ok(())
    }
}
