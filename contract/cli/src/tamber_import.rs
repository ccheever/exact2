//! `contract tamber import <kind> <source> [--app <dir>] [--prefix <p>]`
//! (LLP 1121 D14, stage 3): another system's theme as a proposal, printed as
//! JSON for the wire (`bun exact.mjs tamber import` holds it on the design
//! server): the transaction's operations in `tamber/1`'s shape, each role
//! override offered apart, the unmapped report and the count it was checked
//! against. Planned against the app's design as it is, so an unchanged
//! source proposes nothing.

use exact_tamber::import::{shadcn, Import};
use exact_tamber::Op;
use serde_json::{json, Value};
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str =
    "usage: contract tamber import shadcn <globals.css> [--app <app dir>] [--prefix <prefix>]";

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
    let text = match std::fs::read_to_string(&source) {
        Ok(t) => t,
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
    let imported = match kind.as_str() {
        "shadcn" => shadcn::import(
            &text,
            prefix.as_deref().unwrap_or("shadcn"),
            &design,
            &contract_lower::tamber_vocab(),
        ),
        other => {
            eprintln!("contract tamber import: no importer `{other}` (shadcn)");
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
