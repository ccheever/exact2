//! The web JS target's half (`--js`): the second implementation of Contract
//! (host/web-js, LLP 1071), run headless by `js/drive.mjs` under Bun and
//! compared with the runner line for line.
//!
//! Each case's program is compiled by `exact-web-js` (the build's own
//! compiler), its module bundled with the runtime and run in a fresh VM
//! context over a small DOM. Its data sources answer from the runner's
//! oracle transcript, synchronously as the runner's do, so a call the JS
//! target makes that the runner did not is a divergence. A program
//! `exact-web-js` refuses is outside the JS target, not a failure.

use crate::oracle::{self, Oracle};
use crate::script::{Case, Event};
use crate::{compile, observe, seed_of};
use exact_plan::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// How one case ended on the JS target.
#[derive(Debug, Clone)]
pub enum JsVerdict {
    /// The JS target printed the runner's observation.
    Agree,
    /// The compiler refused the program (the runner never ran it).
    Refused(String),
    /// `exact-web-js` refused it: outside the JS target.
    Outside(String),
    /// The observations differ at line `at` (0-based, `#` lines skipped).
    Diverge {
        /// Index of the first difference.
        at: usize,
        /// The runner's observation (with its `#` commentary).
        rust: Vec<String>,
        /// The JS target's (with its `#` commentary).
        js: Vec<String>,
    },
    /// The driver failed on the case.
    Error(String),
}

