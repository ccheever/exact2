//! App properties and refusal findings under the seeded oracle. Search and
//! shrinking run on the runner before Lean confirms authored scripts, findings
//! and a walk sample. The last output is the reproducing receipt.

use crate::script::{Case, Event, Scripted};
use crate::{check, compile, expectations_on, leanrun, oracle, work_dir, Verdict};
use contract_syntax::Span;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What to run besides the differential run.
pub struct Options {
    /// The Lean type checker on the program.
    pub types: bool,
    /// Run delivery, expectations, the refusal census and coverage without Lean.
    pub runner_only: bool,
    /// Compare all search scripts on the web JS target and check `always`.
    pub js: bool,
    /// Regenerate this module's embedding and build its proofs.
    pub prove: Option<String>,
    /// Number of adaptive walks.
    pub walks: usize,
    /// Maximum chosen events per walk, including landings but excluding automatic clocks.
    pub steps: usize,
    /// Seed for walk selection; defaults to the app path.
    pub walk_seed: Option<u64>,
    /// Walks to confirm with Lean.
    pub confirm: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            types: false,
            runner_only: false,
            js: false,
            prove: None,
            walks: 20,
            steps: 40,
            walk_seed: None,
            confirm: 2,
        }
    }
}

#[derive(Clone)]
struct Located {
    scripted: Scripted,
    steps: Vec<(PathBuf, Span)>,
}
struct Found {
    located: Located,
    observation: Vec<String>,
    predicate: crate::finding::Predicate,
    store_only: bool,
}

