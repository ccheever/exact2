//! Opt-in native release diagnostic; elapsed CPU work is not frame presentation.
use std::hint::black_box;
use std::time::Instant;

use markdown_stress_data::fixture::{generate, Profile};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: parse-cost <mixed|paragraph|code|table|blocks> <16384|262144|1048576|4194304> <1..5 repeats>".into());
    }
    let profile = Profile::parse(&args[0])?;
    let bytes = args[1].parse()?;
    let repeats: usize = args[2].parse()?;
    if !(1..=5).contains(&repeats) {
        return Err("repeats must be 1..5".into());
    }
    println!(
        "profile,budget_bytes,source_bytes,blocks,sample,generate_ms,parse_ms,all_block_values_ms"
    );
    for sample in 1..=repeats {
        let started = Instant::now();
        let text = generate(profile, bytes)?;
        let generated = Instant::now();
        let document = markdown_parse::parse(black_box(&text), &str::to_owned);
        let parsed = Instant::now();
        let values = markdown_parse::value::blocks(black_box(&document));
        let converted = Instant::now();
        black_box(&values);
        println!(
            "{},{},{},{},{},{:.3},{:.3},{:.3}",
            profile.name(),
            bytes,
            text.len(),
            document.blocks.len(),
            sample,
            (generated - started).as_secs_f64() * 1000.0,
            (parsed - generated).as_secs_f64() * 1000.0,
            (converted - parsed).as_secs_f64() * 1000.0,
        );
    }
    Ok(())
}
