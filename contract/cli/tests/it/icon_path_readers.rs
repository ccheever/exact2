//! @ref LLP 1121 stage 14 C2 (theme2's condition, closed hard) — an even-odd
//! icon's path starts `evenodd:`, in the plan's `icons` row and in the source
//! form's `<path>`; every reader strips it through ONE shared parse,
//! `exact_kernel::Icon`. The guard parses nothing:
//! - Rust, by type: an `icons` row's `path` is an `exact_plan::IconPathId`,
//!   which `plan.str()` and the string pool don't take, and which can't be
//!   hashed or ordered (exact_plan's compile-fail doc tests hold the
//!   bypasses). Its one method out is called by `exact_runner::icon_of`
//!   alone. The scan: that method's name, as text (comments and strings
//!   too), occurs EXACTLY twice across every scanned `.rs` file: on its
//!   definition line in exact_plan, and once inside the body of
//!   `exact_runner::icon_of` (the nearest `fn` line above the call). Any
//!   other occurrence, or a second one in `icon_of`, fails; the one call must
//!   be exactly the line `        s(row.path.<escape>()),`, and `fn icon_of`
//!   must be declared exactly once in that file. Excepted: this file, which
//!   builds fixtures of it.
//! - JavaScript (`.js`, `.mjs`, `.ts`) in the files that handle a symbol
//!   source (they name `data-symbol`, `symbolSource`, `symbolPath`,
//!   `symbol-source`, a quoted `symbol:` or `icon|`), tests aside: any string
//!   literal that is exactly a bar or a colon (any quote), a regex on a bar
//!   or a colon (`\|`, `/:/`), or the even-odd marker's text (`evenodd:`),
//!   fails unless that exact line is allowed below, with why.
//!
//! The threat model (theme2): the guard stops ACCIDENTAL raw readers, code
//! that reaches for an icons row's path or a source form's `<path>` by hand.
//! Deliberate laundering of the escape (through a static, a const, a
//! closure, a macro, a re-export) is out of its scope and is caught by
//! review.
//!
//! The JS scan is a TRIPWIRE with a stated limit (theme2's ruling): it covers
//! only the files that name a symbol source, so a reader that never does
//! (say `node.props.imageSource.split("|")` in a file naming none) isn't
//! scanned. The enforcement is Rust's type, and the page's `iconForm`
//! (host/web-js/symbols.js) as the one JavaScript reader of a source string,
//! held to Icon::parse's cases and drawn by the web hole test. QUEUE.md names
//! the limit and the option of widening the selection.

use std::path::{Path, PathBuf};

/// The places a reader may be, from the repository's root.
const SCANNED: &[&str] = &[
    "kernel", "host", "js", "scripts", "contract", "runner", "bake", "plan/src",
];

/// The escape's name, assembled so this file holds it only in fixtures.
fn escape() -> String {
    ["raw", "for", "icon"].join("_")
}

/// The escape's definition line: the one occurrence of its name allowed in
/// plan/src/lib.rs.
const DEFINITION: (&str, &str) = (
    "plan/src/lib.rs",
    "    pub fn raw_for_icon(self) -> StrId {",
);

/// The file and function whose body holds the one call, and the call's
/// exact line.
const CALLER: (&str, &str) = ("runner/src/bridge.rs", "icon_of");
const CALL_LINE: &str = "        s(row.path.raw_for_icon()),";

/// The function a line declares, when it is a `fn` line.
fn declared_fn(line: &str) -> Option<&str> {
    let mut t = line.trim_start();
    for vis in ["pub(crate) ", "pub(super) ", "pub "] {
        t = t.strip_prefix(vis).unwrap_or(t);
    }
    let rest = t.strip_prefix("fn ")?;
    Some(rest.split(['(', '<']).next().unwrap_or("").trim())
}

/// The name of the function a line sits in: the nearest `fn` line above it.
fn enclosing_fn<'a>(lines: &[&'a str], at: usize) -> Option<&'a str> {
    lines[..at].iter().rev().find_map(|l| declared_fn(l))
}

