//! Preserve the carrier API when the build has no Metal compiler.
use super::*;

pub struct Startup(std::convert::Infallible);
impl Startup {
    pub fn waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        match self.0 {}
    }
    pub fn start(_: Arc<dyn Fn() + Send + Sync>) -> Result<Self> {
        Err(Error::Unsupported(
            "direct Metal requires the Metal toolchain",
        ))
    }
    pub fn with_archive(_: std::path::PathBuf, wake: Arc<dyn Fn() + Send + Sync>) -> Result<Self> {
        Self::start(wake)
    }
    pub fn attach(&mut self, _: Layer) {
        match self.0 {}
    }
    #[doc(hidden)]
    #[cfg(debug_assertions)]
    pub fn with_archive_gated(
        path: std::path::PathBuf,
        wake: Arc<dyn Fn() + Send + Sync>,
        _: std::sync::mpsc::Receiver<()>,
    ) -> Result<Self> {
        Self::with_archive(path, wake)
    }
    pub fn poll(&mut self) -> Result<Option<super::super::Selected>> {
        match self.0 {}
    }
    pub fn ready(&self) -> bool {
        match self.0 {}
    }
    pub fn present_cpu(&mut self, _: Arc<tiny_skia::Pixmap>) -> Result<bool> {
        match self.0 {}
    }
}
