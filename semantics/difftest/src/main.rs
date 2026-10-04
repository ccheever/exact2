//! `difftest`: the runner against the Lean semantics.
//!
//!   difftest corpus [<dir|file>…]        every `test` block (default semantics/corpus)
//!   difftest explore <dir|file>…         any programs: tap every testId the boot shows
//!   difftest random [--seed S] [--count N] [--batch B]
//!   difftest numbers [--seed S] [--count N]   number printing alone
//!   difftest types [--seed S] [--count N]     the Lean type checker against the Rust one
//!   difftest show <seed>                       a random case's program and script
//!   difftest apps [--write]              the app embeddings under semantics/Apps
//!                                        are what `contract lean` makes of their source
//!   difftest lowering [--seed S] [--count N] [--batch B]
//!   difftest lowering-corpus [<dir|file>…]     the compiler's bytecode against the
//!                                              Lean VM model and `eval` (semantics/README.md)
//!
//! Prints one line per divergence with where its reproduction was kept, then
//! a summary; exits 1 when anything diverged, a corpus program was refused,
//! a corpus `expect` failed, or the Lean backend refused a program the
//! compiler accepts.

use contract_difftest::{
    check, corpus, expectations, gen, leanrun, lowering, script::Case, shrink, work_dir, Outcome,
    Verdict,
};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage:
  difftest corpus [<dir|file>…]
  difftest explore <dir|file>…
  difftest random [--seed <u64>] [--count <n>] [--batch <n>]
  difftest numbers [--seed <u64>] [--count <n>]
  difftest types [--seed <u64>] [--count <n>]
  difftest show <seed>
  difftest apps [--write]
  difftest lowering [--seed <u64>] [--count <n>] [--batch <n>]
  difftest lowering-corpus [<dir|file>…]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("corpus") => run_corpus(&args[1..]),
        Some("explore") => run_explore(&args[1..]),
        Some("random") => run_random(&args[1..]),
        Some("numbers") => run_numbers(&args[1..]),
        Some("types") => run_types(&args[1..]),
        Some("apps") => run_apps(&args[1..]),
        Some("lowering") => run_lowering(&args[1..]),
        Some("lowering-corpus") => run_lowering_corpus(&args[1..]),
        Some("show") => match args.get(1).map(|s| s.parse::<u64>()) {
            Some(Ok(seed)) => {
                let case = gen::case(seed, &gen::Size::default());
                println!("{}", case.source);
                for e in &case.events {
                    println!("// {}", e.label());
                }
                return ExitCode::SUCCESS;
            }
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        },
        Some("--help" | "-h") => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("difftest: {e}");
            ExitCode::from(2)
        }
    }
}

fn run_corpus(args: &[String]) -> Result<bool, String> {
    let dirs: Vec<PathBuf> = if args.is_empty() {
        vec![leanrun::project().join("corpus")]
    } else {
        args.iter().map(PathBuf::from).collect()
    };
    leanrun::build()?;
    let (scripted, errors) = corpus(&dirs)?;
    for e in &errors {
        println!("SCRIPT {e}");
    }
    let mut failed = Vec::new();
    for s in &scripted {
        if let Err(e) = expectations(s) {
            failed.push(s.case.name.clone());
            println!("EXPECT {}: {e}", s.case.name);
        }
    }
    let cases: Vec<Case> = scripted.into_iter().map(|s| s.case).collect();
    let mut outcomes = check(cases, 64, "corpus")?;
    // An expectation that failed on the runner is the case's verdict when
    // the two sides agree.
    for o in &mut outcomes {
        if matches!(o.verdict, Verdict::Agree) && failed.contains(&o.case.name) {
            o.verdict = Verdict::Expectation("see above".into());
        }
    }
    println!("difftest: {} scripts could not be read", errors.len());
    Ok(report(&outcomes, true, false)? && errors.is_empty())
}

/// Programs written for something else (the apps, the compiler's corpus):
/// each is explored, its tests ignored; one the compiler refuses (a used
/// file that is not a root) is skipped.
fn run_explore(args: &[String]) -> Result<bool, String> {
    if args.is_empty() {
        return Err(format!("explore needs files or directories\n{USAGE}"));
    }
    leanrun::build()?;
    let mut files = Vec::new();
    for a in args {
        contract_difftest::collect(std::path::Path::new(a), &mut files)?;
    }
    files.sort();
    let mut cases = Vec::new();
    for f in &files {
        let src = std::fs::read_to_string(f).map_err(|e| format!("{}: {e}", f.display()))?;
        cases.push(contract_difftest::explore(&f.display().to_string(), &src, Some(f)).case);
    }
    let outcomes = check(cases, 16, "explore")?;
    report(&outcomes, false, false)
}

