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
//! - Either, as an `http(s)://` URL — the app URL: the envelope resolved
//!   and the plan fetched, verified, and booted once per run (LLP 1023
//!   Stage 1; `fetch.rs`). A headless run is per-invocation, so an edit is
//!   the next run; the display loop's live half is owed (QUEUE).
//! - `EXACT_ASSETS=<dir>` — the asset root (the current directory otherwise).
//! - `EXACT_SIZE=WxH` — the headless viewport, points (420×860 otherwise).
//! - `EXACT_SCALE=n` — device pixels per point (1 otherwise).
//! - `EXACT_PAINTER=gpu|cpu` — the painter (the GPU when there is one otherwise).
//! - `EXACT_CACHE=<dir>` — where the GPU's pipeline cache lives (`~/.cache/exact`).
//! - `EXACT_DRM=<card>` — the KMS device (`/dev/dri/card0` otherwise).
//! - `EXACT_VNC=1|<addr:port>` — serve the screen over VNC, the client's
//!   pointer and keys as input (display mode; `1` is `0.0.0.0:5900`).
//! - `EXACT_FONTS=<dir>` — a directory of fonts to add to the system's.
//! - `EXACT_FONT=<family>` — what `sans-serif` means (fontconfig's answer otherwise).
//! - `EXACT_UPDATE_ORIGIN=<url>` — the update store checks this origin
//!   instead of the manifest's (dev apparatus: a drive against a static
//!   directory; the baked keys still bind). `EXACT_UPDATE_DIR=<dir>` — the
//!   store's directory instead of `$XDG_DATA_HOME/exact/<app id>/update`;
//!   under `EXACT_AGENT=1` a fresh temporary one (`update.rs`).
//!
//! With neither `EXACT_PLAN` nor `EXACT_DEV_PLAN`, the boot is the update
//! store's selection (LLP 1026 D9): the selected entry's plan, else the
//! baked one; an entry refused at boot boots the baked plan in the same run.

use crate::presenter::Presenter;
use crate::update::Updates;
use exact_runner::DataSource;
use exact_update::{AssetSet, Generation};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn has_explicit_locator(plan: Option<&str>, dev_plan: Option<&str>) -> bool {
    plan.is_some() || dev_plan.is_some()
}

/// What the environment asked for.
pub struct Config {
    /// The plan to boot.
    pub plan: Vec<u8>,
    /// The binary's plan, only when `plan` came from a URL. A hash-valid
    /// network payload can still fail the format/schema/app gates at boot.
    pub(crate) fallback_plan: Option<Vec<u8>>,
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
    /// Serve the screen over VNC at this address (`1` is `0.0.0.0:5900`).
    pub vnc: Option<String>,
    /// The archive's `compat.json` (LLP 1030 D3a) as the binary carries it:
    /// what the `delivery` resource and `state.delivery` answer from.
    pub compat: String,
    /// Whether the environment named the plan (a URL, `EXACT_PLAN`): the
    /// update store's selection then stands aside.
    pub(crate) explicit: bool,
    /// The update store's entry whose plan `plan` is, when one is selected.
    pub(crate) entry: Option<String>,
    /// The immutable update generation whose plan and asset roster are
    /// pinned to this launch.
    pub(crate) generation: Option<Generation>,
    /// The selected generation's complete, lazily verified asset roster.
    pub(crate) selected_assets: Option<AssetSet>,
    /// The update store, opened before the boot it selects (`run`); the
    /// presenter takes it at boot.
    pub(crate) updates: Option<Updates>,
}

