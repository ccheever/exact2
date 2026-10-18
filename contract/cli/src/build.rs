//! The build CLI's human and structured diagnostics. @ref LLP 1035.005 D2.

use std::{io::Write, path::Path, process::ExitCode};

const USAGE: &str = "usage: contract build <file.contract> [-o <file.plan>] [--json] [--errors-only] [--map (requires -o)] [--entry <kind> (requires -o; no --map)]";

struct Options<'a> {
    input: &'a str,
    output: Option<&'a str>,
    json: bool,
    errors_only: bool,
    map: bool,
    /// A widget entry's kind (LLP 1126 D2): its plan instead of the app's,
    /// compiled as the bake compiles it from `app.json` beside the input.
    entry: Option<&'a str>,
}

fn options(args: &[String]) -> Option<Options<'_>> {
    let (mut input, mut output, mut json, mut map, mut entry) = (None, None, false, false, None);
    let mut errors_only = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" if !json => json = true,
            "--errors-only" if !errors_only => errors_only = true,
            "--map" if !map => map = true,
            "-o" if output.is_none() => {
                output = Some(args.next()?.as_str());
                if output?.starts_with('-') {
                    return None;
                }
            }
            "--entry" if entry.is_none() => {
                entry = Some(args.next()?.as_str());
                if entry?.starts_with('-') {
                    return None;
                }
            }
            name if !name.starts_with('-') && input.is_none() => input = Some(name),
            _ => return None,
        }
    }
    // An entry plan has no source map yet: `--map` with `--entry` is refused.
    if (map || entry.is_some()) && output.is_none() || (map && entry.is_some()) {
        return None;
    }
    Some(Options {
        input: input?,
        output,
        json,
        errors_only,
        map,
        entry,
    })
}

/// `--entry`: the named widget's plan, from `app.json` beside the input.
fn build_entry(input: &str, kind: &str, output: &str, json: bool) -> ExitCode {
    let dir = Path::new(input)
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let manifest = match contract::Manifest::read(dir) {
        Ok(manifest) => manifest,
        Err(message) => {
            return report(
                &error("contract-build-manifest", message, Some(input)),
                json,
                1,
            )
        }
    };
    let entries = match contract::snapshot::compile_widgets(dir, &manifest) {
        Ok(entries) => entries,
        Err(errors) => return report_all(&errors, &[], json, 1),
    };
    let Some(entry) = entries.get(kind) else {
        let message = format!(
            "app.json declares no widget `{kind}`; its kinds are {:?}",
            entries.keys().collect::<Vec<_>>()
        );
        return report(
            &error("contract-build-entry", message, Some(input)),
            json,
            1,
        );
    };
    let bytes = entry.plan.encode();
    if let Err(error) = write_output(output, &bytes) {
        return report(&error, json, 1);
    }
    if json {
        return report_all(&[], &[], true, 0);
    }
    println!(
        "{input}: entry {kind}, {} nodes, {} bytes",
        entry.plan.nodes.len(),
        bytes.len()
    );
    ExitCode::SUCCESS
}

fn error(id: &str, message: String, file: Option<&str>) -> contract::CompileError {
    contract::CompileError {
        pass: "cli",
        id: id.into(),
        message,
        span: contract_syntax::Span::default(),
        file: file.map(|file| Path::new(file).into()),
        related: Box::new([]),
    }
}

fn report(error: &contract::CompileError, json: bool, code: u8) -> ExitCode {
    report_all(std::slice::from_ref(error), &[], json, code)
}

/// Every diagnostic, the refusals and then the warnings (LLP 1116 D6): one
/// JSON array, or each on its own lines.
fn report_all(
    errors: &[contract::CompileError],
    warnings: &[contract::CompileError],
    json: bool,
    code: u8,
) -> ExitCode {
    if json {
        let all: Vec<String> = errors.iter().chain(warnings).map(|e| e.to_json()).collect();
        println!("[{}]", all.join(","));
    } else {
        for error in errors.iter().chain(warnings) {
            eprintln!("{error}");
        }
    }
    ExitCode::from(code)
}

