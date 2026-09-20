//! The build CLI's human and structured diagnostics. @ref LLP 1035.005 D2.

use std::{path::Path, process::ExitCode};

const USAGE: &str = "usage: contract build <file.contract> [-o <file.plan>] [--json]";

struct Options<'a> {
    input: &'a str,
    output: Option<&'a str>,
    json: bool,
}

fn options(args: &[String]) -> Option<Options<'_>> {
    let (mut input, mut output, mut json) = (None, None, false);
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" if !json => json = true,
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
    Some(Options {
        input: input?,
        output,
        json,
    })
}

fn error(id: &str, message: String, file: Option<&str>) -> contract::CompileError {
    contract::CompileError {
        pass: "cli",
        id: id.into(),
        message,
        span: contract_syntax::Span::default(),
        file: file.map(Into::into),
        related: Box::new([]),
    }
}

fn report(error: &contract::CompileError, json: bool, code: u8) -> ExitCode {
    if json {
        println!("[{}]", error.to_json());
    } else {
        eprintln!("{error}");
    }
    ExitCode::from(code)
}

pub(super) fn run(args: &[String]) -> ExitCode {
    let Some(Options {
        input,
        output,
        json,
    }) = options(args)
    else {
        return report(
            &error("contract-build-usage", USAGE.into(), None),
            args.iter().any(|arg| arg == "--json"),
            2,
        );
    };
    let plan = match contract::compile_path(Path::new(input)) {
        Ok(plan) => plan,
        Err(error) => return report(&error, json, 1),
    };
    let bytes = plan.encode();
    if let Some(output) = output {
        if let Err(cause) = std::fs::write(output, &bytes) {
            return report(
                &error("contract-output-write", cause.to_string(), Some(output)),
                json,
                1,
            );
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
    }
    ExitCode::SUCCESS
}
