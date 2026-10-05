//! A game's rules as a Contract program run inside its tick (LLP 1046.009
//! §3.3, experiment 4).
//!
//! The program is an ordinary `.contract` file: `state` for what the rules
//! remember, `derive` for what they say, `action` for what happens. Its view
//! is the event table — one `press=` handler per event the world may raise —
//! and [`Program::act`] runs only those, so the Lean semantics' `Reachable`
//! (every sequence of handler events) covers every run the world can make.
//!
//! The slots live in the world's [`Rules`] resource, by name: they are saved,
//! hashed and carried as any other state, and a new program adopts them where
//! the name and kind still fit ([`Program::adopt`]). Derives are recomputed,
//! never saved. Nothing here reads a host: no resources, mutations, timers,
//! router or rows; [`Program::new`] refuses a plan that has them.
//!
//! The executor is the runner's own expression VM ([`exact_runner::vm::eval`],
//! the machine `semantics/vm-extract` proves refines `Contract/Vm.lean`),
//! without the runner around it: no kernel, no view, no layout.
use exact_game::{Resource, Value};
use exact_plan::{ActionsRow, Plan};
use exact_runner::vm;
use std::collections::BTreeMap;

/// The rules' state, by slot name. Saved and hashed with the world.
#[derive(Clone, Debug, Default, Resource)]
pub struct Rules {
    pub slots: BTreeMap<String, Value>,
    /// The program's derives as of the last change, by name. Never saved: a
    /// restored world recomputes them ([`Program::settle`]).
    #[data(skip)]
    pub derived: BTreeMap<String, Value>,
}

impl Rules {
    pub fn get(&self, name: &str) -> &Value {
        self.slots
            .get(name)
            .or_else(|| self.derived.get(name))
            .unwrap_or_else(|| panic!("rules: no slot or derive `{name}`"))
    }
    pub fn num(&self, name: &str) -> f64 {
        match self.get(name) {
            Value::Number(n) => *n,
            v => panic!("rules: `{name}` is {v:?}, not a number"),
        }
    }
    pub fn flag(&self, name: &str) -> bool {
        match self.get(name) {
            Value::Bool(b) => *b,
            v => panic!("rules: `{name}` is {v:?}, not a bool"),
        }
    }
    pub fn text(&self, name: &str) -> &str {
        self.get(name)
            .as_str()
            .unwrap_or_else(|| panic!("rules: `{name}` is not a string"))
    }
    /// A whole number slot, as the HUD and the kernels count.
    pub fn count(&self, name: &str) -> u32 {
        self.num(name) as u32
    }
}

/// A rules program: a decoded plan and its event table.
pub struct Program {
    plan: Plan,
    strings: Vec<Value>,
    /// Zero-argument handler actions: the events the world may raise.
    events: Vec<String>,
    /// The digest of the plan's bytes, naming the program in logs.
    pub digest: u64,
}

/// What a carried state kept and lost when a new program adopted it.
#[derive(Debug, Default, PartialEq)]
pub struct Adopted {
    /// Slots the new program does not declare.
    pub dropped: Vec<String>,
    /// Slots kept by name whose kind changed, so started over.
    pub reset: Vec<String>,
    /// Slots the old state did not have, starting from their initializers.
    pub added: Vec<String>,
}

fn kind(v: &Value) -> u8 {
    match v {
        Value::Number(_) => 1,
        Value::Bool(_) => 2,
        v if v.is_str() => 3,
        _ => 0,
    }
}