/// Search on the runner, shrink its findings, then confirm with the semantics.
pub fn run(file: &Path, options: &Options) -> Result<bool, String> {
    use std::{collections::BTreeSet, time::Instant};
    let started = Instant::now();
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
    let files = test_files(&file);
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        match contract::tests(&text)
            .and_then(|tests| contract::properties(&text).map(|p| (tests, p)))
        {
            Ok((tests, declarations)) => {
                if let Err(errors) = contract::check_properties_path(&file, &declarations) {
                    for e in errors {
                        println!("PROPERTY REFUSED {}:{e}", show(path));
                    }
                    ok = false;
                }
                property_files.extend(declarations.iter().map(|_| path.clone()));
                properties.extend(declarations);
                authored.push((path.clone(), tests));
            }
            Err(e) => {
                println!("SCRIPT {}:{e}", show(path));
                ok = false;
            }
        }
    }
    if !ok {
        return Ok(false);
    }
    let identity = crate::receipt::Identity::new(&file, &source, &files)?;
    let inputs = crate::walk::Inputs::new(
        &source,
        authored.iter().flat_map(|(_, t)| t.iter().cloned()),
    );
    let seed = options
        .walk_seed
        .unwrap_or_else(|| crate::seed_of(&show(&file)));
    println!(
        "undeliverable parser step kinds: {}",
        crate::script::undeliverable_kinds()
    );
    println!("delivered forms: tap press without modifiers; type replacement; clock +N, data, settle, until state (top-level); leading size (no layout); expect (host-only reads omitted)");
    let mut delivery = crate::receipt::Delivery::default();
    let mut located = Vec::new();
    for (test_file, tests) in authored {
        for t in &tests {
            let scripted =
                crate::script::test_case(&show(&test_file), &source, Some(&file), t, Some(&plan))?;
            let d = &scripted.delivery;
            delivery.settle_bounds += d.settle_bounds;
            delivery.until_met += d.until_met;
            if let Some((_, kind, _)) = &d.stopped {
                delivery.until_not_met += usize::from(kind == "clock until (not met)");
                delivery.until_field_paths += usize::from(kind == "clock until (field path)");
            }
            delivery.mutation_events += d.mutation_events;
            delivery.mutation_bounds += d.mutation_bounds;
            let expansion = if let Some(path) = &d.expansion {
                format!("; settle expanded to {} events ({} bounded); test clock {} advances ({} bounded); kept: {}",
                    d.settle_events.unwrap_or(0), d.settle_bounds, d.mutation_events, d.mutation_bounds,
                    path.display())
            } else {
                String::new()
            };
            if let Some((span, kind, why)) = &d.stopped {
                let skip = d.taken == 0 && scripted.case.events.is_empty();
                let summary = format!("{:?} stopped at {kind}", t.name);
                if skip {
                    delivery.skipped.push(summary);
                } else {
                    delivery.partial.push(summary);
                }
                println!(
                    "{} {}: stopped at {}: {kind}: {why}{expansion}",
                    if skip { "SKIP" } else { "PART" },
                    scripted.case.name,
                    at_span(&test_file, *span)
                );
                if skip {
                    continue;
                }
            } else {
                delivery.whole += 1;
                println!("WHOLE {}{expansion}", scripted.case.name);
            }
            let steps = d.events.iter().map(|s| (test_file.clone(), *s)).collect();
            located.push(Located { scripted, steps });
        }
    }
    let tests = located.len();
    located.push(Located {
        scripted: crate::explore(&show(&file), &source, Some(&file)),
        steps: Vec::new(),
    });
    delivery.mutation_events += located[tests].scripted.delivery.mutation_events;
    delivery.mutation_bounds += located[tests].scripted.delivery.mutation_bounds;
    let base = located[tests].scripted.case.clone();
    let mut observations = Vec::new();
    for l in &located {
        let case = &l.scripted.case;
        observations.push(
            crate::observe::run(
                plan.clone(),
                oracle::Oracle::for_case(&plan, case),
                &case.events,
            )
            .0,
        );
    }
    let mut rendered = BTreeSet::new();
    let mut walk_census = crate::census::Census::default();
    let mut rng = crate::rng::Rng::new(seed);
    for i in 0..options.walks {
        let prefix = if i % 2 == 1 && tests > 0 {
            Some(&located[(i / 2) % tests].scripted.case)
        } else {
            None
        };
        let walk = crate::walk::generate(
            &plan,
            &base,
            prefix,
            rng.next_u64(),
            i,
            options.steps,
            &inputs,
        );
        delivery.mutation_events += walk.scripted.delivery.mutation_events;
        delivery.mutation_bounds += walk.scripted.delivery.mutation_bounds;
        rendered.extend(walk.rendered);
        walk_census.observe(&walk.observation);
        observations.push(walk.observation);
        located.push(Located {
            scripted: walk.scripted,
            steps: Vec::new(),
        });
    }
    delivery.walks = options.walks;
    let originals = located.len();
    if plan.slots.iter().any(|s| s.persist) {
        for i in 0..originals {
            let l = &located[i];
            let scripted = crate::stored_variant(&l.scripted);
            let c = &scripted.case;
            let (mut driver, mut lines) =
                crate::observe::Driver::new(plan.clone(), oracle::Oracle::for_case(&plan, c));
            let is_walk = i > tests;
            let record = |driver: &crate::observe::Driver, rendered: &mut BTreeSet<String>| {
                if is_walk {
                    for control in driver.controls() {
                        rendered.extend(control.press.into_iter().chain(control.input));
                    }
                }
            };
            record(&driver, &mut rendered);
            for event in &c.events {
                lines.extend(driver.step(event));
                record(&driver, &mut rendered);
            }
            if is_walk {
                walk_census.observe(&lines);
            }
            observations.push(lines);
            let steps = l.steps.clone();
            located.push(Located { scripted, steps });
            delivery.stores += 1;
        }
    }
    let mut census = crate::census::Census::default();
    let mut totals = crate::property::Report::new(&properties);
    let mut suggestions = crate::suggest::Suggestions::new(&plan, &file);
    let mut found = Vec::new();
    let mut candidates = 0;
    for (i, (l, rust)) in located.iter().zip(&observations).enumerate() {
        if let Err(e) = expectations_on(&l.scripted, rust) {
            println!(
                "NOTE {}: with the oracle's data, {}",
                l.scripted.case.name,
                e.split_once("; the runner shows")
                    .map_or(e.as_str(), |(h, _)| h)
            );
        }
        census.observe(rust);
        totals.merge(&crate::property::evaluate(&properties, rust));
        suggestions.observe(rust);
        for predicate in crate::finding::predicates(&properties, rust) {
            let store_only = i >= originals
                && !observations[..originals]
                    .iter()
                    .any(|o| predicate.holds(&properties, o));
            let shrunk = crate::shrink::runner(
                &plan,
                &l.scripted.case,
                rust,
                |lines| predicate.holds(&properties, lines),
                |lines| suggestions.observe(lines),
            );
            candidates += shrunk.candidates;
            let steps = shrunk
                .kept
                .iter()
                .filter_map(|i| l.steps.get(*i).cloned())
                .collect();
            let scripted = Scripted {
                case: shrunk.case,
                items: Vec::new(),
                delivery: Default::default(),
            };
            found.push(Found {
                located: Located { scripted, steps },
                observation: shrunk.observation,
                predicate,
                store_only,
            });
        }
    }
    ok &= census.passed() && totals.passed();
    let runner_seconds = started.elapsed().as_secs_f64();
    println!(
        "runner: {:.3}s; {} search scripts; {} findings; {candidates} shrink candidates",
        runner_seconds,
        located.len(),
        found.len()
    );
    let mut semantics = crate::receipt::Semantics::default();
    if options.runner_only || !lean_available() {
        semantics.reason = Some(if options.runner_only {
            "--runner-only".into()
        } else {
            "lake is unavailable; install Lean as semantics/README.md, Lean describes".into()
        });
        for f in &found {
            print_finding(f, &properties, &property_files, &plan, &map, None);
        }
    } else {
        let then = Instant::now();
        leanrun::build()?;
        let mut selected: Vec<Located> = located[..tests + 1].to_vec();
        selected.extend(
            located[tests + 1..originals]
                .iter()
                .take(options.confirm)
                .cloned(),
        );
        let finding_start = selected.len();
        selected.extend(found.iter().map(|f| f.located.clone()));
        let cases = selected.iter().map(|l| l.scripted.case.clone()).collect();
        let outcomes = check(cases, 16, "verify")?;
        semantics.run = outcomes.len();
        for (i, (o, l)) in outcomes.iter().zip(&selected).enumerate() {
            if let Some((rust, lean)) = &o.observations {
                suggestions.observe(rust);
                semantics.trace_disagreed |= lean.iter().any(|l| l == "trace disagrees");
                if i >= finding_start {
                    ok &= found[i - finding_start].predicate.holds(&properties, lean);
                    print_finding(
                        &found[i - finding_start],
                        &properties,
                        &property_files,
                        &plan,
                        &map,
                        Some(lean),
                    );
                }
                let matched = crate::property::evaluate(&properties, lean);
                let runner = crate::property::evaluate(&properties, rust);
                for f in &matched.failures {
                    if !runner
                        .failures
                        .iter()
                        .any(|r| r.property == f.property && r.line == f.line && r.step == f.step)
                    {
                        ok = false;
                        totals.counts[f.property][f.line].false_ += 1;
                        print_property(
                            &properties,
                            &property_files,
                            f,
                            l,
                            &map,
                            rust,
                            Some(lean),
                            true,
                        );
                    }
                }
            }
            match &o.verdict {
                Verdict::Agree => semantics.agreed += 1,
                Verdict::Unsupported(why) => {
                    semantics.outside.push(why.clone());
                    println!("OUTSIDE {}: {why}", o.case.name);
                }
                Verdict::Refused(e) | Verdict::Emit(e) => {
                    ok = false;
                    println!("REFUSED {}: {e}", o.case.name);
                }
                Verdict::Expectation(e) => {
                    println!("NOTE {}: {e}", o.case.name);
                }
                Verdict::Diverge { at, rust, lean, .. } => {
                    ok = false;
                    println!(
                        "DIVERGE {}\n{}",
                        o.case.name,
                        locate(&o.case, &l.steps, &map, *at, rust, lean)
                    );
                    let (case, cost) =
                        crate::shrink::divergence(&o.case, "verify-shrink", |lines| {
                            suggestions.observe(lines)
                        })?;
                    semantics.candidates += cost;
                    let shrunk = check(vec![case], 1, "verify-shrunk")?.remove(0);
                    if let Some((rust, lean)) = &shrunk.observations {
                        suggestions.observe(rust);
                        save_case(&shrunk.case, rust, Some(lean), &format!("divergence-{i}"))?;
                    }
                    println!(
                        "  shrunk: {} -> {} events; {cost} two-engine candidates",
                        o.case.events.len(),
                        shrunk.case.events.len()
                    );
                }
            }
        }
        if options.types {
            ok &= types(&file)?;
        }
        if let Some(module) = &options.prove {
            ok &= prove(&file, module)?;
        }
        semantics.seconds = then.elapsed().as_secs_f64();
    }
    let js = if options.js {
        let (passed, summary) = verify_js(&located, &found, &properties, &property_files, &map);
        ok &= passed;
        summary
    } else {
        "not run".into()
    };
    print_properties(&properties, &property_files, &totals);
    println!(
        "commit kinds the trace leaves out: {}",
        crate::observe::OMITTED_KINDS.join(", ")
    );
    println!("world: {}", crate::settle::HOST_WORLD);
    let precision = suggestions.print(&properties, &file)?;
    let never = rendered
        .difference(&walk_census.attempted)
        .cloned()
        .collect();
    crate::receipt::Receipt {
        file: &show(&file),
        options,
        seed,
        identity: &identity,
        delivery: &delivery,
        census: &census,
        plan: &plan,
        never: &never,
        properties: &properties,
        property_files: &property_files,
        report: &totals,
        semantics: &semantics,
        js: &js,
        runner_seconds,
        candidates,
        precision: &precision,
    }
    .print();
    Ok(ok)
}

