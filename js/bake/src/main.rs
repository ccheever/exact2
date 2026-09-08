//! `exact-js-bake <app-dir> --out <new-generation-dir>`.
use std::io::{BufRead, Write};
use std::path::Path;
use std::process::ExitCode;

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
    match exact_js_bake::bake(Path::new(app), &exact_js_bake::Tools::default()).and_then(|baked| {
        baked.write_new(Path::new(output))?;
        println!("{}", baked.receipt);
        Ok(())
    }) {
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
            let baked = producer.bake(app)?;
            baked.write_new(Path::new(destination))?;
            serde_json::from_str::<serde_json::Value>(&baked.receipt).map_err(|e| e.to_string())
        });
        let reply = match result {
            Ok(receipt) => serde_json::json!({"id":id,"ok":true,"receipt":receipt}),
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
