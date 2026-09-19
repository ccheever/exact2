//! Closed-type inference for Contract.
//!
//! @ref LLP 1004 D3 (closed inferred types; `Option`-only absence) / LLP 0508
//! §4 (research)
//!
//! Types come from initializers, shapes, props, and the stdlib roster; there
//! are no annotations on state. `none` alone has the type `option<?>`, and the
//! `?` is filled in by the first write that says what it holds; a `?` that
//! nothing fills is a rejection, never a guess. Every rejection carries a
//! stable id and a span.
//!
//! This crate also owns [`Scope`] and [`Ref`]: how a name resolves to a slot,
//! derive, resource, prop, parameter, action, `each` item, or `match`
//! binding. Later passes resolve names through the same code, so a name means
//! one thing everywhere.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod checks;
/// Router declaration checking and compile-time path expansion (LLP 1038 D2/D3).
pub mod routes;

use contract_syntax::{BinOp, Component, Expr, File, Node, Span, TemplatePart, TypeExpr, UnOp};
use exact_plan::Stdlib;
use std::collections::BTreeMap;

use checks::{
    check_injects, check_shape_cycles, check_stmts, check_view, infer_owned_state_initializers,
};

/// A closed type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    /// `number`
    Number,
    /// `string`
    String,
    /// `bool`
    Bool,
    /// `unit`
    Unit,
    /// `option<T>`
    Option(Box<Ty>),
    /// `list<T>`
    List(Box<Ty>),
    /// A shape, by name.
    Record(String),
    /// An action reference with its parameter types.
    Action(Vec<Ty>),
    /// Not yet known (only inside an `option` from `none`, or an untyped parameter).
    Unknown,
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Number => write!(f, "number"),
            Ty::String => write!(f, "string"),
            Ty::Bool => write!(f, "bool"),
            Ty::Unit => write!(f, "unit"),
            Ty::Option(t) => write!(f, "option<{t}>"),
            Ty::List(t) => write!(f, "list<{t}>"),
            Ty::Record(n) => write!(f, "{n}"),
            Ty::Action(ps) => write!(
                f,
                "action({})",
                ps.iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Ty::Unknown => write!(f, "?"),
        }
    }
}

impl Ty {
    /// Whether any `?` remains.
    pub fn is_complete(&self) -> bool {
        match self {
            Ty::Unknown => false,
            Ty::Option(t) | Ty::List(t) => t.is_complete(),
            Ty::Action(ps) => ps.iter().all(Ty::is_complete),
            _ => true,
        }
    }

    /// The most specific type both agree on, or `None` when they conflict.
    pub fn unify(&self, other: &Ty) -> Option<Ty> {
        match (self, other) {
            (Ty::Unknown, t) | (t, Ty::Unknown) => Some(t.clone()),
            (Ty::Option(a), Ty::Option(b)) => a.unify(b).map(|t| Ty::Option(Box::new(t))),
            (Ty::List(a), Ty::List(b)) => a.unify(b).map(|t| Ty::List(Box::new(t))),
            // A bare `action` prop (no parameter list) accepts an action of any arity.
            (Ty::Action(a), Ty::Action(b)) if a.is_empty() => Some(Ty::Action(b.clone())),
            (Ty::Action(a), Ty::Action(b)) if b.is_empty() => Some(Ty::Action(a.clone())),
            (Ty::Action(a), Ty::Action(b)) if a.len() == b.len() => a
                .iter()
                .zip(b)
                .map(|(x, y)| x.unify(y))
                .collect::<Option<Vec<_>>>()
                .map(Ty::Action),
            (a, b) if a == b => Some(a.clone()),
            _ => None,
        }
    }

    /// Whether a value of this type may be passed where the roster spells `spec`.
    pub fn matches_roster(&self, spec: &str) -> bool {
        match spec {
            "number" => *self == Ty::Number,
            "string" => *self == Ty::String,
            "bool" => *self == Ty::Bool,
            "any" => matches!(self, Ty::Number | Ty::String | Ty::Bool | Ty::List(_)),
            _ => *self == Self::from_roster(spec) && *self != Ty::Unknown,
        }
    }

    /// The roster's return spelling as a type.
    pub fn from_roster(spec: &str) -> Ty {
        match spec {
            "number" => Ty::Number,
            "string" => Ty::String,
            "bool" => Ty::Bool,
            "Router" | "Entry" => Ty::Record(spec.into()),
            "list<Entry>" => Ty::List(Box::new(Ty::Record("Entry".into()))),
            "list<string>" => Ty::List(Box::new(Ty::String)),
            _ => Ty::Unknown,
        }
    }
}

/// A typed rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeError {
    /// Stable id.
    pub id: &'static str,
    /// What went wrong.
    pub message: String,
    /// Where.
    pub span: Span,
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [{}] {}", self.span, self.id, self.message)
    }
}

fn err<T>(id: &'static str, message: impl Into<String>, span: Span) -> Result<T, TypeError> {
    Err(TypeError {
        id,
        message: message.into(),
        span,
    })
}

/// The shapes a file declares.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shapes {
    /// Checked app route table, when declared. @ref LLP 1038 D2/D3.
    pub routes: Option<exact_route::Table>,
    /// Shape name → fields in order.
    pub map: BTreeMap<String, Vec<(String, Ty)>>,
    /// `fn` name → (parameter types, result type) (LLP 1017 P5).
    pub fns: BTreeMap<String, (Vec<Ty>, Ty)>,
}

