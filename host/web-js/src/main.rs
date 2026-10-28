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

mod base_scale;
#[cfg(test)]
mod base_scale_tests;
#[cfg(test)]
mod budget_tests;
mod calls;
mod code;
mod emit;
#[cfg(test)]
mod emit_tests;
mod events;
mod faces;
mod facts;
mod inspect;
#[cfg(test)]
mod inspect_tests;
#[cfg(test)]
mod layout_transition_tests;
mod measures;
#[cfg(test)]
mod measures_tests;
mod modes;
#[cfg(test)]
mod modes_tests;
mod paint;
#[cfg(test)]
mod paint_tests;
mod reads;
mod style;
mod symbols;

use std::process::ExitCode;

const USAGE: &str =
    "usage: exact-web-js js <app.contract | app.plan> -o <dir> [--dump] [--sites] [--dev-reload] [--app <dir>]\n       exact-web-js normalize-grants <file>\n       exact-web-js design <app.contract> <design text file, or - for stdin>\n       exact-web-js design-serve <app.contract>\n       exact-web-js inspect-table <app.plan> <out.json>";

/// `inspect-table <app.plan> <out.json>` (LLP 1122 T2b, the JS target): the
/// plan's inspect table (inspect.rs), made when the dev loop is first asked
/// for it, not by every build: seconds for a large app.
fn inspect_table(plan: &str, out: &str) -> ExitCode {
    exact_web::link(exact_web_capabilities::ALL);
    exact_kernel::style::roles::set_column(exact_kernel::style::roles::Column::Neutral);
    let _every = modes::every(true);
    let written = std::fs::read(plan)
        .map_err(|e| e.to_string())
        .and_then(|bytes| exact_plan::Plan::decode(&bytes).map_err(|e| format!("{e:?}")))
        .and_then(|plan| emit::Sites::new(&plan).and_then(|sites| inspect::table(&plan, &sites)))
        .and_then(|table| std::fs::write(out, table).map_err(|e| e.to_string()));
    match written {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{plan}: {e}");
            ExitCode::from(1)
        }
    }
}

/// `design <app.contract> <file | ->` (LLP 1121 stage 4, commit 6): a scrub's
/// design, the text (`-`: stdin) compiled in place of the app's design file, as a
/// development build's `dev-design.json` says it, on stdout. Nothing is
/// written.
fn scrub_design(input: &std::path::Path, text: &str) -> ExitCode {
    exact_web::link(exact_web_capabilities::ALL);
    exact_kernel::style::roles::set_column(exact_kernel::style::roles::Column::Neutral);
    let read = if text == "-" {
        std::io::read_to_string(std::io::stdin())
    } else {
        std::fs::read_to_string(text)
    };
    let (Some(path), Ok(text)) = (contract::design_path(input), read) else {
        eprintln!("{}: no design, or the text unreadable", input.display());
        return ExitCode::from(1);
    };
    match contract::with_design_text(&path, text, || contract::compile_path_all(input, false)) {
        Ok((plan, _)) => {
            let _every = modes::every(true);
            println!("{}", modes::design(&plan));
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for e in errors {
                eprintln!("{e}");
            }
            ExitCode::from(1)
        }
    }
}

