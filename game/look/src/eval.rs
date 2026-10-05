//! Running a checked look inside `Game::present`: rows read through `Data` by
//! name, resources cached by storage version, rows written through `Present`.
//! A top-level rule that reads no time is kept while what it read is unchanged.
use crate::check::{Case, Ex, Program, Row, Slot, St, B};
use crate::syntax::Op;
use crate::value::{Build, Val};
use crate::Externs;
use exact_game::{math, Entity, Offset, Opacity, Present, Quat, Transform, Vec3};
use std::rc::Rc;

type R<T> = Result<T, String>;

/// A row a rule wrote, kept to write again while its inputs are unchanged.
#[derive(Clone, Copy, Debug)]
enum Written {
    Offset(Transform),
    Opacity(f32),
}

struct Kept {
    versions: Vec<Option<(u64, u64)>>,
    globals: Vec<Val>,
    rows: Vec<(Entity, Written)>,
}

/// What a look keeps between presents: resources by version, rules' rows.
#[derive(Default)]
pub struct Cache {
    rules: Vec<Option<Kept>>,
    resources: Vec<Option<((u64, u64), Val)>>,
    /// Counts from the last present.
    pub stats: Stats,
    /// Microseconds each rule took in the last present (0 when kept).
    pub rule_us: Vec<f64>,
}

/// What the last present did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Rules evaluated.
    pub run: u32,
    /// Rules whose rows were written again from the cache.
    pub kept: u32,
    /// Rows written.
    pub rows: u32,
    /// Component rows read through `Data`.
    pub reads: u32,
}

enum Flow {
    Go,
    Stop,
}

struct Ev<'a, 'w> {
    p: &'a mut Present<'w>,
    prog: &'a Program,
    externs: &'a Externs,
    globals: Vec<Val>,
    stack: Vec<Val>,
    base: usize,
    resources: &'a mut Vec<Option<((u64, u64), Val)>>,
    sink: Vec<(Entity, Written)>,
    stats: Stats,
}

pub(crate) fn present(
    prog: &Program,
    externs: &Externs,
    p: &mut Present<'_>,
    cache: &mut Cache,
    errors: &mut Vec<String>,
) {
    cache.rules.resize_with(prog.rules.len(), || None);
    cache.resources.resize_with(prog.resources.len(), || None);
    let mut rules = std::mem::take(&mut cache.rules);
    let mut timing = Vec::new();
    let mut ev = Ev {
        p,
        prog,
        externs,
        globals: vec![Val::None; prog.globals as usize],
        stack: vec![Val::None; prog.frame as usize],
        base: 0,
        resources: &mut cache.resources,
        sink: Vec::new(),
        stats: Stats::default(),
    };
    for (st, rule) in &prog.body {
        let Some(rule) = rule else {
            match ev.exec(st) {
                Ok(Flow::Go) => continue,
                Ok(Flow::Stop) => break,
                Err(e) => {
                    errors.push(e);
                    break;
                }
            }
        };
        let r = &prog.rules[*rule as usize];
        let versions: Vec<Option<(u64, u64)>> = r
            .reads
            .components
            .iter()
            .map(|&c| prog.components[c as usize])
            .chain(
                r.reads
                    .resources
                    .iter()
                    .map(|&c| prog.resources[c as usize]),
            )
            .map(|name| ev.p.version_of(name))
            .collect();
        let globals: Vec<Val> = r
            .reads
            .globals
            .iter()
            .map(|&g| ev.globals[g as usize].clone())
            .collect();
        let slot = &mut rules[*rule as usize];
        if let Some(kept) = slot.as_ref().filter(|_| r.cacheable()) {
            if kept.versions == versions
                && kept.globals.len() == globals.len()
                && kept.globals.iter().zip(&globals).all(|(a, b)| a.same(b))
            {
                for &(e, w) in &kept.rows {
                    ev.write(e, w);
                }
                ev.stats.kept += 1;
                continue;
            }
        }
        ev.sink.clear();
        ev.stats.run += 1;
        let watch = crate::Stopwatch::start();
        let outcome = ev.exec(st);
        timing.push((*rule, watch.us()));
        let ok = match outcome {
            Ok(_) => true,
            Err(e) => {
                errors.push(format!("{}:{}: {e}", r.at.line, r.at.col));
                false
            }
        };
        *slot = (ok && r.cacheable()).then(|| Kept {
            versions,
            globals,
            rows: std::mem::take(&mut ev.sink),
        });
    }
    let stats = ev.stats;
    cache.rules = rules;
    cache.stats = stats;
    cache.rule_us = vec![0.0; prog.rules.len()];
    for (rule, us) in timing {
        cache.rule_us[rule as usize] = us;
    }
}

