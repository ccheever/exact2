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

#[cfg(test)]
mod budget_tests;
mod code;
mod emit;
mod faces;
mod facts;
mod paint;
#[cfg(test)]
mod paint_tests;
mod reads;
mod style;

use std::process::ExitCode;

const USAGE: &str =
    "usage: exact-web-js js <app.contract | app.plan> -o <dir> [--dump] [--sites] [--dev-reload]\n       exact-web-js normalize-grants <file>";

fn main() -> ExitCode {
    // `--sites` (a development build): each element names its plan node, and
    // a Contract compiled here leaves its source map beside the plan.
    let all: Vec<String> = std::env::args().skip(1).collect();
    if let [cmd, file] = all.as_slice() {
        if cmd == "normalize-grants" {
            return match std::fs::read_to_string(file) {
                Ok(spec) => {
                    println!("{}", exact_runner::grants::normalized_json(&spec));
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{file}: {error}");
                    ExitCode::from(1)
                }
            };
        }
    }
    let sites = all.iter().any(|a| a == "--sites");
    let dev_reload = all.iter().any(|a| a == "--dev-reload");
    let args: Vec<String> = all
        .into_iter()
        .filter(|a| a != "--sites" && a != "--dev-reload")
        .collect();
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
    // The dev loop watches every package the Contract reads (LLP 1091 D10),
    // even when this compile fails: the fix may be in the library.
    if dev_reload && !input.ends_with(".plan") {
        let graph = contract::source_graph(path);
        let mut roots: Vec<String> = graph
            .packages
            .iter()
            .map(|package| package.root.display().to_string())
            // The nearest directory that exists, for a file looked for and
            // not found — but only inside a package or a `node_modules`: never
            // the app (already watched) or a directory above it.
            .chain(graph.consulted.iter().filter_map(|consulted| {
                let dir = consulted.ancestors().skip(1).find(|dir| dir.is_dir())?;
                // A consulted manifest's directory is a package even when
                // resolution then refused it.
                let manifest =
                    consulted.ends_with("package.json") && consulted.parent() == Some(dir);
                let inside = manifest
                    || graph.packages.iter().any(|p| dir.starts_with(&p.root))
                    || dir.components().any(|c| c.as_os_str() == "node_modules");
                inside.then(|| dir.display().to_string())
            }))
            .collect();
        roots.sort();
        roots.dedup();
        // Where a `node_modules` that is not there yet would be made: the
        // loop watches these directories (not their trees) for its creation.
        let mut shallow: Vec<String> = graph
            .consulted
            .iter()
            .filter_map(|consulted| {
                let modules = consulted
                    .ancestors()
                    .find(|a| a.file_name().is_some_and(|n| n == "node_modules"))?;
                let parent = modules.parent()?;
                (!modules.exists() && parent.is_dir()).then(|| parent.display().to_string())
            })
            .collect();
        shallow.sort();
        shallow.dedup();
        let list = |dirs: &[String]| {
            dirs.iter()
                .map(|r| format!("{r:?}"))
                .collect::<Vec<_>>()
                .join(",")
        };
        let json = format!(
            "{{\"packages\":[{}],\"shallow\":[{}]}}\n",
            list(&roots),
            list(&shallow)
        );
        let _ = std::fs::create_dir_all(out);
        let _ = std::fs::write(std::path::Path::new(out).join("dev-sources.json"), json);
    }
    let mut map = None;
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
        // Every refusal, each naming its own file (LLP 1054 L9), as `contract build` prints them.
        match contract::compile_path_all(path, sites) {
            Ok((p, m)) => {
                // The native bake's refusals, here too: this build bakes
                // nothing, and a plan every native build refuses must not
                // pass the web loop (files diary F13).
                if let Err(e) = contract::check(&p) {
                    match &m {
                        Some(m) => eprintln!("{}", m.bake_error(&e)),
                        None => eprintln!("{input}: {e}"),
                    }
                    return ExitCode::from(1);
                }
                map = m;
                p
            }
            Err(errors) => {
                for e in errors {
                    eprintln!("{e}");
                }
                return ExitCode::from(1);
            }
        }
    };
    if dump {
        dump_plan(&plan);
    }
    match emit::emit(&plan, sites, dev_reload) {
        Ok(out_files) => {
            let dir = std::path::Path::new(out);
            if let Err(e) = std::fs::create_dir_all(dir) {
                eprintln!("{out}: {e}");
                return ExitCode::from(1);
            }
            for (name, text) in [
                ("app.js", &out_files.js),
                ("paint.js", &out_files.paint),
                ("app.css", &out_files.css),
                ("names.js", &out_files.names),
                ("pages.json", &out_files.pages),
            ] {
                if let Err(e) = std::fs::write(dir.join(name), text) {
                    eprintln!("{name}: {e}");
                    return ExitCode::from(1);
                }
            }
            // What a Rust data module binds with (rust-data.js): the plan
            // without the bake's answers, which the page already has in
            // `app.js` and which can be most of a baked plan's bytes.
            if let Err(e) = std::fs::write(
                dir.join("app.bind.plan"),
                plan.without_compiled_values().encode(),
            ) {
                eprintln!("app.bind.plan: {e}");
                return ExitCode::from(1);
            }
            // What `app.ts` is type-checked against, as the native bake
            // checks it (host/web-js/build.mjs; calendar F9).
            if let Err(e) = contract::typescript(&plan).and_then(|d| {
                std::fs::write(dir.join("app.contract.d.ts"), d).map_err(|e| e.to_string())
            }) {
                eprintln!("app.contract.d.ts: {e}");
                return ExitCode::from(1);
            }
            // A Contract compiled here is also the plan beside the pages
            // (the build's `app.plan`), so the build runs no second compile.
            if !input.ends_with(".plan") {
                let bytes = plan.encode();
                if let Err(e) = std::fs::write(dir.join("app.plan"), &bytes) {
                    eprintln!("app.plan: {e}");
                    return ExitCode::from(1);
                }
                // Keyed by this plan's digest; the driver joins it (LLP 1035.002 D6).
                let map_path = dir.join("app.plan.map.json");
                let _ = std::fs::remove_file(&map_path);
                if let Some(map) = &map {
                    if let Err(e) = std::fs::write(&map_path, map.json(&bytes)) {
                        eprintln!("app.plan.map.json: {e}");
                        return ExitCode::from(1);
                    }
                }
            }
            // A file input, `saveFile` or `share` (files.js, LLP 1069.002,
            // 1069.010 D3, 1069.003).
            {
                use exact_runner::uses::{uses, Capability};
                let u = uses(&plan);
                if [Capability::Picker, Capability::Documents, Capability::Share]
                    .into_iter()
                    .any(|c| u.has(c))
                {
                    let _ = std::fs::write(dir.join("files.flag"), "");
                }
                // `showNotification`/`closeNotification` (notify.js).
                let _ = std::fs::remove_file(dir.join("notify.flag"));
                if u.has(Capability::Notifications) {
                    let _ = std::fs::write(dir.join("notify.flag"), "");
                }
            }
            // Every portable symbol role, which symbols.js loads when a bound
            // source names one the plan's strings don't (ledger diary F10).
            let roles: Vec<String> = exact_kernel::generated::SYMBOL_ROLES
                .iter()
                .filter_map(|r| exact_kernel::generated::symbol(r).map(|s| (r, s)))
                .map(|(r, (_, path, filled))| {
                    format!(
                        "{}:[{},{}]",
                        serde_json::to_string(r).unwrap(),
                        serde_json::to_string(path).unwrap(),
                        filled as u8
                    )
                })
                .collect();
            let roles = format!("export default {{{}}};", roles.join(","));
            if let Err(e) = std::fs::write(dir.join("symbol-roles.js"), roles) {
                eprintln!("symbol-roles.js: {e}");
                return ExitCode::from(1);
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

fn dump_plan(plan: &exact_plan::Plan) {
    use crate::code::{Scope, Uses};
    use exact_kernel::NodeType;
    let mut uses = Uses::default();
    let s = Scope::default();
    let f = |code: exact_plan::Code, uses: &mut Uses, scope: &Scope, params: usize| {
        code::function(plan, plan.code(code), scope, params, uses)
            .unwrap_or_else(|e| format!("<{e}>"))
    };
    eprintln!("router: {:?}", plan.router);
    for (i, r) in plan.slots.iter().enumerate() {
        eprintln!(
            "slot {i} {} = {}",
            plan.str(r.name),
            f(r.init, &mut uses, &s, 0)
        );
    }
    for (i, r) in plan.derives.iter().enumerate() {
        eprintln!(
            "derive {i} {} = {}",
            plan.str(r.name),
            f(r.body, &mut uses, &s, 0)
        );
    }
    for (i, r) in plan.resources.iter().enumerate() {
        eprintln!(
            "resource {i} {} = {}(..{})",
            plan.str(r.name),
            plan.str(r.source),
            r.args.len
        );
    }
    let a = Scope {
        action: true,
        ..Scope::default()
    };
    for (i, r) in plan.actions.iter().enumerate() {
        eprintln!(
            "action {i} {} = {}",
            plan.str(r.name),
            f(r.body, &mut uses, &a, r.params.len as usize)
        );
    }
    for (i, r) in plan.regions.iter().enumerate() {
        eprintln!(
            "region {i} {:?} parent {:?} arm {:?} order {} arms {:?}",
            r.kind, r.parent, r.arm, r.order, r.arms
        );
    }
    for (i, n) in plan.nodes.iter().enumerate() {
        eprintln!(
            "node {i} type {:?} parent {:?} arm {:?} order {} bindings {} handlers {}",
            NodeType::from_wire(n.node_type),
            n.parent,
            n.arm,
            n.order,
            n.bindings.len,
            n.handlers.len
        );
    }
}
