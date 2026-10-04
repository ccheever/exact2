//! Plan bytecode → JavaScript.
//!
//! The VM (`runner/src/vm.rs`) is a stack machine with forward jumps only, so
//! a body translates one instruction at a time: straight-line code builds
//! expressions on a symbolic stack; at a jump, a label or a side effect the
//! stack is flushed to registers (`s0`, `s1`, …, one per depth), and a
//! forward jump is a `break` out of a labeled block that ends at its target.
//! Locals are `l<index>` (the VM's absolute locals index). A `Map`/`Filter`
//! callback body is its own arrow function over `l<base>`, `l<base+1>`.
//!
//! Values: numbers, strings and bools are JavaScript's; a record or list is
//! an array; `none` is `null` and `some(x)` is `x` (the JS target does not
//! represent `some(none)`); unit is `null`.

use exact_plan::{Opcode, Plan, Stdlib};
use exact_runner::vm::{instructions, Instruction};
use std::collections::BTreeSet;

/// What names a body may read, from where it is compiled.
#[derive(Clone, Default)]
pub struct Scope {
    /// JavaScript getters for the enclosing region frames, innermost last:
    /// (item, index, bound).
    pub frames: Vec<Frame>,
    /// Whether this is an action body (writes and commands are allowed).
    pub action: bool,
    /// The JavaScript object holding the row slots in force (LLP 1017 P4c:
    /// a slot owned by an `each` lives on its row), by slot index.
    pub rows: Option<String>,
}

/// One region frame's getters.
#[derive(Clone, Default)]
pub struct Frame {
    pub item: Option<String>,
    pub index: Option<String>,
    pub bound: Option<String>,
}

/// Runtime names the generated code uses (for the import list).
#[derive(Default)]
pub struct Uses {
    pub names: BTreeSet<String>,
}

impl Uses {
    pub fn rt(&mut self, name: &str) -> String {
        self.names.insert(name.to_string());
        name.to_string()
    }
}

/// Whether `code` reads or writes a row slot.
pub fn touches_rows(plan: &Plan, code: &[u8]) -> bool {
    instructions(code).flatten().any(|i| {
        matches!(i.op, Opcode::LoadSlot | Opcode::StoreSlot)
            && plan.slots[i.args[0] as usize].owner.is_some()
    })
}

/// A JavaScript function expression for `code` (`() => …`, or `(p0, …) =>`
/// with `params` parameters).
pub fn function(
    plan: &Plan,
    code: &[u8],
    scope: &Scope,
    params: usize,
    uses: &mut Uses,
) -> Result<String, String> {
    let ins: Vec<Instruction> = instructions(code)
        .collect::<Result<_, _>>()
        .map_err(|t| format!("malformed code: {t:?}"))?;
    let mut t = Translator {
        plan,
        scope,
        uses,
        out: String::new(),
        stack: Vec::new(),
        max_depth: 0,
        locals: 0,
        max_local: None,
        base_local: 0,
    };
    let body = t.range(&ins, 0, ins.len())?;
    let ps: Vec<String> = (0..params).map(|i| format!("p{i}")).collect();
    Ok(wrap(&format!("({})", ps.join(",")), &t, body))
}

/// `code` as one JavaScript expression evaluated where it is written: the
/// expression itself for straight-line code, else an immediately called
/// function.
pub fn expression(
    plan: &Plan,
    code: &[u8],
    scope: &Scope,
    uses: &mut Uses,
) -> Result<String, String> {
    let f = function(plan, code, scope, 0, uses)?;
    Ok(match f.strip_prefix("()=>") {
        Some(e) if !e.starts_with('{') => e.to_string(),
        _ => format!("({f})()"),
    })
}

fn wrap(head: &str, t: &Translator<'_>, body: Body) -> String {
    match body {
        Body::Expr(e) if t.out.is_empty() => format!("{head}=>{}", paren_object(&e)),
        _ => {
            let out = t.out.strip_suffix("return;").unwrap_or(&t.out);
            format!("{head}=>{{{}{out}}}", t.decls())
        }
    }
}

fn paren_object(e: &str) -> String {
    if e.starts_with('{') {
        format!("({e})")
    } else {
        e.to_string()
    }
}

