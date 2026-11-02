use contract_syntax::{
    BinOp, Component, Expr, File, FnDecl, Node, PropertyValue as V, Stmt, TypeExpr, UnOp,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Bottom,
    Literals(Vec<V>),
    Whole,
    Finite,
    Text,
    List(Box<Value>),
    Unknown,
}
impl Value {
    fn literal(v: V) -> Self {
        Self::Literals(vec![v])
    }

    fn whole(&self) -> bool {
        match self {
            Self::Whole => true,
            Self::Literals(vs) => {
                !vs.is_empty()
                    && vs
                        .iter()
                        .all(|v| matches!(v, V::Number(n) if n.is_finite() && n.fract() == 0.0))
            }
            _ => false,
        }
    }

    fn text(&self) -> bool {
        matches!(self, Self::Text)
            || matches!(self, Self::Literals(vs) if !vs.is_empty() && vs.iter().all(|v| matches!(v, V::String(_))))
    }

    fn finite(&self) -> bool {
        matches!(self, Self::Whole | Self::Finite)
            || matches!(self, Self::Literals(vs) if !vs.is_empty() && vs.iter().all(|v| matches!(v, V::Number(n) if n.is_finite())))
    }

    fn fits(&self, set: Option<&[V]>) -> bool {
        match set {
            Some(set) => {
                matches!(self, Self::Literals(vs) if !vs.is_empty() && vs.iter().all(|v| set.iter().any(|s| same_literal(s, v))))
            }
            None => self.whole(),
        }
    }

    fn refusal(&self, set: Option<&[V]>) -> Option<String> {
        if self.fits(set) {
            return None;
        }
        if let Self::Literals(values) = self {
            let outside = values
                .iter()
                .find(|v| !Self::literal((*v).clone()).fits(set));
            if let Some(value) = outside {
                return Some(format!("value {value:?} is outside the candidate"));
            }
        }
        Some(if matches!(self, Self::Whole) {
            "a whole number is not confined to the candidate set".into()
        } else if matches!(self, Self::Finite) {
            "a finite number need not be whole or confined to the candidate set".into()
        } else {
            "the value is not bounded (free input, host value or unsupported expression)".into()
        })
    }

    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Bottom, v) | (v, Self::Bottom) => v.clone(),
            (Self::Literals(a), Self::Literals(b)) => {
                let mut values = a.clone();
                for v in b {
                    if !values.iter().any(|old| same_literal(old, v)) {
                        values.push(v.clone());
                    }
                }
                // Bound the lattice so call and state cycles reach a fixed point.
                if values.len() <= 32 {
                    Self::Literals(values)
                } else if self.whole() && other.whole() {
                    Self::Whole
                } else if self.finite() && other.finite() {
                    Self::Finite
                } else if self.text() && other.text() {
                    Self::Text
                } else {
                    Self::Unknown
                }
            }
            (Self::List(a), Self::List(b)) => Self::List(Box::new(a.join(b))),
            _ if self.whole() && other.whole() => Self::Whole,
            _ if self.finite() && other.finite() => Self::Finite,
            _ if self.text() && other.text() => Self::Text,
            _ => Self::Unknown,
        }
    }

    fn typed(self, ty: Option<&TypeExpr>) -> Self {
        if self != Self::Unknown {
            return self;
        }
        match ty {
            Some(TypeExpr::Named(n, _)) if n == "string" => Self::Text,
            Some(TypeExpr::List(_, _)) => Self::List(Box::new(Self::Unknown)),
            _ => self,
        }
    }
}

type Env = BTreeMap<String, Value>;

struct Write {
    target: String,
    value: Value,
    site: String,
    unchanged: bool,
}

pub(super) struct Facts<'a> {
    root: &'a Component,
    fns: &'a [FnDecl],
    slots: Env,
    params: BTreeMap<String, Env>,
    writes: Vec<Write>,
}

