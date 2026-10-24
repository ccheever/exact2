//! @ref LLP 1121 stage 14 C2 (theme2's condition, closed hard) — an even-odd
//! icon's path starts `evenodd:`, in the plan's `icons` row and in the source
//! form's `<path>`; every reader strips it through ONE shared parse,
//! `exact_kernel::Icon`. The guard parses nothing:
//! - Rust, by type: an `icons` row's `path` is an `exact_plan::IconPathId`,
//!   which `plan.str()` and the string pool don't take (exact_plan's
//!   compile-fail doc tests hold the bypasses). Its one method out is called
//!   by `exact_runner::icon_of` alone. The scan: that method's name, as text,
//!   anywhere in any `.rs` file (comments and strings too) outside its own
//!   definition and `icon_of` (each with the doc comment above it) fails.
//!   Excepted: this file, which builds fixtures of it.
//! - JavaScript (`.js`, `.mjs`, `.ts`) in the files that handle a symbol
//!   source (they name `data-symbol`, `symbolSource`, `symbolPath`,
//!   `symbol-source`, a quoted `symbol:` or `icon|`), tests aside: any string
//!   literal that is exactly a bar or a colon (any quote), or a `\|` (a regex
//!   on a bar), fails unless that exact line is allowed below, with why.

use std::path::{Path, PathBuf};

/// The places a reader may be, from the repository's root.
const SCANNED: &[&str] = &[
    "kernel", "host", "js", "scripts", "contract", "runner", "bake", "plan/src",
];

/// The escape's name, assembled so this file holds it only in fixtures.
fn escape() -> String {
    ["raw", "for", "icon"].join("_")
}

/// The Rust functions whose body (and doc comment) may name the escape.
const FUNCTIONS: &[(&str, &str)] = &[
    (
        "plan/src/lib.rs",
        "    pub fn raw_for_icon(self) -> StrId {",
    ),
    ("runner/src/bridge.rs", "pub fn icon_of<'a>("),
];

