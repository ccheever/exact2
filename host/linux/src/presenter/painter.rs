//! Which backend paints: the choice, the registry `EXACT_PAINTER=custom`
//! boots from, and opening it.
use super::*;

/// The first scene, to present after showing the window and before another paint.
#[cfg(target_os = "macos")]
pub struct PreparedAppleFrame(Option<Arc<Pixmap>>);

/// Which backend paints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PainterChoice {
    /// The GPU when there is an adapter, else the CPU with a note on stderr.
    Auto,
    /// vello over wgpu; a boot error when there is no adapter.
    Gpu,
    /// Instanced geometry; `EXACT_INK_DEVICE` selects wgpu, Metal or D3D11 (LLP 1149).
    Ink,
    /// tiny-skia.
    Cpu,
    /// Recorded for Android's Canvas (`canvas.rs`).
    #[cfg(any(target_os = "android", test))]
    Canvas,
    /// The painter an app registered with [`set_custom_painter`]
    /// (`EXACT_PAINTER=custom`): an experiment's backend kept out of this crate.
    Custom,
}

/// Makes a backend on the thread that will paint with it.
pub type PainterFactory = fn() -> Result<Box<dyn Backend>, String>;
static CUSTOM: std::sync::OnceLock<(&'static str, PainterFactory)> = std::sync::OnceLock::new();

/// Register the backend `EXACT_PAINTER=custom` boots with, named for reports
/// (LLP 1076's renderer comparison builds one outside the host).
pub fn set_custom_painter(name: &'static str, make: PainterFactory) {
    let _ = CUSTOM.set((name, make));
}
impl PainterChoice {
    /// The baked `host.<platform>.painter`, or this host's existing default.
    pub fn from_baked(compat: &str) -> Self {
        #[derive(Default, serde::Deserialize)]
        struct Baked {
            painter: Option<String>,
        }
        match serde_json::from_str::<Baked>(compat)
            .unwrap_or_default()
            .painter
            .as_deref()
        {
            Some("ink") => Self::Ink,
            _ if cfg!(windows) => Self::Cpu,
            _ => Self::Auto,
        }
    }

    /// `EXACT_PAINTER` overrides the manifest's choice (LLP 1149 D12).
    pub fn from_env(manifest: PainterChoice) -> PainterChoice {
        manifest.with_override(std::env::var("EXACT_PAINTER").ok().as_deref())
    }

    fn with_override(self, value: Option<&str>) -> Self {
        match value {
            None => self,
            Some("gpu") => Self::Gpu,
            Some("ink") => Self::Ink,
            Some("cpu") => Self::Cpu,
            Some("custom") => Self::Custom,
            #[cfg(any(target_os = "android", test))]
            Some("canvas") => Self::Canvas,
            _ => Self::Auto,
        }
    }
}

#[cfg(any(target_os = "android", test))]
const ANDROID_INK_UNAVAILABLE: &str = "host.android.painter=\"ink\" / EXACT_PAINTER=ink is not yet honoured: ink's wgpu Vulkan surface presenter is not implemented (LLP 1149 S4)";

/// What the painter is, for the smoke's report.
#[derive(Debug, Clone)]
pub struct PainterInfo {
    /// The selected backend's name.
    pub name: &'static str,
    /// The adapter and API, on the GPU.
    pub adapter: Option<String>,
    /// The rendering API, when a device is installed.
    pub api: Option<String>,
    /// Device creation, milliseconds, on the GPU.
    pub device_ms: f64,
    /// Shader compilation, milliseconds, on the GPU.
    pub shaders_ms: f64,
    /// Whether the shaders came from the pipeline cache on disk.
    pub cached: bool,
}

pub(super) fn cpu_info() -> PainterInfo {
    PainterInfo {
        name: "cpu",
        adapter: None,
        api: None,
        device_ms: 0.0,
        shaders_ms: 0.0,
        cached: false,
    }
}

type OpenedPainter = Result<(Box<dyn Backend>, PainterInfo), String>;

/// A carrier chooses its constructor without retaining every native backend.
/// The backend is installed after validation, on the paint thread.
#[derive(Clone, Copy)]
pub(crate) struct PainterBoot {
    pub(super) choice: PainterChoice,
    open: fn(PainterChoice) -> OpenedPainter,
}

impl PainterBoot {
    #[cfg(windows)]
    pub(super) fn prepare_direct(
        self,
    ) -> Option<Result<exact_ink::device::d3d11::Preparing, String>> {
        if self.choice != PainterChoice::Ink
            || !std::ptr::fn_addr_eq(
                self.open,
                open_backend as fn(PainterChoice) -> OpenedPainter,
            )
        {
            return None;
        }
        let device = std::env::var("EXACT_INK_DEVICE").ok();
        let force_warp = exact_ink::device::direct_d3d11(device.as_deref())?;
        Some(exact_ink::device::d3d11::Preparing::start(force_warp).map_err(|e| e.to_string()))
    }

    #[cfg(windows)]
    pub(super) fn finish_direct(preparing: exact_ink::device::d3d11::Preparing) -> OpenedPainter {
        let start = std::time::Instant::now();
        let warp = preparing.force_warp();
        let device = preparing
            .finish()
            .or_else(|e| {
                if warp {
                    Err(e)
                } else {
                    exact_ink::device::d3d11::D3d11::open(true)
                }
            })
            .map_err(|e| e.to_string())?;
        let ink =
            crate::ink::Ink::from_device(exact_ink::device::Selected::D3d11(Box::new(device)));
        if std::env::var_os("EXACT_AGENT_WINDOW_FRAMES").is_some() {
            eprintln!(
                "exact-ink: startup device {:.3} ms, pipelines {:.3} ms, join {:.3} ms",
                ink.info().device_ms,
                ink.info().shaders_ms,
                start.elapsed().as_secs_f64() * 1000.
            );
        }
        Ok(ink_backend(ink))
    }

    pub(crate) fn selected(choice: PainterChoice) -> Self {
        Self {
            choice,
            open: open_backend,
        }
    }

    pub(crate) fn from_env(manifest: PainterChoice) -> Self {
        Self::selected(PainterChoice::from_env(manifest))
    }

    #[cfg(target_os = "android")]
    pub(crate) fn canvas(manifest: PainterChoice) -> Result<Self, String> {
        Self::canvas_selected(PainterChoice::from_env(manifest))
    }

    #[cfg(any(target_os = "android", test))]
    fn canvas_selected(choice: PainterChoice) -> Result<Self, String> {
        if choice == PainterChoice::Ink {
            return Err(ANDROID_INK_UNAVAILABLE.into());
        }
        Ok(Self {
            choice: PainterChoice::Canvas,
            open: open_canvas,
        })
    }

    pub(super) fn open(self) -> OpenedPainter {
        (self.open)(self.choice)
    }
}

#[cfg(any(target_os = "android", test))]
fn open_canvas(_: PainterChoice) -> OpenedPainter {
    Ok((
        Box::new(crate::canvas::Recorder::new()),
        PainterInfo {
            name: "canvas",
            ..cpu_info()
        },
    ))
}

pub(super) fn open_backend(
    choice: PainterChoice,
) -> Result<(Box<dyn Backend>, PainterInfo), String> {
    let cpu = || (Box::new(Raster::new()) as Box<dyn Backend>, cpu_info());
    match choice {
        PainterChoice::Cpu => Ok(cpu()),
        #[cfg(target_os = "android")]
        PainterChoice::Ink => Err(ANDROID_INK_UNAVAILABLE.into()),
        #[cfg(not(target_os = "android"))]
        PainterChoice::Ink => {
            let ink = crate::ink::Ink::new()?;
            Ok(ink_backend(ink))
        }
        PainterChoice::Custom => {
            let (name, make) = CUSTOM
                .get()
                .ok_or("EXACT_PAINTER=custom: no painter registered")?;
            Ok((make()?, PainterInfo { name, ..cpu_info() }))
        }
        #[cfg(any(target_os = "android", test))]
        PainterChoice::Canvas => open_canvas(choice),
        PainterChoice::Gpu | PainterChoice::Auto => match Gpu::new() {
            Ok(g) => {
                let info = PainterInfo {
                    name: "gpu",
                    adapter: Some(format!("{} ({})", g.adapter, g.api)),
                    api: Some(g.api.clone()),
                    device_ms: g.device_ms,
                    shaders_ms: g.shaders_ms,
                    cached: g.cached,
                };
                Ok((Box::new(g), info))
            }
            Err(e) if choice == PainterChoice::Auto => {
                // A note, not an error (the smoke reads stderr for errors).
                eprintln!("painting on the CPU: no GPU ({e})");
                Ok(cpu())
            }
            Err(e) => Err(format!("no GPU: {e}")),
        },
    }
}

#[cfg(not(target_os = "android"))]
fn ink_backend(ink: crate::ink::Ink) -> (Box<dyn Backend>, PainterInfo) {
    let device = ink.info();
    let info = PainterInfo {
        name: "ink",
        adapter: Some(format!("{} ({})", device.adapter, device.api)),
        api: Some(device.api.clone()),
        device_ms: device.device_ms,
        shaders_ms: device.shaders_ms,
        cached: device.cached,
    };
    (Box::new(ink), info)
}

impl<D: DataSource> Presenter<D> {
    /// Attach S1's benchmark carrier. Shipping Apple apps still use native views.
    #[cfg(target_os = "macos")]
    pub fn attach_apple_layer(
        &mut self,
        layer: exact_ink::device::apple::Layer,
    ) -> Result<(), String> {
        if let Some(boot) = &mut self.surfaces.apple_boot {
            self.surfaces.apple_layer = Some(layer.clone());
            boot.layer = Some(layer.clone());
            if let Some(preparing) = &mut boot.preparing {
                preparing.attach(layer);
                return Ok(());
            }
        }
        let info = self.brush.attach_apple_layer(layer)?;
        self.painter.adapter = Some(format!("{} ({})", info.adapter, info.api));
        self.painter.api = Some(info.api);
        self.painter.shaders_ms = info.shaders_ms;
        self.painter.cached = info.cached;
        Ok(())
    }

    /// Install the archive-only device or keep the CPU until its worker is ready.
    #[cfg(target_os = "macos")]
    pub(crate) fn install_apple_painter(
        &mut self,
        preparing: exact_ink::device::apple::Startup,
    ) -> Result<(), String> {
        self.surfaces.apple_boot = Some(crate::surfaces::apple::Boot::new(preparing));
        self.painter.adapter = Some("CPU startup; Metal pipelines pending".into());
        self.poll_apple_painter()?;
        self.choice = PainterChoice::Ink;
        Ok(())
    }

    /// Whether a worker wake needs to poll Apple's startup or CPU presentation.
    #[cfg(target_os = "macos")]
    pub fn apple_painter_starting(&self) -> bool {
        self.surfaces
            .apple_boot
            .as_ref()
            .is_some_and(|b| b.preparing.is_some() || b.recovering)
    }

    /// Device recreation has its own deadline, independent of display frames.
    #[cfg(target_os = "macos")]
    pub fn apple_painter_retry_deadline(&self) -> Option<std::time::Instant> {
        self.surfaces.apple_boot.as_ref().and_then(|b| b.deadline())
    }

    /// Keep damage without rasterizing while no presentation device exists.
    #[cfg(target_os = "macos")]
    pub fn apple_painter_can_draw(&self) -> bool {
        self.apple_painter_retry_deadline().is_none()
    }

    /// Poll completion without creating damage while pipelines compile.
    #[cfg(target_os = "macos")]
    pub fn poll_apple_painter(&mut self) -> Result<(), String> {
        let Some(boot) = &mut self.surfaces.apple_boot else {
            return Ok(());
        };
        let refusal = boot.last_refusal.clone();
        let waiting_for_device = boot.deadline().is_some();
        let device = boot.poll(std::time::Instant::now());
        if boot.last_refusal.is_some() && self.painter.name == "cpu" {
            self.painter.adapter = Some(
                if boot.recovering {
                    "CPU startup; Metal device recovery pending"
                } else {
                    "CPU startup; Metal pipelines failed"
                }
                .into(),
            );
        }
        if boot.last_refusal != refusal {
            self.host.log(format!(
                "Metal startup: {}; keeping the CPU painter",
                boot.last_refusal.as_deref().unwrap_or("device unavailable")
            ));
        }
        if waiting_for_device || boot.deadline().is_some() {
            self.repaint();
        }
        let Some(device) = device else {
            return Ok(());
        };
        let (backend, info) = ink_backend(crate::ink::Ink::from_device(device));
        self.brush.replace_backend(backend);
        self.painter = info;
        self.repaint();
        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn apple_frame_for(
        &mut self,
        target: crate::paint::FrameTarget,
    ) -> Result<Option<Arc<Pixmap>>, String> {
        self.poll_apple_painter()?;
        if !self.apple_painter_can_draw() {
            self.last_frame_succeeded = false;
            return Ok(None);
        }
        let pixels = self.frame_for(target);
        if let Some(boot) = &mut self.surfaces.apple_boot {
            if let Some(preparing) = &mut boot.preparing {
                let frame = pixels.as_ref().ok_or("CPU startup frame missing")?;
                if matches!(target, crate::paint::FrameTarget::Window) {
                    let accepted = match preparing.present_cpu(frame.clone(), false) {
                        Ok(accepted) => accepted,
                        Err(error) => {
                            boot.refused(error, std::time::Instant::now());
                            false
                        }
                    };
                    if !accepted {
                        self.last_frame_succeeded = false;
                        self.repaint();
                        return Ok(pixels);
                    }
                }
                if boot.preparing.as_ref().is_some_and(|p| p.ready()) {
                    self.repaint();
                }
            }
        }
        if !self.last_frame_succeeded() {
            return Err("Mac painter frame failed".into());
        }
        if matches!(target, crate::paint::FrameTarget::Pixels) {
            if let Some(boot) = &mut self.surfaces.apple_boot {
                boot.presented = Some(self.painter.name);
            }
        }
        Ok(pixels)
    }

    /// Paint before showing the window, without acquiring a drawable or reading back Ink.
    #[cfg(target_os = "macos")]
    pub fn prepare_apple_window_frame(&mut self) -> Result<Option<PreparedAppleFrame>, String> {
        let pixels = self.apple_frame_for(crate::paint::FrameTarget::PreparedWindow)?;
        Ok(self
            .last_frame_succeeded
            .then_some(PreparedAppleFrame(pixels)))
    }

    /// Present in the show call's transaction. Do not poll or repaint between the pair.
    #[cfg(target_os = "macos")]
    pub fn present_prepared_apple_frame(
        &mut self,
        frame: PreparedAppleFrame,
    ) -> Result<(), String> {
        if let Some(pixels) = frame.0 {
            let boot = self
                .surfaces
                .apple_boot
                .as_mut()
                .ok_or("no CPU presentation device")?;
            let preparing = boot
                .preparing
                .as_mut()
                .ok_or("CPU presentation device changed")?;
            let accepted = match preparing.present_cpu(pixels, true) {
                Ok(accepted) => accepted,
                Err(error) => {
                    boot.refused(error, std::time::Instant::now());
                    false
                }
            };
            if !accepted {
                self.last_frame_succeeded = false;
                self.repaint();
            }
            Ok(())
        } else {
            self.brush.present_prepared_apple_frame()
        }
    }

    /// Submit either a CPU startup frame or a complete ink frame to the same layer.
    #[cfg(target_os = "macos")]
    pub fn apple_window_frame(&mut self) -> Result<(), String> {
        self.apple_frame_for(crate::paint::FrameTarget::Window)
            .map(|_| ())
    }
    pub(crate) fn painter_state(&self) -> serde_json::Value {
        #[cfg(windows)]
        let recovery = self.surfaces.direct_window.as_ref();
        #[cfg(windows)]
        let (recovering, refusal) = (
            recovery.is_some_and(|d| d.recovering),
            recovery.and_then(|d| d.last_refusal.as_deref()),
        );
        #[cfg(not(any(windows, target_os = "macos")))]
        let (recovering, refusal) = (false, None::<&str>);
        #[cfg(target_os = "macos")]
        let (recovering, refusal) = (
            self.surfaces
                .apple_boot
                .as_ref()
                .is_some_and(|b| b.recovering),
            self.surfaces
                .apple_boot
                .as_ref()
                .and_then(|b| b.last_refusal.as_deref()),
        );
        let name = Some(self.painter.name);
        #[cfg(target_os = "macos")]
        let name = if let Some(layer) = &self.surfaces.apple_layer {
            layer.presented_painter()
        } else if let Some(boot) = &self.surfaces.apple_boot {
            boot.presented
        } else {
            name
        };
        let device = if name == Some("cpu") && self.painter.name == "ink" {
            Some("CPU startup; first Metal frame pending")
        } else {
            self.painter.adapter.as_deref()
        };
        serde_json::json!({
            "name": name,
            "installed": self.painter.name,
            "device": device,
            "api": if name == Some(self.painter.name) { self.painter.api.as_deref() } else { None },
            "deviceMs": self.painter.device_ms,
            "shadersMs": self.painter.shaders_ms,
            "cached": self.painter.cached,
            "recovering": recovering,
            "lastRefusal": refusal,
            "counters": crate::ink::counter_state()
        })
    }

    /// Install the device prepared before app boot, before any frame is drawn.
    /// @ref LLP 1149 S2.4.
    #[cfg(windows)]
    pub(crate) fn install_prepared_painter(
        &mut self,
        preparing: exact_ink::device::d3d11::Startup,
    ) -> Result<(), String> {
        let (startup, device) = crate::surfaces::window::Startup::wait(preparing)?;
        if let Some(device) = device {
            let ink =
                crate::ink::Ink::from_device(exact_ink::device::Selected::D3d11(Box::new(device)));
            let (backend, info) = ink_backend(ink);
            self.brush.replace_backend(backend);
            self.painter = info;
        } else {
            self.painter.adapter = Some("CPU startup; composition device pending".into());
        }
        self.surfaces.startup_device = Some(startup);
        self.choice = PainterChoice::Ink;
        self.repaint();
        Ok(())
    }

    /// Boot the app under a viewport (points) at a device scale, with its
    /// asset root. The boot error, if any, is reported beside the presenter
    /// (the tree is what booted).
    pub fn boot(
        plan: &[u8],
        data: D,
        viewport: (f32, f32),
        scale: f32,
        assets: PathBuf,
    ) -> Result<(Presenter<D>, Option<String>), HostError> {
        Presenter::boot_with(
            plan,
            data,
            viewport,
            scale,
            assets,
            PainterChoice::from_env(PainterChoice::Auto),
        )
    }

    /// Boot with a chosen painter (`boot` reads `EXACT_PAINTER`).
    pub fn boot_with(
        plan: &[u8],
        data: D,
        viewport: (f32, f32),
        scale: f32,
        assets: PathBuf,
        choice: PainterChoice,
    ) -> Result<(Presenter<D>, Option<String>), HostError> {
        let (mut presenter, error) = Self::boot_with_assets(
            PlanBytes::Copied(plan),
            data,
            viewport,
            scale,
            Assets::embedded(assets),
            PainterBoot::selected(choice),
            None,
            "/",
            None,
        )?;
        presenter.measure();
        Ok((presenter, error))
    }

    /// The carrier transfers a retained window to a directly presenting backend.
    #[cfg(windows)]
    pub fn attach_painter_window(
        &mut self,
        window: exact_ink::device::d3d11::Window,
        size: [u32; 2],
    ) -> Result<bool, String> {
        if let Some(startup) = self.surfaces.startup_device.take() {
            let direct =
                crate::surfaces::window::DirectWindow::attach(self, window, size, startup)?;
            self.surfaces.direct_window = Some(direct);
            return Ok(true);
        }
        let attached = self.brush.attach_window(window.clone(), size)?;
        if attached {
            self.surfaces.direct_window = Some(crate::surfaces::window::DirectWindow::new(window));
        }
        Ok(attached)
    }

    /// A bounded measurement may wait for the last frame's displayed timestamp.
    #[cfg(windows)]
    pub fn displayed_process_ms(&self) -> Result<Option<f64>, String> {
        if let Some((device, _)) = self
            .surfaces
            .direct_window
            .as_ref()
            .and_then(|d| d.fallback.as_ref())
        {
            return device
                .presented_process_ms()
                .map(Some)
                .map_err(|e| e.to_string());
        }
        self.brush.displayed_process_ms()
    }

    /// The pixels, as a PNG at `path`.
    pub fn screenshot(&mut self, path: &str) -> Result<String, String> {
        let frame = self.frame();
        let png = frame.encode_png().map_err(|e| format!("png: {e}"))?;
        std::fs::write(path, png).map_err(|e| format!("write {path}: {e}"))?;
        let mut s = String::from("{\"screenshot\":");
        quote(path, &mut s);
        let _ = write!(
            s,
            ",\"w\":{},\"h\":{}}}",
            num(r2(self.viewport.0)),
            num(r2(self.viewport.1))
        );
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn the_manifest_selects_the_painter_and_the_environment_overrides_it() {
        let manifest = PainterChoice::from_baked(r#"{"painter":"ink"}"#);
        assert_eq!(manifest.with_override(None), PainterChoice::Ink);
        for (value, expected) in [
            ("cpu", PainterChoice::Cpu),
            ("gpu", PainterChoice::Gpu),
            ("ink", PainterChoice::Ink),
            ("auto", PainterChoice::Auto),
            ("custom", PainterChoice::Custom),
            ("canvas", PainterChoice::Canvas),
        ] {
            assert_eq!(manifest.with_override(Some(value)), expected);
        }
        assert_eq!(
            PainterChoice::from_baked("{}"),
            if cfg!(windows) {
                PainterChoice::Cpu
            } else {
                PainterChoice::Auto
            }
        );
        for default in [PainterChoice::Cpu, PainterChoice::Auto] {
            assert_eq!(default.with_override(None), default);
            assert_eq!(default.with_override(Some("ink")), PainterChoice::Ink);
        }
    }

    #[test]
    fn android_names_the_missing_ink_surface_presenter() {
        let manifest = PainterChoice::from_baked(r#"{"painter":"ink"}"#);
        assert!(matches!(
            PainterBoot::canvas_selected(manifest),
            Err(reason) if reason == ANDROID_INK_UNAVAILABLE
        ));
        let overridden = manifest.with_override(Some("canvas"));
        assert_eq!(
            PainterBoot::canvas_selected(overridden)
                .unwrap()
                .open()
                .unwrap()
                .1
                .name,
            "canvas"
        );
    }

    thread_local! {
        static OPENED: Cell<usize> = const { Cell::new(0) };
    }

    fn counted(choice: PainterChoice) -> OpenedPainter {
        assert_eq!(choice, PainterChoice::Gpu);
        OPENED.with(|n| n.set(n.get() + 1));
        Ok((
            Box::new(Raster::new()),
            PainterInfo {
                name: "carrier-cpu",
                ..cpu_info()
            },
        ))
    }

    fn refused(choice: PainterChoice) -> OpenedPainter {
        assert_eq!(choice, PainterChoice::Gpu);
        OPENED.with(|n| n.set(n.get() + 1));
        Err("carrier constructor refused".into())
    }

    fn plan() -> Vec<u8> {
        contract::compile(
            "component App\n  view\n    box width=8 height=8 background-color=\"#224466\"\n",
        )
        .expect("fixture compiles")
        .encode()
    }

    fn boot(
        plan: &[u8],
        open: fn(PainterChoice) -> OpenedPainter,
    ) -> Result<(Presenter<()>, Option<String>), HostError> {
        Presenter::boot_selected(
            PlanBytes::Copied(plan),
            (),
            (8., 8.),
            1.,
            PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
            None,
            ("{}", exact_runner::Delivery::default()),
            "/",
            None,
            PainterBoot {
                choice: PainterChoice::Gpu,
                open,
            },
        )
    }

    #[test]
    fn selected_carrier_constructor_paints_without_opening_the_gpu_selector() {
        OPENED.with(|n| n.set(0));
        let (mut presenter, error) = boot(&plan(), counted).expect("direct painter boots");
        assert!(error.is_none(), "{error:?}");
        assert_eq!(presenter.painter.name, "carrier-cpu");
        assert_eq!(presenter.compat, "{}");
        assert_eq!(&presenter.frame().data()[..4], &[0x22, 0x44, 0x66, 255]);
        assert_eq!(OPENED.with(Cell::get), 1);
    }

    #[test]
    fn plan_validation_precedes_a_carrier_constructor_refusal() {
        OPENED.with(|n| n.set(0));
        assert!(matches!(
            boot(b"invalid plan", refused),
            Err(HostError::Plan(_))
        ));
        assert_eq!(OPENED.with(Cell::get), 0);
        assert!(
            matches!(boot(&plan(), refused), Err(HostError::Painter(e)) if e == "carrier constructor refused")
        );
        assert_eq!(OPENED.with(Cell::get), 1);
    }

    /// A display whose clock started 15 s before the boot (a slow start):
    /// the boot's clock begins at that reading, so its `after(10000)` is due
    /// 10 s after the boot, not at the first frame; and the date at the
    /// clock's zero is the display clock's start.
    #[test]
    fn a_boot_on_the_display_clock_starts_where_it_reads() {
        let timer = contract::compile(
            "component App\n  state ticks = 0\n  task tick mount\n    after(10000, step)\n  action step\n    ticks = ticks + 1\n  view\n    box width=8 height=8\n",
        )
        .expect("fixture compiles")
        .encode();
        let started = std::time::Instant::now() - std::time::Duration::from_secs(15);
        let mut config = crate::app::Config::from_env(&timer, "{}");
        config.clock_started = Some(started);
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            * 1000.0;
        let (presenter, error) = crate::app::boot_presenter_with_painter::<()>(
            &mut config,
            (8., 8.),
            PainterBoot {
                choice: PainterChoice::Gpu,
                open: counted,
            },
        )
        .expect("boots");
        assert!(error.is_none(), "{error:?}");
        let runner = presenter.host.runner();
        let due = runner.timer_due_ms().expect("the mount timer is armed");
        let now = started.elapsed().as_secs_f64() * 1000.0;
        assert!(
            (25_000.0..=now + 10_000.0).contains(&due),
            "due {due}, clock now {now}"
        );
        let zero = runner.wall_time().epoch_at_zero;
        assert!(
            (zero - (before - 15_000.0)).abs() < 1_000.0,
            "zero {zero}, started {}",
            before - 15_000.0
        );
        // A driver's launch (the Android reader's --agent too) is the runner's
        // clock from zero whatever the display's reads: its boot arms at 10000.
        let mut config = crate::app::Config::from_env(&timer, "{}");
        (config.clock_started, config.agent) = (Some(started), true);
        let (presenter, _) = crate::app::boot_presenter_with_painter::<()>(
            &mut config,
            (8., 8.),
            PainterBoot {
                choice: PainterChoice::Gpu,
                open: counted,
            },
        )
        .expect("boots");
        assert_eq!(presenter.host.runner().timer_due_ms(), Some(10_000.0));
    }

    #[test]
    fn boot_keeps_the_carrier_constructor_and_only_the_running_generation_identity() {
        for (selected, development, identity) in [
            (plan(), true, Some("A")),
            (b"invalid plan".to_vec(), true, None),
            (b"invalid plan".to_vec(), false, None),
        ] {
            OPENED.with(|n| n.set(0));
            let mut config = crate::app::Config {
                content_region: None,
                launch: "/".into(),
                plan: selected.into(),
                fallback_plan: Some(plan()),
                assets: PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
                scale: 1.,
                size: (8., 8.),
                agent: false,
                smoke: false,
                shot: None,
                dev_plan: None,
                dev_url: development.then(|| "http://127.0.0.1:1/".into()),
                dev_said: None,
                dev_identity: development.then(|| "A".into()),
                card: String::new(),
                vnc: None,
                compat: "{}".into(),
                explicit: true,
                entry: None,
                selected_assets: None,
                module: Ok(None),
                updates: None,
                hatches: None,
                packages: Vec::new(),
                clock_started: None,
            };
            let (mut presenter, error) = crate::app::boot_presenter_with_painter::<()>(
                &mut config,
                (8., 8.),
                PainterBoot {
                    choice: PainterChoice::Gpu,
                    open: counted,
                },
            )
            .expect("the baked plan uses the direct painter");
            assert!(error.is_none(), "{error:?}");
            assert_eq!(presenter.painter.name, "carrier-cpu");
            assert!(!presenter.painted);
            if development {
                let dev = presenter
                    .dev
                    .as_ref()
                    .expect("fallback keeps polling the URL");
                assert_eq!(dev.identity.as_deref(), identity);
                assert_eq!(dev.link.as_ref().unwrap().plan.as_deref(), identity);
            } else {
                assert!(presenter.dev.is_none());
            }
            assert_eq!(&presenter.frame().data()[..4], &[0x22, 0x44, 0x66, 255]);
            assert_eq!(OPENED.with(Cell::get), 1);
        }
    }
}

#[cfg(all(test, target_os = "macos", debug_assertions))]
#[path = "painter/apple_tests.rs"]
mod apple_tests;
