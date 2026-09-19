//! `contract build <file.contract> [-o <file.plan>] [--json]` — compile,
//! print a one-line summary, write the bytes; with `--json`, a rejection is
//! one JSON object on stdout. Baking needs the app's data crate and
//! happens in the app's own build (see `apps/caltrain`), not here.
//! `contract fmt [--check | --stdout] <file.contract>` — the canonical form
//! (LLP 1035.005 D1): rewritten in place, printed, or diffed (exit 1 on a
//! difference). Not one of the five checks; nothing runs it unasked.
//! `contract symbols <file.contract>` — every definition and reference as
//! JSON (LLP 1035.005 D2).
//! `contract compat <app-dir> --platform <p> [--target <triple>] [--json]`
//! — the compatibility id (LLP 1030 D3a) the bake writes beside the plan,
//! and with `--json` the inputs it digests, for reading why two differ.
//! `contract types <file.contract> [-o <app.d.ts>]` — generate TypeScript's
//! data-source signatures from the same plan tables the executor reads.

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("build") => build(&args[1..]),
        Some("fmt") => fmt(&args[1..]),
        Some("symbols") => symbols(&args[1..]),
        Some("test") => tests(&args[1..]),
        Some("compat") => compat(&args[1..]),
        Some("types") => types(&args[1..]),
        _ => {
            eprintln!("usage: contract build <file.contract> [-o <file.plan>] [--json] | contract fmt [--check | --stdout] <file.contract> | contract symbols <file.contract> | contract types <file.contract> [-o <app.d.ts>] | contract test <file.test.contract> | contract compat <app-dir> --platform <ios|macos|linux|web> [--target <triple>] [--json]");
            ExitCode::from(2)
        }
    }
}

fn types(args: &[String]) -> ExitCode {
    let valid = matches!(args, [input] if !input.starts_with('-'))
        || matches!(args, [input, flag, output] if !input.starts_with('-') && flag == "-o" && !output.starts_with('-'));
    if !valid {
        eprintln!("usage: contract types <file.contract> [-o <app.d.ts>]");
        return ExitCode::from(2);
    }
    let result = contract::compile_path(Path::new(&args[0]))
        .map_err(|e| e.to_string())
        .and_then(|plan| contract::typescript(&plan));
    match result {
        Ok(declarations) => {
            if let Some(output) = args.get(2) {
                if let Err(error) = std::fs::write(output, declarations) {
                    eprintln!("{output}: {error}");
                    return ExitCode::from(1);
                }
            } else {
                print!("{declarations}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}:{error}", args[0]);
            ExitCode::from(1)
        }
    }
}

/// `contract compat <app-dir> --platform <p> [--target <triple>] [--json]`:
/// the compatibility id of the app built for that platform — the id alone,
/// or with `--json` the id and every input it digests. The grants are the
/// data crate's to declare and cannot be asked here without linking it;
/// the bake, which links it, passes them (`build.rs` of each host crate).
fn compat(args: &[String]) -> ExitCode {
    let Some(dir) = args.first().filter(|a| !a.starts_with("--")) else {
        eprintln!("usage: contract compat <app-dir> --platform <ios|macos|linux|web> [--target <triple>] [--json]");
        return ExitCode::from(2);
    };
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let Some(platform) = flag("--platform") else {
        eprintln!("contract compat: --platform <ios|macos|linux|web> is required");
        return ExitCode::from(2);
    };
    let target = flag("--target")
        .unwrap_or_else(|| format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS));
    let app_dir = Path::new(dir);
    let result = contract::Manifest::read(app_dir)
        .and_then(|m| contract::compatibility_id(app_dir, &platform, &target, &m, None));
    match result {
        Ok(c) => {
            if args.iter().any(|a| a == "--json") {
                print!("{}", c.to_json());
            } else {
                println!("{}", c.id);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("contract compat: {e}");
            ExitCode::from(1)
        }
    }
}

/// `contract test <file.test.contract>`: the tests as JSON for the agent
/// driver (`bun scripts/agent.mjs <host> --test <file>` runs them).
fn tests(args: &[String]) -> ExitCode {
    let Some(input) = args.first() else {
        eprintln!("usage: contract test <file.test.contract>");
        return ExitCode::from(2);
    };
    let src = match std::fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{input}: {e}");
            return ExitCode::from(1);
        }
    };
    match contract::tests(&src) {
        Ok(t) => {
            println!("{}", contract::tests_json(&t));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{input}:{e}");
            ExitCode::from(1)
        }
    }
}

/// `contract fmt [--check | --stdout] <file.contract>`.
fn fmt(args: &[String]) -> ExitCode {
    let check = args.iter().any(|a| a == "--check");
    let stdout = args.iter().any(|a| a == "--stdout");
    let inputs: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();
    let invalid = check && stdout
        || args
            .iter()
            .any(|a| a.starts_with('-') && a != "--check" && a != "--stdout");
    let (Some(input), 1, false) = (inputs.first(), inputs.len(), invalid) else {
        eprintln!("usage: contract fmt [--check | --stdout] <file.contract>");
        return ExitCode::from(2);
    };
    let src = match std::fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{input}: {e}");
            return ExitCode::from(1);
        }
    };
    let formatted = match contract_syntax::fmt::format(&src) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{input}:{e}");
            return ExitCode::from(1);
        }
    };
    if stdout {
        print!("{formatted}");
        return ExitCode::SUCCESS;
    }
    if check {
        let diff = contract::diff::unified(input, &src, &formatted);
        if diff.is_empty() {
            return ExitCode::SUCCESS;
        }
        print!("{diff}");
        return ExitCode::from(1);
    }
    if formatted != src {
        if let Err(e) = std::fs::write(input, formatted) {
            eprintln!("{input}: {e}");
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}

/// `contract symbols <file.contract>`.
fn symbols(args: &[String]) -> ExitCode {
    let (Some(input), 1) = (args.first(), args.len()) else {
        eprintln!("usage: contract symbols <file.contract>");
        return ExitCode::from(2);
    };
    match contract::symbols::symbols_json(Path::new(input)) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{}:{e}", if e.file.is_empty() { input } else { &e.file });
            ExitCode::from(1)
        }
    }
}

fn build(args: &[String]) -> ExitCode {
    let json = args.iter().any(|a| a == "--json");
    let output = args
        .iter()
        .position(|a| a == "-o")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let Some(input) = args
        .iter()
        .find(|a| !a.starts_with('-') && Some(*a) != output.as_ref())
    else {
        eprintln!("usage: contract build <file.contract> [-o <file.plan>] [--json]");
        return ExitCode::from(2);
    };
    let plan = match contract::compile_path(Path::new(input)) {
        Ok(p) => p,
        Err(e) => {
            if json {
                println!("{}", e.to_json(input));
            } else {
                eprintln!("{}:{e}", if e.file.is_empty() { input } else { &e.file });
            }
            return ExitCode::from(1);
        }
    };
    let bytes = plan.encode();
    if !json {
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
    if let Some(out) = output {
        if let Err(e) = std::fs::write(&out, bytes) {
            eprintln!("{out}: {e}");
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}
