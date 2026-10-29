//! Package designs on the wire (LLP 1121 D11, stage 11 A2 part 2, *1.14*):
//! a package token's rows name its package and the file its value comes
//! from, the package designs are listed read-only, and the app's override
//! of a package (or base scale) token is one op, `set-token`.

use crate::tamber_serve::digest;
use contract::tamber_params::AppParams;
use contract_syntax::DesignSource;
use exact_tamber::Op;
use serde_json::{json, Value};

/// The package designs, read-only: each its package, its file as a token's
/// identity names it (`@exact/reading/reading.tamber`) and its text's digest.
pub(crate) fn listed(app: &AppParams) -> Value {
    json!(app
        .packages
        .iter()
        .map(|p| json!({"package": p.package, "file": p.home, "digest": digest(p.text.as_bytes())}))
        .collect::<Vec<_>>())
}

/// `out` (a `hello` or `describe` reply) with the package designs listed,
/// where the app has any; with `paths`, each where it is on disk (the local
/// wire's watcher asks, `hello {paths: true}`).
pub(crate) fn packaged(app: &AppParams, out: &mut Value, paths: bool) {
    if app.packages.is_empty() {
        return;
    }
    out["packages"] = listed(app);
    if paths {
        for (entry, p) in out["packages"]
            .as_array_mut()
            .into_iter()
            .flatten()
            .zip(&app.packages)
        {
            entry["path"] = json!(p.path);
        }
    }
}

/// The local wire's watcher's `hello {paths: true}`: each package entry in
/// `out` given `assets`, a digest of the package SVGs the app's build reads
/// (stage 13 round 1, Astra 6: an edit of one changes what `draws` says
/// without touching the design's text), so the watcher announces it as a
/// change of the package's design; and `watch.assets`, the same for each
/// package with no design whose SVGs the build reads (a Contract
/// component's icon param, round 2, R2-4), named `<package>/assets/`.
pub(crate) fn assets(app: &AppParams, out: &mut Value, graph: &contract::SourceGraph) {
    // Never `out["packages"]` written: none listed is no key.
    let listed = out.get_mut("packages").and_then(Value::as_array_mut);
    for (entry, p) in listed.into_iter().flatten().zip(&app.packages) {
        if let Some(root) = p.root.as_deref() {
            entry["assets"] = json!(assets_under(graph, std::path::Path::new(root)));
        }
    }
    let designed: Vec<&str> = app
        .packages
        .iter()
        .filter_map(|p| p.root.as_deref())
        .collect();
    // Each owner of an SVG, there or not (narrow re-check, RE-1).
    let mut others: Vec<(String, std::path::PathBuf)> = graph
        .owners
        .iter()
        .filter(|p| !designed.iter().any(|d| std::path::Path::new(d) == p.root))
        .map(|p| (p.name.clone(), p.root.clone()))
        .collect();
    others.sort();
    let listed: Vec<Value> = others
        .iter()
        .map(|(name, root)| json!({"package": name, "file": format!("{name}/assets/"), "assets": assets_under(graph, root)}))
        .collect();
    out["watch"]["assets"] = json!(listed);
}

/// A digest of the SVGs under `root`'s `assets/` the build reads, each by
/// its path and its bytes as the build reads them, from the file checked
/// to be the package's own (rounds 2 and 3, R2-3, R3-4), one looked for and
/// not there (or leaving) by its path.
fn assets_under(graph: &contract::SourceGraph, root: &std::path::Path) -> String {
    let under = root.join("assets");
    let mut read: Vec<(String, Option<Vec<u8>>)> = graph
        .sources
        .iter()
        .map(|s| (&s.path, true))
        .chain(graph.consulted.iter().map(|c| (c, false)))
        .filter(|(path, _)| path.starts_with(&under))
        .map(|(path, there)| {
            let body = there.then(|| contract_lower::icons::read_confined(path, root).ok());
            (path.display().to_string(), body.flatten())
        })
        .collect();
    read.sort();
    let mut bytes = Vec::new();
    for (path, body) in &read {
        bytes.extend(path.as_bytes());
        bytes.push(0);
        bytes.extend(body.as_deref().unwrap_or(b"(none)"));
        bytes.push(0);
    }
    digest(&bytes)
}