impl Shapes {
    /// Resolve a written type.
    pub fn resolve(&self, t: &TypeExpr) -> Result<Ty, TypeError> {
        Ok(match t {
            TypeExpr::Named(n, span) => match n.as_str() {
                "number" => Ty::Number,
                "string" => Ty::String,
                "bool" => Ty::Bool,
                "unit" => Ty::Unit,
                other => {
                    if self.map.contains_key(other) {
                        Ty::Record(other.to_string())
                    } else if other == "action" {
                        // Bare `action` is an action prop of inferred arity
                        // (LLP 1006 §2). Near-prefix names are ordinary
                        // unknown types, never action typos accepted silently.
                        Ty::Action(Vec::new())
                    } else {
                        return err("type-unknown", format!("unknown type `{other}`"), *span);
                    }
                }
            },
            TypeExpr::Option(inner, _) => Ty::Option(Box::new(self.resolve(inner)?)),
            TypeExpr::List(inner, _) => Ty::List(Box::new(self.resolve(inner)?)),
        })
    }

    /// A field's position and type in a shape.
    pub fn field(&self, shape: &str, name: &str) -> Option<(usize, Ty)> {
        self.map
            .get(shape)?
            .iter()
            .position(|(n, _)| n == name)
            .map(|i| (i, self.map[shape][i].1.clone()))
    }
}

/// What a name refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ref {
    /// A state slot, by index in the component.
    Slot(u32),
    /// A derive, by index.
    Derive(u32),
    /// A resource, by index.
    Resource(u32),
    /// A mutation, by index (its value is `option<T>`).
    Mutation(u32),
    /// An action, by index.
    Action(u32),
    /// A prop, by index.
    Prop(u32),
    /// An action parameter, by index.
    Param(u32),
    /// An `each` item, `depth` region frames out (0 = innermost).
    Item(u32),
    /// A `match` binding, `depth` region frames out.
    Bound(u32),
    /// A name an inline `match` expression binds; the index is lowering's.
    Local(u32),
}

#[derive(Debug, Clone)]
struct Frame {
    names: Vec<(String, Ref, Ty)>,
    /// Whether this frame is a region scope (counts toward `Item`/`Bound` depth).
    region: bool,
}

/// A stack of scopes: component declarations at the bottom, then action
/// parameters or region frames.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    frames: Vec<Frame>,
}

impl Scope {
    /// Push a non-region frame (component declarations, action parameters).
    pub fn push(&mut self, names: Vec<(String, Ref, Ty)>) {
        self.frames.push(Frame {
            names,
            region: false,
        });
    }

    /// Push a region frame binding at most one name (`each` item or `match` binding).
    pub fn push_region(&mut self, name: Option<(String, Ref, Ty)>) {
        self.frames.push(Frame {
            names: name.into_iter().collect(),
            region: true,
        });
    }

    /// Pop the innermost frame.
    pub fn pop(&mut self) {
        self.frames.pop();
    }

    /// How many region frames are on the stack.
    pub fn region_depth(&self) -> u32 {
        self.frames.iter().filter(|f| f.region).count() as u32
    }

    /// Resolve a name. `Item`/`Bound` come back with their depth from the
    /// innermost region frame.
    pub fn lookup(&self, name: &str) -> Option<(Ref, Ty)> {
        let mut depth = 0u32;
        for frame in self.frames.iter().rev() {
            for (n, r, t) in &frame.names {
                if n == name {
                    let r = match r {
                        Ref::Item(_) => Ref::Item(depth),
                        Ref::Bound(_) => Ref::Bound(depth),
                        other => *other,
                    };
                    return Some((r, t.clone()));
                }
            }
            if frame.region {
                depth += 1;
            }
        }
        None
    }
}

/// The inferred types of one component's declarations.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ComponentTypes {
    /// The component's name.
    pub name: String,
    /// Prop types, in declaration order.
    pub props: Vec<Ty>,
    /// Slot types.
    pub slots: Vec<Ty>,
    /// Derive types.
    pub derives: Vec<Ty>,
    /// Resource types (their declared shapes).
    pub resources: Vec<Ty>,
    /// Mutation reply types, `T` (the name reads as `option<T>`).
    pub mutations: Vec<Ty>,
    /// Action parameter types, per action.
    pub actions: Vec<Vec<Ty>>,
    /// The data seam's signatures (LLP 1027 D2): for each source name a
    /// `resource` or a `send` uses, its parameter types and result type,
    /// unified across every use — a disagreement is `type-source-signature`.
    pub sources: BTreeMap<String, (Vec<Ty>, Ty)>,
}

/// Record one use of data source `source` in the component's signature
/// table, unifying with the uses before it: one source, one signature.
pub(crate) fn record_source(
    ct: &mut ComponentTypes,
    source: &str,
    params: Vec<Ty>,
    result: Ty,
    span: Span,
) -> Result<(), TypeError> {
    // The runner fills each reader's own shape for the sources it answers, so
    // their uses are not one signature; the plan's source table still names
    // them (LLP 1030 D7), with the first reader's row.
    if exact_plan::runner_owned_source(source) {
        ct.sources
            .entry(source.to_string())
            .or_insert((params, result));
        return Ok(());
    }
    let Some((have_params, have_result)) = ct.sources.get(source) else {
        ct.sources.insert(source.to_string(), (params, result));
        return Ok(());
    };
    if have_params.len() != params.len() {
        return err(
            "type-source-signature",
            format!(
                "`{source}` takes {} arguments here and {} elsewhere: one source, one signature",
                params.len(),
                have_params.len()
            ),
            span,
        );
    }
    let mut unified = Vec::with_capacity(params.len());
    for (i, (a, b)) in have_params.iter().zip(&params).enumerate() {
        match a.unify(b) {
            Some(u) => unified.push(u),
            None => {
                return err(
                    "type-source-signature",
                    format!("`{source}` takes `{b}` as argument {i} here and `{a}` elsewhere: one source, one signature"),
                    span,
                )
            }
        }
    }
    let result = match have_result.unify(&result) {
        Some(u) => u,
        None => {
            return err(
                "type-source-signature",
                format!("`{source}` answers `{result}` here and `{have_result}` elsewhere: one source, one signature"),
                span,
            )
        }
    };
    ct.sources.insert(source.to_string(), (unified, result));
    Ok(())
}

