//! A plan's timers on the JS target: each root task, in plan order, as
//! `every` or `frames` from mount — or, for a task with a gate or a key
//! (LLP 1092 D7–D10), as `gated`, its gate and key compiled to functions
//! the commit's gate step calls (schedule.js). Each carries the task's name
//! for the agent's `state.tasks`.

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
