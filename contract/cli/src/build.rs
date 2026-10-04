//! The build CLI's human and structured diagnostics. @ref LLP 1035.005 D2.

use std::{io::Write, path::Path, process::ExitCode};

const USAGE: &str =
    "usage: contract build <file.contract> [-o <file.plan>] [--json] [--map (requires -o)]";

struct Options<'a> {
    input: &'a str,
    output: Option<&'a str>,
    json: bool,
    map: bool,
}

fn options(args: &[String]) -> Option<Options<'_>> {
    let (mut input, mut output, mut json, mut map) = (None, None, false, false);
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" if !json => json = true,
            "--map" if !map => map = true,
            "-o" if output.is_none() => {
                output = Some(args.next()?.as_str());
                if output?.starts_with('-') {
                    return None;
                }
            }
            name if !name.starts_with('-') && input.is_none() => input = Some(name),
            _ => return None,
        }
    }
    if map && output.is_none() {
        return None;
    }
    Some(Options {
        input: input?,
        output,
        json,
        map,
    })
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
    report_all(std::slice::from_ref(error), json, code)
}

/// Every diagnostic: one JSON array, or each on its own lines.
fn report_all(errors: &[contract::CompileError], json: bool, code: u8) -> ExitCode {
    if json {
        let all: Vec<String> = errors.iter().map(|e| e.to_json()).collect();
        println!("[{}]", all.join(","));
    } else {
        for error in errors {
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
        map,
    }) = options(args)
    else {
        return report(
            &error("contract-build-usage", USAGE.into(), None),
            args.iter().any(|arg| arg == "--json"),
            2,
        );
    };
    // The map is always built: it prices each component for the summary.
    let want_map = map;
    let (plan, map) = match contract::compile_path_all(Path::new(input), true) {
        Ok(compiled) => compiled,
        Err(errors) => return report_all(&errors, json, 1),
    };
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
        // A complete JSON value even on success; no prose on either stream.
        println!("[]");
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
