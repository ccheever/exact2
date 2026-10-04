//! `difftest verify <file.contract>`: one app against the semantics, for
//! its author (`contract verify` runs it).
//!
//! The program's `test` blocks (its own, and those of an
//! `<stem>.test.contract` beside it) and an explore script run on the
//! runner and the Lean semantics; a divergence names the step where the
//! two first differ, where that step is written, the element it reached
//! and the declaration whose line differs. `--types` runs the Lean type
//! checker on the program; `--prove <Module>` regenerates
//! `semantics/Apps/<Module>.lean` and builds the proofs of it under
//! `semantics/Apps/Proofs/`.

use crate::script::{Case, Event, Scripted};
use crate::{check, compile, expectations, leanrun, oracle, seed_of, work_dir, Verdict};
use contract_syntax::{Span, Step};
use std::path::{Path, PathBuf};
use std::process::Command;

/// What to run besides the differential run.
#[derive(Default)]
pub struct Options {
    /// The Lean type checker on the program.
    pub types: bool,
    /// Regenerate this module's embedding and build its proofs.
    pub prove: Option<String>,
}

/// A case with where each of its events is written, when a test wrote it.
struct Located {
    scripted: Scripted,
    steps: Vec<(PathBuf, Span)>,
}

/// Run everything `options` asks for; whether all of it passed.
pub fn run(file: &Path, options: &Options) -> Result<bool, String> {
    let file = file
        .canonicalize()
        .map_err(|e| format!("{}: {e}", file.display()))?;
    let source = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let map = match contract::compile_path_mapped(&file) {
        Ok((_, map)) => map,
        Err(e) => {
            println!("REFUSED {}: {e}", show(&file));
            return Ok(false);
        }
    };
    leanrun::CACHE.store(true, std::sync::atomic::Ordering::Relaxed);
    leanrun::build()?;
    let mut ok = true;

    let mut located = Vec::new();
    let tests_file = file.with_extension("test.contract");
    for test_file in [file.clone(), tests_file] {
        let Ok(text) = std::fs::read_to_string(&test_file) else {
            continue;
        };
        let tests = match contract::tests(&text) {
            Ok(t) => t,
            Err(e) => {
                println!("SCRIPT {}: {e}", show(&test_file));
                ok = false;
                continue;
            }
        };
        for t in &tests {
            match crate::script::test_case(&show(&test_file), &source, Some(&file), t) {
                Ok(scripted) => located.push(Located {
                    scripted,
                    steps: t
                        .steps
                        .iter()
                        .filter_map(|s| match s {
                            Step::Tap { span, .. }
                            | Step::Type { span, .. }
                            | Step::Clock { span, .. } => Some((test_file.clone(), *span)),
                            _ => None,
                        })
                        .collect(),
                }),
                Err(why) => println!("SKIP {why}"),
            }
        }
    }
    let tests = located.len();
    located.push(Located {
        scripted: crate::explore(&show(&file), &source, Some(&file)),
        steps: Vec::new(),
    });
    // An `expect` is the runner's against the seeded oracle's data, not the
    // app's own: a note, never the verdict.
    for l in &located {
        if let Err(e) = expectations(&l.scripted) {
            println!(
                "NOTE {}: with the oracle's data, {}",
                l.scripted.case.name,
                e.split_once("; the runner shows")
                    .map_or(e.as_str(), |(head, _)| head)
            );
        }
    }
    let cases: Vec<Case> = located.iter().map(|l| l.scripted.case.clone()).collect();
    let outcomes = check(cases, 16, "verify")?;
    let (mut agree, mut diverge, mut outside) = (0, 0, 0);
    let dir = work_dir().join("failures");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (i, (o, l)) in outcomes.iter().zip(&located).enumerate() {
        match &o.verdict {
            Verdict::Agree => agree += 1,
            Verdict::Unsupported(why) => {
                outside += 1;
                println!(
                    "OUTSIDE {}: {why} (semantics/README.md, \"What the semantics leaves out\")",
                    o.case.name
                );
            }
            Verdict::Refused(e) => {
                ok = false;
                println!("REFUSED {}: {e}", o.case.name);
            }
            Verdict::Emit(e) => {
                ok = false;
                println!("EMIT {}: the Lean backend refuses it: {e}", o.case.name);
            }
            Verdict::Expectation(e) => println!("NOTE {}: {e}", o.case.name),
            Verdict::Diverge {
                at,
                rust,
                lean,
                lean_raw,
            } => {
                ok = false;
                diverge += 1;
                let stem = dir.join(format!("{}-verify-{i}", std::process::id()));
                let events: Vec<String> = o.case.events.iter().map(Event::label).collect();
                let _ = std::fs::write(stem.with_extension("contract"), &o.case.source);
                let _ = std::fs::write(stem.with_extension("events"), events.join("\n"));
                let _ = std::fs::write(stem.with_extension("rust.txt"), rust.join("\n"));
                let _ = std::fs::write(stem.with_extension("lean.txt"), lean_raw.join("\n"));
                println!(
                    "DIVERGE {}\n{}  kept: {}.{{contract,events,rust.txt,lean.txt}}",
                    o.case.name,
                    locate(&o.case, &l.steps, &map, *at, rust, lean),
                    stem.display()
                );
            }
        }
    }
    println!(
        "verify: {} cases ({tests} tests, 1 explored): {agree} agree, {diverge} diverge, {outside} outside the semantics",
        outcomes.len()
    );
    if options.types {
        ok &= types(&file)?;
    }
    if let Some(module) = &options.prove {
        ok &= prove(&file, module)?;
    }
    Ok(ok)
}

