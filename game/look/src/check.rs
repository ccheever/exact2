//! The checker: a look's names resolved against the game's registered types
//! (their `Default` shapes; enum arms by a strict probe read), the closed row
//! vocabulary and the game's externs, then lowered to the IR `eval` runs. It
//! also records what each rule reads, for caching and for the lowering report.
use crate::syntax::{error, Error, Expr, File, Op, Span, Stmt};
use crate::value::{Shape, Tree, Val};
use crate::Externs;
use exact_game::Registered;
use std::collections::BTreeSet;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Ty {
    Num,
    Bool,
    Str,
    Ent,
    V3,
    Quat,
    Col,
    Arm(Rc<ArmOf>),
    List(Box<Ty>),
    Rec(Rc<Vec<(String, Ty)>>),
    Unknown,
}
/// Where an enum field lives, so an arm name can be probed and resolved.
#[derive(Debug, PartialEq)]
pub(crate) struct ArmOf {
    owner: usize,
    path: Vec<String>,
    default: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    Global(u16),
    Local(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum B {
    Seconds,
    Tick,
    Some,
    WorldPosition,
    Vec,
    Xz,
    Length,
    Dot,
    Normalize,
    Min,
    Max,
    Abs,
    Floor,
    Ceil,
    Sqrt,
    Sin,
    Cos,
    Atan2,
    Yaw,
    Pitch,
    Roll,
    Identity,
    Inverse,
    Len,
}
const BUILTINS: [(&str, B, usize); 24] = [
    ("seconds", B::Seconds, 0),
    ("tick", B::Tick, 0),
    ("some", B::Some, 1),
    ("world_position", B::WorldPosition, 1),
    ("vec", B::Vec, 3),
    ("xz", B::Xz, 1),
    ("length", B::Length, 1),
    ("dot", B::Dot, 2),
    ("normalize", B::Normalize, 1),
    ("min", B::Min, 2),
    ("max", B::Max, 2),
    ("abs", B::Abs, 1),
    ("floor", B::Floor, 1),
    ("ceil", B::Ceil, 1),
    ("sqrt", B::Sqrt, 1),
    ("sin", B::Sin, 1),
    ("cos", B::Cos, 1),
    ("atan2", B::Atan2, 2),
    ("yaw", B::Yaw, 1),
    ("pitch", B::Pitch, 1),
    ("roll", B::Roll, 1),
    ("identity", B::Identity, 0),
    ("inverse", B::Inverse, 1),
    ("len", B::Len, 1),
];
const MATH: [(&str, f32); 3] = [
    ("PI", std::f32::consts::PI),
    ("TAU", std::f32::consts::TAU),
    ("FRAC_PI_2", std::f32::consts::FRAC_PI_2),
];

#[derive(Clone, Debug)]
pub(crate) enum Ex {
    Lit(Val),
    Get(Slot),
    Field(Box<Ex>, u16),
    Axis(Box<Ex>, u8),
    Comp(Box<Ex>, u16),
    /// A field path into a component row (`e.Transform.position`): read
    /// without building the row where the executor knows the type.
    CompPath(Box<Ex>, u16, Vec<u16>),
    Res(u16),
    Named(Rc<str>),
    Index(Box<Ex>, Box<Ex>),
    Un(Op, Box<Ex>),
    Bin(Op, Box<Ex>, Box<Ex>),
    Cond(Box<Ex>, Box<Ex>, Box<Ex>),
    Is(Box<Ex>, u32),
    Has(Box<Ex>, u16),
    Builtin(B, Vec<Ex>),
    Extern(u16, Vec<Ex>),
    Call(u16, Vec<Ex>),
    Rec(Vec<Ex>),
    MatchEnt(Box<Ex>, Vec<(Case, Ex)>),
    MatchArm(Box<Ex>, Vec<(Option<u32>, Ex)>),
}
#[derive(Clone, Debug)]
pub(crate) enum Case {
    Same(Ex),
    Has(u16, Option<u16>),
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Row {
    Offset {
        position: bool,
        rotation: bool,
        scale: bool,
    },
    Opacity,
}

#[derive(Clone, Debug)]
pub(crate) enum St {
    Let(Slot, Ex),
    Guard(Ex),
    If(Ex, Vec<St>, Vec<St>),
    Each {
        comp: u16,
        ent: u16,
        val: Option<u16>,
        body: Vec<St>,
    },
    For {
        index: Option<u16>,
        item: u16,
        list: Ex,
        body: Vec<St>,
    },
    Range {
        slot: u16,
        lo: Ex,
        hi: Ex,
        body: Vec<St>,
    },
    Emit(Row, Ex, Vec<Ex>, Span),
}

/// What a piece of a look reads: the inputs a cached rule is keyed on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reads {
    pub time: bool,
    pub components: BTreeSet<u16>,
    pub resources: BTreeSet<u16>,
    pub globals: BTreeSet<u16>,
    /// Components read on an entity other than the one an `each` visits.
    pub elsewhere: bool,
    /// Reads no version covers (names, global poses): never kept.
    pub volatile: bool,
}
impl Reads {
    fn add(&mut self, other: &Reads) {
        self.time |= other.time;
        self.components.extend(&other.components);
        self.resources.extend(&other.resources);
        self.globals.extend(&other.globals);
        self.elsewhere |= other.elsewhere;
        self.volatile |= other.volatile;
    }
}

pub(crate) struct FnIr {
    pub frame: u16,
    pub lets: Vec<(u16, Ex)>,
    pub body: Ex,
    pub reads: Reads,
    /// The highest global its body reads (transitively), plus one.
    pub needs: u16,
}

/// A top-level statement that writes rows: the unit the cache keys and keeps.
pub struct Rule {
    pub at: Span,
    pub what: String,
    pub reads: Reads,
}
impl Rule {
    /// Whether its rows can be kept while what it read is unchanged.
    pub fn cacheable(&self) -> bool {
        !self.reads.time && !self.reads.volatile
    }
}

pub(crate) struct Program {
    pub components: Vec<&'static str>,
    pub resources: Vec<&'static str>,
    pub globals: u16,
    pub frame: u16,
    pub fns: Vec<FnIr>,
    /// Top-level statements; `Some(rule)` on those that write rows.
    pub body: Vec<(St, Option<u16>)>,
    pub rules: Vec<Rule>,
    pub global_reads: Vec<Reads>,
    pub global_names: Vec<String>,
    /// Per body-frame slot: whether its value reads the time.
    pub local_time: Vec<bool>,
}

struct Scope {
    names: Vec<(String, Slot, Ty)>,
}

struct Ck<'a> {
    types: &'a [Registered],
    shapes: Vec<Option<Shape>>,
    externs: &'a Externs,
    consts: Vec<(String, f32)>,
    errors: Vec<Error>,
    prog: Program,
    fn_names: Vec<(String, Vec<Ty>, Ty)>,
    scopes: Vec<Scope>,
    // The frame being filled: body or fn.
    frame: u16,
    in_fn: bool,
    global_ty: Vec<Ty>,
    globals: Vec<(String, u16)>,
    // Per local slot of the current frame: what its value read.
    local_reads: Vec<Reads>,
    // The entity slot (and its value slot) of the innermost `each`.
    own: Vec<(u16, Option<u16>)>,
}

pub(crate) fn check(
    file: &File,
    types: &[Registered],
    externs: &Externs,
) -> Result<Program, Vec<Error>> {
    let shapes = types
        .iter()
        .map(|t| {
            let mut tree = Tree::default();
            t.write_default(&mut tree);
            tree.finish()
        })
        .collect();
    let mut ck = Ck {
        types,
        shapes,
        externs,
        consts: Vec::new(),
        errors: Vec::new(),
        prog: Program {
            components: Vec::new(),
            resources: Vec::new(),
            globals: 0,
            frame: 0,
            fns: Vec::new(),
            body: Vec::new(),
            rules: Vec::new(),
            global_reads: Vec::new(),
            global_names: Vec::new(),
            local_time: Vec::new(),
        },
        fn_names: Vec::new(),
        scopes: vec![Scope { names: Vec::new() }],
        frame: 0,
        in_fn: false,
        global_ty: Vec::new(),
        globals: Vec::new(),
        local_reads: Vec::new(),
        own: Vec::new(),
    };
    for (name, e, at) in &file.consts {
        match ck.constant(e) {
            Ok(v) => ck.consts.push((name.clone(), v)),
            Err(m) => ck.errors.push(error(*at, m)),
        }
    }
    let mut next_fn = 0;
    for (k, stmt) in file.body.iter().enumerate() {
        while next_fn < file.fns.len() && file.fn_after[next_fn] <= k {
            ck.function(&file.fns[next_fn]);
            next_fn += 1;
        }
        ck.top(stmt);
    }
    while next_fn < file.fns.len() {
        ck.function(&file.fns[next_fn]);
        next_fn += 1;
    }
    if ck.errors.is_empty() {
        ck.prog.frame = ck.prog.frame.max(ck.frame);
        ck.prog.local_time = ck.local_reads.iter().map(|r| r.time).collect();
        Ok(ck.prog)
    } else {
        Err(ck.errors)
    }
}

impl Ck<'_> {
    fn constant(&self, e: &Expr) -> Result<f32, String> {
        Ok(match e {
            Expr::Num(v, _) => *v,
            Expr::Name(n, _) => self
                .consts
                .iter()
                .find(|(c, _)| c == n)
                .map(|(_, v)| *v)
                .or_else(|| MATH.iter().find(|(c, _)| c == n).map(|(_, v)| *v))
                .or_else(|| self.externs.constant_of(n))
                .ok_or(format!("`{n}` is not a constant"))?,
            Expr::Unary(Op::Neg, a, _) => -self.constant(a)?,
            Expr::Binary(op, a, b, _) => {
                let (a, b) = (self.constant(a)?, self.constant(b)?);
                match op {
                    Op::Add => a + b,
                    Op::Sub => a - b,
                    Op::Mul => a * b,
                    Op::Div => a / b,
                    _ => return Err("a constant is arithmetic on numbers".into()),
                }
            }
            _ => return Err("a constant is arithmetic on numbers".into()),
        })
    }

