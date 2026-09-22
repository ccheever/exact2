//! Where the loader's time actually goes.
//!
//! `boot_real.rs` shows a 570-module graph taking well over a second to load
//! from source. This isolates why: it is not parse throughput, which is fine,
//! but a large FIXED cost per `evaluateJavaScript` call — and the loader makes
//! one call per module.
//!
//! That is the measurement that says how urgent LLP 0062 R3 (wrappers compiled
//! ahead of time) is, and it says: very.
//!
//!     cargo test -p ibex2 --features hermes --release --test eval_overhead -- --ignored --nocapture

#![cfg(feature = "hermes")]

use ibex2::engine::hermes::{DynamicCode, Hermes};
use std::time::Instant;

/// ~9.6KB of plausible JavaScript, the per-module size of Exact's real graph.
fn body(i: usize) -> String {
    let mut out = String::new();
    let mut n = 0;
    while out.len() < 9_600 {
        out.push_str(&format!(
            "function h{i}_{n}(a,b){{const s=a*{n}+b;if(s>100)return{{k:\'l\',s}};return{{k:\'s\',s}};}}\nconst t{i}_{n}={{id:{n},ap:h{i}_{n}}};\n"
        ));
        n += 1;
    }
    out
}

#[test]
#[ignore]
fn per_evaluation_overhead_dominates_parse_throughput() {
    let count = 570;
    let sources: Vec<String> = (0..count).map(body).collect();
    let total_bytes: usize = sources.iter().map(String::len).sum();

    // One evaluation per module, as the loader does today.
    let mut rt = Hermes::new(DynamicCode::Closed).expect("rt");
    let t = Instant::now();
    for (i, s) in sources.iter().enumerate() {
        rt.eval(&format!("(function(){{\n{s}\nreturn {i};}})"))
            .unwrap();
    }
    let many = t.elapsed();

    // The same bytes, one evaluation.
    let mut rt2 = Hermes::new(DynamicCode::Closed).expect("rt");
    let combined: String = sources
        .iter()
        .enumerate()
        .map(|(i, s)| format!("(function(){{\n{s}\nreturn {i};}});\n"))
        .collect();
    let t = Instant::now();
    rt2.eval(&combined).unwrap();
    let one = t.elapsed();

    let mb = total_bytes as f64 / 1_048_576.0;
    println!("\n=== per-evaluation overhead ({count} units, {mb:.2}MB) ===");
    println!(
        "  {count} separate evaluations : {many:?}  ({:.1} MB/s)",
        mb / many.as_secs_f64()
    );
    println!(
        "  1 combined evaluation      : {one:?}  ({:.1} MB/s)",
        mb / one.as_secs_f64()
    );
    println!(
        "  fixed cost per evaluation  : {:?}",
        many.saturating_sub(one) / count as u32
    );
    println!();
    println!("  Parse throughput is not the problem. LLP 0057 §1 measures ibex 1 at");
    println!("  155ms for 5.47MB, which is the same ~35 MB/s the combined run gets.");
}
