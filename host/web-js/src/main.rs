//! `exact-web-js js <app.contract | app.plan> -o <dir>` — the JS target's
//! compiler backend: a plan (compiled here, or a baked `app.plan` from
//! `host/web/build.mjs`, whose resources carry their build-time answers)
//! becomes `app.js` (an ES module over `rt.js`) and `app.css` (the static
//! rows, as the live web host's CSS); a Contract's plan is written beside
//! them as `app.plan`.
//!
//! It lives beside the web host rather than in `contract`'s CLI because it
//! reuses the host's element and CSS rules (`exact_web::host::template`),
//! and `exact-web` already depends on `contract`.

mod code;
mod emit;
mod faces;
mod facts;
mod style;

use std::process::ExitCode;

const USAGE: &str = "usage: exact-web-js js <app.contract | app.plan> -o <dir> [--dump]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (input, out, dump) = match args.as_slice() {
        [cmd, input, o, out] if cmd == "js" && o == "-o" => (input, out, false),
        [cmd, input, o, out, d] if cmd == "js" && o == "-o" && d == "--dump" => (input, out, true),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    // The stylesheet is the live host's CSS for any plan: every grammar
    // (animations, clip paths and filters, gradients, drag timelines) is
    // linked, as the render host links them (LLP 1047 D7).
    exact_web::link(exact_web_capabilities::ALL);
    let path = std::path::Path::new(input);
    let plan = if input.ends_with(".plan") {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("{input}: {e}");
                return ExitCode::from(1);
            }
        };
        match exact_plan::Plan::decode(&bytes) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{input}: {e}");
                return ExitCode::from(1);
            }
        }
    } else {
        match contract::compile_path(path) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{input}:{e}");
                return ExitCode::from(1);
            }
        }
    };
    if dump {
        emit::dump(&plan);
    }
    match emit::emit(&plan) {
        Ok(out_files) => {
            let dir = std::path::Path::new(out);
            if let Err(e) = std::fs::create_dir_all(dir) {
                eprintln!("{out}: {e}");
                return ExitCode::from(1);
            }
            for (name, text) in [
                ("app.js", &out_files.js),
                ("app.css", &out_files.css),
                ("names.js", &out_files.names),
                ("pages.json", &out_files.pages),
            ] {
                if let Err(e) = std::fs::write(dir.join(name), text) {
                    eprintln!("{name}: {e}");
                    return ExitCode::from(1);
                }
            }
            // A Contract compiled here is also the plan beside the pages
            // (the build's `app.plan`), so the build runs no second compile.
            if !input.ends_with(".plan") {
                if let Err(e) = std::fs::write(dir.join("app.plan"), plan.encode()) {
                    eprintln!("app.plan: {e}");
                    return ExitCode::from(1);
                }
            }
            if out_files.markdown {
                let _ = std::fs::write(dir.join("markdown.flag"), "");
            }
            if out_files.canvas2d {
                let _ = std::fs::write(dir.join("canvas2d.flag"), "");
            }
            if out_files.motion {
                let _ = std::fs::write(dir.join("motion.flag"), "");
            }
            if out_files.editor {
                let _ = std::fs::write(dir.join("editor.flag"), "");
            }
            if out_files.flow {
                let _ = std::fs::write(dir.join("flow.flag"), "");
            }
            if !out_files.preloads.is_empty() {
                let _ = std::fs::write(dir.join("preloads.html"), &out_files.preloads);
            }
            if let Some(meta) = &out_files.viewport {
                let _ = std::fs::write(dir.join("viewport.txt"), meta);
            }
            for w in &out_files.warnings {
                eprintln!("warning: {w}");
            }
            println!(
                "{out}: app.js {} B, app.css {} B, {} warnings",
                out_files.js.len(),
                out_files.css.len(),
                out_files.warnings.len()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{input}: {e}");
            ExitCode::from(1)
        }
    }
}