fn num(v: &Val) -> R<f32> {
    match v {
        Val::Num(n) => Ok(*n),
        other => Err(format!("expected a number, found {other:?}")),
    }
}
fn v3(v: &Val) -> R<Vec3> {
    match v {
        Val::V3(n) => Ok(*n),
        other => Err(format!("expected a vec3, found {other:?}")),
    }
}
fn quat(v: &Val) -> R<Quat> {
    match v {
        Val::Q(q) => Ok(*q),
        other => Err(format!("expected a quat, found {other:?}")),
    }
}
fn ent(v: &Val) -> R<Entity> {
    match v {
        Val::Ent(e) => Ok(*e),
        Val::None => Err("an entity that is not there".into()),
        other => Err(format!("expected an entity, found {other:?}")),
    }
}
fn truth(v: &Val) -> R<bool> {
    match v {
        Val::Bool(b) => Ok(*b),
        other => Err(format!("expected a bool, found {other:?}")),
    }
}

impl Ev<'_, '_> {
    fn write(&mut self, e: Entity, w: Written) {
        let written = match w {
            Written::Offset(t) => self.p.insert(e, Offset(t)),
            Written::Opacity(o) => self.p.insert(e, Opacity(o)),
        };
        if written {
            self.stats.rows += 1;
            self.sink.push((e, w));
        }
    }
    fn set(&mut self, slot: Slot, v: Val) {
        match slot {
            Slot::Global(g) => self.globals[g as usize] = v,
            Slot::Local(l) => self.stack[self.base + l as usize] = v,
        }
    }
    fn block(&mut self, body: &[St]) -> R<Flow> {
        for st in body {
            if let Flow::Stop = self.exec(st)? {
                return Ok(Flow::Stop);
            }
        }
        Ok(Flow::Go)
    }
    fn exec(&mut self, st: &St) -> R<Flow> {
        match st {
            St::Let(slot, ex) => {
                let v = self.eval(ex)?;
                self.set(*slot, v);
            }
            St::Guard(ex) => {
                if !truth(&self.eval(ex)?)? {
                    return Ok(Flow::Stop);
                }
            }
            St::If(c, then, otherwise) => {
                let branch = if truth(&self.eval(c)?)? {
                    then
                } else {
                    otherwise
                };
                return self.block(branch);
            }
            St::Each {
                comp,
                ent,
                val,
                body,
            } => {
                let name = self.prog.components[*comp as usize];
                let mut all = Vec::new();
                self.p.visit_component(name, &mut |e| all.push(e));
                for e in all {
                    self.stack[self.base + *ent as usize] = Val::Ent(e);
                    if let Some(val) = val {
                        let row = self.row(*comp, e);
                        self.stack[self.base + *val as usize] = row;
                    }
                    self.block(body)?;
                }
            }
            St::For {
                index,
                item,
                list,
                body,
            } => {
                let Val::List(items) = self.eval(list)? else {
                    return Err("`for` over a value that is not a list".into());
                };
                for (k, v) in items.iter().enumerate() {
                    if let Some(index) = index {
                        self.stack[self.base + *index as usize] = Val::Num(k as f32);
                    }
                    self.stack[self.base + *item as usize] = v.clone();
                    self.block(body)?;
                }
            }
            St::Range { slot, lo, hi, body } => {
                let (lo, hi) = (num(&self.eval(lo)?)?, num(&self.eval(hi)?)?);
                let mut k = lo;
                while k < hi {
                    self.stack[self.base + *slot as usize] = Val::Num(k);
                    self.block(body)?;
                    k += 1.0;
                }
            }
            St::Emit(row, e, args, _) => {
                let e = ent(&self.eval(e)?)?;
                let w = match row {
                    Row::Offset {
                        position,
                        rotation,
                        scale,
                    } => {
                        let mut t = Transform::default();
                        let mut args = args.iter();
                        if *position {
                            t.position = v3(&self.eval(args.next().unwrap())?)?;
                        }
                        if *rotation {
                            t.rotation = quat(&self.eval(args.next().unwrap())?)?;
                        }
                        if *scale {
                            t.scale = v3(&self.eval(args.next().unwrap())?)?;
                        }
                        Written::Offset(t)
                    }
                    Row::Opacity => Written::Opacity(num(&self.eval(&args[0])?)?),
                };
                self.write(e, w);
            }
        }
        Ok(Flow::Go)
    }

