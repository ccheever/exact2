//! `parity cases` prints the cases as JSON for `parity.html`; `parity check
//! <fixture>` holds the engine to a recorded fixture and prints every
//! disagreement. `host/web/parity.mjs` runs both around a headless browser.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("cases") => {
            println!("{}", exact_web::parity::cases_json());
            ExitCode::SUCCESS
        }
        Some("header") => {
            print!(
                "{}",
                exact_web::parity::fixture_header(args.get(1).map_or("", String::as_str))
            );
            ExitCode::SUCCESS
        }
        Some("check") => {
            let Some(path) = args.get(1) else {
                eprintln!("usage: parity check <fixture>");
                return ExitCode::from(2);
            };
            let fixture = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("{path}: {e}");
                    return ExitCode::from(1);
                }
            };
            let mismatches = exact_web::parity::check(&fixture);
            let samples = fixture.lines().filter(|l| l.starts_with("sample ")).count();
            if mismatches.is_empty() {
                println!(
                    "motion parity: the engine matches the browser on all {samples} samples (within {}; a colour within {:?})",
                    exact_web::parity::TOLERANCE,
                    exact_web::parity::COLOR_TOLERANCE
                );
                ExitCode::SUCCESS
            } else {
                for m in &mismatches {
                    println!("  {m}");
                }
                println!(
                    "motion parity: {} disagreements over {samples} samples",
                    mismatches.len()
                );
                ExitCode::from(1)
            }
        }
        _ => {
            eprintln!("usage: parity cases | header <recorder> | check <fixture>");
            ExitCode::from(2)
        }
    }
}