impl<'a> Facts<'a> {
    pub(super) fn new(root: &'a Component, file: &'a File) -> Self {
        let mut facts = Self {
            root,
            fns: &file.fns,
            slots: root
                .states
                .iter()
                .map(|s| (s.name.clone(), Value::Bottom))
                .chain(
                    root.mutations
                        .iter()
                        .map(|m| (m.name.clone(), Value::Unknown)),
                )
                .collect(),
            params: root
                .actions
                .iter()
                .map(|a| {
                    (
                        a.name.clone(),
                        a.params
                            .iter()
                            .map(|p| (p.name.clone(), Value::Bottom))
                            .collect(),
                    )
                })
                .collect(),
            writes: Vec::new(),
        };
        loop {
            let mut slots = facts.slots.clone();
            let mut params = facts.params.clone();
            for s in &root.states {
                merge(&mut slots, &s.name, facts.expr(&s.expr, &Env::new()));
            }
            let mut writes = Vec::new();
            for a in &root.actions {
                facts.body(
                    &a.body,
                    &facts.params[&a.name],
                    &a.name,
                    &mut writes,
                    &mut params,
                );
            }
            for w in &writes {
                merge(&mut slots, &w.target, w.value.clone());
            }
            facts.view(&root.view, &Env::new(), &mut params);
            for t in &root.tasks {
                facts.call(&t.timer.1, &[], &Env::new(), &mut params);
            }
            for sequence in &root.sequences {
                if let Some((ended, _)) = &sequence.ended {
                    facts.call(ended, &[], &Env::new(), &mut params);
                }
            }
            // Loading moves a module event's binding off the view onto its declaration.
            for module in &file.modules {
                if let Some(handler) = &module.handler {
                    facts.call(handler, &[], &Env::new(), &mut params);
                }
            }
            for m in &root.mutations {
                if let Some((then, _)) = &m.then {
                    facts.call(then, &[], &Env::new(), &mut params);
                }
            }
            for tool in &root.tools {
                facts.call(&tool.action.0, &[], &Env::new(), &mut params);
            }
            facts.writes = writes;
            if slots == facts.slots && params == facts.params {
                break;
            }
            facts.slots = slots;
            facts.params = params;
        }
        facts
    }

    pub(super) fn check(&self, name: &str, set: Option<&[V]>) -> Option<String> {
        if let Some(s) = self.root.states.iter().find(|s| s.name == name) {
            if let Some(why) = self.expr(&s.expr, &Env::new()).refusal(set) {
                return Some(format!("`{name}` initializer: {why}"));
            }
            for w in self.writes.iter().filter(|w| w.target == name) {
                if let Some(why) = w.value.refusal(set) {
                    return Some(format!("{}: `{name}` write: {why}", w.site));
                }
            }
        }
        let value = self.expr(&Expr::Ident(name.into(), self.root.span), &Env::new());
        value.refusal(set).map(|why| format!("`{name}`: {why}"))
    }

    pub(super) fn latch(&self, name: &str, value: &V) -> Option<String> {
        if !self.root.states.iter().any(|s| s.name == name) {
            return Some(format!(
                "`{name}` is not a state with enumerable assignments"
            ));
        }
        self.writes
            .iter()
            .filter(|w| w.target == name)
            .find_map(|w| {
                (!w.unchanged && !w.value.fits(Some(std::slice::from_ref(value))))
                    .then(|| format!("{}: `{name}` may be reset after the latch", w.site))
            })
    }

    fn expr(&self, e: &Expr, env: &Env) -> Value {
        self.eval(e, env, &mut BTreeSet::new())
    }

