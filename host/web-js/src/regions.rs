//! Regions (`when`, `match`, `each`) and the slots a component instance
//! owns: an arm's or a row's, declared where it is built, and a late root
//! slot's (LLP 1017 P4c).

use super::{type_code, type_json, Em};
use crate::code::{self, Frame, Scope};
use exact_plan::{Plan, RegionKind};
use std::fmt::Write as _;

impl Em<'_> {
    pub(super) fn region(&mut self, r: u32, parent: &str, scope: &Scope) -> Result<(), String> {
        let plan = self.plan;
        let row = &plan.regions[r as usize];
        let subject = self
            .f(row.subject, scope)
            .map_err(|x| format!("region {r}: {x}"))?;
        let arms: Vec<u32> = row.arms.iter().map(|a| a.0).collect();
        match row.kind {
            RegionKind::When | RegionKind::Match => {
                let bound = (row.kind == RegionKind::Match).then(|| format!("b{r}"));
                let mut inner = scope.clone();
                inner.frames.push(Frame {
                    bound: bound.clone(),
                    ..Frame::default()
                });
                let mut bodies = Vec::new();
                for (k, arm) in arms.iter().enumerate() {
                    let saved = std::mem::take(&mut self.out);
                    // `match`'s none arm holds no binding.
                    let mut sc = if k == 0 {
                        inner.clone()
                    } else {
                        let mut s = scope.clone();
                        s.frames.push(Frame::default());
                        s
                    };
                    // The arm's own slots, started from their initializers
                    // each time it shows, dropped when it hides (LLP 1017 P4c).
                    let decl = self.owned(*arm, &format!("$a{arm}"), &mut sc)?;
                    self.out.push_str(&decl);
                    self.children(self.sites.of_arm(*arm), "p", &sc)?;
                    let built = std::mem::replace(&mut self.out, saved);
                    let params = match (&bound, k) {
                        (Some(b), 0) => format!("(p,{b})"),
                        _ => "p".into(),
                    };
                    bodies.push(format!("{params}=>{{{built}}}"));
                }
                while bodies.len() < 2 {
                    bodies.push("0".into());
                }
                let f = self.uses.rt(if row.kind == RegionKind::When {
                    "when"
                } else {
                    "match"
                });
                let _ = write!(
                    self.out,
                    "{f}({parent},{subject},{},{});",
                    bodies[0], bodies[1]
                );
            }
            RegionKind::Each => self.each(r, parent, scope, None)?,
        }
        Ok(())
    }

    /// The declaration of arm `arm`'s own slots, as `name` (the enclosing
    /// instances' slots spread in), each a signal started from its
    /// initializer in `scope`, which then reads them through `name`; empty
    /// when it owns none.
    fn owned(&mut self, arm: u32, name: &str, scope: &mut Scope) -> Result<String, String> {
        let plan = self.plan;
        let mut own = Vec::new();
        for (k, slot) in plan.slots.iter().enumerate() {
            if slot.owner.map(|o| o.0) == Some(arm) {
                let init = code::expression(plan, plan.code(slot.init), scope, &mut self.uses)
                    .map_err(|x| format!("child state {}: {x}", plan.str(slot.name)))?;
                // Typed as a root slot is: its initializer and every write
                // conform, or the runner refuses (SlotType).
                let sig = self.uses.rt("sig");
                let ty = serde_json::to_string(&type_code(plan, slot.ty)).unwrap();
                let name = string_name(plan, slot);
                own.push(format!("{k}:{sig}({init},{ty}{name})"));
            }
        }
        if own.is_empty() {
            return Ok(String::new());
        }
        let decl = match &scope.rows {
            Some(outer) => format!("const {name}={{...{outer},{}}};", own.join(",")),
            None => format!("const {name}={{{}}};", own.join(",")),
        };
        scope.rows = Some(name.to_string());
        Ok(decl)
    }

    /// An `each`, or a virtualized list's rows when `list` carries the
    /// list's options (list.js `vl`).
    pub(super) fn each(
        &mut self,
        r: u32,
        parent: &str,
        scope: &Scope,
        list: Option<String>,
    ) -> Result<(), String> {
        let plan = self.plan;
        let row = &plan.regions[r as usize];
        let subject = self
            .f(row.subject, scope)
            .map_err(|x| format!("region {r}: {x}"))?;
        let arms: Vec<u32> = row.arms.iter().map(|a| a.0).collect();
        let (item, index) = (format!("i{r}"), format!("x{r}"));
        let mut inner = scope.clone();
        inner.frames.push(Frame {
            item: Some(item.clone()),
            index: Some(index.clone()),
            bound: None,
        });
        let key = code::expression(plan, plan.code(row.key), &inner, &mut self.uses)
            .map_err(|x| format!("region {r} key: {x}"))?;
        // The row's own slots, started from their initializers when
        // the row is created and kept with its key (LLP 1017 P4c).
        let rows_decl = self.owned(arms[0], &format!("$r{r}"), &mut inner)?;
        let saved = std::mem::take(&mut self.out);
        self.out.push_str(&rows_decl);
        self.children(self.sites.of_arm(arms[0]), "p", &inner)?;
        let built = std::mem::replace(&mut self.out, saved);
        match list {
            Some(opts) => {
                let _ = write!(
                    self.out,
                    "$vl({parent},{subject},({item},{index})=>{key},(p,{item},{index})=>{{{built}}},{opts});"
                );
            }
            None => {
                let each = self.uses.rt("each");
                let pure = if crate::reads::pure_key(&key, &item, &index) {
                    ",1"
                } else {
                    ""
                };
                let _ = write!(
                    self.out,
                    "{each}({parent},{subject},({item},{index})=>{key},(p,{item},{index})=>{{{built}}}{pure});"
                );
            }
        }
        Ok(())
    }
}

/// A root slot's signal, `s_<i>`, from its initializer.
pub(super) fn root_slot(
    plan: &Plan,
    i: usize,
    top: &Scope,
    dev_reload: bool,
    uses: &mut code::Uses,
    body: &mut String,
) -> Result<(), String> {
    let r = &plan.slots[i];
    let init = code::expression(plan, plan.code(r.init), top, uses)
        .map_err(|e| format!("slot {}: {e}", plan.str(r.name)))?;
    let ty = serde_json::to_string(&type_code(plan, r.ty)).unwrap();
    if dev_reload {
        let _ = write!(
            body,
            "const s_{i}=$devSig({},{init},{ty},{});",
            serde_json::to_string(plan.str(r.name)).unwrap(),
            type_json(plan, r.ty)
        );
    } else {
        let sig = uses.rt("sig");
        let name = string_name(plan, r);
        let _ = write!(body, "const s_{i}={sig}({init},{ty}{name});");
    }
    Ok(())
}

/// A string slot's name, as `sig`'s third argument: a write past MAX_STRING
/// is refused naming it (the runner's StringTooLong, LLP 1090 D6).
fn string_name(plan: &Plan, slot: &exact_plan::SlotsRow) -> String {
    if type_code(plan, slot.ty) == "s" {
        format!(",{}", serde_json::to_string(plan.str(slot.name)).unwrap())
    } else {
        String::new()
    }
}