fn run_random(args: &[String]) -> Result<bool, String> {
    let mut seed: u64 = 1;
    let mut count = 100usize;
    let mut batch = 50usize;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--seed" => seed = value()?.parse().map_err(|e| format!("--seed: {e}"))?,
            "--count" => count = value()?.parse().map_err(|e| format!("--count: {e}"))?,
            "--batch" => batch = value()?.parse().map_err(|e| format!("--batch: {e}"))?,
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    leanrun::build()?;
    let size = gen::Size::default();
    let cases: Vec<Case> = (0..count as u64)
        .map(|i| gen::case(seed.wrapping_add(i), &size))
        .collect();
    let outcomes = check(cases, batch, &format!("random-{seed}"))?;
    report(&outcomes, false, true)
}

/// Random programs' bytecode against the Lean VM model and the semantics.
fn run_lowering(args: &[String]) -> Result<bool, String> {
    let mut seed: u64 = 1;
    let mut count = 100usize;
    let mut batch = 25usize;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--seed" => seed = value()?.parse().map_err(|e| format!("--seed: {e}"))?,
            "--count" => count = value()?.parse().map_err(|e| format!("--count: {e}"))?,
            "--batch" => batch = value()?.parse().map_err(|e| format!("--batch: {e}"))?,
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    let size = gen::Size::default();
    let cases: Vec<Case> = (0..count as u64)
        .map(|i| gen::case(seed.wrapping_add(i), &size))
        .collect();
    lowering::run(cases, batch, &format!("lowering-{seed}"))
}

/// The corpus's programs' bytecode against the Lean VM model and the
/// semantics.
fn run_lowering_corpus(args: &[String]) -> Result<bool, String> {
    let dirs: Vec<PathBuf> = if args.is_empty() {
        vec![leanrun::project().join("corpus")]
    } else {
        args.iter().map(PathBuf::from).collect()
    };
    let (scripted, errors) = corpus(&dirs)?;
    for e in &errors {
        println!("SCRIPT {e}");
    }
    let cases: Vec<Case> = scripted.into_iter().map(|s| s.case).collect();
    lowering::run(cases, 16, "lowering-corpus")
}

/// The Lean type checker against the Rust one over generated programs and
/// a mutant of each (`contract_difftest::types`).
fn run_types(args: &[String]) -> Result<bool, String> {
    let mut seed: u64 = 1;
    let mut count = 100usize;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--seed" => seed = value()?.parse().map_err(|e| format!("--seed: {e}"))?,
            "--count" => count = value()?.parse().map_err(|e| format!("--count: {e}"))?,
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    contract_difftest::types::run(seed, count, &work_dir())
}

/// The runner's number printing against the semantics' over random
/// doubles: arbitrary bit patterns, short decimals, integers, and the
/// neighbours of the printing thresholds.
fn run_numbers(args: &[String]) -> Result<bool, String> {
    let mut seed: u64 = 1;
    let mut count = 100_000usize;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--seed" => seed = value()?.parse().map_err(|e| format!("--seed: {e}"))?,
            "--count" => count = value()?.parse().map_err(|e| format!("--count: {e}"))?,
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    leanrun::build()?;
    let mut rng = contract_difftest::rng::Rng::new(seed);
    let edges = [
        1e21,
        1e-6,
        1e-7,
        5e-324,
        f64::MAX,
        f64::MIN_POSITIVE,
        0.1,
        9007199254740992.0,
    ];
    let values: Vec<f64> = (0..count)
        .map(|i| match i % 5 {
            0 => f64::from_bits(rng.next_u64()),
            1 => (rng.below(2_000_000) as f64 - 1_000_000.0) / 10f64.powi(rng.below(8) as i32),
            2 => rng.next_u64() as f64,
            3 => {
                let e = edges[rng.below(edges.len() as u64) as usize];
                f64::from_bits(e.to_bits().wrapping_add(rng.below(5)).wrapping_sub(2))
            }
            _ => (rng.below(1000) as f64) * 10f64.powi(rng.below(60) as i32 - 30),
        })
        .collect();
    let bits: Vec<u64> = values.iter().map(|v| v.to_bits()).collect();
    let lean = leanrun::numbers(&bits, &work_dir())?;
    let mut bad = 0;
    for (v, l) in values.iter().zip(&lean) {
        let r = exact_runner::stdlib::format_number(*v);
        if &r != l {
            bad += 1;
            if bad <= 20 {
                println!("DIVERGE number {:016x}: rust {r} lean {l}", v.to_bits());
            }
        }
    }
    println!(
        "difftest: {} numbers, {bad} printed differently",
        values.len()
    );
    Ok(bad == 0 && lean.len() == values.len())
}

