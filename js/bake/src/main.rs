//! `exact-js-bake <app-dir> --out <new-generation-dir>`.
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [app, flag, output] = args.as_slice() else {
        eprintln!("usage: exact-js-bake <app-dir> --out <new-generation-dir>");
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
