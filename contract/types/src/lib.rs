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

use contract_syntax::{
    BinOp, Component, Expr, File, Node, Span, Stmt, TemplatePart, TypeExpr, UnOp,
};
use exact_plan::Stdlib;
use std::collections::BTreeMap;

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
            _ => false,
        }
    }

    /// The roster's return spelling as a type.
    pub fn from_roster(spec: &str) -> Ty {
        match spec {
            "number" => Ty::Number,
            "string" => Ty::String,
            "bool" => Ty::Bool,
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
    /// Shape name → fields in order.
    pub map: BTreeMap<String, Vec<(String, Ty)>>,
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
                    } else if let Some(inner) = other.strip_prefix("action") {
                        // `action(string, number)` is parsed as Named("action") followed by a call;
                        // the parser gives us only the name here, so accept bare `action` as any-arity.
                        let _ = inner;
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
            if let Some(f) = Stdlib::from_name(name) {
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
                Ty::from_roster(f.returns())
            } else if let Some((Ref::Action(_) | Ref::Prop(_), Ty::Action(params))) =
                scope.lookup(name)
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

/// Check a file: shapes, then every component.
pub fn check(file: &File) -> Result<Types, TypeError> {
    let mut shapes = Shapes::default();
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
    for s in &file.shapes {
        let mut fields = Vec::new();
        for f in &s.fields {
            fields.push((f.name.clone(), shapes.resolve(&f.ty)?));
        }
        shapes.map.insert(s.name.clone(), fields);
    }
    let mut types = Types {
        shapes,
        components: Vec::new(),
    };
    // The root is checked against its inlined view, so a handler's real call
    // site (behind a child's prop) types the action's parameters; children
    // are checked standalone as views over their props.
    let inlined = contract_syntax::inline(file).map_err(|e| TypeError {
        id: e.id,
        message: e.message,
        span: e.span,
    })?;
    for (i, c) in file.components.iter().enumerate() {
        let ct = if i == 0 {
            let mut root = c.clone();
            root.view = inlined.clone();
            check_component(&root, &types)?
        } else {
            check_component(c, &types)?
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
        check_uses(&c.view, &types.component_scope(c, ct), &types, file)?;
    }
    Ok(types)
}

fn check_uses(nodes: &[Node], scope: &Scope, types: &Types, file: &File) -> Result<(), TypeError> {
    for n in nodes {
        match n {
            Node::Use { name, args, span } => {
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

fn check_component(c: &Component, types: &Types) -> Result<ComponentTypes, TypeError> {
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
        for (i, s) in c.states.iter().enumerate() {
            scope.frames_reset(&names);
            let t = infer(&s.expr, &scope, shapes)?;
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
    // Handler call sites give untyped parameters their types.
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
            Node::Element {
                attrs, children, ..
            } => {
                for a in attrs {
                    if matches!(
                        a.name.as_str(),
                        "press" | "change" | "hover" | "focus" | "blur" | "key" | "submit"
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
                            // The event's payload is the action's last parameter:
                            // `change` the new text, `key` the key's name (strings),
                            // `hover` whether the pointer is over (a bool).
                            let payload = match a.name.as_str() {
                                "change" | "key" => Some(Ty::String),
                                "hover" => Some(Ty::Bool),
                                _ => None,
                            };
                            if let Some(ty) = payload {
                                let last = ct.actions[ai].len().saturating_sub(1);
                                if args.len() < ct.actions[ai].len() {
                                    if let Some(u) = ct.actions[ai][last].unify(&ty) {
                                        ct.actions[ai][last] = u;
                                    }
                                }
                            }
                        }
                    }
                }
                refine_params_from_view(children, scope, c, ct, shapes)?;
            }
            Node::Use { args, .. } => {
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

/// An action body's statements, through every branch (LLP 1017 P2): an
/// assignment unifies its slot's type; a `send` names a mutation and a
/// `refresh` a resource; an `if` needs a bool; a `match` needs an option
/// and binds its `some` name as a local, as the inline `match` does.
fn check_stmts(
    stmts: &[Stmt],
    scope: &Scope,
    c: &Component,
    ct: &mut ComponentTypes,
    shapes: &Shapes,
) -> Result<(), TypeError> {
    for stmt in stmts {
        match stmt {
            Stmt::Assign { target, expr, span } => {
                let Some(si) = c.states.iter().position(|s| &s.name == target) else {
                    // A mutation's slot may be assigned (`session = none`);
                    // its type is `option<T>` and is never inferred from here.
                    if let Some(mi) = c.mutations.iter().position(|m| &m.name == target) {
                        let t = infer(expr, scope, shapes)?;
                        let mt = Ty::Option(Box::new(ct.mutations[mi].clone()));
                        if mt.unify(&t).is_none() {
                            return err(
                                "type-assign",
                                format!("`{target}` is `{mt}`, cannot assign `{t}`"),
                                *span,
                            );
                        }
                        continue;
                    }
                    return err(
                        "type-assign-not-state",
                        format!("`{target}` is not a state or a mutation"),
                        *span,
                    );
                };
                let t = infer(expr, scope, shapes)?;
                match ct.slots[si].unify(&t) {
                    Some(u) => ct.slots[si] = u,
                    None => {
                        return err(
                            "type-assign",
                            format!("`{target}` is `{}`, cannot assign `{t}`", ct.slots[si]),
                            *span,
                        )
                    }
                }
            }
            Stmt::Command { args, .. } => {
                for arg in args {
                    infer(arg, scope, shapes)?;
                }
            }
            Stmt::Send {
                target, args, span, ..
            } => {
                if !c.mutations.iter().any(|m| &m.name == target) {
                    return err(
                        "type-send-not-mutation",
                        format!(
                            "`{target}` is not a mutation: declare `mutation {target} as shape T`"
                        ),
                        *span,
                    );
                }
                for arg in args {
                    infer(arg, scope, shapes)?;
                }
            }
            Stmt::Refresh { target, span } => {
                if !c.resources.iter().any(|r| &r.name == target) {
                    return err(
                        "type-refresh-not-resource",
                        format!("`{target}` is not a resource"),
                        *span,
                    );
                }
            }
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                if infer(cond, scope, shapes)? != Ty::Bool {
                    return err("type-condition", "`if` needs a bool", cond.span());
                }
                check_stmts(then, scope, c, ct, shapes)?;
                check_stmts(otherwise, scope, c, ct, shapes)?;
            }
            Stmt::Match {
                subject,
                some,
                none,
                ..
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
                inner_scope.push(vec![(some.0.clone(), Ref::Local(0), (*inner).clone())]);
                check_stmts(&some.1, &inner_scope, c, ct, shapes)?;
                check_stmts(none, scope, c, ct, shapes)?;
            }
        }
    }
    Ok(())
}

fn check_view(nodes: &[Node], scope: &Scope, shapes: &Shapes) -> Result<(), TypeError> {
    for n in nodes {
        match n {
            Node::Element {
                positional,
                attrs,
                children,
                ..
            } => {
                for p in positional {
                    infer(p, scope, shapes)?;
                }
                for a in attrs {
                    if a.name == "surface" {
                        // `surface=name(args)`: the name is the GPU module's,
                        // not a function; the arguments are expressions.
                        if let Expr::Call(_, args, _) = &a.value {
                            for arg in args {
                                infer(arg, scope, shapes)?;
                            }
                        }
                        continue;
                    }
                    infer(&a.value, scope, shapes)?;
                }
                check_view(children, scope, shapes)?;
            }
            Node::Use { args, .. } => {
                for a in args {
                    infer(&a.value, scope, shapes)?;
                }
            }
            Node::When {
                cond,
                then,
                otherwise,
                ..
            } => {
                if infer(cond, scope, shapes)? != Ty::Bool {
                    return err("type-condition", "`when` needs a bool", cond.span());
                }
                check_view(then, scope, shapes)?;
                check_view(otherwise, scope, shapes)?;
            }
            Node::Each {
                var,
                list,
                key,
                body,
                ..
            } => {
                let lt = infer(list, scope, shapes)?;
                let Ty::List(item) = lt else {
                    return err(
                        "type-each-list",
                        format!("`each` needs a list, given `{lt}`"),
                        list.span(),
                    );
                };
                let mut inner = scope.clone();
                inner.push_region(Some((var.clone(), Ref::Item(0), *item)));
                let kt = infer(key, &inner, shapes)?;
                if !matches!(kt, Ty::String | Ty::Number | Ty::Bool) {
                    return err(
                        "type-each-key",
                        format!("a key must be a string, number, or bool, not `{kt}`"),
                        key.span(),
                    );
                }
                check_view(body, &inner, shapes)?;
            }
            Node::Match {
                subject,
                some,
                none,
                ..
            } => {
                let st = infer(subject, scope, shapes)?;
                let Ty::Option(item) = st else {
                    return err(
                        "type-match-subject",
                        format!("`match` needs an option, given `{st}`"),
                        subject.span(),
                    );
                };
                let mut inner = scope.clone();
                inner.push_region(Some((some.0.clone(), Ref::Bound(0), *item)));
                check_view(&some.1, &inner, shapes)?;
                let mut none_scope = scope.clone();
                none_scope.push_region(None);
                check_view(none, &none_scope, shapes)?;
            }
        }
    }
    Ok(())
}