    fn type_index(&self, name: &str, component: bool) -> Option<usize> {
        self.types.iter().position(|t| {
            t.name == name && !t.presentation && if component { t.component } else { t.resource }
        })
    }
    fn component(&mut self, name: &str, at: Span) -> Option<(u16, Ty)> {
        let Some(t) = self.type_index(name, true) else {
            let msg = if self.types.iter().any(|t| t.name == name && t.presentation) {
                format!("`{name}` is a presentation component; a look writes it, never reads it")
            } else {
                format!("no component `{name}` is registered{}", self.suggest(name))
            };
            self.errors.push(error(at, msg));
            return None;
        };
        let id = intern(&mut self.prog.components, self.types[t].name);
        Some((id, self.shape_ty(t)))
    }
    fn suggest(&self, name: &str) -> String {
        let lower = name.to_lowercase();
        self.types
            .iter()
            .filter(|t| !t.presentation)
            .find(|t| {
                t.name.to_lowercase() == lower
                    || t.name
                        .to_lowercase()
                        .starts_with(&lower[..lower.len().min(3)])
            })
            .map_or(String::new(), |t| format!(" (did you mean `{}`?)", t.name))
    }
    fn shape_ty(&self, t: usize) -> Ty {
        match &self.shapes[t] {
            Some(s) => ty_of(s, t, &mut Vec::new()),
            None => Ty::Unknown,
        }
    }