/// Everything the checker learned.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Types {
    /// Shapes.
    pub shapes: Shapes,
    /// Per component, in file order.
    pub components: Vec<ComponentTypes>,
}

impl Types {
    /// The component's declaration scope: props, slots, derives, resources, actions.
    pub fn component_scope(&self, c: &Component, ct: &ComponentTypes) -> Scope {
        let mut names = Vec::new();
        for (i, p) in c.props.iter().enumerate() {
            names.push((p.name.clone(), Ref::Prop(i as u32), ct.props[i].clone()));
        }
        for (j, p) in c.injects.iter().enumerate() {
            let i = c.props.len() + j;
            names.push((p.name.clone(), Ref::Prop(i as u32), ct.props[i].clone()));
        }
        for (i, s) in c.states.iter().enumerate() {
            names.push((s.name.clone(), Ref::Slot(i as u32), ct.slots[i].clone()));
        }
        for (i, d) in c.derives.iter().enumerate() {
            names.push((d.name.clone(), Ref::Derive(i as u32), ct.derives[i].clone()));
        }
        for (i, r) in c.resources.iter().enumerate() {
            names.push((
                r.name.clone(),
                Ref::Resource(i as u32),
                ct.resources[i].clone(),
            ));
        }
        for (i, m) in c.mutations.iter().enumerate() {
            names.push((
                m.name.clone(),
                Ref::Mutation(i as u32),
                Ty::Option(Box::new(ct.mutations[i].clone())),
            ));
        }
        for (i, a) in c.actions.iter().enumerate() {
            names.push((
                a.name.clone(),
                Ref::Action(i as u32),
                Ty::Action(ct.actions[i].clone()),
            ));
        }
        let mut scope = Scope::default();
        scope.push(names);
        scope
    }
}

