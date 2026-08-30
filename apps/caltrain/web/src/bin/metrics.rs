//! Native speed metrics for the v1 app, as one JSON line.
//!
//! Run by `scripts/metrics.mjs`; every number is a p50 over repeated runs
//! (or a size), measured on the real pipeline: compile → bake → boot →
//! layout → update → tick, plus the web host's batches.

use exact_kernel::{Kernel, Offer, PropId};
use exact_runner::{Event, Runner};
use exact_web::Host;
use std::time::Instant;

fn p50(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
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

fn view_of(k: &Kernel, test_id: &str) -> u32 {
    let key = k.find_by_test_id(test_id)[0];
    k.node_by_key(key).unwrap().id
}

fn main() {
    let (plan, compile_ms) = repeat(20, || caltrain::compile().unwrap());
    let plan_bytes = plan.encode().len();
    let (baked, bake_ms) = repeat(5, || {
        contract::bake(plan.clone(), caltrain_data::Caltrain).unwrap()
    });
    let baked_bytes = baked.encode().len();
    let encoded = baked.encode();
    let (_, decode_ms) = repeat(20, || exact_plan::Plan::decode(&encoded).unwrap());

    let (runner, boot_ms) = repeat(10, || {
        Runner::boot(
            baked.clone(),
            caltrain_data::Caltrain,
            Kernel::with_monospace(),
        )
        .unwrap()
    });
    let nodes = runner.kernel().live_count();
    let mut runner = runner;
    // The first layout after boot: every node dirty, the real first-frame cost.
    let layout_ms = {
        let mut samples = Vec::new();
        for _ in 0..10 {
            let mut fresh = Runner::boot(
                baked.clone(),
                caltrain_data::Caltrain,
                Kernel::with_monospace(),
            )
            .unwrap();
            let root = fresh.roots()[0];
            let (_, ms) = time(|| {
                fresh
                    .kernel_mut()
                    .compute_layout(root, Offer::definite(390.0, 844.0))
                    .unwrap()
            });
            samples.push(ms);
        }
        p50(samples)
    };

    // An update that swaps a screen (the heaviest common interaction), alternating.
    let change = view_of(runner.kernel(), "change-station");
    runner.dispatch(change, Event::Press).unwrap();
    let back = view_of(runner.kernel(), "stations-back");
    runner.dispatch(back, Event::Press).unwrap();
    let mut samples = Vec::new();
    for i in 0..20 {
        let id = if i % 2 == 0 {
            view_of(runner.kernel(), "change-station")
        } else {
            view_of(runner.kernel(), "stations-back")
        };
        let (_, ms) = time(|| runner.dispatch(id, Event::Press).unwrap());
        samples.push(ms);
    }
    let update_ms = p50(samples);
    let mut now = runner.now_ms();
    let (_, tick_ms) = repeat(20, || {
        now += 1000.0;
        runner.advance(now).unwrap()
    });
    let text_nodes = {
        let k = runner.kernel();
        let mut n = 0;
        let mut stack = k.roots();
        while let Some(id) = stack.pop() {
            let node = k.node(id).unwrap();
            if node.props.str(PropId::Text).is_some() {
                n += 1;
            }
            stack.extend(node.children());
        }
        n
    };

    // The web host's batches.
    let ((host, first_batch), web_boot_ms) =
        repeat(5, || Host::boot(&encoded, caltrain_data::Caltrain).unwrap());
    let mut host = host;
    let change = view_of(host.runner().kernel(), "change-station");
    let (batch, web_update_ms) = time(|| host.dispatch(change, Event::Press));

    println!(
        "{{\"compile_ms\":{compile_ms:.3},\"bake_ms\":{bake_ms:.3},\"decode_ms\":{decode_ms:.3},\"plan_bytes\":{plan_bytes},\"baked_bytes\":{baked_bytes},\"boot_ms\":{boot_ms:.3},\"nodes\":{nodes},\"text_nodes\":{text_nodes},\"layout_ms\":{layout_ms:.3},\"update_ms\":{update_ms:.3},\"tick_ms\":{tick_ms:.3},\"web_boot_ms\":{web_boot_ms:.3},\"web_first_batch_bytes\":{},\"web_update_ms\":{web_update_ms:.3},\"web_update_batch_bytes\":{}}}",
        first_batch.len(),
        batch.len()
    );
}