pub(super) fn run(args: &[String]) -> ExitCode {
    if matches!(args, [flag] if flag == "--help" || flag == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let Some(Options {
        input,
        output,
        json,
        errors_only,
        map,
        entry,
    }) = options(args)
    else {
        return report(
            &error("contract-build-usage", USAGE.into(), None),
            args.iter().any(|arg| arg == "--json"),
            2,
        );
    };
    if let (Some(kind), Some(output)) = (entry, output) {
        return build_entry(input, kind, output, json);
    }
    // The map is always built: it prices each component for the summary.
    let want_map = map;
    // A game's surface arguments are checked apart: against a declaration
    // older than the game's Rust they are warnings, never hiding the
    // compile's own refusals (the platformer's diary, R4).
    let surfaces = contract::surface_findings(Path::new(input));
    let mut refused = match &surfaces {
        Ok(s) if s.newer.is_none() => s.findings.clone(),
        Ok(_) => Vec::new(),
        // An unreadable declaration; the compile reports unreadable sources.
        Err(all) => all
            .iter()
            .filter(|e| e.id == "analyze-surface-declaration")
            .cloned()
            .collect(),
    };
    refused.extend(crate::ambient::check(Path::new(input)));
    let clocks = contract::resource_clock_warnings(Path::new(input)).unwrap_or_default();
    if !json && !errors_only {
        for finding in &clocks {
            eprintln!("warning: {finding}");
        }
    }
    // Warnings never fail the build; they follow its refusals (LLP 1116 D6).
    let (compiled, mut warnings) = contract::compile_path_all_warned(Path::new(input), true);
    if let Ok(s) = &surfaces {
        if s.newer.is_some() && !json && !errors_only {
            crate::surface_warnings(s);
        }
    }
    // Under `--json` the advice above joins the array's warnings: stdout is
    // one JSON value and stderr stays empty.
    if json {
        let stale = surfaces.as_ref().ok().filter(|s| s.newer.is_some());
        warnings.extend(
            clocks
                .into_iter()
                .chain(stale.map(|s| s.findings.clone()).unwrap_or_default())
                .map(|e| contract::CompileError {
                    pass: "warning",
                    ..e
                }),
        );
    }
    if errors_only {
        warnings.clear();
    }
    let (plan, map) = match compiled {
        Ok(_) if !refused.is_empty() => return report_all(&refused, &warnings, json, 1),
        Ok(compiled) => compiled,
        Err(mut errors) => {
            errors.extend(refused);
            if !errors
                .iter()
                .any(|e| contract_syntax::migrate::is_retired(&e.id))
            {
                errors.truncate(contract::MAX_DIAGNOSTICS);
            }
            return report_all(&errors, &warnings, json, 1);
        }
    };
    // Existing declarations are an author's data seam. Refresh them from this
    // successful compile, including imported source changes, without creating
    // declarations for builds that never requested TypeScript.
    let declaration = format!("{input}.d.ts");
    if Path::new(&declaration).is_file() {
        let exports = contract::design_exports(Path::new(&input));
        let types = match contract::typescript_exporting(&plan, exports.as_deref()) {
            Ok(types) => types,
            Err(message) => return report(&error("contract-types", message, Some(input)), json, 1),
        };
        if std::fs::read(&declaration).ok().as_deref() != Some(types.as_bytes()) {
            if let Err(error) = write_output(&declaration, types.as_bytes()) {
                return report(&error, json, 1);
            }
            if !json {
                eprintln!("{declaration}: regenerated, since a source's signature changed (the types app.ts imports)");
            }
        }
    }
    let bytes = plan.encode();
    if let Some(output) = output {
        if let Some(map) = map.as_ref().filter(|_| want_map) {
            // Publish the map first. Between these atomic renames a reader
            // may see a digest mismatch, which it must refuse; it can never
            // mistake a partial map or a partial plan for an accepted pair.
            if let Err(error) =
                write_output(&format!("{output}.map.json"), map.json(&bytes).as_bytes())
            {
                return report(&error, json, 1);
            }
        }
        if let Err(error) = write_output(output, &bytes) {
            return report(&error, json, 1);
        }
    }
    if json {
        // A complete JSON value even on success, the warnings its only
        // entries; no prose on either stream.
        return report_all(&[], &warnings, true, 0);
    } else {
        println!(
            "{input}: {} slots, {} derives, {} resources, {} actions, {} nodes, {} regions, {} bytes",
            plan.slots.len(),
            plan.derives.len(),
            plan.resources.len(),
            plan.actions.len(),
            plan.nodes.len(),
            plan.regions.len(),
            bytes.len()
        );
        // Every use is inlined: name what the plan's size is made of.
        if let Some(map) = &map {
            let costs = map.component_costs();
            if !costs.is_empty() {
                let top: Vec<String> = costs
                    .iter()
                    .take(5)
                    .map(|(name, uses, nodes)| format!("{name} {nodes} ({uses}×)"))
                    .collect();
                println!(
                    "  nodes by component, inlined uses included: {}",
                    top.join(", ")
                );
            }
        }
        for warning in &warnings {
            eprintln!("{warning}");
        }
    }
    ExitCode::SUCCESS
}

// A reader observes either complete output. Only a temporary file this
// invocation successfully created is removed on failure.
fn write_output(path: &str, bytes: &[u8]) -> Result<(), contract::CompileError> {
    let temporary = format!("{path}.{}.tmp", std::process::id());
    let refusal =
        |cause: std::io::Error| error("contract-output-write", cause.to_string(), Some(path));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(refusal)?;
    let written = file.write_all(bytes);
    drop(file);
    let result = written.and_then(|_| std::fs::rename(&temporary, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map_err(refusal)
}

#[cfg(test)]
mod options_tests {
    use super::options;

    #[test]
    fn an_entry_takes_an_output_and_no_map() {
        let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(options(&args(&["a.contract", "--entry", "card", "-o", "card.plan"])).is_some());
        assert!(options(&args(&["a.contract", "--entry", "card"])).is_none());
        assert!(options(&args(&[
            "a.contract",
            "--entry",
            "card",
            "--map",
            "-o",
            "card.plan"
        ]))
        .is_none());
    }
}
