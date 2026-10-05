//! `difftest quick`: the semantics before a change lands, in about a
//! minute warm. Advice, not a gate: the async lane's `semantics` step is
//! the full run.
//!
//! From the files changed since `--base` (default: where HEAD left
//! origin/main) plus the working tree's: when none is under `contract/`,
//! `runner/`, `plan/` or `semantics/` there is nothing to run. Otherwise
//! the corpus programs relevant to them (a changed corpus file itself; the
//! corpus directories a changed file's name points at; the whole corpus
//! when none does), a small random sweep and the lowering check on a few
//! generated programs, both seeded by the base. Lean's observations are
//! reused from earlier runs when the case and the semantics are unchanged
//! (`leanrun::CACHE`), so a warm run spends its time on the runner's half
//! and on what changed.

use crate::{check, corpus, gen, leanrun, lowering, script::Case, seed_of, Outcome, Verdict};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// The trees whose changes the semantics can see.
const WATCHED: [&str; 4] = ["contract/", "runner/", "plan/", "semantics/"];

/// Words in a changed file's path, and the corpus directories they point at.
const AREAS: &[(&[&str], &[&str])] = &[
    (&["route", "router"], &["routes"]),
    (
        &["row", "each", "keyed", "list", "collection"],
        &["rows", "lists"],
    ),
    (&["timer", "task", "clock"], &["timers"]),
    (
        &["resource", "source", "oracle", "settle"],
        &["resources", "derives"],
    ),
    (&["mutation", "send"], &["mutations"]),
    (&["number", "float", "format"], &["numbers"]),
    (&["text", "string", "str"], &["text"]),
    (
        &["view", "render", "instance", "node", "element"],
        &["views", "rows"],
    ),
    (&["action", "stmt", "exec", "transaction"], &["actions"]),
    (&["derive"], &["derives"]),
    (&["fn", "call", "stdlib"], &["fns"]),
    (&["option", "match"], &["options"]),
    (&["record", "shape"], &["records"]),
    (&["component", "expand", "use"], &["components"]),
    (&["logic"], &["logic"]),
];

/// `difftest quick [--base <rev>]`.
pub fn run(args: &[String]) -> Result<bool, String> {
    let mut base = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--base" => base = Some(it.next().ok_or("--base needs a revision")?.clone()),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let started = Instant::now();
    let semantics = leanrun::project();
    let root = semantics.parent().expect("the repository").to_path_buf();
    let (base, changed) = changed(&root, base.as_deref())?;
    let watched: Vec<&String> = changed
        .iter()
        .filter(|f| WATCHED.iter().any(|w| f.starts_with(w)))
        .collect();
    if watched.is_empty() {
        println!(
            "quick: nothing changed under {}; nothing to run",
            WATCHED.join(", ")
        );
        return Ok(true);
    }
    leanrun::CACHE.store(true, std::sync::atomic::Ordering::Relaxed);
    let t = Instant::now();
    leanrun::build()?;
    println!(
        "quick: Lean library ready ({:.1}s)",
        t.elapsed().as_secs_f64()
    );

    let files = corpus_files(&root, &semantics, &watched)?;
    let mut ok = true;
    let t = Instant::now();
    let (scripted, errors) = corpus(&files)?;
    for e in &errors {
        println!("SCRIPT {e}");
        ok = false;
    }
    let jobs = std::thread::available_parallelism().map_or(2, |n| n.get());
    let failed: Vec<String> = std::thread::scope(|scope| {
        let handles: Vec<_> = scripted
            .chunks(scripted.len().div_ceil(jobs).max(1))
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .filter_map(|s| {
                            crate::expectations(s)
                                .err()
                                .map(|e| format!("EXPECT {}: {e}", s.case.name))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("expectations"))
            .collect()
    });
    for f in &failed {
        ok = false;
        println!("{f}");
    }
    let cases: Vec<Case> = scripted.into_iter().map(|s| s.case).collect();
    let n = cases.len();
    ok &= report(&check(cases, batch(n), "quick-corpus")?);
    println!(
        "quick: corpus, {} file(s), {n} case(s) ({:.1}s)",
        files.len(),
        t.elapsed().as_secs_f64()
    );

    // Seeded by the base, not HEAD: the same programs (and Lean's kept
    // observations of them) across a branch's commits, new ones as
    // origin/main moves.
    let seed = seed_of(&base);
    let t = Instant::now();
    let size = gen::Size::default();
    let cases: Vec<Case> = (0..40u64)
        .map(|i| gen::case(seed.wrapping_add(i), &size))
        .collect();
    ok &= report(&check(cases, batch(40), &format!("quick-random-{seed}"))?);
    println!(
        "quick: random --seed {seed} --count 40 ({:.1}s)",
        t.elapsed().as_secs_f64()
    );

    let t = Instant::now();
    let cases: Vec<Case> = (0..8u64)
        .map(|i| gen::case(seed.wrapping_add(i), &size))
        .collect();
    ok &= lowering::run(cases, 2, &format!("quick-lowering-{seed}"))?;
    println!(
        "quick: lowering --seed {seed} --count 8 ({:.1}s)",
        t.elapsed().as_secs_f64()
    );

    if watched.iter().any(|f| {
        f.starts_with("contract/types/")
            || ["Types", "TypeCheck", "ValTy"]
                .iter()
                .any(|m| f.as_str() == format!("semantics/Contract/{m}.lean"))
    }) {
        println!("quick: the type checker changed; `difftest types --count 20` compares the two checkers (minutes)");
    }
    println!(
        "quick: {} in {:.1}s",
        if ok { "agreed" } else { "FAILED (see above)" },
        started.elapsed().as_secs_f64()
    );
    Ok(ok)
}

/// Cases per Lean process: the cases spread over the jobs `check` runs.
fn batch(n: usize) -> usize {
    let jobs = std::env::var("DIFFTEST_JOBS")
        .ok()
        .and_then(|j| j.parse::<usize>().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).max(1))
        });
    n.div_ceil(jobs.max(1)).max(4)
}

