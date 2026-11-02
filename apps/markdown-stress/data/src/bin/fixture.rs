//! Print an exact synthetic fixture for opening in the existing Markdown reader.
use markdown_stress_data::fixture::{generate, Profile};
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err(
            "usage: fixture <mixed|paragraph|code|table|blocks> <16384|262144|1048576|4194304>"
                .into(),
        );
    }
    let text = generate(Profile::parse(&args[0])?, args[1].parse()?)?;
    std::io::stdout().lock().write_all(text.as_bytes())?;
    Ok(())
}
