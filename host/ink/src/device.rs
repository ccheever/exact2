//! Resource and submission boundary. Presentation joins this in `present.rs`.
//! @ref LLP 1149 D5, D11: retirement follows GPU completion, never frame age.

use crate::instance::{Frame, TextureId};
use std::fmt;

#[cfg(windows)]
pub mod d3d11;

#[cfg(exact_ink_metal)]
pub mod metal;
mod selected;
pub mod wgpu;
pub use selected::Selected;

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
/// one queue. Every mutation waits for the resource's last use or chooses fresh
/// storage. Submissions include uploads; a larger completed value covers all
/// earlier submissions. Retire keeps storage alive until its last use completes.
/// Readback is synchronous on native devices; browser callers must
/// use the device's asynchronous readback instead of blocking its event loop.
pub trait Device {
    type Buffer;
    fn info(&self) -> &Info;
    fn create_buffer(&self, bytes: u64) -> Result<Self::Buffer>;
    fn upload_buffer(
        &mut self,
        buffer: &Self::Buffer,
        offset: u64,
        data: &[u8],
    ) -> Result<Submission>;
    fn create_texture(&mut self, desc: TextureDesc) -> Result<TextureId>;
    fn upload_texture(
        &mut self,
        texture: TextureId,
        origin: [u32; 2],
        size: [u32; 2],
        data: &[u8],
    ) -> Result<Submission>;
    /// Latest upload, read or write using this texture.
    fn last_use(&self, texture: TextureId) -> Result<Submission>;
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

#[cfg(test)]
mod tests;

/// Process counters for the S0 harness without spawning a child in the worker.
/// CPU milliseconds, resident bytes, peak resident bytes, footprint bytes and
/// peak footprint bytes. @ref LLP 1149 D15.
#[cfg(target_os = "macos")]
pub fn process_usage() -> std::io::Result<[f64; 5]> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage_info_v4>::zeroed();
    let mut peak = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: both outputs have the sizes their selectors require. Neither
    // pointer escapes the calls; values are read only on success.
    unsafe {
        if libc::proc_pid_rusage(
            std::process::id() as i32,
            libc::RUSAGE_INFO_V4,
            usage.as_mut_ptr().cast(),
        ) != 0
            || libc::getrusage(libc::RUSAGE_SELF, peak.as_mut_ptr()) != 0
        {
            return Err(std::io::Error::last_os_error());
        }
        let usage = usage.assume_init();
        let peak = peak.assume_init();
        Ok([
            (peak.ru_utime.tv_sec + peak.ru_stime.tv_sec) as f64 * 1000.
                + (peak.ru_utime.tv_usec + peak.ru_stime.tv_usec) as f64 / 1000.,
            usage.ri_resident_size as f64,
            peak.ru_maxrss as f64,
            usage.ri_phys_footprint as f64,
            usage.ri_lifetime_max_phys_footprint as f64,
        ])
    }
}

#[cfg(windows)]
impl From<windows::core::Error> for Error {
    fn from(error: windows::core::Error) -> Self {
        Self::Device(error.to_string())
    }
}