    fn eval(&self, e: &Expr, env: &Env, seen: &mut BTreeSet<String>) -> Value {
        match e {
            Expr::Number(n, _) if n.is_finite() => Value::literal(V::Number(*n)),
            Expr::Str(s, _) => Value::literal(V::String(s.clone())),
            Expr::Bool(b, _) => Value::literal(V::Bool(*b)),
            Expr::None(_) => Value::literal(V::None),
            Expr::Some(_, _) => Value::literal(V::Some),
            Expr::Typed(e, ty, _) => self.eval(e, env, seen).typed(Some(ty)),
            Expr::NamedArg(_, e, _) => self.eval(e, env, seen),
            Expr::Ident(name, _) => {
                if let Some(v) = env.get(name).or_else(|| self.slots.get(name)) {
                    return v.clone();
                }
                let key = format!("derive:{name}");
                if !seen.insert(key.clone()) {
                    return Value::Unknown;
                }
                let result = self
                    .root
                    .derives
                    .iter()
                    .find(|d| d.name == *name)
                    .map_or(Value::Unknown, |d| self.eval(&d.expr, &Env::new(), seen));
                seen.remove(&key);
                result
            }
            Expr::Ternary(_, a, b, _) => self.eval(a, env, seen).join(&self.eval(b, env, seen)),
            Expr::Match {
                var, some, none, ..
            } => {
                let mut inner = env.clone();
                inner.insert(var.clone(), Value::Unknown);
                self.eval(some, &inner, seen)
                    .join(&self.eval(none, env, seen))
            }
            Expr::Let {
                name, value, body, ..
            } => {
                let mut inner = env.clone();
                inner.insert(name.clone(), self.eval(value, env, seen));
                self.eval(body, &inner, seen)
            }
            Expr::List(items, _) => Value::List(Box::new(
                items
                    .iter()
                    .fold(Value::Bottom, |v, e| v.join(&self.eval(e, env, seen))),
            )),
            Expr::Unary(UnOp::Neg, e, _) => match self.eval(e, env, seen) {
                Value::Literals(vs) if vs.iter().all(|v| matches!(v, V::Number(_))) => {
                    Value::Literals(
                        vs.into_iter()
                            .map(|v| match v {
                                V::Number(n) => V::Number(-n),
                                _ => unreachable!(),
                            })
                            .collect(),
                    )
                }
                Value::Whole => Value::Whole,
                Value::Finite => Value::Finite,
                Value::Bottom => Value::Bottom,
                _ => Value::Unknown,
            },
            Expr::Unary(UnOp::Not, _, _) => booleans(),
            Expr::Binary(op, a, b, _) => {
                if matches!(
                    op,
                    BinOp::Eq
                        | BinOp::Ne
                        | BinOp::Lt
                        | BinOp::Le
                        | BinOp::Gt
                        | BinOp::Ge
                        | BinOp::And
                        | BinOp::Or
                ) {
                    return booleans();
                }
                let av = self.eval(a, env, seen);
                let bv = self.eval(b, env, seen);
                if av == Value::Bottom || bv == Value::Bottom {
                    return Value::Bottom;
                }
                // Adding or subtracting one preserves finite whole binary64 values,
                // including the magnitudes at which one rounds away.
                if matches!(op, BinOp::Add | BinOp::Sub)
                    && av.whole()
                    && matches!(b.as_ref(), Expr::Number(n, _) if *n == 1.0)
                {
                    Value::Whole
                } else {
                    Value::Unknown
                }
            }
            Expr::Call(name, args, _) => {
                if let Some(f) = self.fns.iter().find(|f| f.name == *name) {
                    let values: Vec<_> = args.iter().map(|e| self.eval(e, env, seen)).collect();
                    let key = format!("fn:{name}");
                    if !seen.insert(key.clone()) {
                        return Value::Unknown;
                    }
                    let inner = f
                        .params
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            (
                                p.name.clone(),
                                values.get(i).cloned().unwrap_or(Value::Unknown),
                            )
                        })
                        .collect();
                    let result = self.eval(&f.body, &inner, seen);
                    seen.remove(&key);
                    return result;
                }
                match (name.as_str(), args.as_slice()) {
                    ("floor" | "round" | "ceil", [e]) => match self.eval(e, env, seen) {
                        Value::Bottom => Value::Bottom,
                        v if v.finite() => Value::Whole,
                        _ => Value::Unknown,
                    },
                    ("performanceNow", []) => Value::Finite,
                    ("length", [e]) => match self.eval(e, env, seen) {
                        Value::Bottom => Value::Bottom,
                        v if v.text() || matches!(v, Value::List(_)) => Value::Whole,
                        _ => Value::Unknown,
                    },
                    ("min" | "max", [a, b]) => {
                        let a = self.eval(a, env, seen);
                        let b = self.eval(b, env, seen);
                        if a == Value::Bottom || b == Value::Bottom {
                            Value::Bottom
                        } else if a.whole() && b.whole() {
                            Value::Whole
                        } else {
                            Value::Unknown
                        }
                    }
                    _ => Value::Unknown,
                }
            }
            _ => Value::Unknown,
        }
    }

    fn call(&self, name: &str, args: &[Expr], env: &Env, params: &mut BTreeMap<String, Env>) {
        let Some(incoming) = params.get_mut(name) else {
            return;
        };
        let Some(a) = self.root.actions.iter().find(|a| a.name == name) else {
            return;
        };
        for (i, p) in a.params.iter().enumerate() {
            let value = args
                .get(i)
                .map_or(Value::Unknown, |e| self.expr(e, env))
                .typed(p.ty.as_ref());
            merge(incoming, &p.name, value);
        }
    }

    fn body(
        &self,
        body: &[Stmt],
        env: &Env,
        action: &str,
        writes: &mut Vec<Write>,
        params: &mut BTreeMap<String, Env>,
    ) {
        let mut env = env.clone();
        for stmt in body {
            match stmt {
                Stmt::Let { name, expr, .. } => {
                    env.insert(name.clone(), self.expr(expr, &env));
                }
                Stmt::Assign { target, expr, span } => writes.push(Write {
                    target: target.clone(),
                    value: self.expr(expr, &env),
                    site: format!("action `{action}` at {span}"),
                    unchanged: matches!(expr, Expr::Ident(n, _) if n == target && !env.contains_key(n)),
                }),
                Stmt::Send { target, span, .. } => writes.push(Write {
                    target: target.clone(), value: Value::Unknown,
                    site: format!("send in `{action}` at {span}"), unchanged: false,
                }),
                Stmt::If { then, otherwise, .. } => {
                    self.body(then, &env, action, writes, params);
                    self.body(otherwise, &env, action, writes, params);
                }
                Stmt::Match { some, none, .. } => {
                    let mut inner = env.clone();
                    inner.insert(some.0.clone(), Value::Unknown);
                    self.body(&some.1, &inner, action, writes, params);
                    self.body(none, &env, action, writes, params);
                }
                Stmt::Call { action, args, .. } => self.call(action, args, &env, params),
                Stmt::Command { name, args, .. } => self.call(name, args, &env, params),
                Stmt::Refresh { .. } => {}
            }
        }
    }

    fn view(&self, nodes: &[Node], env: &Env, params: &mut BTreeMap<String, Env>) {
        for n in nodes {
            match n {
                Node::Element {
                    attrs, children, ..
                } => {
                    for a in attrs {
                        match &a.value {
                            Expr::Ident(n, _) => self.call(n, &[], env, params),
                            Expr::Call(n, args, _) => self.call(n, args, env, params),
                            _ => {}
                        }
                    }
                    self.view(children, env, params);
                }
                Node::When {
                    then, otherwise, ..
                } => {
                    self.view(then, env, params);
                    self.view(otherwise, env, params);
                }
                Node::Each {
                    var,
                    index,
                    list,
                    body,
                    ..
                } => {
                    let mut inner = env.clone();
                    let items = match self.expr(list, env) {
                        Value::List(items) => *items,
                        Value::Bottom => Value::Bottom,
                        _ => Value::Unknown,
                    };
                    inner.insert(var.clone(), items);
                    if let Some(index) = index {
                        inner.insert(index.clone(), Value::Whole);
                    }
                    self.view(body, &inner, params);
                }
                Node::Match { some, none, .. } => {
                    let mut inner = env.clone();
                    inner.insert(some.0.clone(), Value::Unknown);
                    self.view(&some.1, &inner, params);
                    self.view(none, env, params);
                }
                Node::Use { children, .. } => self.view(children, env, params),
                Node::Children { .. } => {}
            }
        }
    }
}

fn booleans() -> Value {
    Value::Literals(vec![V::Bool(false), V::Bool(true)])
}

fn merge(env: &mut Env, name: &str, value: Value) {
    let joined = env.get(name).unwrap_or(&Value::Bottom).join(&value);
    env.insert(name.into(), joined);
}

fn same_literal(a: &V, b: &V) -> bool {
    match (a, b) {
        (V::Number(a), V::Number(b)) => a.to_bits() == b.to_bits(),
        _ => a == b,
    }
}
