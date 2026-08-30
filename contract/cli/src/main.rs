//! `contract build <file.contract> [-o <file.plan>]` — compile, print a
//! one-line summary, write the bytes. Baking needs the app's data crate and
//! happens in the app's own build (see `apps/caltrain`), not here.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("build") => build(&args[1..]),
        Some("test") => tests(&args[1..]),
        _ => {
            eprintln!("usage: contract build <file.contract> [-o <file.plan>] | contract test <file.test.contract>");
            ExitCode::from(2)
        }
    }
}

/// `contract test <file.test.contract>`: the tests as JSON for the agent
/// driver (`node scripts/agent.mjs <host> --test <file>` runs them).
fn tests(args: &[String]) -> ExitCode {
    let Some(input) = args.first() else {
        eprintln!("usage: contract test <file.test.contract>");
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

fn build(args: &[String]) -> ExitCode {
    let Some(input) = args.first() else {
        eprintln!("usage: contract build <file.contract> [-o <file.plan>]");
        return ExitCode::from(2);
    };
    let output = args
        .iter()
        .position(|a| a == "-o")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let plan = match contract::compile_path(std::path::Path::new(input)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{input}:{e}");
            return ExitCode::from(1);
        }
    };
    let bytes = plan.encode();
    println!(
        "{input}: {} slots, {} derives, {} resources, {} actions, {} nodes, {} regions, {} bytes",
        plan.slots.len(),
        plan.derives.len(),
        plan.resources.len(),
        plan.actions.len(),
        plan.nodes.len(),
        plan.regions.len(),
        bytes.len()
    );
    if let Some(out) = output {
        if let Err(e) = std::fs::write(&out, bytes) {
            eprintln!("{out}: {e}");
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}
