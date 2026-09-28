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
//!   and the plan plus selected Rust pair fetched, verified, and booted
//!   (`fetch.rs`). `EXACT_DEV_PLAN` URLs continue polling after first pixel;
//!   each verified change prepares and commits a carried-state replacement.
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

use crate::delivery::Store;
use crate::image::AssetResolver;
use crate::presenter::Presenter;
use exact_runner::DataSource;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The first non-flag path or scheme argument. @ref LLP 1038 D8
pub fn launch_location(args: impl IntoIterator<Item = String>) -> String {
    args.into_iter()
        .find(|arg| !arg.starts_with('-'))
        .map_or_else(|| "/".into(), |href| exact_route::location_of(&href))
}

/// `EXACT_SCALE`: device pixels per point, 1 when unset. A scale the
/// painters cannot draw the frame at — not a positive finite number, or one
/// that takes it past their largest side — is refused by name, and the app
/// draws at 1 rather than aborting in its first frame.
fn scale(value: Option<String>, (width, height): (f32, f32)) -> f32 {
    let Some(value) = value else { return 1.0 };
    let largest = crate::raster::MAX_FRAME_SIDE as f32;
    match value.parse::<f32>() {
        Ok(s)
            if s.is_finite()
                && s > 0.0
                && (width * s).round() <= largest
                && (height * s).round() <= largest =>
        {
            s
        }
        _ => {
            eprintln!("exact: EXACT_SCALE={value} refused: a {width}×{height} frame needs a positive scale within {largest} device pixels a side; drawing at scale 1");
            1.0
        }
    }
}

fn has_explicit_locator(plan: Option<&str>, dev_plan: Option<&str>) -> bool {
    plan.is_some() || dev_plan.is_some()
}

/// What the environment asked for.
pub struct Config {
    /// Explicit consumer registration before the first layout; ordinary default
    /// is None. This trial never guesses bindings from test IDs.
    pub content_region: Option<crate::content_region::ContentRegionRegistration>,
    /// The first location-bearing argument, canonicalized by exact-route.
    pub launch: String,
    /// The plan to boot.
    pub plan: Vec<u8>,
    /// The binary's plan, only when `plan` came from a URL. A hash-valid
    /// network payload can still fail the format/schema/app gates at boot.
    pub fallback_plan: Option<Vec<u8>>,
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
    /// A development app URL polled off the presentation thread after first pixel.
    pub dev_url: Option<String>,
    /// The last accepted URL generation, avoiding redundant payload transfers.
    pub dev_identity: Option<String>,
    /// The KMS device.
    pub card: String,
    /// Serve the screen over VNC at this address (`1` is `0.0.0.0:5900`).
    pub vnc: Option<String>,
    /// The archive's `compat.json` (LLP 1030 D3a) as the binary carries it:
    /// what the `delivery` resource and `state.delivery` answer from.
    pub compat: String,
    /// Whether the environment named the plan (a URL, `EXACT_PLAN`): the
    /// update store's selection then stands aside.
    pub explicit: bool,
    /// The update store's entry whose plan `plan` is, when one is selected.
    pub entry: Option<String>,
    /// The selected generation's complete, lazily verified asset roster.
    pub selected_assets: Option<AssetResolver>,
    /// Explicit development logic paired with the selected plan.
    pub module: Result<Option<crate::delivery::Module>, String>,
    /// The update store, opened before the boot it selects (`run`); the
    /// presenter takes it at boot.
    pub updates: Option<Box<dyn Store>>,
}

