//! `exact-gpu-reflect digest <file.wgsl>…` — each shader's interface digest
//! (LLP 1030 D8), one `<name> <hex>` line per file, so the dev server can
//! classify a `.wgsl` edit as an asset (the interface unchanged) or a
//! rebuild (it moved) without linking naga into Node. A shader that fails to
//! parse or validate is `<name> error <message>` and exit code 1 after every
//! file is reported (every failure in one run).

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(("digest", files)) = args.split_first().map(|(a, b)| (a.as_str(), b)) else {
        eprintln!("usage: exact-gpu-reflect digest <file.wgsl>…");
        return ExitCode::from(2);
    };
    let mut failed = false;
    for file in files {
        let name = std::path::Path::new(file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(file);
        match std::fs::read_to_string(file)
            .map_err(|e| e.to_string())
            .and_then(|source| exact_gpu_reflect::interface_digest(&source))
        {
            Ok(digest) => println!("{name} {digest:016x}"),
            Err(e) => {
                failed = true;
                println!("{name} error {}", e.replace('\n', " "));
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
