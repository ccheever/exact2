//! Native runner and transaction overhead, separate from the on-device JNI,
//! retained Android tree and presentation measurements in the app benchmark.

use android_core_android::{COMPAT, PLAN};
use android_core_data::Core;
use exact_android::session::Session;
use exact_android::wire::{Encoder, HEADER_BYTES};
use std::time::Instant;

fn view(session: &mut Session<Core>, target: &str) -> u32 {
    let request = format!(r#"{{"op":"tree","target":"{target}","shallow":true}}"#);
    let n = session.bridge.input_write(request.as_bytes());
    let length = session.bridge.agent(n);
    let text = std::str::from_utf8(session.bridge.output_bytes(length as usize)).unwrap();
    text.split("\"roots\":[")
        .nth(1)
        .unwrap()
        .split(']')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

fn report(name: &str, mut samples: Vec<f64>, bytes: usize, records: u32) {
    samples.sort_by(f64::total_cmp);
    let p50 = samples[samples.len() / 2];
    let p95 = samples[(samples.len() * 95 / 100).min(samples.len() - 1)];
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    println!(
        r#"{{"scenario":"{name}","samples":{},"mean_us":{mean:.3},"p50_us":{p50:.3},"p95_us":{p95:.3},"wire_bytes":{bytes},"records":{records}}}"#,
        samples.len()
    );
}

fn main() {
    let iterations = std::env::args()
        .nth(1)
        .map_or(1000, |s| s.parse::<usize>().expect("positive sample count"));
    assert!(iterations > 0);
    let mut session = Session::<Core>::default();
    session.bridge.set_compat(COMPAT);
    let length = session
        .bridge
        .boot_selected(PLAN, || Core, session.hooks, 390., 844.);
    session.publish(length);
    let increment = view(&mut session, "increment");
    let toggle = view(&mut session, "toggle-batch");
    let transform = view(&mut session, "toggle-move");
    let mut clock = 1.;
    println!(
        r#"{{"scope":"native runner plus Android wire; reference text measurer; no JVM or Android draw"}}"#
    );
    for (scenario, row_count, target) in [
        ("counter", 1, increment),
        ("batch-100", 100, toggle),
        ("batch-1000", 1000, toggle),
        ("transform-1000", 1000, transform),
    ] {
        let selector = view(&mut session, &format!("rows-{row_count}"));
        let length = session.bridge.dispatch(selector, 0, 0, clock);
        session.publish(length);
        clock += 1.;
        for _ in 0..100 {
            let length = session.bridge.dispatch(target, 0, 0, clock);
            session.publish(length);
            clock += 1.;
        }
        let mut samples = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = Instant::now();
            let length = session.bridge.dispatch(target, 0, 0, clock);
            session.publish(length);
            samples.push(start.elapsed().as_secs_f64() * 1_000_000.);
            clock += 1.;
        }
        let records = u32::from_le_bytes(session.output()[8..12].try_into().unwrap());
        report(scenario, samples, session.output().len(), records);
        if session.bridge.binary_output() {
            println!(
                r#"{{"scenario":"{scenario}-wire-only","scope":"not applicable: Android publishes EXA1 directly"}}"#
            );
            continue;
        }
        let json = session.bridge.output_bytes(u32::MAX as usize).to_vec();
        let length = session.bridge.dispatch(target, 0, 0, clock);
        session.publish(length);
        clock += 1.;
        let alternate = session.bridge.output_bytes(u32::MAX as usize).to_vec();
        let mut encoder = Encoder::default();
        let mut out = Vec::new();
        encoder.encode(&json, &mut out).unwrap();
        let mut samples = Vec::with_capacity(iterations);
        for i in 0..iterations {
            let input = if i % 2 == 0 { &alternate } else { &json };
            let start = Instant::now();
            encoder
                .encode(std::hint::black_box(input), std::hint::black_box(&mut out))
                .unwrap();
            samples.push(start.elapsed().as_secs_f64() * 1_000_000.);
        }
        assert!(out.len() >= HEADER_BYTES);
        report(
            &format!("{scenario}-wire-only"),
            samples,
            out.len(),
            u32::from_le_bytes(out[8..12].try_into().unwrap()),
        );
    }
}