/// Infer an expression's type in `scope`.
pub fn infer(e: &Expr, scope: &Scope, shapes: &Shapes) -> Result<Ty, TypeError> {
    Ok(match e {
        Expr::Number(..) => Ty::Number,
        Expr::Str(..) => Ty::String,
        Expr::Bool(..) => Ty::Bool,
        Expr::Template(parts, _) => {
            for p in parts {
                if let TemplatePart::Expr(x) = p {
                    let t = infer(x, scope, shapes)?;
                    if !matches!(t, Ty::Number | Ty::String | Ty::Bool) {
                        return err(
                            "type-template-part",
                            format!("a template part must be a number, string, or bool, not `{t}`"),
                            x.span(),
                        );
                    }
                }
            }
            Ty::String
        }
        Expr::None(_) => Ty::Option(Box::new(Ty::Unknown)),
        Expr::Some(inner, _) => Ty::Option(Box::new(infer(inner, scope, shapes)?)),
        Expr::NamedArg(_, _, span) => {
            return err(
                "type-named-argument",
                "named arguments belong to a canvas surface binding",
                *span,
            )
        }
        Expr::Ident(name, span) => match scope.lookup(name) {
            Some((_, t)) => t,
            None => {
                let hint = if name.contains('-') {
                    " (a name may contain hyphens, as in CSS, so subtraction between two names needs spaces: `a - b`)"
                } else {
                    ""
                };
                return err(
                    "type-unknown-name",
                    format!("unknown name `{name}`{hint}"),
                    *span,
                );
            }
        },
        Expr::Member(obj, field, span) => {
            let t = infer(obj, scope, shapes)?;
            match &t {
                Ty::Record(shape) => match shapes.field(shape, field) {
                    Some((_, ft)) => ft,
                    None => {
                        return err(
                            "type-unknown-field",
                            format!("`{shape}` has no field `{field}`"),
                            *span,
                        )
                    }
                },
                other => {
                    return err(
                        "type-not-a-record",
                        format!("`{other}` has no fields"),
                        *span,
                    )
                }
            }
        }
        Expr::Call(name, args, span) => {
            if name == "path" && !shapes.fns.contains_key(name) {
                routes::expand_path(args, *span, scope, shapes)?;
                return Ok(Ty::String);
            }
            if name == "pending" {
                // `pending(x)`: whether resource or mutation `x` has a
                // request in flight (LLP 1016 D3). Not a roster call: its
                // argument is a name, not a value.
                let [Expr::Ident(target, tspan)] = args.as_slice() else {
                    return err(
                        "type-pending-argument",
                        "`pending(x)` names one resource or mutation",
                        *span,
                    );
                };
                return match scope.lookup(target) {
                    Some((Ref::Resource(_) | Ref::Mutation(_), _)) => Ok(Ty::Bool),
                    _ => err(
                        "type-pending-argument",
                        format!("`{target}` is not a resource or a mutation"),
                        *tspan,
                    ),
                };
            }
            if let Some((params, ret)) = shapes.fns.get(name) {
                // A `fn` (LLP 1017 P5): typed like a roster call.
                if args.len() != params.len() {
                    return err(
                        "type-arity",
                        format!(
                            "`{name}` takes {} argument(s), given {}",
                            params.len(),
                            args.len()
                        ),
                        *span,
                    );
                }
                for (arg, want) in args.iter().zip(params) {
                    let t = infer(arg, scope, shapes)?;
                    if want.unify(&t).is_none() {
                        return err(
                            "type-argument",
                            format!("`{name}` expects `{want}`, given `{t}`"),
                            arg.span(),
                        );
                    }
                }
                return Ok(ret.clone());
            }
            // @ref LLP 1038 D3 — adding roster names must not capture existing
            // scoped action/prop references, e.g. a reader's `open(path)` handler.
            if let Some((Ref::Action(_) | Ref::Prop(_), Ty::Action(params))) = scope
                .lookup(name)
                .filter(|_| !routes::value_call(name, args, scope, shapes))
            {
                // A curried action reference: `action(args)` binds the leading parameters.
                if args.len() > params.len() && !params.is_empty() {
                    return err(
                        "type-arity",
                        format!(
                            "`{name}` takes at most {} argument(s), given {}",
                            params.len(),
                            args.len()
                        ),
                        *span,
                    );
                }
                for (arg, pt) in args.iter().zip(params.iter()) {
                    let t = infer(arg, scope, shapes)?;
                    if t.unify(pt).is_none() {
                        return err(
                            "type-argument",
                            format!("`{name}` expects `{pt}`, given `{t}`"),
                            arg.span(),
                        );
                    }
                }
                Ty::Action(params.iter().skip(args.len()).cloned().collect())
            } else if let Some(f) = Stdlib::from_name(name) {
                routes::require_table(f, shapes, *span)?;
                if args.len() != f.arity() {
                    return err(
                        "type-arity",
                        format!(
                            "`{name}` takes {} argument(s), given {}",
                            f.arity(),
                            args.len()
                        ),
                        *span,
                    );
                }
                for (arg, spec) in args.iter().zip(f.params()) {
                    let t = infer(arg, scope, shapes)?;
                    if !t.matches_roster(spec) {
                        return err(
                            "type-argument",
                            format!("`{name}` expects `{spec}`, given `{t}`"),
                            arg.span(),
                        );
                    }
                }
                routes::location(f, args, shapes)?;
                Ty::from_roster(f.returns())
            } else {
                return err("type-unknown-function", format!("`{name}` is not in the stdlib roster and is not an action; data comes from a `resource`"), *span);
            }
        }
        Expr::Unary(op, inner, span) => {
            let t = infer(inner, scope, shapes)?;
            if t == Ty::Unknown {
                return Ok(Ty::Unknown);
            }
            match (op, &t) {
                (UnOp::Neg, Ty::Number) => Ty::Number,
                (UnOp::Not, Ty::Bool) => Ty::Bool,
                _ => {
                    return err(
                        "type-operand",
                        format!("cannot apply `{op:?}` to `{t}`"),
                        *span,
                    )
                }
            }
        }
        Expr::Binary(op, a, b, span) => {
            let ta = infer(a, scope, shapes)?;
            let tb = infer(b, scope, shapes)?;
            // An operand still `?` (a derive not yet settled in the fixpoint)
            // defers the whole expression; the fixpoint retries it.
            if ta == Ty::Unknown || tb == Ty::Unknown {
                return Ok(match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => Ty::Unknown,
                    _ => Ty::Bool,
                });
            }
            match op {
                BinOp::Add => match (&ta, &tb) {
                    (Ty::Number, Ty::Number) => Ty::Number,
                    (Ty::String, Ty::String) => Ty::String,
                    _ => {
                        return err(
                            "type-operand",
                            format!(
                                "`+` needs two numbers or two strings, given `{ta}` and `{tb}`"
                            ),
                            *span,
                        )
                    }
                },
                BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                    if ta != Ty::Number || tb != Ty::Number {
                        return err(
                            "type-operand",
                            format!("arithmetic needs numbers, given `{ta}` and `{tb}`"),
                            *span,
                        );
                    }
                    Ty::Number
                }
                BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                    if ta != Ty::Number || tb != Ty::Number {
                        return err(
                            "type-operand",
                            format!("comparison needs numbers, given `{ta}` and `{tb}`"),
                            *span,
                        );
                    }
                    Ty::Bool
                }
                BinOp::Eq | BinOp::Ne => {
                    if ta.unify(&tb).is_none() {
                        return err(
                            "type-operand",
                            format!("cannot compare `{ta}` with `{tb}`"),
                            *span,
                        );
                    }
                    Ty::Bool
                }
                BinOp::And | BinOp::Or => {
                    if ta != Ty::Bool || tb != Ty::Bool {
                        return err(
                            "type-operand",
                            format!("`{op:?}` needs bools, given `{ta}` and `{tb}`"),
                            *span,
                        );
                    }
                    Ty::Bool
                }
            }
        }
        Expr::Ternary(c, a, b, span) => {
            if infer(c, scope, shapes)? != Ty::Bool {
                return err("type-condition", "a condition must be a bool", c.span());
            }
            let ta = infer(a, scope, shapes)?;
            let tb = infer(b, scope, shapes)?;
            match ta.unify(&tb) {
                Some(t) => t,
                None => {
                    return err(
                        "type-branches",
                        format!("branches disagree: `{ta}` and `{tb}`"),
                        *span,
                    )
                }
            }
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            span,
        } => {
            let ts = infer(subject, scope, shapes)?;
            let Ty::Option(inner) = ts else {
                return err(
                    "type-match-subject",
                    format!("`match` needs an option, given `{ts}`"),
                    subject.span(),
                );
            };
            let mut inner_scope = scope.clone();
            inner_scope.push(vec![(var.clone(), Ref::Local(0), (*inner).clone())]);
            let ta = infer(some, &inner_scope, shapes)?;
            let tb = infer(none, scope, shapes)?;
            match ta.unify(&tb) {
                Some(t) => t,
                None => {
                    return err(
                        "type-branches",
                        format!("`match` arms disagree: `{ta}` and `{tb}`"),
                        *span,
                    )
                }
            }
        }
    })
}

