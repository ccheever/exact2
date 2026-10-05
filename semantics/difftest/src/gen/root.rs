//! The root component: its slots, resources, mutation, derives, actions
//! and tasks. The view is [`super::view`]'s.

use super::action::Writes;
use super::ty::Ty;
use super::{ActionSig, Env, Gen, Mutation};

/// Where a mutation's `then` goes once the actions are known.
const THEN: &str = "{THEN}";

/// The root's declarations, written before the children are chosen.
pub(crate) struct Root {
    decls: String,
    states: Vec<(String, Ty)>,
    /// States that start as `none` or `[]`: some action writes each a
    /// value that says its type.
    pub(crate) untyped: Vec<(String, Ty)>,
    /// The resources alone: what a typing write is built from.
    resources: Env,
    /// Everything a root expression reads.
    pub(crate) env: Env,
    /// The states alone, less those typed only by a write.
    pub(crate) states_env: Env,
}

impl Gen<'_> {
    pub(crate) fn root_decls(&mut self) -> Root {
        let mut decls = String::new();
        let mut env = Env {
            vars: Vec::new(),
            now: true,
            fns: self.fns.len(),
        };
        let mut states = Vec::new();
        let mut untyped = Vec::new();
        for k in 0..self.rng.range(1, self.size.states.max(1)) {
            let name = format!("s{k}");
            let t = self.any_ty();
            let init = self.lit(&t, true);
            if t.needs_list() || init == "none" {
                untyped.push((name.clone(), t.clone()));
            }
            decls.push_str(&format!("  state {name} = {init}\n"));
            states.push((name, t));
        }
        env.vars.extend(states.iter().cloned());
        let mut states_env = env.clone();
        states_env.vars.retain(|v| !untyped.contains(v));

        // The first resource is always a list, so every scope has one to map.
        let mut resources = Env::default();
        for k in 0..self.rng.range(1, 2) {
            let t = if k == 0 {
                Ty::list(self.base_ty())
            } else {
                match self.rng.weighted(&[3, 2, 2, 1, 1]) {
                    0 => Ty::list(self.base_ty()),
                    1 => self.base_ty(),
                    2 => Ty::Num,
                    3 => Ty::Str,
                    _ => Ty::Bool,
                }
            };
            let args: Vec<String> = (0..self.rng.range(0, 2))
                .map(|_| {
                    let a = self.scalar_ty();
                    self.expr(&states_env, &a, 1, false)
                })
                .collect();
            let name = format!("r{k}");
            decls.push_str(&format!(
                "  resource {name} = load{k}({}) as shape {}\n",
                args.join(", "),
                self.ty_name(&t)
            ));
            resources.vars.push((name, t));
        }
        env.vars.extend(resources.vars.iter().cloned());

        if self.rng.chance(1, 3) {
            let t = self.base_ty();
            let name = "m0".to_string();
            // Never two sends to one mutation without `queue` (D12). `then`
            // is filled in once the actions are known.
            let queue = self.size.schedule && self.rng.chance(1, 2);
            decls.push_str(&format!(
                "  mutation {name} as shape {}{}{THEN}\n",
                self.ty_name(&t),
                if queue { " queue" } else { "" }
            ));
            let args = (0..self.rng.range(0, 2))
                .map(|_| self.scalar_ty())
                .collect();
            self.mutations.push(Mutation {
                name: name.clone(),
                source: "send0".into(),
                args,
                queue,
            });
            env.vars.push((name, Ty::opt(t)));
        }

        if self.routes {
            let reads = self.nav_derives(&mut decls);
            env.vars.extend(reads);
        }

        // A derive is typed before the writes that type a state starting
        // as `none` or `[]`: reading one there is refused (`?` has no
        // fields), so derives leave those states out.
        let mut derive_env = env.clone();
        derive_env.vars.retain(|v| !untyped.contains(v));
        for k in 0..self.rng.range(0, self.size.derives) {
            let t = self.pick_ty(&derive_env);
            let e = self.expr(&derive_env, &t, self.size.depth, false);
            let name = format!("d{k}");
            decls.push_str(&format!("  derive {name} = {e}\n"));
            env.vars.push((name.clone(), t.clone()));
            derive_env.vars.push((name, t));
        }

        for k in 0..self.rng.range(1, self.size.actions.max(1)) {
            let mut params = Vec::new();
            match self.rng.weighted(&[9, 7, 4]) {
                0 => {}
                1 => {
                    for _ in 0..self.rng.range(1, 2) {
                        params.push((self.fresh("p"), self.any_ty()));
                    }
                }
                _ => {
                    // An input's handler: the typed text is the last argument.
                    if self.rng.chance(1, 3) {
                        params.push((self.fresh("p"), self.any_ty()));
                    }
                    params.push((self.fresh("p"), Ty::Str));
                }
            }
            self.actions.push(ActionSig {
                name: format!("a{k}"),
                params,
            });
        }

        // A mutation's `then`: an action without parameters, run as its
        // own commit at the next clock advance after an answer lands. It
        // never sends (else every advance would re-arm it until the fire
        // limit).
        let idle: Vec<usize> = (0..self.actions.len())
            .filter(|&k| self.actions[k].params.is_empty())
            .collect();
        self.then = None;
        let then = if !self.mutations.is_empty() && !idle.is_empty() && self.rng.chance(1, 2) {
            let k = *self.rng.pick(&idle);
            self.then = Some(k);
            self.targets.clock = true;
            format!(" then {}", self.actions[k].name)
        } else {
            String::new()
        };
        let decls = decls.replace(THEN, &then);

        Root {
            decls,
            states,
            untyped,
            resources,
            env,
            states_env,
        }
    }

    pub(crate) fn root(&mut self, root: Root) -> String {
        let mut out = String::from("component App\n");
        out.push_str(&root.decls);

        let mut first: Vec<Vec<String>> = vec![Vec::new(); self.actions.len()];
        for (slot, t) in &root.untyped {
            let k = self.rng.below(self.actions.len() as u64) as usize;
            let v = self.leaf(&root.resources, t, false);
            first[k].push(format!("{slot} = {v}"));
        }
        for (k, a) in self.actions.clone().iter().enumerate() {
            let writes = Writes {
                slots: &root.states,
                send: self.then != Some(k),
                nav: self.routes,
            };
            let mut env = root.env.clone();
            env.vars.extend(a.params.iter().cloned());
            let ps: Vec<String> = a
                .params
                .iter()
                .map(|(n, t)| format!("{n}: {}", self.ty_name(t)))
                .collect();
            if ps.is_empty() {
                out.push_str(&format!("  action {}\n", a.name));
            } else {
                out.push_str(&format!("  action {}({})\n", a.name, ps.join(", ")));
            }
            self.callable = k;
            self.sent.clear();
            // A body that only calls needs a callee: a mutation's `then`
            // action may call only actions that send nothing.
            if k > 0 && first[k].is_empty() && !self.callees().is_empty() && self.rng.chance(1, 3) {
                out.push_str(&self.dispatch(&env, 4));
            } else {
                out.push_str(&self.body(&env, &writes, &first[k], 4));
            }
            let sending = std::mem::take(&mut self.sending);
            self.sends.push(sending);
        }
        self.callable = 0;

        let idle: Vec<String> = self
            .actions
            .iter()
            .filter(|a| a.params.is_empty())
            .map(|a| a.name.clone())
            .collect();
        if !idle.is_empty() {
            for k in 0..2 {
                if !self.rng.chance(1, 3) {
                    continue;
                }
                let a = self.rng.pick(&idle).clone();
                let line = if self.rng.chance(1, 5) {
                    // Virtual frames: every 1000/60 ms of an advance.
                    format!("every(frame, {a})")
                } else if self.rng.chance(2, 3) {
                    let ms = self.rng.pick(&[16, 100, 250, 1000]);
                    format!("every({ms}, {a})")
                } else {
                    let ms = self.rng.pick(&[1, 100, 500]);
                    format!("after({ms}, {a})")
                };
                // A gate and a key over the states alone: never `now()`,
                // directly or through a derive or a `fn` (D9, D12).
                let start = if self.size.schedule && self.rng.chance(1, 2) {
                    let mut genv = root.states_env.clone();
                    genv.now = false;
                    genv.fns = 0;
                    let gate = self.expr(&genv, &Ty::Bool, 2, false);
                    let kt = self.scalar_ty();
                    let key = self.expr(&genv, &kt, 2, false);
                    match self.rng.below(3) {
                        0 => format!("when {gate}"),
                        1 => format!("when {gate} key={key}"),
                        _ => format!("key={key}"),
                    }
                } else {
                    "mount".to_string()
                };
                out.push_str(&format!("  task t{k} {start}\n    {line}\n"));
                self.targets.clock = true;
            }
        }

        out.push_str(&self.root_view(&root));
        out
    }
}
