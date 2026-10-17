//! `contract tamber import <kind> <source> [--app <dir>] [--prefix <p>]`
//! (LLP 1121 D14, stage 3): another system's theme as a proposal, printed as
//! JSON for the wire (`bun exact.mjs tamber import` holds it on the design
//! server): the transaction's operations in `tamber/1`'s shape, each role
//! override offered apart, the unmapped report and the count it was checked
//! against. Planned against the app's design as it is, so an unchanged
//! source proposes nothing.

use exact_tamber::import::{astryx, dtcg, shadcn, tailwind, Import};
use exact_tamber::Op;
use serde_json::{json, Value};
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str =
    "usage: contract tamber import shadcn|tailwind <stylesheet> | dtcg <file or directory> | astryx <resolved theme json> [--app <app dir>] [--prefix <prefix>]";

pub fn run(args: &[String]) -> ExitCode {
    let (mut kind, mut source, mut app, mut prefix) = (None, None, None, None);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--app" if i + 1 < args.len() => {
                app = Some(args[i + 1].clone());
                i += 2;
            }
            "--prefix" if i + 1 < args.len() => {
                prefix = Some(args[i + 1].clone());
                i += 2;
            }
            word if !word.starts_with('-') && kind.is_none() => {
                kind = Some(word.to_string());
                i += 1;
            }
            word if !word.starts_with('-') && source.is_none() => {
                source = Some(word.to_string());
                i += 1;
            }
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let (Some(kind), Some(source)) = (kind, source) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    // The source's files: one, or every `.json` under a directory (DTCG's
    // sets and modes), by their paths from it.
    let files = match read_source(Path::new(&source)) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("contract tamber import: {source}: {e}");
            return ExitCode::from(2);
        }
    };
    // The design it is planned against: the app's, or none yet.
    let current = match app
        .as_deref()
        .and_then(|a| crate::tamber::design(Path::new(a)))
    {
        Some(path) => match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("contract tamber import: {}: {e}", path.display());
                return ExitCode::from(2);
            }
        },
        None => "tamber app\n".to_string(),
    };
    let design = match exact_tamber::parse(&current) {
        Ok(d) => d,
        Err(all) => {
            for d in &all {
                eprintln!("contract tamber import: the design: {d}");
            }
            return ExitCode::from(1);
        }
    };
    // A Tamber export's baselines: kept by each export beside the design.
    let kept = app
        .as_deref()
        .and_then(|a| crate::tamber::design(Path::new(a)))
        .map(|p| crate::tamber_export::kept(&p))
        .unwrap_or_default();
    let imported = match import_with(&kind, &files, prefix.as_deref(), &design, &current, &kept) {
        Some(i) => i,
        None => {
            eprintln!(
                "contract tamber import: no importer `{kind}` (shadcn, tailwind, dtcg, astryx)"
            );
            return ExitCode::from(2);
        }
    };
    match imported {
        Ok(import) => {
            println!("{}", import_json(&kind, &import));
            ExitCode::SUCCESS
        }
        Err(all) => {
            for d in &all {
                eprintln!("contract tamber import: {source}: {d}");
            }
            ExitCode::from(1)
        }
    }
}

/// The importer `kind` over the source's files, planned against `design`
/// (`current`'s text); `None` when there is no such importer. A Tamber DTCG
/// export of its own is compared with the baseline it came from, one of
/// `kept` or a fresh export of the design as it is (LLP 1121 D14).
fn import_with(
    kind: &str,
    files: &[(String, String)],
    prefix: Option<&str>,
    design: &exact_tamber::Design,
    current: &str,
    kept: &[String],
) -> Option<Result<Import, Vec<exact_tamber::Diagnostic>>> {
    let text = files.first().map(|(_, t)| t.as_str()).unwrap_or_default();
    let vocab = contract_lower::tamber_vocab();
    if kind == "dtcg" && files.len() == 1 && exact_tamber::export::is_own(text) {
        return Some(exact_tamber::export::reimport(text, kept, current, &vocab));
    }
    Some(match kind {
        "shadcn" => shadcn::import(text, prefix.unwrap_or("shadcn"), design, &vocab),
        "tailwind" => tailwind::import_with(text, prefix.unwrap_or("tw"), design, &vocab),
        "dtcg" => dtcg::import_with(files, prefix.unwrap_or("dtcg"), design, &vocab),
        "astryx" => astryx::import(text, prefix.unwrap_or("astryx"), design, &vocab),
        _ => return None,
    })
}

