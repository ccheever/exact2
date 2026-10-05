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
    /// tiny-skia.
    Cpu,
    /// Recorded for Android's Canvas (`canvas.rs`).
    #[cfg(target_os = "android")]
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
    /// `EXACT_PAINTER`: `gpu`, `cpu`, or unset (auto).
    pub fn from_env() -> PainterChoice {
        match std::env::var("EXACT_PAINTER").as_deref() {
            Ok("gpu") => PainterChoice::Gpu,
            Ok("cpu") => PainterChoice::Cpu,
            Ok("custom") => PainterChoice::Custom,
            #[cfg(target_os = "android")]
            Ok("canvas") => PainterChoice::Canvas,
            _ => PainterChoice::Auto,
        }
    }
}

/// What the painter is, for the smoke's report.
#[derive(Debug, Clone)]
pub struct PainterInfo {
    /// `"gpu"` or `"cpu"`.
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

pub(super) fn open_backend(
    choice: PainterChoice,
) -> Result<(Box<dyn Backend>, PainterInfo), String> {
    let cpu = || (Box::new(Raster::new()) as Box<dyn Backend>, cpu_info());
    match choice {
        PainterChoice::Cpu => Ok(cpu()),
        PainterChoice::Custom => {
            let (name, make) = CUSTOM
                .get()
                .ok_or("EXACT_PAINTER=custom: no painter registered")?;
            Ok((make()?, PainterInfo { name, ..cpu_info() }))
        }
        #[cfg(target_os = "android")]
        PainterChoice::Canvas => Ok((
            Box::new(crate::canvas::Recorder::new()) as Box<dyn Backend>,
            PainterInfo {
                name: "canvas",
                ..cpu_info()
            },
        )),
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
