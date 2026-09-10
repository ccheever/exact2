//! Native speed metrics for the v1 app, as one JSON line.
//!
//! Run by `scripts/metrics.mjs`; every number is a p50 over repeated runs
//! (or a size), measured on the real pipeline: compile → bake → boot →
//! layout → update → tick, plus the web host's batches.

use exact_kernel::{Kernel, Offer, PropId};
use exact_runner::{DataError, DataSource, Event, Runner, Value};
use exact_web::Host;
use std::cell::Cell;
use std::rc::Rc;
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
    if std::env::args().any(|arg| arg == "--scaling") {
        scaling();
        return;
    }
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
    // An inherited row changed on the root (LLP 1035.000 D2): the kernel
    // re-derives the computed value down the tree, stopping under any node
    // that sets its own — the invalidation's whole cost, and how far it
    // reached, alternating two inks so every change is a change.
    let (inherit_ms, inherit_touched) = {
        use exact_kernel::{Color, ColorValue, Op, StyleId, StyleMask, StyleProps};
        let root = runner.roots()[0];
        let mut samples = Vec::new();
        let mut touched = 0;
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
            let (receipt, ms) = time(|| runner.kernel_mut().apply(0, 0, &[op]).unwrap());
            touched = receipt.touched.len();
            samples.push(ms);
        }
        (p50(samples), touched)
    };
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
        "{{\"compile_ms\":{compile_ms:.3},\"bake_ms\":{bake_ms:.3},\"decode_ms\":{decode_ms:.3},\"plan_bytes\":{plan_bytes},\"baked_bytes\":{baked_bytes},\"boot_ms\":{boot_ms:.3},\"nodes\":{nodes},\"text_nodes\":{text_nodes},\"layout_ms\":{layout_ms:.3},\"update_ms\":{update_ms:.3},\"inherit_ms\":{inherit_ms:.3},\"inherit_touched\":{inherit_touched},\"tick_ms\":{tick_ms:.3},\"web_boot_ms\":{web_boot_ms:.3},\"web_first_batch_bytes\":{},\"web_update_ms\":{web_update_ms:.3},\"web_update_batch_bytes\":{}}}",
        first_batch.len(),
        batch.len()
    );
}

// A diagnostic workload, not a blocking benchmark or a runtime dependency graph.
// Every row has one text node; unrelated rows remain present during local edits.
#[derive(Clone)]
struct Rows {
    count: usize,
    requests: Rc<Cell<usize>>,
}

impl DataSource for Rows {
    fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
        self.requests.set(self.requests.get() + 1);
        let mut rows: Vec<_> = (0..self.count)
            .map(|id| Value::Record(vec![Value::Number(id as f64)].into()))
            .collect();
        if args == [Value::Bool(true)] {
            rows.reverse();
        }
        Ok(Value::List(rows.into()))
    }
}

fn summary(mut samples: Vec<f64>) -> String {
    samples.sort_by(f64::total_cmp);
    format!(
        "{{\"p50\":{:.4},\"p95\":{:.4}}}",
        samples[samples.len() / 2],
        samples[(samples.len() * 95).div_ceil(100) - 1]
    )
}

fn scaling() {
    let source = r#"shape Row
  id: number
component App
  state count = 0
  state reverse = false
  state shown = true
  resource rows = rows(reverse) as shape list<Row>
  derive total = length(rows)
  action bump writes count
    count = count + 1
  action reorder writes reverse
    reverse = !reverse
  action topology writes shown
    shown = !shown
  view
    column
      button press=bump testId="bump"
        text `${count}`
      button press=reorder testId="reorder"
        text "reorder"
      button press=topology testId="topology"
        text "topology"
      text `${total}`
      when shown
        each row in rows key=row.id
          text `${row.id}` testId=`row-${row.id}`
"#;
    let plan = contract::compile(source).unwrap();
    let mut results = Vec::new();
    for count in [300, 3000, 10000] {
        let data = Rows {
            count,
            requests: Rc::new(Cell::new(0)),
        };
        let encoded = plan.encode();
        for action in ["bump", "reorder", "topology"] {
            let mut runner =
                Runner::boot(plan.clone(), data.clone(), Kernel::with_monospace()).unwrap();
            let root = runner.roots()[0];
            runner
                .kernel_mut()
                .compute_layout(root, Offer::definite(390.0, 844.0))
                .unwrap();
            let (mut host, _) = Host::boot(&encoded, data.clone()).unwrap();
            let id = view_of(runner.kernel(), action);
            let host_id = view_of(host.runner().kernel(), action);
            let mut updates = Vec::new();
            let mut layouts = Vec::new();
            let mut batches = Vec::new();
            let mut requests = Vec::new();
            let mut touched = Vec::new();
            let mut created = Vec::new();
            let mut destroyed = Vec::new();
            let mut batch_bytes = Vec::new();
            for iteration in 0..44 {
                let before = data.requests.get();
                let (receipt, update_ms) = time(|| runner.dispatch(id, Event::Press).unwrap());
                let requested = data.requests.get() - before;
                let (_, layout_ms) = time(|| {
                    runner
                        .kernel_mut()
                        .compute_layout(root, Offer::definite(390.0, 844.0))
                        .unwrap()
                });
                let (batch, host_ms) = time(|| host.dispatch(host_id, Event::Press));
                assert!(!batch.contains("\"error\":\""), "{batch}");
                if iteration >= 4 {
                    updates.push(update_ms);
                    layouts.push(layout_ms);
                    batches.push(host_ms);
                    requests.push(requested as f64);
                    touched.push(receipt.touched.len() as f64);
                    created.push(receipt.created.len() as f64);
                    destroyed.push(receipt.destroyed.len() as f64);
                    batch_bytes.push(batch.len() as f64);
                }
                if action == "bump" {
                    assert_eq!(receipt.touched.len(), 1);
                    assert_eq!(requested, 0);
                }
                if action == "reorder" {
                    assert!(receipt.created.is_empty() && receipt.destroyed.is_empty());
                    assert_eq!(requested, 1);
                }
            }
            results.push(format!(
                "{{\"rows\":{count},\"nodes\":{},\"action\":\"{action}\",\"samples\":40,\"runner_update_ms\":{},\"layout_ms\":{},\"web_runner_and_batch_ms\":{},\"source_requests\":{},\"touched\":{},\"created\":{},\"destroyed\":{},\"batch_bytes\":{}}}",
                runner.kernel().live_count(), summary(updates), summary(layouts), summary(batches),
                summary(requests), summary(touched), summary(created), summary(destroyed), summary(batch_bytes)
            ));
        }
    }
    println!("{{\"scaling\":[{}],\"scaling_note\":\"release; four warmup updates, 40 measured; runner includes settlement/evaluation/kernel apply; web timings include runner and serialization, not browser; layout uses monospace, no physical device or allocation measurement\"}}", results.join(","));
}