fn verify_js(
    located: &[Located],
    found: &[Found],
    properties: &[contract_syntax::PropertyDecl],
    files: &[PathBuf],
    map: &contract::SourceMap,
) -> (bool, String) {
    use crate::js::{self, JsVerdict};
    use contract_syntax::PropertyKind;
    let unchecked: usize = properties
        .iter()
        .filter(|p| p.kind != PropertyKind::Always)
        .map(|p| p.lines.len().max(1))
        .sum();
    let scope = format!("{unchecked} lines not checked (after, its frames and commands, and never refuses); no JS commit trace");
    let started = std::time::Instant::now();
    let run = || -> Result<(bool, String), String> {
        let compiler = js::build()?;
        let selected: Vec<_> = located
            .iter()
            .chain(found.iter().map(|f| &f.located))
            .collect();
        let cases: Vec<_> = selected.iter().map(|l| l.scripted.case.clone()).collect();
        let verdicts = js::check(&compiler, &cases, 16, "verify")?;
        let mut passed = js::report(&compiler, &cases, &verdicts, None, false, true)?;
        let always: Vec<_> = properties
            .iter()
            .filter(|p| p.kind == PropertyKind::Always)
            .cloned()
            .collect();
        let always_files: Vec<_> = properties
            .iter()
            .zip(files)
            .filter(|(p, _)| p.kind == PropertyKind::Always)
            .map(|(_, f)| f.clone())
            .collect();
        let mut totals = crate::property::Report::new(&always);
        let (mut agree, mut diverge, mut outside, mut errors) = (0, 0, 0, 0);
        for (l, verdict) in selected.iter().zip(&verdicts) {
            let observation = match verdict {
                JsVerdict::Agree { js } => {
                    agree += 1;
                    Some(js)
                }
                JsVerdict::Diverge { js, .. } => {
                    diverge += 1;
                    Some(js)
                }
                JsVerdict::Outside(_) => {
                    outside += 1;
                    None
                }
                JsVerdict::Refused(_) | JsVerdict::Error(_) => {
                    errors += 1;
                    passed = false;
                    None
                }
            };
            let Some(observation) = observation else {
                continue;
            };
            let report = crate::property::evaluate(&always, observation);
            passed &= report.passed();
            totals.merge(&report);
            for failure in &report.failures {
                let p = &always[failure.property];
                let line = &p.lines[failure.line];
                println!(
                    "PROPERTY {:?} is false on web JS\n  property line: {}\n  script: {}",
                    p.name,
                    at_span(&always_files[failure.property], line.span),
                    l.scripted.case.name
                );
                if failure.step == 0 {
                    println!("  at boot");
                } else {
                    println!(
                        "  at step {}: {}{}",
                        failure.step,
                        l.scripted.case.events[failure.step - 1].label(),
                        l.steps
                            .get(failure.step - 1)
                            .map(|(f, s)| format!(" ({})", at_span(f, *s)))
                            .unwrap_or_default()
                    );
                }
                if let contract_syntax::PropertyForm::Hold { name, .. } = &line.form {
                    if let Some((f, span)) = map.declared(name) {
                        println!("  {name} is declared at {}", at_span(f, span));
                    }
                }
                println!("  js: {}", readable(&failure.observed));
                if let Some(previous) = &failure.previous {
                    println!("  before js: {}", readable(previous));
                }
                let dir = work_dir().join("failures");
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let stem = dir.join(format!(
                    "{}-js-property-{:x}",
                    std::process::id(),
                    crate::seed_of(&l.scripted.case.name)
                ));
                std::fs::write(stem.with_extension("contract"), &l.scripted.case.source)
                    .map_err(|e| e.to_string())?;
                std::fs::write(
                    stem.with_extension("events"),
                    l.scripted
                        .case
                        .events
                        .iter()
                        .map(Event::label)
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .map_err(|e| e.to_string())?;
                std::fs::write(stem.with_extension("js.txt"), observation.join("\n"))
                    .map_err(|e| e.to_string())?;
                println!("  kept: {}.{{contract,events,js.txt}}", stem.display());
            }
        }
        let checked: usize = totals.counts.iter().flatten().map(|c| c.checked).sum();
        let false_: usize = totals.counts.iter().flatten().map(|c| c.false_).sum();
        let counted: usize = totals.counts.iter().flatten().map(|c| c.counted).sum();
        Ok((passed, format!("{agree} of {} scripts agreed; {diverge} diverged; {outside} outside; {errors} errors; always: {checked} line checks, {false_} false, {counted} counted only", cases.len())))
    };
    let (passed, summary) = match run() {
        Ok(result) => result,
        Err(e) => {
            println!("ERROR-JS {e}");
            (
                false,
                format!("did not complete: {}", e.lines().next().unwrap_or(&e)),
            )
        }
    };
    (
        passed,
        format!(
            "{summary}; {scope}; {:.3}s",
            started.elapsed().as_secs_f64()
        ),
    )
}

fn print_finding(
    f: &Found,
    properties: &[contract_syntax::PropertyDecl],
    files: &[PathBuf],
    plan: &exact_plan::Plan,
    map: &contract::SourceMap,
    lean: Option<&[String]>,
) {
    let case = &f.located.scripted.case;
    println!(
        "FINDING {} ({} events, oracle seed 0x{:016x})",
        case.name,
        case.events.len(),
        case.seed.unwrap_or_else(|| crate::seed_of(&case.name))
    );
    if f.store_only {
        println!(
            "  false from a store holding {}",
            readable(&crate::finding::store(plan, case))
        );
    }
    let disagrees = lean.is_some_and(|lines| !f.predicate.holds(properties, lines));
    match &f.predicate {
        crate::finding::Predicate::Property(p, l) => {
            if let Some(failure) = crate::property::evaluate(properties, &f.observation)
                .failures
                .iter()
                .find(|v| v.property == *p && v.line == *l)
            {
                print_property(
                    properties,
                    files,
                    failure,
                    &f.located,
                    map,
                    &f.observation,
                    lean,
                    disagrees,
                );
            }
        }
        crate::finding::Predicate::Refusal { class, action } => {
            if let Some(refusal) = crate::census::Census::default()
                .observe(&f.observation)
                .iter()
                .find(|v| v.class == *class && crate::finding::action_of(&v.line) == *action)
            {
                let location = locate(case, &f.located.steps, map, refusal.at, &f.observation, &[]);
                let location = location.split("  runner:").next().unwrap_or(&location);
                println!(
                    "REFUSED {} ({class}){}:\n{}  {}\n  # {}",
                    case.name,
                    if disagrees {
                        "; DIVERGE (the finding differs between runner and Lean)"
                    } else {
                        ""
                    },
                    location,
                    refusal.line,
                    refusal.reason
                );
                if let Err(e) = save_case(case, &f.observation, lean, "refusal") {
                    println!("  could not keep observation: {e}");
                }
            }
        }
    }
}

fn save_case(
    case: &Case,
    rust: &[String],
    lean: Option<&[String]>,
    tag: &str,
) -> Result<(), String> {
    let dir = work_dir().join("failures");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stem = dir.join(format!(
        "{}-{tag}-{:x}",
        std::process::id(),
        crate::seed_of(&case.name)
    ));
    for (extension, text) in [
        ("contract", case.source.clone()),
        (
            "events",
            case.events
                .iter()
                .map(Event::label)
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        ("rust.txt", rust.join("\n")),
        (
            "case.txt",
            format!(
                "{}\noracle seed: 0x{:016x}\nstored: {}\nlaunch: {:?}",
                case.name,
                case.seed.unwrap_or_else(|| crate::seed_of(&case.name)),
                case.stored,
                case.launch
            ),
        ),
    ] {
        std::fs::write(stem.with_extension(extension), text).map_err(|e| e.to_string())?;
    }
    if let Some(lean) = lean {
        std::fs::write(stem.with_extension("lean.txt"), lean.join("\n"))
            .map_err(|e| e.to_string())?;
    }
    println!(
        "  kept: {}.{{contract,events,rust.txt,case.txt{}}}",
        stem.display(),
        if lean.is_some() { ",lean.txt" } else { "" }
    );
    Ok(())
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
        let input_step = if matches!(event, Event::InputLand) && step > 1 {
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

pub(crate) fn show(file: &Path) -> String {
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
    let tag = format!(
        "property-{}-{}-{}",
        failure.property, failure.line, failure.step
    );
    if let Err(e) = save_case(&located.scripted.case, rust, lean, &tag) {
        println!("  could not keep observation: {e}");
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
