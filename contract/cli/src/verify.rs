//! `contract verify <file.contract> [--types] [--prove <Module>]`: the app
//! against the Lean semantics (semantics/README.md, "Using it day to day").
//! The work is `difftest verify`'s; this runs it through Cargo from the
//! exact2 checkout this binary was built from, so the compiler never
//! depends on the tester and nothing but this command needs Lean.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "usage: contract verify <file.contract> [--types] [--prove <Module>]

Runs the app's `test` blocks (its own and an `app.test.contract` beside it)
and an explore script on the runner and on the Lean semantics, and names
where they first differ. --types runs the Lean type checker on it;
--prove <Module> regenerates semantics/Apps/<Module>.lean and builds the
proofs in semantics/Apps/Proofs/<Module>.lean.";

/// The checkout this binary was built in.
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Whether `lake` runs, from elan's default place or `PATH`.
fn lean_installed() -> bool {
    let elan = std::env::var_os("HOME").map(|h| Path::new(&h).join(".elan/bin/lake"));
    if elan.is_some_and(|l| l.is_file()) {
        return true;
    }
    Command::new("lake")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
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
            "--types" => rest.push(a.clone()),
            "--prove" => match it.next() {
                Some(m) if !m.starts_with('-') => rest.extend([a.clone(), m.clone()]),
                _ => {
                    eprintln!("--prove needs a module name\n{USAGE}");
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
    if !lean_installed() {
        eprintln!(
            "contract verify: Lean is not installed. It runs the app on the Lean semantics; install \
             elan, which fetches the toolchain semantics/lean-toolchain pins on first use:\n  \
             curl -sSfL https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh | sh -s -- -y\n\
             (semantics/README.md, \"Lean\"). The first run then builds the semantics library (minutes, once)."
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
