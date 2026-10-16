//! A plan's timers on the JS target: each root task, in plan order, as
//! `every` or `frames` from mount — or, for a task with a gate or a key
//! (LLP 1092 D7–D10), as `gated`, its gate and key compiled to functions
//! the commit's gate step calls (schedule.js). Each carries the task's name
//! for the agent's `state.tasks`. Then each `sequence` (LLP 1125 D2), its
//! gate, key and `play` arguments compiled to functions transport.js's
//! gate step calls, after the `exactTransport` readers' fields.

use super::Em;
use crate::code::{self, Scope};
use std::fmt::Write as _;

pub(super) fn timers(em: &mut Em<'_>, body: &mut String) -> Result<(), String> {
    let plan = em.plan;
    let top = Scope::default();
    for t in plan.timers.iter() {
        let name = serde_json::to_string(plan.str(t.name)).unwrap();
        if t.gated || t.keyed {
            let mut f = |c| code::expression(plan, plan.code(c), &top, &mut em.uses);
            let gate = f(t.gate).map_err(|e| format!("task {name}: {e}"))?;
            let key = if t.keyed {
                format!("()=>{}", f(t.key).map_err(|e| format!("task {name}: {e}"))?)
            } else {
                "0".into()
            };
            let gated = em.uses.rt("gated");
            let _ = write!(
                body,
                "{gated}({},a_{},{},{},()=>{gate},{key},{name});",
                t.interval_ms, t.action.0, t.once as u8, t.frame as u8
            );
            continue;
        }
        if t.frame {
            // LLP 1073: once per presented frame, virtual frames on a seek.
            let frames = em.uses.rt("frames");
            let _ = write!(body, "{frames}(a_{},{name});", t.action.0);
            continue;
        }
        let every = em.uses.rt("every");
        let _ = write!(
            body,
            "{every}({},a_{},{},{name});",
            t.interval_ms, t.action.0, t.once as u8
        );
    }
    Ok(())
}

/// The `exactMIDI` readers, each its fields and a port list's port fields,
/// and whether the plan uses MIDI, for midi.js (LLP 1125 D6), ahead of the
/// sequences whose notes go on its queue; whether the module imports it.
pub(super) fn midi(em: &mut Em<'_>, body: &mut String) -> Result<bool, String> {
    use exact_runner::midi::{FIELDS, PORT_FIELDS, SOURCE};
    let plan = em.plan;
    if !exact_runner::uses::keeps_midi(plan) {
        return Ok(false);
    }
    let name = |s: &str| serde_json::to_string(s).unwrap();
    let mut readers = Vec::new();
    for r in plan
        .resources
        .iter()
        .filter(|r| plan.str(r.source) == SOURCE)
    {
        let resource = plan.str(r.name);
        let t = &plan.types[r.ty.0 as usize];
        if t.kind != exact_plan::TypeKind::Record {
            return Err(format!("resource {resource}: `{SOURCE}` answers a record"));
        }
        let (mut fields, mut ports) = (Vec::new(), Vec::new());
        for f in t.fields.iter() {
            let field = &plan.fields[f.0 as usize];
            let fname = plan.str(field.name);
            if !FIELDS.contains(&fname) {
                return Err(format!("resource {resource}: `{SOURCE}` has no `{fname}`"));
            }
            fields.push(name(fname));
            let list = &plan.types[field.ty.0 as usize];
            if let Some(item) = list.elem.map(|e| &plan.types[e.0 as usize]) {
                let keys: Vec<String> = item
                    .fields
                    .iter()
                    .map(|p| plan.str(plan.fields[p.0 as usize].name))
                    .filter(|p| PORT_FIELDS.contains(p))
                    .map(name)
                    .collect();
                ports.push(format!("{}:[{}]", name(fname), keys.join(",")));
            }
        }
        readers.push(format!(
            "{}:[[{}],{{{}}}]",
            name(resource),
            fields.join(","),
            ports.join(",")
        ));
    }
    let uses = exact_runner::uses::uses_midi(plan);
    let _ = write!(body, "$midi({{{}}},{});", readers.join(","), uses as u8);
    Ok(true)
}

/// The `exactTransport` readers and every sequence, for transport.js;
/// whether the module imports it.
pub(super) fn sequences(em: &mut Em<'_>, body: &mut String) -> Result<bool, String> {
    use exact_runner::transport::{FIELDS, SOURCE};
    let plan = em.plan;
    let mut readers = Vec::new();
    for r in plan
        .resources
        .iter()
        .filter(|r| plan.str(r.source) == SOURCE)
    {
        let resource = plan.str(r.name);
        let t = &plan.types[r.ty.0 as usize];
        if t.kind != exact_plan::TypeKind::Record {
            return Err(format!("resource {resource}: `{SOURCE}` answers a record"));
        }
        let mut names = Vec::new();
        for f in t.fields.iter() {
            let field = plan.str(plan.fields[f.0 as usize].name);
            if !FIELDS.contains(&field) {
                return Err(format!("resource {resource}: `{SOURCE}` has no `{field}`"));
            }
            names.push(serde_json::to_string(field).unwrap());
        }
        readers.push(format!(
            "{}:[{}]",
            serde_json::to_string(resource).unwrap(),
            names.join(",")
        ));
    }
    if plan.sequences.is_empty() && readers.is_empty() {
        return Ok(false);
    }
    let _ = write!(body, "$transport({{{}}});", readers.join(","));
    let top = Scope::default();
    // One call for them all: transport.js reads every gate before it arms any.
    let mut entries = Vec::new();
    for q in plan.sequences.iter() {
        let name = serde_json::to_string(plan.str(q.name)).unwrap();
        let mut f = |c| {
            code::expression(plan, plan.code(c), &top, &mut em.uses)
                .map_err(|e| format!("sequence {name}: {e}"))
        };
        let gate = f(q.gate)?;
        let key = if q.keyed {
            format!("()=>{}", f(q.key)?)
        } else {
            "0".into()
        };
        let args = [q.tracks, q.bpm, q.from, q.count_in, q.clock, q.clock_out]
            .into_iter()
            .map(&mut f)
            .collect::<Result<Vec<_>, _>>()?;
        let ended = q.ended.map_or("null".into(), |a| format!("a_{}", a.0));
        entries.push(format!(
            "[()=>{gate},{key},()=>[{}],{ended},{name}]",
            args.join(",")
        ));
    }
    if !entries.is_empty() {
        let _ = write!(body, "$sequence([{}]);", entries.join(","));
    }
    Ok(true)
}
