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

impl Renderer {
    /// Replace a placeholder with a validated immutable page and rebind both passes.
    pub fn upload_page(
        &mut self,
        reader: &clod_format::Reader<'_>,
        id: usize,
        bytes: &[u8],
    ) -> Result<()> {
        reader.validate_page(id, bytes).map_err(|e| e.to_string())?;
        let page = self.pages.get_mut(id).ok_or("page index")?;
        self.static_bytes -= page.geometry.size();
        page.geometry = crate::gpu::buffer(
            &self.device,
            "streamed page",
            bytes,
            wgpu::BufferUsages::STORAGE,
        );
        self.static_bytes += page.geometry.size();
        let compute = self.compute.as_ref().ok_or("GPU selection not enabled")?;
        for (pass, lists) in compute
            .passes
            .iter()
            .zip([&mut self.lists, &mut self.shadow_lists])
        {
            lists[id] = Self::bind_list(
                &self.device,
                &self.page_layout,
                page,
                &self.clusters,
                &self.instances,
                pass.visible.clone(),
                &pass.draws,
            );
        }
        Ok(())
    }
}

impl Renderer {
    pub fn set_scene(
        &mut self,
        reader: &clod_format::Reader<'_>,
        scene: &crate::scene::Scene,
    ) -> Result<()> {
        scene.validate()?;
        self.static_bytes -= self.instances.size();
        self.instances = crate::gpu::buffer(
            &self.device,
            "instances",
            bytemuck::cast_slice(&scene.instances),
            wgpu::BufferUsages::STORAGE,
        );
        self.static_bytes += self.instances.size();
        self.instance_count = scene.instances.len() as u32;
        if self.mode == crate::Mode::Cluster {
            self.enable_gpu_selection(reader, None)?;
        } else {
            for lists in [&mut self.lists, &mut self.shadow_lists] {
                for (page, list) in self.pages.iter().zip(lists) {
                    *list = Self::make_list(
                        &self.device,
                        &self.page_layout,
                        page,
                        &self.clusters,
                        &self.instances,
                        &self.dummy_draws,
                        8,
                    );
                }
            }
        }
        Ok(())
    }
}
