//! Resource and submission boundary. Presentation joins this in `present.rs`.
//! @ref LLP 1149 D5, D11: retirement follows GPU completion, never frame age.

use crate::instance::{Frame, TextureId};
use std::fmt;

pub mod wgpu;

#[derive(Clone, Debug)]
pub struct Info {
    pub adapter: String,
    pub api: String,
    pub device_ms: f64,
    pub shaders_ms: f64,
    /// A cache file was supplied; the driver may still reject it.
    pub cached: bool,
}

#[derive(Debug)]
pub enum Error {
    NoAdapter(String),
    Device(String),
    Invalid(String),
    Unsupported(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAdapter(e) => write!(f, "no adapter: {e}"),
            Self::Device(e) => write!(f, "device: {e}"),
            Self::Invalid(e) => write!(f, "ink: {e}"),
            Self::Unsupported(e) => write!(f, "unsupported: {e}"),
        }
    }
}

impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Alpha,
    Rgba,
}

impl Format {
    pub fn channels(self) -> u32 {
        if self == Self::Alpha {
            1
        } else {
            4
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TextureDesc {
    pub width: u32,
    pub height: u32,
    pub format: Format,
    pub mipmapped: bool,
}

impl TextureDesc {
    pub fn rgba(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            format: Format::Rgba,
            mipmapped: false,
        }
    }
    pub fn levels(self) -> u32 {
        if self.mipmapped {
            self.width.max(self.height).ilog2() + 1
        } else {
            1
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Submission(pub u64);

/// The headless operations used by S0. Buffer uploads and draws are ordered on
/// one queue. Readback is synchronous on native devices; browser callers must
/// use the device's asynchronous readback instead of blocking its event loop.
pub trait Device {
    type Buffer;
    fn info(&self) -> &Info;
    fn create_buffer(&self, bytes: u64) -> Result<Self::Buffer>;
    fn upload_buffer(&self, buffer: &Self::Buffer, offset: u64, data: &[u8]) -> Result<()>;
    fn create_texture(&mut self, desc: TextureDesc) -> Result<TextureId>;
    fn upload_texture(
        &mut self,
        texture: TextureId,
        origin: [u32; 2],
        size: [u32; 2],
        data: &[u8],
    ) -> Result<()>;
    fn generate_mips(&mut self, texture: TextureId) -> Result<Submission>;
    fn create_target(&mut self, width: u32, height: u32) -> Result<TextureId> {
        self.create_texture(TextureDesc::rgba(width, height))
    }
    fn resize_target(&mut self, target: TextureId, width: u32, height: u32) -> Result<TextureId>;
    fn copy_region(
        &mut self,
        source: TextureId,
        origin: [u32; 2],
        target: TextureId,
        dst: [u32; 2],
        size: [u32; 2],
    ) -> Result<Submission>;
    fn render(
        &mut self,
        frame: &Frame,
        target: TextureId,
        clear: Option<[f64; 4]>,
    ) -> Result<Submission>;
    fn readback(&mut self, texture: TextureId) -> Result<Vec<u8>>;
    fn wait(&mut self, submission: Submission) -> Result<()>;
    fn completed(&mut self) -> Result<Submission>;
    fn retire_texture(&mut self, texture: TextureId) -> Result<()>;
    fn trim(&mut self) -> Result<()>;
}