enum Body {
    /// Straight-line code whose whole result is one expression.
    Expr(String),
    /// Statements were written to `out` (ending in a return or not).
    Stmts,
}

struct Translator<'a> {
    plan: &'a Plan,
    scope: &'a Scope,
    uses: &'a mut Uses,
    out: String,
    stack: Vec<String>,
    max_depth: usize,
    locals: u16,
    max_local: Option<u16>,
    base_local: u16,
}

fn reg(d: usize) -> String {
    format!("s{d}")
}

impl Translator<'_> {
    fn decls(&self) -> String {
        let used = |r: &String| self.out.contains(&format!("{r}="));
        let mut names: Vec<String> = (0..self.max_depth).map(reg).filter(used).collect();
        if let Some(m) = self.max_local {
            names.extend((self.base_local..=m).map(|k| format!("l{k}")));
        }
        if names.is_empty() {
            String::new()
        } else {
            format!("let {};", names.join(","))
        }
    }

    fn push(&mut self, e: String) {
        self.stack.push(e);
        self.max_depth = self.max_depth.max(self.stack.len());
    }

    fn pop(&mut self) -> Result<String, String> {
        self.stack
            .pop()
            .ok_or_else(|| "stack underflow".to_string())
    }

    fn popn(&mut self, n: usize) -> Result<Vec<String>, String> {
        if self.stack.len() < n {
            return Err("stack underflow".into());
        }
        Ok(self.stack.split_off(self.stack.len() - n))
    }

    /// Materialize every symbolic entry into its register.
    fn flush(&mut self) {
        for (d, e) in self.stack.iter_mut().enumerate() {
            let r = reg(d);
            if *e != r {
                self.out.push_str(&format!("{r}={e};"));
                *e = r;
            }
        }
    }

    fn registers(&mut self, depth: usize) {
        self.stack = (0..depth).map(reg).collect();
        self.max_depth = self.max_depth.max(depth);
    }

    fn range(&mut self, ins: &[Instruction], from: usize, to: usize) -> Result<Body, String> {
        let end_pc = ins.get(to).map(|i| i.pc).unwrap_or(usize::MAX);
        // Top-level jumps of this range (callback bodies are their own).
        let mut targets: Vec<(usize, usize)> = Vec::new(); // (start pc, target pc)
        let mut i = from;
        while i < to {
            let x = &ins[i];
            match x.op {
                Opcode::Jump | Opcode::JumpIfFalse | Opcode::JumpIfNone => {
                    let target = x.args[0] as usize;
                    match targets.iter_mut().find(|(_, t)| *t == target) {
                        Some(e) => e.0 = e.0.min(x.pc),
                        None => targets.push((x.pc, target)),
                    }
                }
                Opcode::Map | Opcode::Filter => {
                    let end = x.args[0] as usize;
                    while i + 1 < to && ins[i + 1].pc < end {
                        i += 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        // Laminar: a block that starts inside another and ends after it
        // opens where the other opens instead.
        loop {
            let mut changed = false;
            for a in 0..targets.len() {
                for b in 0..targets.len() {
                    let (sa, ta) = targets[a];
                    let (sb, tb) = targets[b];
                    if sa < sb && sb < ta && ta < tb {
                        targets[b].0 = sa;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        let label = |t: usize| format!("L{t}");
        let mut depth_at: Vec<(usize, usize, u16)> = Vec::new(); // target, depth, locals
        let mut dead = false;
        let mut i = from;
        while i < to {
            let x = ins[i];
            // Close blocks ending here, then open blocks starting here.
            let closing: Vec<usize> = targets
                .iter()
                .filter(|(_, t)| *t == x.pc)
                .map(|(_, t)| *t)
                .collect();
            if !closing.is_empty() {
                let recorded = depth_at.iter().find(|(t, _, _)| *t == x.pc).copied();
                if dead {
                    let (_, d, l) = recorded.ok_or("unreached label")?;
                    self.registers(d);
                    self.locals = l;
                } else {
                    self.flush();
                }
                for _ in &closing {
                    self.out.push('}');
                }
                dead = false;
            }
            let mut opening: Vec<(usize, usize)> = targets
                .iter()
                .filter(|(s, _)| *s == x.pc)
                .copied()
                .collect();
            opening.sort_by_key(|e| std::cmp::Reverse(e.1));
            for (_, t) in &opening {
                self.out.push_str(&format!("{}:{{", label(*t)));
            }
            if dead {
                return Err(format!("unreachable code at pc {}", x.pc));
            }
            let mut record = |me: &mut Self, target: usize| {
                depth_at.push((target, me.stack.len(), me.locals));
            };
            match x.op {
                Opcode::Number => self.push(number(x.number)),
                Opcode::Bool => self.push(if x.args[0] != 0 { "!0" } else { "!1" }.into()),
                Opcode::Str => {
                    let s = self.plan.str(exact_plan::StrId(x.args[0] as u32));
                    self.push(serde_json::to_string(s).unwrap())
                }
                Opcode::None | Opcode::Unit => self.push("null".into()),
                Opcode::Some | Opcode::Unwrap => {}
                Opcode::LoadSlot => {
                    let slot = &self.plan.slots[x.args[0] as usize];
                    if slot.owner.is_some() {
                        let rows = self
                            .scope
                            .rows
                            .clone()
                            .ok_or("a row slot read with no row in force")?;
                        self.push(format!("{rows}[{}]()", x.args[0]))
                    } else {
                        self.push(format!("s_{}()", x.args[0]))
                    }
                }
                Opcode::LoadDerive => self.push(format!("d_{}()", x.args[0])),
                Opcode::LoadResource => self.push(format!("r_{}()", x.args[0])),
                Opcode::LoadParam => self.push(format!("p{}", x.args[0])),
                Opcode::LoadItem | Opcode::LoadIndex | Opcode::LoadBound => {
                    let d = x.args[0] as usize;
                    let f = self
                        .scope
                        .frames
                        .len()
                        .checked_sub(d + 1)
                        .and_then(|i| self.scope.frames.get(i))
                        .ok_or("a frame read outside its region")?;
                    let g = match x.op {
                        Opcode::LoadItem => &f.item,
                        Opcode::LoadIndex => &f.index,
                        _ => &f.bound,
                    }
                    .clone()
                    .ok_or("a frame read the frame does not hold")?;
                    self.push(format!("{g}()"))
                }
                Opcode::Field => {
                    let e = self.pop()?;
                    self.push(format!("{e}[{}]", x.args[0]))
                }
                Opcode::Record => {
                    let n = self.plan.types[x.args[0] as usize].fields.len as usize;
                    let f = self.popn(n)?;
                    self.push(format!("[{}]", f.join(",")))
                }
                Opcode::List => {
                    let f = self.popn(x.args[0] as usize)?;
                    self.push(format!("[{}]", f.join(",")))
                }
                Opcode::Add | Opcode::Concat => self.bin("+")?,
                Opcode::Sub => self.bin("-")?,
                Opcode::Mul => self.bin("*")?,
                Opcode::Div => self.bin("/")?,
                Opcode::Rem => self.bin("%")?,
                Opcode::Lt => self.bin("<")?,
                Opcode::Le => self.bin("<=")?,
                Opcode::Gt => self.bin(">")?,
                Opcode::Ge => self.bin(">=")?,
                Opcode::Eq | Opcode::Ne => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let not = x.op == Opcode::Ne;
                    if primitive(&a) || primitive(&b) {
                        self.push(format!("({a}{}{b})", if not { "!==" } else { "===" }))
                    } else {
                        let eq = self.uses.rt("equal");
                        self.push(format!("{}{eq}({a},{b})", if not { "!" } else { "" }))
                    }
                }
                Opcode::Not => {
                    let a = self.pop()?;
                    self.push(format!("!{a}"))
                }
                Opcode::Neg => {
                    let a = self.pop()?;
                    self.push(format!("(-{a})"))
                }
                Opcode::Call => {
                    let f = Stdlib::from_wire(x.args[0] as u8).ok_or("unknown stdlib entry")?;
                    let args = self.popn(f.arity())?;
                    let name = self.uses.rt(&format!("x_{}", f.name()));
                    self.push(format!("{name}({})", args.join(",")))
                }
                Opcode::Jump => {
                    self.flush();
                    record(self, x.args[0] as usize);
                    self.out
                        .push_str(&format!("break {};", label(x.args[0] as usize)));
                    dead = true;
                }
                Opcode::JumpIfFalse => {
                    let c = self.pop()?;
                    self.flush();
                    record(self, x.args[0] as usize);
                    self.out
                        .push_str(&format!("if(!{c})break {};", label(x.args[0] as usize)));
                }
                Opcode::JumpIfNone => {
                    self.flush();
                    record(self, x.args[0] as usize);
                    let top = self.stack.last().ok_or("stack underflow")?.clone();
                    self.out.push_str(&format!(
                        "if({top}==null)break {};",
                        label(x.args[0] as usize)
                    ));
                }
                Opcode::Pop => {
                    self.pop()?;
                }
                Opcode::BindLocal => {
                    let v = self.pop()?;
                    self.flush();
                    let k = self.locals;
                    self.locals += 1;
                    self.max_local = Some(self.max_local.map_or(k, |m| m.max(k)));
                    self.out.push_str(&format!("l{k}={v};"));
                }
                Opcode::LoadLocal => self.push(format!("l{}", x.args[0])),
                Opcode::DropLocal => self.locals = self.locals.saturating_sub(1),
                Opcode::Map | Opcode::Filter => {
                    let end = x.args[0] as usize;
                    let list = self.pop()?;
                    let mut j = i + 1;
                    while j < to && ins[j].pc < end {
                        j += 1;
                    }
                    let base = self.locals;
                    let mut inner = Translator {
                        plan: self.plan,
                        scope: self.scope,
                        uses: self.uses,
                        out: String::new(),
                        stack: Vec::new(),
                        max_depth: 0,
                        locals: base + 2,
                        max_local: None,
                        base_local: base + 2,
                    };
                    let body = inner.range(ins, i + 1, j)?;
                    let body = match body {
                        Body::Expr(e) if inner.out.is_empty() => paren_object(&e),
                        Body::Expr(e) => {
                            inner.out.push_str(&format!("return {e};"));
                            format!("{{{}{}}}", inner.decls(), inner.out)
                        }
                        Body::Stmts => format!("{{{}{}}}", inner.decls(), inner.out),
                    };
                    let f = format!("(l{base},l{})=>{body}", base + 1);
                    let m = if x.op == Opcode::Map { "map" } else { "filter" };
                    self.push(format!("{list}.{m}({f})"));
                    i = j;
                    continue;
                }
                Opcode::StoreSlot => {
                    if !self.scope.action {
                        return Err("a write outside an action".into());
                    }
                    let v = self.pop()?;
                    self.flush();
                    let w = self.uses.rt("W");
                    let target = if self.plan.slots[x.args[0] as usize].owner.is_some() {
                        let rows = self
                            .scope
                            .rows
                            .clone()
                            .ok_or("a row slot write with no row in force")?;
                        format!("{rows}[{}]", x.args[0])
                    } else {
                        format!("s_{}", x.args[0])
                    };
                    self.out.push_str(&format!("{w}({target},{v});"));
                }
                Opcode::Command => {
                    if !self.scope.action {
                        return Err("a command outside an action".into());
                    }
                    let name = self
                        .plan
                        .str(exact_plan::StrId(x.args[0] as u32))
                        .to_string();
                    let args = self.popn(x.args[1] as usize)?;
                    self.flush();
                    let c = self.uses.rt("C");
                    self.out.push_str(&format!(
                        "{c}({},[{}]);",
                        serde_json::to_string(&name).unwrap(),
                        args.join(",")
                    ));
                }
                Opcode::Refresh => {
                    self.flush();
                    let r = self.uses.rt("R");
                    self.out.push_str(&format!("{r}(r_{});", x.args[0]));
                }
                Opcode::PendingResource => self.push(format!("r_{}.p()", x.args[0])),
                Opcode::FailedResource => self.push(format!("r_{}.f()", x.args[0])),
                Opcode::Return => {
                    match self.stack.pop() {
                        Some(e) => {
                            // Statements already written (writes, commands) run first.
                            if self.out.is_empty() && i + 1 == to {
                                return Ok(Body::Expr(e));
                            }
                            self.out.push_str(&format!("return {e};"));
                        }
                        None => self.out.push_str("return;"),
                    }
                    dead = true;
                }
                Opcode::Send => {
                    if !self.scope.action {
                        return Err("a send outside an action".into());
                    }
                    let source = self
                        .plan
                        .str(exact_plan::StrId(x.args[1] as u32))
                        .to_string();
                    let args = self.popn(x.args[2] as usize)?;
                    self.flush();
                    let m = self.uses.rt("M");
                    self.out.push_str(&format!(
                        "{m}(m_{},{},[{}]);",
                        x.args[0],
                        serde_json::to_string(&source).unwrap(),
                        args.join(",")
                    ));
                }
                Opcode::PendingMutation => self.push(format!("m_{}.p()", x.args[0])),
                // A native module's props (LLP 1024 D1): the pairs as one
                // JSON object of strings, as `stdlib::native_props` writes it.
                Opcode::NativeProps => {
                    let f = self.popn(x.args[0] as usize * 2)?;
                    let np = self.uses.rt("NP");
                    self.push(format!("{np}([{}])", f.join(",")))
                }
            }
            i += 1;
        }
        // A callback body ends at its `end`: its value is the stack's top.
        if end_pc != usize::MAX || to == ins.len() {
            // Close blocks that end exactly at the range's end.
            let closing = targets.iter().filter(|(_, t)| *t == end_pc).count();
            if closing > 0 {
                if dead {
                    let (_, d, l) = depth_at
                        .iter()
                        .find(|(t, _, _)| *t == end_pc)
                        .copied()
                        .ok_or("unreached label")?;
                    self.registers(d);
                    self.locals = l;
                } else {
                    self.flush();
                }
                for _ in 0..closing {
                    self.out.push('}');
                }
                dead = false;
            }
        }
        if dead {
            return Ok(Body::Stmts);
        }
        match self.stack.pop() {
            Some(e) if self.out.is_empty() => Ok(Body::Expr(e)),
            Some(e) => {
                self.out.push_str(&format!("return {e};"));
                Ok(Body::Stmts)
            }
            None => Ok(Body::Stmts),
        }
    }

    fn bin(&mut self, op: &str) -> Result<(), String> {
        let b = self.pop()?;
        let a = self.pop()?;
        self.push(format!("({a}{op}{b})"));
        Ok(())
    }
}

/// A number as a JavaScript literal.
pub fn number(n: f64) -> String {
    if n.is_nan() {
        "NaN".into()
    } else if n.is_infinite() {
        if n > 0.0 { "Infinity" } else { "(-Infinity)" }.into()
    } else if n == 0.0 && n.is_sign_negative() {
        "(-0)".into()
    } else if n < 0.0 {
        format!("({n})")
    } else {
        format!("{n}")
    }
}

/// Whether a translated expression is a primitive literal, so `===` is the
/// VM's equality for it.
fn primitive(e: &str) -> bool {
    e.starts_with('"')
        || e == "!0"
        || e == "!1"
        || e.parse::<f64>().is_ok()
        || e.starts_with("(-") && e[2..].trim_end_matches(')').parse::<f64>().is_ok()
}

#[cfg(test)]
mod tests {
    use exact_plan::Stdlib;

    /// Every roster entry an `Opcode::Call` can name has its `x_` export in
    /// rt.js (or roster.js, which it re-exports); `at`, `formatDate` and `formatNumber` compiled and then failed
    /// the bundle for want of one (an app's diary, 2026-10-04). `map` and
    /// `filter` are opcodes with a callback body, never a call.
    #[test]
    fn every_called_roster_entry_has_a_runtime_export() {
        let rt = [
            include_str!("../rt.js"),
            include_str!("../roster.js"),
            include_str!("../format.js"),
        ]
        .concat();
        let exported = |name: &str| {
            rt.match_indices(name).any(|(at, _)| {
                !rt[at + name.len()..]
                    .starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$')
            })
        };
        let missing: Vec<&str> = Stdlib::ALL
            .iter()
            .map(|f| f.name())
            .filter(|name| !matches!(*name, "map" | "filter"))
            .filter(|name| !exported(&format!("x_{name}")))
            .collect();
        assert!(missing.is_empty(), "rt.js lacks x_ exports for {missing:?}");
    }
}