    fn function(&mut self, f: &crate::syntax::FnDecl) {
        let saved = (
            std::mem::take(&mut self.local_reads),
            self.frame,
            std::mem::replace(&mut self.scopes, vec![Scope { names: Vec::new() }]),
        );
        self.prog.frame = self.prog.frame.max(self.frame);
        self.frame = 0;
        self.in_fn = true;
        let mut params = Vec::new();
        for (name, ty) in &f.params {
            let ty = match ty.as_str() {
                "number" => Ty::Num,
                "bool" => Ty::Bool,
                "string" => Ty::Str,
                "entity" => Ty::Ent,
                "vec3" => Ty::V3,
                "quat" => Ty::Quat,
                other => {
                    self.errors.push(error(
                        f.at,
                        format!(
                            "unknown type `{other}`: number, bool, string, entity, vec3 or quat"
                        ),
                    ));
                    Ty::Unknown
                }
            };
            let slot = self.local(name, ty.clone(), Reads::default());
            params.push((slot, ty));
        }
        let mut lets = Vec::new();
        for (name, e, _) in &f.lets {
            let (ex, ty, reads) = self.expr(e);
            let slot = self.local(name, ty, reads);
            lets.push((slot, ex));
        }
        let (body, ty, mut reads) = self.expr(&f.body);
        for r in &self.local_reads {
            reads.add(r);
        }
        let needs = reads.globals.iter().map(|g| g + 1).max().unwrap_or(0);
        let frame = self.frame;
        self.prog.fns.push(FnIr {
            frame,
            lets,
            body,
            reads,
            needs,
        });
        self.fn_names.push((
            f.name.clone(),
            params.into_iter().map(|(_, t)| t).collect(),
            ty,
        ));
        self.in_fn = false;
        (self.local_reads, self.frame, self.scopes) = saved;
    }

    fn top(&mut self, s: &Stmt) {
        match s {
            Stmt::Let(name, e, at) => {
                if self.globals.iter().any(|(n, _)| n == name) {
                    self.errors.push(error(
                        *at,
                        format!("`{name}` is already defined at the top level"),
                    ));
                }
                let (ex, ty, reads) = self.expr(e);
                let g = self.prog.globals;
                self.prog.globals += 1;
                self.global_ty.push(ty.clone());
                self.prog.global_reads.push(reads);
                let _ = ty;
                self.globals.push((name.clone(), g));
                self.prog.global_names.push(name.clone());
                self.prog.body.push((St::Let(Slot::Global(g), ex), None));
            }
            Stmt::Guard(..) => {
                let (st, _) = self.stmt(s);
                self.prog.body.push((st, None));
            }
            _ => {
                let (st, reads) = self.stmt(s);
                let id = self.prog.rules.len() as u16;
                let what = match s {
                    Stmt::Each { component, .. } => format!("each … in {component}"),
                    Stmt::For { .. } => "for".into(),
                    Stmt::If(..) => "if".into(),
                    Stmt::Emit { row, .. } => row.clone(),
                    _ => String::new(),
                };
                let at = match s {
                    Stmt::Each { at, .. }
                    | Stmt::For { at, .. }
                    | Stmt::If(_, _, _, at)
                    | Stmt::Emit { at, .. } => *at,
                    _ => Span::default(),
                };
                self.prog.rules.push(Rule { at, what, reads });
                self.prog.body.push((st, Some(id)));
            }
        }
    }

    fn local(&mut self, name: &str, ty: Ty, reads: Reads) -> u16 {
        let slot = self.frame;
        self.frame += 1;
        self.local_reads.push(reads);
        self.scopes
            .last_mut()
            .unwrap()
            .names
            .push((name.to_string(), Slot::Local(slot), ty));
        slot
    }
    fn block(&mut self, body: &[Stmt], reads: &mut Reads) -> Vec<St> {
        self.scopes.push(Scope { names: Vec::new() });
        let out = body
            .iter()
            .map(|s| {
                let (st, r) = self.stmt(s);
                reads.add(&r);
                st
            })
            .collect();
        self.scopes.pop();
        out
    }

