//! `difftest verify <file.contract>`: one app against the semantics, for
//! its author (`contract verify` runs it).
//!
//! The program's `test` blocks (its own, those of an `<stem>.test.contract`
//! beside it and, for an `app.contract`, those of every other
//! `*.test.contract` beside it) and an explore script run on the
//! runner and the Lean semantics; a divergence names the step where the
//! two first differ, where that step is written, the element it reached
//! and the declaration whose line differs. `--types` runs the Lean type
//! checker on the program; `--prove <Module>` regenerates
//! `semantics/Apps/<Module>.lean` and builds the proofs of it under
//! `semantics/Apps/Proofs/`.

use crate::script::{Case, Event, Scripted};
use crate::{check, compile, expectations_on, leanrun, oracle, work_dir, Verdict};
use contract_syntax::Span;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What to run besides the differential run.
#[derive(Default)]
pub struct Options {
    /// The Lean type checker on the program.
    pub types: bool,
    /// Run delivery, expectations, the refusal census and coverage without Lean.
    pub runner_only: bool,
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
    let (plan, map) = match contract::compile_path_mapped(&file) {
        Ok(compiled) => compiled,
        Err(e) => {
            println!("REFUSED {}: {e}", show(&file));
            return Ok(false);
        }
    };
    leanrun::CACHE.store(true, std::sync::atomic::Ordering::Relaxed);
    let mut ok = true;