/// Every function name an expression calls, for the `fn` cycle check.
fn calls_in(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Call(n, args, _) => {
            out.push(n.clone());
            for a in args {
                calls_in(a, out);
            }
        }
        Expr::Some(x, _)
        | Expr::Unary(_, x, _)
        | Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _) => calls_in(x, out),
        Expr::Binary(_, a, b, _) => {
            calls_in(a, out);
            calls_in(b, out);
        }
        Expr::Ternary(a, b, c, _) => {
            calls_in(a, out);
            calls_in(b, out);
            calls_in(c, out);
        }
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => {
            calls_in(subject, out);
            calls_in(some, out);
            calls_in(none, out);
        }
        Expr::Template(parts, _) => {
            for p in parts {
                if let TemplatePart::Expr(x) = p {
                    calls_in(x, out);
                }
            }
        }
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) | Expr::Ident(..) => {}
    }
}

/// Check a file: shapes, then every component.
pub fn check(file: &File) -> Result<Types, TypeError> {
    let mut shapes = Shapes::default();
    routes::declare(file, &mut shapes)?;
    if file.components.is_empty() {
        return err(
            "analyze-no-component",
            "a file needs a component",
            Span {
                line: 1,
                col: 1,
                end_col: 1,
            },
        );
    }
    for s in &file.shapes {
        if shapes.map.contains_key(&s.name) {
            return err(
                "type-duplicate-shape",
                format!("shape `{}` declared twice", s.name),
                s.span,
            );
        }
        shapes.map.insert(s.name.clone(), Vec::new());
    }
    check_shape_cycles(file)?;
    for s in &file.shapes {
        let mut fields = Vec::new();
        for f in &s.fields {
            fields.push((f.name.clone(), shapes.resolve(&f.ty)?));
        }
        shapes.map.insert(s.name.clone(), fields);
    }
    // `fn`s (LLP 1017 P5): signatures first, then each body in a scope of
    // its parameters only — pure by construction — against the declared
    // result; a cycle through calls is refused, since a body is expanded
    // where it is called.
    for f in &file.fns {
        if Stdlib::from_name(&f.name).is_some() {
            return err(
                "contract-fn-shadows-roster",
                format!(
                    "`fn {}` has the roster's name; a roster entry is the framework's — pick another",
                    f.name
                ),
                f.span,
            );
        }
        if shapes.fns.contains_key(&f.name) {
            return err(
                "type-duplicate-fn",
                format!("`fn {}` declared twice", f.name),
                f.span,
            );
        }
        let mut params = Vec::new();
        for p in &f.params {
            let Some(t) = &p.ty else {
                return err(
                    "type-fn-param",
                    format!("parameter `{}` of `fn {}` needs a type", p.name, f.name),
                    p.span,
                );
            };
            params.push(shapes.resolve(t)?);
        }
        let ret = shapes.resolve(&f.ret)?;
        shapes.fns.insert(f.name.clone(), (params, ret));
    }
    for f in &file.fns {
        let (params, ret) = shapes.fns[&f.name].clone();
        let mut scope = Scope::default();
        scope.push(
            f.params
                .iter()
                .enumerate()
                .map(|(i, p)| (p.name.clone(), Ref::Local(i as u32), params[i].clone()))
                .collect(),
        );
        let t = infer(&f.body, &scope, &shapes)?;
        if ret.unify(&t).is_none() {
            return err(
                "type-fn-return",
                format!("`fn {}` declares `{ret}` but its body is `{t}`", f.name),
                f.body.span(),
            );
        }
    }
    {
        // Acyclic: depth-first over the calls each body makes to other fns.
        let graph: BTreeMap<&str, Vec<String>> = file
            .fns
            .iter()
            .map(|f| {
                let mut out = Vec::new();
                calls_in(&f.body, &mut out);
                (f.name.as_str(), out)
            })
            .collect();
        fn visit(
            name: &str,
            graph: &BTreeMap<&str, Vec<String>>,
            path: &mut Vec<String>,
        ) -> Option<Vec<String>> {
            if path.iter().any(|p| p == name) {
                path.push(name.to_string());
                return Some(path.clone());
            }
            path.push(name.to_string());
            for callee in graph.get(name).into_iter().flatten() {
                if graph.contains_key(callee.as_str()) {
                    if let Some(cycle) = visit(callee, graph, path) {
                        return Some(cycle);
                    }
                }
            }
            path.pop();
            None
        }
        for f in &file.fns {
            if let Some(cycle) = visit(&f.name, &graph, &mut Vec::new()) {
                return err(
                    "type-fn-recursive",
                    format!(
                        "`fn {}` calls itself ({}): a fn is expanded where it is called, so it cannot recurse — a traversal is the data crate's",
                        f.name,
                        cycle.join(" → ")
                    ),
                    f.span,
                );
            }
        }
    }
    let mut types = Types {
        shapes,
        components: Vec::new(),
    };
    // The root is checked against its inlined view, so a handler's real call
    // site (behind a child's prop) types the action's parameters; children
    // are checked standalone as views over their props.
    // The expanded root (LLP 1017 P4c): the inlined view plus every stateful
    // child's own declarations, lifted in — what lowering will lower.
    let expanded = contract_syntax::expand(file).map_err(|e| TypeError {
        id: e.id,
        message: e.message,
        span: e.span,
    })?;
    // A child may own `state`, `derive`, and `action` (LLP 1017 P4c: its
    // instances' own), never a `resource`, `mutation`, or `task` — a row
    // must not open N requests, and only the root has a clock. Checked
    // before the root, whose inlined view would otherwise trip on the
    // child's unknown name first.
    for c in file.components.iter().skip(1) {
        if !c.resources.is_empty() || !c.mutations.is_empty() || !c.tasks.is_empty() {
            let span = c
                .resources
                .first()
                .map(|r| r.span)
                .or(c.mutations.first().map(|m| m.span))
                .or(c.tasks.first().map(|t| t.span))
                .unwrap_or(c.span);
            return err(
                "type-child-resource",
                format!("component `{}` takes props: a resource, mutation, or task lives in the root (a child may own state, derives, and actions)", c.name),
                span,
            );
        }
    }
    for (i, c) in file.components.iter().enumerate() {
        let ct = if i == 0 {
            check_component(&expanded.root, &types, Some(&expanded.owners))?
        } else {
            check_component(c, &types, None)?
        };
        types.components.push(ct);
    }
    // Component uses: arguments must match props.
    for c in &file.components {
        let ct = &types.components[file
            .components
            .iter()
            .position(|x| x.name == c.name)
            .unwrap()];
        let scoped = if c.name == expanded.root.name {
            &expanded.root
        } else {
            c
        };
        check_uses(&c.view, &types.component_scope(scoped, ct), &types, file)?;
    }
    let root = &file.components[0];
    check_injects(
        &root.view,
        &types.component_scope(&expanded.root, &types.components[0]),
        &types,
        file,
    )?;
    Ok(types)
}