    fn stmt(&mut self, s: &Stmt) -> (St, Reads) {
        let mut reads = Reads::default();
        let st = match s {
            Stmt::Let(name, e, _) => {
                let (ex, ty, r) = self.expr(e);
                reads.add(&r);
                let slot = self.local(name, ty, r);
                St::Let(Slot::Local(slot), ex)
            }
            Stmt::Guard(e, at) => {
                let ex = self.want(e, Ty::Bool, *at, &mut reads);
                St::Guard(ex)
            }
            Stmt::If(c, then, otherwise, at) => {
                let c = self.want(c, Ty::Bool, *at, &mut reads);
                let then = self.block(then, &mut reads);
                let otherwise = self.block(otherwise, &mut reads);
                St::If(c, then, otherwise)
            }
            Stmt::Each {
                entity,
                value,
                component,
                body,
                at,
            } => {
                let Some((comp, ty)) = self.component(component, *at) else {
                    return (St::Guard(Ex::Lit(Val::Bool(false))), reads);
                };
                reads.components.insert(comp);
                self.scopes.push(Scope { names: Vec::new() });
                let ent = self.local(entity, Ty::Ent, Reads::default());
                let val = value.as_ref().map(|v| {
                    let mut r = Reads::default();
                    r.components.insert(comp);
                    self.local(v, ty.clone(), r)
                });
                self.own.push((ent, val));
                let body = self.block(body, &mut reads);
                self.own.pop();
                self.scopes.pop();
                St::Each {
                    comp,
                    ent,
                    val,
                    body,
                }
            }
            Stmt::For {
                index,
                item,
                list,
                body,
                at,
            } => {
                if let Expr::Call(f, args, _) = list {
                    if f == "range" && args.len() == 2 && index.is_none() {
                        let lo = self.want(&args[0], Ty::Num, *at, &mut reads);
                        let hi = self.want(&args[1], Ty::Num, *at, &mut reads);
                        self.scopes.push(Scope { names: Vec::new() });
                        let slot = self.local(item, Ty::Num, reads.clone());
                        let body = self.block(body, &mut reads);
                        self.scopes.pop();
                        return (St::Range { slot, lo, hi, body }, reads);
                    }
                }
                let (list, ty, r) = self.expr(list);
                reads.add(&r);
                let elem = match ty {
                    Ty::List(t) => *t,
                    Ty::Unknown => Ty::Unknown,
                    other => {
                        self.errors.push(error(
                            *at,
                            format!("`for` walks a list, not {}", name(&other)),
                        ));
                        Ty::Unknown
                    }
                };
                self.scopes.push(Scope { names: Vec::new() });
                let index = index
                    .as_ref()
                    .map(|i| self.local(i, Ty::Num, Reads::default()));
                let item = self.local(item, elem, r);
                let body = self.block(body, &mut reads);
                self.scopes.pop();
                St::For {
                    index,
                    item,
                    list,
                    body,
                }
            }
            Stmt::Emit {
                row,
                entity,
                args,
                at,
            } => {
                let e = self.want(entity, Ty::Ent, *at, &mut reads);
                let mut out = Vec::new();
                let row = match row.as_str() {
                    "Offset" => {
                        let mut has = [false; 3];
                        let mut slots = vec![None, None, None];
                        for (name, a) in args {
                            let (k, ty) = match name.as_deref() {
                                Some("position") => (0, Ty::V3),
                                Some("rotation") => (1, Ty::Quat),
                                Some("scale") => (2, Ty::V3),
                                _ => {
                                    self.errors.push(error(
                                        a.span(),
                                        "Offset takes position=, rotation= and scale=",
                                    ));
                                    continue;
                                }
                            };
                            has[k] = true;
                            slots[k] = Some(self.want(a, ty, a.span(), &mut reads));
                        }
                        out.extend(slots.into_iter().flatten());
                        Row::Offset {
                            position: has[0],
                            rotation: has[1],
                            scale: has[2],
                        }
                    }
                    "Opacity" => {
                        match args.as_slice() {
                            [(None, a)] => out.push(self.want(a, Ty::Num, a.span(), &mut reads)),
                            _ => self
                                .errors
                                .push(error(*at, "Opacity takes one number: Opacity(e, 0.3)")),
                        }
                        Row::Opacity
                    }
                    other => {
                        let known = self.types.iter().any(|t| t.name == other && t.presentation);
                        self.errors.push(error(
                            *at,
                            if known {
                                format!("`{other}` is a presentation component this look cannot write yet (Offset, Opacity)")
                            } else {
                                format!("`{other}` is not a row a look writes (Offset, Opacity)")
                            },
                        ));
                        Row::Opacity
                    }
                };
                St::Emit(row, e, out, *at)
            }
        };
        (st, reads)
    }