/// The design server's `import-plan` `{kind, source, name?, prefix?}` or
/// `{kind, sources: [{name, text}], prefix?}` (LLP
/// 1122 T1a), for the wire's `import`: the source's text, as a person picked
/// it in the tool, planned against `current`, the app's design as it is.
/// `name` is the file's (a Figma export's `<Collection>.<Mode>.json` names
/// its modes). Astryx is not taken here: resolving its package needs a Bun
/// install, which its CLI does.
pub fn plan(request: &Value, current: &str, kept: &[String]) -> Result<Value, Value> {
    let bad = |m: &str| json!({"message": m});
    let field = |k: &str| match &request[k] {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        _ => Err(bad(&format!("`{k}` is a string"))),
    };
    let kind = field("kind")?.ok_or_else(|| bad("`kind` is shadcn, tailwind or dtcg"))?;
    if kind == "astryx" {
        return Err(bad("Astryx resolves its theme package with a Bun install: run `bun exact.mjs tamber import astryx <source> <url> --app <app>`"));
    }
    // One file's text (`source`, named by `name`), or several (`sources`, each
    // `{name, text}`): a DTCG export of several modes, a set and its themes.
    let files: Vec<(String, String)> = match &request["sources"] {
        Value::Null => vec![(
            field("name")?.unwrap_or_else(|| "source".into()),
            field("source")?
                .ok_or_else(|| bad("`source` is the file's text, or `sources` the files'"))?,
        )],
        Value::Array(all) if !all.is_empty() && all.len() <= 256 => all
            .iter()
            .map(|f| match (f["name"].as_str(), f["text"].as_str()) {
                (Some(n), Some(t)) => Ok((n.to_string(), t.to_string())),
                _ => Err(bad("each of `sources` is {name, text}")),
            })
            .collect::<Result<_, _>>()?,
        _ => return Err(bad("`sources` is a list of 1 to 256 {name, text}")),
    };
    // By name, as the CLI reads a directory, so the order a person picked
    // the files in decides nothing.
    let mut files = files;
    files.sort();
    // A stylesheet's importer reads one sheet; only DTCG takes several.
    if files.len() > 1 && kind != "dtcg" {
        return Err(bad(&format!(
            "`{kind}` imports one stylesheet, not {}: several files are DTCG's",
            files.len()
        )));
    }
    let design = exact_tamber::parse(current).map_err(|all| {
        json!({"message": "the design does not parse", "diagnostics": all.iter().map(ToString::to_string).collect::<Vec<_>>()})
    })?;
    match import_with(
        &kind,
        &files,
        field("prefix")?.as_deref(),
        &design,
        current,
        kept,
    ) {
        None => Err(bad(&format!(
            "no importer `{kind}` (shadcn, tailwind, dtcg; astryx through its CLI)"
        ))),
        // Each diagnostic in the shape the wire gives a checker's: named by
        // the source file when there is one, else by its message alone (an
        // importer of several names the file in it), no place invented.
        Some(Err(all)) => Err(
            json!({"message": "the source is refused", "diagnostics": all.iter().map(|d| if files.len() == 1 { json!({"file": files[0].0, "line": d.line, "col": d.col, "id": d.id, "message": d.message}) } else { json!({"id": d.id, "message": d.message}) }).collect::<Vec<_>>()}),
        ),
        Some(Ok(import)) => Ok(import_json(&kind, &import)),
    }
}