fn check_uses(nodes: &[Node], scope: &Scope, types: &Types, file: &File) -> Result<(), TypeError> {
    for n in nodes {
        match n {
            Node::Use {
                name,
                args,
                children,
                span,
            } => {
                check_uses(children, scope, types, file)?;
                let Some(target) = file.components.iter().position(|c| &c.name == name) else {
                    return err(
                        "type-unknown-component",
                        format!("unknown component `{name}`"),
                        *span,
                    );
                };
                let target_c = &file.components[target];
                let target_t = &types.components[target];
                for (i, p) in target_c.props.iter().enumerate() {
                    let Some(arg) = args.iter().find(|a| a.name == p.name) else {
                        return err(
                            "type-missing-prop",
                            format!("`{name}` needs `{}`", p.name),
                            *span,
                        );
                    };
                    let t = infer(&arg.value, scope, &types.shapes)?;
                    if t.unify(&target_t.props[i]).is_none() {
                        return err(
                            "type-prop",
                            format!("`{}` expects `{}`, given `{t}`", p.name, target_t.props[i]),
                            arg.span,
                        );
                    }
                }
                for a in args {
                    if !target_c.props.iter().any(|p| p.name == a.name) {
                        return err(
                            "type-unknown-prop",
                            format!("`{name}` has no prop `{}`", a.name),
                            a.span,
                        );
                    }
                }
            }
            Node::Element { children, .. } => check_uses(children, scope, types, file)?,
            Node::Provide { expr, body, .. } => {
                infer(expr, scope, &types.shapes)?;
                check_uses(body, scope, types, file)?;
            }
            Node::Children { .. } => {}
            Node::When {
                then, otherwise, ..
            } => {
                check_uses(then, scope, types, file)?;
                check_uses(otherwise, scope, types, file)?;
            }
            Node::Each {
                var, list, body, ..
            } => {
                let lt = infer(list, scope, &types.shapes)?;
                let Ty::List(item) = lt else {
                    return err(
                        "type-each-list",
                        format!("`each` needs a list, given `{lt}`"),
                        list.span(),
                    );
                };
                let mut inner = scope.clone();
                inner.push_region(Some((var.clone(), Ref::Item(0), *item)));
                check_uses(body, &inner, types, file)?;
            }
            Node::Match {
                subject,
                some,
                none,
                ..
            } => {
                let st = infer(subject, scope, &types.shapes)?;
                let Ty::Option(item) = st else {
                    return err(
                        "type-match-subject",
                        format!("`match` needs an option, given `{st}`"),
                        subject.span(),
                    );
                };
                let mut inner = scope.clone();
                inner.push_region(Some((some.0.clone(), Ref::Bound(0), *item)));
                check_uses(&some.1, &inner, types, file)?;
                let mut none_scope = scope.clone();
                none_scope.push_region(None);
                check_uses(none, &none_scope, types, file)?;
            }
        }
    }
    Ok(())
}

