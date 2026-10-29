//! `grant-device-missing`: a device the view reaches through the runner
//! answers only under the app's grant (LLP 1069.008 D3; `runner/src/device.rs`):
//! `exactMIDI()` reads `denied` on every host, at run time. The grants are the
//! data module's (`export const grants` in `app.ts`, `fn grants()` in a Rust
//! data crate) and may be computed, so
//! this reads the string literals of the app's source ([`literals`]) and
//! says something only when it is certain: a `device.midi <key>` whose key
//! the strings table has is the grant; a grant built otherwise, or a file
//! this cannot read with certainty, says nothing; a grant line no literal
//! holds is missing, and one whose key the table lacks is what the bake would
//! refuse (`bake/src/reach.rs`). A plan compiled without an `app.json` (a
//! corpus file, a conformance app) is checked for none.
//!
//! The other device grants (camera, microphone, location, speech, photos,
//! health) are asked for by native modules, which the compiler does not see.

mod literals;

use super::CompileError;
use contract_syntax::{Expr, File, Span, Stmt};
use contract_types::strings::Strings;
use std::path::Path;

/// A device the runner gates, and the Contract that reaches it.
struct Device {
    /// The row of `runner/src/device.rs`.
    name: &'static str,
    /// What reaching it without the grant does.
    without: &'static str,
    /// The resources (sources the runner answers) that read it.
    sources: &'static [&'static str],
    /// The commands that use it.
    commands: &'static [&'static str],
    /// What the purpose text says, for the example.
    purpose: &'static str,
}

const DEVICES: &[Device] = &[Device {
    name: "midi",
    without: "reads `denied` on every host",
    sources: &["exactMIDI"],
    commands: &["requestMIDIAccess", "sendMIDI"],
    purpose: "Plays the pattern on your instruments.",
}];

/// Directories that hold no source of the app's data module.
const SKIP: &[&str] = &["node_modules", "target", "dist", "assets", "strings"];

/// The app's data-module source files, TypeScript and Rust, outside build
/// output and dependencies, by path (so no answer depends on `read_dir`'s order).
fn sources(dir: &Path, depth: usize, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            if depth < 6 && !SKIP.contains(&&*name) {
                sources(&path, depth + 1, out);
            }
        } else if (name.ends_with(".ts") && !name.ends_with(".d.ts")) || name.ends_with(".rs") {
            out.push(path);
        }
    }
}

