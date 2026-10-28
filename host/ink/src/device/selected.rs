use super::*;

/// The boot choice; both layers consume the same core's frames and resources.
pub enum Selected {
    Wgpu(Box<wgpu::Wgpu>),
    #[cfg(windows)]
    D3d11(Box<d3d11::D3d11>),
    #[cfg(exact_ink_metal)]
    Metal(Box<metal::Metal>),
}

pub enum Buffer {
    Wgpu(wgpu::Buffer),
    #[cfg(windows)]
    D3d11(d3d11::Buffer),
    #[cfg(exact_ink_metal)]
    Metal(metal::Buffer),
}

impl Selected {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_env() -> Result<Self> {
        match std::env::var("EXACT_INK_DEVICE").as_deref() {
            Err(std::env::VarError::NotPresent) => {
                #[cfg(windows)]
                {
                    d3d11::D3d11::new().map(|d| Self::D3d11(Box::new(d)))
                }
                #[cfg(not(windows))]
                {
                    wgpu::Wgpu::new().map(Into::into)
                }
            }
            Ok("wgpu") => wgpu::Wgpu::new().map(Into::into),
            #[cfg(windows)]
            Ok("d3d11") => d3d11::D3d11::open(std::env::var_os("EXACT_INK_WARP").is_some())
                .map(|d| Self::D3d11(Box::new(d))),
            Ok("metal") => {
                #[cfg(exact_ink_metal)]
                {
                    metal::Metal::new().map(|d| Self::Metal(Box::new(d)))
                }
                #[cfg(not(exact_ink_metal))]
                {
                    Err(Error::Unsupported(
                        "direct Metal requires an Apple target built with the Metal toolchain",
                    ))
                }
            }
            _ => Err(Error::Invalid(
                "EXACT_INK_DEVICE must be wgpu, metal or d3d11".into(),
            )),
        }
    }
}

impl From<wgpu::Wgpu> for Selected {
    fn from(device: wgpu::Wgpu) -> Self {
        Self::Wgpu(Box::new(device))
    }
}

