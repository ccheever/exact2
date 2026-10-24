//! @ref LLP 1121 stage 14 C2 (theme2's condition) — an even-odd icon's path
//! starts `evenodd:`, in the plan's `icons` row and in the source form's
//! `<path>`; every reader strips it through ONE shared parse,
//! `exact_kernel::Icon` (`Icon::parse` for a source, `Icon::of_row` for a
//! row). This scan of the kernel, the hosts, the runtimes and the tools finds
//! any other reader (C2 round 1, Astra 4 = Grok: hardened):
//! - in Rust, a row of the plan's `icons` (a binding a `for` over the table,
//!   a closure on it, a `let` of it or an `IconsRow` parameter introduces)
//!   whose `.path` is acquired outside the arguments of an `Icon::of_row(…)`
//!   call: `s(i.path.0)`, `plan.strings.get(i.path.0 as usize)`,
//!   `lookup(row.path)` and `let p = i.path;` alike, whatever reads it after;
//! - in any file that writes the source form (`icon|`, either quote), a
//!   split or search on `|` of a name, source or form, or a split, slice,
//!   search or strip on a line naming the form.
//!
//! The allowed readers are whole functions and single lines, named below.

use std::path::{Path, PathBuf};

/// The places a reader may be, from the repository's root.
const SCANNED: &[&str] = &[
    "kernel", "host", "js", "scripts", "contract", "runner", "bake", "plan/src",
];

/// The readers allowed: a file and the first line of the function (its
/// whole body is allowed), each with why.
const ALLOWED: &[(&str, &str, &str)] = &[
    (
        "kernel/src/replaced.rs",
        "    pub fn parse(name: &'a str) -> Option<Icon<'a>> {",
        "Icon::parse itself: the one reader",
    ),
    // The JS target's page reads a computed source with no Rust to call:
    // Icon::parse mirrored once, held to the kernel's own cases
    // (kernel/tests/fixtures/icon-forms.json) by tests/symbol-hole.test.mjs.
    (
        "host/web-js/symbols.js",
        "const iconForm = name => {",
        "the page's JavaScript mirror of Icon::parse",
    ),
    // Known exception (iosfix's note after theme2's approval): Apple's
    // `icon_name` compares the raw row's path against Icon::parse's stripped
    // one, so it misses an even-odd icon's name. iosfix's Apple commit routes
    // it through `Icon::of_row` and removes this entry; the scan then fails
    // until it is gone ("no longer present"), so the exception can't outlive it.
    (
        "host/apple/src/icon_path.rs",
        "pub(crate) fn icon_name<'a>(",
        "iosfix's commit replaces it with Icon::of_row",
    ),
];

/// Single lines that touch a form and read no path, each with why.
const LINES_ALLOWED: &[(&str, &str, &str)] = &[(
    "host/web-js/symbols.js",
    r#"if (!Table[v.slice(7)] && !All && !v.startsWith("symbol:icon|")) everyRole();"#,
    "slices `symbol:` off a source to look a role up; a form is never looked up",
)];

/// The bindings in Rust `text` that hold a row of the plan's `icons`.
fn row_bindings(text: &str) -> Vec<String> {
    let ident = |s: &str| {
        s.chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>()
    };
    let mut out = Vec::new();
    for (at, _) in text.match_indices(".icons") {
        if text[at + 6..]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            continue;
        }
        // The statement the table is read in: back to its start, on to its `;`.
        let start = text[..at].rfind([';', '{', '}']).map_or(0, |i| i + 1);
        let end = text[at..].find(';').map_or(text.len(), |i| at + i);
        let statement = &text[start..end];
        let head = statement.trim_start();
        if let Some(rest) = head.strip_prefix("for ") {
            out.push(ident(rest.trim_start_matches(['&', '('])));
        }
        if let Some(rest) = head.strip_prefix("let ") {
            out.push(ident(
                rest.trim_start_matches("mut ").trim_start_matches("Some("),
            ));
        }
        // Each closure on it: `|i|`, `|&i|`, `|row|`.
        let mut rest = &text[at..end];
        while let Some(open) = rest.find('|') {
            let after = &rest[open + 1..];
            let Some(close) = after.find('|') else { break };
            let name = ident(after[..close].trim_start_matches('&'));
            if !name.is_empty()
                && after[..close]
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '&' || c == ' ' || c == ':')
            {
                out.push(name);
            }
            rest = &after[close + 1..];
        }
    }
    // A parameter typed as a row: `row: &exact_plan::IconsRow`.
    for (at, _) in text.match_indices("IconsRow") {
        let path =
            text[..at].trim_end_matches(|c: char| c.is_alphanumeric() || c == '_' || c == ':');
        let typed = path.trim_end_matches(['&', ' ']);
        if let Some(head) = typed.strip_suffix(':').filter(|h| !h.ends_with(':')) {
            let name: String = head
                .trim_end()
                .chars()
                .rev()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            out.push(name);
        }
    }
    out.retain(|n| !n.is_empty() && n != "_");
    out.sort();
    out.dedup();
    out
}

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

