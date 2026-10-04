//! Differential testing of the Contract runner against the Lean semantics.
//!
//! A case is a program and a script of host events. Its program is compiled
//! twice: to a plan, which the runner boots and steps through the script,
//! printing a canonical observation after every step ([`observe`]); and by
//! the `contract lean` backend to a term of `semantics/`, whose
//! `Contract.Observe.run` prints the same observation from the operational
//! semantics ([`leanrun`]). The two texts must be equal. Data sources are a
//! seeded oracle; the runner's transcript of it is the Lean side's oracle
//! ([`oracle`]).
//!
//! Corpus cases are `test` blocks in `semantics/corpus/*.contract`; random
//! cases come from [`gen`].

pub mod arith;
pub mod gen;
pub mod js;
pub mod leanrun;
pub mod lowering;
pub mod observe;
pub mod oracle;
pub mod quick;
pub mod rng;
pub mod script;
pub mod types;
pub mod verify;

use script::{Case, Expect, Item, Scripted};
use std::path::{Path, PathBuf};

/// How one case ended.
#[derive(Debug, Clone)]
pub enum Verdict {
    /// Both sides printed the same observation.
    Agree,
    /// The compiler refused the program.
    Refused(String),
    /// The Lean backend refused a program the compiler accepted.
    Emit(String),
    /// The observations differ: the first differing line (0-based), the
    /// runner's and the semantics' lines.
    Diverge {
        /// Index of the first difference.
        at: usize,
        /// The runner's observation (with its `#` commentary).
        rust: Vec<String>,
        /// The semantics' observation.
        lean: Vec<String>,
        /// The semantics' observation with its `#` commentary.
        lean_raw: Vec<String>,
    },
    /// A corpus `expect` line did not hold on the runner.
    Expectation(String),
    /// The semantics refused a construct it leaves out (see
    /// `semantics/README.md`) where the two first differ.
    Unsupported(String),
}

/// A case and its verdict.
pub struct Outcome {
    /// The case.
    pub case: Case,
    /// What happened.
    pub verdict: Verdict,
}

/// A deterministic seed for a case name.
pub fn seed_of(name: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// A case run on the runner, ready for Lean.
pub struct Prepared {
    /// The runner's observation.
    pub rust: Vec<String>,
    /// The Lean half: the embedding, the oracle transcript, the events.
    pub lean: leanrun::LeanCase,
}

/// Compile a case's program: by its file when it has one (its `use`s
/// resolve), else as a text.
pub fn compile(case: &Case) -> Result<exact_plan::Plan, String> {
    match &case.path {
        Some(p) => contract::compile_path_source(p, &case.source),
        None => contract::compile(&case.source),
    }
    .map_err(|e| e.to_string())
}

/// Compile a case, run it on the runner, and emit its Lean half as the
/// `def` named `p<index>`.
pub fn prepare(case: &Case, index: usize) -> Result<Prepared, Verdict> {
    let plan = compile(case).map_err(Verdict::Refused)?;
    let name = format!("p{index}");
    let program = match &case.path {
        Some(p) => contract::lean::lean_path(p, &name),
        None => contract::lean::lean(&case.source, &name),
    }
    .map_err(|e| Verdict::Emit(e.to_string()))?;
    let oracle = oracle::Oracle::new(&plan, seed_of(&case.name));
    let (rust, data) = observe::run(plan, oracle, &case.events);
    let oracle = data.map_or_else(String::new, |o| o.text());
    Ok(Prepared {
        rust,
        lean: leanrun::LeanCase {
            name,
            program,
            oracle,
            events: case.events.iter().map(script::Event::lean).collect(),
        },
    })
}

/// Where generated modules and failing cases are kept.
pub fn work_dir() -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| leanrun::project().join("../target"));
    target.join("difftest")
}