/// The commands of `file`'s actions, in order, through `if`, `match` and calls.
fn commands(file: &File) -> Vec<(&str, &[Expr], Span)> {
    fn walk<'f>(stmts: &'f [Stmt], out: &mut Vec<(&'f str, &'f [Expr], Span)>) {
        for s in stmts {
            match s {
                Stmt::Command { name, args, span } => out.push((name, args, *span)),
                Stmt::If {
                    then, otherwise, ..
                } => {
                    walk(then, out);
                    walk(otherwise, out);
                }
                Stmt::Match { some, none, .. } => {
                    walk(&some.1, out);
                    walk(none, out);
                }
                Stmt::Call { body, .. } => walk(body, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    for a in file.components.iter().flat_map(|c| &c.actions) {
        walk(&a.body, &mut out);
    }
    out
}

/// Whether the file's MIDI provably reaches no device: it never asks for
/// access, so none is granted; it sends only to `""`, no output, whose
/// messages `sent` lists with no device (LLP 1125 D6); and its `exactMIDI()`
/// reads, by shapes this file declares, `sent` alone, not the `state` or
/// ports a grant would change. An activity log of that kind needs no grant.
fn midi_silent(file: &File, commands: &[(&str, &[Expr], Span)]) -> bool {
    let reads_sent_only = |r: &contract_syntax::ResourceDecl| match &r.shape {
        contract_syntax::TypeExpr::Named(shape, _) => file
            .shapes
            .iter()
            .find(|s| s.name == *shape)
            .is_some_and(|s| s.fields.iter().all(|f| f.name == "sent")),
        _ => false,
    };
    commands.iter().all(|(name, args, _)| match *name {
        "requestMIDIAccess" => false,
        "sendMIDI" => matches!(args.first(), Some(Expr::Str(o, _)) if o.is_empty()),
        _ => true,
    }) && file
        .components
        .iter()
        .flat_map(|c| &c.resources)
        .filter(|r| r.source == "exactMIDI")
        .all(reads_sent_only)
}

/// The first place `file` reaches `device`: a resource reading one of its
/// sources, else a command, in any action.
fn first_use(file: &File, device: &Device) -> Option<(Span, String)> {
    let commands = commands(file);
    if device.name == "midi" && midi_silent(file, &commands) {
        return None;
    }
    file.components
        .iter()
        .flat_map(|c| &c.resources)
        .find(|r| device.sources.contains(&r.source.as_str()))
        .map(|r| (r.span, format!("`{}()`", r.source)))
        .or_else(|| {
            commands
                .iter()
                .find(|(name, _, _)| device.commands.contains(name))
                .map(|(name, _, span)| (*span, format!("`{name}`")))
        })
}

/// The warnings for `file` in the app at `root`.
pub(crate) fn check(file: &File, root: &Path, strings: Option<&Strings>) -> Vec<CompileError> {
    if !root.join("app.json").is_file() {
        return Vec::new();
    }
    let uses: Vec<_> = DEVICES
        .iter()
        .filter_map(|d| Some((d, first_use(file, d)?)))
        .collect();
    let modules = module_uses(file);
    if uses.is_empty() && modules.is_empty() {
        return Vec::new();
    }
    let mut files = Vec::new();
    sources(root, 0, &mut files);
    files.sort();
    // Every file's string literals, or none when one cannot be read with
    // certainty: then a grant may be in it, and nothing is said.
    let Some(lits) = files.iter().try_fold(Vec::new(), |mut all, path| {
        let text = std::fs::read_to_string(path).ok()?;
        all.extend(literals::literals(
            &text,
            path.extension().is_some_and(|e| e == "ts"),
        )?);
        Some(all)
    }) else {
        return Vec::new();
    };
    let typescript = root.join("app.ts").is_file();
    let base = strings.map_or("en", |s| s.base.as_str());
    let warn = |span, message| CompileError {
        pass: "warning",
        id: "grant-device-missing".into(),
        message,
        span,
        file: None,
        related: Box::new([]),
    };
    let mut out = Vec::new();
    for (device, (span, call)) in uses {
        let line = format!("device.{}", device.name);
        let has = |key: &str| strings.is_some_and(|s| s.base_table().contains_key(key));
        // A grant built otherwise cannot be read, so it is not warned about;
        // granted with its purpose anywhere settles it; else the first key written.
        let Some(keys) = literals::keys(&lits, &line) else {
            continue;
        };
        if keys.iter().any(|k| has(k)) {
            continue;
        }
        let granted = keys.into_iter().next();
        let key = granted
            .clone()
            .unwrap_or_else(|| format!("purpose.{}", device.name));
        let entry = format!("\"{key}\": \"{}\"", device.purpose);
        let has_key = has(&key);
        match granted {
            None => {
                let grant = format!("{line} {key}");
                let r#where = if typescript {
                    format!("app.ts's `export const grants` (`export const grants = '{grant}';`, a line of its own beside any others)")
                } else {
                    format!("the Rust data crate's `fn grants()` (`\"{grant}\"`, a line of its own beside any others)")
                };
                let strings_too = if has_key {
                    String::new()
                } else {
                    format!(", and its purpose to strings/{base}.json (`{entry}`), the text the person is shown when asked")
                };
                out.push(warn(span, format!(
                    "{call} {} unless the app grants `{line}`, and no file of the app's data module names it: add `{grant}` to {where}{strings_too}",
                    device.without
                )));
            }
            Some(_) if !has_key => out.push(warn(span, format!(
                "{call} needs `{line} {key}`'s purpose, and strings/{base}.json has no \"{key}\": add `{entry}`, the text the person is shown when asked (the build refuses the grant without it)"
            ))),
            Some(_) => {}
        }
    }
    // A native module that asks permission (LLP 1133.002 D6, D13): the app
    // writes one of its interface's grant lines.
    for (grants, span, call) in modules {
        let Some(found) = grants
            .iter()
            .map(|g| literals::keys(&lits, g))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        if found.iter().any(|keys| !keys.is_empty()) {
            continue;
        }
        let lines: Vec<String> = grants.iter().map(|g| format!("`{g}`")).collect();
        let last = grants.last().cloned().unwrap_or_default();
        // A device line names its purpose key; any other its target (`auth.passkey <rp id>`).
        let named = |d: &str| {
            (exact_runner::device::DEVICES.iter()).any(|row| row.name == d && row.purpose)
        };
        let add = match last.strip_prefix("device.") {
            Some(device) if !named(device) => format!("`{last}` to the app's grants"),
            Some(device) => format!("`{last} purpose.{device}` to the app's grants, with its purpose in strings/{base}.json"),
            // A line that names no target (LLP 1020.002 D9).
            None if matches!(last.as_str(), "browser.automation" | "payment.google-pay") => {
                format!("`{last}` to the app's grants")
            }
            None => format!("`{last} <its target>` to the app's grants"),
        };
        out.push(CompileError {
            id: "grant-missing".into(),
            ..warn(span, format!(
                "{call} answers only under one of {}, and no file of the app's data module names one: add {add}",
                lines.join(" or ")
            ))
        });
    }
    out
}

/// Each set of grant lines a native module's names the app reads or sends
/// answer under, once, by its first use: the lines, where, and the call. A
/// name's own list of lines is its set (an empty one, as a picker's, needs
/// none: LLP 1133.018 D4); else its interface's.
fn module_uses(file: &File) -> Vec<(Vec<String>, Span, String)> {
    let mut out: Vec<(Vec<String>, Span, String)> = Vec::new();
    let mut seen: Vec<(&str, Vec<String>)> = Vec::new();
    for m in &file.modules {
        let sub = serde_json::from_str::<serde_json::Value>(&m.substitute).unwrap_or_default();
        let lines = match &sub["nameGrants"] {
            serde_json::Value::Array(own) => own.clone(),
            _ => sub["grants"].as_array().cloned().unwrap_or_default(),
        };
        let grants: Vec<String> = (lines.iter())
            .filter_map(|g| g.as_str().map(str::to_owned))
            .collect();
        if grants.is_empty() || seen.contains(&(m.extension.as_str(), grants.clone())) {
            continue;
        }
        fn sends(stmts: &[Stmt], name: &str) -> Option<Span> {
            stmts.iter().find_map(|s| match s {
                Stmt::Send { source, span, .. } if source == name => Some(*span),
                // A send that answers nothing is a statement (LLP 1133.004 D5).
                Stmt::Command { name: n, span, .. } if n == name => Some(*span),
                Stmt::If {
                    then, otherwise, ..
                } => sends(then, name).or_else(|| sends(otherwise, name)),
                Stmt::Match { some, none, .. } => {
                    sends(&some.1, name).or_else(|| sends(none, name))
                }
                Stmt::Call { body, .. } => sends(body, name),
                _ => None,
            })
        }
        let used = (file.components.iter())
            .flat_map(|c| &c.resources)
            .find(|r| r.source == m.name)
            .map(|r| r.span)
            .or_else(|| {
                (file.components.iter())
                    .flat_map(|c| &c.actions)
                    .find_map(|a| sends(&a.body, &m.name))
            });
        if let Some(span) = used {
            seen.push((&m.extension, grants.clone()));
            out.push((grants, span, format!("`{}`", m.declared)));
        }
    }
    out
}
