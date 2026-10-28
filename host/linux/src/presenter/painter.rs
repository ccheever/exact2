//! Which backend paints: the choice, the registry `EXACT_PAINTER=custom`
//! boots from, and opening it.
use super::*;

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
    /// `EXACT_PAINTER`: `gpu`, `ink`, `cpu`, or unset (auto).
    pub fn from_env() -> PainterChoice {
        match std::env::var("EXACT_PAINTER").as_deref() {
            Ok("gpu") => PainterChoice::Gpu,
            Ok("ink") => PainterChoice::Ink,
            Ok("cpu") => PainterChoice::Cpu,
            Ok("custom") => PainterChoice::Custom,
            #[cfg(any(target_os = "android", test))]
            Ok("canvas") => PainterChoice::Canvas,
            _ => PainterChoice::Auto,
        }
    }
}

/// What the painter is, for the smoke's report.
#[derive(Debug, Clone)]
pub struct PainterInfo {
    /// The selected backend's name.
    pub name: &'static str,
    /// The adapter and API, on the GPU.
    pub adapter: Option<String>,
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
        device_ms: 0.0,
        shaders_ms: 0.0,
        cached: false,
    }
}

type OpenedPainter = Result<(Box<dyn Backend>, PainterInfo), String>;

/// A carrier chooses its constructor without retaining every native backend.
/// The constructor still runs after plan and font validation, on the paint thread.
#[derive(Clone, Copy)]
pub(crate) struct PainterBoot {
    pub(super) choice: PainterChoice,
    open: fn(PainterChoice) -> OpenedPainter,
}

impl PainterBoot {
    pub(super) fn selected(choice: PainterChoice) -> Self {
        Self {
            choice,
            open: open_backend,
        }
    }

    pub(crate) fn from_env() -> Self {
        Self::selected(PainterChoice::from_env())
    }

    #[cfg(target_os = "android")]
    pub(crate) fn canvas() -> Self {
        Self {
            choice: PainterChoice::Canvas,
            open: open_canvas,
        }
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
        PainterChoice::Ink => {
            let ink = crate::ink::Ink::new()?;
            let device = ink.info();
            let info = PainterInfo {
                name: "ink",
                adapter: Some(format!("{} ({})", device.adapter, device.api)),
                device_ms: device.device_ms,
                shaders_ms: device.shaders_ms,
                cached: device.cached,
            };
            Ok((Box::new(ink), info))
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

impl<D: DataSource> Presenter<D> {
    /// Route characters to a platform editor when it owns the field.
    #[cfg(windows)]
    pub fn platform_text(&mut self, text: &str) -> Result<bool, String> {
        let handled = self.brush.platform_text(text)?;
        if handled {
            self.repaint();
        }
        Ok(handled)
    }

    /// The carrier transfers a retained window to a directly presenting backend.
    #[cfg(windows)]
    pub fn attach_painter_window(
        &mut self,
        window: exact_ink::device::d3d11::Window,
        size: [u32; 2],
    ) -> Result<bool, String> {
        self.brush.attach_window(window, size)
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