fn check_component(
    c: &Component,
    types: &Types,
    owners: Option<&[Option<u32>]>,
) -> Result<ComponentTypes, TypeError> {
    let shapes = &types.shapes;
    let mut ct = ComponentTypes {
        name: c.name.clone(),
        ..ComponentTypes::default()
    };
    // Duplicate names across all declarations.
    let mut seen = BTreeMap::new();
    for (name, span) in c
        .props
        .iter()
        .map(|p| (&p.name, p.span))
        .chain(c.injects.iter().map(|p| (&p.name, p.span)))
        .chain(c.states.iter().map(|s| (&s.name, s.span)))
        .chain(c.derives.iter().map(|d| (&d.name, d.span)))
        .chain(c.resources.iter().map(|r| (&r.name, r.span)))
        .chain(c.mutations.iter().map(|m| (&m.name, m.span)))
        .chain(c.actions.iter().map(|a| (&a.name, a.span)))
    {
        if seen.insert(name.clone(), span).is_some() {
            return err(
                "type-duplicate-name",
                format!("`{name}` declared twice"),
                span,
            );
        }
    }
    for p in &c.props {
        let ty = match &p.ty {
            Some(t) => shapes.resolve(t)?,
            None => {
                return err(
                    "type-prop-untyped",
                    format!("prop `{}` needs a type", p.name),
                    p.span,
                )
            }
        };
        ct.props.push(ty);
    }
    for p in &c.injects {
        let Some(t) = &p.ty else {
            return err(
                "type-inject-untyped",
                format!("inject `{}` needs a type", p.name),
                p.span,
            );
        };
        ct.props.push(shapes.resolve(t)?);
    }
    for r in &c.resources {
        ct.resources.push(shapes.resolve(&r.shape)?);
    }
    for m in &c.mutations {
        ct.mutations.push(shapes.resolve(&m.shape)?);
    }
    // Slots from initializers (may hold `?` inside an option).
    {
        let mut scope = Scope::default();
        let mut names: Vec<(String, Ref, Ty)> = c
            .props
            .iter()
            .enumerate()
            .map(|(i, p)| (p.name.clone(), Ref::Prop(i as u32), ct.props[i].clone()))
            .collect();
        for (j, p) in c.injects.iter().enumerate() {
            let i = c.props.len() + j;
            names.push((p.name.clone(), Ref::Prop(i as u32), ct.props[i].clone()));
        }
        for (i, s) in c.states.iter().enumerate() {
            scope.frames_reset(&names);
            let t = if i == 0 && owners.is_some() && shapes.routes.is_some() {
                Ty::Record("Router".into())
            } else if owners
                .and_then(|owners| owners.get(i))
                .is_some_and(Option::is_some)
            {
                Ty::Unknown
            } else {
                infer(&s.expr, &scope, shapes)?
            };
            names.push((s.name.clone(), Ref::Slot(i as u32), t.clone()));
            ct.slots.push(t);
        }
    }
    // Actions: parameters (declared or `?`), then refine slots from writes.
    for a in &c.actions {
        let mut params = Vec::new();
        for p in &a.params {
            params.push(match &p.ty {
                Some(t) => shapes.resolve(t)?,
                None => Ty::Unknown,
            });
        }
        ct.actions.push(params);
    }
    // Derives: iterate to a fixpoint so order does not matter and `?` fills.
    ct.derives = vec![Ty::Unknown; c.derives.len()];
    for _round in 0..(c.derives.len() + 2) {
        let scope = types_scope(c, &ct, types);
        let mut changed = false;
        for (i, d) in c.derives.iter().enumerate() {
            match infer(&d.expr, &scope, shapes) {
                Ok(t) => {
                    if t != ct.derives[i] {
                        ct.derives[i] = t;
                        changed = true;
                    }
                }
                Err(e)
                    if e.id == "type-unknown-name"
                        && c.derives
                            .iter()
                            .any(|x| e.message.contains(&format!("`{}`", x.name))) => {}
                // An expression over a derive this round has not typed yet
                // (`current.ok` while `current` is still `?`): the next round
                // has it, and the strict pass below reports what never types.
                Err(e) if e.message.contains("`?`") => {}
                Err(e) => return Err(e),
            }
        }
        if !changed {
            break;
        }
    }
    // Everything must now type; re-infer derives strictly to surface errors.
    let scope = types_scope(c, &ct, types);
    for (i, d) in c.derives.iter().enumerate() {
        ct.derives[i] = infer(&d.expr, &scope, shapes)?;
        if !ct.derives[i].is_complete() {
            return err(
                "type-derive-cycle",
                format!(
                    "cannot infer the type of `{}`: it depends on itself through other derives",
                    d.name
                ),
                d.span,
            );
        }
    }
    for r in &c.resources {
        for arg in &r.args {
            infer(arg, &scope, shapes)?;
        }
    }
    infer_owned_state_initializers(c, &mut ct, types, owners)?;
    // Handler call sites give untyped parameters their types.
    // Row initializers have just resolved the lifted child slots. Curried
    // action-prop arguments must see those types too, not the earlier scope.
    let scope = types_scope(c, &ct, types);
    refine_params_from_view(&c.view, &scope, c, &mut ct, shapes)?;
    // Action bodies: writes refine slots; assignments must unify.
    for (ai, a) in c.actions.iter().enumerate() {
        let mut scope = types_scope(c, &ct, types);
        scope.push(
            a.params
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    (
                        p.name.clone(),
                        Ref::Param(i as u32),
                        ct.actions[ai][i].clone(),
                    )
                })
                .collect(),
        );
        check_stmts(&a.body, &scope, c, &mut ct, shapes)?;
    }
    // The seam's signatures (LLP 1027 D2): every resource's arguments against
    // the final scope, unified with the sends' (recorded as their bodies were
    // checked). One source, one signature.
    {
        let scope = types_scope(c, &ct, types);
        for (i, r) in c.resources.iter().enumerate() {
            let mut params = Vec::with_capacity(r.args.len());
            for arg in &r.args {
                params.push(infer(arg, &scope, shapes)?);
            }
            let result = ct.resources[i].clone();
            record_source(&mut ct, &r.source, params, result, r.span)?;
        }
    }
    for (i, s) in c.states.iter().enumerate() {
        if !ct.slots[i].is_complete() {
            return err(
                "type-cannot-infer",
                format!(
                    "cannot infer the type of `{}`: nothing writes a value into it",
                    s.name
                ),
                s.span,
            );
        }
    }
    for (ai, a) in c.actions.iter().enumerate() {
        for (i, p) in a.params.iter().enumerate() {
            if !ct.actions[ai][i].is_complete() {
                return err("type-cannot-infer", format!("cannot infer the type of parameter `{}`; write `{}: <type>` or call the action from a handler", p.name, p.name), p.span);
            }
        }
    }
    // The view types.
    let scope = types_scope(c, &ct, types);
    check_view(&c.view, &scope, shapes)?;
    for t in &c.tasks {
        if infer(&t.every.0, &scope, shapes)? != Ty::Number {
            return err(
                "type-timer",
                "`every` needs a number of milliseconds",
                t.every.2,
            );
        }
    }
    Ok(ct)
}