/// JavaScript lines allowed, by file and whole (trimmed) text, with why.
const LINES: &[(&str, &str, &str)] = &[
    ("host/web-js/symbols.js", r#"for (let i = 0; i < 3; i++) { const at = rest.indexOf("|"); if (at < 0) return null; f.push(rest.slice(0, at)); rest = rest.slice(at + 1); }"#, "iconForm, Icon::parse's JavaScript mirror: its four-part split"),
    ("host/web-js/symbols.js", r#"const [, box, raw] = f, colon = raw.indexOf(":");"#, "iconForm: the rule's marker"),
    ("host/web-js/symbols.js", r#"if (rule !== "evenodd" || !d.trim() || d.includes(":")) return null;"#, "iconForm: a second colon is no icon"),
    ("host/web-js/dom.js", "set cssText(t) { this.map.clear(); for (const d of t.split(';')) { const i = d.indexOf(':'); if (i > 0) this.map.set(d.slice(0, i).trim(), d.slice(i + 1).trim()); } }", "the DOM shim's style declarations"),
    ("host/web-js/rt.js", r#"if (typeof k + ":" + (Object.is(k, -0) ? 0 : k) !== r.k) break;"#, "a keyed list's key, type and value"),
    ("host/web-js/rt.js", r#"k = typeof k + ":" + (Object.is(k, -0) ? 0 : k);"#, "a keyed list's key, type and value"),
    ("host/web-js/rt.js", r#"if (next.has(k)) { const n = seen.get(k) ?? 1; seen.set(k, n + 1); k = "d" + n + ":" + k; journal.push(`each: repeated key ${k}`); }"#, "a repeated key's disambiguation"),
    ("host/web/glue.js", r#"const need=`surface.${op.mode==="capture"?"read":"write"} ${op.name}`;"#, "a canvas surface's grant (`\"read\":\"write\"` holds a colon literal's text)"),
    ("host/web/navigation.js", "const namespace = at < 0 ? target.startsWith('/') ? '' : null : target.slice(0, at) + ':';", "a route's namespace"),
    ("host/web/navigation.js", "const namespace = at < 0 ? path.startsWith('/') ? '' : null : path.slice(0, at) + ':';", "a route's namespace"),
    (
        "host/web/navigation.js",
        "const isWide = wide(), said = [isWide, list?.parentElement === nav && list.getAttribute(\"aria-orientation\"), ...[nav, list].map(el => el && [el.className, el.style.display, el.style.flexDirection, el.style.position, el.style.appearance].join(\"|\"))].join(\"/\"); if (placed.get(nav) !== said) { nav.removeAttribute(\"data-exact-sidebar\"); nav.toggleAttribute(\"data-exact-sidebar\", isWide && unsaidBar(nav, list)); placed.set(nav, said); }",
        "the sidebar's placement key, its styles joined",
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
                "target" | "node_modules" | "dist" | "web-dist"
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

/// The 1-based line ranges of the allowed functions in `rel`, each from the
/// doc comment above its first line to its closing brace.
fn allowed_lines(rel: &str, text: &str) -> Vec<(usize, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (file, first) in FUNCTIONS {
        if *file != rel {
            continue;
        }
        let Some(at) = lines.iter().position(|l| l.starts_with(first)) else {
            continue;
        };
        let mut doc = at;
        while doc > 0 && lines[doc - 1].trim_start().starts_with("///") {
            doc -= 1;
        }
        let (mut depth, mut opened) = (0i32, false);
        for (n, line) in lines.iter().enumerate().skip(at) {
            depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
            opened |= line.contains('{');
            if opened && depth <= 0 {
                out.push((doc + 1, n + 1));
                break;
            }
        }
    }
    out
}

/// Whether a JavaScript file handles a symbol source.
fn handles_a_source(text: &str) -> bool {
    [
        "data-symbol",
        "symbolSource",
        "symbolPath",
        "symbol-source",
        "\"symbol:",
        "'symbol:",
        "icon|",
    ]
    .iter()
    .any(|w| text.contains(w))
}

/// Every reader in one file, as `rel:line: text`, and the allowed lines it
/// holds (their index in [`LINES`]).
fn readers(rel: &str, text: &str) -> (Vec<String>, Vec<usize>) {
    let (mut found, mut seen) = (Vec::new(), Vec::new());
    if rel.ends_with(".rs") {
        let allowed = allowed_lines(rel, text);
        let escape = escape();
        for (i, line) in text.lines().enumerate() {
            if line.contains(&escape) && !allowed.iter().any(|(a, b)| (*a..=*b).contains(&(i + 1)))
            {
                found.push(format!("{rel}:{}: {}", i + 1, line.trim()));
            }
        }
    } else if handles_a_source(text) {
        for (i, line) in text.lines().enumerate() {
            let hit = ["\"|\"", "'|'", "`|`", "\":\"", "':'", "`:`", "\\|"]
                .iter()
                .any(|t| line.contains(t));
            if !hit {
                continue;
            }
            match LINES
                .iter()
                .position(|(f, l, _)| *f == rel && line.trim() == *l)
            {
                Some(k) => seen.push(k),
                None => found.push(format!("{rel}:{}: {}", i + 1, line.trim())),
            }
        }
    }
    (found, seen)
}

/// JavaScript tests build fixtures of the form; the Rust scan excepts only
/// this file (which builds the escape's fixtures).
fn excepted(rel: &str) -> bool {
    let js = [".js", ".mjs", ".ts"].iter().any(|x| rel.ends_with(x));
    rel.ends_with("contract/cli/tests/it/icon_path_readers.rs")
        || (js && (rel.contains("/tests/") || rel.contains(".test.")))
}

#[test]
fn nothing_reads_an_icons_path_but_exact_kernel_icon() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (mut found, mut seen) = (Vec::new(), vec![false; LINES.len()]);
    for dir in SCANNED {
        let mut all = Vec::new();
        files(&root.join(dir), &mut all);
        for path in all {
            let rel = path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if excepted(&rel) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let (f, s) = readers(&rel, &text);
            found.extend(f);
            s.into_iter().for_each(|k| seen[k] = true);
        }
    }
    assert!(
        found.is_empty(),
        "read an icon's path through exact_runner::icon_of / exact_kernel::Icon, not by hand:\n{}",
        found.join("\n")
    );
    let stale: Vec<_> = LINES
        .iter()
        .zip(&seen)
        .filter(|(_, s)| !**s)
        .map(|((f, l, _), _)| format!("{f}: `{l}`"))
        .collect();
    assert!(
        stale.is_empty(),
        "an allowed line is no longer present; remove it:\n{}",
        stale.join("\n")
    );
}

/// Every guard bypass from rounds 1-4 fails; what isn't one passes.
#[test]
fn the_scan_catches_each_bypass() {
    let e = escape();
    let rust = |body: String| readers("x/a.rs", &body).0;
    for bypass in [
        format!("let d = plan.str(row.path.{e}());"),
        format!("/* x */ let d = row.path.{e}();"),
        format!("/* a\n   comment */ let d = row.path.{e}();"),
        format!("fn f(out: &mut String) {{\n    *out = plan.str(row.path.{e}()).to_owned();\n}}"),
        format!("/// {e} in a comment is a mention too"),
        format!("let name = \"{e}\";"),
    ] {
        assert!(!rust(bypass.clone()).is_empty(), "missed: {bypass}");
    }
    // Inside the allowed definition (and its doc comment), nothing.
    let allowed = "/// The one way out.\n    pub fn raw_for_icon(self) -> StrId {\n        StrId(self.0)\n    }\n";
    assert!(readers("plan/src/lib.rs", allowed).0.is_empty());
    let js = |body: &str| readers("x/a.js", &format!("// handles data-symbol-source\n{body}")).0;
    for bypass in [
        r#"const f = name.split("|");"#,
        "const f = name.split('|');",
        "const f = name.split(`|`);",
        r#"const f = name.split ("|", 4);"#,
        r#"const f = name.split?.("|");"#,
        r#"const f = name.split(("|"));"#,
        r"const f = name.split(/\|/);",
        "const at = name.search(/:/) + name.indexOf(':');",
        r#"const f = name.split("|" /* sep */, 4);"#,
        r#"const at = source.indexOf("|", i);"#,
        "const f = el.dataset.symbolSource.split(\n  '|'\n);",
        r#"const rule = path.slice(0, path.indexOf(":"));"#,
    ] {
        assert!(!js(bypass).is_empty(), "missed: {bypass}");
    }
    // A file that handles no source is not scanned; an allowed line passes.
    assert!(readers("x/b.js", "const [host, port] = url.split(':');")
        .0
        .is_empty());
    let (found, seen) = readers("host/web/navigation.js", "// data-symbol\n  const namespace = at < 0 ? path.startsWith('/') ? '' : null : path.slice(0, at) + ':';\n");
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(seen, vec![9]);
}
