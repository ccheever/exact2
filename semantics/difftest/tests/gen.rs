//! Every generated program compiles, and a seed always writes the same case.

use contract_difftest::gen::{case, Size};

/// Seeds checked per run; `DIFFTEST_GEN_SEEDS` raises it.
fn seeds() -> u64 {
    std::env::var("DIFFTEST_GEN_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000)
}

#[test]
fn generated_programs_compile() {
    refusals(&Size::default(), seeds());
}

#[test]
fn larger_programs_compile() {
    let size = Size {
        states: 8,
        derives: 6,
        actions: 6,
        depth: 4,
        events: 30,
    };
    refusals(&size, seeds() / 4);
}

/// Compile `n` cases, print the first few refusals in full and every
/// refusal's message, and fail if there were any.
fn refusals(size: &Size, n: u64) {
    let mut refused = Vec::new();
    for seed in 0..n {
        let c = case(seed, size);
        if let Err(e) = contract::compile(&c.source) {
            if refused.len() < 5 {
                eprintln!("--- seed {seed}: {e}\n{}", c.source);
            }
            refused.push((seed, e.to_string()));
        }
    }
    for (seed, e) in &refused {
        eprintln!("seed {seed}: {e}");
    }
    assert!(
        refused.is_empty(),
        "{} of {n} programs refused",
        refused.len()
    );
}

#[test]
fn a_seed_is_deterministic() {
    let size = Size::default();
    for seed in [0, 1, 7, 12345] {
        let (a, b) = (case(seed, &size), case(seed, &size));
        assert_eq!(a.source, b.source);
        assert_eq!(a.events, b.events);
    }
}

#[test]
fn scripts_aim_at_buttons() {
    let size = Size::default();
    let taps = (0..200)
        .flat_map(|seed| case(seed, &size).events)
        .filter(|e| matches!(e, contract_difftest::script::Event::Tap(_)))
        .count();
    assert!(taps > 200, "only {taps} taps in 200 scripts");
}
