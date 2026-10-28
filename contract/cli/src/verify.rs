//! `contract verify <file.contract> [--runner-only] [--walks N] [--steps M] [--walk-seed N] [--confirm N] [--types] [--prove <Module>]`: the app
//! against the Lean semantics (semantics/README.md, "Using it day to day").
//! The work is `difftest verify`'s; this runs it through Cargo from the
//! exact2 checkout this binary was built from, so the compiler never
//! depends on the tester and nothing but this command needs Lean.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "usage: contract verify <file.contract> [--runner-only] [--js] [--walks N] [--steps M] [--walk-seed N] [--confirm N] [--types] [--prove <Module>]

Runs delivered test prefixes, an explore script and adaptive walks on the runner,
including seeded stores. Shrinks findings before Lean confirms the tests, explore,
findings and a walk sample. Prints property suggestions and a reproducing receipt.
Defaults: --walks 20 --steps 40 --confirm 2; --walk-seed defaults to the app path.
Without Lean or with --runner-only, the receipt says the semantics did not run.
--js also compares every search script on the web JS target and checks always lines.
--types runs the Lean type checker; --prove <Module> regenerates the embedding
and builds semantics/Apps/Proofs/<Module>.lean.";

/// The checkout this binary was built in.
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn run(args: &[String]) -> ExitCode {
    let mut file = None;
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--help" | "-h" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--types" | "--runner-only" | "--js" => rest.push(a.clone()),
            "--prove" | "--walks" | "--steps" | "--walk-seed" | "--confirm" => match it.next() {
                Some(m) if !m.starts_with('-') => rest.extend([a.clone(), m.clone()]),
                _ => {
                    eprintln!("{a} needs a value\n{USAGE}");
                    return ExitCode::from(2);
                }
            },
            f if !f.starts_with('-') && file.is_none() => file = Some(f.to_string()),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(file) = file else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let file = match Path::new(&file).canonicalize() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{file}: {e}");
            return ExitCode::from(1);
        }
    };
    let root = repository();
    if !root.join("semantics/lakefile.toml").is_file() {
        eprintln!(
            "contract verify: the semantics are not at {} (this binary was built from a checkout that has moved)",
            root.join("semantics").display()
        );
        return ExitCode::from(2);
    }
    let status = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["run", "-q", "-p", "contract-difftest", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .args(["--", "verify"])
        .arg(&file)
        .args(&rest)
        .status();
    match status {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => ExitCode::from(s.code().map_or(1, |c| c.clamp(1, 255) as u8)),
        Err(e) => {
            eprintln!("contract verify: cargo: {e}");
            ExitCode::from(2)
        }
    }
}