    let mut property_files = Vec::new();
    let mut properties = Vec::new();
    let mut authored = Vec::new();
    for path in test_files(&file) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        match contract::tests(&text)
            .and_then(|tests| contract::properties(&text).map(|p| (tests, p)))
        {
            Ok((tests, declarations)) => {
                if let Err(errors) = contract::check_properties_path(&file, &declarations) {
                    for e in errors {
                        println!("PROPERTY REFUSED {}:{e}", show(&path));
                    }
                    ok = false;
                }
                property_files.extend(declarations.iter().map(|_| path.clone()));
                properties.extend(declarations);
                authored.push((path, tests));
            }
            Err(e) => {
                println!("SCRIPT {}:{e}", show(&path));
                ok = false;
            }
        }
    }
    if !ok {
        return Ok(false);
    }
    let mut property_totals = crate::property::Report::new(&properties);
    let mut located = Vec::new();
    let (mut whole, mut partial, mut skipped) = (0, 0, 0);
    for (test_file, tests) in authored {
        for t in &tests {
            let scripted =
                crate::script::test_case(&show(&test_file), &source, Some(&file), t, Some(&plan))?;
            let delivery = &scripted.delivery;
            let expansion = delivery
                .settle_events
                .map(|k| {
                    format!(
                        "; settle expanded to {k} events; kept: {}",
                        delivery
                            .expansion
                            .as_ref()
                            .map_or_else(String::new, |p| p.display().to_string())
                    )
                })
                .unwrap_or_default();
            if let Some((span, kind, why)) = &delivery.stopped {
                let verdict = if delivery.taken == 0 && scripted.case.events.is_empty() {
                    skipped += 1;
                    "SKIP"
                } else {
                    partial += 1;
                    "PART"
                };
                println!(
                    "{verdict} {}: stopped at {}: {kind}: {why}{expansion}",
                    scripted.case.name,
                    at_span(&test_file, *span)
                );
                if delivery.taken == 0 && scripted.case.events.is_empty() {
                    continue;
                }
            } else {
                whole += 1;
                println!("WHOLE {}{expansion}", scripted.case.name);
            }
            let steps = delivery
                .events
                .iter()
                .map(|s| (test_file.clone(), *s))
                .collect();
            located.push(Located { scripted, steps });
        }
    }
    let tests = located.len();
    located.push(Located {
        scripted: crate::explore(&show(&file), &source, Some(&file)),
        steps: Vec::new(),
    });
    let mut census = crate::census::Census::default();
    let mut findings = Vec::new();
    for (i, l) in located.iter().enumerate() {
        let case = &l.scripted.case;
        let oracle = oracle::Oracle::for_case(&plan, case);
        let (rust, _) = crate::observe::run(plan.clone(), oracle, &case.events);
        // Expectations use the seeded oracle's data, not the app's own.
        if let Err(e) = expectations_on(&l.scripted, &rust) {
            println!(
                "NOTE {}: with the oracle's data, {}",
                case.name,
                e.split_once("; the runner shows")
                    .map_or(e.as_str(), |(head, _)| head)
            );
        }
        let matched = crate::property::evaluate(&properties, &rust);
        property_totals.merge(&matched);
        for failure in &matched.failures {
            print_property(
                &properties,
                &property_files,
                failure,
                l,
                &map,
                &rust,
                None,
                false,
            );
        }
        findings.push((i, census.observe(&rust), rust));
    }
    for (i, found, rust) in &findings {
        let l = &located[*i];
        for f in found {
            let location = locate(&l.scripted.case, &l.steps, &map, f.at, rust, &[]);
            let location = location.split("  runner:").next().unwrap_or(&location);
            println!(
                "REFUSED {} ({}):\n{}  {}\n  # {}",
                l.scripted.case.name, f.class, location, f.line, f.reason
            );
        }
    }
    ok &= census.passed() && property_totals.passed();
    census.print(&plan);
    println!("world: settle omits host waits on motion, scrolling and hatches, and advances to animation ends");
    println!("verify: {} cases ({tests} tests, 1 explored); blocks: {whole} whole, {partial} partial, {skipped} skipped", located.len());
    if options.runner_only || !lean_available() {
        println!("semantics did not run ({}); install Lean: curl -sSfL https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh | sh -s -- -y", if options.runner_only { "--runner-only" } else { "lake is unavailable" });
        print_properties(&properties, &property_files, &property_totals);
        return Ok(ok);
    }
    leanrun::build()?;
    let cases: Vec<Case> = located.iter().map(|l| l.scripted.case.clone()).collect();
    let outcomes = check(cases, 16, "verify")?;
    let (mut agree, mut diverge, mut outside) = (0, 0, 0);
    let dir = work_dir().join("failures");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (i, (o, l)) in outcomes.iter().zip(&located).enumerate() {
        let mut property_diverged = false;
        if let Some((rust, lean)) = &o.observations {
            let r = crate::property::evaluate(&properties, rust);
            let m = crate::property::evaluate(&properties, lean);
            ok &= m.passed();
            let mut printed = std::collections::BTreeSet::new();
            for f in r.failures.iter().chain(&m.failures) {
                let key = (f.property, f.line, f.step);
                if !printed.insert(key) {
                    continue;
                }
                let differs = r
                    .judged
                    .get(&key)
                    .zip(m.judged.get(&key))
                    .is_some_and(|(a, b)| a != b);
                property_diverged |= differs;
                if !r
                    .failures
                    .iter()
                    .any(|v| (v.property, v.line, v.step) == key)
                {
                    property_totals.counts[f.property][f.line].false_ += 1;
                }
                print_property(
                    &properties,
                    &property_files,
                    f,
                    l,
                    &map,
                    rust,
                    Some(lean),
                    differs,
                );
            }
        }
        if property_diverged {
            ok = false;
        }
        match &o.verdict {
            Verdict::Agree if property_diverged => diverge += 1,
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
        "semantics: {} cases: {agree} agree, {diverge} diverge, {outside} outside the semantics",
        outcomes.len()
    );
    print_properties(&properties, &property_files, &property_totals);
    if options.types {
        ok &= types(&file)?;
    }
    if let Some(module) = &options.prove {
        ok &= prove(&file, module)?;
    }
    Ok(ok)
}