/// The escape's occurrences in one `.rs` file: those allowed (`(definition,
/// call)` counts, the definition line in exact_plan and lines inside
/// `icon_of`'s body in exact_runner) and every other one, as `rel:line: text`.
/// The whole scan must find exactly one of each and no other (C2, the
/// escape named exactly twice).
fn rust_escapes(rel: &str, text: &str) -> ((usize, usize), Vec<String>) {
    let escape = escape();
    let lines: Vec<&str> = text.lines().collect();
    let (mut allowed, mut other) = ((0, 0), Vec::new());
    for (i, line) in lines.iter().enumerate() {
        let n = line.matches(&escape).count();
        if n == 0 {
            continue;
        }
        if (rel, *line) == DEFINITION && n == 1 {
            allowed.0 += 1;
        } else if rel == CALLER.0 && *line == CALL_LINE && enclosing_fn(&lines, i) == Some(CALLER.1)
        {
            allowed.1 += 1;
        } else {
            other.push(format!("{rel}:{}: {}", i + 1, line.trim()));
        }
    }
    // `fn icon_of` declared exactly once in its file (a second one, even in
    // a comment, could hold a second call that passes as the first's).
    if rel == CALLER.0 {
        let declared = lines
            .iter()
            .filter(|l| declared_fn(l) == Some(CALLER.1))
            .count();
        if declared != 1 {
            other.push(format!(
                "{rel}: `fn {}` declared {declared} times, not once",
                CALLER.1
            ));
        }
    }
    (allowed, other)
}

