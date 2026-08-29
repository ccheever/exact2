//! The app's entry: the environment read once, then one of three ways to
//! run — the agent API over stdio, a headless frame (a smoke run or a
//! screenshot), or the display.
//!
//! @ref LLP 1015 §5–§6
//!
//! - `EXACT_AGENT=1` — the agent API (LLP 1012) on stdio; no display.
//! - `EXACT_SMOKE=1` — boot, paint one frame, print the phases, exit.
//! - `EXACT_SHOT=<png>` — boot, paint one frame, write it, exit.
//! - `EXACT_PLAN=<file>` — boot that plan instead of the baked one.
//! - `EXACT_DEV_PLAN=<file>` — restart from it whenever it changes, state
//!   carried (the dev loop, LLP 1007 §6; display mode).
//! - `EXACT_ASSETS=<dir>` — the asset root (the current directory otherwise).
//! - `EXACT_SIZE=WxH` — the headless viewport, points (420×860 otherwise).
//! - `EXACT_SCALE=n` — device pixels per point (1 otherwise).
//! - `EXACT_DRM=<card>` — the KMS device (`/dev/dri/card0` otherwise).
//! - `EXACT_FONTS=<dir>` — a directory of fonts to add to the system's.

use crate::presenter::Presenter;
use exact_runner::DataSource;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// What the environment asked for.
pub struct Config {
    /// The plan to boot.
    pub plan: Vec<u8>,
    /// The asset root.
    pub assets: PathBuf,
    /// Device pixels per point.
    pub scale: f32,
    /// The headless viewport, points.
    pub size: (f32, f32),
    /// The agent API on stdio.
    pub agent: bool,
    /// One frame, the phases, exit.
    pub smoke: bool,
    /// One frame to a PNG.
    pub shot: Option<String>,
    /// The dev loop's plan file.
    pub dev_plan: Option<PathBuf>,
    /// The KMS device.
    pub card: String,
}

impl Config {
    /// Read the environment; `baked` is the plan compiled into the binary.
    pub fn from_env(baked: &[u8]) -> Config {
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let plan = env("EXACT_PLAN")
            .and_then(|p| match std::fs::read(&p) {
                Ok(b) => Some(b),
                Err(e) => {
                    eprintln!("exact: EXACT_PLAN {p}: {e}; booting the baked plan");
                    None
                }
            })
            .unwrap_or_else(|| baked.to_vec());
        let size = env("EXACT_SIZE")
            .and_then(|s| {
                let (w, h) = s.split_once('x')?;
                Some((w.parse().ok()?, h.parse().ok()?))
            })
            .unwrap_or((420.0, 860.0));
        Config {
            plan,
            assets: env("EXACT_ASSETS")
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
            scale: env("EXACT_SCALE")
                .and_then(|s| s.parse().ok())
                .filter(|s: &f32| *s > 0.0)
                .unwrap_or(1.0),
            size,
            agent: env("EXACT_AGENT").as_deref() == Some("1"),
            smoke: env("EXACT_SMOKE").as_deref() == Some("1"),
            shot: env("EXACT_SHOT"),
            dev_plan: env("EXACT_DEV_PLAN").map(PathBuf::from),
            card: env("EXACT_DRM").unwrap_or_else(|| "/dev/dri/card0".to_string()),
        }
    }

    /// Whether to run without a display.
    pub fn headless(&self) -> bool {
        self.agent
            || self.smoke
            || self.shot.is_some()
            || std::env::var("EXACT_DISPLAY").as_deref() == Ok("headless")
            || !cfg!(target_os = "linux")
    }
}

/// Run the app: the process's exit code.
pub fn run<D: DataSource + Default>(baked: &[u8]) -> i32 {
    let started = Instant::now();
    let config = Config::from_env(baked);
    if config.headless() {
        return headless::<D>(&config, started);
    }
    #[cfg(target_os = "linux")]
    {
        crate::display::run::<D>(&config, started)
    }
    #[cfg(not(target_os = "linux"))]
    {
        1
    }
}

fn headless<D: DataSource + Default>(config: &Config, started: Instant) -> i32 {
    let t_boot = Instant::now();
    let (mut p, error) = match Presenter::boot(
        &config.plan,
        D::default(),
        config.size,
        config.scale,
        config.assets.clone(),
    ) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("exact: boot: {e}");
            return 1;
        }
    };
    let runner_ms = t_boot.elapsed().as_secs_f64() * 1000.0;
    // Local files decode in a moment; a first frame with the pictures in
    // it is what a smoke, a screenshot, and an agent's first `layout` want.
    p.wait_images(Duration::from_millis(500));
    let boot_ms = started.elapsed().as_secs_f64() * 1000.0;
    if config.agent {
        return crate::agent::serve(&mut p, boot_ms, error.as_deref());
    }
    let t_paint = Instant::now();
    let frame = p.frame();
    let paint_ms = t_paint.elapsed().as_secs_f64() * 1000.0;
    if let Some(path) = &config.shot {
        match frame
            .encode_png()
            .map_err(|e| e.to_string())
            .and_then(|png| std::fs::write(path, png).map_err(|e| e.to_string()))
        {
            Ok(()) => println!("wrote {path} ({}x{})", frame.width(), frame.height()),
            Err(e) => {
                eprintln!("exact: screenshot {path}: {e}");
                return 1;
            }
        }
    }
    if config.smoke {
        let root = p
            .host()
            .roots()
            .first()
            .and_then(|r| p.host().kernel().node(*r))
            .map(|n| (n.frame.width as i64, n.frame.height as i64))
            .unwrap_or((0, 0));
        println!(
            "boot {boot_ms:.1} ms; {} nodes; root {}x{}; error {}",
            p.node_count(),
            root.0,
            root.1,
            error.as_deref().unwrap_or("none")
        );
        let (measures, hits, shaping, faces) = {
            let t = p.text().borrow();
            (
                t.measures,
                t.hits,
                t.shaping.as_secs_f64() * 1000.0,
                t.face_count(),
            )
        };
        println!(
            "phases: runner+layout {runner_ms:.1} ms of which {measures} text measurements ({hits} cached) {shaping:.1} ms shaping ({faces} font faces); paint {paint_ms:.1} ms at {}x{}",
            frame.width(),
            frame.height()
        );
        let loaded: Vec<String> = p
            .images()
            .loaded
            .iter()
            .map(|(s, (w, h))| format!("{s} {w}x{h}"))
            .collect();
        println!(
            "images: {}",
            if loaded.is_empty() {
                "none".to_string()
            } else {
                loaded.join("; ")
            }
        );
        println!("smoke ok");
    }
    0
}
