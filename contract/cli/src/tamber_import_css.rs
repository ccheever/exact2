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
//! `b`. Each scheme's value is the declaration that wins it, as CSS's cascade
//! picks one. Everything else is refused by name, and nothing is written for
//! it: a scope the page switches (`.dark`, `[data-theme="dark"]`), which is
//! an app axis the author declares; a value composed around a `var()`; a
//! value the evaluator, or the compiler after it, refuses. The import exits 1
//! when it refused anything, having written the rest. When the design it
//! would write is refused outside the markers (a token of the author's
//! naming one the stylesheet no longer declares), it writes nothing.

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
    let built = dir.join("app.contract");
    let app = match built.is_file() {
        true => contract::tamber_params::params_of(&built),
        false => contract::tamber_params::AppParams::default(),
    };
    let (text, kinds, outside) = loop {
        let (text, first) = render(&base, at, &start, &tokens);
        let block = first..first + tokens.len();
        let (said, kinds) = match checked(&text, &path, &built, &dir, &app) {
            Ok(checked) => checked,
            Err(why) => {
                eprintln!("contract tamber import css: {why}; nothing written");
                return ExitCode::from(2);
            }
        };
        let (mut dropped, mut outside) = (vec![false; tokens.len()], Vec::new());
        for refusal in said {
            match refusal.line.filter(|line| block.contains(line)) {
                Some(line) if !dropped[line - first] => {
                    dropped[line - first] = true;
                    let t = &tokens[line - first];
                    let why = format!("[{}] {}", refusal.id, refusal.message);
                    refused.push((
                        format!("--{}", t.name),
                        format!("`{} = {}`: {why}", t.name, t.value),
                    ));
                }
                Some(_) => {}
                None => outside.push(refusal.shown),
            }
        }
        if !dropped.contains(&true) {
            break (text, kinds, outside);
        }
        let mut k = 0;
        tokens.retain(|_| {
            k += 1;
            !dropped[k - 1]
        });
    };
    let say_refused = || {
        for (name, why) in &refused {
            println!("  refused {name}: {why}");
        }
    };
    if !outside.is_empty() {
        eprintln!(
            "contract tamber import css: with this import, {} is refused outside the import's markers, so it is left as it was:",
            path.display()
        );
        for shown in &outside {
            eprintln!("  {shown}");
        }
        say_refused();
        return ExitCode::from(1);
    }
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
    say_refused();
    if refused.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// One refusal of a candidate design: the design's line it is on (`None`
/// for another file's), its id and words, and how it is said.
struct Refusal {
    line: Option<usize>,
    id: String,
    message: String,
    shown: String,
}

