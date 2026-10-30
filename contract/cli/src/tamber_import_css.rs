//! @ref LLP 1153 D6.2 — `contract tamber import css <stylesheet> [--app <dir>]`:
//! a stylesheet's custom properties as tokens of the app's design, written
//! in its `theme` block between `// tamber:import <file>` and
//! `// tamber:end`. A later import of the same file rewrites that block and
//! nothing else, so an edited stylesheet is imported again rather than
//! translated by hand; the author's tokens outside the markers stay as
//! written. With no design, `app.tamber` is written.
//!
//! What it translates: each `--name: value` on `:root` is the token `name`,
//! of the kind the evaluator infers; a redeclaration under
//! `@media (prefers-color-scheme: dark)` makes the two a `light-dark()`, which
//! is what that query means; `--a: var(--b)` is a reference to the token
//! `b`. Everything else is refused by name, and nothing is written for it: a
//! scope the page switches (`.dark`, `[data-theme="dark"]`), which is an app
//! axis the author declares; a value composed around a `var()`; a value the
//! evaluator, or the compiler after it, refuses. The import exits 1 when it
//! refused anything, having written the rest.

use exact_tamber::import::css_read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: contract tamber import css <stylesheet> [--app <app dir>]";

const END: &str = "// tamber:end";

/// One token the stylesheet declares: its name, its value as the design
/// writes it, and what was dropped from it.
struct Token {
    name: String,
    value: String,
    note: Option<&'static str>,
}