    fn want(&mut self, e: &Expr, want: Ty, at: Span, reads: &mut Reads) -> Ex {
        let (ex, ty, r) = self.expr(e);
        reads.add(&r);
        if ty != want && ty != Ty::Unknown {
            self.errors.push(error(
                at,
                format!("expected {}, found {}", name(&want), name(&ty)),
            ));
        }
        ex
    }

    fn lookup(&self, n: &str) -> Option<(Slot, Ty)> {
        for scope in self.scopes.iter().rev() {
            if let Some((_, slot, ty)) = scope.names.iter().rev().find(|(m, _, _)| m == n) {
                return Some((*slot, ty.clone()));
            }
        }
        // The globals declared above this line (a fn's, above the fn).
        self.globals
            .iter()
            .rev()
            .find(|(m, _)| m == n)
            .map(|(_, g)| (Slot::Global(*g), self.global_ty[*g as usize].clone()))
    }

    fn expr(&mut self, e: &Expr) -> (Ex, Ty, Reads) {
        let mut reads = Reads::default();
        let (ex, ty) = self.expr_in(e, &mut reads);
        (ex, ty, reads)
    }

    fn expr_in(&mut self, e: &Expr, reads: &mut Reads) -> (Ex, Ty) {
        match e {
            Expr::Num(v, _) => (Ex::Lit(Val::Num(*v)), Ty::Num),
            Expr::Str(s, _) => (Ex::Lit(Val::Str(s.as_str().into())), Ty::Str),
            Expr::Name(n, at) => {
                if n == "true" || n == "false" {
                    return (Ex::Lit(Val::Bool(n == "true")), Ty::Bool);
                }
                if let Some((slot, ty)) = self.lookup(n) {
                    match slot {
                        Slot::Global(g) => {
                            reads.globals.insert(g);
                            let r = self.prog.global_reads[g as usize].clone();
                            reads.time |= r.time;
                        }
                        Slot::Local(l) => {
                            if let Some(r) = self.local_reads.get(l as usize).cloned() {
                                reads.add(&r);
                            }
                        }
                    }
                    return (Ex::Get(slot), ty);
                }
                if let Some((_, v)) = self.consts.iter().find(|(c, _)| c == n) {
                    return (Ex::Lit(Val::Num(*v)), Ty::Num);
                }
                if let Some((_, v)) = MATH.iter().find(|(c, _)| c == n) {
                    return (Ex::Lit(Val::Num(*v)), Ty::Num);
                }
                if let Some(v) = self.externs.constant_of(n) {
                    return (Ex::Lit(Val::Num(v)), Ty::Num);
                }
                if n.starts_with(char::is_uppercase) {
                    if let Some(t) = self.type_index(n, false) {
                        let id = intern(&mut self.prog.resources, self.types[t].name);
                        reads.resources.insert(id);
                        return (Ex::Res(id), self.shape_ty(t));
                    }
                    self.errors.push(error(
                        *at,
                        format!("no resource `{n}` is registered{}", self.suggest(n)),
                    ));
                } else {
                    self.errors
                        .push(error(*at, format!("`{n}` is not defined here")));
                }
                (Ex::Lit(Val::None), Ty::Unknown)
            }
            Expr::Field(recv, field, at) => {
                if field.starts_with(char::is_uppercase) {
                    let (r, ty) = self.expr_in(recv, reads);
                    if ty != Ty::Ent && ty != Ty::Unknown {
                        self.errors.push(error(
                            *at,
                            format!(
                                "`.{field}` reads a component of an entity, not of {}",
                                name(&ty)
                            ),
                        ));
                    }
                    let Some((id, cty)) = self.component(field, *at) else {
                        return (Ex::Lit(Val::None), Ty::Unknown);
                    };
                    reads.components.insert(id);
                    if !self.is_own(&r) {
                        reads.elsewhere = true;
                    }
                    return (Ex::Comp(Box::new(r), id), cty);
                }
                let (r, ty) = self.expr_in(recv, reads);
                match &ty {
                    Ty::V3 | Ty::Quat => {
                        let axis = match field.as_str() {
                            "x" => 0,
                            "y" => 1,
                            "z" => 2,
                            "w" if ty == Ty::Quat => 3,
                            _ => {
                                self.errors.push(error(
                                    *at,
                                    format!("{} has x, y and z, not `{field}`", name(&ty)),
                                ));
                                return (Ex::Lit(Val::None), Ty::Unknown);
                            }
                        };
                        (Ex::Axis(Box::new(r), axis), Ty::Num)
                    }
                    Ty::Rec(fields) => match fields.iter().position(|(n, _)| n == field) {
                        Some(i) => (fuse(r, i as u16), fields[i].1.clone()),
                        None => {
                            let names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
                            self.errors.push(error(
                                *at,
                                format!(
                                    "no field `{field}` here; its fields are {}",
                                    names.join(", ")
                                ),
                            ));
                            (Ex::Lit(Val::None), Ty::Unknown)
                        }
                    },
                    _ => {
                        self.errors.push(error(
                            *at,
                            format!("`.{field}` on {}, whose fields are not known", name(&ty)),
                        ));
                        (Ex::Lit(Val::None), Ty::Unknown)
                    }
                }
            }
            Expr::Index(list, i, at) => {
                let (l, ty) = self.expr_in(list, reads);
                let (i, ity) = self.expr_in(i, reads);
                if ity != Ty::Num && ity != Ty::Unknown {
                    self.errors.push(error(*at, "an index is a number"));
                }
                let elem = match ty {
                    Ty::Col => Ty::Num,
                    Ty::List(t) => *t,
                    Ty::Unknown => Ty::Unknown,
                    other => {
                        self.errors
                            .push(error(*at, format!("indexing {}", name(&other))));
                        Ty::Unknown
                    }
                };
                (Ex::Index(Box::new(l), Box::new(i)), elem)
            }
            Expr::Unary(op, a, at) => {
                let (a, ty) = self.expr_in(a, reads);
                let out = match (op, &ty) {
                    (Op::Neg, Ty::Num | Ty::V3) | (Op::Not, Ty::Bool) | (_, Ty::Unknown) => {
                        ty.clone()
                    }
                    _ => {
                        self.errors.push(error(
                            *at,
                            format!(
                                "`{}` on {}",
                                if *op == Op::Neg { "-" } else { "!" },
                                name(&ty)
                            ),
                        ));
                        Ty::Unknown
                    }
                };
                (Ex::Un(*op, Box::new(a)), out)
            }
            Expr::Binary(op, a, b, at) => {
                let (a, ta) = self.expr_in(a, reads);
                let (b, tb) = self.expr_in(b, reads);
                let out = binary_ty(*op, &ta, &tb).unwrap_or_else(|| {
                    self.errors
                        .push(error(*at, format!("{} {:?} {}", name(&ta), op, name(&tb))));
                    Ty::Unknown
                });
                (Ex::Bin(*op, Box::new(a), Box::new(b)), out)
            }
            Expr::Cond(c, a, b, at) => {
                let c = self.want(c, Ty::Bool, *at, reads);
                let (a, ta) = self.expr_in(a, reads);
                let (b, tb) = self.expr_in(b, reads);
                (
                    Ex::Cond(Box::new(c), Box::new(a), Box::new(b)),
                    self.join(ta, tb, *at),
                )
            }
            Expr::Is(a, arm, at) => {
                let (a, ty) = self.expr_in(a, reads);
                let index = self.arm(&ty, arm, *at);
                (Ex::Is(Box::new(a), index), Ty::Bool)
            }
            Expr::Record(fields, _) => {
                let mut exs = Vec::new();
                let mut tys = Vec::new();
                for (n, e) in fields {
                    let (ex, ty) = self.expr_in(e, reads);
                    exs.push(ex);
                    tys.push((n.clone(), ty));
                }
                (Ex::Rec(exs), Ty::Rec(Rc::new(tys)))
            }
            Expr::Match(scrutinee, arms, at) => self.matching(scrutinee, arms, *at, reads),
            Expr::Call(f, args, at) => self.call(f, args, *at, reads),
        }
    }