/// The corpus files for `watched`: changed corpus files, then the
/// directories the other files' names point at; every file when a change
/// points nowhere in particular.
fn corpus_files(
    root: &Path,
    semantics: &Path,
    watched: &[&String],
) -> Result<Vec<PathBuf>, String> {
    let corpus_dir = semantics.join("corpus");
    let mut files = Vec::new();
    let mut dirs: Vec<&str> = Vec::new();
    let mut everything = false;
    for f in watched {
        if f.starts_with("semantics/corpus/") {
            let p = root.join(f.as_str());
            if p.is_file() && f.ends_with(".contract") {
                files.push(p);
            }
            continue;
        }
        let lower = f.to_ascii_lowercase();
        let stem = Path::new(&lower)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let words: Vec<&str> = stem.split(|c: char| !c.is_ascii_alphanumeric()).collect();
        let mut hit = false;
        for (keys, areas) in AREAS {
            if words.iter().any(|w| keys.contains(w)) {
                hit = true;
                dirs.extend(areas.iter());
            }
        }
        everything |= !hit;
    }
    if everything {
        return Ok(vec![corpus_dir]);
    }
    dirs.sort_unstable();
    dirs.dedup();
    for d in dirs {
        let p = corpus_dir.join(d);
        if p.is_dir() {
            files.push(p);
        }
    }
    // Always one program end to end, so a run is never empty.
    files.push(corpus_dir.join("counter.contract"));
    files.sort();
    files.dedup();
    Ok(files)
}

/// Files changed since `base` (default: the merge base with origin/main),
/// staged, unstaged and untracked, relative to the repository; and the
/// base's commit.
fn changed(root: &Path, base: Option<&str>) -> Result<(String, Vec<String>), String> {
    let git = |args: &[&str]| -> Result<String, String> {
        let out = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .map_err(|e| format!("git: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    };
    let base = match base {
        Some(b) => b.to_string(),
        None => git(&["merge-base", "HEAD", "origin/main"])?
            .trim()
            .to_string(),
    };
    let mut files: Vec<String> = git(&["diff", "--name-only", &base])?
        .lines()
        .chain(git(&["ls-files", "--others", "--exclude-standard"])?.lines())
        .map(str::to_string)
        .collect();
    files.sort();
    files.dedup();
    let base = git(&["rev-parse", &base])?.trim().to_string();
    Ok((base, files))
}

/// Print what did not agree; whether everything did. A case outside the
/// semantics, or a generated program the compiler refuses, is not a
/// failure.
fn report(outcomes: &[Outcome]) -> bool {
    let mut ok = true;
    for o in outcomes {
        match &o.verdict {
            Verdict::Agree | Verdict::Unsupported(_) | Verdict::Refused(_) => {}
            Verdict::Emit(e) => {
                ok = false;
                println!("EMIT {}: {e}", o.case.name);
            }
            Verdict::Expectation(e) => {
                ok = false;
                println!("EXPECT {}: {e}", o.case.name);
            }
            Verdict::Diverge { at, rust, lean, .. } => {
                ok = false;
                let rust: Vec<&String> = rust.iter().filter(|l| !l.starts_with('#')).collect();
                println!(
                    "DIVERGE {} at line {at}:\n  rust: {}\n  lean: {}\n  reproduce: cargo run -p contract-difftest -- {}",
                    o.case.name,
                    rust.get(*at).map_or("<end>", |l| l.as_str()),
                    lean.get(*at).map_or("<end>", String::as_str),
                    reproduce(&o.case.name)
                );
            }
        }
    }
    ok
}

/// The command that runs one case again.
fn reproduce(name: &str) -> String {
    match name.split_once(':') {
        Some((file, _)) if file.ends_with(".contract") => format!("corpus semantics/{file}"),
        _ => match name.strip_prefix("gen-") {
            Some(seed) => format!("random --seed {seed} --count 1"),
            None => "corpus".into(),
        },
    }
}
