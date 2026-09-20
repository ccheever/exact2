//! `contract build <file.contract> [-o <file.plan>] [--json] [--map]` — compile, print a
//! one-line summary, write the bytes. Baking needs the app's data crate and
//! happens in the app's own build (see `apps/caltrain`), not here.
//! `contract compat <app-dir> --platform <p> [--target <triple>] [--json]`
//! — the compatibility id (LLP 1030 D3a) the bake writes beside the plan,
//! and with `--json` the inputs it digests, for reading why two differ.
//! `contract types <file.contract> [-o <app.d.ts>]` — generate TypeScript's
//! data-source signatures from the same plan tables the executor reads.

use std::process::ExitCode;

mod build;
mod diff;

const USAGE: &str = "usage:
  contract build <file.contract> [-o <file.plan>] [--json] [--map (requires -o)]
  contract symbols <file.contract> [--name <exact-name>]
  contract fmt [--check | --stdout] <file.contract>
  contract types <file.contract> [-o <app.d.ts>]
  contract test <file.test.contract>
  contract compat <app-dir> --platform <ios|macos|linux|web> [--target <triple>] [--json]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--help" | "-h") if args.len() == 1 => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("build") => build::run(&args[1..]),
        Some("symbols") => symbols(&args[1..]),
        Some("fmt") => fmt(&args[1..]),
        Some("test") => tests(&args[1..]),
        Some("compat") => compat(&args[1..]),
        Some("types") => types(&args[1..]),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn symbols(args: &[String]) -> ExitCode {
    const USAGE: &str = "usage: contract symbols <file.contract> [--name <exact-name>]";
    let (input, name) = match args {
        [flag] if matches!(flag.as_str(), "--help" | "-h") => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        [input] if !input.starts_with('-') => (input, None),
        [input, flag, name] if !input.starts_with('-') && flag == "--name" => {
            (input, Some(name.as_str()))
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match contract::symbols_json(std::path::Path::new(input), name) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn types(args: &[String]) -> ExitCode {
    const USAGE: &str = "usage: contract types <file.contract> [-o <app.d.ts>]";
    if matches!(args, [flag] if flag == "--help" || flag == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let valid = matches!(args, [input] if !input.starts_with('-'))
        || matches!(args, [input, flag, output] if !input.starts_with('-') && flag == "-o" && !output.starts_with('-'));
    if !valid {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let result = contract::compile_path(std::path::Path::new(&args[0]))
        .map_err(|e| e.to_string())
        .and_then(|plan| contract::typescript(&plan).map_err(|e| format!("{}: {e}", args[0])));
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
            eprintln!("{error}");
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
    const USAGE: &str = "usage: contract compat <app-dir> --platform <ios|macos|linux|web> [--target <triple>] [--json]";
    if matches!(args, [flag] if flag == "--help" || flag == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let Some(dir) = args.first().filter(|a| !a.starts_with("--")) else {
        eprintln!("{USAGE}");
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
    let app_dir = std::path::Path::new(dir);
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
    const USAGE: &str = "usage: contract test <file.test.contract>";
    if matches!(args, [flag] if flag == "--help" || flag == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let Some(input) = args.first() else {
        eprintln!("{USAGE}");
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

/// Explicit formatting, with read-only preview and check modes.
fn fmt(args: &[String]) -> ExitCode {
    const USAGE: &str = "usage: contract fmt [--check | --stdout] <file.contract>";
    if matches!(args, [flag] if flag == "--help" || flag == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let (input, mode) = match args {
        [input] if !input.starts_with('-') => (input, ""),
        [flag, input]
            if matches!(flag.as_str(), "--check" | "--stdout") && !input.starts_with('-') =>
        {
            (input, flag.as_str())
        }
        [input, flag]
            if matches!(flag.as_str(), "--check" | "--stdout") && !input.starts_with('-') =>
        {
            (input, flag.as_str())
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let result = std::fs::read_to_string(input)
        .map_err(|e| e.to_string())
        .and_then(|before| {
            let after = contract_syntax::fmt::format(&before).map_err(|e| e.to_string())?;
            Ok((before, after))
        });
    match result {
        Ok((before, after)) => match mode {
            "--stdout" => {
                print!("{after}");
                ExitCode::SUCCESS
            }
            "--check" if before != after => {
                print!("{}", diff::unified(input, &before, &after));
                ExitCode::from(1)
            }
            "--check" => ExitCode::SUCCESS,
            _ if before == after => ExitCode::SUCCESS,
            _ => match std::fs::write(input, after) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("{input}: {error}");
                    ExitCode::from(1)
                }
            },
        },
        Err(error) => {
            eprintln!("{input}:{error}");
            ExitCode::from(1)
        }
    }
}
