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
            return frame;
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

/// The window retains late preparations and both refusal reasons until it presents.
pub(crate) struct Startup {
    attempts: Option<exact_ink::device::d3d11::Startup>,
    warp: Option<exact_ink::device::d3d11::Preparing>,
    force_warp: bool,
    warp_attempted: bool,
    ready_warp: Option<exact_ink::device::d3d11::D3d11>,
    refusals: [Option<String>; 2],
}

impl Startup {
    pub(crate) fn wait(
        mut attempts: exact_ink::device::d3d11::Startup,
    ) -> Result<(Self, Option<exact_ink::device::d3d11::D3d11>), String> {
        let joined = std::time::Instant::now();
        let deadline = attempts.deadline();
        let mut startup = Self::new(attempts.force_warp());
        startup.warp_attempted = true;
        let mut selected = None;
        while let Some((warp, result)) = attempts.wait_until(deadline) {
            match result {
                Ok(device) => {
                    if warp && attempts.hardware_pending() {
                        startup.ready_warp = Some(device);
                    } else {
                        selected = Some(device);
                    }
                    break;
                }
                Err(error) => startup.refused(warp, error.to_string()),
            }
        }
        if std::env::var_os("EXACT_AGENT_WINDOW_READY").is_some() {
            eprintln!(
                "exact-ink: startup join {:.3} ms",
                joined.elapsed().as_secs_f64() * 1000.
            );
        }
        startup.attempts = Some(attempts);
        if selected.is_none() && startup.ready_warp.is_none() {
            startup.check_exhausted()?;
        }
        Ok((startup, selected))
    }

    fn new(force_warp: bool) -> Self {
        Self {
            attempts: None,
            warp: None,
            force_warp,
            warp_attempted: false,
            ready_warp: None,
            refusals: [None, None],
        }
    }

    fn refused(&mut self, warp: bool, error: String) {
        eprintln!(
            "exact-ink: {}: {error}",
            if warp { "WARP" } else { "hardware" }
        );
        self.refusals[usize::from(warp)] = Some(error);
    }

    fn check_exhausted(&self) -> Result<(), String> {
        if !self.pending() && self.warp_attempted {
            return Err(format!(
                "no composition device: hardware: {}; WARP: {}",
                self.refusals[0].as_deref().unwrap_or(if self.force_warp {
                    "not requested"
                } else {
                    "unavailable"
                }),
                self.refusals[1].as_deref().unwrap_or("unavailable")
            ));
        }
        Ok(())
    }

    fn start_warp(&mut self) {
        if !self.warp_attempted {
            self.warp_attempted = true;
            match exact_ink::device::d3d11::Preparing::start(true) {
                Ok(p) => self.warp = Some(p),
                Err(e) => self.refused(true, e.to_string()),
            }
        }
    }

    fn pending(&self) -> bool {
        self.attempts.as_ref().is_some_and(|a| a.pending()) || self.warp.is_some()
    }
    fn hardware_pending(&self) -> bool {
        self.attempts.as_ref().is_some_and(|a| a.hardware_pending())
    }
}