/// What the local wire's watcher watches for the packages' designs
/// (`hello {paths: true}`'s `watch`): each linked package's directory,
/// whole (a design made, renamed by its `package.json`, or in a directory
/// of its own), and the directory each is installed in (a link retargeted,
/// an install replacing it). Package directories only: never a path a
/// `package.json` names outside its package.
pub(crate) fn watched(app_contract: &std::path::Path, graph: &contract::SourceGraph) -> Value {
    // Each package resolved, and each directory a resolution looked for a package.json in and failed
    // past (an export or a manifest missing for now): mending it inside the directory is a change too.
    // And each package an icon's SVG is owned by, there or not, one vendored in the app included
    // (round 3, R3-3; narrow re-check, RE-1).
    let owners = graph.owners.iter().map(|p| p.root.clone());
    let mut roots: Vec<String> = graph
        .packages
        .iter()
        .map(|p| p.root.clone())
        .chain(owners)
        .chain(
            graph
                .consulted
                .iter()
                .filter(|p| {
                    p.file_name().is_some_and(|n| n == "package.json")
                        && p.components().any(|c| c.as_os_str() == "node_modules")
                })
                // Its directory while it stands, a manifest there or not (a link retargeted to one being made).
                .filter(|p| p.parent().is_some_and(std::path::Path::is_dir))
                .filter_map(|p| p.parent()?.canonicalize().ok()),
        )
        .filter(|d| {
            app_contract
                .parent()
                .and_then(|a| a.canonicalize().ok())
                .is_none_or(|a| d != &a)
        })
        .map(|d| d.display().to_string())
        .collect();
    // Each install directory's parent, and theirs up to the app's (a `node_modules` removed and made again).
    let app = app_contract.parent().and_then(|d| d.canonicalize().ok());
    let mut links: Vec<String> = graph
        .consulted
        .iter()
        .filter(|p| {
            p.file_name().is_some_and(|n| n == "package.json")
                && p.components().any(|c| c.as_os_str() == "node_modules")
        })
        // The nearest that is there (one removed is watched for from its parent).
        .filter_map(|p| {
            p.parent()?
                .parent()?
                .ancestors()
                .find_map(|a| a.canonicalize().ok())
        })
        .flat_map(|d| {
            d.ancestors()
                .take_while(|a| {
                    app.as_deref()
                        .is_none_or(|app| a.starts_with(app) && *a != app)
                })
                .map(|a| a.display().to_string())
                .collect::<Vec<_>>()
        })
        .collect();
    for list in [&mut roots, &mut links] {
        list.sort();
        list.dedup();
    }
    // Each install path as the resolution saw it (a link, maybe dangling for now): looked at, not
    // watched, so a target made later is seen.
    let mut installs: Vec<String> = graph
        .consulted
        .iter()
        .filter(|p| {
            p.file_name().is_some_and(|n| n == "package.json")
                && p.components().any(|c| c.as_os_str() == "node_modules")
        })
        .filter_map(|p| p.parent().map(|d| d.display().to_string()))
        .collect();
    installs.sort();
    installs.dedup();
    json!({"roots": roots, "links": links, "installs": installs})
}

