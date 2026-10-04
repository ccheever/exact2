//! `contract build <file.contract> [-o <file.plan>] [--json] [--map]` — compile, print a
//! one-line summary, write the bytes. Baking needs the app's data crate and
//! happens in the app's own build (see `apps/caltrain`), not here.
//! `contract types <file.contract> [-o <app.d.ts>]` — generate TypeScript's
//! data-source signatures from the same plan tables the executor reads.
//! `contract rust <file.contract> [-o <shapes.rs>]` — the same shapes as Rust
//! structs with their `Value` conversions, for a data crate's `build.rs`.
//! `contract verify <file.contract>` — the app against the Lean semantics
//! (`verify.rs`; it needs Lean, and nothing else here does).

use std::process::ExitCode;

mod build;
mod diff;
mod verify;
mod vocab;

const USAGE: &str = "usage:
  contract build <file.contract> [-o <file.plan>] [--json] [--map (requires -o)]
  contract symbols <file.contract> [--name <exact-name>]
  contract fmt [--check | --stdout] <file.contract>
  contract types <file.contract> [-o <app.d.ts>]
  contract rust <file.contract> [-o <shapes.rs>]
  contract test <file.test.contract>
  contract lean <file.contract> [--name <ident>] [-o <file.lean>]
  contract verify <file.contract> [--types] [--prove <Module>]
  contract vocab [--json] [<name>]";

#[cfg(windows)]
fn main() -> ExitCode {
    // @ref LLP 1035.005 D2 — the Windows executable's default 1 MiB stack
    // cannot compile the supported 256-site depth in a debug build.
    std::thread::Builder::new()
        .name("contract-cli".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(run)
        .expect("cannot start compiler worker")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

#[cfg(not(windows))]
fn main() -> ExitCode {
    run()
}

fn run() -> ExitCode {
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
        Some("lean") => lean(&args[1..]),
        Some("verify") => verify::run(&args[1..]),
        Some("vocab") => vocab::run(&args[1..]),
        Some("types") => types(&args[1..], "types", "app.d.ts", contract::typescript),
        Some("rust") => types(&args[1..], "rust", "shapes.rs", contract::rust),
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

/// `contract types` (TypeScript) or `contract rust` (Rust structs): the
/// data seam's shapes, derived from the plan.
fn types(
    args: &[String],
    command: &str,
    out: &str,
    generate: fn(&exact_plan::Plan) -> Result<String, String>,
) -> ExitCode {
    let usage = format!("usage: contract {command} <file.contract> [-o <{out}>]");
    if matches!(args, [flag] if flag == "--help" || flag == "-h") {
        println!("{usage}");
        return ExitCode::SUCCESS;
    }
    let valid = matches!(args, [input] if !input.starts_with('-'))
        || matches!(args, [input, flag, output] if !input.starts_with('-') && flag == "-o" && !output.starts_with('-'));
    if !valid {
        eprintln!("{usage}");
        return ExitCode::from(2);
    }
    let result = contract::compile_path(std::path::Path::new(&args[0]))
        .map_err(|e| e.to_string())
        .and_then(|plan| generate(&plan).map_err(|e| format!("{}: {e}", args[0])));
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

/// `contract test <file.test.contract>`: the tests as JSON for the agent
/// driver (`bun scripts/agent.mjs <host> --test <file>` runs them).
fn tests(args: &[String]) -> ExitCode {
    const USAGE: &str = "usage: contract test <file.test.contract>";
    if matches!(args, [flag] if flag == "--help" || flag == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let input = match args {
        [input] if !input.is_empty() && !input.starts_with('-') => input,
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
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

/// `contract lean <file.contract>`: the program as a `Contract.Program`
/// term of the Lean semantics (`semantics/`), after the whole compiler
/// accepts it.
fn lean(args: &[String]) -> ExitCode {
    const USAGE: &str = "usage: contract lean <file.contract> [--name <ident>] [-o <file.lean>]";
    let mut input = None;
    let mut name = "program".to_string();
    let mut output = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--help" | "-h" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--name" => match it.next() {
                Some(n) => name = n.clone(),
                None => {
                    eprintln!("{USAGE}");
                    return ExitCode::from(2);
                }
            },
            "-o" => match it.next() {
                Some(o) => output = Some(o.clone()),
                None => {
                    eprintln!("{USAGE}");
                    return ExitCode::from(2);
                }
            },
            other if !other.starts_with('-') && input.is_none() => input = Some(other.to_string()),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(input) = input else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match contract::lean::lean_path(std::path::Path::new(&input), &name) {
        Ok(text) => {
            let text = format!("import Contract\n\n{text}");
            match output {
                Some(o) => {
                    if let Err(e) = std::fs::write(&o, text) {
                        eprintln!("{o}: {e}");
                        return ExitCode::from(1);
                    }
                }
                None => print!("{text}"),
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