pub fn run(args: &[String]) -> ExitCode {
    let (mut source, mut app) = (None, None);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--app" if i + 1 < args.len() => {
                app = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            word if !word.starts_with('-') && source.is_none() => {
                source = Some(PathBuf::from(word));
                i += 1;
            }
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(source) = source else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let css = match std::fs::read_to_string(&source) {
        Ok(css) => css,
        Err(e) => {
            eprintln!("contract tamber import css: {}: {e}", source.display());
            return ExitCode::from(2);
        }
    };
    let app = app.unwrap_or_else(|| PathBuf::from("."));
    let dir = if app.is_dir() {
        app.clone()
    } else {
        app.parent().map(Path::to_path_buf).unwrap_or_default()
    };
    // The design the compiler reads: the file app.json's "tamber" names, else
    // `app.tamber` beside `app.contract`.
    let path = crate::tamber::design(&app)
        .or_else(|| contract::design_path(&dir.join("app.contract")))
        .unwrap_or_else(|| dir.join("app.tamber"));
    let current = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            eprintln!("contract tamber import css: {}: {e}", path.display());
            return ExitCode::from(2);
        }
    };
    let key = marker_key(&source, &dir);
    let start = format!("// tamber:import {key}");
    let (base, at) = match without_block(current.as_deref().unwrap_or("tamber app\n"), &start) {
        Ok(found) => found,
        Err(why) => {
            eprintln!("contract tamber import css: {}: {why}", path.display());
            return ExitCode::from(2);
        }
    };
    let own = match exact_tamber::parse(&base) {
        Ok(design) => design,
        Err(all) => {
            for d in &all {
                eprintln!("{}:{d}", path.display());
            }
            eprintln!("contract tamber import css: the design does not parse; nothing written");
            return ExitCode::from(1);
        }
    };
    let taken: Vec<String> = own
        .tokens
        .iter()
        .chain(&own.icons)
        .map(|t| t.name.value.clone())
        .chain(own.dials.iter().map(|d| d.name.value.clone()))
        .collect();
    let (mut tokens, mut refused) = read(&css);
    tokens.retain(|t| {
        let outside = taken.contains(&t.name);
        if outside {
            refused.push((
                format!("--{}", t.name),
                format!(
                    "the design declares `{}` outside the import's markers, and that token stays",
                    t.name
                ),
            ));
        }
        !outside
    });
    // Each token the evaluator or the compiler refuses is dropped, and the
    // rest checked again: a reference to a dropped one is refused in turn.
    let vocab = contract_lower::tamber_vocab();
    let (text, kinds) = loop {
        let (text, first) = render(&base, at, &start, &tokens);
        let block = first..first + tokens.len();
        let (lines, kinds) = match exact_tamber::check(&text, &vocab) {
            Ok(folding) => {
                let kinds: Vec<(String, &'static str)> = folding
                    .tokens
                    .iter()
                    .map(|t| (t.name.clone(), t.kind.name()))
                    .collect();
                let compiled = crate::tamber::admission(&text, &[], None).unwrap_or_default();
                (
                    compiled
                        .into_iter()
                        .map(|(_, line, _, id, message)| (line as usize, id, message))
                        .collect(),
                    kinds,
                )
            }
            Err(all) => (
                all.into_iter()
                    .map(|d| (d.line as usize, d.id.to_string(), d.message))
                    .collect::<Vec<_>>(),
                Vec::new(),
            ),
        };
        let mut dropped = vec![false; tokens.len()];
        for (line, id, message) in lines {
            if block.contains(&line) && !dropped[line - first] {
                dropped[line - first] = true;
                let t = &tokens[line - first];
                refused.push((
                    format!("--{}", t.name),
                    format!("`{} = {}`: [{id}] {message}", t.name, t.value),
                ));
            }
        }
        if !dropped.contains(&true) {
            break (text, kinds);
        }
        let mut k = 0;
        tokens.retain(|_| {
            k += 1;
            !dropped[k - 1]
        });
    };
    if let Err(e) = std::fs::write(&path, &text) {
        eprintln!("contract tamber import css: {}: {e}", path.display());
        return ExitCode::from(2);
    }
    println!(
        "{} → {}: {} token{} between `{start}` and `{END}`{}",
        source.display(),
        path.display(),
        tokens.len(),
        if tokens.len() == 1 { "" } else { "s" },
        match refused.len() {
            0 => String::new(),
            n => format!(", {n} refused"),
        }
    );
    for t in &tokens {
        let kind = kinds
            .iter()
            .find(|(n, _)| *n == t.name)
            .map_or("", |(_, k)| k);
        let note = t.note.map_or(String::new(), |n| format!("  ({n})"));
        println!("  {} = {}  {kind}{note}", t.name, t.value);
    }
    for (name, why) in &refused {
        println!("  refused {name}: {why}");
    }
    if refused.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// The stylesheet's tokens in the order it declares them, and each custom
/// property refused, with why.
fn read(css: &str) -> (Vec<Token>, Vec<(String, String)>) {
    let (blocks, deep) = css_read(css);
    let mut refused: Vec<(String, String)> = deep
        .into_iter()
        .map(|d| (d.prelude, format!("line {}: {}, not read", d.line, d.why)))
        .collect();
    // Each name: its light (`:root`) and dark values, last declaration winning.
    let mut sides: Vec<(String, Option<String>, Option<String>)> = Vec::new();
    for block in &blocks {
        let dark = match scope(&block.context) {
            Ok(dark) => dark,
            Err(why) => {
                for (name, _) in &block.decls {
                    refused.push((name.clone(), why.clone()));
                }
                continue;
            }
        };
        for (name, value) in &block.decls {
            let at = match sides.iter().position(|(n, ..)| n == name) {
                Some(at) => at,
                None => {
                    sides.push((name.clone(), None, None));
                    sides.len() - 1
                }
            };
            let side = if dark {
                &mut sides[at].2
            } else {
                &mut sides[at].1
            };
            *side = Some(value.clone());
        }
    }
    let mut tokens = Vec::new();
    for (var, light, dark) in sides {
        let name = var.trim_start_matches('-').to_string();
        if !exact_tamber::is_name(&name) {
            refused.push((
                var,
                format!(
                    "`{name}` is no Tamber token name: one or more identifiers, joined by dots"
                ),
            ));
            continue;
        }
        let Some(light) = light else {
            refused.push((
                var,
                "declared only under `prefers-color-scheme: dark`: `:root` gives it no light side"
                    .into(),
            ));
            continue;
        };
        let (light, mut note) = match value(&light) {
            Ok(v) => v,
            Err(why) => {
                refused.push((var, why));
                continue;
            }
        };
        let value = match dark.map(|d| value(&d)) {
            None => light,
            Some(Err(why)) => {
                refused.push((var, format!("its dark value: {why}")));
                continue;
            }
            Some(Ok((dark, dropped))) => {
                note = note.or(dropped);
                match dark == light {
                    true => light,
                    false => format!("light-dark({light}, {dark})"),
                }
            }
        };
        tokens.push(Token { name, value, note });
    }
    (tokens, refused)
}

/// Whether a block's context is the dark side of the page's root (`true`)
/// or its light side, or why it is neither.
fn scope(context: &[String]) -> Result<bool, String> {
    let squash = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase()
    };
    // `html` is the root of an HTML page, as `:root` is.
    let root = |s: &str| matches!(squash(s).as_str(), ":root" | "html");
    match context {
        [r] if root(r) => Ok(false),
        [media, r]
            if root(r)
                && matches!(
                    squash(media).as_str(),
                    "@media (prefers-color-scheme: dark)" | "@media (prefers-color-scheme:dark)"
                ) =>
        {
            Ok(true)
        }
        _ => {
            let shown = context.join(" { ") + &" }".repeat(context.len().saturating_sub(1));
            let switched = context.iter().any(|c| c.contains('.') || c.contains('['));
            Err(match switched {
                true => format!("`{shown}` is a scope the page switches: in Tamber that is an app axis, which the design declares (`axes`, docs/recipes/tamber.md)"),
                false => format!("`{shown}` is neither block the import reads: `:root` (or `html`), and `:root` inside `@media (prefers-color-scheme: dark)`"),
            })
        }
    }
}

