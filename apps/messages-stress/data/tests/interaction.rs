//! Opt-in CPU/runner diagnostic, not native input or frame presentation.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::Runner;
use messages_stress_data::MessagesStress;
use std::time::Instant;

#[test]
#[ignore = "opt-in release-mode interaction measurement; not a CI timing gate"]
fn typing_and_streaming() {
    for count in [100, 1000, 10000] {
        let plan = contract::compile(include_str!("../../app.contract")).unwrap();
        let mut runner = Runner::boot(
            plan,
            MessagesStress,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        runner.act("toggleEager", vec![]).unwrap();
        runner
            .act("chooseCount", vec![Value::Number(count as f64)])
            .unwrap();
        runner
            .act("chooseBatch", vec![Value::Number(32.0)])
            .unwrap();
        for mode in ["typing", "streaming"] {
            let mut samples = Vec::new();
            for sample in 0..40 {
                let now = Instant::now();
                if mode == "typing" {
                    runner
                        .act(
                            "editDraft",
                            vec![Value::str(&format!(
                                "sample {sample}: keep typing while history is mounted"
                            ))],
                        )
                        .unwrap();
                } else {
                    runner.act("step", vec![]).unwrap();
                }
                samples.push(now.elapsed().as_secs_f64() * 1000.0);
            }
            let mut sorted = samples.clone();
            sorted.sort_by(f64::total_cmp);
            println!("runner-only rows={count} mode={mode} samples_ms={samples:?} p50_ms={} p95_ms={} max_ms={}",sorted[19],sorted[37],sorted[39]);
        }
    }
}
