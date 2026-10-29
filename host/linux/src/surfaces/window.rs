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

/// Pending startup devices remain owned after the first-frame budget expires.
pub(crate) struct Startup {
    primary: Option<exact_ink::device::d3d11::Preparing>,
    warp: Option<exact_ink::device::d3d11::Preparing>,
    force_warp: bool,
    warp_attempted: bool,
}

impl Startup {
    pub(crate) fn wait(
        mut preparing: exact_ink::device::d3d11::Preparing,
        started: std::time::Instant,
    ) -> (Self, Option<exact_ink::device::d3d11::D3d11>) {
        let deadline = started + std::time::Duration::from_millis(100);
        let force_warp = preparing.force_warp();
        let result = preparing.finish_until(deadline);
        let mut startup = Self {
            primary: None,
            warp: None,
            force_warp,
            warp_attempted: force_warp,
        };
        match result {
            Some(Ok(device)) => return (startup, Some(device)),
            Some(Err(error)) => eprintln!("exact-ink: startup device: {error}"),
            None => startup.primary = Some(preparing),
        }
        startup.start_warp();
        if let Some(result) = startup.warp.as_mut().and_then(|p| p.finish_until(deadline)) {
            startup.warp = None;
            match result {
                Ok(device) => return (startup, Some(device)),
                Err(error) => eprintln!("exact-ink: startup WARP: {error}"),
            }
        }
        (startup, None)
    }

    fn start_warp(&mut self) {
        if !self.warp_attempted {
            self.warp_attempted = true;
            match exact_ink::device::d3d11::Preparing::start(true) {
                Ok(p) => self.warp = Some(p),
                Err(e) => eprintln!("exact-ink: WARP preparation: {e}"),
            }
        }
    }

    fn pending(&self) -> bool {
        self.primary.is_some() || self.warp.is_some()
    }
}

