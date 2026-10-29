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
    DeviceLost(String),
    Invalid(String),
    Unsupported(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAdapter(e) => write!(f, "no adapter: {e}"),
            Self::Device(e) => write!(f, "device: {e}"),
            Self::DeviceLost(e) => write!(f, "device lost: {e}"),
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
    fn max_texture_dimension(&self) -> u32 {
        16384
    }
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
        use windows::Win32::Graphics::Dxgi::{
            DXGI_ERROR_DEVICE_HUNG, DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET,
            DXGI_ERROR_DRIVER_INTERNAL_ERROR,
        };
        if [
            DXGI_ERROR_DEVICE_HUNG,
            DXGI_ERROR_DEVICE_REMOVED,
            DXGI_ERROR_DEVICE_RESET,
            DXGI_ERROR_DRIVER_INTERNAL_ERROR,
        ]
        .contains(&error.code())
        {
            Self::DeviceLost(error.to_string())
        } else {
            Self::Device(error.to_string())
        }
    }
}

/// Process age after the calling thread's outstanding updates reach a DWM sync.
/// A synchronization proxy, not a swap-chain frame timestamp.
#[cfg(windows)]
pub fn compositor_sync_process_ms() -> Result<f64> {
    unsafe { windows::Win32::Graphics::Dwm::DwmFlush()? };
    process_ms()
}

/// Process age without waiting for presentation, for startup phase measurements.
#[cfg(windows)]
pub fn process_ms() -> Result<f64> {
    use windows::Win32::{
        Foundation::FILETIME,
        System::{
            SystemInformation::GetSystemTimePreciseAsFileTime,
            Threading::{GetCurrentProcess, GetProcessTimes},
        },
    };
    unsafe {
        let now = GetSystemTimePreciseAsFileTime();
        let (mut created, mut exited, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        GetProcessTimes(
            GetCurrentProcess(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )?;
        let ticks = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
        Ok((ticks(now) - ticks(created)) as f64 / 10_000.)
    }
}

/// Windows process CPU and working set, in the same units as the S0 Mac probe.
#[cfg(windows)]
pub fn process_usage() -> std::io::Result<[f64; 5]> {
    use windows::Win32::{
        Foundation::FILETIME,
        System::{
            ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            Threading::{GetCurrentProcess, GetProcessTimes},
        },
    };
    unsafe {
        let process = GetCurrentProcess();
        let (mut created, mut exited, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user)
            .map_err(std::io::Error::other)?;
        let mut memory = PROCESS_MEMORY_COUNTERS {
            cb: size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        if !K32GetProcessMemoryInfo(
            process,
            &mut memory,
            size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
        .as_bool()
        {
            return Err(std::io::Error::last_os_error());
        }
        let ticks = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
        Ok([
            (ticks(kernel) + ticks(user)) as f64 / 10_000.,
            memory.WorkingSetSize as f64,
            memory.PeakWorkingSetSize as f64,
            memory.PagefileUsage as f64,
            memory.PeakPagefileUsage as f64,
        ])
    }
}

pub(crate) fn check_region(desc: TextureDesc, origin: [u32; 2], size: [u32; 2]) -> Result<()> {
    if size.contains(&0)
        || origin[0]
            .checked_add(size[0])
            .is_none_or(|x| x > desc.width)
        || origin[1]
            .checked_add(size[1])
            .is_none_or(|y| y > desc.height)
    {
        Err(Error::Invalid("texture region bounds".into()))
    } else {
        Ok(())
    }
}
pub(crate) fn texture_ids(b: crate::instance::Bindings) -> [TextureId; 7] {
    [
        b.glyph,
        b.color,
        b.path,
        b.ramp,
        b.image,
        b.mask,
        b.destination,
    ]
}

/// Device-loss retries are scheduled independently of frame rate.
#[derive(Default)]
pub struct Retry {
    due: Option<std::time::Instant>,
    failures: u32,
}
impl Retry {
    pub fn ready(&self, now: std::time::Instant) -> bool {
        self.due.is_none_or(|due| now >= due)
    }
    pub fn deadline(&self) -> Option<std::time::Instant> {
        self.due
    }
    pub fn failed(&mut self, now: std::time::Instant) {
        let ms = (250u64 << self.failures.min(7)).min(30_000);
        self.failures = self.failures.saturating_add(1);
        self.due = Some(now + std::time::Duration::from_millis(ms));
    }
}
