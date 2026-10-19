//! Apply the parser's repairs to a source file, retaining its comments and layout.

use std::process::ExitCode;

pub(super) fn run(args: &[String]) -> ExitCode {
    const USAGE: &str = "usage: contract migrate <file.contract>";
    let input = match args {
        [flag] if matches!(flag.as_str(), "--help" | "-h") => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        [input] if !input.starts_with('-') => input,
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let before = match std::fs::read_to_string(input) {
        Ok(src) => src,
        Err(e) => {
            eprintln!("{input}: {e}");
            return ExitCode::from(1);
        }
    };
    match contract_syntax::migrate::migrate(&before) {
        Ok(after) => {
            if before != after {
                if let Err(e) = std::fs::write(input, after) {
                    eprintln!("{input}: {e}");
                    return ExitCode::from(1);
                }
                println!("{input}: migrated retired spellings");
            } else {
                println!("{input}: no retired spellings");
            }
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for e in errors {
                eprintln!("{input}:{e}");
            }
            ExitCode::from(1)
        }
    }
}
