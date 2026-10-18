//! `grant-device-missing`: a device the view reaches through the runner
//! answers only under the app's grant (LLP 1069.008 D3; `runner/src/device.rs`):
//! `exactMIDI()` reads `denied` and `showNotification` is refused, on every
//! host, at run time. The grants are the data module's (`export const grants`
//! in `app.ts`, `fn grants()` in a Rust data crate) and may be computed, so
//! this reads the app's source text: a grant line no file of the app names is
//! certainly missing, and its purpose key absent from the strings table is
//! what the bake would refuse (`bake/src/reach.rs`). A plan compiled without
//! an `app.json` (a corpus file, a conformance app) is checked for none.
//!
//! The other device grants (camera, microphone, location, speech, photos,
//! health) are asked for by native modules, which the compiler does not see.

use super::CompileError;
use contract_syntax::{File, Span, Stmt};
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

const DEVICES: &[Device] = &[
    Device {
        name: "midi",
        without: "reads `denied` on every host",
        sources: &["exactMIDI"],
        commands: &["requestMIDIAccess", "sendMIDI"],
        purpose: "Plays the pattern on your instruments.",
    },
    Device {
        name: "notifications",
        without: "is refused on every host (`showNotification: refused`)",
        sources: &[],
        commands: &["showNotification"],
        purpose: "Reminds you when a timer ends.",
    },
];

/// Directories that hold no source of the app's data module.
const SKIP: &[&str] = &["node_modules", "target", "dist", "assets", "strings"];

/// The app's data-module source: its TypeScript and Rust files, outside
/// build output and dependencies.
fn sources(dir: &Path, depth: usize, out: &mut String) {
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
            if let Ok(text) = std::fs::read_to_string(&path) {
                out.push_str(&text);
                out.push('\n');
            }
        }
    }
}

/// The first place `file` reaches `device`: a resource reading one of its
/// sources, else a command, in any action.
fn first_use(file: &File, device: &Device) -> Option<(Span, String)> {
    fn walk(stmts: &[Stmt], device: &Device) -> Option<(Span, String)> {
        stmts.iter().find_map(|s| match s {
            Stmt::Command { name, span, .. } if device.commands.contains(&name.as_str()) => {
                Some((*span, format!("`{name}`")))
            }
            Stmt::If {
                then, otherwise, ..
            } => walk(then, device).or_else(|| walk(otherwise, device)),
            Stmt::Match { some, none, .. } => walk(&some.1, device).or_else(|| walk(none, device)),
            Stmt::Call { body, .. } => walk(body, device),
            _ => None,
        })
    }
    let components = || file.components.iter();
    components()
        .flat_map(|c| &c.resources)
        .find(|r| device.sources.contains(&r.source.as_str()))
        .map(|r| (r.span, format!("`{}()`", r.source)))
        .or_else(|| {
            components()
                .flat_map(|c| &c.actions)
                .find_map(|a| walk(&a.body, device))
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
    if uses.is_empty() {
        return Vec::new();
    }
    let mut text = String::new();
    sources(root, 0, &mut text);
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
        // The grant's purpose key, where the source writes the line.
        let granted = text.match_indices(&line).find_map(|(at, _)| {
            let after = &text[at + line.len()..];
            // The key ends where the source's quote or escape (`\n`) begins.
            after.starts_with([' ', '\t']).then(|| {
                after
                    .trim_start_matches([' ', '\t'])
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_'))
                    .collect::<String>()
            })
        });
        let key = granted
            .clone()
            .unwrap_or_else(|| format!("purpose.{}", device.name));
        let entry = format!("\"{key}\": \"{}\"", device.purpose);
        let has_key = strings.is_some_and(|s| s.base_table().contains_key(&key));
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
    out
}