impl Device for Selected {
    type Buffer = Buffer;
    fn create_buffer(&self, bytes: u64) -> Result<Buffer> {
        match self {
            Self::Wgpu(d) => d.create_buffer(bytes).map(Buffer::Wgpu),
            #[cfg(exact_ink_metal)]
            Self::Metal(d) => d.create_buffer(bytes).map(Buffer::Metal),
            #[cfg(windows)]
            Self::D3d11(d) => d.create_buffer(bytes).map(Buffer::D3d11),
        }
    }
    fn upload_buffer(&mut self, buffer: &Buffer, offset: u64, data: &[u8]) -> Result<Submission> {
        match (self, buffer) {
            (Self::Wgpu(d), Buffer::Wgpu(b)) => d.upload_buffer(b, offset, data),
            #[cfg(exact_ink_metal)]
            (Self::Metal(d), Buffer::Metal(b)) => d.upload_buffer(b, offset, data),
            #[cfg(windows)]
            (Self::D3d11(d), Buffer::D3d11(b)) => d.upload_buffer(b, offset, data),
            #[cfg(any(exact_ink_metal, windows))]
            _ => Err(Error::Invalid(
                "buffer belongs to another device layer".into(),
            )),
        }
    }
    fn info(&self) -> &Info {
        match self {
            Self::Wgpu(d) => d.info(),
            #[cfg(exact_ink_metal)]
            Self::Metal(d) => d.info(),
            #[cfg(windows)]
            Self::D3d11(d) => d.info(),
        }
    }
    fn create_texture(&mut self, desc: TextureDesc) -> Result<TextureId> {
        match self {
            Self::Wgpu(d) => d.create_texture(desc),
            #[cfg(exact_ink_metal)]
            Self::Metal(d) => d.create_texture(desc),
            #[cfg(windows)]
            Self::D3d11(d) => d.create_texture(desc),
        }
    }
    fn upload_texture(
        &mut self,
        texture: TextureId,
        origin: [u32; 2],
        size: [u32; 2],
        data: &[u8],
    ) -> Result<Submission> {
        match self {
            Self::Wgpu(d) => d.upload_texture(texture, origin, size, data),
            #[cfg(exact_ink_metal)]
            Self::Metal(d) => d.upload_texture(texture, origin, size, data),
            #[cfg(windows)]
            Self::D3d11(d) => d.upload_texture(texture, origin, size, data),
        }
    }
    fn last_use(&self, texture: TextureId) -> Result<Submission> {
        match self {
            Self::Wgpu(d) => d.last_use(texture),
            #[cfg(exact_ink_metal)]
            Self::Metal(d) => d.last_use(texture),
            #[cfg(windows)]
            Self::D3d11(d) => d.last_use(texture),
        }
    }
    fn generate_mips(&mut self, texture: TextureId) -> Result<Submission> {
        match self {
            Self::Wgpu(d) => d.generate_mips(texture),
            #[cfg(exact_ink_metal)]
            Self::Metal(d) => d.generate_mips(texture),
            #[cfg(windows)]
            Self::D3d11(d) => d.generate_mips(texture),
        }
    }
    fn resize_target(&mut self, target: TextureId, width: u32, height: u32) -> Result<TextureId> {
        match self {
            Self::Wgpu(d) => d.resize_target(target, width, height),
            #[cfg(exact_ink_metal)]
            Self::Metal(d) => d.resize_target(target, width, height),
            #[cfg(windows)]
            Self::D3d11(d) => d.resize_target(target, width, height),
        }
    }
    fn copy_region(
        &mut self,
        source: TextureId,
        origin: [u32; 2],
        target: TextureId,
        dst: [u32; 2],
        size: [u32; 2],
    ) -> Result<Submission> {
        match self {
            Self::Wgpu(d) => d.copy_region(source, origin, target, dst, size),
            #[cfg(exact_ink_metal)]
            Self::Metal(d) => d.copy_region(source, origin, target, dst, size),
            #[cfg(windows)]
            Self::D3d11(d) => d.copy_region(source, origin, target, dst, size),
        }
    }
    fn render(
        &mut self,
        frame: &Frame,
        target: TextureId,
        clear: Option<[f64; 4]>,
    ) -> Result<Submission> {
        match self {
            Self::Wgpu(d) => d.render(frame, target, clear),
            #[cfg(exact_ink_metal)]
            Self::Metal(d) => d.render(frame, target, clear),
            #[cfg(windows)]
            Self::D3d11(d) => d.render(frame, target, clear),
        }
    }
    fn readback(&mut self, texture: TextureId) -> Result<Vec<u8>> {
        match self {
            Self::Wgpu(d) => d.readback(texture),
            #[cfg(exact_ink_metal)]
            Self::Metal(d) => d.readback(texture),
            #[cfg(windows)]
            Self::D3d11(d) => d.readback(texture),
        }
    }
    fn wait(&mut self, submission: Submission) -> Result<()> {
        match self {
            Self::Wgpu(d) => d.wait(submission),
            #[cfg(exact_ink_metal)]
            Self::Metal(d) => d.wait(submission),
            #[cfg(windows)]
            Self::D3d11(d) => d.wait(submission),
        }
    }
    fn completed(&mut self) -> Result<Submission> {
        match self {
            Self::Wgpu(d) => d.completed(),
            #[cfg(exact_ink_metal)]
            Self::Metal(d) => d.completed(),
            #[cfg(windows)]
            Self::D3d11(d) => d.completed(),
        }
    }
    fn retire_texture(&mut self, texture: TextureId) -> Result<()> {
        match self {
            Self::Wgpu(d) => d.retire_texture(texture),
            #[cfg(exact_ink_metal)]
            Self::Metal(d) => d.retire_texture(texture),
            #[cfg(windows)]
            Self::D3d11(d) => d.retire_texture(texture),
        }
    }
    fn trim(&mut self) -> Result<()> {
        match self {
            Self::Wgpu(d) => d.trim(),
            #[cfg(exact_ink_metal)]
            Self::Metal(d) => d.trim(),
            #[cfg(windows)]
            Self::D3d11(d) => d.trim(),
        }
    }
}
