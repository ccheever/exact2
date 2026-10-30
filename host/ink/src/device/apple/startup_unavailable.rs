//! Preserve the carrier API when the build has no Metal compiler.
use super::*;

pub struct Startup(std::convert::Infallible);
impl Startup {
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
    pub fn poll(&mut self, _: bool) -> Result<Option<super::super::Selected>> {
        match self.0 {}
    }
    pub fn pending(&self) -> bool {
        match self.0 {}
    }
    pub fn present_cpu(&mut self, _: Arc<tiny_skia::Pixmap>) -> Result<()> {
        match self.0 {}
    }
}
