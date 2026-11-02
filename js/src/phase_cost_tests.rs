//! Manual measurement of the production lean VM and trusted prelude, before
//! application bytecode. Compiled inputs are produced by the ordinary bake.

use super::*;
use std::time::Duration;

const WARMUP: usize = 20;
const SAMPLES: usize = 200;

fn sample() -> [Duration; 6] {
    // The module's ordinary host doors stay alive until after the runtime.
    let mut module = Module::new(Vec::new(), "phase-cost", "");
    let started = Instant::now();
    let session = storage::Session::open("", storage::WAIT, 0, None).expect("storage context");
    let context_ready = Instant::now();
    let mut engine = Engine::new(
        module.max_heap,
        host_door,
        crypto::bytes_door,
        &mut *module.host as *mut HostState as *mut c_void,
        &session.context,
    )
    .expect("production lean runtime and Ibex binding install");
    let installed = Instant::now();
    engine
        .load_static(PRELUDE)
        .expect("production trusted prelude");
    engine
        .call("__exact_main_thread", ["", "", ""])
        .expect("production main-thread policy");
    let prelude_ready = Instant::now();
    engine
        .harden()
        .expect("capture and harden production realm");
    let hardened = Instant::now();
    drop(engine);
    drop(session);
    let destroyed = Instant::now();
    [
        context_ready.duration_since(started),
        installed.duration_since(context_ready),
        prelude_ready.duration_since(installed),
        hardened.duration_since(prelude_ready),
        hardened.duration_since(started),
        destroyed.duration_since(hardened),
    ]
}

fn distribution(label: &str, values: impl Iterator<Item = Duration>) {
    let mut values: Vec<_> = values.collect();
    values.sort_unstable();
    let percentile = |numerator: usize| {
        values[((values.len() - 1) * numerator).div_ceil(100)].as_secs_f64() * 1_000_000.0
    };
    eprintln!(
        "{label:>28}: min {:.1} us, p25 {:.1} us, median {:.1} us, p75 {:.1} us, p95 {:.1} us, max {:.1} us",
        percentile(0), percentile(25), percentile(50), percentile(75), percentile(95), percentile(100)
    );
}

fn load() -> String {
    std::process::Command::new("uptime")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "host load unavailable".into())
}

#[test]
#[ignore = "manual production startup diagnostic; run alone on an idle host for performance qualification"]
fn production_lean_bootstrap_phase_costs() {
    assert_eq!(
        hermes_lean_sys::LINKED_ENGINE_DIGEST,
        hermes_lean_sys::LEAN_ENGINE_DIGEST,
        "the diagnostic must link the actual lean VM"
    );
    assert!(hermes_lean_sys::LEAN_ENGINE_DIGEST.is_some());
    eprintln!(
        "host: {} {}; lean engine: {}; HBC: {}; prelude: {} bytes",
        std::env::consts::OS,
        std::env::consts::ARCH,
        hermes_lean_sys::LEAN_ENGINE_DIGEST.unwrap(),
        BYTECODE_VERSION,
        PRELUDE.len()
    );
    eprintln!("load before: {}", load());
    eprintln!(
        "Production Session, Engine, PRELUDE, main-thread policy and guarded harden; no application module, storage roots, native module or watchdog. Runtime and binding install share the production entrance; capture and freeze share the production harden entrance. All bytecode compiled at build time."
    );
    for _ in 0..WARMUP {
        let _ = sample();
    }
    let samples: Vec<_> = (0..SAMPLES).map(|_| sample()).collect();
    for (index, label) in [
        "storage Context",
        "runtime + binding install",
        "production trusted prelude",
        "capture + guarded harden",
        "startup through harden",
        "runtime + Context teardown",
    ]
    .into_iter()
    .enumerate()
    {
        distribution(label, samples.iter().map(|sample| sample[index]));
    }
    eprintln!(
        "{SAMPLES} samples after {WARMUP} discarded warmups; load after: {}",
        load()
    );
}