impl Config {
    /// Read the environment; `baked` is the plan compiled into the binary
    /// and `compat` its `compat.json` (LLP 1030 D3a).
    pub fn from_env(baked: &[u8], compat: &str) -> Config {
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let exact_plan = env("EXACT_PLAN");
        let exact_dev_plan = env("EXACT_DEV_PLAN");
        let from_url = [exact_plan.as_deref(), exact_dev_plan.as_deref()]
            .into_iter()
            .flatten()
            .find(|v| crate::fetch::is_url(v))
            .and_then(|u| match crate::fetch::fetch_app(u) {
                Ok(b) => {
                    eprintln!("exact: plan ← {u} ({} bytes)", b.len());
                    Some(b)
                }
                Err(e) => {
                    eprintln!("exact url: {e}; booting the baked plan");
                    None
                }
            });
        let named = exact_plan
            .as_deref()
            .filter(|p| !crate::fetch::is_url(p))
            .and_then(|p| match std::fs::read(p) {
                Ok(b) => Some(b),
                Err(e) => {
                    eprintln!("exact: EXACT_PLAN {p}: {e}; booting the baked plan");
                    None
                }
            });
        let dev_plan = exact_dev_plan
            .as_deref()
            .filter(|p| !crate::fetch::is_url(p))
            .map(PathBuf::from);
        let dev = dev_plan.as_deref().and_then(|p| match std::fs::read(p) {
            Ok(b) => Some(b),
            Err(e) => {
                eprintln!(
                    "exact: EXACT_DEV_PLAN {}: {e}; booting the baked plan until it appears",
                    p.display()
                );
                None
            }
        });
        // Network plans and an initial dev file are candidates, never the
        // only bootable copy. A compiler may be killed between its truncate
        // and replace; decode refusal must still show the baked app.
        let fallback_plan = (from_url.is_some() || dev.is_some()).then(|| baked.to_vec());
        // A locator is explicit even when its first read failed. The dev
        // compiler may not have produced the file yet; a persisted release
        // selection must not win in that window or have its boot counted.
        let explicit = has_explicit_locator(exact_plan.as_deref(), exact_dev_plan.as_deref());
        let plan = from_url.or(named).or(dev).unwrap_or_else(|| baked.to_vec());
        let size = env("EXACT_SIZE")
            .and_then(|s| {
                let (w, h) = s.split_once('x')?;
                Some((w.parse().ok()?, h.parse().ok()?))
            })
            .unwrap_or((420.0, 860.0));
        Config {
            plan,
            fallback_plan,
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
            dev_plan,
            card: env("EXACT_DRM").unwrap_or_else(|| "/dev/dri/card0".to_string()),
            vnc: env("EXACT_VNC"),
            compat: compat.to_string(),
            explicit,
            entry: None,
            generation: None,
            selected_assets: None,
            updates: None,
        }
    }

    /// Open the update store (LLP 1026 D9) and, unless the environment named
    /// the plan, take its selection as what boots: the selected entry's plan
    /// with the baked one to fall back on, counting the boot (D11). A
    /// binary that links no store, or a directory that cannot be made, is
    /// one stderr line and the baked plan.
    pub fn select_update(&mut self, baked: &[u8]) {
        match Updates::open(&self.compat, baked, &self.assets) {
            Ok(updates) => self.use_updates(updates, baked),
            Err(e) => eprintln!("exact update: {e}"),
        }
    }