/// CPU recovery retains composition presentation. GDI is confined to a child
/// HWND when neither device is ready. @ref LLP 1149 D5.4, D6.3.
pub(crate) struct DirectWindow {
    pub(crate) window: exact_ink::device::d3d11::Window,
    pub(crate) recovering: bool,
    retry: exact_ink::device::Retry,
    present_due: Option<std::time::Instant>,
    poll_due: std::time::Instant,
    startup: Startup,
    bitmap: Option<exact_ink::device::d3d11::CpuWindow>,
    open: fn(bool) -> exact_ink::device::Result<exact_ink::device::d3d11::D3d11>,
    pub(crate) fallback: Option<(
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
            present_due: None,
            poll_due: std::time::Instant::now(),
            startup: Startup {
                primary: None,
                warp: None,
                force_warp: exact_ink::device::direct_d3d11(
                    std::env::var("EXACT_INK_DEVICE").ok().as_deref(),
                )
                .unwrap_or(false),
                warp_attempted: false,
            },
            bitmap: None,
            open: exact_ink::device::d3d11::D3d11::open,
            fallback: None,
        }
    }

    pub(crate) fn attach<D: DataSource>(
        p: &mut Presenter<D>,
        window: exact_ink::device::d3d11::Window,
        size: [u32; 2],
        startup: Startup,
    ) -> Self {
        let mut direct = Self::new(window);
        direct.startup = startup;
        match p.brush.attach_window(direct.window.clone(), size) {
            Ok(true) => return direct,
            Ok(false) => {}
            Err(error) => eprintln!("exact-ink: startup composition: {error}"),
        }
        direct.use_cpu(p, "CPU startup; composition pending");
        direct.startup.start_warp();
        direct.retry.failed(std::time::Instant::now());
        direct
    }

    fn use_cpu<D: DataSource>(&mut self, p: &mut Presenter<D>, reason: &str) {
        p.brush
            .replace_backend(Box::new(crate::raster::Raster::new()));
        p.painter = crate::presenter::PainterInfo {
            name: "cpu",
            adapter: Some(reason.into()),
            device_ms: 0.,
            shaders_ms: 0.,
            cached: false,
        };
        self.recovering = true;
        p.repaint();
    }

    fn install<D: DataSource>(
        &mut self,
        p: &mut Presenter<D>,
        mut device: exact_ink::device::d3d11::D3d11,
        size: [u32; 2],
    ) -> exact_ink::device::Result<()> {
        use exact_ink::device::Device;
        device.attach_window(self.window.clone(), size)?;
        let pixels = p.frame();
        let target = device.create_target(size[0], size[1])?;
        device.upload_texture(target, [0, 0], size, pixels.data())?;
        device.present(target)?;
        device.retire_texture(target)?;
        if let Some((previous, _)) = self.fallback.take() {
            device.retire_after_present(previous);
        }
        let ink =
            crate::ink::Ink::from_device(exact_ink::device::Selected::D3d11(Box::new(device)));
        let info = ink.info();
        p.painter = crate::presenter::PainterInfo {
            name: "ink",
            adapter: Some(format!("{} ({})", info.adapter, info.api)),
            device_ms: info.device_ms,
            shaders_ms: info.shaders_ms,
            cached: info.cached,
        };
        p.brush.replace_backend(Box::new(ink));
        self.recovering = false;
        p.repaint();
        Ok(())
    }

    fn poll_startup<D: DataSource>(&mut self, p: &mut Presenter<D>, size: [u32; 2]) {
        use exact_ink::device::Device;
        if let Some(result) = self.startup.primary.as_mut().and_then(|p| p.poll()) {
            self.startup.primary = None;
            if let Err(e) = result.and_then(|d| self.install(p, d, size)) {
                eprintln!("exact-ink: startup device: {e}");
                self.startup.start_warp();
            }
        }
        if let Some(result) = self.startup.warp.as_mut().and_then(|p| p.poll()) {
            self.startup.warp = None;
            if self.recovering {
                let installed = result.and_then(|mut d| {
                    if self.startup.primary.is_some() {
                        d.attach_window(self.window.clone(), size)?;
                        let target = d.create_target(size[0], size[1])?;
                        self.fallback = Some((d, target));
                        p.painter.adapter = Some("WARP composition; hardware pending".into());
                        Ok(())
                    } else {
                        self.install(p, d, size)
                    }
                });
                if let Err(e) = installed {
                    eprintln!("exact-ink: startup WARP: {e}");
                }
            }
        }
    }

    fn frame<D: DataSource>(&mut self, p: &mut Presenter<D>) -> Option<Arc<Pixmap>> {
        use crate::paint::FrameTarget;
        use exact_ink::device::Device;
        let now = std::time::Instant::now();
        self.poll_due = now + std::time::Duration::from_millis(16);
        let size =
            [p.viewport().0, p.viewport().1].map(|v| (v * p.brush.scale).round().max(1.) as u32);
        self.poll_startup(p, size);
        // Keep the bitmap until DXGI observes its replacement on screen, not
        // merely until Present returns. The composition target is above children.
        if self.bitmap.is_some()
            && (matches!(p.brush.displayed_process_ms(), Ok(Some(_)))
                || self
                    .fallback
                    .as_ref()
                    .is_some_and(|(d, _)| d.presented_process_ms().is_ok()))
        {
            self.bitmap = None;
        }
        let retry_due = self.retry.ready(now) && !self.startup.pending();
        // WARP refusal must not prevent a later hardware attempt.
        if self.recovering && retry_due {
            let result = (self.open)(self.startup.force_warp)
                .and_then(|device| self.install(p, device, size));
            if let Err(error) = result {
                eprintln!("exact-ink: hardware recovery: {error}");
            }
        }
        if !self.recovering {
            let frame = p.frame_for(FrameTarget::Window);
            self.present_due = p
                .brush
                .presentation_pending()
                .then(|| now + std::time::Duration::from_millis(16));
            if self.present_due.is_some() {
                p.repaint();
                p.last_frame_succeeded = false;
            }
            if p.last_frame_succeeded() {
                self.retry = Default::default();
                return frame;
            }
            if !p.brush.device_lost() {
                return frame;
            }
            eprintln!("exact-ink: device lost; painting on the CPU through WARP composition");
            self.use_cpu(p, "WARP composition recovery");
        }
        let pixels = p.frame();
        if self.fallback.is_none() && retry_due {
            let opened = (|| {
                let mut d = (self.open)(true)?;
                d.attach_window(self.window.clone(), size)?;
                let target = d.create_target(size[0], size[1])?;
                Ok::<_, exact_ink::device::Error>((d, target))
            })();
            match opened {
                Ok(d) => {
                    self.fallback = Some(d);
                    p.painter.adapter = Some("WARP composition recovery".into());
                }
                Err(e) => eprintln!("exact-ink: recovery: {e}"),
            }
        }
        p.last_frame_succeeded = false;
        self.present_due = None;
        if let Some((device, target)) = &mut self.fallback {
            let presented = (|| {
                *target = device.resize_target(*target, pixels.width(), pixels.height())?;
                device.upload_texture(*target, [0, 0], size, pixels.data())?;
                device.present(*target)
            })();
            match presented {
                Err(e) => {
                    eprintln!("exact-ink: recovery present: {e}");
                    self.fallback = None;
                }
                Ok(ms) => {
                    self.present_due =
                        (ms == 0.).then(|| now + std::time::Duration::from_millis(16));
                    p.last_frame_succeeded = ms > 0.;
                    if ms > 0. && std::env::var_os("EXACT_AGENT_WINDOW_READY").is_some() {
                        eprintln!("exact-ink: CPU recovery frame presented through composition");
                    }
                }
            }
        }
        if !p.last_frame_succeeded {
            let shown = (|| {
                if self.bitmap.is_none() {
                    self.bitmap = Some(exact_ink::device::d3d11::CpuWindow::new(
                        self.window.clone(),
                    )?);
                }
                self.bitmap.as_mut().unwrap().present(size, pixels.data())
            })();
            p.last_frame_succeeded = shown.is_ok();
            p.painter.adapter = Some(
                if self.startup.pending() {
                    "CPU bitmap; composition device pending"
                } else {
                    "CPU bitmap; composition unavailable"
                }
                .into(),
            );
            if let Err(error) = shown {
                eprintln!("exact-ink: CPU startup frame: {error}");
            }
        }
        if retry_due {
            self.retry.failed(std::time::Instant::now());
        }
        None
    }
}