fn types_scope(c: &Component, ct: &ComponentTypes, types: &Types) -> Scope {
    let mut ct = ct.clone();
    // Fill any not-yet-computed derive slots so the scope has every name.
    while ct.derives.len() < c.derives.len() {
        ct.derives.push(Ty::Unknown);
    }
    while ct.actions.len() < c.actions.len() {
        ct.actions.push(Vec::new());
    }
    types.component_scope(c, &ct)
}

impl Scope {
    fn frames_reset(&mut self, names: &[(String, Ref, Ty)]) {
        self.frames.clear();
        self.push(names.to_vec());
    }
}

fn refine_params_from_view(
    nodes: &[Node],
    scope: &Scope,
    c: &Component,
    ct: &mut ComponentTypes,
    shapes: &Shapes,
) -> Result<(), TypeError> {
    for n in nodes {
        match n {
            Node::Provide { body, .. } => refine_params_from_view(body, scope, c, ct, shapes)?,
            Node::Children { .. } => {}
            Node::Element {
                attrs, children, ..
            } => {
                for a in attrs {
                    if matches!(
                        a.name.as_str(),
                        "press"
                            | "change"
                            | "hover"
                            | "focus"
                            | "blur"
                            | "key"
                            | "submit"
                            | "load"
                            | "message"
                            | "contextmenu"
                            | "dblclick"
                            | "swiperight"
                            | "scroll"
                            | "navigate"
                    ) {
                        let (name, args): (&str, &[Expr]) = match &a.value {
                            Expr::Ident(n, _) => (n, &[]),
                            Expr::Call(n, args, _) => (n, args),
                            _ => continue,
                        };
                        if let Some(ai) = c.actions.iter().position(|x| x.name == name) {
                            for (i, arg) in args.iter().enumerate() {
                                if i < ct.actions[ai].len() {
                                    let t = infer(arg, scope, shapes)?;
                                    if let Some(u) = ct.actions[ai][i].unify(&t) {
                                        ct.actions[ai][i] = u;
                                    }
                                }
                            }
                            // Event payloads: change/key/message are strings;
                            // hover is whether the pointer is over.
                            let payload = match a.name.as_str() {
                                "change" | "key" | "message" | "navigate" => vec![Ty::String],
                                "hover" => vec![Ty::Bool],
                                "scroll" => vec![Ty::Number, Ty::Number],
                                _ => vec![],
                            };
                            let start = ct.actions[ai].len().saturating_sub(payload.len());
                            for (offset, ty) in payload.into_iter().enumerate() {
                                let last = start + offset;
                                if args.len() < ct.actions[ai].len() {
                                    let declared = ct.actions[ai][last].clone();
                                    let Some(unified) = declared.unify(&ty) else {
                                        return err(
                                            "type-handler-payload",
                                            format!(
                                                "`{}=` supplies `{ty}` to parameter `{}`, declared `{declared}`",
                                                a.name, c.actions[ai].params[last].name
                                            ),
                                            a.span,
                                        );
                                    };
                                    ct.actions[ai][last] = unified;
                                }
                            }
                        }
                    }
                }
                refine_params_from_view(children, scope, c, ct, shapes)?;
            }
            Node::Use { args, children, .. } => {
                refine_params_from_view(children, scope, c, ct, shapes)?;
                for a in args {
                    let _ = a;
                }
            }
            Node::When {
                then, otherwise, ..
            } => {
                refine_params_from_view(then, scope, c, ct, shapes)?;
                refine_params_from_view(otherwise, scope, c, ct, shapes)?;
            }
            Node::Each {
                var, list, body, ..
            } => {
                if let Ok(Ty::List(item)) = infer(list, scope, shapes) {
                    let mut inner = scope.clone();
                    inner.push_region(Some((var.clone(), Ref::Item(0), *item)));
                    refine_params_from_view(body, &inner, c, ct, shapes)?;
                }
            }
            Node::Match {
                subject,
                some,
                none,
                ..
            } => {
                if let Ok(Ty::Option(item)) = infer(subject, scope, shapes) {
                    let mut inner = scope.clone();
                    inner.push_region(Some((some.0.clone(), Ref::Bound(0), *item)));
                    refine_params_from_view(&some.1, &inner, c, ct, shapes)?;
                }
                let mut none_scope = scope.clone();
                none_scope.push_region(None);
                refine_params_from_view(none, &none_scope, c, ct, shapes)?;
            }
        }
    }
    Ok(())
}
