//! `contract tamber import <kind> <source> [--app <dir>] [--prefix <p>]`
//! (LLP 1121 D14, stage 3): another system's theme as a proposal, printed as
//! JSON for the wire (`bun exact.mjs tamber import` holds it on the design
//! server): the transaction's operations in `tamber/1`'s shape, each role
//! override offered apart, the unmapped report and the count it was checked
//! against. Planned against the app's design as it is, so an unchanged
//! source proposes nothing.

use exact_tamber::import::brand::Brand;
use exact_tamber::import::{astryx, dtcg, shadcn, tailwind, Import};
use exact_tamber::Op;
use serde_json::{json, Value};
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str =
    "usage: contract tamber import shadcn|tailwind <stylesheet> | dtcg <file or directory> | pen <file.pen> | astryx <resolved theme json> [--app <app dir>] [--prefix <prefix>]";

/// A request the serve may plan: a held re-import's marks, and nothing of
/// a source (a source is read alone, `plan_alone`).
pub(crate) fn marks_only(request: &Value) -> bool {
    request["marks"].is_array()
        && ["source", "sources", "baseline"]
            .iter()
            .all(|k| request[*k].is_null())
}

/// One import planned in a process of its own (stage 10b; the lead's
/// 12:31Z ruling): a paired client's file is read here, never in the shared
/// `tamber serve`, so whatever it costs, the wire's watchdog can stop it.
/// Stdin is `{request, design}` (the design's text, null with none), the
/// argument the design file (its kept exports beside it); stdout `{ok}` or
/// `{error}`, as `import-plan` answers on the serve.
pub fn plan_alone(args: &[String]) -> ExitCode {
    use std::io::Read;
    let [design] = args else {
        eprintln!("usage: contract tamber import-plan <design file>");
        return ExitCode::from(2);
    };
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        return ExitCode::from(2);
    }
    let reply = match serde_json::from_str::<Value>(&input) {
        Ok(v) => {
            let kept = crate::tamber_export::kept(Path::new(design));
            match plan(
                &v["request"],
                v["design"].as_str(),
                &kept,
                Path::new(design).parent(),
            ) {
                Ok(ok) => json!({ "ok": ok }),
                Err(error) => json!({ "error": error }),
            }
        }
        Err(e) => json!({"error": {"message": format!("not JSON: {e}")}}),
    };
    println!("{reply}");
    ExitCode::SUCCESS
}

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
    let imported = match import_with(
        &kind,
        &files,
        prefix.as_deref(),
        &design,
        &current,
        &kept,
        None,
    ) {
        Some(i) => i,
        None => {
            eprintln!(
                "contract tamber import: no importer `{kind}` (shadcn, tailwind, dtcg, pen, astryx)"
            );
            return ExitCode::from(2);
        }
    };
    match imported {
        Ok(import) => {
            let pairs = brand_pairs(app.as_deref().map(Path::new));
            let brands = exact_tamber::import::brand::brands(
                &import,
                &design,
                &contract_lower::tamber_vocab(),
                &pairs,
            );
            println!("{}", import_json(&kind, &import, &brands));
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
    explicit: Option<&str>,
) -> Option<Result<Import, Vec<exact_tamber::Diagnostic>>> {
    let text = files.first().map(|(_, t)| t.as_str()).unwrap_or_default();
    let vocab = contract_lower::tamber_vocab();
    if kind == "dtcg" && files.len() == 1 && exact_tamber::export::is_own(text) {
        return Some(exact_tamber::export::reimport_against(
            text, explicit, kept, current, &vocab,
        ));
    }
    // A prefix starts every token name an importer makes: bounded as a name is.
    if prefix.is_some_and(|p| p.chars().count() > 128) {
        return Some(Err(vec![exact_tamber::Diagnostic {
            id: "tamber-import-source",
            message: "a prefix is at most 128 characters".into(),
            line: 1,
            col: 1,
        }]));
    }

    Some(match kind {
        "shadcn" => shadcn::import(text, prefix.unwrap_or("shadcn"), design, &vocab),
        "tailwind" => tailwind::import_with(text, prefix.unwrap_or("tw"), design, &vocab),
        "dtcg" => dtcg::import_with(files, prefix.unwrap_or("dtcg"), design, &vocab),
        "astryx" => astryx::import(text, prefix.unwrap_or("astryx"), design, &vocab),
        // Pencil's `.pen` (stage 13 B): no Tamber export to compare against, so
        // its re-import's dials and pins are marked against the design as it is.
        "pen" => {
            let name = files.first().map_or("source.pen", |(n, _)| n.as_str());
            exact_tamber::import::dtcg::pen::import_with(
                name,
                text,
                prefix.unwrap_or("pen"),
                design,
                &vocab,
            )
            .map(|mut import| {
                if let Some((dials, pins)) =
                    exact_tamber::import::marks_of(current, &import.ops, &vocab)
                {
                    (import.dials, import.pins) = (dials, pins);
                }
                import
            })
        }
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
pub fn plan(
    request: &Value,
    design: Option<&str>,
    kept: &[String],
    app_dir: Option<&Path>,
) -> Result<Value, Value> {
    // The design it planned on, by digest (none with no design file yet), so
    // the wire holds the proposal on that same text or plans again.
    let digest = design.map(|t| crate::tamber_serve::digest(t.as_bytes()));
    let current = design.unwrap_or("tamber app\n");
    let bad = |m: &str| json!({"message": m});
    let field = |k: &str| match &request[k] {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        _ => Err(bad(&format!("`{k}` is a string"))),
    };
    // A held re-import recomputed: its operations' marks on the design as it is.
    if !request["marks"].is_null() {
        if design.is_none() {
            return Err(bad("the design file is not there: nothing to mark"));
        }
        let ops: Vec<Op> = crate::tamber_ops::parse_ops(&request["marks"], "")?
            .into_iter()
            .map(|(_, op)| op)
            .collect();
        let (dials, pins) =
            exact_tamber::import::marks_of(current, &ops, &contract_lower::tamber_vocab())
                .ok_or_else(|| bad("the design does not check"))?;
        return Ok(
            json!({"dials": dials_json(&dials), "pins": pins_json(&pins), "digest": digest}),
        );
    }
    let kind = field("kind")?.ok_or_else(|| bad("`kind` is shadcn, tailwind, dtcg or pen"))?;
    // Astryx's theme resolved by a Bun install on this machine (`resolved`,
    // which only `bun exact.mjs tamber import` sets; the wire never forwards it).
    if kind == "astryx" && request["resolved"] != true {
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
    // A baseline the client carries (stage 10b, `import`'s `baseline`): an
    // export this server gave out (`recognised`, by digest), a kept one, or
    // the design's own; else refused, so a client pulls first. It decides
    // only which of the file's values are the client's edits: the operations
    // are planned against the design as it is.
    let own = kind == "dtcg" && files.len() == 1 && exact_tamber::export::is_own(&files[0].1);
    if !request["baseline"].is_null() && !own {
        return Err(bad(
            "a `baseline` is for re-importing a Tamber export, one file",
        ));
    }
    let explicit = match &request["baseline"] {
        Value::Null => None,
        Value::String(baseline) => {
            let digest = exact_tamber::export::digest(baseline)
                .ok_or_else(|| bad("`baseline` is a Tamber export, as `export` gave it"))?;
            let known = request["recognised"]
                .as_array()
                .is_some_and(|all| all.iter().any(|d| d.as_str() == Some(&digest)))
                || kept
                    .iter()
                    .any(|k| exact_tamber::export::digest(k).as_deref() == Some(&digest));
            // Else the design's own export, inside a re-import's budget: a
            // design too large for one is refused with its message.
            let known = known
                || exact_tamber::own_digest(current, &contract_lower::tamber_vocab())
                    .map_err(|all| json!({"message": "the source is refused", "diagnostics": all.iter().map(|d| json!({"id": d.id, "message": d.message})).collect::<Vec<_>>()}))?
                    .as_deref()
                    == Some(&digest);
            if !known {
                return Err(
                    json!({"message": "pull first: the baseline is no export of this design this server gave out, kept or has now", "stale": true}),
                );
            }
            Some(baseline.as_str())
        }
        _ => return Err(bad("`baseline` is the export text the client pulled")),
    };

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
        explicit,
    ) {
        None => Err(bad(&format!(
            "no importer `{kind}` (shadcn, tailwind, dtcg, pen; astryx through its CLI)"
        ))),
        // Each diagnostic in the shape the wire gives a checker's: named by
        // the source file when there is one, else by its message alone (an
        // importer of several names the file in it), no place invented.
        // A baseline whose contexts are no longer the design's: pull again.
        Some(Err(all)) if all.iter().any(|d| d.message.starts_with("pull first")) => Err(
            json!({"message": all.iter().find(|d| d.message.starts_with("pull first")).map(|d| &d.message), "stale": true}),
        ),
        Some(Err(all)) => Err(
            json!({"message": "the source is refused", "diagnostics": all.iter().map(|d| if files.len() == 1 { json!({"file": files[0].0, "line": d.line, "col": d.col, "id": d.id, "message": d.message}) } else { json!({"id": d.id, "message": d.message}) }).collect::<Vec<_>>()}),
        ),
        Some(Ok(import)) => {
            let brands = exact_tamber::import::brand::brands(
                &import,
                &design,
                &contract_lower::tamber_vocab(),
                &brand_pairs(app_dir),
            );
            let mut out = import_json(&kind, &import, &brands);
            out["digest"] = json!(digest);
            Ok(out)
        }
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

/// The packages' brand fills, each with its ink's token (theme2's brand-fill
/// ruling, 2026-10-29): a filled control the platform's accent doesn't
/// reach, so an import's accent is offered for it.
const BRAND_PAIRS: [(&str, &str); 2] = [
    ("actions.follow-fill", "actions.follow-ink"),
    ("cards.empty-button", "cards.empty-button-ink"),
];

/// The brand pairs the app's packages declare, fill and ink both; none
/// without an app.
fn brand_pairs(app_dir: Option<&Path>) -> Vec<(String, String)> {
    let Some(contract) = app_dir
        .map(|d| d.join("app.contract"))
        .filter(|c| c.is_file())
    else {
        return Vec::new();
    };
    let app = contract::tamber_params::params_of(&contract);
    BRAND_PAIRS
        .iter()
        .filter(|(fill, ink)| {
            crate::tamber_packages::defines_package_token(&app, fill)
                && crate::tamber_packages::defines_package_token(&app, ink)
        })
        .map(|(fill, ink)| (fill.to_string(), ink.to_string()))
        .collect()
}

fn import_json(kind: &str, import: &Import, brands: &[Brand]) -> Value {
    let mut out = json!({
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
        // An accent offered as the packages' brand fills (*1.18*): told apart
        // from a role override by `sets`, a package token, never a role.
        "brands": brands.iter().map(|b| json!({
            "sets": b.sets,
            "token": b.token,
            "ops": b.ops.iter().map(op_json).collect::<Vec<_>>(),
            "note": b.note,
            "contrast": {"light": b.contrast[0], "dark": b.contrast[1]},
        })).collect::<Vec<_>>(),
        // A re-import's dial changes and marked pins (LLP 1121 D14, stage 10b).
        "dials": dials_json(&import.dials),
        "pins": pins_json(&import.pins),
    });
    // What decided a choice the source didn't say (a Pencil number's kind; *1.17*), when there is one.
    if !import.notes.is_empty() {
        out["notes"] = json!(import.notes);
    }
    out
}

fn dials_json(dials: &[exact_tamber::import::DialChange]) -> Vec<Value> {
    dials.iter().map(|d| json!({
        "dial": d.dial,
        "value": d.value,
        "from": d.from,
        "impact": d.impact.iter().map(|i| json!({"token": i.token, "tuple": i.tuple, "before": i.before, "after": i.after})).collect::<Vec<_>>(),
    })).collect()
}

fn pins_json(pins: &[exact_tamber::import::Pin]) -> Vec<Value> {
    pins.iter().map(|p| json!({
        "token": p.token,
        "when": p.when.iter().map(|(a, v)| json!({"axis": a, "value": v})).collect::<Vec<_>>(),
        "follows": p.follows,
    })).collect()
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
        Op::SetParam {
            name, when, value, ..
        } => {
            json!({"op": "set-param", "name": name, "when": conds(when), "value": value})
        }
        Op::UnsetParam { name, when, .. } => {
            json!({"op": "unset-param", "name": name, "when": conds(when)})
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A re-import with a baseline the server does not recognise or keep
    /// checks it against the design's own export, built inside the
    /// re-import's budget: a design too large for one re-import (3,000
    /// tokens over 64 contexts) is refused with the budget's message, not
    /// exported whole first, nor told to pull (LLP 1121 §10).
    #[test]
    fn a_baseline_too_large_to_check_is_refused_by_the_budget() {
        let mut design = String::from("tamber t\n\naxes\n");
        for a in 0..5 {
            design += &format!("  a{a} = p | q\n");
        }
        design += "\ntheme\n";
        for i in 0..3000 {
            design += &format!(
                "  c{i} = light-dark(#{i:06x}, #{:06x})\n    when a{}=q: #{i:06x}\n",
                i + 1,
                i % 5
            );
        }
        let vocab = contract_lower::tamber_vocab();
        let small = exact_tamber::check("tamber s\n\ntheme\n  ink = #000000\n", &vocab).unwrap();
        let export = exact_tamber::export::dtcg(&small, &vocab).unwrap();
        let request =
            json!({"kind": "dtcg", "name": "s.json", "source": export, "baseline": export});
        let refused = plan(&request, Some(&design), &[], None).unwrap_err();
        assert_eq!(refused["message"], "the source is refused", "{refused}");
        assert!(refused.to_string().contains("expands past"), "{refused}");
    }
}