    fn is_own(&self, r: &Ex) -> bool {
        matches!((r, self.own.last()), (Ex::Get(Slot::Local(s)), Some((ent, _))) if s == ent)
    }

    fn join(&mut self, a: Ty, b: Ty, at: Span) -> Ty {
        if a == b || b == Ty::Unknown {
            a
        } else if a == Ty::Unknown {
            b
        } else {
            self.errors.push(error(
                at,
                format!("the branches are {} and {}", name(&a), name(&b)),
            ));
            Ty::Unknown
        }
    }

    fn arm(&mut self, ty: &Ty, arm: &str, at: Span) -> u32 {
        let Ty::Arm(of) = ty else {
            if *ty != Ty::Unknown {
                self.errors.push(error(
                    at,
                    format!("`is {arm}` tests an enum field, not {}", name(ty)),
                ));
            }
            return u32::MAX;
        };
        let mut text = format!("{{\"{arm}\":{{}}}}");
        for field in of.path.iter().rev() {
            text = format!("{{\"{field}\":{text}}}");
        }
        let owner = &self.types[of.owner];
        let mut tree = Tree::default();
        let found = owner
            .read_json(&text, &mut tree)
            .ok()
            .and_then(|_| tree.finish())
            .and_then(|s| arm_at(&s, &of.path));
        match found {
            Some((name, index)) if name == arm => index,
            _ => {
                self.errors.push(error(
                    at,
                    format!(
                        "`{arm}` is not an arm of {}.{} (its default is `{}`)",
                        owner.name,
                        of.path.join("."),
                        of.default
                    ),
                ));
                u32::MAX
            }
        }
    }