/// Refused when one of `files` (the files a write edits, by their names on
/// the wire) is a package's design (D11): by its name, its path, or the file
/// it is (an app design that is a package's), and, whatever the app's load
/// says, any file in a package ([`in_package`]).
pub(crate) fn read_only<'a>(
    app: &AppParams,
    dir: &std::path::Path,
    files: impl Iterator<Item = &'a str>,
) -> Result<(), Value> {
    let real = |f: &str| std::fs::canonicalize(dir.join(f)).ok();
    let app_dir = dir.canonicalize().ok();
    // One real path, shown as `file`: in a package by (a), (b) or a link (in_package), under a root the
    // load resolved (c), or, when the load failed, under any directory with a package.json (fail closed).
    let judge = |file: &str, real_file: &std::path::Path| -> Result<(), Value> {
        let refuse = |why: String| {
            Err(
                json!({"message": format!("`{file}` {why}: not written (D11, read-only); set the token in the app's design")}),
            )
        };
        match in_package(dir, real_file) {
            Err(why) => return refuse(format!("can't be told a package's or not ({why})")),
            Ok(Some(name)) => return refuse(format!("is in the package `{name}`")),
            Ok(None) => {}
        }
        if let Some(root) = app.package_roots.iter().find(|r| {
            real_file.starts_with(r) && !app_dir.as_deref().is_some_and(|a| a.starts_with(r))
        }) {
            return refuse(format!("is in the package at `{}`", root.display()));
        }
        if let (true, Some(a)) = (app.failed, app_dir.as_deref()) {
            if let Some(d) = real_file
                .ancestors()
                .skip(1)
                .take_while(|d| d.starts_with(a) && *d != a)
                .find(|d| d.join("package.json").exists())
            {
                return refuse(format!("is in `{}`, which has a package.json, taken for a package's as the app doesn't load to say whose it is", d.display()));
            }
        }
        Ok(())
    };
    // The server's own state beside the design (`.exact`: a write's temporary files and journal) is
    // judged as the files are.
    if let Ok(state) = dir.join(".exact").canonicalize() {
        judge(".exact", &state.join("tamber.journal"))?;
    }
    for file in files {
        let real_file = real(file);
        if let Some(f) = real_file.as_deref() {
            judge(file, f)?;
        }
        if let Some(p) = app.packages.iter().find(|p| {
            file == p.home || file == p.path || (real_file.is_some() && real_file == real(&p.path))
        }) {
            return Err(json!({"message": format!(
                "`{file}` is the package `{}`'s design, which is read-only (D11): set the token in the app's design",
                p.package.as_deref().unwrap_or_default()
            )}));
        }
    }
    Ok(())
}

/// The package a file is in, as a component's identity finds one (the
/// nearest directory above it, below the app's, whose `package.json` has a
/// name; one under a `node_modules`; or one a link anywhere in a
/// `node_modules`, in the app's tree or above it, points to, a manifest there
/// or not), whatever the app's load says: as `tamber_contract::owned` judges
/// a Contract source. `Err` when there are too many directories to tell
/// (refused, not taken for none).
fn in_package(dir: &std::path::Path, file: &std::path::Path) -> Result<Option<String>, String> {
    const LIMIT: usize = 400_000;
    let Ok(app) = dir.canonicalize() else {
        return Ok(None);
    };
    // A directory listed: none when it isn't there; one that can't be listed (or an entry read) refuses.
    let read = |d: &std::path::Path| -> Result<Vec<std::fs::DirEntry>, String> {
        match std::fs::read_dir(d) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(format!("{} can't be listed: {e}", d.display())),
            Ok(entries) => entries
                .map(|e| e.map_err(|e| format!("{}: {e}", d.display())))
                .collect(),
        }
    };
    // Every directory in the app's tree and each node_modules above it, links not followed: each link
    // inside a node_modules (at any depth) is a package's root, and any directory under one a package's.
    let mut stack: Vec<(std::path::PathBuf, bool)> = vec![(app.clone(), false)];
    stack.extend(
        app.ancestors()
            .skip(1)
            .map(|a| (a.join("node_modules"), true)),
    );
    let mut seen = 0;
    while let Some((d, modules)) = stack.pop() {
        seen += 1;
        if seen > LIMIT {
            return Err(format!("more than {LIMIT} directories to look through"));
        }
        for e in read(&d)? {
            let kind = e
                .file_type()
                .map_err(|err| format!("{}: {err}", e.path().display()))?;
            let name = e.file_name();
            let path = e.path();
            if kind.is_symlink() {
                if modules {
                    // A link back to the app or above it (a workspace's own) is no package.
                    if let Ok(root) = path.canonicalize() {
                        if !app.starts_with(&root) && file.starts_with(&root) {
                            return Ok(Some(root.display().to_string()));
                        }
                    }
                }
            } else if kind.is_dir() && !matches!(name.to_str(), Some(".git" | "target" | ".exact"))
            {
                stack.push((path, modules || name == "node_modules"));
            }
        }
    }
    for d in file.ancestors().skip(1) {
        if app.starts_with(d) {
            break;
        }
        // A manifest there that can't be read refuses (fail closed); one not there is none.
        let text = match std::fs::read_to_string(d.join("package.json")) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(format!("{}: {e}", d.join("package.json").display())),
            Ok(t) => Some(t),
        };
        let named = text
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|m| m["name"].as_str().map(str::to_string));
        if let Some(name) = named {
            return Ok(Some(name));
        }
    }
    Ok(file
        .components()
        .any(|c| c.as_os_str() == "node_modules")
        .then(|| "node_modules".to_string()))
}

