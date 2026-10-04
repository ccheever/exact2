//! Action bodies: assignments (the same slot written twice, slots read
//! after a write, which still sees the starting value), `let`, `if`/`else`,
//! `match` on an option, and `send`.

use super::ty::Ty;
use super::{Env, Gen};

/// Longest a string-holding slot may print before a write resets it, so a
/// self-concatenating action cannot grow a slot past the runner's limits.
const GROWTH_CAP: usize = 200;

/// What a body may write.
pub(crate) struct Writes<'a> {
    /// Assignable slots.
    pub(crate) slots: &'a [(String, Ty)],
    /// Whether `send` statements may be written (the root's mutations).
    pub(crate) send: bool,
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
            if w.send && !self.mutations.is_empty() {
                2
            } else {
                0
            },
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
                self.block(env.clone(), w, depth - 1, indent + 2, n, out);
                if self.rng.chance(1, 2) {
                    out.push_str(&format!("{pad}else\n"));
                    let n = self.rng.range(1, 2);
                    self.block(env.clone(), w, depth - 1, indent + 2, n, out);
                }
            }
            3 => {
                let inner = self.held(env, true);
                let subject = self.expr(env, &Ty::opt(inner.clone()), 1, false);
                let v = self.fresh("v");
                out.push_str(&format!("{pad}match {subject}\n{pad}  case some({v})\n"));
                let n = self.rng.range(1, 2);
                self.block(env.with(&v, inner), w, depth - 1, indent + 4, n, out);
                out.push_str(&format!("{pad}  case none\n"));
                let n = self.rng.range(1, 2);
                self.block(env.clone(), w, depth - 1, indent + 4, n, out);
            }
            _ => {
                let m = self.rng.pick(&self.mutations).clone();
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

    /// `slot = …`. A value holding a string is bound first and reset to a
    /// literal when it prints longer than [`GROWTH_CAP`].
    fn assign(&mut self, env: &Env, slot: &str, t: &Ty, d: usize, pad: &str, out: &mut String) {
        if !self.holds_str(t) {
            let e = self.expr(env, t, d, true);
            out.push_str(&format!("{pad}{slot} = {e}\n"));
            return;
        }
        if !self.makeable(env, t, false) {
            let e = self.lit(t, true);
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