/// A source path's files: the file, or each `.json` under the directory
/// (any case; links to directories not followed; at most 10,000), by their
/// paths from it.
fn read_source(path: &Path) -> std::io::Result<Vec<(String, String)>> {
    if !path.is_dir() {
        let name = path
            .file_name()
            .map_or(String::new(), |n| n.to_string_lossy().into_owned());
        return Ok(vec![(name, std::fs::read_to_string(path)?)]);
    }
    let mut out = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let p = entry.path();
            let kind = entry.file_type()?;
            // A link to a file is read; a link to a directory is not followed.
            let file = kind.is_file()
                || (kind.is_symlink() && std::fs::metadata(&p).is_ok_and(|m| m.is_file()));
            if kind.is_dir() {
                stack.push(p);
            } else if file
                && p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("json"))
            {
                if out.len() >= 10_000 {
                    return Err(std::io::Error::other("more than 10,000 JSON files"));
                }
                let name = p
                    .strip_prefix(path)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((name, std::fs::read_to_string(&p)?));
            }
        }
    }
    out.sort();
    Ok(out)
}

fn import_json(kind: &str, import: &Import) -> Value {
    json!({
        "importer": kind,
        "ops": import.ops.iter().map(op_json).collect::<Vec<_>>(),
        "offers": import.offers.iter().map(|o| json!({
            "role": o.role,
            "token": o.token,
            "ops": o.ops.iter().map(op_json).collect::<Vec<_>>(),
            "note": o.note,
        })).collect::<Vec<_>>(),
        "unmapped": import.unmapped.iter().map(|u| json!({
            "block": u.block,
            "name": u.name,
            "value": u.value,
            "why": u.why,
            "token": u.token,
        })).collect::<Vec<_>>(),
        "counted": {
            "source": import.counted.source,
            "mapped": import.counted.mapped,
            "reported": import.counted.reported,
        },
    })
}

/// An operation as `tamber/1` writes it: kebab-case names, named fields.
pub fn op_json(op: &Op) -> Value {
    let conds = |c: &[(String, String)]| {
        c.iter()
            .map(|(a, v)| json!({"axis": a, "value": v}))
            .collect::<Vec<_>>()
    };
    match op {
        Op::SetDial { name, value } => json!({"op": "set-dial", "name": name, "value": value}),
        Op::SetToken {
            name,
            when,
            value,
            comment,
            ..
        } => {
            json!({"op": "set-token", "name": name, "when": conds(when), "value": value, "comment": comment})
        }
        Op::UnsetTokenWhen { name, when } => {
            json!({"op": "unset-token-when", "name": name, "when": conds(when)})
        }
        Op::AddToken {
            name,
            kind,
            value,
            meta,
            whens,
            comment,
            before,
        } => json!({
            "op": "add-token", "name": name, "kind": kind, "value": value, "meta": meta,
            "whens": whens.iter().map(|(c, v, k)| json!({"when": conds(c), "value": v, "comment": k})).collect::<Vec<_>>(),
            "comment": comment, "before": before,
        }),
        Op::RenameToken { from, to } => json!({"op": "rename-token", "from": from, "to": to}),
        Op::RemoveToken { name } => json!({"op": "remove-token", "name": name}),
        Op::AddAxisValue {
            axis,
            value,
            before,
        } => json!({"op": "add-axis-value", "axis": axis, "value": value, "before": before}),
        Op::RemoveAxisValue { axis, value } => {
            json!({"op": "remove-axis-value", "axis": axis, "value": value})
        }
        Op::AddAxis {
            name,
            values,
            under_dark,
        } => {
            json!({"op": "add-axis", "name": name, "values": values, "under": under_dark.then_some("scheme=dark")})
        }
        // No client's to send; an import never makes one.
        Op::Restore { .. } => json!({"op": "restore"}),
    }
}