/// Whether one of the app's package designs defines the token `name`.
pub(crate) fn defines_package_token(app: &AppParams, name: &str) -> bool {
    owner(app, name, false).is_some()
}

/// The package design that defines the token `name`, and its line for it;
/// with `icons`, an icon of that name too (stage 13): what a row says of
/// it, where a write's ops take tokens alone (no op writes an icon).
fn owner<'a>(
    app: &'a AppParams,
    name: &str,
    icons: bool,
) -> Option<(&'a DesignSource, exact_tamber::Token)> {
    app.packages.iter().find_map(|p| {
        let design = exact_tamber::parse(&p.text).ok()?;
        let icons = design.icons.into_iter().filter(|_| icons);
        let token = design
            .tokens
            .into_iter()
            .chain(icons)
            .find(|t| t.name.value == name)?;
        Some((p, token))
    })
}

/// Whether the app's design text `design` defines `name`, a token or an
/// icon.
fn defines(design: &str, name: &str) -> bool {
    exact_tamber::parse(design).is_ok_and(|d| {
        d.tokens
            .iter()
            .chain(&d.icons)
            .any(|t| t.name.value == name)
    })
}

/// What a package token's row says: its package, the file its value comes
/// from (the app's design `file` when the app sets it, else the package's),
/// and its identity, `<package>/<file>#<token>`. None for the app's own.
pub(crate) fn about(app: &AppParams, design: &str, file: &str, name: &str) -> Option<Value> {
    let (source, _) = owner(app, name, true)?;
    let from = match defines(design, name) {
        true => file.to_string(),
        false => source.home.clone(),
    };
    Some(
        json!({"package": source.package, "file": from, "identity": format!("{}#{name}", source.home)}),
    )
}

/// `out` (a `resolve` or `explain` answer) with a package token's
/// `package`, `file` and `identity`, and with `own` (explain) an app-set
/// one's `package_value`, the package's own line. `file` is the design asked
/// about, as `snap` has it.
pub(crate) fn fields(
    app: &AppParams,
    snap: &[(String, String, String)],
    file: &str,
    name: &str,
    out: &mut Value,
    own: bool,
) {
    let text = snap
        .iter()
        .find(|(f, _, _)| f == file)
        .map_or("", |(_, t, _)| t.as_str());
    if let Some(about) = about(app, text, file, name) {
        for k in ["package", "file", "identity"] {
            out[k] = about[k].clone();
        }
    }
    if let Some(line) = own.then(|| package_value(app, text, name)).flatten() {
        out["package_value"] = line;
    }
}

/// Each package token's row in `rows` (the IR's tokens, by `name`) given
/// its package and file.
pub(crate) fn mark(rows: &mut [Value], app: &AppParams, design: &str, file: &str) {
    if app.packages.is_empty() {
        return;
    }
    for row in rows {
        let Some(name) = row["name"].as_str().map(str::to_string) else {
            continue;
        };
        if let Some(about) = about(app, design, file, &name) {
            row["package"] = about["package"].clone();
            row["file"] = about["file"].clone();
        }
    }
}

