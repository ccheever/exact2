//! @ref LLP 1121 stage 14 C2 (theme2's condition) — an even-odd icon's path
//! starts `evenodd:`, in the plan's `icons` row and in the source form's
//! `<path>`; every reader strips it through ONE shared parse,
//! `exact_kernel::Icon`. Round 2 made the guard conservative:
//! - Rust, by type: an `icons` row's `path` is an `exact_plan::IconPathId`,
//!   which `plan.str()` and the string pool don't take (exact_plan's
//!   compile-fail doc tests hold the bypasses: a binding the table hands out,
//!   a parenthesised field, a destructured row). Its one way out,
//!   `raw_for_icon`, is called by `exact_runner::icon_of` alone, which hands
//!   it to `Icon::of_row`. So the Rust scan is: no `raw_for_icon` anywhere
//!   else; and no `"icon|"` form split, searched or stripped outside
//!   `Icon::parse`.
//! - JavaScript (`.js`, `.mjs`, `.ts`, tests aside), with no receiver
//!   heuristics: any `evenodd:`, any string starting `icon|` (either quote or
//!   a backtick), and, in a file that names a symbol source at all, any
//!   split, search or slice on `"|"` or `":"`, outside the allowed function
//!   (the page's `iconForm`) and the named lines.

use std::path::{Path, PathBuf};

/// The places a reader may be, from the repository's root.
const SCANNED: &[&str] = &[
    "kernel", "host", "js", "scripts", "contract", "runner", "bake", "plan/src",
];

/// The readers allowed: a file and the first line of a function, whose whole
/// body is allowed, each with why.
const FUNCTIONS: &[(&str, &str, &str)] = &[
    (
        "plan/src/lib.rs",
        "    pub fn raw_for_icon(self) -> StrId {",
        "the escape itself",
    ),
    (
        "runner/src/bridge.rs",
        "pub fn icon_of<'a>(",
        "the one caller of the escape, which hands the row to Icon::of_row",
    ),
    (
        "kernel/src/replaced.rs",
        "    pub fn parse(name: &'a str) -> Option<Icon<'a>> {",
        "Icon::parse itself",
    ),
    // The JS target's page reads a computed source with no Rust to call:
    // Icon::parse mirrored once, held to the kernel's own cases
    // (kernel/tests/fixtures/icon-forms.json) by tests/symbol-hole.test.mjs.
    (
        "host/web-js/symbols.js",
        "const iconForm = name => {",
        "the page's JavaScript mirror of Icon::parse",
    ),
];

/// Single lines allowed, by file and their whole text, each with why.
const LINES: &[(&str, &str, &str)] = &[
    (
        "host/web-js/symbols.js",
        r#"const form = name.startsWith("icon|") ? iconForm(name) : undefined;"#,
        "hands a form to iconForm, unread",
    ),
    (
        "host/web-js/dom.js",
        "set cssText(t) { this.map.clear(); for (const d of t.split(';')) { const i = d.indexOf(':'); if (i > 0) this.map.set(d.slice(0, i).trim(), d.slice(i + 1).trim()); } }",
        "the DOM shim's style declarations (`name: value`), no source",
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
            if !matches!(
                name.as_str(),
                "target" | "node_modules" | "dist" | "web-dist" | "fixtures"
            ) && !name.starts_with('.')
            {
                files(&path, out);
            }
        } else if [".rs", ".js", ".mjs", ".ts"]
            .iter()
            .any(|x| name.ends_with(x))
            && !name.ends_with(".d.ts")
        {
            out.push(path);
        }
    }
}

/// The line ranges (1-based, inclusive) of the allowed functions in `rel`,
/// with their index in [`FUNCTIONS`].
fn allowed_lines(rel: &str, text: &str) -> Vec<(usize, usize, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (k, (file, first, _)) in FUNCTIONS.iter().enumerate() {
        if *file != rel {
            continue;
        }
        let Some(at) = lines.iter().position(|l| l.starts_with(first)) else {
            continue;
        };
        let (mut depth, mut opened) = (0i32, false);
        for (n, line) in lines.iter().enumerate().skip(at) {
            for c in line.chars() {
                match c {
                    '{' => {
                        depth += 1;
                        opened = true
                    }
                    '}' => depth -= 1,
                    _ => {}
                }
            }
            if opened && depth <= 0 {
                out.push((k, at + 1, n + 1));
                break;
            }
        }
    }
    out
}

/// Whether a JavaScript file names a symbol source at all.
fn names_a_source(text: &str) -> bool {
    [
        "symbol:",
        "icon|",
        "evenodd",
        "symbolSource",
        "symbol-source",
        "imageSource",
        "symbol-path",
    ]
    .iter()
    .any(|w| text.contains(w))
}

