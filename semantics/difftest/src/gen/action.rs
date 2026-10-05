//! Action bodies: assignments (the same slot written twice, slots read
//! after a write, which still sees the starting value), `let`, `if`/`else`,
//! `match` on an option, `send`, and calls of the root's earlier actions
//! (LLP 1089 D9: action *i* calls actions *j < i*, so no call cycles):
//! anywhere among the statements, and in bodies that only call
//! ([`Gen::dispatch`]). A program whose statement calls the compiler
//! refuses is written again with only the latter ([`super::case`]).

use super::ty::Ty;
use super::{Env, Gen};

/// Longest a slot holding a string or a list may print before a write resets
/// it, so a self-concatenating action cannot grow a slot past the runner's
/// limits.
const GROWTH_CAP: usize = 200;

/// What a body may write.
pub(crate) struct Writes<'a> {
    /// Assignable slots.
    pub(crate) slots: &'a [(String, Ty)],
    /// Whether `send` statements may be written (the root's mutations).
    pub(crate) send: bool,
    /// Whether `nav = verb(…)` may be written (the root's router slot).
    pub(crate) nav: bool,
}

impl Gen<'_> {
    /// An action's statements at `indent`, after `first` (writes that fix
    /// an uninitialized slot's type).
    pub(crate) fn body(
        &mut self,
        env: &Env,
        w: &Writes,
        first: &[String],
        indent: usize,
    ) -> String {
        let mut out = String::new();
        let pad = " ".repeat(indent);
        self.sent.clear();
        self.sending.clear();
        for line in first {
            out.push_str(&format!("{pad}{line}\n"));
        }
        let n = self.rng.range(usize::from(first.is_empty()), 4);
        self.block(env.clone(), w, 2, indent, n, &mut out);
        out
    }

    fn block(
        &mut self,
        mut env: Env,
        w: &Writes,
        depth: usize,
        indent: usize,
        n: usize,
        out: &mut String,
    ) {
        for _ in 0..n {
            self.stmt(&mut env, w, depth, indent, out);
        }
    }

    fn stmt(&mut self, env: &mut Env, w: &Writes, depth: usize, indent: usize, out: &mut String) {
        let pad = " ".repeat(indent);
        let d = self.size.depth.saturating_sub(1).max(1);
        let weights = [
            if w.slots.is_empty() { 0 } else { 10 },
            3,
            if depth > 0 { 2 } else { 0 },
            if depth > 0 { 2 } else { 0 },
            if w.send && !self.unsent().is_empty() {
                2
            } else {
                0
            },
            if self.calls && !self.callees().is_empty() {
                2
            } else {
                0
            },
            if w.nav { 3 } else { 0 },
        ];
        match self.rng.weighted(&weights) {
            0 => {
                let (slot, t) = self.rng.pick(w.slots).clone();
                self.assign(env, &slot, &t, d, &pad, out);
                // The same slot again, sometimes: the later write wins.
                if self.rng.chance(1, 6) {
                    self.assign(env, &slot, &t, d, &pad, out);
                }
            }
            1 => {
                let t = self.pick_ty(env);
                let name = self.fresh("l");
                let e = self.expr(env, &t, d, false);
                out.push_str(&format!("{pad}let {name} = {e}\n"));
                env.vars.push((name, t));
            }
            2 => {
                let c = self.expr(env, &Ty::Bool, d, false);
                out.push_str(&format!("{pad}if {c}\n"));
                let n = self.rng.range(1, 2);
                let before = self.sent.clone();
                self.block(env.clone(), w, depth - 1, indent + 2, n, out);
                let then = std::mem::replace(&mut self.sent, before);
                if self.rng.chance(1, 2) {
                    out.push_str(&format!("{pad}else\n"));
                    let n = self.rng.range(1, 2);
                    self.block(env.clone(), w, depth - 1, indent + 2, n, out);
                }
                self.join_sent(then);
            }
            3 => {
                let inner = self.held(env, true);
                let subject = self.expr(env, &Ty::opt(inner.clone()), 1, false);
                let v = self.fresh("v");
                out.push_str(&format!("{pad}match {subject}\n{pad}  case some({v})\n"));
                let n = self.rng.range(1, 2);
                let before = self.sent.clone();
                self.block(env.with(&v, inner), w, depth - 1, indent + 4, n, out);
                let some = std::mem::replace(&mut self.sent, before);
                out.push_str(&format!("{pad}  case none\n"));
                let n = self.rng.range(1, 2);
                self.block(env.clone(), w, depth - 1, indent + 4, n, out);
                self.join_sent(some);
            }
            5 => self.call_action(env, &pad, out),
            6 => out.push_str(&self.nav_stmt(env, &pad)),
            _ => {
                let unsent = self.unsent();
                let m = self.rng.pick(&unsent).clone();
                self.sent.push(m.name.clone());
                self.sending.push(m.name.clone());
                let args: Vec<String> = m.args.iter().map(|t| self.expr(env, t, 1, true)).collect();
                out.push_str(&format!(
                    "{pad}send {} = {}({})\n",
                    m.name,
                    m.source,
                    args.join(", ")
                ));
            }
        }
    }

    /// A call of one of the root's earlier actions, its arguments typed by
    /// its parameters.
    fn call_action(&mut self, env: &Env, pad: &str, out: &mut String) {
        let callees = self.callees();
        let j = callees[self.rng.below(callees.len() as u64) as usize];
        let callee = self.actions[j].clone();
        let sends = self.sends[j].clone();
        self.sent.extend(sends.iter().cloned());
        self.sending.extend(sends);
        let args: Vec<String> = callee
            .params
            .iter()
            .map(|(_, t)| self.expr(env, t, 1, true))
            .collect();
        out.push_str(&format!("{pad}{}({})\n", callee.name, args.join(", ")));
        self.called = true;
    }

    /// A body that only calls: `let`s, then one call, or one in each arm of
    /// an `if`. It writes nothing of its own, so no read in it or its
    /// callee is made stale by another frame (LLP 1089 D3), whatever the
    /// callees read.
    pub(crate) fn dispatch(&mut self, env: &Env, indent: usize) -> String {
        let mut out = String::new();
        self.sent.clear();
        self.sending.clear();
        let pad = " ".repeat(indent);
        let mut env = env.clone();
        for _ in 0..self.rng.range(0, 1) {
            let t = self.pick_ty(&env);
            let name = self.fresh("l");
            let e = self.expr(&env, &t, 1, false);
            out.push_str(&format!("{pad}let {name} = {e}\n"));
            env.vars.push((name, t));
        }
        if self.rng.chance(1, 2) {
            self.call_action(&env, &pad, &mut out);
        } else {
            let c = self.expr(&env, &Ty::Bool, 1, false);
            out.push_str(&format!("{pad}if {c}\n"));
            let inner = " ".repeat(indent + 2);
            self.call_action(&env, &inner, &mut out);
            self.sent.clear();
            out.push_str(&format!("{pad}else\n"));
            self.call_action(&env, &inner, &mut out);
        }
        out
    }

    /// The mutations the path being written has not sent yet.
    fn unsent(&self) -> Vec<super::Mutation> {
        self.mutations
            .iter()
            .filter(|m| !self.sent.contains(&m.name))
            .cloned()
            .collect()
    }

    /// The earlier actions this path may call: none sends what it has sent.
    /// A mutation's `then` action sends nothing, through a call neither.
    pub(crate) fn callees(&self) -> Vec<usize> {
        let then = self.then == Some(self.callable);
        (0..self.callable)
            .filter(|&j| {
                self.sends[j].iter().all(|m| !self.sent.contains(m))
                    && !(then && !self.sends[j].is_empty())
            })
            .collect()
    }

    /// After a branch: what either arm sent (the other arm's is in `sent`).
    fn join_sent(&mut self, arm: Vec<String>) {
        for m in arm {
            if !self.sent.contains(&m) {
                self.sent.push(m);
            }
        }
    }

    /// `slot = …`. A value that can grow is bound first and reset to a
    /// literal when it prints longer than [`GROWTH_CAP`].
    fn assign(&mut self, env: &Env, slot: &str, t: &Ty, d: usize, pad: &str, out: &mut String) {
        if !self.grows(t) {
            let e = self.expr(env, t, d, true);
            out.push_str(&format!("{pad}{slot} = {e}\n"));
            return;
        }
        let name = self.fresh("l");
        let e = self.expr(env, t, d, false);
        let shown = self.show(&name, t);
        let reset = self.lit(t, true);
        out.push_str(&format!(
            "{pad}let {name} = {e}\n{pad}{slot} = ((length({shown}) > {GROWTH_CAP}) ? {reset} : {name})\n"
        ));
    }
}
