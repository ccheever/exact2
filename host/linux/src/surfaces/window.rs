//! A full-window game is composed on its module's GPU, with the ordinary
//! painter supplying the UI before and after that canvas. All other layouts
//! keep the capture path. No wgpu object crosses this DLL boundary.
use super::*;
use crate::paint::WindowLayer;
use std::sync::Arc;
use tiny_skia::Pixmap;

type Present = unsafe extern "C" fn(
    u32,
    usize,
    usize,
    f32,
    f32,
    f32,
    f64,
    *const u8,
    *const u8,
    usize,
    u32,
) -> u32;

pub(super) struct Layers {
    view: u32,
    below: WindowLayer,
    above: Arc<Pixmap>,
}

impl Surfaces {
    fn window_canvas(&self) -> Option<u32> {
        if self.canvases.len() != 1 {
            return None;
        }
        let (&view, canvas) = self.canvases.first_key_value()?;
        self.abis.get(&canvas.artifact).filter(|a| a.rendered)?;
        Some(view)
    }
}

impl<D: DataSource> Presenter<D> {
    /// Paint and present a Windows frame. `None` means it was presented on
    /// the GPU; `Some` is the complete CPU frame for the window to upload.
    ///
    /// # Safety
    /// `hwnd` is a live HWND that outlives this presenter.
    pub unsafe fn window_frame(
        &mut self,
        hwnd: usize,
        hinstance: usize,
    ) -> Result<Option<Arc<Pixmap>>, String> {
        let candidate = (self.brush.backend() == "cpu" && self.host.content_region().is_none())
            .then(|| self.surfaces.window_canvas())
            .flatten();
        if let Some(view) = candidate {
            let artifact = &self.surfaces.canvases[&view].artifact;
            if let Some(previous) = self
                .surfaces
                .window_artifact
                .as_ref()
                .filter(|old| *old != artifact)
            {
                if let Some(abi) = self.surfaces.abis.get(previous) {
                    unsafe {
                        abi.symbol::<unsafe extern "C" fn()>(b"gpu_window_detach")();
                    }
                }
                self.surfaces.window_artifact = None;
                self.surfaces.window_layers = None;
            }
            let cached = !self.dirty()
                && self.surfaces.placements(&self.host).is_empty()
                && self
                    .surfaces
                    .window_layers
                    .as_ref()
                    .is_some_and(|l| l.view == view);
            if !cached {
                if let Some(layers) = self.surfaces.window_layers.take() {
                    self.brush
                        .recycle_window_layers(layers.below.storage(), layers.above);
                }
                self.brush.window_canvas = Some(view);
                let above = self.frame();
                self.brush.window_canvas = None;
                if let Some(below) = self
                    .brush
                    .take_window_underlay()
                    .filter(|_| self.last_frame_succeeded)
                {
                    self.surfaces.window_layers = Some(Layers { view, below, above });
                }
            }
            if let Some(Layers { below, above, .. }) = &self.surfaces.window_layers {
                let canvas = &self.surfaces.canvases[&view];
                let abi = &self.surfaces.abis[&canvas.artifact];
                let present = unsafe { abi.symbol::<Present>(b"gpu_window_frame") };
                let result = unsafe {
                    present(
                        canvas.id,
                        hwnd,
                        hinstance,
                        self.viewport().0,
                        self.viewport().1,
                        self.brush.scale,
                        self.host.now(),
                        below.bytes().as_ptr(),
                        above.data().as_ptr(),
                        above.data().len(),
                        u32::from(!cached)
                            | (u32::from(matches!(below, WindowLayer::Solid { .. })) << 1),
                    )
                };
                if result == 0 {
                    self.surfaces.window_artifact = Some(canvas.artifact.clone());
                    return Ok(None);
                }
                return Err(abi
                    .error()
                    .unwrap_or("GPU window presentation failed".into()));
            }
        }
        self.surfaces.window_layers = None;
        let frame = self.frame();
        if let Some(abi) = self
            .surfaces
            .window_artifact
            .as_ref()
            .and_then(|a| self.surfaces.abis.get(a))
        {
            // A complete CPU frame has the host's opaque background. Continue
            // displaying it on the GPU once this HWND has acquired a swapchain.
            let result = unsafe {
                abi.symbol::<Present>(b"gpu_window_frame")(
                    0,
                    hwnd,
                    hinstance,
                    self.viewport().0,
                    self.viewport().1,
                    self.brush.scale,
                    self.host.now(),
                    frame.data().as_ptr(),
                    frame.data().as_ptr(),
                    frame.data().len(),
                    1,
                )
            };
            if result == 0 {
                return Ok(None);
            }
            return Err(abi
                .error()
                .unwrap_or("GPU window presentation failed".into()));
        }
        Ok(Some(frame))
    }
}