/// Where a divergence is: the step (and the test line that wrote it), the
/// element it reached, the line that differs and its declaration.
fn locate(
    case: &Case,
    steps: &[(PathBuf, Span)],
    map: &contract::SourceMap,
    at: usize,
    rust: &[String],
    lean: &[String],
) -> String {
    let rust: Vec<&String> = rust.iter().filter(|l| !l.starts_with('#')).collect();
    let step = rust
        .iter()
        .take(at + 1)
        .filter(|l| l.starts_with("== "))
        .count()
        .saturating_sub(1);
    let mut out = String::new();
    let site = |events: &[Event], id: &str| -> Option<String> {
        let plan = compile(case).ok()?;
        let oracle = oracle::Oracle::new(&plan, seed_of(&case.name));
        let node = crate::observe::site(plan, oracle, events, id)?;
        map.node(node).map(|(f, s)| at_span(f, s))
    };
    if step == 0 {
        out.push_str("  at boot\n");
    } else {
        let event = &case.events[step - 1];
        out.push_str(&format!("  at step {step}: {}", event.label()));
        if let Some((f, s)) = steps.get(step - 1) {
            out.push_str(&format!(" ({})", at_span(f, *s)));
        }
        out.push('\n');
        if let Event::Tap(id) | Event::Type(id, _) = event {
            if let Some(where_) = site(&case.events[..step - 1], id) {
                out.push_str(&format!("  the element {id:?} is {where_}\n"));
            }
        }
    }
    let line = rust.get(at).map_or("<end>", |l| l.as_str());
    let mut words = line.split(' ');
    match (words.next(), words.next()) {
        (Some("slot" | "derive"), Some(name)) => {
            if let Some((f, s)) = map.declared(name) {
                out.push_str(&format!("  {name} is declared at {}\n", at_span(f, s)));
            }
        }
        (Some("view"), Some(_)) => {
            let id = line
                .strip_prefix("view \"")
                .and_then(|l| l.split_once("\" "))
                .map(|(id, _)| id);
            if let Some((id, where_)) =
                id.and_then(|id| Some((id, site(&case.events[..step], id)?)))
            {
                out.push_str(&format!("  the element {id:?} that differs is {where_}\n"));
            }
        }
        _ => {}
    }
    out.push_str(&format!(
        "  runner: {}\n  lean:   {}\n",
        readable(line),
        readable(lean.get(at).map_or("<end>", String::as_str))
    ));
    out
}

/// `file:line:col`, the file relative to the working directory.
fn at_span(file: &Path, span: Span) -> String {
    format!("{}:{}:{}", show(file), span.line, span.col)
}

fn show(file: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|d| file.strip_prefix(d).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| file.to_path_buf())
        .display()
        .to_string()
}