/// `design-serve <app.contract>` (LLP 1121 D13): a scrub's frames, resident.
/// One JSON request a line on stdin, `{id, key, patch: [[name, [css…]]…]}`:
/// the app compiled once with its design file as it is on disk (again when
/// `key`, the shape and digest of the design the dev loop runs, is not the
/// one it holds),
/// each patched row's value set per full app tuple on that plan, answered
/// `{id, digest, structure, tokens}`: the patched design's digest, its
/// structure (`modes::structure`, which the dev loop's scrub refuses a
/// change of) and the rows of the tokens it patched, as `design` gives them.
/// `{id, fallback}` when it cannot patch the frame (the plan it compiled is
/// not `key`'s, or a row would gain or lose its slices), `{id, error}` when
/// the app does not compile. Nothing is written.
fn design_serve(input: &std::path::Path) -> ExitCode {
    use std::io::{BufRead, Write};
    exact_web::link(exact_web_capabilities::ALL);
    exact_kernel::style::roles::set_column(exact_kernel::style::roles::Column::Neutral);
    let _every = modes::every(true);
    let mut held: Option<(String, exact_plan::Plan, modes::Declared)> = None;
    let mut out = std::io::stdout();
    // What this process holds of the wide colour table between frames:
    // nothing (a plan keeps its colours as text).
    let holder = exact_kernel::style::wide::Holder::new();
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let request: serde_json::Value = serde_json::from_str(&line).unwrap_or_default();
        let id = request["id"].clone();
        let key = request["key"].as_str().unwrap_or_default().to_string();
        if held.as_ref().is_none_or(|(k, ..)| *k != key) {
            held = None;
            match contract::compile_path_all(input, false) {
                Ok((plan, _)) => {
                    let tokens = exact_runner::design::tokens(&plan);
                    let digest = format!(
                        "{} {}",
                        exact_runner::design::shape(&plan),
                        exact_runner::design::digest(&tokens)
                    );
                    let declared = modes::Declared::of(&plan);
                    held = Some((digest, plan, declared));
                }
                Err(errors) => {
                    let message: Vec<String> = errors.iter().map(ToString::to_string).collect();
                    let _ = writeln!(
                        out,
                        "{}",
                        serde_json::json!({"id": id, "error": message.join("\n")})
                    );
                    continue;
                }
            }
        }
        let reply = match &held {
            Some((digest, plan, declared)) if *digest == key => {
                let values: Vec<(String, Vec<String>)> =
                    serde_json::from_value(request["patch"].clone()).unwrap_or_default();
                let names: Vec<String> = values.iter().map(|(n, _)| n.clone()).collect();
                patched(plan, &values).map_or_else(
                    || serde_json::json!({"id": id, "fallback": true}),
                    |next| {
                        // The runner's theme-table cache knows a plan by its
                        // buffers, which a patched copy may reuse: started over.
                        let _ = exact_runner::bridge::role_overrides(&next);
                        let tokens = exact_runner::design::tokens(&next);
                        let structure = modes::structure_of(plan, &next);
                        serde_json::json!({"id": id, "digest": exact_runner::design::digest(&tokens), "structure": structure, "tokens": modes::rows(&next, declared, Some(&names))})
                    },
                )
            }
            _ => serde_json::json!({"id": id, "fallback": true}),
        };
        let _ = writeln!(out, "{reply}");
        let _ = out.flush();
        // The frame's wide colours are text in its reply now: this process
        // holds none, so the table may sweep them (a sweep waits for a
        // holder, and this one's empty mark closes the frame's batch).
        holder.publish(exact_kernel::style::wide::WideMark::default());
    }
    ExitCode::SUCCESS
}

/// `plan` with each row of `values` set per full app tuple (the first its
/// value), or none when a row would gain or lose its slices. A name with no
/// row changes nothing.
fn patched(plan: &exact_plan::Plan, values: &[(String, Vec<String>)]) -> Option<exact_plan::Plan> {
    let mut plan = plan.clone();
    for (name, css) in values {
        let Some(i) = plan.theme.iter().position(|r| plan.str(r.name) == name) else {
            continue;
        };
        let first = css.first()?;
        let (start, len) = (
            plan.theme[i].slices.start as usize,
            plan.theme[i].slices.len as usize,
        );
        let varies = css.iter().any(|c| c != first);
        if varies != (len > 0) || varies && css.len() != len {
            return None;
        }
        let mut intern = |text: &str| {
            plan.strings.push(text.to_string());
            exact_plan::StrId(plan.strings.len() as u32 - 1)
        };
        let value = intern(first);
        let slices: Vec<exact_plan::StrId> = if varies {
            css.iter().map(|c| intern(c)).collect()
        } else {
            Vec::new()
        };
        plan.theme[i].value = value;
        for (k, s) in slices.into_iter().enumerate() {
            plan.theme_slices[start + k].value = s;
        }
    }
    Some(plan)
}