impl<D: DataSource> Presenter<D> {
    /// Next readiness or recovery attempt for a directly presenting painter.
    pub fn painter_retry_deadline(&self) -> Option<std::time::Instant> {
        self.surfaces.direct_window.as_ref().and_then(|d| {
            if d.startup.pending()
                || (d.bitmap.is_some() && (!d.recovering || d.fallback.is_some()))
            {
                Some(d.poll_due)
            } else if d.recovering {
                d.retry.deadline().into_iter().chain(d.present_due).min()
            } else {
                d.present_due
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{Backend, GradientPaint, Rect4, Shape};
    use tiny_skia::Transform;

    struct Lost(bool);
    impl Backend for Lost {
        fn attach_window(
            &mut self,
            _: exact_ink::device::d3d11::Window,
            _: [u32; 2],
        ) -> Result<bool, String> {
            Err("injected hardware attachment refusal".into())
        }
        fn device_lost(&self) -> bool {
            self.0
        }
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
        p.brush.replace_backend(Box::new(Lost(true)));
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
    fn cpu_presenter() -> Presenter<()> {
        Presenter::boot_with(
            &crate::test_plan(
                "component App\n  view width=80 height=60 background-color=\"#229944\"\n",
            ),
            (),
            (80., 60.),
            1.,
            PathBuf::new(),
            crate::presenter::PainterChoice::Cpu,
        )
        .unwrap()
        .0
    }

    #[test]
    #[ignore = "native Windows composition requires fxc and a desktop session"]
    fn refused_hardware_attachment_retries_warp_and_reports_it() {
        let window = exact_ink::device::d3d11::Window::fixture([80, 60]).unwrap();
        let mut p = cpu_presenter();
        p.brush.replace_backend(Box::new(Lost(true)));
        let startup = Startup {
            primary: None,
            warp: None,
            force_warp: false,
            warp_attempted: false,
        };
        let direct = DirectWindow::attach(&mut p, window.clone(), [80, 60], startup);
        p.surfaces.direct_window = Some(direct);
        let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while p.painter.name != "ink" || !p.last_frame_succeeded() {
            assert!(
                std::time::Instant::now() < end,
                "WARP did not display the frame"
            );
            window.dispatch_pending();
            p.window_frame(window.clone()).unwrap();
            std::thread::yield_now();
        }
        assert!(p.painter.adapter.as_ref().unwrap().contains("WARP"));
        assert!(p
            .frame()
            .data()
            .chunks_exact(4)
            .all(|c| c == [34, 153, 68, 255]));
    }

    #[test]
    #[ignore = "native Windows composition requires fxc and a desktop session"]
    fn refused_warp_does_not_block_later_hardware_recovery() {
        let window = exact_ink::device::d3d11::Window::fixture([80, 60]).unwrap();
        let mut p = cpu_presenter();
        p.brush.replace_backend(Box::new(Lost(true)));
        let mut direct = DirectWindow::new(window.clone());
        direct.open = |warp| {
            if warp {
                Err(exact_ink::device::Error::Device(
                    "injected WARP refusal".into(),
                ))
            } else {
                exact_ink::device::d3d11::D3d11::open(false)
            }
        };
        p.surfaces.direct_window = Some(direct);
        p.window_frame(window.clone()).unwrap();
        assert!(
            p.last_frame_succeeded(),
            "the CPU child must show the frame"
        );
        assert!(p
            .surfaces
            .direct_window
            .as_ref()
            .unwrap()
            .fallback
            .is_none());
        assert!(p.surfaces.direct_window.as_ref().unwrap().bitmap.is_some());
        p.surfaces.direct_window.as_mut().unwrap().retry = Default::default();
        p.window_frame(window.clone()).unwrap();
        assert_eq!(p.painter.name, "ink");
        assert!(!p.surfaces.direct_window.as_ref().unwrap().recovering);
        assert!(!p.painter.adapter.as_ref().unwrap().contains("WARP"));
        assert!(p
            .frame()
            .data()
            .chunks_exact(4)
            .all(|c| c == [34, 153, 68, 255]));
    }
    #[test]
    #[ignore = "native Windows fixture requires a desktop session"]
    fn startup_poll_deadline_does_not_slide_while_idle() {
        let window = exact_ink::device::d3d11::Window::fixture([80, 60]).unwrap();
        let mut p = cpu_presenter();
        let mut direct = DirectWindow::new(window.clone());
        direct.bitmap = Some(exact_ink::device::d3d11::CpuWindow::new(window).unwrap());
        let due = std::time::Instant::now() - std::time::Duration::from_millis(1);
        direct.poll_due = due;
        p.surfaces.direct_window = Some(direct);
        assert_eq!(p.painter_retry_deadline(), Some(due));
        assert_eq!(p.painter_retry_deadline(), Some(due));
    }

    #[test]
    #[ignore = "native Windows fixture requires a desktop session"]
    fn an_ordinary_paint_error_does_not_replace_the_device() {
        let window = exact_ink::device::d3d11::Window::fixture([80, 60]).unwrap();
        let mut p = cpu_presenter();
        p.brush.replace_backend(Box::new(Lost(false)));
        p.surfaces.direct_window = Some(DirectWindow::new(window.clone()));
        p.window_frame(window).unwrap();
        let direct = p.surfaces.direct_window.as_ref().unwrap();
        assert!(!direct.recovering);
        assert!(direct.fallback.is_none());
        assert_eq!(p.brush.backend(), "ink");
    }
}