/// Every reader in one file (`rel` from the root), as `rel:line: text`; the
/// allowed functions and lines it holds.
fn readers(rel: &str, text: &str) -> (Vec<String>, Vec<usize>, Vec<usize>) {
    let allowed = allowed_lines(rel, text);
    let in_allowed = |n: usize| allowed.iter().any(|(_, a, b)| (*a..=*b).contains(&n));
    let js = [".js", ".mjs", ".ts"].iter().any(|x| rel.ends_with(x));
    let source_file = js && names_a_source(text);
    let (mut found, mut lines_seen) = (Vec::new(), Vec::new());
    for (i, line) in text.lines().enumerate() {
        let code = line.trim();
        if code.starts_with("//")
            || code.starts_with("///")
            || code.starts_with("//!")
            || code.starts_with('*')
        {
            continue;
        }
        let hit = if js {
            let marker = code.contains("evenodd:")
                || ["\"icon|", "'icon|", "`icon|"]
                    .iter()
                    .any(|q| code.contains(q));
            let on_bar = source_file
                && [
                    "split",
                    "indexOf",
                    "lastIndexOf",
                    "slice",
                    "substring",
                    "substr",
                ]
                .iter()
                .any(|op| {
                    ["\"|\"", "'|'", "`|`", "\":\"", "':'", "`:`"]
                        .iter()
                        .any(|arg| code.contains(&format!("{op}({arg}")))
                });
            marker || on_bar
        } else {
            code.contains("raw_for_icon")
                || (code.contains("\"icon|\"")
                    && ["split", "strip_prefix", "find(", "split_once"]
                        .iter()
                        .any(|w| code.contains(w)))
        };
        if !hit || in_allowed(i + 1) {
            continue;
        }
        match LINES.iter().position(|(f, l, _)| *f == rel && code == *l) {
            Some(k) => lines_seen.push(k),
            None => found.push(format!("{rel}:{}: {code}", i + 1)),
        }
    }
    (
        found,
        allowed.into_iter().map(|(k, _, _)| k).collect(),
        lines_seen,
    )
}

/// Test files build fixtures of the form; they read none.
fn is_test(rel: &str) -> bool {
    rel.contains("/tests/") || rel.contains(".test.") || rel.ends_with("_tests.rs")
}

#[test]
fn nothing_reads_an_icons_path_but_exact_kernel_icon() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (mut found, mut fns, mut lines) = (
        Vec::new(),
        vec![false; FUNCTIONS.len()],
        vec![false; LINES.len()],
    );
    for dir in SCANNED {
        let mut all = Vec::new();
        files(&root.join(dir), &mut all);
        for path in all {
            let rel = path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if is_test(&rel) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let (f, a, l) = readers(&rel, &text);
            found.extend(f);
            a.into_iter().for_each(|k| fns[k] = true);
            l.into_iter().for_each(|k| lines[k] = true);
        }
    }
    assert!(
        found.is_empty(),
        "read an icon's path through exact_runner::icon_of / exact_kernel::Icon, not by hand:\n{}",
        found.join("\n")
    );
    let stale: Vec<_> = FUNCTIONS
        .iter()
        .zip(&fns)
        .map(|((f, n, _), s)| (f, n, s))
        .chain(LINES.iter().zip(&lines).map(|((f, n, _), s)| (f, n, s)))
        .filter(|(_, _, s)| !**s)
        .map(|(f, n, _)| format!("{f}: `{n}`"))
        .collect();
    assert!(
        stale.is_empty(),
        "an allowed reader is no longer present; remove it:\n{}",
        stale.join("\n")
    );
}

/// Each bypass the reviews named is caught, and what is no reader isn't.
/// (The Rust row bypasses are compile errors: exact_plan's `IconPathId`
/// compile-fail doc tests hold them.)
#[test]
fn the_scan_catches_each_bypass() {
    let rust = |body: &str| readers("x/a.rs", body).0;
    assert!(
        !rust("let d = plan.str(row.path.raw_for_icon());").is_empty(),
        "the escape outside icon_of"
    );
    assert!(!rust(
        "let parts: Vec<_> = form.strip_prefix(\"icon|\").unwrap().split('|').collect();"
    )
    .is_empty());
    let js = |body: &str| readers("x/a.js", body).0;
    for bypass in [
        // The split after a prefix check (round 2).
        "if (s.startsWith(\"symbol:\")) { const f = s.slice(7).split(\"|\"); }",
        r#"const f = name.startsWith("icon|") ? name.split("|") : null;"#,
        "const f = name.startsWith('icon|') ? name.split('|') : null;",
        "const parts = `icon|${a}`.split(`|`);",
        "const at = form.indexOf('|'); const src = el.dataset.symbolSource;",
        "const rule = path.slice(0, path.indexOf(\":\")); // data-symbol-path",
        "const rule = d.startsWith('evenodd:');",
        "const s = el.getAttribute('data-symbol-source').substring(5);\nconst [a, b] = s.split(':');",
    ] {
        assert!(!js(bypass).is_empty(), "missed: {bypass}");
    }
    // What is no reader: the allowed function and line; a split in a file
    // that names no source; a comment.
    let allowed = "const iconForm = name => {\n  const at = name.indexOf(\"|\");\n};\nconst form = name.startsWith(\"icon|\") ? iconForm(name) : undefined;\n";
    let (found, fns, lines) = readers("host/web-js/symbols.js", allowed);
    assert!(found.is_empty(), "{found:?}");
    assert_eq!((fns, lines), (vec![3], vec![0]));
    assert!(readers("x/b.js", "const [host, port] = url.split(':');")
        .0
        .is_empty());
    assert!(readers("x/c.js", "// evenodd: a comment\nconst x = 1;")
        .0
        .is_empty());
    assert!(readers("x/d.rs", "let v = plan.str(a.values).split('|');")
        .0
        .is_empty());
}
