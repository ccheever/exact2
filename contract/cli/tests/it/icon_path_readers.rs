//! @ref LLP 1121 stage 14 C2 (theme2's condition) — an even-odd icon's path
//! starts `evenodd:`, in the plan's `icons` row and in the source form's
//! `<path>`; every reader strips it through ONE shared parse,
//! `exact_kernel::Icon` (`Icon::parse` for a source, `Icon::of_row` for a
//! row). This scan of the kernel, the hosts and the tools finds any other
//! reader: a row's path read as a string (`s(i.path.0)`, `plan.str(row.path)`)
//! other than as an argument to `Icon::of_row` (the kernel holds no plan
//! type, so a caller hands it the row's four strings), or a symbol source
//! split by hand (`"icon|"` with `split`/`strip_prefix`).

use std::path::{Path, PathBuf};

/// The places a reader may be, from the repository's root.
const SCANNED: &[&str] = &[
    "kernel",
    "host/apple/src",
    "host/linux",
    "host/web",
    "host/web-js",
    "contract",
    "runner",
    "bake",
    "plan/src",
];

/// The readers allowed, by file and the text of the line, each with why.
const ALLOWED: &[(&str, &str, &str)] = &[
    (
        "kernel/src/replaced.rs",
        r#"name.strip_prefix("icon|")?.splitn(4, '|')"#,
        "Icon::parse itself: the one reader",
    ),
    // Known exception (iosfix's note after theme2's approval): Apple's
    // `icon_name` compares the raw row's path against Icon::parse's stripped
    // one, so it misses an even-odd icon's name. iosfix's Apple commit routes
    // it through `Icon::of_row` and removes this line; the scan then fails
    // until it is gone ("no longer present"), so the exception can't outlive it.
    (
        "host/apple/src/icon_path.rs",
        "&& s(i.path.0) == icon.path",
        "iosfix's commit replaces it with Icon::of_row",
    ),
    // The JS target's page reads a computed source with no Rust to call:
    // Icon::parse's rule mirrored once (`iconForm`), tested beside the mask.
    (
        "host/web-js/symbols.js",
        r#"const f = name.split("|"), p = f[3] ?? """#,
        "the page's JavaScript mirror of Icon::parse",
    ),
];

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !matches!(name.as_str(), "target" | "node_modules" | "dist" | ".git")
                && !name.starts_with('.')
            {
                files(&path, out);
            }
        } else if [".rs", ".js", ".mjs"].iter().any(|x| name.ends_with(x)) {
            out.push(path);
        }
    }
}

/// Whether `line` reads an icons row's path as a string: a string lookup
/// (`s(…)`, the hosts' closure, or `.str(…)`, the plan's) of `<row>.path`.
fn reads_row_path(line: &str) -> bool {
    ["s(", ".str("].iter().any(|call| {
        line.match_indices(call).any(|(at, _)| {
            let before = line[..at].chars().next_back();
            if *call == "s(" && before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                return false;
            }
            let rest = &line[at + call.len()..];
            let arg = rest.split(')').next().unwrap_or("").trim();
            let mut parts = arg.split('.');
            let (owner, field) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
            !owner.is_empty()
                && owner.chars().all(|c| c.is_alphanumeric() || c == '_')
                && field == "path"
        })
    })
}

/// Whether `line` splits a symbol source by hand (`file_has_form`: the file
/// names the form at all).
fn splits_source(line: &str, file_has_form: bool) -> bool {
    let by_hand = ["split", "strip_prefix", "splitn"]
        .iter()
        .any(|w| line.contains(w));
    (line.contains("\"icon|\"") && by_hand)
        || (file_has_form && (line.contains(r#".split("|")"#) || line.contains(".split('|')")))
}

#[test]
fn nothing_reads_an_icons_path_but_exact_kernel_icon() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let me = Path::new(file!())
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let (mut found, mut seen) = (Vec::new(), vec![false; ALLOWED.len()]);
    for dir in SCANNED {
        let mut all = Vec::new();
        files(&root.join(dir), &mut all);
        for path in all {
            if path.file_name().is_some_and(|n| n.to_string_lossy() == me) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let rel = path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let has_form = text.contains("\"icon|\"") || text.contains("\"icon|");
            let lines: Vec<&str> = text.lines().collect();
            for (n, line) in lines.iter().enumerate() {
                // A row's strings handed to the one reader (its call may wrap).
                let to_reader = lines[n.saturating_sub(4)..=n]
                    .iter()
                    .any(|l| l.contains("Icon::of_row("));
                if !((reads_row_path(line) && !to_reader) || splits_source(line, has_form)) {
                    continue;
                }
                match ALLOWED
                    .iter()
                    .position(|(f, needle, _)| rel == *f && line.contains(needle))
                {
                    Some(k) => seen[k] = true,
                    None => found.push(format!("{rel}:{}: {}", n + 1, line.trim())),
                }
            }
        }
    }
    assert!(found.is_empty(), "read an icon's path through exact_kernel::Icon (Icon::parse, Icon::of_row), not by hand:\n{}", found.join("\n"));
    let stale: Vec<_> = ALLOWED
        .iter()
        .zip(&seen)
        .filter(|(_, s)| !**s)
        .map(|((f, n, why), _)| format!("{f}: `{n}` ({why})"))
        .collect();
    assert!(
        stale.is_empty(),
        "an allowed reader is no longer present; remove it from ALLOWED:\n{}",
        stale.join("\n")
    );
}

/// The scan's own rules catch what they are for.
#[test]
fn the_scan_catches_a_reader() {
    assert!(reads_row_path(
        "                && s(i.path.0) == icon.path"
    ));
    assert!(reads_row_path("assert_eq!(plan.str(row.path), d);"));
    assert!(!reads_row_path("let at = sources.path(c.span);"));
    assert!(!reads_row_path("for seg in &s.path.0 {"));
    assert!(!reads_row_path(
        "tool.built = mtimeMs(tool.path) === started;"
    ));
    assert!(splits_source(
        r#"let mut parts = name.strip_prefix("icon|")?.splitn(4, '|');"#,
        true
    ));
    assert!(splits_source(
        r#"const f = name.startsWith("icon|") ? name.split("|") : null;"#,
        true
    ));
    assert!(!splits_source(r#"|| r.starts_with("icon|")"#, true));
    assert!(!splits_source(
        r#"let values: Vec<&str> = plan.str(a.values).split('|').collect();"#,
        false
    ));
}