    fn matching(
        &mut self,
        scrutinee: &Expr,
        arms: &[crate::syntax::Arm],
        at: Span,
        reads: &mut Reads,
    ) -> (Ex, Ty) {
        let (s, ty) = self.expr_in(scrutinee, reads);
        let mut out = Ty::Unknown;
        if ty == Ty::Ent {
            let mut cases = Vec::new();
            for arm in arms {
                self.scopes.push(Scope { names: Vec::new() });
                let case = match &arm.pat {
                    None => Case::Any,
                    Some((n, binder)) if n.starts_with(char::is_uppercase) => {
                        match self.component(n, arm.at) {
                            Some((id, cty)) => {
                                reads.components.insert(id);
                                if !self.is_own(&s) {
                                    reads.elsewhere = true;
                                }
                                let mut r = Reads::default();
                                r.components.insert(id);
                                let slot = binder.as_ref().map(|b| self.local(b, cty, r));
                                Case::Has(id, slot)
                            }
                            None => Case::Any,
                        }
                    }
                    Some((n, _)) => {
                        let (e, ety) = self.expr_in(&Expr::Name(n.clone(), arm.at), reads);
                        if ety != Ty::Ent && ety != Ty::Unknown {
                            self.errors.push(error(
                                arm.at,
                                format!("`case {n}` compares entities; `{n}` is {}", name(&ety)),
                            ));
                        }
                        Case::Same(e)
                    }
                };
                let (body, bty) = self.expr_in(&arm.body, reads);
                self.scopes.pop();
                out = self.join(out, bty, arm.at);
                cases.push((case, body));
            }
            return (Ex::MatchEnt(Box::new(s), cases), out);
        }
        let mut cases = Vec::new();
        for arm in arms {
            let index = match &arm.pat {
                None => None,
                Some((n, None)) => Some(self.arm(&ty, n, arm.at)),
                Some((n, Some(_))) => {
                    self.errors.push(error(
                        arm.at,
                        format!(
                            "`case {n} …` binds a component; this match is over {}",
                            name(&ty)
                        ),
                    ));
                    None
                }
            };
            let (body, bty) = self.expr_in(&arm.body, reads);
            out = self.join(out, bty, arm.at);
            cases.push((index, body));
        }
        if !arms.iter().any(|a| a.pat.is_none()) {
            // Data does not list a type's arms, so a match without `_` cannot
            // be proved total; it fails at run time on an arm it does not name.
            let _ = at;
        }
        (Ex::MatchArm(Box::new(s), cases), out)
    }

    fn call(&mut self, f: &str, args: &[Expr], at: Span, reads: &mut Reads) -> (Ex, Ty) {
        if f == "named" {
            return match args {
                [Expr::Str(s, _)] => {
                    reads.volatile = true;
                    (Ex::Named(s.as_str().into()), Ty::Ent)
                }
                _ => {
                    self.errors
                        .push(error(at, "named(\"…\") takes a name in quotes"));
                    (Ex::Lit(Val::None), Ty::Unknown)
                }
            };
        }
        if f == "has" {
            let [e, Expr::Name(c, cat)] = args else {
                self.errors.push(error(at, "has(entity, Component)"));
                return (Ex::Lit(Val::None), Ty::Unknown);
            };
            let (e, _) = self.expr_in(e, reads);
            let Some((id, _)) = self.component(c, *cat) else {
                return (Ex::Lit(Val::None), Ty::Unknown);
            };
            reads.components.insert(id);
            if !self.is_own(&e) {
                reads.elsewhere = true;
            }
            return (Ex::Has(Box::new(e), id), Ty::Bool);
        }
        let mut exs = Vec::new();
        let mut tys = Vec::new();
        for a in args {
            let (e, t) = self.expr_in(a, reads);
            exs.push(e);
            tys.push(t);
        }
        if let Some((_, b, arity)) = BUILTINS.iter().find(|(n, _, _)| *n == f) {
            if *arity != args.len() {
                self.errors
                    .push(error(at, format!("{f} takes {arity} argument(s)")));
                return (Ex::Lit(Val::None), Ty::Unknown);
            }
            if matches!(b, B::Seconds | B::Tick) {
                reads.time = true;
            }
            if *b == B::WorldPosition {
                reads.volatile = true;
            }
            let ty = builtin_ty(*b, &tys);
            if ty.is_none() {
                let given: Vec<String> = tys.iter().map(name).collect();
                self.errors
                    .push(error(at, format!("{f} cannot take {}", given.join(", "))));
            }
            return (Ex::Builtin(*b, exs), ty.unwrap_or(Ty::Unknown));
        }
        if let Some(i) = self.externs.function_of(f) {
            let arity = self.externs.arity(i);
            if arity != args.len() || tys.iter().any(|t| *t != Ty::Num && *t != Ty::Unknown) {
                self.errors
                    .push(error(at, format!("{f} takes {arity} number(s)")));
            }
            return (Ex::Extern(i as u16, exs), Ty::Num);
        }
        if let Some(i) = self.fn_names.iter().position(|(n, _, _)| n == f) {
            let (_, params, ret) = self.fn_names[i].clone();
            if params.len() != args.len() {
                self.errors
                    .push(error(at, format!("{f} takes {} argument(s)", params.len())));
            }
            for (k, (p, t)) in params.iter().zip(&tys).enumerate() {
                if p != t && *t != Ty::Unknown && *p != Ty::Unknown {
                    self.errors.push(error(
                        args[k].span(),
                        format!("{f}'s argument {} is {}, not {}", k + 1, name(p), name(t)),
                    ));
                }
            }
            let fr = &self.prog.fns[i];
            if !self.in_fn && fr.needs > self.prog.globals {
                self.errors.push(error(
                    at,
                    format!("{f} reads a value defined below this line"),
                ));
            }
            reads.add(&fr.reads.clone());
            return (Ex::Call(i as u16, exs), ret);
        }
        let hint = if f == "range" {
            " (`range(a, b)` is only a `for`'s list)"
        } else {
            ""
        };
        self.errors
            .push(error(at, format!("no function `{f}`{hint}")));
        (Ex::Lit(Val::None), Ty::Unknown)
    }
}