/// Run `cases` on both sides, `batch` cases per Lean process, the Lean
/// processes side by side (`DIFFTEST_JOBS`, default half the cores).
pub fn check(cases: Vec<Case>, batch: usize, tag: &str) -> Result<Vec<Outcome>, String> {
    struct Batch {
        done: Vec<Option<Verdict>>,
        ready: Vec<Prepared>,
    }
    let chunks: Vec<&[Case]> = cases.chunks(batch.max(1)).collect();
    let jobs = std::env::var("DIFFTEST_JOBS")
        .ok()
        .and_then(|j| j.parse::<usize>().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).max(1))
        });
    // The runner's half, and the Lean modules' text, a batch per thread;
    // Lean's half in parallel below.
    let prepared: Vec<std::sync::Mutex<Option<Batch>>> =
        chunks.iter().map(|_| std::sync::Mutex::new(None)).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(chunks.len()) {
            scope.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let Some(chunk) = chunks.get(k) else { break };
                let mut b = Batch {
                    done: Vec::new(),
                    ready: Vec::new(),
                };
                for (i, case) in chunk.iter().enumerate() {
                    match prepare(case, i) {
                        Ok(p) => {
                            b.ready.push(p);
                            b.done.push(None);
                        }
                        Err(v) => b.done.push(Some(v)),
                    }
                }
                *prepared[k].lock().expect("batch slot") = Some(b);
            });
        }
    });
    let batches: Vec<Batch> = prepared
        .into_iter()
        .map(|b| {
            b.into_inner()
                .expect("batch slot")
                .expect("every batch prepared")
        })
        .collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    type Observed = Result<Vec<Vec<String>>, String>;
    let results: Vec<std::sync::Mutex<Option<Observed>>> = batches
        .iter()
        .map(|_| std::sync::Mutex::new(None))
        .collect();
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(batches.len()) {
            scope.spawn(|| loop {
                let b = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let Some(batch) = batches.get(b) else { break };
                let leans: Vec<leanrun::LeanCase> =
                    batch.ready.iter().map(|p| clone_case(&p.lean)).collect();
                let r = leanrun::run(&leans, &work_dir(), &format!("{tag}-{b}"));
                *results[b].lock().expect("result slot") = Some(r);
            });
        }
    });
    let mut outcomes = Vec::new();
    for ((chunk, batch), result) in chunks.iter().zip(batches).zip(results) {
        let lean = result
            .into_inner()
            .expect("result slot")
            .expect("every batch ran")?;
        let mut lean = lean.into_iter().zip(batch.ready);
        for (case, verdict) in chunk.iter().zip(batch.done) {
            let verdict = match verdict {
                Some(v) => v,
                None => {
                    let (lean, p) = lean.next().expect("one result per prepared case");
                    compare(p.rust, lean)
                }
            };
            outcomes.push(Outcome {
                case: case.clone(),
                verdict,
            });
        }
    }
    Ok(outcomes)
}

/// A copy of a Lean case.
pub fn clone_case(c: &leanrun::LeanCase) -> leanrun::LeanCase {
    leanrun::LeanCase {
        name: c.name.clone(),
        program: c.program.clone(),
        oracle: c.oracle.clone(),
        events: c.events.clone(),
    }
}

fn compare(rust: Vec<String>, lean: Vec<String>) -> Verdict {
    // `#` lines are commentary (a refusal's reason), never compared.
    let keep = |v: Vec<String>| -> Vec<String> {
        v.into_iter()
            .flat_map(|l| l.lines().map(str::to_string).collect::<Vec<_>>())
            .filter(|l| !l.starts_with('#'))
            .collect()
    };
    let lean_raw = lean.clone();
    let rust_raw = rust.clone();
    let (rust, lean) = (keep(rust), keep(lean));
    // Where the two first differ, a refusal for a construct the semantics
    // leaves out is not a divergence.
    let unsupported = |at: usize| -> Option<String> {
        let mut seen = 0;
        for l in lean_raw.iter().flat_map(|l| l.lines()) {
            if l.starts_with('#') {
                if seen > at && l.starts_with("# unsupported") {
                    return Some(l.to_string());
                }
            } else {
                seen += 1;
                if seen > at + 1 {
                    return None;
                }
            }
        }
        None
    };
    // The semantics asked a question the runner never did: the two took
    // different paths, whatever the outcome lines say.
    if let Some(at) = lean_raw
        .iter()
        .position(|l| l.contains("# refused: the oracle has no answer"))
    {
        return Verdict::Diverge {
            at: lean_raw[..at]
                .iter()
                .filter(|l| !l.starts_with('#'))
                .count(),
            rust: rust_raw,
            lean,
            lean_raw,
        };
    }
    let first = rust.iter().zip(&lean).position(|(a, b)| a != b);
    if let Some(at) = first.or((rust.len() != lean.len()).then(|| rust.len().min(lean.len()))) {
        if let Some(why) = unsupported(at) {
            return Verdict::Unsupported(why);
        }
    }
    let at = rust.iter().zip(&lean).position(|(a, b)| a != b);
    match at {
        None if rust.len() == lean.len() => Verdict::Agree,
        None => Verdict::Diverge {
            at: rust.len().min(lean.len()),
            rust: rust_raw,
            lean,
            lean_raw,
        },
        Some(at) => Verdict::Diverge {
            at,
            rust: rust_raw,
            lean,
            lean_raw,
        },
    }
}

/// The runner's observation split into steps: entry 0 is boot, entry `k`
/// follows the `k`-th event.
fn chunks(lines: &[String]) -> Vec<&[String]> {
    let mut starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("== "))
        .map(|(i, _)| i)
        .collect();
    starts.push(lines.len());
    starts.windows(2).map(|w| &lines[w[0]..w[1]]).collect()
}