    fn row(&mut self, comp: u16, e: Entity) -> Val {
        self.stats.reads += 1;
        let name = self.prog.components[comp as usize];
        // The engine's own types are read typed; the game's through `Data`.
        if name == "Transform" {
            return self.p.get::<Transform>(e).map_or(Val::None, |t| {
                Val::Rec(Rc::new(vec![
                    Val::V3(t.position),
                    Val::Q(t.rotation),
                    Val::V3(t.scale),
                ]))
            });
        }
        let mut b = Build::default();
        if self.p.write_component(name, e, &mut b) {
            b.finish()
        } else {
            Val::None
        }
    }
    fn path(&mut self, comp: u16, e: Entity, path: &[u16]) -> R<Val> {
        if self.prog.components[comp as usize] == "Transform" {
            self.stats.reads += 1;
            let Some(t) = self.p.get::<Transform>(e) else {
                return Err("a Transform that is not there".into());
            };
            return Ok(match path {
                [0] => Val::V3(t.position),
                [1] => Val::Q(t.rotation),
                [2] => Val::V3(t.scale),
                _ => Val::None,
            });
        }
        let mut v = self.row(comp, e);
        for &i in path {
            v = match v {
                Val::Rec(fields) => fields.get(i as usize).cloned().unwrap_or_default(),
                Val::None => return Err("a field of a row that is not there".into()),
                other => return Err(format!("a field of {other:?}")),
            };
        }
        Ok(v)
    }

