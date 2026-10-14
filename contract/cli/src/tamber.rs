//! `contract tamber check | eval | ir` — an app's design on its own (LLP
//! 1121 D2): the file the compiler reads (`app.json`'s `"tamber"`, else
//! `app.tamber` beside `app.contract`), checked against the schema's roles
//! by the build's one evaluator (`exact-tamber`), its tokens in one app tuple,
//! or its IR (`tamber.json`, generated and never committed).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage:
  contract tamber check <app dir | app.contract | file.tamber>
  contract tamber eval <app dir | app.contract | file.tamber> [--mode <axis>=<value>[,…]]
  contract tamber ir <app dir | app.contract | file.tamber> [-o <tamber.json>]
  (a flag may come before the path or after it)";

pub fn run(args: &[String]) -> ExitCode {
    if matches!(args, [] | [_]) || args.iter().any(|a| a == "--help" || a == "-h") {
        let help = args.iter().any(|a| a == "--help" || a == "-h");
        println!("{USAGE}");
        return if help {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(2)
        };
    }
    // The one path, and a flag with its value before or after it.
    let verb = args[0].as_str();
    let (mut input, mut rest) = (None, Vec::new());
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            flag @ ("--mode" | "-o") if i + 1 < args.len() => {
                rest.push(flag.to_string());
                rest.push(args[i + 1].clone());
                i += 2;
            }
            path if input.is_none() && !path.starts_with('-') => {
                input = Some(path.to_string());
                i += 1;
            }
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
    let rest = rest.as_slice();
    let Some(path) = design(Path::new(&input)) else {
        eprintln!("contract tamber: {input} has no design (an app.tamber, or the file app.json's \"tamber\" names)");
        return ExitCode::from(2);
    };
    let src = match std::fs::read_to_string(&path) {
        Ok(src) => src,
        Err(e) => {
            eprintln!("contract tamber: {}: {e}", path.display());
            return ExitCode::from(2);
        }
    };
    let folding = match exact_tamber::check(&src, &contract_lower::tamber_vocab()) {
        Ok(folding) => folding,
        Err(all) => {
            for d in &all {
                eprintln!("{}:{d}", path.display());
            }
            return ExitCode::from(1);
        }
    };
    match (verb, rest) {
        ("check", []) => {
            let design = &folding.design;
            println!(
                "{}: {} token{}, {} ax{} ({} tuple{}): checked",
                path.display(),
                folding.tokens.len(),
                if folding.tokens.len() == 1 { "" } else { "s" },
                design.axes.len(),
                if design.axes.len() == 1 { "is" } else { "es" },
                folding.tuples,
                if folding.tuples == 1 { "" } else { "s" }
            );
            ExitCode::SUCCESS
        }
        ("eval", []) => eval(&folding, ""),
        ("eval", [flag, modes]) if flag == "--mode" => eval(&folding, modes),
        ("ir", []) => {
            println!("{}", folding.ir());
            ExitCode::SUCCESS
        }
        ("ir", [flag, out]) if flag == "-o" => match std::fs::write(out, folding.ir() + "\n") {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("contract tamber: {out}: {e}");
                ExitCode::from(2)
            }
        },
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// The design `input` names: a `.tamber` file, or an app's (its directory or
/// its `app.contract`), where the compiler finds it.
fn design(input: &Path) -> Option<PathBuf> {
    if input.extension().is_some_and(|e| e == "tamber") {
        return Some(input.to_path_buf());
    }
    let root = if input.is_dir() {
        input.join("app.contract")
    } else {
        input.to_path_buf()
    };
    contract::design_path(&root).filter(|p| p.is_file())
}

/// Each token's value in the tuple `modes` names (`axis=value`, by commas or
/// spaces, each axis once; the rest at their defaults), as the plan carries
/// it.
fn eval(folding: &exact_tamber::Folding, modes: &str) -> ExitCode {
    let mut pins = Vec::new();
    for pin in modes.split([',', ' ']).filter(|p| !p.is_empty()) {
        match pin.split_once('=') {
            Some((axis, value)) if !pins.iter().any(|(a, _)| *a == axis) => {
                pins.push((axis, value))
            }
            _ => {
                eprintln!("contract tamber eval: `{pin}`: a mode is `axis=value`, each axis once");
                return ExitCode::from(2);
            }
        }
    }
    let Some(tuple) = folding.tuple(&pins) else {
        eprintln!("contract tamber eval: `{modes}` names an axis or value the design lacks");
        return ExitCode::from(2);
    };
    for token in &folding.tokens {
        if let Some(value) = folding.value(&token.name, tuple) {
            println!("{} = {}", token.name, value.css());
        }
    }
    ExitCode::SUCCESS
}