/// FNV-1a: a stable name for a program's bytes.
fn digest(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

impl Program {
    /// Decode a compiled rules program and check it reads nothing a world
    /// cannot give it.
    pub fn new(bytes: &[u8]) -> Result<Program, String> {
        let plan = Plan::decode(bytes).map_err(|e| format!("rules plan: {e:?}"))?;
        let refuse = |what: &str| Err(format!("rules plan: a rules program has no {what}"));
        if !plan.resources.is_empty() {
            return refuse("resources");
        }
        if !plan.mutations.is_empty() {
            return refuse("mutations");
        }
        if !plan.timers.is_empty() {
            return refuse("tasks");
        }
        if plan.router.is_some() || !plan.routes.is_empty() {
            return refuse("routes");
        }
        if plan.slots.iter().any(|s| s.owner.is_some()) || !plan.regions.is_empty() {
            return refuse("rows or arms (each, match)");
        }
        let mut events = Vec::new();
        for h in &plan.handlers {
            let action = plan.action(h.action);
            if h.args.is_empty() && action.params.is_empty() {
                events.push(plan.str(action.name).to_string());
            }
        }
        Ok(Program {
            strings: vm::intern(&plan),
            events,
            plan,
            digest: digest(bytes),
        })
    }

    /// The events the world may raise, in the view's order.
    pub fn events(&self) -> &[String] {
        &self.events
    }

    /// The rules before anything has happened.
    pub fn boot(&self) -> Result<Rules, String> {
        let mut values: Vec<Value> = Vec::with_capacity(self.plan.slots.len());
        for (i, row) in self.plan.slots.iter().enumerate() {
            let mut padded = values.clone();
            padded.resize(self.plan.slots.len(), Value::Unit);
            let out = self
                .eval(self.plan.code(row.init), &padded, &[], &[], &[])
                .map_err(|e| format!("rules: slot {}'s initializer: {e}", self.slot_name(i)))?;
            values.push(out.value);
        }
        let mut rules = Rules {
            slots: (0..values.len())
                .map(|i| (self.slot_name(i).to_string(), values[i].clone()))
                .collect(),
            derived: BTreeMap::new(),
        };
        self.settle(&mut rules)?;
        Ok(rules)
    }

    /// Raise one event. A refused event — one the view does not declare, or a
    /// body the VM refuses — changes nothing.
    pub fn act(&self, rules: &mut Rules, event: &str) -> Result<(), String> {
        if !self.events.iter().any(|e| e == event) {
            return Err(format!(
                "rules: `{event}` is not an event this program declares"
            ));
        }
        let action = (self.plan.actions.iter())
            .find(|a| self.plan.str(a.name) == event)
            .ok_or_else(|| format!("rules: no action `{event}`"))?;
        let slots = self.slot_values(rules)?;
        let derives = self.derive_values(&slots)?;
        let allowed: Vec<u32> = self.writes(action);
        let out = self
            .eval(self.plan.code(action.body), &slots, &derives, &[], &allowed)
            .map_err(|e| format!("rules: `{event}` refused: {e}"))?;
        for (slot, v) in out.writes {
            rules
                .slots
                .insert(self.slot_name(slot as usize).to_string(), v);
        }
        self.settle(rules)
    }

    /// `rules` with its derives: as they are, or settled now for a restored
    /// state that has not been settled yet.
    pub fn derived<'a>(&self, rules: &'a Rules) -> Result<std::borrow::Cow<'a, Rules>, String> {
        if !rules.derived.is_empty() || self.plan.derives.is_empty() {
            return Ok(std::borrow::Cow::Borrowed(rules));
        }
        let mut owned = rules.clone();
        self.settle(&mut owned)?;
        Ok(std::borrow::Cow::Owned(owned))
    }

    /// Recompute the derives into [`Rules::derived`].
    pub fn settle(&self, rules: &mut Rules) -> Result<(), String> {
        let slots = self.slot_values(rules)?;
        let derives = self.derive_values(&slots)?;
        rules.derived = (self.plan.derives.iter().zip(derives))
            .map(|(row, v)| (self.plan.str(row.name).to_string(), v.expect("settled")))
            .collect();
        Ok(())
    }

    /// This program's rules, from another program's: slots kept by name
    /// where the kind still fits, the rest from this program's initializers.
    pub fn adopt(&self, old: &Rules) -> Result<(Rules, Adopted), String> {
        let mut rules = self.boot()?;
        let mut adopted = Adopted::default();
        for (name, fresh) in rules.slots.iter_mut() {
            match old.slots.get(name) {
                Some(v) if kind(v) == kind(fresh) => *fresh = v.clone(),
                Some(_) => adopted.reset.push(name.clone()),
                None => adopted.added.push(name.clone()),
            }
        }
        adopted.dropped = (old.slots.keys())
            .filter(|k| !rules.slots.contains_key(*k))
            .cloned()
            .collect();
        self.settle(&mut rules)?;
        Ok((rules, adopted))
    }

    fn slot_name(&self, i: usize) -> &str {
        self.plan.str(self.plan.slots[i].name)
    }

    fn writes(&self, action: &ActionsRow) -> Vec<u32> {
        action
            .writes
            .iter()
            .map(|w| self.plan.write(w).slot.0)
            .collect()
    }

    fn slot_values(&self, rules: &Rules) -> Result<Vec<Value>, String> {
        (0..self.plan.slots.len())
            .map(|i| {
                let name = self.slot_name(i);
                rules
                    .slots
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("rules: the state has no slot `{name}`; adopt it first"))
            })
            .collect()
    }

    /// Every derive, in as many passes as their dependencies need.
    fn derive_values(&self, slots: &[Value]) -> Result<Vec<Option<Value>>, String> {
        let mut derives: Vec<Option<Value>> = vec![None; self.plan.derives.len()];
        for _ in 0..=derives.len() {
            let mut progress = false;
            for (i, row) in self.plan.derives.iter().enumerate() {
                if derives[i].is_some() {
                    continue;
                }
                match self.eval(self.plan.code(row.body), slots, &derives, &[], &[]) {
                    Ok(out) => {
                        derives[i] = Some(out.value);
                        progress = true;
                    }
                    Err(e) if e.starts_with("Pending") => {}
                    Err(e) => {
                        return Err(format!("rules: derive {}: {e}", self.plan.str(row.name)))
                    }
                }
            }
            if !progress {
                break;
            }
        }
        if let Some(i) = derives.iter().position(Option::is_none) {
            return Err(format!(
                "rules: derive {} never settled",
                self.plan.str(self.plan.derives[i].name)
            ));
        }
        Ok(derives)
    }

    fn eval(
        &self,
        code: &[u8],
        slots: &[Value],
        derives: &[Option<Value>],
        params: &[Value],
        allowed: &[u32],
    ) -> Result<vm::Outcome, String> {
        let env = vm::Env {
            plan: &self.plan,
            strings: &self.strings,
            router: None,
            lists: None,
            format: None,
            geometry: None,
            slots,
            derives,
            resources: &[],
            params,
            frames: &[],
            // The rules have no clock of their own; `now()` reads zero.
            now_ms: 0.0,
            pending_resources: &[],
            failed_resources: &[],
            pending_mutations: &[],
            store_dependent_derives: &[],
            store_dependent_resources: &[],
        };
        vm::eval(code, &env, allowed).map_err(|t| format!("{t:?}"))
    }
}

#[cfg(test)]
mod tests;
