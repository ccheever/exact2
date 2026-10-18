//! `grant-device-missing`: a device the view reaches through the runner
//! answers only under the app's grant (LLP 1069.008 D3; `runner/src/device.rs`):
//! `exactMIDI()` reads `denied` and `showNotification` is refused, on every
//! host, at run time. The grants are the data module's (`export const grants`
//! in `app.ts`, `fn grants()` in a Rust data crate) and may be computed, so
//! this reads the app's source text, its comments left out. A
//! `device.midi <key>` whose key the strings table has is the grant; a
//! `device.midi` built otherwise (`` `device.midi ${key}` ``) cannot be read,
//! so nothing is said; a grant line no file names is certainly missing, and
//! one whose key the table lacks is what the bake would refuse
//! (`bake/src/reach.rs`). A plan compiled without an `app.json` (a corpus
//! file, a conformance app) is checked for none.
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

/// `text` with its `//` and `/* */` comments blanked, past string literals:
/// `"…"` in both languages, and `'…'` and `` `…` `` in TypeScript (in Rust a
/// `'` begins a lifetime as often as a character).
fn code(text: &str, typescript: bool) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let quote = b == b'"' || (typescript && (b == b'\'' || b == b'`'));
        if quote {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != b {
                i += if bytes[i] == b'\\' { 2 } else { 1 };
            }
            i = (i + 1).min(bytes.len());
            out.extend_from_slice(&bytes[start..i]);
        } else if bytes[i..].starts_with(b"//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if bytes[i..].starts_with(b"/*") {
            let end = text[i + 2..]
                .find("*/")
                .map_or(bytes.len(), |e| i + 2 + e + 2);
            out.push(b' ');
            i = end;
        } else {
            out.push(b);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What one occurrence of a grant line in the source says.
#[derive(Debug, PartialEq)]
enum Found {
    /// `device.midi purpose.midi`, written out: its key.
    Key(String),
    /// Built otherwise (`` `device.midi ${key}` ``, `['device.midi', key].join(' ')`):
    /// whether it is the grant cannot be read.
    Unknown,
}

/// Each occurrence of `line` (`device.midi`) as a word in `code`.
fn found(code: &str, line: &str) -> Vec<Found> {
    let key_char = |c: char| c.is_alphanumeric() || matches!(c, '.' | '-' | '_');
    code.match_indices(line)
        .filter(|(at, _)| {
            // A line begins after an escape (`\ndevice.midi`) as after a quote.
            let head = &code[..*at];
            let word = head.ends_with("\\n")
                || head.ends_with("\\t")
                || !head
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_');
            word && !code[at + line.len()..].chars().next().is_some_and(key_char)
        })
        .map(|(at, _)| {
            let rest = &code[at + line.len()..];
            let Some(spelled) = rest.strip_prefix([' ', '\t']) else {
                return Found::Unknown;
            };
            let spelled = spelled.trim_start_matches([' ', '\t']);
            let key: String = spelled.chars().take_while(|c| key_char(*c)).collect();
            // A key ends at the string's end, an escape (`\n`) or a separator;
            // anything else (`${`) is computed.
            let ends = spelled[key.len()..].chars().next();
            let key = key.trim_end_matches('.').to_string();
            match ends {
                _ if key.is_empty() => Found::Unknown,
                None => Found::Key(key),
                Some(c)
                    if matches!(c, '\'' | '"' | '`' | '\\' | ',' | ')' | ';')
                        || c.is_whitespace() =>
                {
                    Found::Key(key)
                }
                Some(_) => Found::Unknown,
            }
        })
        .collect()
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
    let mut files = Vec::new();
    sources(root, 0, &mut files);
    files.sort();
    let code: Vec<String> = files
        .iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(path).ok()?;
            Some(code(&text, path.extension().is_some_and(|e| e == "ts")))
        })
        .collect();
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
        let all: Vec<Found> = code.iter().flat_map(|c| found(c, &line)).collect();
        // Granted with its purpose anywhere settles it; a grant built otherwise
        // cannot be read, so it is not warned about; else the first key written.
        if all
            .iter()
            .any(|f| *f == Found::Unknown || matches!(f, Found::Key(k) if has(k)))
        {
            continue;
        }
        let granted = all.into_iter().find_map(|f| match f {
            Found::Key(k) => Some(k),
            Found::Unknown => None,
        });
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
    out
}

#[cfg(test)]
mod tests {
    use super::{code, found, Found};

    fn read(source: &str, typescript: bool) -> Vec<Found> {
        found(&code(source, typescript), "device.midi")
    }

    #[test]
    fn a_written_grant_is_its_key_and_a_built_one_is_unknown() {
        let key = |k: &str| vec![Found::Key(k.into())];
        assert_eq!(
            read("export const grants = 'device.midi purpose.midi';", true),
            key("purpose.midi")
        );
        assert_eq!(
            read(
                "const g = 'net.fetch https://x/\\ndevice.midi purpose.midi\\nsecret.keep k';",
                true
            ),
            key("purpose.midi")
        );
        assert_eq!(
            read(
                "fn grants(&self) -> &str { \"device.midi purpose.midi\" }",
                false
            ),
            key("purpose.midi")
        );
        for built in [
            "export const grants = `device.midi ${PURPOSE}`;",
            "export const grants = ['device.midi', 'purpose.midi'].join(' ');",
            "export const grants = 'device.midi ' + key;",
            "export const grants = `device.midi purpose.${name}`;",
        ] {
            assert_eq!(read(built, true), [Found::Unknown], "{built}");
        }
    }

    #[test]
    fn comments_and_other_words_are_no_grant() {
        assert!(read(
            "// device.midi grants instrument access\nexport const grants = '';",
            true
        )
        .is_empty());
        assert!(read(
            "/* device.midi purpose.midi */ export const grants = '';",
            true
        )
        .is_empty());
        assert!(read(
            "//! It grants device.midi purpose.midi\nfn grants() -> &'static str { \"\" }",
            false
        )
        .is_empty());
        assert!(read(
            "const a = 'mydevice.midi x'; const b = 'device.midis x';",
            true
        )
        .is_empty());
        // A `//` in a string is no comment.
        assert_eq!(
            read(
                "const g = 'net.fetch https://x/ device.midi purpose.midi';",
                true
            ),
            [Found::Key("purpose.midi".into())]
        );
    }
}