/// Every refusal of `text` as the design at `path`, as `contract tamber
/// check` makes them: the evaluator's fold with the app's params and its
/// packages' designs, then the compiler's build; and each token's kind when
/// it folds. An `Err` when the build could not be made.
#[allow(clippy::type_complexity)]
fn checked(
    text: &str,
    path: &Path,
    built: &Path,
    dir: &Path,
    app: &contract::tamber_params::AppParams,
) -> Result<(Vec<Refusal>, Vec<(String, &'static str)>), String> {
    let vocab = contract_lower::tamber_vocab();
    let folded = match (app.tokens.is_empty(), app.packages.is_empty()) {
        (true, true) => exact_tamber::check(text, &vocab),
        (_, false) => contract::tamber_params::fold_with(text, app, &vocab).map(|(f, _)| f),
        (false, true) => exact_tamber::parse(text).and_then(|d| {
            exact_tamber::check_design(contract::tamber_params::laid(&d, app), &vocab)
        }),
    };
    let folding = match folded {
        Ok(folding) => folding,
        Err(all) => {
            let said = all.iter().map(|d| Refusal {
                line: Some(d.line as usize),
                id: d.id.to_string(),
                message: d.message.clone(),
                shown: contract::tamber_params::located(d, app, path),
            });
            return Ok((said.collect(), Vec::new()));
        }
    };
    let kinds = (folding.tokens.iter())
        .map(|t| (t.name.clone(), t.kind.name()))
        .collect();
    let svgs = crate::tamber::svgs(&folding, Some(dir), app);
    // An app with params or packages' designs is admitted by its own build,
    // which reads this text in place of the file's.
    let whole = !app.tokens.is_empty() || app.failed || !app.packages.is_empty();
    let compiled = match whole && path.is_file() {
        true => contract::with_design_text(path, text.to_string(), || {
            crate::tamber::admission(text, &svgs, Some(built.to_path_buf()))
        })?,
        false => crate::tamber::admission(text, &svgs, None)?,
    };
    let canonical = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let said = compiled.into_iter().map(|(file, line, col, id, message)| {
        let design = file
            .as_deref()
            .is_none_or(|f| canonical(f) == canonical(path));
        let at = file.unwrap_or_else(|| path.to_path_buf());
        Refusal {
            line: design.then_some(line as usize),
            shown: format!("{}:{line}:{col}: {id}: {message}", at.display()),
            id,
            message,
        }
    });
    Ok((said.collect(), kinds))
}

/// One custom property declaration read: its name, its value, whether only
/// the dark scheme applies it, and what decides between two for one name
/// (CSS Cascade 4 §6: importance, then specificity, then order of
/// appearance).
struct Decl {
    name: String,
    value: String,
    dark: bool,
    rank: (bool, u8, usize),
}

/// The stylesheet's tokens in the order it first declares them, and each
/// custom property refused, with why.
fn read(css: &str) -> (Vec<Token>, Vec<(String, String)>) {
    let (blocks, deep) = css_read(css);
    let mut refused: Vec<(String, String)> = deep
        .into_iter()
        .map(|d| (d.prelude, format!("line {}: {}, not read", d.line, d.why)))
        .collect();
    let mut decls: Vec<Decl> = Vec::new();
    for block in &blocks {
        let (dark, specificity) = match scope(&block.context) {
            Ok(scope) => scope,
            Err(why) => {
                for (name, _) in &block.decls {
                    refused.push((name.clone(), why.clone()));
                }
                continue;
            }
        };
        for (name, raw) in &block.decls {
            let (value, important) = importance(raw);
            let rank = (important, specificity, decls.len());
            let name = name.clone();
            decls.push(Decl {
                name,
                value,
                dark,
                rank,
            });
        }
    }
    let mut names: Vec<&str> = Vec::new();
    for d in &decls {
        if !names.contains(&d.name.as_str()) {
            names.push(&d.name);
        }
    }
    // The declaration that wins `name` in a scheme: the light one reads
    // `:root` alone, the dark one `:root` and the dark query's.
    let winner = |name: &str, dark: bool| {
        (decls.iter())
            .filter(|d| d.name == name && (dark || !d.dark))
            .max_by_key(|d| d.rank)
    };
    let mut tokens = Vec::new();
    for var in names {
        let Some(name) = var.strip_prefix("--").filter(|n| exact_tamber::is_name(n)) else {
            refused.push((
                var.to_string(),
                format!("`{var}` names no Tamber token: past its `--`, a name is identifiers joined by dots"),
            ));
            continue;
        };
        let Some(light) = winner(var, false) else {
            refused.push((
                var.to_string(),
                "declared only under `prefers-color-scheme: dark`: `:root` gives it no light side"
                    .into(),
            ));
            continue;
        };
        let (light_value, mut note) = match value(&light.value) {
            Ok(v) => v,
            Err(why) => {
                refused.push((var.to_string(), why));
                continue;
            }
        };
        let dark = winner(var, true).filter(|d| d.rank != light.rank);
        let value = match dark.map(|d| value(&d.value)) {
            None => light_value,
            Some(Err(why)) => {
                refused.push((var.to_string(), format!("its dark value: {why}")));
                continue;
            }
            Some(Ok((dark, dropped))) => {
                note = note.or(dropped);
                match dark == light_value {
                    true => light_value,
                    false => format!("light-dark({light_value}, {dark})"),
                }
            }
        };
        let name = name.to_string();
        tokens.push(Token { name, value, note });
    }
    (tokens, refused)
}

/// A declaration's value without its `!important`, and whether it had one.
fn importance(raw: &str) -> (String, bool) {
    let v = raw.trim();
    let lower = v.to_ascii_lowercase();
    let bang = lower
        .strip_suffix("important")
        .map(str::trim_end)
        .and_then(|rest| rest.strip_suffix('!'));
    match bang {
        Some(rest) => (v[..rest.len()].trim_end().to_string(), true),
        None => (v.to_string(), false),
    }
}

/// Whether a block's context applies in the dark scheme only (`true`) or in
/// both, with its selector's specificity (`:root`, a pseudo-class, over
/// `html`, a type), or why it is neither.
fn scope(context: &[String]) -> Result<(bool, u8), String> {
    let squash = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase()
    };
    // `html` is the root of an HTML page, as `:root` is.
    let root = |s: &str| match squash(s).as_str() {
        ":root" => Some(2),
        "html" => Some(1),
        _ => None,
    };
    let dark = |s: &str| {
        matches!(
            squash(s).as_str(),
            "@media (prefers-color-scheme: dark)" | "@media (prefers-color-scheme:dark)"
        )
    };
    match context {
        [r] => root(r).map(|s| (false, s)),
        [media, r] if dark(media) => root(r).map(|s| (true, s)),
        _ => None,
    }
    .ok_or_else(|| {
        let shown = context.join(" { ") + &" }".repeat(context.len().saturating_sub(1));
        let switched = context.iter().any(|c| c.contains('.') || c.contains('['));
        match switched {
            true => format!("`{shown}` is a scope the page switches: in Tamber that is an app axis, which the design declares (`axes`, docs/recipes/tamber.md)"),
            false => format!("`{shown}` is neither block the import reads: `:root` (or `html`), and `:root` inside `@media (prefers-color-scheme: dark)`"),
        }
    })
}

/// A custom property's value as a token's: on one line, and one whole
/// `var()` as the name it references; what was dropped from it, if anything.
fn value(raw: &str) -> Result<(String, Option<&'static str>), String> {
    let v = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if v.is_empty() {
        return Err("an empty value".into());
    }
    if !v.to_ascii_lowercase().contains("var(") {
        return Ok((v, None));
    }
    match contract_lower::var_tokens::whole_var(&v) {
        Some((name, fallback)) if exact_tamber::is_name(&name) => Ok((
            name,
            fallback.then_some("its fallback dropped: a token is never undefined"),
        )),
        Some((name, _)) => Err(format!(
            "`{v}` references `--{name}`, which names no Tamber token"
        )),
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