fn lean_available() -> bool {
    Command::new(leanrun::lake())
        .arg("--version")
        .current_dir(leanrun::project())
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The files whose `test` blocks run: the program, `<stem>.test.contract`
/// beside it, and for an app's `app.contract` every other
/// `*.test.contract` beside it in name order (an app's tests split by area
/// under the line cap; scripts/app.mjs `authoredTests`).
fn test_files(file: &Path) -> Vec<PathBuf> {
    let own = file.with_extension("test.contract");
    let mut others = Vec::new();
    if let (Some("app.contract"), Some(dir)) =
        (file.file_name().and_then(|n| n.to_str()), file.parent())
    {
        others = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.to_str().is_some_and(|p| p.ends_with(".test.contract")) && *p != own)
            .collect();
        others.sort();
    }
    [file.to_path_buf(), own]
        .into_iter()
        .chain(others)
        .collect()
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
    let rust_original = rust;
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
        let oracle = oracle::Oracle::for_case(&plan, case);
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
        let input_step = if matches!(event, Event::Land) && step > 1 {
            step - 1
        } else {
            step
        };
        if let Event::Tap(id) | Event::Type(id, _) = &case.events[input_step - 1] {
            if let Some(where_) = site(&case.events[..input_step - 1], id) {
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
    if crate::chunks(rust_original)
        .get(step)
        .is_some_and(|s| s.iter().any(|l| l == crate::observe::OMITTED_COMMIT))
    {
        out.push_str("  a commit the semantics has no branch for\n");
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
        leanrun::run_module_uncached(&module, &work_dir(), "verify-types")
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

fn print_properties(
    properties: &[contract_syntax::PropertyDecl],
    files: &[PathBuf],
    report: &crate::property::Report,
) {
    let (mut held, mut false_, mut counted, mut unexercised) = (0, 0, 0, 0);
    for ((p, file), counts) in properties.iter().zip(files).zip(&report.counts) {
        let bad = counts.iter().any(|c| c.false_ > 0);
        let checked = counts.iter().all(|c| c.checked > 0);
        let verdict = if bad {
            false_ += 1;
            "false"
        } else if checked {
            held += 1;
            "held"
        } else if counts.iter().any(|c| c.counted > 0) {
            counted += 1;
            "counted only"
        } else {
            unexercised += 1;
            "not exercised"
        };
        println!("property {:?}: {verdict}", p.name);
        for (i, c) in counts.iter().enumerate() {
            let span = p.lines.get(i).map_or(p.span, |l| l.span);
            println!(
                "  {}: {} checked, {} counted only, {} violations",
                at_span(file, span),
                c.checked,
                c.counted,
                c.false_
            );
        }
    }
    println!("properties: {held} held, {false_} false, {counted} counted only, {unexercised} not exercised; {} violations, {} counted-only line steps", report.counts.iter().flatten().map(|c| c.false_).sum::<usize>(), report.counts.iter().flatten().map(|c| c.counted).sum::<usize>());
}

#[allow(clippy::too_many_arguments)]
fn print_property(
    properties: &[contract_syntax::PropertyDecl],
    files: &[PathBuf],
    failure: &crate::property::Failure,
    located: &Located,
    map: &contract::SourceMap,
    rust: &[String],
    lean: Option<&[String]>,
    divergence: bool,
) {
    use contract_syntax::PropertyForm;
    let p = &properties[failure.property];
    let line = p.lines.get(failure.line);
    let span = line.map_or(p.span, |l| l.span);
    let value = |observation: &[String], at_step: usize| {
        let steps = crate::chunks(observation);
        let step = steps.get(at_step).copied().unwrap_or(&[]);
        match line.map(|l| &l.form) {
            Some(PropertyForm::Hold { name, .. }) => step
                .iter()
                .find(|l| {
                    l.starts_with(&format!("slot {name} "))
                        || l.starts_with(&format!("derive {name} "))
                })
                .cloned()
                .unwrap_or_else(|| format!("slot {name} <not printed>")),
            Some(PropertyForm::OnlyWrites(_)) => step
                .iter()
                .filter(|l| l.starts_with("slot "))
                .cloned()
                .collect::<Vec<_>>()
                .join("; "),
            Some(PropertyForm::Command { .. }) => steps
                .iter()
                .take(at_step + 1)
                .skip(
                    if matches!(p.kind, contract_syntax::PropertyKind::After(_)) {
                        at_step.saturating_sub(1)
                    } else {
                        at_step
                    },
                )
                .flat_map(|s| s.iter())
                .filter(|l| l.starts_with("command "))
                .cloned()
                .collect::<Vec<_>>()
                .join("; "),
            None => step
                .iter()
                .filter(|l| l.starts_with("ran ") && l.split_whitespace().nth(2) == Some(&p.name))
                .cloned()
                .collect::<Vec<_>>()
                .join("; "),
        }
    };
    let steps = crate::chunks(rust);
    let mut lines: Vec<String> = steps
        .iter()
        .take(failure.step + 1)
        .flat_map(|s| s.iter())
        .filter(|l| !l.starts_with('#'))
        .cloned()
        .collect();
    lines.push(value(rust, failure.step));
    let at = lines.len() - 1;
    let mut other = lines.clone();
    other[at] = lean
        .map(|o| value(o, failure.step))
        .unwrap_or_else(|| "not run".into());
    println!(
        "PROPERTY {:?} is false{}\n  property line: {}\n{}",
        p.name,
        if divergence {
            "; DIVERGE (the property differs between runner and Lean)"
        } else {
            ""
        },
        at_span(&files[failure.property], span),
        locate(
            &located.scripted.case,
            &located.steps,
            map,
            at,
            &lines,
            &other
        )
    );
    if failure.previous.is_some() {
        let back = if matches!(p.kind, contract_syntax::PropertyKind::After(_)) {
            2
        } else {
            1
        };
        if let Some(previous) = failure.step.checked_sub(back) {
            println!("  before runner: {}", readable(&value(rust, previous)));
            if let Some(lean) = lean {
                println!("  before lean:   {}", readable(&value(lean, previous)));
            }
        }
    }
    let dir = work_dir().join("failures");
    let stem = dir.join(format!(
        "{}-property-{:x}-{}-{}-{}",
        std::process::id(),
        crate::seed_of(&located.scripted.case.name),
        failure.property,
        failure.line,
        failure.step
    ));
    let saved = std::fs::create_dir_all(&dir).and_then(|()| {
        std::fs::write(
            stem.with_extension("contract"),
            &located.scripted.case.source,
        )?;
        std::fs::write(
            stem.with_extension("events"),
            located
                .scripted
                .case
                .events
                .iter()
                .map(Event::label)
                .collect::<Vec<_>>()
                .join("\n"),
        )?;
        std::fs::write(stem.with_extension("rust.txt"), rust.join("\n"))?;
        if let Some(lean) = lean {
            std::fs::write(stem.with_extension("lean.txt"), lean.join("\n"))?;
        }
        Ok(())
    });
    match saved {
        Ok(()) => println!(
            "  kept: {}.{{contract,events,rust.txt{}}}",
            stem.display(),
            if lean.is_some() { ",lean.txt" } else { "" }
        ),
        Err(e) => println!("  could not keep observation: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::test_files;

    #[test]
    fn an_app_runs_every_test_file_beside_it_and_another_program_its_own() {
        let dir = std::env::temp_dir().join(format!("difftest-verify-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for f in [
            "app.contract",
            "b.test.contract",
            "app.test.contract",
            "a.test.contract",
        ] {
            std::fs::write(dir.join(f), "").unwrap();
        }
        let names = |program: &str| -> Vec<String> {
            test_files(&dir.join(program))
                .iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect()
        };
        assert_eq!(
            names("app.contract"),
            [
                "app.contract",
                "app.test.contract",
                "a.test.contract",
                "b.test.contract"
            ]
        );
        assert_eq!(
            names("other.contract"),
            ["other.contract", "other.test.contract"]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