/// Startup and recovery upload CPU frames through WARP on the shared visual.
/// Without either device, keep waiting for preparation. @ref LLP 1149 D5.4, D6.3.
pub(crate) struct DirectWindow {
    pub(crate) window: exact_ink::device::d3d11::Window,
    pub(crate) recovering: bool,
    retry: exact_ink::device::Retry,
    present_due: Option<std::time::Instant>,
    reservation_error_reported: bool,
    ever_presented: bool,
    pub(crate) last_refusal: Option<String>,
    retire_due: Option<std::time::Instant>,
    poll_due: std::time::Instant,
    startup: Startup,
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
            reservation_error_reported: false,
            ever_presented: false,
            last_refusal: None,
            retire_due: None,
            poll_due: std::time::Instant::now(),
            startup: Startup::new(
                exact_ink::device::direct_d3d11(std::env::var("EXACT_INK_DEVICE").ok().as_deref())
                    .unwrap_or(false),
            ),
            open: exact_ink::device::d3d11::D3d11::open,
            fallback: None,
        }
    }

    pub(crate) fn attach<D: DataSource>(
        p: &mut Presenter<D>,
        window: exact_ink::device::d3d11::Window,
        size: [u32; 2],
        startup: Startup,
    ) -> Result<Self, String> {
        let mut direct = Self::new(window);
        direct.last_refusal = startup.refusals.iter().flatten().last().cloned();
        direct.startup = startup;
        match p.brush.attach_window(direct.window.clone(), size) {
            Ok(true) => return Ok(direct),
            Ok(false) => {}
            Err(error) => {
                direct.refused(direct.startup.force_warp, error);
                // The scheduled WARP may have been cancelled when hardware opened.
                direct.startup.warp_attempted = false;
            }
        }
        direct.use_cpu(p, "CPU startup; composition pending");
        direct.startup.start_warp();
        if let Some(device) = direct.startup.ready_warp.take() {
            if let Err(error) = direct.install_fallback(p, device, size) {
                direct.refused(true, error.to_string());
            }
        }
        direct.retry.failed(std::time::Instant::now());
        if direct.fallback.is_none() {
            direct.startup.check_exhausted()?;
        }
        Ok(direct)
    }

    fn use_cpu<D: DataSource>(&mut self, p: &mut Presenter<D>, reason: &str) {
        p.brush
            .replace_backend(Box::new(crate::raster::Raster::new()));
        p.painter = crate::presenter::PainterInfo {
            name: "cpu",
            adapter: Some(reason.into()),
            api: None,
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
            api: Some(info.api.clone()),
            device_ms: info.device_ms,
            shaders_ms: info.shaders_ms,
            cached: info.cached,
        };
        p.brush.replace_backend(Box::new(ink));
        self.retire_due = Some(std::time::Instant::now());
        self.recovering = false;
        p.repaint();
        Ok(())
    }

    fn install_fallback<D: DataSource>(
        &mut self,
        p: &mut Presenter<D>,
        mut device: exact_ink::device::d3d11::D3d11,
        size: [u32; 2],
    ) -> exact_ink::device::Result<()> {
        use exact_ink::device::Device;
        device.attach_window(self.window.clone(), size)?;
        let target = device.create_target(size[0], size[1])?;
        let info = device.info();
        p.painter.adapter = Some(format!("{} ({})", info.adapter, info.api));
        p.painter.api = Some(info.api.clone());
        p.painter.device_ms = info.device_ms;
        p.painter.shaders_ms = info.shaders_ms;
        p.painter.cached = info.cached;
        self.fallback = Some((device, target));
        Ok(())
    }

    fn refused(&mut self, warp: bool, error: String) {
        self.last_refusal = Some(error.clone());
        self.startup.refused(warp, error);
    }

    fn poll_startup<D: DataSource>(&mut self, p: &mut Presenter<D>, size: [u32; 2]) {
        loop {
            let arrival = self.startup.attempts.as_mut().and_then(|a| a.poll());
            let arrival = arrival.or_else(|| {
                let result = self.startup.warp.as_mut().and_then(|p| p.poll())?;
                self.startup.warp = None;
                Some((true, result))
            });
            let Some((warp, result)) = arrival else { break };
            if warp && !self.recovering {
                continue;
            }
            let installed = result.and_then(|device| {
                if warp && self.startup.hardware_pending() {
                    self.install_fallback(p, device, size)
                } else {
                    self.install(p, device, size)
                }
            });
            if let Err(e) = installed {
                self.refused(warp, e.to_string());
                if !warp && !self.startup.pending() && self.startup.refusals[1].is_none() {
                    self.startup.warp_attempted = false;
                    self.startup.start_warp();
                }
            }
        }
    }

    fn frame<D: DataSource>(
        &mut self,
        p: &mut Presenter<D>,
    ) -> Result<Option<Arc<Pixmap>>, String> {
        use crate::paint::FrameTarget;
        use exact_ink::device::Device;
        let now = std::time::Instant::now();
        self.poll_due = now + std::time::Duration::from_millis(16);
        let size =
            [p.viewport().0, p.viewport().1].map(|v| (v * p.brush.scale).round().max(1.) as u32);
        self.window
            .set_background(crate::paint::page_color(p.dark()));
        self.poll_startup(p, size);
        if !self.ever_presented && self.recovering && self.fallback.is_none() {
            self.startup.check_exhausted()?;
        }
        let retry_due = self.retry.ready(now) && !self.startup.hardware_pending();
        // WARP refusal must not prevent a later hardware attempt.
        if self.recovering && retry_due {
            let result = (self.open)(self.startup.force_warp)
                .and_then(|device| self.install(p, device, size));
            if let Err(error) = result {
                self.refused(self.startup.force_warp, error.to_string());
            }
        }
        if !self.recovering {
            if !p.dirty() && !p.wants_display_frames() && !p.host().motion() {
                p.last_frame_succeeded = false;
                return Ok(None);
            }
            let ready = reserve_frame(p, size, &mut self.reservation_error_reported);
            let frame = if ready {
                p.frame_for(FrameTarget::Window)
            } else {
                p.last_frame_succeeded = false;
                None
            };
            self.present_due = p
                .brush
                .presentation_retry_delay()
                .or_else(|| (!ready).then_some(std::time::Duration::from_secs_f64(1. / 60.)))
                .map(|delay| std::time::Instant::now() + delay);
            if self.present_due.is_some() {
                p.repaint();
                p.last_frame_succeeded = false;
            }
            if p.last_frame_succeeded() {
                self.retry = Default::default();
                self.ever_presented = true;
                return Ok(frame);
            }
            if !p.brush.device_lost() {
                return Ok(frame);
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
                    let info = d.0.info();
                    p.painter.adapter = Some(format!("{} ({})", info.adapter, info.api));
                    p.painter.api = Some(info.api.clone());
                    p.painter.device_ms = info.device_ms;
                    p.painter.shaders_ms = info.shaders_ms;
                    p.painter.cached = info.cached;
                    self.fallback = Some(d);
                }
                Err(e) => self.refused(true, e.to_string()),
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
                    self.present_due = device
                        .presentation_retry_delay()
                        .map(|delay| std::time::Instant::now() + delay);
                    p.last_frame_succeeded = ms > 0.;
                    self.ever_presented |= ms > 0.;
                    if ms > 0. && std::env::var_os("EXACT_AGENT_WINDOW_READY").is_some() {
                        eprintln!("exact-ink: CPU recovery frame presented through composition");
                    }
                }
            }
        }
        if retry_due {
            self.retry.failed(std::time::Instant::now());
        }
        Ok(None)
    }
}