/// The byte spans of every `Icon::of_row(…)` call's arguments in `text`.
fn reader_calls(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    for (at, call) in text.match_indices("Icon::of_row(") {
        let start = at + call.len();
        let mut depth = 1;
        for (i, c) in text[start..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            if depth == 0 {
                spans.push((start, start + i));
                break;
            }
        }
    }
    spans
}

/// The line ranges (1-based, inclusive) of the allowed functions in `rel`.
fn allowed_lines(rel: &str, text: &str) -> Vec<(usize, usize, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (k, (file, first, _)) in ALLOWED.iter().enumerate() {
        if *file != rel {
            continue;
        }
        let Some(at) = lines.iter().position(|l| l.starts_with(first)) else {
            continue;
        };
        let mut depth = 0i32;
        let mut opened = false;
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

/// Every reader in one file (`rel` from the root), as `line: text`; and
/// which allowed functions it holds.
fn readers(rel: &str, text: &str) -> (Vec<String>, Vec<usize>) {
    let allowed = allowed_lines(rel, text);
    let in_allowed = |n: usize| allowed.iter().any(|(_, a, b)| (*a..=*b).contains(&n));
    let line_of = |at: usize| text[..at].matches('\n').count() + 1;
    let mut found = std::collections::BTreeMap::new();
    // A row's path, acquired outside a reader call, in Rust.
    let rows = if rel.ends_with(".rs") {
        row_bindings(text)
    } else {
        Vec::new()
    };
    if !rows.is_empty() {
        let calls = reader_calls(text);
        for (at, _) in text.match_indices(".path") {
            let owner: String = text[..at]
                .chars()
                .rev()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let after = text[at + 5..].chars().next();
            let field = !after.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '(');
            if rows.contains(&owner) && field && !calls.iter().any(|(a, b)| (*a..*b).contains(&at))
            {
                let n = line_of(at);
                found.insert(n, text.lines().nth(n - 1).unwrap_or("").trim().to_string());
            }
        }
    }
    // A source form split, searched or sliced by hand.
    if text.contains("icon|") {
        for (i, line) in text.lines().enumerate() {
            // A `|` split or search whose receiver is a name, source or form.
            let on_bar = [
                "split('|')",
                "split(\"|\")",
                "split(`|`)",
                "indexOf('|')",
                "indexOf(\"|\")",
                "find('|')",
                "find(\"|\")",
                "splitn(",
            ]
            .iter()
            .flat_map(|w| line.match_indices(w).map(move |(at, _)| (at, *w)))
            .filter(|(at, w)| {
                !w.starts_with("splitn")
                    || line[*at..]
                        .split(')')
                        .next()
                        .is_some_and(|args| args.contains("'|'") || args.contains("\"|\""))
            })
            .any(|(at, _)| {
                let receiver: String = line[..at.saturating_sub(1)]
                    .chars()
                    .rev()
                    .take_while(|c| c.is_alphanumeric() || "_.()[]?".contains(*c))
                    .collect::<String>()
                    .to_ascii_lowercase();
                ["eman", "ecruos", "mrof", "lobmys", "noci", "elor", "crs"]
                    .iter()
                    .any(|w| receiver.contains(w))
            });
            let on_form = line.contains("icon|")
                && [
                    "split",
                    "slice",
                    "substring",
                    "substr(",
                    "strip_prefix",
                    "indexOf",
                    "find(",
                    "splitn",
                ]
                .iter()
                .any(|w| line.contains(w));
            let known = LINES_ALLOWED
                .iter()
                .any(|(f, l, _)| *f == rel && line.trim() == *l);
            if (on_bar || on_form) && !known {
                found.insert(i + 1, line.trim().to_string());
            }
        }
    }
    let lines = found
        .into_iter()
        .filter(|(n, _)| !in_allowed(*n))
        .map(|(n, l)| format!("{rel}:{n}: {l}"))
        .collect();
    (lines, allowed.into_iter().map(|(k, _, _)| k).collect())
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
            let (lines, allowed) = readers(&rel, &text);
            found.extend(lines);
            for k in allowed {
                seen[k] = true;
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

/// Each bypass the review found is caught, and what is no reader isn't.
#[test]
fn the_scan_catches_each_bypass() {
    let rust = |body: &str| {
        readers(
            "x/a.rs",
            &format!(
                "fn f(plan: &Plan) {{\n    for i in plan.icons.iter() {{\n{body}\n    }}\n}}\n"
            ),
        )
        .0
    };
    for bypass in [
        "        let d = s(i.path.0);",
        "        let p = i.path;\n        let d = s(p.0);",
        "        let d = plan.strings.get(i.path.0 as usize);",
        "        let d = lookup(i.path);",
        "        let d = plan.str(i.path);",
        // A raw read on the line after a reader call is outside its arguments.
        "        let icon = exact_kernel::Icon::of_row(s(i.sf.0), s(i.view_box.0), s(i.path.0), s(i.svg.0));\n        let raw = s(i.path.0);",
    ] {
        assert!(!rust(bypass).is_empty(), "missed: {bypass}");
    }
    let js = |body: &str| readers("x/a.js", body).0;
    for bypass in [
        r#"const f = name.startsWith("icon|") ? name.split("|") : null;"#,
        "const f = name.startsWith('icon|') ? name.split('|') : null;",
        "const at = name.indexOf('|'); // icon|",
        r#"const path = source.slice("icon|".length).split(':');"#,
        r#"const rest = source.substring(5); const ok = source.startsWith("icon|");"#,
        "const parts = `icon|${a}`.split(`|`);",
    ] {
        assert!(!js(bypass).is_empty(), "missed: {bypass}");
    }
    // What is no reader: a reader call's own arguments, wrapped or not; a
    // path that is no row's (a file's, a method); `|` split where no form is.
    assert!(rust("        let icon = exact_kernel::Icon::of_row(s(i.sf.0), s(i.view_box.0), s(i.path.0), s(i.svg.0));").is_empty());
    assert!(rust("        let icon = exact_kernel::Icon::of_row(\n            s(i.sf.0),\n            s(i.view_box.0),\n            s(i.path.0),\n            s(i.svg.0),\n        );").is_empty());
    assert!(
        rust("        let at = sources.path(c.span);\n        let p = entry.path();").is_empty()
    );
    assert!(
        readers("x/b.rs", "fn g(s: &Shape) { for seg in &s.path.0 {} }")
            .0
            .is_empty(),
        "no icons table, no row"
    );
    assert!(
        readers("x/b.js", "const values = text.split('|');")
            .0
            .is_empty(),
        "no form in the file"
    );
    assert!(readers("x/c.rs", "fn h() {\n    let t = \"symbol:icon|a\";\n    let v = plan.str(a.values).split('|');\n}").0.is_empty(), "a `|` split of no name or form");
    assert!(readers("x/d.rs", "fn k(plan: &Plan) { let icon = Icon::parse(s)?; let d = icon.path; for s in graph.sources.iter() { s.path; } let n = plan.icons.len(); }").0.is_empty(), "an Icon's path, a source's path");
    // A row as a parameter, a `let` and a `for` binding are rows.
    assert!(!readers(
        "x/e.rs",
        "fn icon_path(plan: &Plan, row: &exact_plan::IconsRow) -> &str { plan.str(row.path) }"
    )
    .0
    .is_empty());
    assert!(!readers("x/f.rs", "fn m(plan: &Plan) { let glyph = plan.icons.iter().find(|g| g.sf.0 == 0).unwrap();\n    let d = plan.str(glyph.path); }").0.is_empty());
    assert!(!readers("x/g.rs", "fn n(plan: &Plan) { for row in &plan.icons {\n        let d = plan.str(row.path);\n    } }").0.is_empty());
}