/// A custom property's value as a token's: on one line, a whole `var()` as
/// the name it references; what was dropped from it, if anything.
fn value(raw: &str) -> Result<(String, Option<&'static str>), String> {
    let mut v = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let important = v.to_ascii_lowercase().rfind("!important");
    if let Some(at) = important.filter(|at| v[at + 10..].trim().is_empty()) {
        v = v[..at].trim_end().to_string();
    }
    if v.is_empty() {
        return Err("an empty value".into());
    }
    let lower = v.to_ascii_lowercase();
    if !lower.contains("var(") {
        return Ok((v, None));
    }
    // One `var(--b)`, or `var(--b, fallback)`, and nothing around it.
    let inner = lower
        .strip_prefix("var(")
        .and_then(|_| v[4..].strip_suffix(')'))
        .map(str::trim)
        .and_then(|i| i.strip_prefix("--"));
    let whole = inner.and_then(|i| {
        let (name, fallback) = match i.split_once(',') {
            Some((n, f)) => (n.trim(), Some(f)),
            None => (i.trim(), None),
        };
        (exact_tamber::is_name(name) && !name.contains('('))
            .then(|| (name.to_string(), fallback.is_some()))
    });
    match whole {
        Some((name, fallback)) => Ok((name, fallback.then_some("its fallback dropped: a token is never undefined"))),
        None => Err(format!("`{v}` is composed around a `var()`: Tamber writes the value it makes, or an expression over the tokens it names bare (`color-mix(in srgb, brand 60%, white)`)")),
    }
}

/// The source's name in the markers: its path from the app's directory, as
/// written when it is outside.
fn marker_key(source: &Path, dir: &Path) -> String {
    let canonical = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let (file, dir) = (canonical(source), canonical(dir));
    match file.strip_prefix(&dir) {
        Ok(inside) => inside.display().to_string(),
        Err(_) => source.display().to_string(),
    }
}

/// `design` without the block `start` opens, and the line it stood at.
fn without_block(design: &str, start: &str) -> Result<(String, Option<usize>), String> {
    let lines: Vec<&str> = design.lines().collect();
    let Some(open) = lines.iter().position(|l| l.trim() == start) else {
        return Ok((design.to_string(), None));
    };
    let Some(close) = lines[open..].iter().position(|l| l.trim() == END) else {
        return Err(format!(
            "`{start}` has no `{END}` after it: mend the markers, then import"
        ));
    };
    let mut kept: Vec<&str> = lines[..open].to_vec();
    kept.extend(&lines[open + close + 1..]);
    Ok((kept.join("\n") + "\n", Some(open)))
}

/// The design with the block written: where it stood (`at`), else at the end
/// of the `theme` block, else in a `theme` block of its own at the end. The
/// text, and the line (1-based) of the block's first token.
fn render(base: &str, at: Option<usize>, start: &str, tokens: &[Token]) -> (String, usize) {
    let mut lines: Vec<String> = base.lines().map(str::to_string).collect();
    let theme = lines.iter().position(|l| l.trim_end() == "theme");
    let (at, indent) = match (at, theme) {
        (Some(at), _) => (at, theme_indent(&lines, theme)),
        (None, Some(t)) => {
            // After the block's last indented line: a line at the margin,
            // a comment's too, opens what follows it.
            let mut end = t + 1;
            for (k, l) in lines.iter().enumerate().skip(t + 1) {
                if !l.trim().is_empty() && !l.starts_with(' ') {
                    break;
                }
                if !l.trim().is_empty() {
                    end = k + 1;
                }
            }
            (end, theme_indent(&lines, theme))
        }
        (None, None) => {
            while lines.last().is_some_and(|l| l.trim().is_empty()) {
                lines.pop();
            }
            lines.push(String::new());
            lines.push("theme".into());
            (lines.len(), "  ".to_string())
        }
    };
    let mut block = vec![format!("{indent}{start}")];
    block.extend(
        tokens
            .iter()
            .map(|t| format!("{indent}{} = {}", t.name, t.value)),
    );
    block.push(format!("{indent}{END}"));
    lines.splice(at..at, block);
    (lines.join("\n") + "\n", at + 2)
}

/// The indent of the `theme` block's entries, two spaces when it has none.
fn theme_indent(lines: &[String], theme: Option<usize>) -> String {
    theme
        .and_then(|t| lines.get(t + 1..)?.iter().find(|l| !l.trim().is_empty()))
        .filter(|l| l.starts_with(' '))
        .map(|l| " ".repeat(l.len() - l.trim_start().len()))
        .unwrap_or_else(|| "  ".to_string())
}
