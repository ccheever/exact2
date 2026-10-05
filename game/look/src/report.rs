//! The checker's view of a look's cost: what each rule reads, whether it can
//! be kept, and for each row that follows the time, the residual a GPU could
//! evaluate per frame (`p0 * sin(t * p1 + p2)`) over parameters that change
//! only when their inputs do.
use crate::check::{Ex, Program, Row, Slot, St, B};
use crate::syntax::Op;
use std::fmt::Write;

pub(crate) fn report(prog: &Program) -> String {
    let mut out = String::new();
    for (st, rule) in &prog.body {
        let Some(rule) = rule else { continue };
        let r = &prog.rules[*rule as usize];
        let names = |set: &std::collections::BTreeSet<u16>, of: &[&str]| -> String {
            set.iter()
                .map(|&i| of[i as usize])
                .collect::<Vec<_>>()
                .join(" ")
        };
        let globals: Vec<&str> = r
            .reads
            .globals
            .iter()
            .map(|&g| prog.global_names[g as usize].as_str())
            .collect();
        let _ = writeln!(
            out,
            "rule {} at {}:{} — reads [{}]{}{} globals [{}]{}{} → {}",
            r.what,
            r.at.line,
            r.at.col,
            names(&r.reads.components, &prog.components),
            if r.reads.resources.is_empty() {
                String::new()
            } else {
                format!(
                    " resources [{}]",
                    names(&r.reads.resources, &prog.resources)
                )
            },
            if r.reads.elsewhere {
                " (other entities' rows)"
            } else {
                " (own rows only)"
            },
            globals.join(" "),
            if r.reads.time { " time" } else { "" },
            if r.reads.volatile { " names/poses" } else { "" },
            if r.cacheable() {
                "kept while its reads are unchanged"
            } else {
                "derived every present"
            },
        );
        rows(prog, st, &mut out);
    }
    out
}

fn rows(prog: &Program, st: &St, out: &mut String) {
    match st {
        St::If(_, a, b) => a.iter().chain(b).for_each(|s| rows(prog, s, out)),
        St::Each { body, .. } | St::For { body, .. } | St::Range { body, .. } => {
            body.iter().for_each(|s| rows(prog, s, out))
        }
        St::Emit(row, _, args, at) => {
            let fields: Vec<&str> = match row {
                Row::Offset {
                    position,
                    rotation,
                    scale,
                } => [
                    ("position", *position),
                    ("rotation", *rotation),
                    ("scale", *scale),
                ]
                .iter()
                .filter(|(_, on)| *on)
                .map(|(n, _)| *n)
                .collect(),
                Row::Opacity => vec!["value"],
            };
            let kind = match row {
                Row::Offset { .. } => "Offset",
                Row::Opacity => "Opacity",
            };
            for (field, ex) in fields.iter().zip(args) {
                if !timed(prog, ex) {
                    let _ = writeln!(out, "  {kind}.{field} at {}:{} — follows the simulation only: CPU, when its inputs change", at.line, at.col);
                    continue;
                }
                let mut params = 0;
                let residual = print(prog, ex, &mut params);
                let gpu = gpu_ok(prog, ex);
                let _ = writeln!(
                    out,
                    "  {kind}.{field} at {}:{} — follows the time: {} `{residual}` over {params} per-instance parameter(s)",
                    at.line,
                    at.col,
                    if gpu { "GPU-lowerable" } else { "CPU only" },
                );
            }
        }
        _ => {}
    }
}

fn timed(prog: &Program, ex: &Ex) -> bool {
    match ex {
        Ex::Lit(_) | Ex::Named(_) | Ex::Res(_) => false,
        Ex::Get(Slot::Global(g)) => prog.global_reads[*g as usize].time,
        Ex::Get(Slot::Local(l)) => prog.local_time.get(*l as usize).copied().unwrap_or(false),
        Ex::Builtin(B::Seconds | B::Tick, _) => true,
        Ex::Call(f, args) => {
            prog.fns[*f as usize].reads.time || args.iter().any(|a| timed(prog, a))
        }
        Ex::Field(a, _)
        | Ex::Axis(a, _)
        | Ex::Comp(a, _)
        | Ex::CompPath(a, _, _)
        | Ex::Un(_, a)
        | Ex::Is(a, _)
        | Ex::Has(a, _) => timed(prog, a),
        Ex::Index(a, b) | Ex::Bin(_, a, b) => timed(prog, a) || timed(prog, b),
        Ex::Cond(a, b, c) => timed(prog, a) || timed(prog, b) || timed(prog, c),
        Ex::Builtin(_, args) | Ex::Extern(_, args) | Ex::Rec(args) => {
            args.iter().any(|a| timed(prog, a))
        }
        Ex::MatchEnt(s, arms) => timed(prog, s) || arms.iter().any(|(_, b)| timed(prog, b)),
        Ex::MatchArm(s, arms) => timed(prog, s) || arms.iter().any(|(_, b)| timed(prog, b)),
    }
}

// The operations a vertex shader evaluates per instance: arithmetic, the
// trigonometry the engine's WGSL has, and building a rotation from an angle.
fn gpu_ok(prog: &Program, ex: &Ex) -> bool {
    if !timed(prog, ex) {
        return true;
    }
    match ex {
        Ex::Get(_) => true,
        Ex::Builtin(B::Seconds | B::Tick, _) => true,
        Ex::Builtin(
            B::Sin
            | B::Cos
            | B::Min
            | B::Max
            | B::Abs
            | B::Floor
            | B::Yaw
            | B::Pitch
            | B::Roll
            | B::Vec,
            args,
        ) => args.iter().all(|a| gpu_ok(prog, a)),
        Ex::Bin(Op::Add | Op::Sub | Op::Mul | Op::Div, a, b) => gpu_ok(prog, a) && gpu_ok(prog, b),
        Ex::Un(Op::Neg, a) => gpu_ok(prog, a),
        _ => false,
    }
}

fn print(prog: &Program, ex: &Ex, params: &mut usize) -> String {
    if !timed(prog, ex) {
        if let Ex::Lit(crate::value::Val::Num(n)) = ex {
            return format!("{n}");
        }
        *params += 1;
        return format!("p{}", *params - 1);
    }
    match ex {
        Ex::Get(Slot::Global(g)) => prog.global_names[*g as usize].clone(),
        Ex::Get(Slot::Local(l)) => format!("local{l}"),
        Ex::Builtin(b, args) => format!(
            "{}({})",
            format!("{b:?}").to_lowercase(),
            args.iter()
                .map(|a| print(prog, a, params))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Ex::Bin(op, a, b) => {
            let sym = match op {
                Op::Add => "+",
                Op::Sub => "-",
                Op::Mul => "*",
                Op::Div => "/",
                _ => "?",
            };
            format!(
                "{} {sym} {}",
                print(prog, a, params),
                print(prog, b, params)
            )
        }
        Ex::Un(_, a) => format!("-{}", print(prog, a, params)),
        _ => "…".into(),
    }
}