fn fuse(r: Ex, i: u16) -> Ex {
    match r {
        Ex::Comp(e, c) => Ex::CompPath(e, c, vec![i]),
        Ex::CompPath(e, c, mut path) => {
            path.push(i);
            Ex::CompPath(e, c, path)
        }
        r => Ex::Field(Box::new(r), i),
    }
}

fn intern(list: &mut Vec<&'static str>, name: &'static str) -> u16 {
    match list.iter().position(|n| *n == name) {
        Some(i) => i as u16,
        None => {
            list.push(name);
            (list.len() - 1) as u16
        }
    }
}

fn ty_of(s: &Shape, owner: usize, path: &mut Vec<String>) -> Ty {
    match s {
        Shape::Num => Ty::Num,
        Shape::Bool => Ty::Bool,
        Shape::Str => Ty::Str,
        Shape::Ent => Ty::Ent,
        Shape::V3 => Ty::V3,
        Shape::Quat => Ty::Quat,
        Shape::Col => Ty::Col,
        Shape::List(e) => Ty::List(Box::new(
            e.as_ref().map_or(Ty::Unknown, |e| ty_of(e, owner, path)),
        )),
        Shape::Rec(fields) => Ty::Rec(Rc::new(
            fields
                .iter()
                .map(|(n, s)| {
                    path.push(n.clone());
                    let t = ty_of(s, owner, path);
                    path.pop();
                    (n.clone(), t)
                })
                .collect(),
        )),
        Shape::Arm(default, _) => Ty::Arm(Rc::new(ArmOf {
            owner,
            path: path.clone(),
            default: default.clone(),
        })),
        Shape::Opt(s) => s.as_ref().map_or(Ty::Unknown, |s| ty_of(s, owner, path)),
    }
}

fn arm_at(s: &Shape, path: &[String]) -> Option<(String, u32)> {
    match (s, path.split_first()) {
        (Shape::Arm(n, i), None) => Some((n.clone(), *i)),
        (Shape::Rec(fields), Some((head, rest))) => fields
            .iter()
            .find(|(n, _)| n == head)
            .and_then(|(_, s)| arm_at(s, rest)),
        _ => None,
    }
}

pub(crate) fn name(t: &Ty) -> String {
    match t {
        Ty::Num => "a number".into(),
        Ty::Bool => "a bool".into(),
        Ty::Str => "a string".into(),
        Ty::Ent => "an entity".into(),
        Ty::V3 => "a vec3".into(),
        Ty::Quat => "a quat".into(),
        Ty::Col => "a column".into(),
        Ty::Arm(of) => format!("an enum (default `{}`)", of.default),
        Ty::List(_) => "a list".into(),
        Ty::Rec(f) => format!(
            "a record {{{}}}",
            f.iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Ty::Unknown => "a value of unknown type".into(),
    }
}

fn binary_ty(op: Op, a: &Ty, b: &Ty) -> Option<Ty> {
    use Ty::*;
    if *a == Unknown || *b == Unknown {
        return Some(match op {
            Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge | Op::And | Op::Or => Bool,
            _ => Unknown,
        });
    }
    Some(match (op, a, b) {
        (Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Rem, Num, Num) => Num,
        (Op::Add | Op::Sub, V3, V3) => V3,
        (Op::Mul, V3, Num) | (Op::Mul, Num, V3) | (Op::Div, V3, Num) => V3,
        (Op::Mul, Quat, Quat) => Quat,
        (Op::Mul, Quat, V3) => V3,
        (Op::Lt | Op::Le | Op::Gt | Op::Ge, Num, Num) => Bool,
        (Op::Eq | Op::Ne, x, y) if x == y => Bool,
        (Op::And | Op::Or, Bool, Bool) => Bool,
        _ => return None,
    })
}

fn builtin_ty(b: B, args: &[Ty]) -> Option<Ty> {
    use Ty::*;
    let ok = |want: &[Ty]| args.iter().zip(want).all(|(a, w)| a == w || *a == Unknown);
    Some(match b {
        B::Seconds | B::Tick => Num,
        B::Some => Bool,
        B::WorldPosition if ok(&[Ent]) => V3,
        B::Vec if ok(&[Num, Num, Num]) => V3,
        B::Xz | B::Normalize if ok(&[V3]) => V3,
        B::Length if ok(&[V3]) => Num,
        B::Dot if ok(&[V3, V3]) => Num,
        B::Min | B::Max | B::Atan2 if ok(&[Num, Num]) => Num,
        B::Abs | B::Floor | B::Ceil | B::Sqrt | B::Sin | B::Cos if ok(&[Num]) => Num,
        B::Yaw | B::Pitch | B::Roll if ok(&[Num]) => Quat,
        B::Identity => Quat,
        B::Inverse if ok(&[Quat]) => Quat,
        B::Len => Num,
        _ => return None,
    })
}
