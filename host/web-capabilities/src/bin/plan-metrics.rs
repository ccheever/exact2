//! Native speed metrics for any app, from the baked plan its web build wrote,
//! as one JSON line for `scripts/metrics.mjs` (LLP 1054 O4).
//!
//! `plan-metrics <app.plan> [--contract <app.contract>] [--press <testId>[,<testId>]]`
//!
//! The plan carries what the app's data source answered at the bake; nothing
//! answers after boot, so a row that needs a live answer measures without it.
//! The bake itself needs the app's source and is not measured here. The
//! update rows need `--press`: one test id is pressed each time, two
//! alternate (a screen and back). Every timing is a p50.

use exact_kernel::{Kernel, Offer, PropId};
use exact_runner::{Answer, DataError, DataSource, Event, Request, Runner, Store, Value};
use exact_web::Host;
use std::fmt::Write as _;
use std::process::ExitCode;
use std::time::Instant;

/// Answers later, with a request nothing runs: every resource keeps what the
/// bake gave it, pending, as on a device before its first reply.
struct Baked;
impl DataSource for Baked {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        Ok(Answer::Later(Request::get("https://plan-metrics.invalid/")))
    }
}

fn p50(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

fn time<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let t = Instant::now();
    let v = f();
    (v, t.elapsed().as_secs_f64() * 1000.0)
}

fn repeat<T>(n: usize, mut f: impl FnMut() -> T) -> (T, f64) {
    let mut samples = Vec::with_capacity(n);
    let mut last = None;
    for _ in 0..n {
        let (v, ms) = time(&mut f);
        samples.push(ms);
        last = Some(v);
    }
    (last.unwrap(), p50(samples))
}

fn view_of(k: &Kernel, test_id: &str) -> Result<u32, String> {
    let key = *k.find_by_test_id(test_id).first().ok_or_else(|| {
        format!("--press {test_id}: no node has that test id on the first screen")
    })?;
    Ok(k.node_by_key(key).unwrap().id)
}

fn main() -> ExitCode {
    const USAGE: &str =
        "usage: plan-metrics <app.plan> [--contract <app.contract>] [--press <testId>[,<testId>]]";
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let Some(path) = args.first().filter(|a| !a.starts_with('-')) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let encoded = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("{path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let presses: Vec<String> = flag("--press")
        .map(|p| p.split(',').map(str::to_string).collect())
        .unwrap_or_default();
    match measure(&encoded, flag("--contract"), &presses) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("plan-metrics: {e}");
            ExitCode::FAILURE
        }
    }
}