/// An observation line with its numbers (`n<bits>`) as numbers.
fn readable(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(i) = rest.find('n') {
        let hex = rest.get(i + 1..i + 17);
        let boundary = i == 0 || !rest.as_bytes()[i - 1].is_ascii_alphanumeric();
        match hex.filter(|h| boundary && h.bytes().all(|b| b.is_ascii_hexdigit())) {
            Some(h) => {
                let v = f64::from_bits(u64::from_str_radix(h, 16).unwrap_or(0));
                out.push_str(&rest[..i]);
                out.push_str(&exact_runner::stdlib::format_number(v));
                rest = &rest[i + 17..];
            }
            None => {
                out.push_str(&rest[..=i]);
                rest = &rest[i + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The Lean checker (`Contract.check`) on the program. The compiler
/// accepted it, so the checker must too.
fn types(file: &Path) -> Result<bool, String> {
    let program =
        contract::lean::lean_path(file, "app").map_err(|e| format!("{}: {e}", show(file)))?;
    let module = format!(
        "import Contract.TypeCheck\nopen Contract\n\nset_option maxRecDepth 100000\n\n{program}\n\
         def main : IO Unit := IO.println s!\"#check {{check app}} {{(checkFailure app).getD \"\"}}\"\n"
    );
    let lines = leanrun::memo(&work_dir(), &module, || {
        crate::types::build_checker()?;
        leanrun::run_module(&module, &work_dir(), "verify-types")
    })?;
    let verdict = lines
        .iter()
        .find_map(|l| l.strip_prefix("#check "))
        .ok_or("the Lean checker printed no verdict")?;
    let (verdict, failure) = verdict.split_once(' ').unwrap_or((verdict, ""));
    if verdict == "true" {
        println!("types: the Lean checker accepts {}", show(file));
        Ok(true)
    } else {
        println!(
            "types: the Lean checker refuses {} at {failure}; the Rust checker accepts it, so one of them is wrong (semantics/README.md, `types`)",
            show(file)
        );
        Ok(false)
    }
}

/// Regenerate `Apps/<module>.lean` and build what is proved of it.
fn prove(file: &Path, module: &str) -> Result<bool, String> {
    if !module.starts_with(|c: char| c.is_ascii_uppercase())
        || !module.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return Err(format!(
            "--prove {module}: a module name is an identifier starting with a capital"
        ));
    }
    let semantics = leanrun::project();
    let embedding = semantics.join("Apps").join(format!("{module}.lean"));
    // The checked-in name, else the module's with a small first letter.
    let name = std::fs::read_to_string(&embedding)
        .ok()
        .and_then(|t| {
            t.lines().find_map(|l| {
                l.strip_prefix("def ")
                    .and_then(|l| l.strip_suffix(" : Contract.Program :="))
                    .map(str::to_string)
            })
        })
        .unwrap_or_else(|| module[..1].to_ascii_lowercase() + &module[1..]);
    let text =
        contract::lean::lean_path(file, &name).map_err(|e| format!("{}: {e}", show(file)))?;
    let text = format!("import Contract\n\n{text}");
    if std::fs::read_to_string(&embedding).ok().as_deref() != Some(text.as_str()) {
        std::fs::write(&embedding, &text).map_err(|e| format!("{}: {e}", embedding.display()))?;
        println!("prove: wrote {} (`def {name}`)", show(&embedding));
    }
    let proofs = semantics.join("Apps/Proofs").join(format!("{module}.lean"));
    let target = if proofs.is_file() {
        format!("Apps.Proofs.{module}")
    } else {
        println!(
            "prove: no {} yet; building the embedding alone. Write the properties there \
             (semantics/README.md, \"Verifying an app\") and add the app to `APPS` in \
             semantics/difftest/src/main.rs so `difftest apps` keeps the embedding current",
            show(&proofs)
        );
        format!("Apps.{module}")
    };
    let out = Command::new(leanrun::lake())
        .args(["build", &target])
        .current_dir(&semantics)
        .output()
        .map_err(|e| format!("lake build: {e} (is Lean installed? see semantics/README.md)"))?;
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let sorry = log.contains("declaration uses 'sorry'");
    if !out.status.success() {
        println!("prove: lake build {target} failed:\n{log}");
        return Ok(false);
    }
    if sorry {
        println!("prove: {target} builds, but a proof uses `sorry`:\n{log}");
        return Ok(false);
    }
    println!("prove: {target} builds; every proof checked");
    Ok(true)
}