/// `exact-web-js`, built once (as `leanrun::build` builds the semantics).
pub fn build() -> Result<PathBuf, String> {
    let out = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["build", "-q", "-p", "exact-web-js"])
        .current_dir(repo())
        .output()
        .map_err(|e| format!("cargo build -p exact-web-js: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo build -p exact-web-js failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo().join("target"));
    Ok(target.join("debug/exact-web-js"))
}

fn repo() -> PathBuf {
    crate::leanrun::project()
        .parent()
        .expect("the repository")
        .to_path_buf()
}

/// `bun`, from `BUN`, `PATH`, or its installer's default place.
pub fn bun() -> PathBuf {
    if let Some(b) = std::env::var_os("BUN") {
        return PathBuf::from(b);
    }
    if let Some(home) = std::env::var_os("HOME") {
        let b = Path::new(&home).join(".bun/bin/bun");
        if b.is_file() {
            return b;
        }
    }
    PathBuf::from("bun")
}

/// A number as the batch carries it: its bits, which JSON cannot lose.
fn number(n: f64) -> String {
    format!("{{\"$\":\"{:016x}\"}}", n.to_bits())
}

fn string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A value as the JS runtime holds it (rt.js `value_`): lists and records
/// are arrays, unit and `none` null, `some(v)` v.
fn value(v: &Value) -> String {
    match v {
        Value::Number(n) => number(*n),
        Value::Bool(b) => b.to_string(),
        v if v.is_str() => string(v.as_str().unwrap_or_default()),
        Value::Option(Some(inner)) => value(inner),
        Value::List(items) | Value::Record(items) => {
            let parts: Vec<String> = items.iter().map(value).collect();
            format!("[{}]", parts.join(","))
        }
        _ => "null".into(),
    }
}

fn event(e: &Event) -> String {
    match e {
        Event::Tap(t) => format!("{{\"tap\":{}}}", string(t)),
        Event::Type(t, s) => format!("{{\"type\":[{},{}]}}", string(t), string(s)),
        Event::Clock(ms) => format!("{{\"clock\":{}}}", number(*ms)),
    }
}

/// A case run on the runner, ready for the driver.
struct Ready {
    rust: Vec<String>,
    json: String,
}

/// Run `case` on the runner and write its half of a batch; the program goes
/// to `dir` unless it is its own file, unchanged (its `use`s resolve there).
fn prepare(case: &Case, dir: &Path, index: usize) -> Result<Ready, JsVerdict> {
    let plan = compile(case).map_err(JsVerdict::Refused)?;
    let (rust, data) = observe::run(plan.clone(), Oracle::new(&plan, seed_of(&case.name)), &case.events);
    let data = data.expect("the oracle comes back");
    let file = match &case.path {
        Some(p) if std::fs::read_to_string(p).ok().as_deref() == Some(case.source.as_str()) => {
            p.clone()
        }
        _ => {
            let f = dir.join(format!("case-{index}.contract"));
            std::fs::write(&f, &case.source)
                .map_err(|e| JsVerdict::Error(format!("{}: {e}", f.display())))?;
            f
        }
    };
    let answers: Vec<String> = data
        .transcript
        .borrow()
        .iter()
        .map(|c| {
            let args: Vec<String> = c.args.iter().map(|(_, a)| value(a)).collect();
            format!(
                "[{},[{}],{}]",
                string(&c.source),
                args.join(","),
                value(&c.answer.1)
            )
        })
        .collect();
    let facts: Vec<String> = data
        .facts
        .borrow()
        .iter()
        .map(|(name, _, v)| format!("{}:{}", string(name), value(v)))
        .collect();
    let events: Vec<String> = case.events.iter().map(event).collect();
    let json = format!(
        "{{\"file\":{},\"events\":[{}],\"answers\":[{}],\"facts\":{{{}}}}}",
        string(&file.display().to_string()),
        events.join(","),
        answers.join(","),
        facts.join(",")
    );
    Ok(Ready { rust, json })
}

/// One driver process over `ready`; each case's result line.
fn drive(compiler: &Path, dir: &Path, ready: &[&Ready]) -> Result<Vec<String>, String> {
    let hosts: Vec<String> = oracle::HOST_SOURCES.iter().map(|s| string(s)).collect();
    let cases: Vec<&str> = ready.iter().map(|r| r.json.as_str()).collect();
    let batch = format!(
        "{{\"compiler\":{},\"work\":{},\"hostSources\":[{}],\"cases\":[{}]}}",
        string(&compiler.display().to_string()),
        string(&dir.join("runtime").display().to_string()),
        hosts.join(","),
        cases.join(",\n")
    );
    let file = dir.join("batch.json");
    std::fs::write(&file, batch).map_err(|e| format!("{}: {e}", file.display()))?;
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("js/drive.mjs");
    let out = Command::new(bun())
        .arg(&script)
        .arg(&file)
        .output()
        .map_err(|e| format!("bun {}: {e} (is Bun installed?)", script.display()))?;
    let lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    if !out.status.success() || lines.len() != ready.len() {
        return Err(format!(
            "{}: the driver printed {} of {} cases:\n{}",
            file.display(),
            lines.len(),
            ready.len(),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(lines)
}

/// A driver line's field `name`: its strings (`lines`), or its one string
/// (`outside`, `error`).
fn field(line: &serde_json::Value, name: &str) -> Option<Vec<String>> {
    match line.get(name)? {
        serde_json::Value::String(s) => Some(vec![s.clone()]),
        serde_json::Value::Array(items) => Some(
            items
                .iter()
                .map(|i| i.as_str().unwrap_or_default().to_string())
                .collect(),
        ),
        _ => None,
    }
}

fn verdict(rust: Vec<String>, text: &str) -> JsVerdict {
    let line: serde_json::Value = match serde_json::from_str(text) {
        Ok(l) => l,
        Err(e) => return JsVerdict::Error(format!("{e}: {text}")),
    };
    let line = &line;
    if let Some(why) = field(line, "outside") {
        return JsVerdict::Outside(why.concat());
    }
    let Some(js) = field(line, "lines") else {
        return JsVerdict::Error(field(line, "error").map_or_else(|| text.to_string(), |e| e.concat()));
    };
    let keep = |v: &[String]| -> Vec<String> {
        v.iter().filter(|l| !l.starts_with('#')).cloned().collect()
    };
    let (r, j) = (keep(&rust), keep(&js));
    // A question the runner never asked shows as a `#` note; it counts only
    // where the lines differ (inside a commit both refuse, the order of
    // what each evaluates before refusing is not observable).
    let at = r.iter().zip(&j).position(|(a, b)| a != b);
    match at {
        None if r.len() == j.len() => JsVerdict::Agree,
        None => JsVerdict::Diverge {
            at: r.len().min(j.len()),
            rust,
            js,
        },
        Some(at) => JsVerdict::Diverge { at, rust, js },
    }
}

/// Run `cases` on the runner and the JS target, `batch` cases per driver
/// process, the processes side by side (`DIFFTEST_JOBS`, as [`crate::check`]).
pub fn check(
    compiler: &Path,
    cases: &[Case],
    batch: usize,
    tag: &str,
) -> Result<Vec<JsVerdict>, String> {
    let root = crate::work_dir().join("js").join(format!("{tag}-{}", std::process::id()));
    let chunks: Vec<&[Case]> = cases.chunks(batch.max(1)).collect();
    let jobs = std::env::var("DIFFTEST_JOBS")
        .ok()
        .and_then(|j| j.parse::<usize>().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).max(1))
        });
    let next = std::sync::atomic::AtomicUsize::new(0);
    type Done = Result<Vec<JsVerdict>, String>;
    let results: Vec<std::sync::Mutex<Option<Done>>> =
        chunks.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(chunks.len()) {
            scope.spawn(|| loop {
                let b = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let Some(chunk) = chunks.get(b) else { break };
                let r = run_chunk(compiler, chunk, &root.join(b.to_string()));
                *results[b].lock().expect("result slot") = Some(r);
            });
        }
    });
    let mut out = Vec::new();
    for r in results {
        out.extend(r.into_inner().expect("result slot").expect("every batch ran")?);
    }
    Ok(out)
}

