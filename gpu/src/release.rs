//! Letting go of a canvas's presentation target, on every path that does:
//! a window going, buffers replacing it, the canvas destroyed, the device
//! lost, the module dropped.
//!
//! A drawable goes before its surface. wgpu discards an unpresented drawable
//! through the surface it came from and panics when that surface is gone
//! (wgpu-core's `surface_texture_discard`: "Cannot get non-existent
//! resource"), and wgpu-hal's Vulkan swapchain refuses to be destroyed while
//! a drawable still holds its semaphores.
//!
//! Nothing here waits for the compositor. A canvas's acquisition thread may
//! be inside `vkAcquireNextImageKHR` for an image Android's compositor
//! releases only after the window's `surfaceDestroyed` returns, and that
//! callback waits for this ([`Module::detach`]). The thread is hung up on
//! ([`crate::acquire::Acquire::retire`]); it ends when its acquisition does,
//! and the module drops its surface here once it has ([`Module::reap`]).

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use crate::acquire::Retired;
use crate::{Instance, Module};

/// The web acquires on the browser's thread: nothing is ever retired.
#[cfg(target_arch = "wasm32")]
pub(crate) struct Retired;

#[cfg(target_arch = "wasm32")]
impl Retired {
    fn finished(&self) -> bool {
        true
    }
    fn join(self) {}
}

impl Instance {
    /// Drop the canvas's presentation target, drawables first; an
    /// acquisition thread still running is returned for [`Module::reap`].
    /// Its configuration stays, for a device found again (`Module::set_gpu`).
    pub(crate) fn release(&mut self) -> Option<Retired> {
        self.held = None;
        #[cfg(not(target_arch = "wasm32"))]
        let retired = {
            self.scratch = None;
            self.readback = None;
            self.acquire.retire()
        };
        #[cfg(target_arch = "wasm32")]
        let retired = None;
        self.presentation = None;
        #[cfg(target_os = "android")]
        {
            self.ring = None;
        }
        retired
    }
}

impl Module {
    /// Release canvas `id`'s presentation target ([`Instance::release`])
    /// and its configuration.
    pub(crate) fn release(&mut self, id: u32) {
        if let Some(inst) = self.instances.get_mut(&id) {
            self.retired.extend(inst.release());
            inst.config = None;
        }
        self.reap();
    }

    /// Drop the surfaces whose acquisition threads have ended.
    pub(crate) fn reap(&mut self) {
        let mut at = 0;
        while at < self.retired.len() {
            if self.retired[at].finished() {
                self.retired.swap_remove(at).join();
            } else {
                at += 1;
            }
        }
    }

    /// Wait for every retired acquisition thread to end: a window attached
    /// again may be the one a retired swapchain is still connected to, and
    /// Android refuses a second (`VK_ERROR_NATIVE_WINDOW_IN_USE_KHR`). The
    /// window is not going (it is being attached), so its compositor
    /// releases what the thread waits for.
    #[cfg(target_os = "android")]
    pub(crate) fn join_retired(&mut self) {
        for retired in self.retired.drain(..) {
            retired.join();
        }
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        // The open frame's drawables, then each canvas's own, before their
        // surfaces.
        self.open = None;
        for inst in self.instances.values_mut() {
            self.retired.extend(inst.release());
        }
        // A thread still acquiring keeps its surface and drops it when its
        // acquisition ends; nothing else uses this device by then.
        self.reap();
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "release_tests.rs"]
mod tests;
