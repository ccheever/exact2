//! Explicit S0 measurements; never part of the gate. @ref LLP 1149 D15.
// The workers must compile in debug and refuse only when explicitly run there.
#![allow(clippy::assertions_on_constants)]
use super::super::*;
use std::{path::Path, process::Command, time::Instant};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[cfg(windows)]
mod desktop {
    use super::*;
    pub(super) struct Window(exact_ink::device::d3d11::Window);
    impl Window {
        pub(super) fn new(w: u32, h: u32) -> Self {
            Self(exact_ink::device::d3d11::Window::fixture([w, h]).unwrap())
        }
        pub(super) fn attach<D: DataSource>(&self, p: &mut Presenter<D>, w: u32, h: u32) {
            assert!(p.attach_painter_window(self.0.clone(), [w, h]).unwrap());
        }
        pub(super) fn frame<D: DataSource>(&self, p: &mut Presenter<D>) {
            self.0.dispatch_pending();
            assert!(p.window_frame(self.0.clone()).unwrap().is_none());
            assert!(p.last_frame_succeeded());
        }
    }
}

fn baked(app: &str) -> PathBuf {
    if let Some(path) = std::env::var_os(format!("INK_BAKED_{}", app.to_uppercase())) {
        return path.into();
    }
    let prefix = if app == "excord" {
        "excord-windows-"
    } else {
        "shelf-linux-"
    };
    std::fs::read_dir(root().join("target/debug/build"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(prefix))
        .map(|p| p.join("out"))
        .filter(|p| p.join("app.hbc").exists())
        .max_by_key(|p| p.join("app.plan").metadata().unwrap().modified().unwrap())
        .expect("build app first")
}

fn source(app: &str, out: &Path) -> exact_js::Module {
    let module = std::fs::read_to_string(out.join("module.rs")).unwrap();
    let string = |name: &str| {
        let line = module
            .lines()
            .find(|l| l.starts_with(&format!("pub const {name}:")))
            .unwrap();
        serde_json::from_str::<String>(line.split_once(" = ").unwrap().1.trim_end_matches(';'))
            .unwrap()
    };
    let data = exact_js::Module::new(
        std::fs::read(out.join("app.hbc")).unwrap(),
        string("APP"),
        string("GRANTS"),
    );
    eprintln!("workload {app}: {}", out.display());
    data
}

fn ink_arm(arm: &str) -> bool {
    arm.starts_with('C') || arm == "D"
}

fn select_device(arm: &str) {
    std::env::set_var(
        "EXACT_INK_DEVICE",
        if arm == "D" {
            if cfg!(windows) {
                "d3d11"
            } else {
                "metal"
            }
        } else {
            "wgpu"
        },
    );
}

fn memory_phase(phase: &str) {
    #[cfg(target_os = "macos")]
    if std::env::var_os("INK_MEMORY_TRACE").is_some() {
        eprintln!(
            "INK_PHASE {phase} {:?}",
            exact_ink::device::process_usage().unwrap()
        );
    }
    let _ = phase;
}

fn painter(arm: &str) -> (Box<dyn Backend>, Option<crate::ink::StatisticsReader>) {
    if ink_arm(arm) {
        let ink = crate::ink::Ink::new().unwrap();
        let stats = ink.statistics();
        (Box::new(ink), Some(stats))
    } else {
        (Box::new(crate::gpu::Gpu::new().unwrap()), None)
    }
}

fn row_text<D: DataSource>(
    p: &mut Presenter<D>,
    scroller: Option<u32>,
    height: f32,
) -> Option<(u32, String)> {
    let mut boxes = p.boxes().to_vec();
    boxes.sort_by(|a, b| a.rect.1.total_cmp(&b.rect.1).then(a.id.cmp(&b.id)));
    boxes
        .iter()
        .filter(|b| b.rect.1 >= 0. && b.rect.1 + b.rect.3 < height)
        .filter(|b| {
            b.clip
                .is_none_or(|c| b.rect.1 < c.1 + c.3 && b.rect.1 + b.rect.3 > c.1)
        })
        .filter_map(|b| p.host.kernel().node(b.id))
        .find(|n| {
            let mut ancestor = n.parent;
            let inside = scroller.is_some_and(|scroller| {
                while let Some(id) = ancestor {
                    if id == scroller {
                        return true;
                    }
                    ancestor = p.host.kernel().node(id).and_then(|n| n.parent);
                }
                false
            });
            inside
                && n.node_type == NodeType::Text
                && n.props
                    .str(PropId::Text)
                    .is_some_and(|s| s.chars().any(char::is_alphanumeric))
        })
        .map(|n| (n.id, n.props.str(PropId::Text).unwrap().to_owned()))
}

fn measure<D: DataSource>(plan: &[u8], data: D, app: &str, width: f32, height: f32, arm: &str) {
    #[cfg(windows)]
    let window = std::env::var_os("INK_WINDOW").map(|_| {
        assert_eq!(
            arm, "D",
            "the composition worker uses the direct D3D11 device"
        );
        desktop::Window::new(width as u32, height as u32)
    });
    let assets = root().join("apps").join(app);
    let (mut p, error) =
        Presenter::boot_with(plan, data, (width, height), 1., assets, PainterChoice::Cpu).unwrap();
    assert!(error.is_none(), "boot {app}: {error:?}");
    assert!(p.set_time(1_793_174_400_000., -420.).is_none());
    p.frame();
    p.first_pixel();
    let start = Instant::now();
    loop {
        p.pump(p.host.now());
        p.frame();
        p.first_pixel();
        p.land_then();
        let ready = app != "excord"
            || p.host
                .kernel()
                .find_by_test_id("directory-title")
                .first()
                .and_then(|key| p.host.kernel().node_by_key(*key))
                .and_then(|node| node.props.str(PropId::Text).map(str::to_owned))
                .is_some_and(|title| !title.is_empty() && title != "Excord")
                && p.host
                    .kernel()
                    .find_by_test_id("directory-loading")
                    .is_empty();
        if ready && !p.data_activating() && !p.pending() {
            break;
        }
        if start.elapsed().as_secs() >= 20 {
            panic!(
                "settle {app}: {}\n{}",
                exact_runner::agent::state(p.host.runner()),
                exact_runner::agent::logs(p.host.runner(), 0)
            );
        }
        p.clock(p.host.now() + 50.);
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    p.wait_images(std::time::Duration::from_secs(5));
    if app == "messages-stress" {
        for id in ["history-1000", "toggle-windowed"] {
            let k = p.host.kernel();
            let n = k
                .find_by_test_id(id)
                .first()
                .and_then(|k0| k.node_by_key(*k0))
                .expect("Messages Stress control");
            let id = n.id;
            p.tap(id).unwrap();
            p.clock(p.host.now() + 20_000.);
        }
    }
    memory_phase("before device");
    let (backend, stats) = painter(arm);
    memory_phase("after device");
    p.reader_replaced(backend);
    p.painter.name = if ink_arm(arm) { "ink" } else { "gpu" };
    #[cfg(windows)]
    if let Some(window) = &window {
        window.attach(&mut p, width as u32, height as u32);
    }
    p.frame();
    memory_phase("first frame complete");
    let boxes = p.boxes().to_vec();
    let limits = p.collection_scroll_limits();
    let list_name = match app {
        "shelf" => "library-list",
        "excord" => "channels",
        "messages-stress" => "transcript",
        _ => "departures",
    };
    let k = p.host.kernel();
    let list = k
        .find_by_test_id(list_name)
        .into_iter()
        .chain(k.find_by_id(list_name))
        .find_map(|key| k.node_by_key(key).map(|n| n.id));
    let scroll = list.and_then(|id| p.scroller(id, &limits));
    let text = row_text(&mut p, list, height);
    eprintln!(
        "screen {app} {width}x{height}: list={list:?}, scroll={scroll:?}, text={text:?}, nodes={}, row_groups={:?}",
        boxes.len(), p.brush.row_groups()
    );
    if std::env::var_os("INK_PREVIEW").is_some() {
        p.resize_scaled(width, height, 2.);
        p.screenshot(
            root()
                .join(format!("target/ink-{app}.png"))
                .to_str()
                .unwrap(),
        )
        .unwrap();
        return;
    }
    for mode in ["still", "scroll", "text"] {
        if mode == "scroll" && scroll.is_none_or(|s| s.max.1 < 20.) {
            println!(
                "INK_RESULT {}",
                serde_json::json!({"sampler":sampler(),"app":app,"arm":arm,"size":[width,height],"mode":mode,"unavailable":"first screen has no 20 pt scroll range"})
            );
            continue;
        }
        if let Some(s) = scroll {
            p.scroll_to(s.id, (0., 0.));
        }
        p.frame();
        let text = row_text(&mut p, list, height);
        if mode == "text" && text.is_none() {
            println!(
                "INK_RESULT {}",
                serde_json::json!({"sampler":sampler(),"app":app,"arm":arm,"size":[width,height],"mode":mode,"unavailable":"first screen has no visible row text"})
            );
            continue;
        }
        let mut values = Vec::new();
        let mut max_rss = 0.;
        let mut counters = [0u64; 9];
        memory_phase("before cpu sample");
        let cpu_start = cpu_ms();
        memory_phase("after cpu sample");
        for frame in 0..300 {
            let step = Instant::now();
            if mode == "scroll" {
                let s = scroll.unwrap();
                let range = (s.max.1 / 20.).floor().max(1.) as usize;
                let direction = if (frame / range).is_multiple_of(2) {
                    20.
                } else {
                    -20.
                };
                let before = p.scroll_of(s.id).1;
                p.wheel(s.id, 0., direction).unwrap();
                assert!(
                    (p.scroll_of(s.id).1 - before - direction).abs() < 0.1,
                    "scroll must move 20 pt"
                );
            }
            if mode == "text" {
                let (id, original) = text.as_ref().unwrap();
                let value = format!("{original} {}", if frame % 2 == 0 { "A" } else { "B" });
                let receipt = p
                    .host
                    .runner_mut()
                    .kernel_mut()
                    .apply(
                        0,
                        0,
                        &[exact_kernel::Op::SetProp {
                            id: *id,
                            prop: PropId::Text,
                            value: exact_kernel::PropValue::Str(value.into()),
                        }],
                    )
                    .unwrap();
                p.host.row_dirty.commit(&receipt, p.host.now());
                assert!(p.after_commit().is_none());
            }
            p.repaint();
            let started = Instant::now();
            let presented = {
                #[cfg(windows)]
                if let Some(window) = &window {
                    window.frame(&mut p);
                    true
                } else {
                    assert_eq!(p.frame().width(), width as u32);
                    false
                }
                #[cfg(not(windows))]
                {
                    assert_eq!(p.frame().width(), width as u32);
                    false
                }
            };
            let wall = started.elapsed().as_secs_f64() * 1000.;
            assert_eq!(p.painter.name, if ink_arm(arm) { "ink" } else { "gpu" });
            let (encode, readback) = p.last_frame_ms().unwrap();
            // Window frames report encode/present; headless frames encode/readback.
            assert_eq!(presented, std::env::var_os("INK_WINDOW").is_some());
            values.push([wall, encode, readback, step.elapsed().as_secs_f64() * 1000.]);
            if let Some(stats) = &stats {
                let s = stats.read();
                let atlases = [s.glyphs, s.color_glyphs, s.paths, s.ramps];
                counters[0] += atlases.iter().map(|s| s.misses).sum::<u64>();
                counters[1] += atlases.iter().map(|s| s.hits).sum::<u64>();
                counters[2] += atlases.iter().map(|s| s.uploaded_bytes).sum::<u64>();
                counters[3] = counters[3].max(atlases.iter().map(|s| s.bytes).sum());
                counters[4] = counters[4].max(atlases.iter().map(|s| s.overflow_bytes).sum());
                counters[5] =
                    counters[5].max(atlases.iter().map(|s| s.pinned_textures as u64).sum());
                counters[6] = counters[6].max(s.layers as u64);
                counters[7] = counters[7].max(s.layer_targets as u64);
                counters[8] = counters[8].max(s.layer_bytes);
            }
            if frame % 20 == 19 {
                memory_phase("before rss sample");
                max_rss = f64::max(max_rss, rss_kib());
                memory_phase("after rss sample");
            }
        }
        let cpu = (cpu_ms() - cpu_start) / 300.;
        println!(
            "INK_RESULT {}",
            serde_json::json!({"sampler":sampler(),"app":app,"arm":arm,"size":[width,height],"window":std::env::var_os("INK_WINDOW").is_some(),"mode":mode,"frames":values,"cpu_ms":cpu,"peak_rss_kib":max_rss,"counters":stats.as_ref().map(|_| counters),"scroll_max":scroll.map(|s| s.max.1),"text_node":text.as_ref().map(|t| t.0),"text_initial":text.as_ref().map(|t| &t.1),"row_groups":p.brush.row_groups().len()})
        );
    }
}

fn ps(field: &str) -> String {
    let out = Command::new("ps")
        .args(["-o", field, "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}
fn rss_kib() -> f64 {
    #[cfg(any(target_os = "macos", windows))]
    if std::env::var_os("INK_SAMPLE_PS").is_none() {
        return exact_ink::device::process_usage().unwrap()[1] / 1024.;
    }
    ps("rss=").parse().unwrap()
}
fn cpu_ms() -> f64 {
    #[cfg(any(target_os = "macos", windows))]
    if std::env::var_os("INK_SAMPLE_PS").is_none() {
        return exact_ink::device::process_usage().unwrap()[0];
    }
    ps("time=")
        .split(':')
        .fold(0., |n, part| n * 60. + part.parse::<f64>().unwrap())
        * 1000.
}

#[test]
#[ignore = "S0 optimized measurement worker; selects one arm and workload by environment"]
fn worker() {
    assert!(!cfg!(debug_assertions), "measure an optimized profile");
    let arm = std::env::var("INK_ARM").expect("INK_ARM");
    select_device(&arm);
    let app = std::env::var("INK_APP").expect("INK_APP");
    std::env::set_var("EXACT_AGENT", "1");
    std::env::set_var("EXACT_AGENT_STORAGE", format!("ink-{}", std::process::id()));
    std::env::set_var("EXACT_AGENT_STORAGE_FRESH", "1");
    std::env::set_var("XDG_CACHE_HOME", root().join("target/ink-c4-storage"));
    std::env::set_var("XDG_DATA_HOME", root().join("target/ink-c4-storage"));
    std::env::set_var(
        "EXACT_ROWS",
        if arm == "A" || arm == "C-rows" {
            "0"
        } else {
            "1"
        },
    );
    std::env::set_var("EXACT_FONTS", root().join("scripts/fixtures/fonts/assets"));
    std::env::set_var("EXACT_FONT", "DejaVu Sans");
    let width = std::env::var("INK_WIDTH")
        .unwrap_or("390".into())
        .parse()
        .unwrap();
    let height = std::env::var("INK_HEIGHT")
        .map(|s| s.parse().unwrap())
        .unwrap_or(if width == 390. { 844. } else { 1080. });
    if app == "messages-stress" {
        let plan =
            contract::compile_path(&root().join("apps/messages-stress/app.contract")).unwrap();
        let plan = contract::bake(plan, messages_stress_data::MessagesStress)
            .unwrap()
            .encode();
        measure(
            &plan,
            messages_stress_data::MessagesStress,
            &app,
            width,
            height,
            &arm,
        );
    } else if app == "caltrain" {
        let plan = caltrain::build().unwrap().encode();
        measure(&plan, caltrain_data::Caltrain, &app, width, height, &arm);
    } else {
        let out = baked(&app);
        verify_fixture(&app, &out);
        measure(
            &std::fs::read(out.join("app.plan")).unwrap(),
            source(&app, &out),
            &app,
            width,
            height,
            &arm,
        );
    }
}

#[test]
#[ignore = "S0 boot phase worker"]
fn boot_worker() {
    assert!(!cfg!(debug_assertions), "measure an optimized profile");
    let arm = std::env::var("INK_ARM").unwrap();
    select_device(&arm);
    let started = Instant::now();
    let result = if ink_arm(&arm) {
        let ink = crate::ink::Ink::new().unwrap();
        let boot_ms = started.elapsed().as_secs_f64() * 1000.;
        let i = ink.info();
        serde_json::json!({"sampler":sampler(),"arm":arm,"boot_ms":boot_ms,"device_ms":i.device_ms,"shaders_ms":i.shaders_ms,"cached":i.cached,"adapter":i.adapter,"api":i.api})
    } else {
        let gpu = crate::gpu::Gpu::new().unwrap();
        let boot_ms = started.elapsed().as_secs_f64() * 1000.;
        serde_json::json!({"sampler":sampler(),"arm":arm,"boot_ms":boot_ms,"device_ms":gpu.device_ms,"shaders_ms":gpu.shaders_ms,"cached":gpu.cached,"adapter":gpu.adapter,"api":gpu.api})
    };
    println!("INK_BOOT {result}");
}

#[test]
#[ignore = "S0: five interleaved rounds, optimized profile only"]
fn series() {
    use std::io::Write;
    assert!(!cfg!(debug_assertions), "measure an optimized profile");
    let prefix = "presenter::kept_rows_tests::measurement::";
    let exe = std::env::current_exe().unwrap();
    let shelf = baked("shelf");
    let excord = baked("excord");
    verify_fixture("shelf", &shelf);
    verify_fixture("excord", &excord);
    let direct = std::env::var("INK_ARM").as_deref() == Ok("D");
    let chunk = if direct { "c5" } else { "c4" };
    let arms: &[&str] = if direct {
        &["C", "D"]
    } else {
        &["A", "B", "C", "C-rows"]
    };
    let boot_arms: &[&str] = if direct {
        &["C", "D"]
    } else {
        &["A", "B", "C"]
    };
    let cache = root().join(format!("target/ink-{chunk}-cache"));
    let mut log =
        std::fs::File::create(root().join(format!("target/ink-{chunk}-results.jsonl"))).unwrap();
    for round in 0..5 {
        for app in ["messages-stress", "shelf", "excord"] {
            for width in [390, 1920] {
                for arm in rotated(arms, round) {
                    let output = Command::new("/usr/bin/time")
                        .arg("-l")
                        .arg(&exe)
                        .args([
                            "--ignored",
                            "--exact",
                            &format!("{prefix}worker"),
                            "--nocapture",
                            "--test-threads=1",
                        ])
                        .env("INK_ARM", arm)
                        .env("INK_APP", app)
                        .env("INK_WIDTH", width.to_string())
                        .env("INK_BAKED_SHELF", &shelf)
                        .env("INK_BAKED_EXCORD", &excord)
                        .env("EXACT_CACHE", &cache)
                        .output()
                        .unwrap();
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    assert!(output.status.success(), "{app} {arm}: {stdout}\n{stderr}");
                    let peak = stderr
                        .lines()
                        .find(|l| l.contains("maximum resident set size"))
                        .and_then(|l| l.split_whitespace().next())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap();
                    for line in stdout
                        .lines()
                        .filter_map(|l| l.split_once("INK_RESULT ").map(|(_, r)| r))
                    {
                        let mut result: serde_json::Value = serde_json::from_str(line).unwrap();
                        result["round"] = round.into();
                        result["process_peak_bytes"] = peak.into();
                        result["footprint_peak_bytes"] = stderr
                            .lines()
                            .find(|l| l.contains("peak memory footprint"))
                            .and_then(|l| l.split_whitespace().next())
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap()
                            .into();
                        writeln!(log, "{result}").unwrap();
                    }
                    log.flush().unwrap();
                    eprint!("{stderr}");
                    eprintln!("S0 round {} {app} {width} {arm} complete", round + 1);
                }
            }
        }
    }
    for round in 0..10 {
        for cached in [false, true] {
            for arm in rotated(boot_arms, round) {
                let arm_cache = cache.join(arm);
                if !cached && arm_cache.exists() {
                    std::fs::remove_dir_all(&arm_cache).unwrap();
                }
                let output = Command::new(&exe)
                    .args([
                        "--ignored",
                        "--exact",
                        &format!("{prefix}boot_worker"),
                        "--nocapture",
                        "--test-threads=1",
                    ])
                    .env("INK_ARM", arm)
                    .env("EXACT_CACHE", &arm_cache)
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "boot: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let stdout = String::from_utf8(output.stdout).unwrap();
                let line = stdout
                    .lines()
                    .find_map(|l| l.split_once("INK_BOOT ").map(|(_, r)| r))
                    .unwrap();
                let mut result: serde_json::Value = serde_json::from_str(line).unwrap();
                result["round"] = round.into();
                result["cache_requested"] = cached.into();
                writeln!(log, "{result}").unwrap();
            }
        }
    }
}

fn sampler() -> &'static str {
    if cfg!(any(target_os = "macos", windows)) && std::env::var_os("INK_SAMPLE_PS").is_none() {
        "native process counters"
    } else {
        "ps time/rss (10 ms CPU quantum)"
    }
}

fn rotated<'a>(arms: &'a [&'a str], round: usize) -> Vec<&'a str> {
    (0..arms.len())
        .map(|n| arms[(n + round) % arms.len()])
        .collect()
}

fn verify_fixture(app: &str, out: &Path) {
    let hashes = match app {
        "shelf" => [
            "65b682425c6c28bca40968917f7f9c3dc1885cf80b19fee4f949ca0bdf1b2847",
            "4931b6ca5f3ca72c4f8426687ae40608c399d515a085bc614f977669f3396775",
        ],
        "excord" => [
            "179036d9fb16dc2fca1ece8d571edeca2366274a0e458a3d605eae20eb04a2b6",
            "8370918824a4376654f2d74627ead3213a06b83b625c13dd378c394ace6376ba",
        ],
        _ => panic!("unqualified measurement fixture {app}"),
    };
    for (file, expected) in ["app.plan", "app.hbc"].into_iter().zip(hashes) {
        assert_hash(&std::fs::read(out.join(file)).unwrap(), expected);
    }
}

fn assert_hash(bytes: &[u8], expected: &str) {
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        expected,
        "measurement fixture changed"
    );
}

#[test]
fn series_rotates_each_arm_and_rejects_changed_fixture_bytes() {
    let arms = ["A", "B", "C", "D"];
    assert_eq!(
        (0..4).map(|n| rotated(&arms, n)[0]).collect::<Vec<_>>(),
        arms
    );
    assert_eq!(rotated(&arms, 5), ["B", "C", "D", "A"]);
    let empty = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    assert_hash(b"", empty);
    assert!(std::panic::catch_unwind(|| assert_hash(b"changed", empty)).is_err());
    assert!(!sampler().is_empty());
}