fn reserve_frame<D: DataSource>(p: &mut Presenter<D>, size: [u32; 2], reported: &mut bool) -> bool {
    match p.brush.prepare_window_frame(size) {
        Ok(ready) => {
            if ready {
                *reported = false;
            }
            ready
        }
        Err(error) => {
            if !p.brush.device_lost() && !*reported {
                eprintln!("exact-ink: frame reservation: {error}; retrying");
                *reported = true;
            }
            false
        }
    }
}

impl<D: DataSource> Presenter<D> {
    /// Drain completed devices on an idle wake without painting. @ref LLP 1149 D11.
    pub fn collect_painter_retirement(
        &mut self,
        now: std::time::Instant,
    ) -> Option<std::time::Instant> {
        let direct = self.surfaces.direct_window.as_mut()?;
        if direct.retire_due.is_some_and(|due| now >= due) {
            let pending = self.brush.collect_retired_devices();
            direct.retire_due = (pending > 0).then(|| now + std::time::Duration::from_millis(50));
        }
        direct.retire_due
    }

    /// A newly drawable window gets a fresh capacity check, keeping device recovery's backoff.
    pub fn reset_painter_present_deadline(&mut self) {
        if let Some(direct) = &mut self.surfaces.direct_window {
            direct.present_due = None;
            if let Some((device, _)) = &mut direct.fallback {
                device.reset_present_deadline();
            }
        }
        self.brush.reset_present_deadline();
        self.repaint();
    }

    /// Earliest paint after a refused present, measured from the refusal.
    /// Recovery backoff is separate: it must not delay input on the CPU fallback.
    pub fn painter_present_deadline(&self) -> Option<std::time::Instant> {
        self.surfaces
            .direct_window
            .as_ref()
            .and_then(|d| d.present_due)
    }