    fn eval(&mut self, ex: &Ex) -> R<Val> {
        Ok(match ex {
            Ex::Lit(v) => v.clone(),
            Ex::Get(Slot::Global(g)) => self.globals[*g as usize].clone(),
            Ex::Get(Slot::Local(l)) => self.stack[self.base + *l as usize].clone(),
            Ex::Field(r, i) => match self.eval(r)? {
                Val::Rec(fields) => fields.get(*i as usize).cloned().unwrap_or_default(),
                Val::None => return Err("a field of a value that is not there".into()),
                other => return Err(format!("a field of {other:?}")),
            },
            Ex::Axis(r, a) => match self.eval(r)? {
                Val::V3(v) => Val::Num(v[*a as usize]),
                Val::Q(q) => Val::Num(q.to_array()[*a as usize]),
                other => return Err(format!("an axis of {other:?}")),
            },
            Ex::Comp(r, c) => {
                let e = ent(&self.eval(r)?)?;
                self.row(*c, e)
            }
            Ex::CompPath(r, c, path) => {
                let e = ent(&self.eval(r)?)?;
                self.path(*c, e, path)?
            }
            Ex::Has(r, c) => {
                let e = ent(&self.eval(r)?)?;
                Val::Bool(self.p.has_component(self.prog.components[*c as usize], e))
            }
            Ex::Res(id) => {
                let name = self.prog.resources[*id as usize];
                let version = self.p.version_of(name);
                match (&self.resources[*id as usize], version) {
                    (Some((kept, v)), Some(now)) if *kept == now => v.clone(),
                    _ => {
                        let mut b = Build::default();
                        if !self.p.write_resource(name, &mut b) {
                            return Ok(Val::None);
                        }
                        let v = b.finish();
                        if let Some(now) = version {
                            self.resources[*id as usize] = Some((now, v.clone()));
                        }
                        v
                    }
                }
            }
            Ex::Named(n) => self.p.named(n).map_or(Val::None, Val::Ent),
            Ex::Index(l, i) => {
                let l = self.eval(l)?;
                let i = num(&self.eval(i)?)?;
                if !(i >= 0.0 && i.fract() == 0.0) {
                    return Err(format!("index {i}"));
                }
                match l {
                    Val::Col(c) => Val::Num(
                        c.get(i as usize)
                            .ok_or(format!("index {i} of {}", c.len()))?,
                    ),
                    Val::List(v) => v
                        .get(i as usize)
                        .cloned()
                        .ok_or(format!("index {i} of {}", v.len()))?,
                    other => return Err(format!("indexing {other:?}")),
                }
            }
            Ex::Un(Op::Neg, a) => match self.eval(a)? {
                Val::Num(n) => Val::Num(-n),
                Val::V3(v) => Val::V3(-v),
                other => return Err(format!("-{other:?}")),
            },
            Ex::Un(_, a) => Val::Bool(!truth(&self.eval(a)?)?),
            Ex::Bin(Op::And, a, b) => Val::Bool(truth(&self.eval(a)?)? && truth(&self.eval(b)?)?),
            Ex::Bin(Op::Or, a, b) => Val::Bool(truth(&self.eval(a)?)? || truth(&self.eval(b)?)?),
            Ex::Bin(op, a, b) => {
                let (a, b) = (self.eval(a)?, self.eval(b)?);
                binary(*op, &a, &b)?
            }
            Ex::Cond(c, a, b) => {
                if truth(&self.eval(c)?)? {
                    self.eval(a)?
                } else {
                    self.eval(b)?
                }
            }
            Ex::Is(a, arm) => match self.eval(a)? {
                Val::Arm(i) => Val::Bool(i == *arm),
                other => return Err(format!("`is` on {other:?}")),
            },
            Ex::Builtin(b, args) => {
                // At most three arguments: no allocation per call.
                let mut vals = [Val::None, Val::None, Val::None];
                for (slot, a) in vals.iter_mut().zip(args) {
                    *slot = self.eval(a)?;
                }
                self.builtin(*b, &vals[..args.len()])?
            }
            Ex::Extern(i, args) => {
                let mut nums = [0f32; 8];
                for (k, a) in args.iter().enumerate() {
                    nums[k] = num(&self.eval(a)?)?;
                }
                Val::Num(self.externs.call(*i as usize, &nums[..args.len()]))
            }
            Ex::Call(f, args) => {
                let prog = self.prog;
                let fun = &prog.fns[*f as usize];
                let base = self.stack.len();
                self.stack.resize(base + fun.frame as usize, Val::None);
                for (k, a) in args.iter().enumerate() {
                    let v = self.eval(a)?;
                    self.stack[base + k] = v;
                }
                let saved = std::mem::replace(&mut self.base, base);
                let out = (|| {
                    for (slot, ex) in &fun.lets {
                        let v = self.eval(ex)?;
                        self.stack[self.base + *slot as usize] = v;
                    }
                    self.eval(&fun.body)
                })();
                self.base = saved;
                self.stack.truncate(base);
                out?
            }
            Ex::Rec(fields) => {
                let mut vals = Vec::with_capacity(fields.len());
                for f in fields {
                    vals.push(self.eval(f)?);
                }
                Val::Rec(Rc::new(vals))
            }
            Ex::MatchArm(s, arms) => {
                let Val::Arm(i) = self.eval(s)? else {
                    return Err("match over a value that is not an enum".into());
                };
                match arms.iter().find(|(a, _)| a.is_none_or(|a| a == i)) {
                    Some((_, body)) => self.eval(body)?,
                    None => return Err(format!("no case for arm #{i}")),
                }
            }
            Ex::MatchEnt(s, arms) => {
                let e = ent(&self.eval(s)?)?;
                for (case, body) in arms {
                    let hit = match case {
                        Case::Any => true,
                        Case::Same(x) => matches!(self.eval(x)?, Val::Ent(o) if o == e),
                        Case::Has(c, binder) => {
                            let has = self.p.has_component(self.prog.components[*c as usize], e);
                            if let (true, Some(slot)) = (has, binder) {
                                let row = self.row(*c, e);
                                self.stack[self.base + *slot as usize] = row;
                            }
                            has
                        }
                    };
                    if hit {
                        return self.eval(body);
                    }
                }
                return Err("no case matched".into());
            }
        })
    }

