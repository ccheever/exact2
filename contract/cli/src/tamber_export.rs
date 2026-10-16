//! `contract tamber export dtcg <app dir | app.contract | file.tamber> [-o
//! <file>]` (LLP 1121 D14, stage 5): the design as DTCG Format 2025.10 with
//! the Resolver Module (`exact_tamber::export`), printed or written to `-o`.
//! Each export keeps what it wrote as the baseline a re-import compares with,
//! in the design's `.exact/` (beside it), whose own `.gitignore` keeps it out
//! of a commit wherever the app's ignore file does not.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str =
    "usage: contract tamber export dtcg <app dir | app.contract | file.tamber> [-o <file>]";

pub fn run(args: &[String]) -> ExitCode {
    let (mut kind, mut input, mut out) = (None, None, None);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" if i + 1 < args.len() => {
                out = Some(args[i + 1].clone());
                i += 2;
            }
            word if !word.starts_with('-') && kind.is_none() => {
                kind = Some(word.to_string());
                i += 1;
            }
            word if !word.starts_with('-') && input.is_none() => {
                input = Some(word.to_string());
                i += 1;
            }
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let (Some(kind), Some(input)) = (kind, input) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    if kind != "dtcg" {
        eprintln!("contract tamber export: no exporter `{kind}` (dtcg)");
        return ExitCode::from(2);
    }
    let Some(path) = crate::tamber::design(Path::new(&input)) else {
        eprintln!("contract tamber export: {input} has no design (an app.tamber, or the file app.json's \"tamber\" names)");
        return ExitCode::from(2);
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("contract tamber export: {}: {e}", path.display());
            return ExitCode::from(2);
        }
    };
    let exported = match export(&text) {
        Ok(e) => e,
        Err(all) => {
            for d in &all {
                eprintln!("{}:{d}", path.display());
            }
            return ExitCode::from(1);
        }
    };
    if let Err(e) = keep(&path, &exported) {
        eprintln!("contract tamber export: the baseline: {e}");
        return ExitCode::from(2);
    }
    match out {
        Some(out) => match std::fs::write(&out, &exported) {
            Ok(()) => {
                eprintln!(
                    "{out}: written; its baseline in {}",
                    baseline(&path).display()
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("contract tamber export: {out}: {e}");
                ExitCode::from(2)
            }
        },
        None => {
            print!("{exported}");
            ExitCode::SUCCESS
        }
    }
}

/// Design `text` as a DTCG export.
pub fn export(text: &str) -> Result<String, Vec<exact_tamber::Diagnostic>> {
    let vocab = contract_lower::tamber_vocab();
    let folding = exact_tamber::check(text, &vocab)?;
    exact_tamber::export::dtcg(&folding, &vocab)
}

/// Where the baseline of the design at `design` is kept.
pub fn baseline(design: &Path) -> PathBuf {
    design
        .parent()
        .unwrap_or(Path::new("."))
        .join(".exact")
        .join(exact_tamber::export::BASELINE)
}

/// Keep `exported` as the baseline, `.exact/` ignored by its own file.
pub fn keep(design: &Path, exported: &str) -> std::io::Result<()> {
    let file = baseline(design);
    let dir = file.parent().expect("in .exact");
    std::fs::create_dir_all(dir)?;
    let ignore = dir.join(".gitignore");
    if !ignore.exists() {
        std::fs::write(&ignore, "*\n")?;
    }
    std::fs::write(file, exported)
}