/// The apps whose embeddings `semantics/Apps/` keeps, proved about under
/// `semantics/Apps/Proofs/`: (source, Lean name, module).
const APPS: &[(&str, &str, &str)] = &[
    ("apps/typetour/app.contract", "typeTour", "TypeTour"),
    ("apps/update-lab/app.contract", "updateLab", "UpdateLab"),
    (
        "apps/photo-editor/app.contract",
        "photoEditor",
        "PhotoEditor",
    ),
    (
        "apps/video-player/app.contract",
        "videoPlayer",
        "VideoPlayer",
    ),
];

/// Each checked-in embedding against what `contract lean` makes of the
/// app's source now, so the proofs are about the app as it is. `--write`
/// regenerates them (then `lake build` says which proofs need work).
fn run_apps(args: &[String]) -> Result<bool, String> {
    let write = match args {
        [] => false,
        [w] if w == "--write" => true,
        _ => return Err(format!("unknown arguments\n{USAGE}")),
    };
    let semantics = leanrun::project();
    let root = semantics.parent().expect("the repository");
    let mut stale = 0;
    for (source, name, module) in APPS {
        let text = contract::lean::lean_path(&root.join(source), name)
            .map_err(|e| format!("{source}: {e}"))?;
        let text = format!("import Contract\n\n{text}");
        let file = semantics.join("Apps").join(format!("{module}.lean"));
        if std::fs::read_to_string(&file).ok().as_deref() == Some(text.as_str()) {
            continue;
        }
        if write {
            std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
            println!("WROTE {}", file.display());
        } else {
            stale += 1;
            println!(
                "STALE {}: not what `contract lean {source} --name {name}` makes; run `difftest apps --write`, then `lake build` in semantics/",
                file.display()
            );
        }
    }
    println!("difftest: {} app embeddings, {stale} stale", APPS.len());
    Ok(stale == 0)
}

/// Print every case that did not agree, keeping a reproduction; the
/// summary; whether the run passed.
fn report(outcomes: &[Outcome], strict_refusals: bool, shrinking: bool) -> Result<bool, String> {
    let (mut agree, mut refused, mut emit, mut diverge, mut expect, mut unsupported) =
        (0, 0, 0, 0, 0, 0);
    let dir = work_dir().join("failures");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (i, o) in outcomes.iter().enumerate() {
        match &o.verdict {
            Verdict::Agree => agree += 1,
            Verdict::Unsupported(why) => {
                unsupported += 1;
                println!("UNSUPPORTED {}: {why}", o.case.name);
            }
            Verdict::Refused(e) => {
                refused += 1;
                if strict_refusals {
                    println!("REFUSED {}: {e}", o.case.name);
                }
            }
            Verdict::Emit(e) => {
                emit += 1;
                println!("EMIT {}: {e}", o.case.name);
            }
            Verdict::Expectation(_) => expect += 1,
            Verdict::Diverge {
                at,
                rust,
                lean,
                lean_raw,
            } => {
                diverge += 1;
                let case = if shrinking {
                    shrink(&o.case, &format!("shrink-{i}"))?
                } else {
                    o.case.clone()
                };
                let stem = dir.join(format!("{}-case-{i}", std::process::id()));
                let events: Vec<String> = case.events.iter().map(|e| e.label()).collect();
                let _ = std::fs::write(stem.with_extension("contract"), &case.source);
                let _ = std::fs::write(stem.with_extension("events"), events.join("\n"));
                let _ = std::fs::write(stem.with_extension("rust.txt"), rust.join("\n"));
                let _ = std::fs::write(stem.with_extension("lean.txt"), lean_raw.join("\n"));
                let why = lean_raw
                    .iter()
                    .find(|l| l.starts_with('#'))
                    .map_or(String::new(), |w| format!("\n  lean {w}"));
                let why = why
                    + &rust
                        .iter()
                        .find(|l| l.starts_with('#'))
                        .map_or(String::new(), |w| format!("\n  rust {w}"));
                println!(
                    "DIVERGE {} at line {at}:\n  rust: {}\n  lean: {}{why}\n  kept: {}.{{contract,events,rust.txt,lean.txt}}",
                    o.case.name,
                    rust.iter()
                        .filter(|l| !l.starts_with('#'))
                        .nth(*at)
                        .map_or("<end>", String::as_str),
                    lean.get(*at).map_or("<end>", String::as_str),
                    stem.display()
                );
            }
        }
    }
    println!(
        "difftest: {} cases: {agree} agree, {diverge} diverge, {refused} refused by the compiler, {emit} refused by the Lean backend, {expect} failed expectations, {unsupported} outside the semantics",
        outcomes.len()
    );
    Ok(diverge == 0 && emit == 0 && expect == 0 && (!strict_refusals || refused == 0))
}