/// An app-set package token's own line in its package: its value, its
/// `when` lines (which the app's override drops, whole) and its file.
pub(crate) fn package_value(app: &AppParams, design: &str, name: &str) -> Option<Value> {
    if !defines(design, name) {
        return None;
    }
    let (source, token) = owner(app, name, true)?;
    let whens: Vec<Value> = token
        .whens
        .iter()
        .map(|w| json!({"when": w.conds.iter().map(|(a, v)| json!({"axis": a, "value": v})).collect::<Vec<_>>(), "value": w.value.value}))
        .collect();
    Some(json!({"value": token.value.value, "whens": whens, "file": source.home}))
}

/// The ops as the app's design takes them (theme2's ruling 17:09): an op on
/// a package's design is refused (read-only, D11); `set-token` of a name the
/// app's design doesn't define (as the ops before it leave it) but a package
/// or the base scale does is the app's override, written as a line of its
/// own (`add-token`); `add-token` is for the app's own names, refusing a
/// package's or the base scale's (that is `set-token`'s) and a new name in a
/// package's namespace, as `rename-token` refuses renaming onto one.
/// `restoring` (a revert or an import the server planned): its `add-token`
/// of a package's or base name is the app's override line too. `dir`: the
/// app's directory, where its files are judged read-only (none for a
/// scrub's frame, which writes nothing).
pub(crate) fn ops(
    ops: &mut [(String, Op)],
    app: &AppParams,
    dir: Option<&std::path::Path>,
    design_file: &str,
    design: &str,
    restoring: bool,
) -> Result<(), Value> {
    let refuse = |m: String| Err(json!({"message": m}));
    let base = |name: &str| contract_lower::BASE_SCALE.contains(&name);
    let theirs = |name: &str| owner(app, name, false).is_some() || base(name);
    let namespace = |name: &str| {
        app.packages.iter().find(|p| {
            let ns = p
                .package
                .as_deref()
                .unwrap_or_default()
                .rsplit('/')
                .next()
                .unwrap_or_default();
            name.starts_with(&format!("{ns}."))
        })
    };
    // The app's names as the ops so far leave them.
    let mut defined: std::collections::HashSet<String> = exact_tamber::parse(design)
        .map(|d| d.tokens.into_iter().map(|t| t.name.value).collect())
        .unwrap_or_default();
    for (file, op) in ops.iter_mut() {
        // A write's (a scrub's frame writes nothing, and its release is a write).
        if let Some(dir) = dir {
            read_only(app, dir, std::iter::once(file.as_str()))?;
        }
        if *file != design_file {
            continue;
        }
        match op {
            Op::SetToken {
                name,
                when,
                value,
                comment,
                ..
            } if !defined.contains(name.as_str()) && theirs(name) => {
                if !when.is_empty() {
                    return refuse(format!("`{name}` isn't set for this app yet: set its value (no `when`) first, which takes the token whole"));
                }
                defined.insert(name.clone());
                // The package's written kind (a shadow's must be written), its `when`s not.
                let kind = owner(app, name, false).and_then(|(_, t)| t.kind);
                *op = Op::AddToken {
                    name: name.clone(),
                    kind,
                    value: value.clone(),
                    meta: Vec::new(),
                    whens: Vec::new(),
                    comment: comment.clone(),
                    before: None,
                };
            }
            Op::AddToken { name, .. }
                if theirs(name) && !(restoring && !defined.contains(name.as_str())) =>
            {
                return refuse(format!(
                    "`{name}` is a package's or the base scale's: `set-token` sets it for this app"
                ));
            }
            Op::AddToken { name, .. } => {
                if let (Some(p), false) = (namespace(name), theirs(name)) {
                    return refuse(format!(
                        "`{name}` is in the package `{}`'s namespace, which it doesn't define: the package owns its names",
                        p.package.as_deref().unwrap_or_default()
                    ));
                }
                defined.insert(name.clone());
            }
            Op::RenameToken { to, from } => {
                if theirs(to) || namespace(to).is_some() {
                    return refuse(format!("`{to}` is a package's, the base scale's or in a package's namespace: rename to a name of the app's own"));
                }
                defined.remove(from.as_str());
                defined.insert(to.clone());
            }
            Op::RemoveToken { name } => {
                defined.remove(name.as_str());
            }
            _ => {}
        }
    }
    Ok(())
}
