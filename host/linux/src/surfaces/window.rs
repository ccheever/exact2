//! A full-window game is composed on its module's GPU, with the ordinary
//! painter supplying the UI before and after that canvas. Other layouts use
//! their painter's window target. No wgpu object crosses this DLL boundary.
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
    u32,
    u32,
) -> u32;

pub(super) struct Layers {
    view: u32,
    below: WindowLayer,
    above: Arc<Pixmap>,
    above_rows: (u32, u32),
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
    pub fn window_frame(
        &mut self,
        window: exact_ink::device::d3d11::Window,
    ) -> Result<Option<Arc<Pixmap>>, String> {
        let (hwnd, hinstance) = (window.hwnd(), window.hinstance());
        self.surfaces.window_owner = Some(window);
        if let Some(mut direct) = self.surfaces.direct_window.take() {
            let frame = direct.frame(self);
            self.surfaces.direct_window = Some(direct);
            return Ok(frame);
        }
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
                    self.brush.recycle_window_layers(
                        layers.below.storage(),
                        layers.above,
                        layers.above_rows,
                    );
                }
                self.brush.window_canvas = Some(view);
                let above = self.frame();
                self.brush.window_canvas = None;
                if let Some(below) = self
                    .brush
                    .take_window_underlay()
                    .filter(|_| self.last_frame_succeeded)
                {
                    let above_rows = self
                        .brush
                        .window_overlay_rows()
                        .unwrap_or((0, above.height()));
                    self.surfaces.window_layers = Some(Layers {
                        view,
                        below,
                        above,
                        above_rows,
                    });
                }
            }
            if let Some(Layers {
                below,
                above,
                above_rows,
                ..
            }) = &self.surfaces.window_layers
            {
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
                        above_rows.0,
                        above_rows.1,
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
        if self.surfaces.window_artifact.is_none() {
            return Ok(self.frame_for(crate::paint::FrameTarget::Window));
        }
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
                    0,
                    frame.height(),
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

/// CPU frames retain composition presentation after a device loss; DXGI does
/// not permit returning the HWND to GDI. @ref LLP 1149 D5.4.
pub(crate) struct DirectWindow {
    window: exact_ink::device::d3d11::Window,
    recovering: bool,
    retry: exact_ink::device::Retry,
    fallback: Option<(
        exact_ink::device::d3d11::D3d11,
        exact_ink::instance::TextureId,
    )>,
}
impl DirectWindow {
    pub(crate) fn new(window: exact_ink::device::d3d11::Window) -> Self {
        Self {
            window,
            recovering: false,
            retry: Default::default(),
            fallback: None,
        }
    }
    fn frame<D: DataSource>(&mut self, p: &mut Presenter<D>) -> Option<Arc<Pixmap>> {
        use crate::paint::{Backend, FrameTarget};
        use exact_ink::device::Device;
        let now = std::time::Instant::now();
        let size =
            [p.viewport().0, p.viewport().1].map(|v| (v * p.brush.scale).round().max(1.) as u32);
        let retry_due = self.retry.ready(now);
        if self.recovering && retry_due {
            let opened = crate::ink::Ink::new().and_then(|mut ink| {
                self.fallback = None;
                if !ink.attach_window(self.window.clone(), size)? {
                    return Err("direct device unavailable".into());
                }
                Ok(ink)
            });
            if let Ok(ink) = opened {
                let info = ink.info();
                p.painter = crate::presenter::PainterInfo {
                    name: "ink",
                    adapter: Some(format!("{} ({})", info.adapter, info.api)),
                    device_ms: info.device_ms,
                    shaders_ms: info.shaders_ms,
                    cached: info.cached,
                };
                self.fallback = None;
                p.brush.replace_backend(Box::new(ink));
                self.recovering = false;
                p.repaint();
            }
        }
        if !self.recovering {
            let frame = p.frame_for(FrameTarget::Window);
            if p.last_frame_succeeded() {
                self.retry = Default::default();
                return frame;
            }
            eprintln!("exact-ink: device lost; painting on the CPU through WARP composition");
            p.brush
                .replace_backend(Box::new(crate::raster::Raster::new()));
            p.painter = crate::presenter::PainterInfo {
                name: "cpu",
                adapter: Some("WARP composition recovery".into()),
                device_ms: 0.,
                shaders_ms: 0.,
                cached: false,
            };
            self.recovering = true;
            p.repaint();
        }
        let pixels = p.frame();
        // A failed recovery device is retried on the same bounded schedule as
        // the renderer; an idle window requests the next attempt at the deadline.
        if self.fallback.is_none() && retry_due {
            let opened = (|| {
                let mut d = exact_ink::device::d3d11::D3d11::open(true)?;
                d.attach_window(self.window.clone(), size)?;
                let target = d.create_target(size[0], size[1])?;
                Ok::<_, exact_ink::device::Error>((d, target))
            })();
            match opened {
                Ok(d) => self.fallback = Some(d),
                Err(e) => eprintln!("exact-ink: recovery: {e}"),
            }
        }
        if let Some((device, target)) = &mut self.fallback {
            let presented = (|| {
                *target = device.resize_target(*target, pixels.width(), pixels.height())?;
                device.upload_texture(
                    *target,
                    [0, 0],
                    [pixels.width(), pixels.height()],
                    pixels.data(),
                )?;
                device.present(*target)
            })();
            if let Err(e) = presented {
                eprintln!("exact-ink: recovery present: {e}");
                self.fallback = None;
            }
        }
        if retry_due {
            self.retry.failed(std::time::Instant::now());
        }
        None
    }
}

impl<D: DataSource> Presenter<D> {
    /// Next recovery attempt after a direct painter loses its device.
    pub fn painter_retry_deadline(&self) -> Option<std::time::Instant> {
        self.surfaces
            .direct_window
            .as_ref()
            .filter(|d| d.recovering)
            .and_then(|d| d.retry.deadline())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{Backend, GradientPaint, Rect4, Shape};
    use tiny_skia::Transform;

    struct Lost;
    impl Backend for Lost {
        fn name(&self) -> &'static str {
            "ink"
        }
        fn begin(&mut self, _: f32, _: f32, _: f32) {}
        fn fill(&mut self, _: &Shape, _: [u8; 4], _: Transform) {}
        fn fill_gradient(&mut self, _: &Shape, _: &GradientPaint, _: Transform) {}
        fn fill_border(&mut self, _: &crate::paint::border::BorderFill, _: Transform) {}
        fn image(
            &mut self,
            _: &Arc<crate::image::Bitmap>,
            _: Rect4,
            _: &[Shape],
            _: Transform,
            _: Option<[u8; 4]>,
        ) {
        }
        fn text(
            &mut self,
            _: &mut crate::text::TextEngine,
            _: &crate::text::Paragraph,
            _: &[crate::text::RunPaint],
            _: (f32, f32),
            _: Transform,
        ) {
        }
        fn push_clip(&mut self, _: &Shape, _: Transform) {}
        fn pop_clip(&mut self) {}
        fn push_opacity(&mut self, _: f32) {}
        fn pop_opacity(&mut self) {}
        fn pointer(&mut self, _: f32, _: f32) {}
        fn finish(&mut self) -> Result<Pixmap, String> {
            Err("injected device removal".into())
        }
    }

    #[test]
    #[ignore = "native Windows composition recovery requires fxc and a desktop session"]
    fn device_loss_repaints_on_cpu_through_composition_and_backs_off() {
        use exact_ink::device::Device;
        let window = exact_ink::device::d3d11::Window::fixture([80, 60]).unwrap();
        let (mut p, _) = Presenter::boot_with(
            &crate::test_plan(
                "component App\n  view\n    view width=80 height=60 background-color=\"#229944\"\n",
            ),
            (),
            (80., 60.),
            1.,
            PathBuf::new(),
            crate::presenter::PainterChoice::Cpu,
        )
        .unwrap();
        p.brush.replace_backend(Box::new(Lost));
        p.surfaces.direct_window = Some(DirectWindow::new(window.clone()));
        assert!(p.window_frame(window.clone()).unwrap().is_none());
        assert!(p.last_frame_succeeded());
        assert_eq!(p.painter.name, "cpu");
        let due = p.painter_retry_deadline().unwrap();
        assert!(due > std::time::Instant::now());
        let (device, texture) = p
            .surfaces
            .direct_window
            .as_mut()
            .unwrap()
            .fallback
            .as_mut()
            .unwrap();
        let bytes = device.readback(*texture).unwrap();
        assert!(bytes.chunks_exact(4).all(|c| c == [34, 153, 68, 255]));
        p.window_frame(window).unwrap();
        assert_eq!(p.painter_retry_deadline(), Some(due));
    }
}
