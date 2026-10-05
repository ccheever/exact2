//! Views: every slot shown in a `text` with a `testId`, buttons and inputs
//! bound to actions, `when`/`match`/`each` blocks, and child components
//! with their own state, used once or once per row.

use super::action::Writes;
use super::root::Root;
use super::ty::{quote, Ty};
use super::{Child, Env, Gen};

const LABELS: &[&str] = &["go", "tap", "+", "do it", "Save", "×", "next", "☆"];

/// Where a view item is written.
#[derive(Clone)]
struct Ctx {
    env: Env,
    /// `env` without the states typed only by a write: what a `when`,
    /// `match` or `each` head reads (it is typed as a derive is).
    heads: Env,
    /// Enclosing `each` index names, outermost first.
    ixs: Vec<String>,
    /// The root's states and the enclosing rows' items and indexes: all a
    /// prop that a child's state starts from may read.
    states: Env,
}

impl Gen<'_> {
    pub(crate) fn gen_children(&mut self, out: &mut String) {
        let n = match self.rng.weighted(&[3, 3, 1]) {
            0 => 0,
            1 => 1,
            _ => 2,
        };
        for k in 0..n {
            out.push_str(&self.child(k));
            out.push('\n');
        }
    }

    fn child(&mut self, k: usize) -> String {
        let name = format!("C{k}");
        let props: Vec<(String, Ty)> = (0..self.rng.range(0, 2))
            .map(|_| (self.fresh("q"), self.any_ty()))
            .collect();
        let cb = if !self.actions.is_empty() && self.rng.chance(1, 3) {
            let a = self.rng.pick(&self.actions).clone();
            Some(a.params.iter().map(|(_, t)| t.clone()).collect::<Vec<_>>())
        } else {
            None
        };
        let mut env = Env {
            vars: vec![("idx".to_string(), Ty::Num)],
            now: true,
            fns: self.fns.len(),
        };
        env.vars.extend(props.iter().cloned());
        let props_env = env.clone();

        let mut out = format!("component {name}\n  props\n    idx: number\n");
        for (q, t) in &props {
            out.push_str(&format!("    {q}: {}\n", self.ty_name(t)));
        }
        if cb.is_some() {
            out.push_str("    cb: action\n");
        }

        let mut states = Vec::new();
        let mut untyped = Vec::new();
        let mut first = Vec::new();
        let mut inits = Vec::new();
        for _ in 0..self.rng.range(1, 2) {
            let s = self.fresh("c");
            let t = self.pick_ty(&props_env);
            let same: Vec<String> = props
                .iter()
                .filter(|(_, pt)| *pt == t && !t.needs_list())
                .map(|(q, _)| q.clone())
                .collect();
            let init = if !same.is_empty() && self.rng.chance(1, 2) {
                let q = self.rng.pick(&same).clone();
                inits.push(q.clone());
                q
            } else {
                let init = self.lit(&t, true);
                if t.needs_list() || init == "none" {
                    let v = self.leaf(&props_env, &t, false);
                    first.push(format!("{s} = {v}"));
                    untyped.push((s.clone(), t.clone()));
                }
                init
            };
            out.push_str(&format!("  state {s} = {init}\n"));
            states.push((s, t));
        }
        env.vars.extend(states.iter().cloned());
        let mut heads = env.clone();
        heads.vars.retain(|v| !untyped.contains(v));
        for _ in 0..self.rng.range(0, 1) {
            let d = self.fresh("cd");
            let t = self.pick_ty(&heads);
            let e = self.expr(&heads, &t, self.size.depth.min(2), false);
            out.push_str(&format!("  derive {d} = {e}\n"));
            env.vars.push((d.clone(), t.clone()));
            heads.vars.push((d, t));
        }

        let mut actions = Vec::new();
        for i in 0..self.rng.range(1, 2) {
            let a = self.fresh("ca");
            let params: Vec<(String, Ty)> = (0..self.rng.range(0, 1))
                .map(|_| (self.fresh("p"), self.any_ty()))
                .collect();
            let ps: Vec<String> = params
                .iter()
                .map(|(n, t)| format!("{n}: {}", self.ty_name(t)))
                .collect();
            if ps.is_empty() {
                out.push_str(&format!("  action {a}\n"));
            } else {
                out.push_str(&format!("  action {a}({})\n", ps.join(", ")));
            }
            let mut aenv = env.clone();
            aenv.vars.extend(params.iter().cloned());
            let w = Writes {
                slots: &states,
                send: false,
                nav: false,
            };
            let typing = if i == 0 {
                std::mem::take(&mut first)
            } else {
                Vec::new()
            };
            out.push_str(&self.body(&aenv, &w, &typing, 4));
            actions.push((a, params));
        }

        out.push_str(&format!("  view\n    column testId=`{name}-${{idx}}`\n"));
        let mut press = Vec::new();
        let mut inert = Vec::new();
        for (x, t) in env.vars.clone().iter().skip(1) {
            let shown = self.show(x, t);
            out.push_str(&format!(
                "      text {shown} testId=`{name}-${{idx}}-{x}`\n"
            ));
            inert.push(x.clone());
        }
        for (a, params) in &actions {
            let label = quote(self.rng.pick(LABELS));
            let types: Vec<Ty> = params.iter().map(|(_, t)| t.clone()).collect();
            let press_expr = self.press(&heads, a, &types);
            out.push_str(&format!(
                "      button {label} press={press_expr} testId=`{name}-${{idx}}-{a}`\n"
            ));
            press.push(a.clone());
        }
        if let Some(types) = &cb {
            let press_expr = self.press(&heads, "cb", types);
            out.push_str(&format!(
                "      button \"cb\" press={press_expr} testId=`{name}-${{idx}}-cb`\n"
            ));
            press.push("cb".into());
        }
        self.children.push(Child {
            name,
            props,
            cb,
            inits,
            press,
            inert,
        });
        out
    }

    /// `action` or `action(args)`.
    fn press(&mut self, env: &Env, action: &str, params: &[Ty]) -> String {
        if params.is_empty() {
            return action.to_string();
        }
        let args: Vec<String> = params.iter().map(|t| self.expr(env, t, 2, true)).collect();
        format!("{action}({})", args.join(", "))
    }

    pub(crate) fn root_view(&mut self, root: &Root) -> String {
        let mut out = String::from("  view\n    column testId=\"root\"\n");
        for (x, t) in root.env.vars.clone() {
            let shown = self.show(&x, &t);
            out.push_str(&format!("      text {shown} testId=\"s-{x}\"\n"));
            self.targets.inert.push(format!("s-{x}"));
        }
        let mut heads = root.env.clone();
        heads.vars.retain(|v| !root.untyped.contains(v));
        let ctx = Ctx {
            heads,
            env: root.env.clone(),
            ixs: Vec::new(),
            states: root.states_env.clone(),
        };
        for a in self.actions.clone() {
            let types: Vec<Ty> = a.params.iter().map(|(_, t)| t.clone()).collect();
            if types.last() == Some(&Ty::Str) && self.rng.chance(2, 3) {
                self.input(&ctx, &a.name, &types, 6, &mut out);
            }
            for _ in 0..self.rng.range(1, 2) {
                self.button(&ctx, &a.name, &types, 6, &mut out);
            }
        }
        for _ in 0..self.rng.range(1, 3) {
            self.item(&ctx, 2, 6, &mut out);
        }
        for k in 0..self.children.len() {
            if self.rng.chance(1, 2) {
                self.use_child(&ctx, k, 6, &mut out);
            }
        }
        out
    }

    /// A `testId` attribute value, and the concrete ids it takes for the
    /// first few rows of every enclosing `each`.
    fn tid(&mut self, base: &str, ixs: &[String]) -> (String, Vec<String>) {
        if ixs.is_empty() {
            return (quote(base), vec![base.to_string()]);
        }
        let mut written = format!("`{base}");
        for ix in ixs {
            written.push_str(&format!("-${{{ix}}}"));
        }
        written.push('`');
        let mut ids = Vec::new();
        for pick in 0..3u64 {
            let mut id = base.to_string();
            for _ in ixs {
                let i = if pick < 2 { pick } else { self.rng.below(3) };
                id.push_str(&format!("-{i}"));
            }
            ids.push(id);
        }
        (written, ids)
    }

    fn button(&mut self, ctx: &Ctx, action: &str, types: &[Ty], indent: usize, out: &mut String) {
        let id = self.fresh(&format!("b-{action}-"));
        let (written, ids) = self.tid(&id, &ctx.ixs);
        let press = self.press(&ctx.heads, action, types);
        let label = quote(self.rng.pick(LABELS));
        out.push_str(&format!(
            "{}button {label} press={press} testId={written}\n",
            " ".repeat(indent)
        ));
        self.targets.press.extend(ids);
    }

    /// An input whose `change` calls `action` with the typed text last.
    fn input(&mut self, ctx: &Ctx, action: &str, types: &[Ty], indent: usize, out: &mut String) {
        let id = self.fresh(&format!("in-{action}-"));
        let (written, ids) = self.tid(&id, &ctx.ixs);
        let change = self.press(&ctx.heads, action, &types[..types.len() - 1]);
        let value = self.expr(&ctx.heads, &Ty::Str, 1, false);
        out.push_str(&format!(
            "{}input testId={written} value={value} change={change}\n",
            " ".repeat(indent)
        ));
        self.targets.inputs.extend(ids);
    }

    fn item(&mut self, ctx: &Ctx, depth: usize, indent: usize, out: &mut String) {
        let pad = " ".repeat(indent);
        let w = [
            4,
            if self.actions.is_empty() { 0 } else { 3 },
            if depth > 0 { 2 } else { 0 },
            if depth > 0 { 2 } else { 0 },
            if depth > 0 { 3 } else { 0 },
            if self.children.is_empty() { 0 } else { 2 },
        ];
        match self.rng.weighted(&w) {
            0 => {
                let t = self.scalar_ty();
                let e = self.expr(&ctx.env, &t, self.size.depth, false);
                let shown = match t {
                    Ty::Str => format!("({e})"),
                    _ => format!("`${{{e}}}`"),
                };
                let id = self.fresh("e");
                let (written, ids) = self.tid(&id, &ctx.ixs);
                out.push_str(&format!("{pad}text {shown} testId={written}\n"));
                self.targets.inert.extend(ids);
            }
            1 => {
                let a = self.rng.pick(&self.actions).clone();
                let types: Vec<Ty> = a.params.iter().map(|(_, t)| t.clone()).collect();
                if types.last() == Some(&Ty::Str) && self.rng.chance(1, 3) {
                    self.input(ctx, &a.name, &types, indent, out);
                } else {
                    self.button(ctx, &a.name, &types, indent, out);
                }
            }
            2 => {
                let c = self.expr(&ctx.heads, &Ty::Bool, 2, false);
                out.push_str(&format!("{pad}when {c}\n"));
                self.arm_child(ctx, indent + 2, out);
                self.items(ctx, depth - 1, indent + 2, out);
                if self.rng.chance(1, 2) {
                    out.push_str(&format!("{pad}else\n"));
                    self.arm_child(ctx, indent + 2, out);
                    self.items(ctx, depth - 1, indent + 2, out);
                }
            }
            3 => {
                let inner = self.held(&ctx.heads, true);
                let subject = self.expr(&ctx.heads, &Ty::opt(inner.clone()), 2, false);
                let v = self.fresh("v");
                out.push_str(&format!("{pad}match {subject}\n{pad}  case some({v})\n"));
                let mut some = ctx.clone();
                some.env = some.env.with(&v, inner.clone());
                some.heads = some.heads.with(&v, inner.clone());
                let id = self.fresh("m");
                let (written, ids) = self.tid(&id, &ctx.ixs);
                let shown = self.show(&v, &inner);
                out.push_str(&format!("{pad}    text {shown} testId={written}\n"));
                self.targets.inert.extend(ids);
                self.arm_child(ctx, indent + 4, out);
                if self.rng.chance(1, 2) {
                    self.items(&some, depth - 1, indent + 4, out);
                }
                out.push_str(&format!("{pad}  case none\n"));
                self.arm_child(ctx, indent + 4, out);
                self.items(ctx, depth - 1, indent + 4, out);
            }
            4 => {
                let t = self.held(&ctx.heads, false);
                let list = self.expr(&ctx.heads, &Ty::list(t.clone()), 2, false);
                let it = self.fresh("it");
                let ix = self.fresh("ix");
                out.push_str(&format!("{pad}each {it}, {ix} in {list} key={ix}\n"));
                let mut row = ctx.clone();
                row.env = row.env.with(&it, t.clone()).with(&ix, Ty::Num);
                row.heads = row.heads.with(&it, t.clone()).with(&ix, Ty::Num);
                row.states = row.states.with(&it, t.clone()).with(&ix, Ty::Num);
                row.ixs.push(ix);
                let id = self.fresh("row");
                let (written, _) = self.tid(&id, &row.ixs);
                out.push_str(&format!("{pad}  row testId={written}\n"));
                let shown = self.show(&it, &t);
                let (written, ids) = self.tid(&format!("{id}-t"), &row.ixs);
                out.push_str(&format!("{pad}    text {shown} testId={written}\n"));
                self.targets.inert.extend(ids);
                for _ in 0..self.rng.range(0, 2) {
                    self.item(&row, depth - 1, indent + 4, out);
                }
            }
            _ => {
                let k = self.rng.below(self.children.len() as u64) as usize;
                self.use_child(ctx, k, indent, out);
            }
        }
    }

    /// Now and then a stateful child at the head of a `when` or `match`
    /// arm: the arm instance owns its state, so hiding the arm and showing
    /// it again starts the child afresh. Its props read the arm's
    /// enclosing scope (`ctx`), never the `match` binder.
    fn arm_child(&mut self, ctx: &Ctx, indent: usize, out: &mut String) {
        if !self.children.is_empty() && self.rng.chance(1, 3) {
            let k = self.rng.below(self.children.len() as u64) as usize;
            self.use_child(ctx, k, indent, out);
        }
    }

    fn items(&mut self, ctx: &Ctx, depth: usize, indent: usize, out: &mut String) {
        for _ in 0..self.rng.range(1, 2) {
            self.item(ctx, depth, indent, out);
        }
    }

    /// `Ck(idx=…, …)`: a literal `idx` outside an `each`, the row index
    /// plus an offset inside one, so every instance's ids differ.
    fn use_child(&mut self, ctx: &Ctx, k: usize, indent: usize, out: &mut String) {
        let child = self.children[k].clone();
        self.uses += 1;
        let base = 100 * self.uses;
        let (idx, idxs) = match ctx.ixs.last() {
            None => (base.to_string(), vec![base]),
            Some(ix) => (format!("({ix} + {base})"), vec![base, base + 1, base + 2]),
        };
        // A prop a child's state starts from may read only the root's
        // states and the rows' items: a derive, resource or `match` binder
        // there is refused as an unknown name. And a prop's declared type
        // does not pin its argument (`C(o=none)` leaves `o` an `option<?>`),
        // so no argument is a bare `none` or `[]`.
        let mut args = vec![format!("idx={idx}")];
        for (q, t) in &child.props {
            let env = if child.inits.contains(q) {
                &ctx.states
            } else {
                &ctx.heads
            };
            let env = env.clone();
            args.push(format!("{q}={}", self.expr(&env, t, 2, false)));
        }
        if let Some(types) = &child.cb {
            let fits: Vec<String> = self
                .actions
                .iter()
                .filter(|a| a.params.iter().map(|(_, t)| t).eq(types.iter()))
                .map(|a| a.name.clone())
                .collect();
            args.push(format!("cb={}", self.rng.pick(&fits)));
        }
        out.push_str(&format!(
            "{}{}({})\n",
            " ".repeat(indent),
            child.name,
            args.join(", ")
        ));
        for i in idxs {
            for a in &child.press {
                self.targets.press.push(format!("{}-{i}-{a}", child.name));
            }
            for x in &child.inert {
                self.targets.inert.push(format!("{}-{i}-{x}", child.name));
            }
        }
    }
}
