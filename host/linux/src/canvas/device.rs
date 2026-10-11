//! The device's settings, from the reader (LLP 1115.001 D3.1): one call
//! carries them all, before the host boots and again, whole, on every
//! change. What the reader said last is kept for the process, so a host
//! that boots after the call starts with it: the settings are applied
//! before its first frame is recorded, and nothing is ever painted without
//! them.

use super::CanvasHost;
use crate::presenter::Device;
use exact_runner::DataSource;

/// What the reader last said, for a host that boots after it said so.
static DEVICE: std::sync::Mutex<Option<Device>> = std::sync::Mutex::new(None);

/// Keep `device` for the host that boots next (the reader's `configure`,
/// from any thread; a running host is told on its own, by
/// [`CanvasHost::configure`]).
pub fn configure(device: &Device) {
    *DEVICE.lock().unwrap_or_else(|e| e.into_inner()) = Some(device.clone());
}

/// What the reader last said, if it has said anything.
pub(super) fn configured() -> Option<Device> {
    DEVICE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

impl<D: DataSource + Default> CanvasHost<D> {
    /// The device's settings changed, or are told for the first time: the
    /// presenter takes them through its doors
    /// ([`crate::presenter::Presenter::set_device`]) and the next frame is
    /// a paint.
    pub fn configure(&mut self, device: &Device) {
        if let Some(e) = self.p.set_device(device) {
            eprintln!("exact: device: {e}");
        }
        self.force = true;
    }
}