    fn use_updates(&mut self, mut updates: Updates, baked: &[u8]) {
        if !self.explicit {
            if let Some(prepared) = updates.prepare_selected() {
                self.plan = prepared.plan.to_vec();
                self.fallback_plan = Some(baked.to_vec());
                self.entry = prepared.generation.entry.clone();
                self.generation = Some(prepared.generation);
                self.selected_assets = Some(prepared.assets);
            }
            updates.boot_started();
        }
        self.updates = Some(updates);
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

/// Boot the selected plan, falling back only when it was fetched from the
/// app URL or is an update entry's. Transport, length, and hash refusals
/// already take this path in `Config::from_env`; decode and runner refusals
/// belong to the same gate — for an entry, the refusal stands in the store's
/// record and entry zero boots (LLP 1026 D11). The presenter takes the
/// update store here, so its facts are in the first frame.
pub(crate) fn boot_presenter<D: DataSource + Default>(
    config: &mut Config,
    viewport: (f32, f32),
) -> Result<(Presenter<D>, Option<String>), String> {
    let mut updates = config.updates.take();
    let compat = config.compat.clone();
    let delivered = |mut booted: (Presenter<D>, Option<String>), updates: Option<Updates>| {
        // The binary's delivery facts, before anything reads a frame (LLP
        // 1030 D7). The kernel is the display list here, so the commit a
        // re-answered `delivery` resource makes needs nothing from boot.
        let e = booted.0.set_delivery_from_compat(&compat);
        booted.0.set_updates(updates);
        booted.1 = booted.1.or(e);
        booted
    };
    match Presenter::boot_selected(
        &config.plan,
        D::default(),
        viewport,
        config.scale,
        config.assets.clone(),
        config.selected_assets.clone(),
    ) {
        Ok(value) => Ok(delivered(value, updates)),
        Err(fetched_error) => {
            let Some(baked) = config.fallback_plan.as_deref() else {
                return Err(fetched_error.to_string());
            };
            match (&config.entry, &config.generation, updates.as_mut()) {
                (Some(_), Some(generation), Some(u))
                    if matches!(&fetched_error, crate::host::HostError::Asset(_)) =>
                {
                    u.selection_corrupt(generation, &fetched_error.to_string())
                }
                (Some(entry), _, Some(u)) => {
                    u.entry_refused(entry, &fetched_error.to_string())
                }
                _ => eprintln!(
                    "exact url: fetched plan refused at boot: {fetched_error}; booting the baked plan"
                ),
            }
            config.entry = None;
            config.generation = None;
            config.selected_assets = None;
            Presenter::boot_selected(
                baked,
                D::default(),
                viewport,
                config.scale,
                config.assets.clone(),
                None,
            )
            .map(|v| delivered(v, updates))
            .map_err(|baked_error| {
                format!("fetched plan refused: {fetched_error}; baked plan refused: {baked_error}")
            })
        }
    }
}

/// Run the app: the process's exit code. `compat` is the binary's
/// `compat.json` (LLP 1030 D3a), which the `delivery` resource answers from.
pub fn run<D: DataSource + Default>(baked: &[u8], compat: &str) -> i32 {
    let started = Instant::now();
    let mut config = Config::from_env(baked, compat);
    config.select_update(baked);
    if config.headless() {
        return headless::<D>(&mut config, started);
    }
    #[cfg(target_os = "linux")]
    {
        crate::display::run::<D>(&mut config, started)
    }
    #[cfg(not(target_os = "linux"))]
    {
        1
    }
}

fn headless<D: DataSource + Default>(config: &mut Config, started: Instant) -> i32 {
    let t_boot = Instant::now();
    let size = config.size;
    let (mut p, error) = match boot_presenter::<D>(config, size) {
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
    // First pixel, headless: the boot is whole — laid out, its pictures in
    // — before anything reads it (LLP 1026 D11). The check follows when a
    // drive named an origin; a headless run has no user to wait for.
    p.first_pixel();
    if std::env::var_os("EXACT_UPDATE_ORIGIN").is_some() {
        p.check_update();
    }
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
        let (measures, hits, shaping, faces, sans) = {
            let t = p.text().borrow();
            (
                t.measures,
                t.hits,
                t.shaping.as_secs_f64() * 1000.0,
                t.face_count(),
                t.sans.clone(),
            )
        };
        let painter = &p.painter;
        println!(
            "painter: {}{}{}",
            painter.name,
            painter
                .adapter
                .as_deref()
                .map(|a| format!(" — {a}"))
                .unwrap_or_default(),
            if painter.name == "gpu" {
                format!(
                    "; device {:.1} ms, shaders {:.1} ms ({})",
                    painter.device_ms,
                    painter.shaders_ms,
                    if painter.cached {
                        "pipeline cache"
                    } else {
                        "compiled"
                    }
                )
            } else {
                String::new()
            }
        );
        let gpu_frame = p
            .last_frame_ms()
            .map(|(render, readback)| {
                format!(" (render {render:.1} ms, readback {readback:.1} ms)")
            })
            .unwrap_or_default();
        println!(
            "phases: fonts {:.1} ms ({faces} faces, sans-serif {sans:?}); runner+layout {:.1} ms of which {measures} text measurements ({hits} cached) {shaping:.1} ms shaping; paint {paint_ms:.1} ms{gpu_frame} at {}x{}",
            p.fonts_ms,
            runner_ms - p.fonts_ms - painter.device_ms - painter.shaders_ms,
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

#[cfg(test)]
mod tests {
    use super::*;
    use exact_runner::{DataError, Value};
    use exact_update::{sha256_hex, Client, Outcome};
    use std::path::Path;

    const UPDATE_COMPAT: &str = r#"{"id":"fixture00000000","inputs":{"app":"com.exact.fixture","keys":null,"trust":"development","store":{"L":"A"}},"delivery":{"channel":"prod","origin":"https://updates.example"}}"#;

    #[derive(Default)]
    struct Named;

    impl DataSource for Named {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }

        fn app_id(&self) -> &str {
            "com.exact.fixture"
        }
    }

    fn stage(dir: &Path, baked: &[u8], plan: &[u8], assets: &[(&str, Vec<u8>)]) -> Client {
        let mut client = Client::open(dir, dir, UPDATE_COMPAT, baked).unwrap();
        let head_url = client.head_url().unwrap().to_string();
        let plan_url = head_url.replace("exact.json", "app.plan");
        let cards: Vec<_> = assets
            .iter()
            .map(|(name, bytes)| {
                serde_json::json!({
                    "name": name,
                    "url": format!("./{name}"),
                    "sha256": sha256_hex(bytes),
                    "bytes": bytes.len()
                })
            })
            .collect();
        let head = serde_json::to_vec(&serde_json::json!({
            "exact": 1,
            "app": { "id": "com.exact.fixture", "name": "Fixture" },
            "plan": { "url": "./app.plan", "sha256": sha256_hex(plan), "bytes": plan.len() },
            "assets": cards,
            "stream": { "app": "com.exact.fixture", "channel": "prod", "compatibilityId": "fixture00000000", "seq": 1 }
        }))
        .unwrap();
        let mut fetch = |url: &str| {
            if url == head_url {
                Ok(head.clone())
            } else if url == plan_url {
                Ok(plan.to_vec())
            } else if let Some((_, bytes)) = assets
                .iter()
                .find(|(name, _)| url.ends_with(&format!("/{name}")))
            {
                Ok(bytes.clone())
            } else {
                Err(format!("unexpected fetch {url}"))
            }
        };
        assert!(matches!(
            client.check(&mut fetch),
            Outcome::Staged { seq: 1, .. }
        ));
        drop(client);
        Client::open(dir, dir, UPDATE_COMPAT, baked).unwrap()
    }

    fn selected_config(dir: &Path, baked: &[u8], client: Client) -> Config {
        let mut config = Config {
            plan: baked.to_vec(),
            fallback_plan: None,
            assets: dir.to_path_buf(),
            scale: 1.0,
            size: (390.0, 844.0),
            agent: false,
            smoke: false,
            shot: None,
            dev_plan: None,
            card: String::new(),
            vnc: None,
            compat: UPDATE_COMPAT.into(),
            explicit: false,
            entry: None,
            generation: None,
            selected_assets: None,
            updates: None,
        };
        config.use_updates(Updates::from_client(client).unwrap(), baked);
        config
    }

    #[test]
    fn a_fetched_plan_refused_at_boot_falls_back_to_baked() {
        let source = "component App\n  view\n    text \"ok\"\n";
        let mut foreign = contract::compile(source).unwrap();
        foreign.app_id = "com.exact.foreign".into();
        let baked = contract::compile(source).unwrap().encode();
        let mut config = Config {
            plan: foreign.encode(),
            fallback_plan: Some(baked),
            assets: std::env::current_dir().unwrap(),
            scale: 1.0,
            size: (390.0, 844.0),
            agent: false,
            smoke: false,
            shot: None,
            dev_plan: None,
            card: String::new(),
            vnc: None,
            compat: r#"{"id":"fixture00000000","inputs":{"store":{"L":"0"}}}"#.into(),
            explicit: true,
            entry: None,
            generation: None,
            selected_assets: None,
            updates: None,
        };
        let size = config.size;
        let (presenter, error) = boot_presenter::<Named>(&mut config, size).unwrap();
        assert!(error.is_none(), "{error:?}");
        assert_eq!(presenter.node_count(), 1);
        // The binary's facts reached the runner past the fallback (LLP 1030 D7).
        assert_eq!(presenter.host().runner().delivery().store, '0');
    }

    #[test]
    fn a_dev_plan_locator_stands_a_persisted_selection_aside() {
        assert!(has_explicit_locator(None, Some("not-produced-yet.plan")));
        assert!(has_explicit_locator(None, Some("http://dev.example/")));
        assert!(!has_explicit_locator(None, None));
    }

    #[test]
    fn a_partial_initial_dev_plan_falls_back_without_counting_the_store() {
        let source = "component App\n  view\n    text \"baked\"\n";
        let baked = contract::compile(source).unwrap().encode();
        let update = contract::compile("component App\n  view\n    text \"selected\"\n")
            .unwrap()
            .encode();
        let dir =
            std::env::temp_dir().join(format!("exact-linux-dev-precedence-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let compat = UPDATE_COMPAT;
        let mut client = Client::open(&dir, Path::new("."), compat, &baked).unwrap();
        let head_url = client.head_url().unwrap().to_string();
        let plan_url = head_url.replace("exact.json", "app.plan");
        let head = serde_json::to_vec(&serde_json::json!({
            "exact": 1,
            "app": { "id": "com.exact.fixture", "name": "Fixture" },
            "plan": { "url": "./app.plan", "sha256": sha256_hex(&update), "bytes": update.len() },
            "assets": [],
            "stream": { "app": "com.exact.fixture", "channel": "prod", "compatibilityId": "fixture00000000", "seq": 1 }
        }))
        .unwrap();
        let mut fetch = |url: &str| {
            if url == head_url {
                Ok(head.clone())
            } else if url == plan_url {
                Ok(update.clone())
            } else {
                Err(format!("unexpected fetch {url}"))
            }
        };
        assert!(matches!(
            client.check(&mut fetch),
            Outcome::Staged { seq: 1, .. }
        ));
        let record = client.dir().join("record.json");
        let updates = Updates::from_client(client).unwrap();
        let mut config = Config {
            plan: b"EXPL".to_vec(), // the compiler was interrupted mid-write
            fallback_plan: Some(baked.clone()),
            assets: PathBuf::from("."),
            scale: 1.0,
            size: (390.0, 844.0),
            agent: false,
            smoke: false,
            shot: None,
            dev_plan: Some(PathBuf::from("app.plan")),
            card: String::new(),
            vnc: None,
            compat: compat.into(),
            explicit: true,
            entry: None,
            generation: None,
            selected_assets: None,
            updates: None,
        };
        config.use_updates(updates, &baked);
        assert_eq!(config.plan, b"EXPL");
        assert!(config.entry.is_none(), "the selected entry did not win");
        let (presenter, _) = boot_presenter::<Named>(&mut config, (390.0, 844.0)).unwrap();
        assert_eq!(presenter.node_count(), 1, "the baked plan booted");
        let saved = std::fs::read_to_string(record).unwrap();
        assert!(
            saved.contains("\"failures\":0"),
            "the store boot was not counted: {saved}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_selected_image_loads_from_its_verified_generation() {
        let baked = contract::compile("component App\n  view\n    text \"baked\"\n")
            .unwrap()
            .encode();
        let selected =
            contract::compile("component App\n  view\n    image \"assets/mark.png\" width=96\n")
                .unwrap()
                .encode();
        let png = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/caltrain/assets/caltrain.png"
        ))
        .unwrap();
        let dir =
            std::env::temp_dir().join(format!("exact-linux-selected-asset-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let client = stage(&dir, &baked, &selected, &[("assets/mark.png", png)]);
        let mut config = selected_config(&dir, &baked, client);

        let (mut presenter, error) = boot_presenter::<Named>(&mut config, (390.0, 844.0)).unwrap();
        assert!(error.is_none(), "{error:?}");
        presenter.wait_images(Duration::from_secs(2));
        assert_eq!(
            presenter.images().loaded,
            vec![("assets/mark.png".to_string(), (320, 120))]
        );
        assert_eq!(
            presenter.host().runner().delivery().stream,
            "prod/fixture00000000"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_selected_font_loads_from_its_verified_generation() {
        let baked = contract::compile("component App\n  view\n    text \"baked\"\n")
            .unwrap()
            .encode();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/fixtures/fonts");
        let selected = contract::compile_path_source(
            &fixture.join("app.contract"),
            "font \"Body\" = \"assets/DejaVuSans.ttf\"\ncomponent App\n  view\n    text \"selected\" font-family=\"Body\"\n",
        )
        .unwrap()
        .encode();
        let font = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/fixtures/fonts/assets/DejaVuSans.ttf"
        ))
        .unwrap();
        let dir =
            std::env::temp_dir().join(format!("exact-linux-selected-font-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let client = stage(&dir, &baked, &selected, &[("assets/DejaVuSans.ttf", font)]);
        let mut config = selected_config(&dir, &baked, client);

        let (presenter, error) = boot_presenter::<Named>(&mut config, (390.0, 844.0)).unwrap();
        assert!(error.is_none(), "{error:?}");
        let text = presenter.text().clone();
        let mut text = text.borrow_mut();
        let declared = text
            .declared_face_id(8, 400, false)
            .expect("the selected face bytes were registered");
        assert_eq!(text.resolved_face_id(8, 400, false), Some(declared));
        assert_eq!(
            presenter.host().runner().delivery().stream,
            "prod/fixture00000000"
        );
        drop(text);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_absent_selected_asset_tombstones_the_embedded_file() {
        let baked = contract::compile("component App\n  view\n    text \"baked\"\n")
            .unwrap()
            .encode();
        let selected = contract::compile(
            "component App\n  view\n    image \"assets/caltrain.png\" width=96\n",
        )
        .unwrap()
        .encode();
        let dir = std::env::temp_dir().join(format!(
            "exact-linux-selected-tombstone-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let client = stage(&dir, &baked, &selected, &[]);
        let mut config = selected_config(
            Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
            &baked,
            client,
        );

        let (mut presenter, error) = boot_presenter::<Named>(&mut config, (390.0, 844.0)).unwrap();
        assert!(error.is_none(), "{error:?}");
        presenter.wait_images(Duration::from_millis(50));
        assert!(presenter.images().loaded.is_empty());
        assert_eq!(
            presenter.host().runner().delivery().stream,
            "prod/fixture00000000"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_corrupt_selected_asset_falls_back_before_first_pixel() {
        let baked = contract::compile("component App\n  view\n    text \"baked\"\n")
            .unwrap()
            .encode();
        let selected =
            contract::compile("component App\n  view\n    image \"assets/mark.png\" width=96\n")
                .unwrap()
                .encode();
        let dir = std::env::temp_dir().join(format!(
            "exact-linux-corrupt-selected-asset-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let client = stage(
            &dir,
            &baked,
            &selected,
            &[("assets/mark.png", b"signed asset".to_vec())],
        );
        let asset = client
            .selection()
            .assets_dir
            .unwrap()
            .join("assets/mark.png");
        std::fs::write(asset, b"corrupt").unwrap();
        let record = client.dir().join("record.json");
        let mut config = selected_config(&dir, &baked, client);

        let (presenter, error) = boot_presenter::<Named>(&mut config, (390.0, 844.0)).unwrap();
        assert!(error.is_none(), "{error:?}");
        assert_eq!(presenter.node_count(), 1, "entry zero booted");
        assert_eq!(presenter.host().runner().delivery().stream, "embedded");
        assert!(config.generation.is_none());
        assert!(config.selected_assets.is_none());
        let saved = std::fs::read_to_string(record).unwrap();
        assert!(saved.contains("\"selected\":null"), "{saved}");
        assert!(saved.contains("\"failures\":0"), "{saved}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
