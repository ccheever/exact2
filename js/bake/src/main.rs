//! `exact-js-bake <app-dir> --out <new-generation-dir>`.
use std::io::{BufRead, Write};
use std::path::Path;
use std::process::ExitCode;

/// The source map a development Cargo bake leaves beside its plan
/// (`exact_bake`'s receipt): only when compiling the app's contract again
/// gives the baked plan's own nodes. A drive of the generation (LLP 1116.003)
/// then names Contract lines, as it does on the build that baked them.
fn development_map(app: &Path, plan: &[u8]) -> Option<String> {
    let source = app.join("app.contract").canonicalize().ok()?;
    let (compiled, map) = contract::compile_path_mapped(&source).ok()?;
    let baked = exact_plan::Plan::decode(plan).ok()?;
    (compiled.nodes == baked.nodes).then(|| map.json(plan))
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if let [app, flag] = args.as_slice() {
        if flag == "--serve" {
            return serve(Path::new(app));
        }
    }
    let [app, flag, output] = args.as_slice() else {
        eprintln!("usage: exact-js-bake <app-dir> --out <new-generation-dir> | --serve");
        return ExitCode::from(2);
    };
    if flag != "--out" {
        eprintln!("expected --out <new-generation-dir>");
        return ExitCode::from(2);
    }
    match exact_js_bake::bake(Path::new(app), &exact_js_bake::Tools::default()).and_then(
        |mut baked| {
            baked.source_map = baked
                .source_map
                .or_else(|| development_map(Path::new(app), &baked.plan));
            baked.write_new(Path::new(output))?;
            // stdout is the receipt; the compiler's warnings go beside it.
            for warning in &baked.warnings {
                eprintln!("{warning}");
            }
            println!("{}", baked.receipt);
            Ok(())
        },
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("TypeScript bake: {error}");
            ExitCode::from(1)
        }
    }
}

fn serve(app: &Path) -> ExitCode {
    let mut producer = match exact_js_bake::Producer::new(exact_js_bake::Tools::default()) {
        Ok(producer) => producer,
        Err(error) => {
            eprintln!("TypeScript producer: {error}");
            return ExitCode::from(1);
        }
    };
    let input = std::io::stdin();
    let mut output = std::io::stdout().lock();
    for line in input.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                eprintln!("producer input: {error}");
                return ExitCode::from(1);
            }
        };
        let request: Result<serde_json::Value, _> = serde_json::from_str(&line);
        let id = request
            .as_ref()
            .ok()
            .and_then(|r| r.get("id"))
            .cloned()
            .unwrap_or_default();
        let result = request.map_err(|e| e.to_string()).and_then(|request| {
            let destination = request["out"]
                .as_str()
                .ok_or("request requires an out directory")?;
            // The previous generation's plan and the last Cargo bake's
            // receipt seed the Rust-owned resources (LLP 1027.002 §5 step 0).
            let seed = exact_js_bake::Seed::read(
                request["previous"].as_str().map(Path::new),
                request["receipt"].as_str().map(Path::new),
            )?;
            let baked = producer.bake(app, seed.as_ref())?;
            baked.write_new(Path::new(destination))?;
            let receipt = serde_json::from_str::<serde_json::Value>(&baked.receipt)
                .map_err(|e| e.to_string())?;
            Ok((receipt, design(&baked.plan)?))
        });
        let reply = match result {
            Ok((receipt, design)) => {
                serde_json::json!({"id":id,"ok":true,"receipt":receipt,"design":design})
            }
            Err(error) => {
                serde_json::json!({"id":id,"ok":false,"error":error.chars().take(65536).collect::<String>()})
            }
        };
        if writeln!(output, "{reply}")
            .and_then(|_| output.flush())
            .is_err()
        {
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}

/// The plan's structure digest and design (LLP 1121 stage 4), for the dev
/// loop to send a value-only edit as a batch instead of a generation.
fn design(plan: &[u8]) -> Result<serde_json::Value, String> {
    use exact_runner::design;
    let plan = exact_plan::Plan::decode(plan).map_err(|e| format!("{e:?}"))?;
    let tokens = design::tokens(&plan);
    Ok(serde_json::json!({
        "structure": design::structure(&plan),
        "shape": design::shape(&plan),
        "digest": design::digest(&tokens),
        "tokens": tokens.iter().map(|t| serde_json::json!({"name": t.name, "value": t.value, "slices": t.slices})).collect::<Vec<_>>(),
    }))
}
