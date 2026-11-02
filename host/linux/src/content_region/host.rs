//! Host-facing completion turn. No worker waits or nested after-commit loops.
use super::*;
impl<D: DataSource> Host<D> {
    /// Notify the native scale before draining a completion. A changed DPR
    /// refuses this trial registration while its accepted pixels remain owned.
    pub fn content_region_paint_scale(&mut self, scale: f32) -> Option<String> {
        self.content_region.as_mut()?.configure_scale(scale).err()
    }
    /// Palette revision is part of native publication, independent of unrelated
    /// typing commits. An old accepted palette remains coherent while pending.
    pub fn content_region_appearance(&mut self, dark: bool) -> Option<String> {
        let region = self.content_region.as_mut()?;
        match region.appearance(dark) {
            Ok(true) => self.layout().err(),
            Ok(false) => None,
            Err(e) => Some(e),
        }
    }
    /// Notify only after a successful native frame. Starts metadata capture and
    /// background font initialization after the real placeholder was painted.
    pub fn content_region_painted(&mut self, painter: &crate::paint::Painter) -> Option<String> {
        self.content_region.as_mut()?.first_painted(painter).err()
    }
    /// File-descriptor readiness makes progress without another input event.
    #[cfg(unix)]
    pub fn content_region_fd(&self) -> Option<std::os::unix::io::RawFd> {
        self.content_region.as_ref()?.completion_fd()
    }
    /// Consume at most one result and run one UI-owned offer-discovery pass.
    /// `true` requests a frame; the next pending offer wakes by its own result.
    pub fn poll_content_region(&mut self) -> Result<bool, String> {
        let Some(region) = &mut self.content_region else {
            return Ok(false);
        };
        if !region.poll(self.runner.kernel_mut())? {
            return Ok(false);
        }
        self.layout()?;
        self.present();
        Ok(true)
    }
}