fn main() -> ExitCode {
    // `--sites` (a development build): each element names its plan node, and
    // a Contract compiled here leaves its source map beside the plan.
    let all: Vec<String> = std::env::args().skip(1).collect();
    if let [cmd, file] = all.as_slice() {
        if cmd == "design-serve" {
            return design_serve(std::path::Path::new(file));
        }
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
    if let [cmd, input, text] = all.as_slice() {
        if cmd == "design" {
            return scrub_design(std::path::Path::new(input), text);
        }
        if cmd == "inspect-table" {
            return inspect_table(input, text);
        }
    }
    let sites = all.iter().any(|a| a == "--sites");
    let dev_reload = all.iter().any(|a| a == "--dev-reload");
    // `--app <dir>`: the app a baked plan is of, whose design's `exports`
    // bound the declarations' token union (LLP 1121 D7).
    let app = all
        .iter()
        .position(|a| a == "--app")
        .and_then(|i| all.get(i + 1).cloned());
    let mut skip = false;
    let args: Vec<String> = all
        .into_iter()
        .filter(|a| {
            let dropped = skip || a == "--app";
            skip = a == "--app";
            !dropped && a != "--sites" && a != "--dev-reload"
        })
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
    // Direct compiler invocations use the same roles and metrics as web builds.
    exact_kernel::style::roles::set_column(exact_kernel::style::roles::Column::Neutral);
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
        // Entries to watch in a directory without its tree: a `node_modules`
        // not made yet (an install makes it), and an install that is a link
        // (retargeting it is an edit no file inside sees).
        let mut shallow: Vec<(String, String)> = Vec::new();
        for consulted in &graph.consulted {
            if let Some(modules) = consulted
                .ancestors()
                .find(|a| a.file_name().is_some_and(|n| n == "node_modules"))
            {
                if let Some(parent) = modules.parent().filter(|p| !modules.exists() && p.is_dir()) {
                    shallow.push((parent.display().to_string(), "node_modules".into()));
                }
            }
            // Every link on the way to it — the install, a linked
            // node_modules, a linked scope — is one entry of its directory.
            for dir in consulted.ancestors().skip(1) {
                let link = std::fs::symlink_metadata(dir).is_ok_and(|m| m.file_type().is_symlink());
                if let (true, Some(parent), Some(name)) = (link, dir.parent(), dir.file_name()) {
                    shallow.push((
                        parent.display().to_string(),
                        name.to_string_lossy().into_owned(),
                    ));
                }
            }
        }
        shallow.sort();
        shallow.dedup();
        let list = |dirs: &[String]| {
            dirs.iter()
                .map(|r| format!("{r:?}"))
                .collect::<Vec<_>>()
                .join(",")
        };
        let pairs = shallow
            .iter()
            .map(|(dir, name)| format!("[{dir:?},{name:?}]"))
            .collect::<Vec<_>>()
            .join(",");
        // Every source by path: one in a dot directory the app's watcher
        // would otherwise skip is still an input.
        // And every path resolution looked at, as written: a link in a dot
        // directory is watched by its own path.
        let sources: Vec<String> = graph
            .sources
            .iter()
            .filter(|s| s.path.is_absolute())
            .map(|s| s.path.display().to_string())
            .chain(graph.consulted.iter().map(|c| c.display().to_string()))
            .collect();
        let json = format!(
            "{{\"packages\":[{}],\"shallow\":[{}],\"sources\":[{}]}}\n",
            list(&roots),
            pairs,
            list(&sources)
        );
        let _ = std::fs::create_dir_all(out);
        let _ = std::fs::write(std::path::Path::new(out).join("dev-sources.json"), json);
    }
    let mut map = None;
    // The compiler's warnings, as `contract build` prints them: a build says
    // them too, and counts them, or they reach no one (LLP 1116.004 D7).
    let mut warned = Vec::new();
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
                // Another format's plan: most often one a stale `target/debug/contract` wrote (the
                // host tests compile with it), which says nothing on its own.
                let stale = matches!(
                    e,
                    exact_plan::PlanError::FormatDigestMismatch { .. }
                        | exact_plan::PlanError::UnsupportedVersion(_)
                );
                let hint = if stale {
                    " (the plan was written by a compiler built from another plan format, often a stale target/debug/contract: `cargo build -p contract`, then write the plan again)"
                } else {
                    ""
                };
                eprintln!("{input}: {e}{hint}");
                return ExitCode::from(1);
            }
        }
    } else {
        // Every refusal, each naming its own file (LLP 1054 L9), as `contract build` prints them.
        let (compiled, warnings) = contract::compile_path_all_with_warnings(path, sites);
        warned = warnings;
        match compiled {
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
                    eprintln!("{e}{}", contract::excerpt(&e));
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
            // A development build's design, for the dev loop's live table
            // (LLP 1121 stage 4): a value-only edit goes as a batch.
            if dev_reload {
                let _every = modes::every(true);
                if let Err(e) = std::fs::write(
                    dir.join("dev-design.json"),
                    modes::design(&plan).to_string(),
                ) {
                    eprintln!("dev-design.json: {e}");
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
            let contract_path = match (&app, input.ends_with(".plan")) {
                (Some(app), _) => Some(std::path::Path::new(app).join("app.contract")),
                (None, false) => Some(std::path::PathBuf::from(input)),
                (None, true) => None,
            };
            let exports = contract_path.and_then(|p| contract::design_exports(&p));
            // With the widget entries' refresh sources (LLP 1126 D2): the web
            // draws no widget, but app.ts is one module for every host, and
            // its `Sources` names each refresh.
            let declared = match &app {
                Some(app) => contract::snapshot::entries(std::path::Path::new(app))
                    .and_then(|entries| contract::snapshot::source_plan(&entries, &plan)),
                None => Ok(plan.clone()),
            };
            if let Err(e) = declared
                .and_then(|declared| contract::typescript_exporting(&declared, exports.as_deref()))
                .and_then(|d| {
                    std::fs::write(dir.join("app.contract.d.ts"), d).map_err(|e| e.to_string())
                })
            {
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
                // A declared sound or a sound command (sounds.js, LLP 1096
                // D5): the flag holds the table, `[src, frames, rate]` each.
                let _ = std::fs::remove_file(dir.join("sounds.flag"));
                if exact_runner::uses::runs_sounds(&plan) {
                    let table: Vec<_> = plan
                        .sounds
                        .iter()
                        .map(|r| serde_json::json!([plan.str(r.src), r.frames, r.rate]))
                        .collect();
                    let _ = std::fs::write(
                        dir.join("sounds.flag"),
                        serde_json::json!(table).to_string(),
                    );
                }
            }
            // A native module's names, its package views and their extensions
            // (modules.js): the build writes each extension's web
            // implementation beside the app.
            let _ = std::fs::remove_file(dir.join("modules.flag"));
            if !plan.modules.is_empty() || !plan.views.is_empty() {
                let sources: Vec<&str> = (plan.modules.iter())
                    .map(|m| plan.str(plan.sources[m.source.0 as usize].name))
                    .collect();
                let flag = serde_json::json!({ "extensions": facts::extensions(&plan), "sources": sources });
                let _ = std::fs::write(dir.join("modules.flag"), flag.to_string());
            }
            // `wakelock` (wakelock.js, LLP 1125 D11).
            let _ = std::fs::remove_file(dir.join("wakelock.flag"));
            if exact_runner::wakelock::bound(&plan) {
                let _ = std::fs::write(dir.join("wakelock.flag"), "");
            }
            // `orientationlock` (orientationlock.js, LLP 1133.029 D7).
            let _ = std::fs::remove_file(dir.join("orientationlock.flag"));
            if exact_runner::wakelock::bound_prop(&plan, exact_kernel::PropId::Orientationlock) {
                let _ = std::fs::write(dir.join("orientationlock.flag"), "");
            }
            // Every portable symbol role, which symbols.js loads when a bound
            // source names one the plan's strings don't (ledger diary F10).
            let roles = symbols::every();
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
            let (web, rest): (Vec<_>, Vec<_>) = warned.iter().partition(|w| contract::web_only(w));
            for w in rest {
                eprintln!("{w}");
            }
            if let Some(first) = web.first() {
                eprintln!("{}", contract::web_only_line(&first.to_string(), web.len()));
            }
            for w in &out_files.warnings {
                eprintln!("warning: {w}");
            }
            let count = warned.len() + out_files.warnings.len();
            println!(
                "{out}: app.js {} B, app.css {} B, {count} warning{}",
                out_files.js.len(),
                out_files.css.len(),
                if count == 1 { "" } else { "s" }
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            // A refusal at a plan node is said at its source, as `contract build`
            // says an error (LLP 1143 D3). A production build compiled no map,
            // so it compiles one now, on this failing path only.
            if map.is_none() && !input.ends_with(".plan") {
                map = contract::compile_path_all_with_warnings(path, true)
                    .0
                    .ok()
                    .and_then(|(_, m)| m);
            }
            match map.as_ref().and_then(|m| at_source(&e, m)) {
                Some(located) => eprintln!("{located}{}", contract::excerpt(&located)),
                None => eprintln!("{input}: {e}"),
            }
            ExitCode::from(1)
        }
    }
}

/// An emit refusal that names its plan node (`node 12: …`), resolved through
/// the development source map to the declaration and its call sites.
fn at_source(refusal: &str, map: &contract::SourceMap) -> Option<contract::CompileError> {
    let (node, message) = refusal.strip_prefix("node ")?.split_once(": ")?;
    let site = exact_plan::NodesId(node.parse().ok()?);
    let located = map.bake_error(&contract::BakeError::Lint {
        id: "web-target",
        message: message.to_string(),
        site: Some(site),
    });
    located.file.is_some().then_some(located)
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