fn measure(encoded: &[u8], contract: Option<String>, presses: &[String]) -> Result<String, String> {
    // A native tool that boots the whole plan links every capability (LLP 1047 D7).
    exact_web::link(exact_web_capabilities::ALL);
    let mut out = String::from("{");
    let mut put = |key: &str, value: String| {
        let _ = write!(
            out,
            "{}\"{key}\":{value}",
            if out.len() > 1 { "," } else { "" }
        );
    };
    if let Some(file) = contract {
        let (plan, ms) = repeat(20, || contract::compile_path(std::path::Path::new(&file)));
        let plan = plan.map_err(|e| format!("{file}:{e}"))?;
        put("compile_ms", format!("{ms:.3}"));
        put("plan_bytes", plan.encode().len().to_string());
    }
    put("baked_bytes", encoded.len().to_string());
    let (plan, decode_ms) = repeat(20, || exact_plan::Plan::decode(encoded));
    let plan = plan.map_err(|e| format!("the plan does not decode: {e:?}"))?;
    put("decode_ms", format!("{decode_ms:.3}"));

    let boot = || {
        Runner::boot(
            plan.clone(),
            Baked,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .map_err(|e| format!("boot: {e:?}"))
    };
    let (runner, boot_ms) = repeat(10, boot);
    let mut runner = runner?;
    put("boot_ms", format!("{boot_ms:.3}"));
    put("nodes", runner.kernel().live_count().to_string());
    // The first layout after boot: every node dirty, the real first-frame cost.
    let mut samples = Vec::new();
    for _ in 0..10 {
        let mut fresh = boot()?;
        let root = fresh.roots()[0];
        let (laid, ms) = time(|| {
            fresh
                .kernel_mut()
                .compute_layout(root, Offer::definite(390.0, 844.0))
        });
        laid.map_err(|e| format!("layout: {e:?}"))?;
        samples.push(ms);
    }
    put("layout_ms", format!("{:.3}", p50(samples)));
    let text_nodes = {
        let k = runner.kernel();
        let (mut n, mut stack) = (0, k.roots());
        while let Some(id) = stack.pop() {
            let node = k.node(id).unwrap();
            n += usize::from(node.props.str(PropId::Text).is_some());
            stack.extend(node.children());
        }
        n
    };
    put("text_nodes", text_nodes.to_string());

    // The press, alternating when two are named, so every press changes something.
    let press = |runner: &mut Runner<Baked>, i: usize| -> Result<f64, String> {
        let id = view_of(runner.kernel(), &presses[i % presses.len()])?;
        let (done, ms) = time(|| runner.dispatch(id, Event::Press));
        done.map_err(|e| format!("press {}: {e:?}", presses[i % presses.len()]))?;
        Ok(ms)
    };
    if !presses.is_empty() {
        for i in 0..presses.len() {
            press(&mut runner, i)?;
        }
        let samples = (0..20)
            .map(|i| press(&mut runner, i))
            .collect::<Result<Vec<_>, _>>()?;
        put("update_ms", format!("{:.3}", p50(samples)));
    }
    // An inherited row changed on the root (LLP 1035.000 D2), alternating two
    // inks so every change is a change: the invalidation's cost and reach.
    {
        use exact_kernel::{Color, ColorValue, Op, StyleId, StyleMask, StyleProps};
        let root = runner.roots()[0];
        let (mut samples, mut touched) = (Vec::new(), 0);
        for i in 0..20 {
            let mut mask = StyleMask::default();
            mask.set(StyleId::TextColor);
            let patch = StyleProps {
                text_color: ColorValue::Fixed(Color(if i % 2 == 0 {
                    0x112233ff
                } else {
                    0x445566ff
                })),
                mask,
                ..StyleProps::default()
            };
            let op = Op::SetStyle {
                id: root,
                patch: Box::new(patch),
            };
            let (receipt, ms) = time(|| runner.kernel_mut().apply(0, 0, &[op]));
            touched = receipt
                .map_err(|e| format!("inherit: {e:?}"))?
                .touched
                .len();
            samples.push(ms);
        }
        put("inherit_ms", format!("{:.3}", p50(samples)));
        put("inherit_touched", touched.to_string());
    }
    let mut now = runner.now_ms();
    let (ticked, tick_ms) = repeat(20, || {
        now += 1000.0;
        runner.advance(now)
    });
    ticked.map_err(|e| format!("tick: {e:?}"))?;
    put("tick_ms", format!("{tick_ms:.3}"));

    // The web host's batches.
    let (booted, web_boot_ms) = repeat(5, || Host::boot(encoded, Baked, Default::default(), "/"));
    let (mut host, first_batch) = booted.map_err(|e| format!("web host boot: {e:?}"))?;
    put("web_boot_ms", format!("{web_boot_ms:.3}"));
    put("web_first_batch_bytes", first_batch.len().to_string());
    if let Some(first) = presses.first() {
        let id = view_of(host.runner().kernel(), first)?;
        let (batch, ms) = time(|| host.dispatch(id, Event::Press));
        put("web_update_ms", format!("{ms:.3}"));
        put("web_update_batch_bytes", batch.len().to_string());
    }
    out.push('}');
    Ok(out)
}