/// JavaScript lines allowed, by file and whole (trimmed) text, with why.
const LINES: &[(&str, &str, &str)] = &[
    ("host/web-js/symbols.js", r#"for (let i = 0; i < 3; i++) { const at = rest.indexOf("|"); if (at < 0) return null; f.push(rest.slice(0, at)); rest = rest.slice(at + 1); }"#, "iconForm, Icon::parse's JavaScript mirror: its four-part split"),
    ("host/web-js/symbols.js", r#"const [, box, raw] = f, colon = raw.indexOf(":");"#, "iconForm: the rule's marker"),
    ("host/web-js/symbols.js", r#"if (rule !== "evenodd" || !d.trim() || d.includes(":")) return null;"#, "iconForm: a second colon is no icon"),
    ("host/web-js/dom.js", "set cssText(t) { this.map.clear(); for (const d of t.split(';')) { const i = d.indexOf(':'); if (i > 0) this.map.set(d.slice(0, i).trim(), d.slice(i + 1).trim()); } }", "the DOM shim's style declarations"),
    ("host/web-js/rt.js", r#"if (typeof k + ":" + (Object.is(k, -0) ? 0 : k) !== r.k) break;"#, "a keyed list's key, type and value"),
    ("host/web-js/rt.js", r#"k = typeof k + ":" + (Object.is(k, -0) ? 0 : k);"#, "a keyed list's key, type and value"),
    ("host/web-js/rt.js", r#"if (next.has(k)) { const n = seen.get(k) ?? 1; seen.set(k, n + 1); k = "d" + n + ":" + k; journal.push(`each: repeated key ${k}`); }"#, "a repeated key's disambiguation"),
    ("host/web/glue.js", r#"const need=`surface.${op.mode==="capture"?"read":"write"} ${op.name}`;"#, "the ternary in a canvas surface's grant (`\"read\":\"write\"` holds a colon literal's text)"),
    ("host/web/navigation.js", "const namespace = at < 0 ? target.startsWith('/') ? '' : null : target.slice(0, at) + ':';", "a grant path's namespace (`pathTuple`)"),
    ("host/web/navigation.js", "const namespace = at < 0 ? path.startsWith('/') ? '' : null : path.slice(0, at) + ':';", "a grant path's namespace (`grantPathParts`)"),
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
                "target"
                    | "node_modules"
                    | "dist"
                    | "dist.gen"
                    | "dist.previous"
                    | "web-dist"
                    | "web-dist.gen"
                    | "web-dist.previous"
            ) && !name.starts_with('.')
                && !name.starts_with("dist.stage-")
                && !name.starts_with("web-dist.stage-")
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
        found.extend(rust_escapes(rel, text).1);
    } else if handles_a_source(text) {
        for (i, line) in text.lines().enumerate() {
            let hit = [
                "\"|\"", "'|'", "`|`", "\":\"", "':'", "`:`", "\\|", "/:/", "evenodd:",
            ]
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
    let mut escapes = (0, 0);
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
            if rel.ends_with(".rs") {
                let ((d, c), _) = rust_escapes(&rel, &text);
                escapes = (escapes.0 + d, escapes.1 + c);
            }
            s.into_iter().for_each(|k| seen[k] = true);
        }
    }
    assert!(
        found.is_empty(),
        "read an icon's path through exact_runner::icon_of / exact_kernel::Icon, not by hand:\n{}",
        found.join("\n")
    );
    // The escape is named exactly twice: its definition, and one call in
    // icon_of (a second call there, or a missing one, fails).
    assert_eq!(
        escapes,
        (1, 1),
        "the escape's definition and its one call in icon_of, each exactly once"
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
    // The definition line passes; a doc mention beside it fails; a call
    // that hides inside braces in comments (Astra's `// {` … `// }`) is a
    // second occurrence in icon_of, which the exactly-twice count fails.
    let allowed = "    pub fn raw_for_icon(self) -> StrId {\n        StrId(self.0)\n    }\n";
    assert!(readers("plan/src/lib.rs", allowed).0.is_empty());
    let doc = "/// The one way out, raw_for_icon.\n    pub fn raw_for_icon(self) -> StrId {\n";
    assert_eq!(
        readers("plan/src/lib.rs", doc).0.len(),
        1,
        "the doc mention"
    );
    let braces = format!("pub fn icon_of<'a>(plan: &Plan, row: &IconsRow) {{\n    // {{\n    let raw = row.path.{e}();\n    // }}\n        s(row.path.{e}()),\n}}\n");
    // Astra's duplicate line: a second helper in the same file with the
    // identical call line fails; a second call inside icon_of counts twice.
    let duplicate = format!("pub fn icon_of<'a>(plan: &Plan, row: &IconsRow) {{\n        s(row.path.{e}()),\n}}\nfn other(plan: &Plan, row: &IconsRow) {{\n        s(row.path.{e}()),\n}}\n");
    let (counts, other) = rust_escapes("runner/src/bridge.rs", &duplicate);
    assert_eq!((counts, other.len()), ((0, 1), 1), "{other:?}");
    assert!(other[0].contains(":5:"), "{other:?}");
    let twice = format!("pub fn icon_of<'a>(plan: &Plan, row: &IconsRow) {{\n        s(row.path.{e}()),\n        s(row.path.{e}()),\n}}\n");
    assert_eq!(
        rust_escapes("runner/src/bridge.rs", &twice).0,
        (0, 2),
        "two calls: the total check fails them"
    );
    let elsewhere =
        format!("pub fn icon_of_too(row: &IconsRow) {{\n        s(row.path.{e}()),\n}}\n");
    let (counts, other) = rust_escapes("runner/src/bridge.rs", &elsewhere);
    assert_eq!(
        (counts, other.len()),
        ((0, 0), 2),
        "a function only named alike: {other:?}"
    );
    // Astra's `// {` … `// }` case: the hidden call is not the exact call
    // line, so it fails; the real call line is the one allowed.
    let (counts, other) = rust_escapes("runner/src/bridge.rs", &braces);
    assert_eq!((counts, other.len()), ((0, 1), 1), "{other:?}");
    assert!(other[0].contains("let raw"), "{other:?}");
    // A second `fn icon_of` (in a block comment, or nested) fails, whatever
    // it holds: the function is declared exactly once.
    for second in [
        format!("/*\npub fn icon_of<'a>(plan: &Plan, row: &IconsRow) {{\n*/\npub fn icon_of<'a>(plan: &Plan, row: &IconsRow) {{\n        s(row.path.{e}()),\n}}\n"),
        format!("pub fn icon_of<'a>(plan: &Plan, row: &IconsRow) {{\n    fn icon_of(row: &IconsRow) {{\n        s(row.path.{e}()),\n    }}\n}}\n"),
    ] {
        let other = rust_escapes("runner/src/bridge.rs", &second).1;
        assert!(other.iter().any(|o| o.contains("`fn icon_of` declared 2 times")), "{other:?}");
    }
    // A call line not exactly the one (spaced otherwise) fails.
    let spaced =
        format!("pub fn icon_of<'a>(plan: &Plan, row: &IconsRow) {{\n    s(row.path.{e}()),\n}}\n");
    assert_eq!(rust_escapes("runner/src/bridge.rs", &spaced).1.len(), 1);
    let js = |body: &str| readers("x/a.js", &format!("// handles data-symbol-source\n{body}")).0;
    for bypass in [
        r#"const f = name.split("|");"#,
        "const f = name.split('|');",
        "const f = name.split(`|`);",
        r#"const f = name.split ("|", 4);"#,
        r#"const f = name.split?.("|");"#,
        r#"const f = name.split(("|"));"#,
        r"const f = name.split(/\|/);",
        "const at = name.search(/:/);",
        "const at = name.indexOf(':');",
        // The marker's text in a selected file (round 5 restores it).
        "const rule = d.startsWith('evenodd:');",
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
