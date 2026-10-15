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
  contract tamber fmt <app dir | app.contract | file.tamber> [--check]
  contract tamber serve --stdio <app dir>   (the design's one writer, tamber/1 over JSON lines)
  (a flag may come before the path or after it)";

pub fn run(args: &[String]) -> ExitCode {
    if args.first().is_some_and(|a| a == "serve") {
        return crate::tamber_serve::run(&args[1..]);
    }
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
            "--check" => {
                rest.push("--check".to_string());
                i += 1;
            }
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
    // A verb takes only its own flag: refused before the design is read.
    let fits = match (verb, rest) {
        ("check", []) | ("eval", []) | ("ir", []) | ("fmt", []) => true,
        ("fmt", [flag]) => flag == "--check",
        ("eval", [flag, _]) => flag == "--mode",
        ("ir", [flag, _]) => flag == "-o",
        _ => false,
    };
    if !fits {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
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
    if verb == "fmt" {
        return fmt(&path, &src, !rest.is_empty());
    }
    let folding = match exact_tamber::check(&src, &contract_lower::tamber_vocab()) {
        Ok(folding) => folding,
        Err(all) => {
            for d in &all {
                eprintln!("{}:{d}", path.display());
            }
            // A profile's colour is the compiler's to resolve, with the
            // declarations in the app's Contract (LLP 1100 D3).
            let lines: Vec<&str> = src.lines().collect();
            let profiled = |d: &exact_tamber::Diagnostic| {
                lines
                    .get(d.line as usize - 1)
                    .is_some_and(|l| l.contains("color(--"))
            };
            if all.iter().all(profiled) {
                eprintln!("contract tamber: a profile's colour (`color(--…)`) is resolved by the compiler with the app's declarations; `contract build` checks this design");
                return ExitCode::from(2);
            }
            return ExitCode::from(1);
        }
    };
    // What the compiler refuses of a design beyond the crate's check (a
    // CSS word as a name, a role that varies by an axis …), as a build would.
    let refused = match admission(&src) {
        Ok(refused) => refused,
        Err(why) => {
            eprintln!("contract tamber: {why}");
            return ExitCode::from(2);
        }
    };
    if !refused.is_empty() {
        for (line, col, id, message) in &refused {
            eprintln!("{}:{line}:{col}: {id}: {message}", path.display());
        }
        return ExitCode::from(1);
    }
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

/// The compiler's refusals of design `src`, read as the design of an app
/// with one empty view and nothing else (no manifest: nothing in one but
/// `"tamber"` reaches a design, and its other files would stop the build
/// first): each `(line, col, id, message)` in the design. An `Err` when the
/// build could not be made, or stopped before the design.
#[allow(clippy::type_complexity)]
fn admission(src: &str) -> Result<Vec<(u32, u32, String, String)>, String> {
    let dir = std::env::temp_dir().join(format!("contract-tamber-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let made = std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::write(dir.join("app.tamber"), src))
        .and_then(|()| {
            std::fs::write(
                dir.join("app.contract"),
                "component App\n  view\n    column\n",
            )
        });
    if let Err(e) = made {
        return Err(format!("{}: {e}", dir.display()));
    }
    let result = contract::compile_path_all(&dir.join("app.contract"), false);
    let _ = std::fs::remove_dir_all(&dir);
    match result {
        Ok(_) => Ok(Vec::new()),
        Err(all) => {
            let (design, other): (Vec<_>, Vec<_>) = all
                .into_iter()
                .partition(|e| e.file.as_ref().is_some_and(|f| f.ends_with("app.tamber")));
            match (design.is_empty(), other.first()) {
                (true, Some(e)) => Err(format!(
                    "the design's build stopped first: [{}] {}",
                    e.id, e.message
                )),
                _ => Ok(design
                    .into_iter()
                    .map(|e| (e.span.line, e.span.col, e.id, e.message))
                    .collect()),
            }
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

/// `tamber fmt` (LLP 1121 §3): the design in its canonical layout, written
/// in place; with `check`, nothing written and exit 1 where it would change.
fn fmt(path: &Path, src: &str, check: bool) -> ExitCode {
    let formatted = match exact_tamber::format(src) {
        Ok(f) => f,
        Err(all) => {
            for d in &all {
                eprintln!("{}:{d}", path.display());
            }
            return ExitCode::from(1);
        }
    };
    if formatted == src {
        return ExitCode::SUCCESS;
    }
    if check {
        eprintln!("{}: not in tamber fmt's layout", path.display());
        return ExitCode::from(1);
    }
    match std::fs::write(path, formatted) {
        Ok(()) => {
            println!("{}: formatted", path.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("contract tamber: {}: {e}", path.display());
            ExitCode::from(2)
        }
    }
}