    /// Next readiness, recovery or presentation attempt; asks for a redraw even
    /// without new damage. Only `painter_present_deadline` forbids earlier paint.
    pub fn painter_retry_deadline(&self) -> Option<std::time::Instant> {
        self.surfaces.direct_window.as_ref().and_then(|d| {
            if d.startup.pending() {
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

    #[test]
    fn both_refusals_are_a_terminal_boot_error() {
        for reason in [
            "injected device refusal",
            "D3D11 shaders absent: build with fxc",
        ] {
            let mut startup = Startup::new(false);
            startup.warp_attempted = true;
            startup.refused(false, reason.into());
            startup.refused(true, reason.into());
            let error = startup.check_exhausted().unwrap_err();
            assert!(
                error.contains(&format!("hardware: {reason}; WARP: {reason}")),
                "{error}"
            );
        }
    }

    #[test]
    #[ignore = "native Windows fixture requires a desktop session"]
    fn mica_refusal_uses_page_color_in_both_schemes() {
        let window = exact_ink::device::d3d11::Window::fixture([80, 60]).unwrap();
        let mut p = cpu_presenter();
        p.brush.replace_backend(Box::new(Lost(false, false)));
        p.surfaces.direct_window = Some(DirectWindow::new(window.clone()));
        for dark in [false, true] {
            p.set_system_scheme(dark);
            p.window_frame(window.clone()).unwrap();
            let color = crate::paint::page_color(dark);
            assert_eq!(window.background(), [color[0], color[1], color[2], 255]);
        }
    }

    #[test]
    #[ignore = "native Windows composition requires fxc and a desktop session"]
    fn forced_warp_state_reports_device_api_and_recovery() {
        let window = exact_ink::device::d3d11::Window::fixture([80, 60]).unwrap();
        let mut p = cpu_presenter();
        let mut direct = DirectWindow::new(window);
        direct.last_refusal = Some("hardware refused".into());
        direct
            .install(
                &mut p,
                exact_ink::device::d3d11::D3d11::open(true).unwrap(),
                [80, 60],
            )
            .unwrap();
        p.surfaces.direct_window = Some(direct);
        let state: serde_json::Value =
            serde_json::from_str(&crate::agent::answer(&mut p, r#"{"op":"state"}"#)).unwrap();
        let info = &state["painter"];
        assert_eq!(info["name"], "ink");
        assert!(info["device"].as_str().unwrap().contains("WARP"));
        assert!(info["api"].as_str().unwrap().contains("11"));
        assert_eq!(info["recovering"], false);
        assert_eq!(info["lastRefusal"], "hardware refused");
    }

    #[test]
    #[ignore = "native Windows composition requires fxc and a desktop session"]
    fn idle_recovery_releases_the_old_device_without_repainting() {
        let window = exact_ink::device::d3d11::Window::fixture([80, 60]).unwrap();
        let mut p = cpu_presenter();
        let mut direct = DirectWindow::new(window.clone());
        direct
            .install_fallback(
                &mut p,
                exact_ink::device::d3d11::D3d11::open(true).unwrap(),
                [80, 60],
            )
            .unwrap();
        direct.recovering = true;
        direct.retry.failed(std::time::Instant::now());
        p.surfaces.direct_window = Some(direct);
        p.window_frame(window.clone()).unwrap();
        let mut direct = p.surfaces.direct_window.take().unwrap();
        direct
            .install(
                &mut p,
                exact_ink::device::d3d11::D3d11::open(false).unwrap(),
                [80, 60],
            )
            .unwrap();
        p.surfaces.direct_window = Some(direct);
        let end = std::time::Instant::now() + std::time::Duration::from_millis(250);
        while let Some(due) = p.collect_painter_retirement(std::time::Instant::now()) {
            assert!(
                std::time::Instant::now() < end,
                "retirement did not drain while idle"
            );
            window.dispatch_pending();
            std::thread::sleep(due.saturating_duration_since(std::time::Instant::now()));
        }
        assert_eq!(p.brush.collect_retired_devices(), 0);
    }

    struct Lost(bool, bool);
    impl Backend for Lost {
        fn prepare_window_frame(&mut self, _: [u32; 2]) -> Result<bool, String> {
            if self.1 {
                Err("injected reservation refusal".into())
            } else {
                Ok(true)
            }
        }
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
        p.brush.replace_backend(Box::new(Lost(true, false)));
        p.surfaces.direct_window = Some(DirectWindow::new(window.clone()));
        assert!(p.window_frame(window.clone()).unwrap().is_none());
        assert!(p.last_frame_succeeded());
        assert_eq!(p.painter.name, "cpu");
        let due = p
            .surfaces
            .direct_window
            .as_ref()
            .unwrap()
            .retry
            .deadline()
            .unwrap();
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
        assert_eq!(
            p.surfaces.direct_window.as_ref().unwrap().retry.deadline(),
            Some(due)
        );
    }
    fn cpu_presenter() -> Presenter<()> {
        Presenter::boot_with(
            &crate::test_plan(
                "component App\n  view\n    view width=80 height=60 background-color=\"#229944\"\n",
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
    #[allow(unsafe_code)]
    fn reservation_refusal_retries_and_lifecycle_clears_its_deadline() {
        use wgpu::rwh as raw_window_handle;
        struct UnusedWindow;
        impl raw_window_handle::HasWindowHandle for UnusedWindow {
            fn window_handle(
                &self,
            ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError>
            {
                let raw = raw_window_handle::Win32WindowHandle::new(
                    std::num::NonZeroIsize::new(1).unwrap(),
                );
                // The stub backend never passes this handle to a native API.
                Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw.into()) })
            }
        }
        let window =
            exact_ink::device::d3d11::Window::from_owner(std::rc::Rc::new(UnusedWindow)).unwrap();
        let mut p = cpu_presenter();
        p.frame();
        p.brush.replace_backend(Box::new(Lost(false, true)));
        let mut direct = DirectWindow::new(window);
        assert!(!p.dirty());
        assert!(direct.frame(&mut p).unwrap().is_none());
        assert!(
            !direct.reservation_error_reported,
            "a clean frame must not reserve"
        );
        assert_eq!(direct.present_due, None);
        p.repaint();
        for _ in 0..2 {
            assert!(direct.frame(&mut p).unwrap().is_none());
            assert!(!direct.recovering);
            assert!(direct.reservation_error_reported);
            assert!(direct.present_due.unwrap() > std::time::Instant::now());
            assert!(!p.last_frame_succeeded());
            assert_eq!(p.brush.backend(), "ink");
            assert!(p.dirty());
        }
        direct.retry.failed(std::time::Instant::now());
        let recovery = direct.retry.deadline();
        p.surfaces.direct_window = Some(direct);
        p.reset_painter_present_deadline();
        assert_eq!(p.painter_present_deadline(), None);
        assert_eq!(
            p.surfaces.direct_window.as_ref().unwrap().retry.deadline(),
            recovery
        );
        assert!(p.dirty());
        let mut reported = true;
        p.brush
            .replace_backend(Box::new(crate::raster::Raster::new()));
        p.frame();
        assert!(
            p.dirty(),
            "a CPU snapshot must retain unpresented window damage"
        );
        p.frame_for(crate::paint::FrameTarget::Geometry);
        assert!(p.dirty(), "geometry must retain unpresented window damage");
        assert!(reserve_frame(&mut p, [80, 60], &mut reported));
        assert!(!reported);
        p.brush.replace_backend(Box::new(Lost(true, true)));
        assert!(!reserve_frame(&mut p, [80, 60], &mut reported));
        assert!(p.brush.device_lost());
        assert!(!reported);
    }

    #[test]
    #[ignore = "native Windows composition requires fxc and a desktop session"]
    fn refused_hardware_attachment_retries_warp_and_reports_it() {
        let window = exact_ink::device::d3d11::Window::fixture([80, 60]).unwrap();
        let mut p = cpu_presenter();
        p.brush.replace_backend(Box::new(Lost(true, false)));
        let startup = Startup::new(false);
        let direct = DirectWindow::attach(&mut p, window.clone(), [80, 60], startup).unwrap();
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
        p.brush.replace_backend(Box::new(Lost(true, false)));
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
        assert_eq!(p.painter.name, "cpu");
        assert!(p
            .surfaces
            .direct_window
            .as_ref()
            .unwrap()
            .fallback
            .is_none());
        assert!(!p.last_frame_succeeded());
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
        direct.startup.attempts = Some(exact_ink::device::d3d11::Startup::start(false).unwrap());
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
        p.brush.replace_backend(Box::new(Lost(false, false)));
        p.surfaces.direct_window = Some(DirectWindow::new(window.clone()));
        p.window_frame(window).unwrap();
        let direct = p.surfaces.direct_window.as_ref().unwrap();
        assert!(!direct.recovering);
        assert!(direct.fallback.is_none());
        assert_eq!(p.brush.backend(), "ink");
    }
}