impl Config {
    /// Read the environment; `baked` is the plan compiled into the binary
    /// and `compat` its `compat.json` (LLP 1030 D3a).
    pub fn from_env(baked: &[u8], compat: &str) -> Config {
        // A production bake never enters agent mode (LLP 1069.007 D2, ruled):
        // the agent's variables are dropped before anything reads them —
        // this config, the zone and seed, storage, the update store.
        if exact_runner::delivery::production(compat) {
            ignore_agent_variables();
        }
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let exact_plan = env("EXACT_PLAN");
        let exact_dev_plan = env("EXACT_DEV_PLAN");
        let from_url = [exact_plan.as_deref(), exact_dev_plan.as_deref()]
            .into_iter()
            .flatten()
            .find(|v| crate::fetch::is_url(v))
            .and_then(|u| match crate::fetch::fetch_changed(u, compat, None) {
                Ok(Some(generation)) => {
                    eprintln!("exact: plan ← {u} ({} bytes)", generation.plan.len());
                    Some(generation)
                }
                Ok(None) => None,
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
        let url_module = from_url
            .as_ref()
            .and_then(|generation| generation.module.clone());
        let dev_identity = from_url
            .as_ref()
            .map(|generation| generation.identity.clone());
        let plan = from_url
            .map(|generation| generation.plan)
            .or(named)
            .or(dev)
            .unwrap_or_else(|| baked.to_vec());
        let size = env("EXACT_SIZE")
            .and_then(|s| {
                let (w, h) = s.split_once('x')?;
                Some((w.parse().ok()?, h.parse().ok()?))
            })
            .unwrap_or((420.0, 860.0));
        Config {
            content_region: None,
            launch: launch_location(std::env::args().skip(1)),
            plan,
            fallback_plan,
            assets: env("EXACT_ASSETS")
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
            scale: scale(env("EXACT_SCALE"), size),
            size,
            agent: env("EXACT_AGENT").as_deref() == Some("1"),
            smoke: env("EXACT_SMOKE").as_deref() == Some("1"),
            shot: env("EXACT_SHOT"),
            dev_plan,
            dev_url: exact_dev_plan
                .as_ref()
                .filter(|url| crate::fetch::is_url(url))
                .cloned(),
            dev_identity,
            card: env("EXACT_DRM").unwrap_or_else(|| "/dev/dri/card0".to_string()),
            vnc: env("EXACT_VNC"),
            compat: compat.to_string(),
            explicit,
            entry: None,
            selected_assets: None,
            module: exact_plan
                .as_deref()
                .or(exact_dev_plan.as_deref())
                .filter(|path| !crate::fetch::is_url(path))
                .map_or(Ok(url_module), |path| {
                    crate::delivery::Module::local(std::path::Path::new(path), compat)
                }),
            updates: None,
        }
    }

    /// Attach an app-supplied delivery adapter and pin its selection unless
    /// a dev locator explicitly selected the plan (LLP 1030 D4).
    pub fn use_updates(&mut self, mut updates: Box<dyn Store>, baked: &[u8]) {
        if !self.explicit {
            if let Some(prepared) = updates.prepare_selected() {
                self.plan = prepared.plan.to_vec();
                self.fallback_plan = Some(baked.to_vec());
                self.entry = prepared.entry;
                self.selected_assets = Some(prepared.assets);
            }
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
pub fn boot_presenter<D: DataSource + Default>(
    config: &mut Config,
    viewport: (f32, f32),
) -> Result<(Presenter<D>, Option<String>), String> {
    exact_runner::delivery::refuse_analysis(&config.compat).map_err(str::to_string)?;
    let mut updates = config.updates.take();
    let compat = config.compat.clone();
    let facts = |updates: &Option<Box<dyn Store>>| {
        let mut delivery = exact_runner::Delivery::default().with_compat(&compat);
        if let Some(updates) = updates {
            updates.status_into(&mut delivery);
        }
        delivery
    };
    let place = crate::zone::launch_place(|key| std::env::var(key).ok())?;
    let agent_time = crate::zone::agent_time(|key| std::env::var(key).ok(), &place.time_zone)?;
    let dev_url = config.dev_url.clone();
    let dev_identity = config.dev_identity.clone();
    let delivered = |mut booted: (Presenter<D>, Option<String>),
                     updates: Option<Box<dyn Store>>| {
        // The accepted runner already has the complete facts. Attaching the
        // adapter must never reintroduce intermediate embedded answers.
        booted.0.set_updates(updates);
        if let Some(e) = booted.0.set_place(&place) {
            eprintln!("exact: {e}");
        }
        // @ref LLP 1027.000.000 — the date, at the clock's zero (now: boot);
        // under the agent, the drive's epoch (D3).
        let (epoch, offset) = agent_time.unwrap_or_else(|| {
            let wall = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0.0, |d| d.as_secs_f64() * 1000.0);
            (
                wall - booted.0.host().now(),
                crate::zone::local_offset_minutes(),
            )
        });
        if let Some(e) = booted.0.set_time(epoch, offset) {
            eprintln!("exact: {e}");
        }
        // @ref LLP 1069.000 D2, D6 — this host has no share sheet; under the
        // agent the drive's substitute answers one, as on every host.
        if std::env::var("EXACT_AGENT").as_deref() == Ok("1") {
            let page = exact_runner::Page {
                can_share: true,
                ..Default::default()
            };
            if let Some(e) = booted.0.set_page(page) {
                eprintln!("exact: {e}");
            }
        }
        booted
            .0
            .set_development(dev_url.clone(), dev_identity.clone());
        booted
    };
    // The selected bytes passed verification. Record the attempt before
    // app construction or the painter can crash; a returned integrity
    // refusal below durably discards the selection and clears its count.
    if !config.explicit && config.entry.is_some() {
        if let Some(updates) = updates.as_mut() {
            updates.boot_started();
        }
    }
    let module = match &config.selected_assets {
        Some(assets) => crate::delivery::Module::resolve(assets),
        None => config.module.clone(),
    };
    let admitted = D::default();
    let source = module
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|module| match module {
            Some(module) => module.replacement(&config.plan, &admitted),
            None => Ok(admitted),
        });
    let booted = source
        .map_err(crate::host::HostError::Asset)
        .and_then(|data| {
            Presenter::boot_selected(
                &config.plan,
                data,
                viewport,
                config.scale,
                config.assets.clone(),
                config.selected_assets.clone(),
                (&compat, facts(&updates)),
                &config.launch,
                config.content_region,
            )
        });
    match booted {
        Ok(mut value) => {
            value.0.set_module(module.unwrap_or(None));
            Ok(delivered(value, updates))
        }
        Err(fetched_error) => {
            let Some(baked) = config.fallback_plan.as_deref() else {
                return Err(fetched_error.to_string());
            };
            match (&config.entry, updates.as_mut()) {
                (Some(_), Some(u))
                    if matches!(&fetched_error, crate::host::HostError::Asset(_)) =>
                {
                    u.selection_corrupt(&fetched_error.to_string())
                }
                (Some(entry), Some(u)) => {
                    u.entry_refused(entry, &fetched_error.to_string())
                }
                _ => eprintln!(
                    "exact url: fetched plan refused at boot: {fetched_error}; booting the baked plan"
                ),
            }
            config.entry = None;
            config.selected_assets = None;
            Presenter::boot_selected(
                baked,
                D::default(),
                viewport,
                config.scale,
                config.assets.clone(),
                None,
                (&compat, facts(&updates)),
                &config.launch,
                config.content_region,
            )
            .map(|v| delivered(v, updates))
            .map_err(|baked_error| {
                format!("fetched plan refused: {fetched_error}; baked plan refused: {baked_error}")
            })
        }
    }
}

/// Run a Contract app without application data, sharing its compiled host.
pub fn run_empty(baked: &[u8], compat: &str) -> i32 {
    run::<()>(baked, compat)
}

/// Run the app: the process's exit code. `compat` is the binary's
/// `compat.json` (LLP 1030 D3a), which the `delivery` resource answers from.
pub fn run<D: DataSource + Default>(baked: &[u8], compat: &str) -> i32 {
    run_registered::<D>(baked, compat, None)
}

/// Launch an explicitly registered consumer using the ordinary environment,
/// receipt and store preflight. No generic worker or authoring schema is added.
pub fn run_with_content_region<D: DataSource + Default>(
    baked: &[u8],
    compat: &str,
    region: crate::content_region::ContentRegionRegistration,
) -> i32 {
    run_registered::<D>(baked, compat, Some(region))
}

fn run_registered<D: DataSource + Default>(
    baked: &[u8],
    compat: &str,
    region: Option<crate::content_region::ContentRegionRegistration>,
) -> i32 {
    if print_baked_receipt(compat) {
        return 0;
    }
    if let Err(why) = exact_runner::delivery::refuse_analysis(compat) {
        eprintln!("exact: {why}");
        return 1;
    }
    if exact_runner::Delivery::default().with_compat(compat).store != '0' {
        eprintln!(
            "exact: the baked store level requires the delivery adapter; regenerate the app entry"
        );
        return 1;
    }
    let started = Instant::now();
    let mut config = Config::from_env(baked, compat);
    config.content_region = region;
    run_config::<D>(&mut config, started)
}

/// Drop `EXACT_AGENT` and every `EXACT_AGENT_*` variable from this process,
/// saying so once on stderr when one was set: a production bake runs as a
/// production launch whatever its environment says (LLP 1069.007 D2).
fn ignore_agent_variables() {
    let named: Vec<_> = std::env::vars_os()
        .filter_map(|(k, _)| k.to_str().map(str::to_owned))
        .filter(|k| k == "EXACT_AGENT" || k.starts_with("EXACT_AGENT_"))
        .collect();
    if named.is_empty() {
        return;
    }
    eprintln!(
        "exact: a production build ignores {} (LLP 1069.007 D2)",
        named.join(", ")
    );
    for k in named {
        std::env::remove_var(k);
    }
}

/// Answer the tooling receipt request before boot or opening an update store.
pub fn print_baked_receipt(compat: &str) -> bool {
    if std::env::args().any(|arg| arg == "--exact-receipt") {
        print!("{compat}");
        true
    } else {
        false
    }
}

/// Run a configured app, optionally composed with a delivery adapter.
pub fn run_config<D: DataSource + Default>(config: &mut Config, started: Instant) -> i32 {
    if let Err(why) = exact_runner::delivery::refuse_analysis(&config.compat) {
        eprintln!("exact: {why}");
        return 1;
    }
    if config.headless() {
        return headless::<D>(config, started);
    }
    #[cfg(target_os = "linux")]
    {
        crate::display::run::<D>(config, started)
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
    let _ = p.frame();
    p.first_pixel();
    p.sync_surfaces();
    p.poll_development(D::default);
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
mod scale_tests {
    use super::scale;

    #[test]
    fn a_scale_no_frame_can_take_is_refused_for_one() {
        assert_eq!(scale(None, (420., 860.)), 1.0);
        assert_eq!(scale(Some("2".into()), (420., 860.)), 2.0);
        assert_eq!(scale(Some("19".into()), (420., 860.)), 19.0);
        for refused in ["1000", "20", "0", "-1", "abc", "NaN", "inf"] {
            assert_eq!(scale(Some(refused.into()), (420., 860.)), 1.0, "{refused}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launch_arguments_share_url_interpretation_with_other_hosts() {
        for (args, expected) in [
            (
                vec!["--agent", "s2b://post/42?q=a b#ignored", "/later"],
                "/post/42?q=a%20b",
            ),
            (vec!["--flag", "https://example.com/post/7"], "/post/7"),
            (vec!["/post/../post/8", "s2b://later"], "/post/8"),
            (vec!["--flag", "post/42", "/later"], "/post/42"),
            (vec!["notes", "1bad:thing"], "/notes"),
            (vec!["--agent", "--url=/post/7"], "/"),
        ] {
            assert_eq!(
                launch_location(args.into_iter().map(str::to_owned)),
                expected
            );
        }
    }
    #[test]
    fn a_core_only_entry_refuses_an_update_capable_compatibility_record() {
        assert_eq!(
            run::<caltrain_data::Caltrain>(b"not read", r#"{"inputs":{"store":{"L":"A"}}}"#),
            1
        );
    }
    #[test]
    fn a_dev_plan_locator_stands_a_persisted_selection_aside() {
        assert!(has_explicit_locator(None, Some("not-produced-yet.plan")));
        assert!(has_explicit_locator(None, Some("http://dev.example/")));
        assert!(!has_explicit_locator(None, None));
    }
}