fn run_chunk(compiler: &Path, chunk: &[Case], dir: &Path) -> Result<Vec<JsVerdict>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let prepared: Vec<Result<Ready, JsVerdict>> = chunk
        .iter()
        .enumerate()
        .map(|(i, c)| prepare(c, dir, i))
        .collect();
    let ready: Vec<&Ready> = prepared.iter().filter_map(|p| p.as_ref().ok()).collect();
    let mut lines = if ready.is_empty() {
        Vec::new()
    } else {
        drive(compiler, dir, &ready)?
    }
    .into_iter();
    Ok(prepared
        .into_iter()
        .map(|p| match p {
            Ok(r) => verdict(r.rust, &lines.next().expect("one line per case")),
            Err(v) => v,
        })
        .collect())
}

/// Shrink a case diverging on the JS target: drop events while it still
/// diverges, one driver process per round.
pub fn shrink(compiler: &Path, case: &Case, tag: &str) -> Result<Case, String> {
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
        let verdicts = check(compiler, &candidates, usize::MAX, tag)?;
        match candidates
            .into_iter()
            .zip(verdicts)
            .find(|(_, v)| matches!(v, JsVerdict::Diverge { .. }))
        {
            Some((c, _)) => best = c,
            None => return Ok(best),
        }
    }
}

/// Print every case the JS target did not agree on, keeping a reproduction
/// (shrunk first when `shrinking`), and the summary; whether the run passed.
/// `lean` says, by case, whether the semantics agreed with the runner.
pub fn report(
    compiler: &Path,
    cases: &[Case],
    verdicts: &[JsVerdict],
    lean: Option<&[bool]>,
    shrinking: bool,
    verbose_outside: bool,
) -> Result<bool, String> {
    let (mut agree, mut diverge, mut outside, mut refused, mut errors) = (0, 0, 0, 0, 0);
    let dir = crate::work_dir().join("failures");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (i, (case, v)) in cases.iter().zip(verdicts).enumerate() {
        match v {
            JsVerdict::Agree => agree += 1,
            JsVerdict::Refused(_) => refused += 1,
            JsVerdict::Outside(why) => {
                outside += 1;
                if verbose_outside {
                    println!(
                        "OUTSIDE-JS {}: {}",
                        case.name,
                        why.lines().next().unwrap_or_default()
                    );
                }
            }
            JsVerdict::Error(e) => {
                errors += 1;
                println!("ERROR-JS {}: {e}", case.name);
            }
            JsVerdict::Diverge { at, rust, js } => {
                diverge += 1;
                let (case, rust, js, at) = if shrinking {
                    let c = shrink(compiler, case, &format!("shrink-{i}"))?;
                    match check(compiler, std::slice::from_ref(&c), 1, &format!("shrunk-{i}"))?
                        .pop()
                    {
                        Some(JsVerdict::Diverge { at, rust, js }) => (c, rust, js, at),
                        _ => (case.clone(), rust.clone(), js.clone(), *at),
                    }
                } else {
                    (case.clone(), rust.clone(), js.clone(), *at)
                };
                let stem = dir.join(format!("{}-js-{i}", std::process::id()));
                let events: Vec<String> = case.events.iter().map(Event::label).collect();
                let _ = std::fs::write(stem.with_extension("contract"), &case.source);
                let _ = std::fs::write(stem.with_extension("events"), events.join("\n"));
                let _ = std::fs::write(stem.with_extension("rust.txt"), rust.join("\n"));
                let _ = std::fs::write(stem.with_extension("js.txt"), js.join("\n"));
                let nth = |v: &[String]| {
                    v.iter()
                        .filter(|l| !l.starts_with('#'))
                        .nth(at)
                        .cloned()
                        .unwrap_or_else(|| "<end>".into())
                };
                let why: String = js
                    .iter()
                    .filter(|l| l.starts_with('#'))
                    .take(3)
                    .map(|l| format!("\n  js {l}"))
                    .collect();
                let semantics = match lean.map(|l| l[i]) {
                    Some(true) => "\n  lean: agrees with the runner",
                    Some(false) => "\n  lean: does not agree with the runner either",
                    None => "",
                };
                println!(
                    "DIVERGE-JS {} at line {at}:\n  rust: {}\n  js:   {}{why}{semantics}\n  kept: {}.{{contract,events,rust.txt,js.txt}}",
                    case.name,
                    nth(&rust),
                    nth(&js),
                    stem.display()
                );
            }
        }
    }
    println!(
        "difftest js: {} cases: {agree} agree, {diverge} diverge, {outside} outside the JS target, {refused} refused by the compiler, {errors} driver errors",
        cases.len()
    );
    Ok(diverge == 0 && errors == 0)
}