    fn builtin(&mut self, b: B, a: &[Val]) -> R<Val> {
        Ok(match b {
            B::Seconds => Val::Num(self.p.seconds() as f32),
            B::Tick => Val::Num(self.p.tick() as f32),
            B::Some => Val::Bool(!matches!(a[0], Val::None)),
            B::WorldPosition => match &a[0] {
                Val::Ent(e) => self
                    .p
                    .global(*e)
                    .map_or(Val::None, |g| Val::V3(Vec3::from(g.translation))),
                _ => Val::None,
            },
            B::Vec => Val::V3(Vec3::new(num(&a[0])?, num(&a[1])?, num(&a[2])?)),
            B::Xz => {
                let v = v3(&a[0])?;
                Val::V3(Vec3::new(v.x, 0.0, v.z))
            }
            B::Length => Val::Num(v3(&a[0])?.length()),
            B::Dot => Val::Num(v3(&a[0])?.dot(v3(&a[1])?)),
            B::Normalize => Val::V3(v3(&a[0])?.normalize_or_zero()),
            B::Min => Val::Num(num(&a[0])?.min(num(&a[1])?)),
            B::Max => Val::Num(num(&a[0])?.max(num(&a[1])?)),
            B::Abs => Val::Num(num(&a[0])?.abs()),
            B::Floor => Val::Num(math::floor(num(&a[0])?)),
            B::Ceil => Val::Num(num(&a[0])?.ceil()),
            B::Sqrt => Val::Num(num(&a[0])?.sqrt()),
            B::Sin => Val::Num(math::sin(num(&a[0])?)),
            B::Cos => Val::Num(math::cos(num(&a[0])?)),
            B::Atan2 => Val::Num(math::atan2(num(&a[0])?, num(&a[1])?)),
            B::Yaw => Val::Q(Quat::from_rotation_y(num(&a[0])?)),
            B::Pitch => Val::Q(Quat::from_rotation_x(num(&a[0])?)),
            B::Roll => Val::Q(Quat::from_rotation_z(num(&a[0])?)),
            B::Identity => Val::Q(Quat::IDENTITY),
            B::Inverse => Val::Q(quat(&a[0])?.inverse()),
            B::Len => Val::Num(match &a[0] {
                Val::List(v) => v.len() as f32,
                Val::Col(c) => c.len() as f32,
                other => return Err(format!("len of {other:?}")),
            }),
        })
    }
}

fn binary(op: Op, a: &Val, b: &Val) -> R<Val> {
    use Val::*;
    Ok(match (op, a, b) {
        (Op::Add, Num(x), Num(y)) => Num(x + y),
        (Op::Sub, Num(x), Num(y)) => Num(x - y),
        (Op::Mul, Num(x), Num(y)) => Num(x * y),
        (Op::Div, Num(x), Num(y)) => Num(x / y),
        (Op::Rem, Num(x), Num(y)) => Num(x % y),
        (Op::Add, V3(x), V3(y)) => V3(*x + *y),
        (Op::Sub, V3(x), V3(y)) => V3(*x - *y),
        (Op::Mul, V3(x), Num(y)) => V3(*x * *y),
        (Op::Mul, Num(x), V3(y)) => V3(*x * *y),
        (Op::Div, V3(x), Num(y)) => V3(*x / *y),
        (Op::Mul, Q(x), Q(y)) => Q(*x * *y),
        (Op::Mul, Q(x), V3(y)) => V3(*x * *y),
        (Op::Lt, Num(x), Num(y)) => Bool(x < y),
        (Op::Le, Num(x), Num(y)) => Bool(x <= y),
        (Op::Gt, Num(x), Num(y)) => Bool(x > y),
        (Op::Ge, Num(x), Num(y)) => Bool(x >= y),
        (Op::Eq | Op::Ne, x, y) => {
            let same = match (x, y) {
                (Num(x), Num(y)) => x == y,
                (Bool(x), Bool(y)) => x == y,
                (Ent(x), Ent(y)) => x == y,
                (Arm(x), Arm(y)) => x == y,
                (Str(x), Str(y)) => x == y,
                (None, None) => true,
                _ => false,
            };
            Bool(same == (op == Op::Eq))
        }
        _ => return Err(format!("{a:?} {op:?} {b:?}")),
    })
}