/// Check a corpus case's `expect` lines against the runner's observation.
pub fn expectations(s: &Scripted) -> Result<(), String> {
    let plan = compile(&s.case)?;
    let oracle = oracle::Oracle::new(&plan, seed_of(&s.case.name));
    let (rust, _) = observe::run(plan, oracle, &s.case.events);
    let steps = chunks(&rust);
    let mut k = 0;
    for item in &s.items {
        match item {
            Item::Event(_) => k += 1,
            Item::Expect(e) => {
                let step = steps.get(k).copied().unwrap_or(&[]);
                let ok = match e {
                    Expect::State(name, v) => ["slot", "derive", "resource"]
                        .iter()
                        .any(|kind| step.contains(&format!("{kind} {name} {v}"))),
                    Expect::Text(id, v) => step.contains(&format!(
                        "view {} {}",
                        observe::quote(id),
                        observe::quote(v)
                    )),
                    Expect::Tree(id, present) => {
                        let prefix = format!("view {} ", observe::quote(id));
                        step.iter().any(|l| l.starts_with(&prefix)) == *present
                    }
                };
                if !ok {
                    return Err(format!(
                        "after step {k}: {e:?} does not hold; the runner shows:\n  {}",
                        step.join("\n  ")
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Every scripted case in the corpus files under `dirs`, and the files
/// whose scripts could not be read (each is explored instead).
pub fn corpus(dirs: &[PathBuf]) -> Result<(Vec<Scripted>, Vec<String>), String> {
    let mut files = Vec::new();
    for d in dirs {
        collect(d, &mut files)?;
    }
    files.sort();
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for f in files {
        // Named from the project, however the path was spelled: the name
        // seeds the case's data.
        let f = f.canonicalize().unwrap_or(f);
        let src = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        let name = f
            .strip_prefix(
                leanrun::project()
                    .canonicalize()
                    .unwrap_or_else(|_| leanrun::project()),
            )
            .unwrap_or(&f)
            .display()
            .to_string();
        let cases = script::scripted(&name, &src, Some(&f)).unwrap_or_else(|e| {
            errors.push(e);
            Vec::new()
        });
        if cases.is_empty() {
            // A program without tests is explored: every element with a
            // `testId` the boot shows is tapped, twice, then the clock runs.
            out.push(explore(&name, &src, Some(&f)));
        } else {
            out.extend(cases);
        }
    }
    Ok((out, errors))
}

/// A script for a program that has none: tap each `testId` the runner
/// shows at boot, twice, in preorder, then advance the clock.
pub fn explore(name: &str, source: &str, path: Option<&Path>) -> Scripted {
    let mut events = Vec::new();
    let probe = Case {
        name: name.to_string(),
        source: source.to_string(),
        events: Vec::new(),
        path: path.map(Path::to_path_buf),
    };
    if let Ok(plan) = compile(&probe) {
        let oracle = oracle::Oracle::new(&plan, seed_of(name));
        let (lines, _) = observe::run(plan, oracle, &[]);
        let ids: Vec<String> = lines
            .iter()
            .filter_map(|l| l.strip_prefix("view \""))
            .filter_map(|l| l.split_once("\" ").map(|(id, _)| id.to_string()))
            .filter(|id| !id.contains('\\'))
            .collect();
        for _ in 0..2 {
            events.extend(ids.iter().map(|id| script::Event::Tap(id.clone())));
        }
        events.push(script::Event::Clock(1000.0));
        events.push(script::Event::Clock(60_000.0));
    }
    let case = Case {
        name: format!("{name}: explored"),
        events: events.clone(),
        ..probe
    };
    Scripted {
        case,
        items: events.into_iter().map(Item::Event).collect(),
    }
}

/// Every `.contract` file at `path` (a file, or a directory searched
/// recursively).
pub fn collect(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_file() {
        out.push(path.to_path_buf());
        return Ok(());
    }
    let entries = std::fs::read_dir(path).map_err(|e| format!("{}: {e}", path.display()))?;
    for entry in entries {
        let p = entry.map_err(|e| e.to_string())?.path();
        if p.is_dir() {
            collect(&p, out)?;
        } else if p.extension().is_some_and(|x| x == "contract") {
            out.push(p);
        }
    }
    Ok(())
}

/// Shrink a diverging case's script: drop events while it still diverges,
/// one Lean process per round (every single-event removal at once).
pub fn shrink(case: &Case, tag: &str) -> Result<Case, String> {
    let mut best = case.clone();
    loop {
        if best.events.is_empty() {
            return Ok(best);
        }
        let candidates: Vec<Case> = (0..best.events.len())
            .map(|i| {
                let mut c = best.clone();
                c.events.remove(i);
                c
            })
            .collect();
        let outcomes = check(candidates, usize::MAX, tag)?;
        match outcomes
            .into_iter()
            .find(|o| matches!(o.verdict, Verdict::Diverge { .. }))
        {
            Some(o) => best = o.case,
            None => return Ok(best),
        }
    }
}
