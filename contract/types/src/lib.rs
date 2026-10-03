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

mod actions;
mod checks;
mod choices;
mod component;
mod geometry;
mod lists;
/// Router declaration checking and compile-time path expansion (LLP 1038 D2/D3).
pub mod placeholder;
mod posts;
pub mod records;
pub mod routes;
mod selection;
/// The strings call and the tables it is checked against (LLP 1060).
pub mod strings;
mod uses;

use contract_syntax::{BinOp, Component, Expr, File, Span, TemplatePart, TypeExpr, UnOp};
use exact_plan::Stdlib;
use std::{collections::BTreeMap, sync::Arc};

use checks::{check_injects, check_shape_cycles};

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
    /// `"a" | "b"` (LLP 1035.005.000 D4a): one of these strings, sorted and
    /// distinct, so two spellings of one set are one type.
    Choice(Vec<String>),
    /// An action reference with its parameter types.
    Action(Vec<Ty>),
    /// Not yet known (only inside an `option` from `none`, a `list` from
    /// `[]`, or an untyped parameter).
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
            Ty::Choice(literals) => write!(f, "{}", choices::spell(literals)),
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
            "string" => self.is_text(),
            "bool" => *self == Ty::Bool,
            "any" => self.is_text() || matches!(self, Ty::Number | Ty::Bool | Ty::List(_)),
            _ => *self == Self::from_roster(spec) && *self != Ty::Unknown,
        }
    }

    /// The roster's return spelling as a type.
    pub fn from_roster(spec: &str) -> Ty {
        match spec {
            "number" => Ty::Number,
            "string" => Ty::String,
            "bool" => Ty::Bool,
            "Router" | "Entry" | "Geometry" => Ty::Record(spec.into()),
            "list<Entry>" => Ty::List(Box::new(Ty::Record("Entry".into()))),
            "list<string>" => Ty::List(Box::new(Ty::String)),
            _ => Ty::Unknown,
        }
    }
}

/// Whether a roster parameter admits `t`. The roster table spells three
/// parameters `any`; each is held to the values its runner entry reads, so a
/// call that would fail at runtime (`length(5)`) is refused here instead.
fn roster_accepts(f: Stdlib, spec: &str, t: &Ty) -> bool {
    match (f, spec) {
        (Stdlib::Length | Stdlib::IsEmpty, "any") => t.is_text() || matches!(t, Ty::List(_)),
        (Stdlib::ToString, "any") => t.is_text() || matches!(t, Ty::Number | Ty::Bool),
        (Stdlib::First | Stdlib::At, "any") => matches!(t, Ty::List(_)),
        _ => t.matches_roster(spec),
    }
}

/// A roster parameter spelled as string literals (`"medium" | "month-year"`)
/// takes one of them, written as a literal: a style is chosen where the call
/// is written, never computed or forwarded (@ref LLP 1054.000.003 D9; the
/// precedent is `path()`'s route name).
fn literal_argument(name: &str, i: usize, spec: &str, arg: &Expr) -> Result<(), TypeError> {
    let written = match arg {
        Expr::Str(value, _) => {
            let quoted = format!("\"{value}\"");
            if spec.split(" | ").any(|choice| choice == quoted) {
                return Ok(());
            }
            format!("`{quoted}`")
        }
        _ => "an expression".into(),
    };
    err(
        "type-format-style",
        format!(
            "argument {} of `{name}` is one of {}, written as a string literal; given {written}",
            i + 1,
            spec.split(" | ")
                .map(|c| format!("`{c}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        arg.span(),
    )
}

/// A roster parameter as a refusal spells it.
fn roster_spelling(f: Stdlib, spec: &str) -> &str {
    match (f, spec) {
        (Stdlib::Length | Stdlib::IsEmpty, "any") => "string | list",
        (Stdlib::ToString, "any") => "number | string | bool",
        (Stdlib::First | Stdlib::At, "any") => "list",
        _ => spec,
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

/// At most this many refusals from one check: enough to repair a file in
/// one pass, few enough to read.
pub const MAX_REFUSALS: usize = 20;

/// Refusals from one check, in the order they were found. A refusal that
/// mentions `?` after another is a consequence of it, and a repeat is not
/// news: neither is kept.
#[derive(Debug, Default)]
pub(crate) struct Sink {
    pub(crate) errors: Vec<TypeError>,
}

impl Sink {
    pub(crate) fn push(&mut self, e: TypeError) {
        // The use checks' refusal of a missing prop or an unknown component
        // repeats expansion's at the same use.
        fn same(id: &'static str) -> &'static str {
            match id {
                "type-missing-prop" => "syntax-missing-prop",
                "type-unknown-component" => "syntax-unknown-component",
                id => id,
            }
        }
        if self.errors.len() >= MAX_REFUSALS
            || (!self.errors.is_empty() && e.message.contains("`?`"))
            || self
                .errors
                .iter()
                .any(|x| same(x.id) == same(e.id) && x.span == e.span)
        {
            return;
        }
        self.errors.push(e);
    }

    /// The type, or `?` once the refusal is recorded.
    pub(crate) fn keep(&mut self, result: Result<Ty, TypeError>) -> Ty {
        result.unwrap_or_else(|e| {
            self.push(e);
            Ty::Unknown
        })
    }

    pub(crate) fn keep_unit(&mut self, result: Result<(), TypeError>) {
        if let Err(e) = result {
            self.push(e);
        }
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
#[derive(Debug, Clone, Default)]
pub struct Shapes {
    /// Checked app route table, when declared. @ref LLP 1038 D2/D3.
    pub routes: Option<exact_route::Table>,
    /// Shape name → fields in order.
    pub map: BTreeMap<String, Vec<(String, Ty)>>,
    /// The shapes the app declares, which `Shape(field=…)` builds
    /// (LLP 1035.005.000 D3); the compiler's own are left out.
    pub declared: std::collections::BTreeSet<String>,
    /// `fn` name → (parameter types, result type) (LLP 1017 P5).
    pub fns: BTreeMap<String, (Vec<Ty>, Ty)>,
    /// Which attribute names set style rows (lowering's table): their
    /// branches may mix a number and a string, one CSS value space.
    pub style_attr: Option<fn(&str) -> bool>,
    /// The app's strings tables, when it has them (LLP 1060 D1).
    pub strings: Option<Arc<strings::Strings>>,
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
                        return Err(self.unknown_type(other, *span));
                    }
                }
            },
            TypeExpr::Option(inner, _) => Ty::Option(Box::new(self.resolve(inner)?)),
            TypeExpr::List(inner, _) => Ty::List(Box::new(self.resolve(inner)?)),
            TypeExpr::Choice(literals, _) => Ty::choice(literals),
        })
    }

    /// A field's position and borrowed type in a shape.
    pub fn field(&self, shape: &str, name: &str) -> Option<(usize, &Ty)> {
        self.map
            .get(shape)?
            .iter()
            .enumerate()
            .find(|(_, (n, _))| n == name)
            .map(|(i, (_, ty))| (i, ty))
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
    /// An `each` item's position, `depth` region frames out (LLP 1062 D8).
    Index(u32),
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
    // Branches own their stacks, and shared frames remain immutable.
    frames: Vec<Arc<Frame>>,
    // Inside an action's body, where geometry reads are allowed (LLP 1051.000 D2).
    action: bool,
}

impl Scope {
    /// Mark this scope as an action's body: geometry reads are allowed here
    /// and nowhere else (LLP 1051.000 D2).
    pub fn enter_action(&mut self) {
        self.action = true;
    }

    /// Leave an action's body (a walker reusing one scope across bodies).
    pub fn leave_action(&mut self) {
        self.action = false;
    }

    /// Whether this scope is an action's body.
    pub fn in_action(&self) -> bool {
        self.action
    }

    /// Push a non-region frame (component declarations, action parameters).
    pub fn push(&mut self, names: Vec<(String, Ref, Ty)>) {
        self.frames.push(Arc::new(Frame {
            names,
            region: false,
        }));
    }

    /// Push a region frame binding at most one name (`each` item or `match` binding).
    pub fn push_region(&mut self, name: Option<(String, Ref, Ty)>) {
        self.frames.push(Arc::new(Frame {
            names: name.into_iter().collect(),
            region: true,
        }));
    }

    /// Push an `each` row's frame: its item, and its position when named.
    pub fn push_each(&mut self, item: &str, index: Option<&str>, ty: Ty) {
        let mut names = vec![(item.to_string(), Ref::Item(0), ty)];
        names.extend(index.map(|i| (i.to_string(), Ref::Index(0), Ty::Number)));
        self.frames.push(Arc::new(Frame {
            names,
            region: true,
        }));
    }

    /// The one value name in scope `name` most plausibly misspells. Actions
    /// are the driver's to suggest (it knows the handler's position), and a
    /// generated name (`count#2`, `x@1`) is never offered.
    pub fn suggest(&self, name: &str) -> Option<&str> {
        let names = self
            .frames
            .iter()
            .flat_map(|f| f.names.iter())
            .filter_map(|(n, _, t)| {
                (!matches!(t, Ty::Action(_)) && !n.contains(['#', '@'])).then_some(n.as_str())
            });
        contract_syntax::suggestion(name, names)
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
    /// innermost region frame. The type borrows its immutable scope frame.
    pub fn lookup(&self, name: &str) -> Option<(Ref, &Ty)> {
        let mut depth = 0u32;
        for frame in self.frames.iter().rev() {
            for (n, r, t) in &frame.names {
                if n == name {
                    let r = match r {
                        Ref::Item(_) => Ref::Item(depth),
                        Ref::Index(_) => Ref::Index(depth),
                        Ref::Bound(_) => Ref::Bound(depth),
                        other => *other,
                    };
                    return Some((r, t));
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
        // Arguments agree as branches do: a choice beside a string is a string.
        match a.join(b) {
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

/// A checked component file and the exact expansion its types describe.
/// Later passes borrow this result instead of repeating component expansion.
pub struct Checked<'a> {
    /// Authored declarations, retained for scopes and source locations.
    pub file: &'a File,
    /// Inferred types, including the expanded root's lifted declarations.
    pub types: Types,
    /// The root and row ownership used during inference.
    pub expanded: contract_syntax::Expanded,
}

/// Everything the checker learned.
#[derive(Debug, Clone, Default)]
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

/// A source's argument. The seam's signature (LLP 1027 D2) is built from
/// the call sites, so a literal `[]` or `none` (or `some(…)` of one) passed
/// to a source has nothing to complete its `?` and is refused here, at the
/// argument. Any other argument is inferred as it stands: a state a later
/// action writes is `?` now and complete once every body has been checked,
/// which the seam's final pass confirms.
pub(crate) fn source_argument(
    arg: &Expr,
    source: &str,
    scope: &Scope,
    shapes: &Shapes,
) -> Result<Ty, TypeError> {
    fn untyped_literal(e: &Expr) -> bool {
        match e {
            Expr::EmptyList(_) | Expr::None(_) => true,
            Expr::Some(inner, _) => untyped_literal(inner),
            _ => false,
        }
    }
    let t = infer(arg, scope, shapes)?;
    if t.is_complete() || !untyped_literal(arg) {
        return Ok(t);
    }
    err(
        "type-cannot-infer",
        format!(
            "cannot infer what `{t}` passes to `{source}`: nothing here says its type; pass a typed value (a state, `some(x)`, a list from a source or `map`/`filter`)"
        ),
        arg.span(),
    )
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
                    if !t.is_text() && !matches!(t, Ty::Number | Ty::Bool) {
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
        // `[]` is a `list<?>` as `none` is an `option<?>`: the other arm of a
        // `match` or `?:`, a declared `list<T>`, or a write into the state
        // it initializes fills the `?` through `unify`.
        Expr::EmptyList(_) => Ty::List(Box::new(Ty::Unknown)),
        Expr::Some(inner, _) => Ty::Option(Box::new(infer(inner, scope, shapes)?)),
        Expr::NamedArg(_, _, span) => {
            return err(
                "type-named-argument",
                "named arguments belong to a canvas surface binding",
                *span,
            )
        }
        Expr::Ident(name, span) => match scope.lookup(name) {
            Some((_, t)) => t.clone(),
            None => {
                let hint = if name.contains('-') {
                    " (a name may contain hyphens, as in CSS, so subtraction between two names needs spaces: `a - b`)".to_owned()
                } else {
                    scope
                        .suggest(name)
                        .map(|guess| format!("; did you mean `{guess}`?"))
                        .unwrap_or_default()
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
                    Some((_, ft)) => ft.clone(),
                    None => return Err(shapes.unknown_field(shape, field, *span)),
                },
                other => {
                    // `xs.length`, `xs.map`: the web's properties and methods.
                    let fix = match field.as_str() {
                        "length" | "map" | "filter" | "join" | "includes" | "startsWith"
                        | "endsWith" => {
                            format!(": {}", contract_syntax::idioms::method_fix(field))
                        }
                        _ => String::new(),
                    };
                    return err(
                        "type-not-a-record",
                        format!("`{other}` has no fields{fix}"),
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
            if strings::is_text_call(name, scope) {
                return strings::check_call(args, *span, scope, shapes);
            }
            if name == "failed" {
                // Like `pending`, this reads a resource's status, not its value.
                let [Expr::Ident(target, tspan)] = args.as_slice() else {
                    return err(
                        "type-failed-argument",
                        "`failed(x)` names one resource",
                        *span,
                    );
                };
                return match scope.lookup(target) {
                    Some((Ref::Resource(_), _)) => Ok(Ty::Bool),
                    _ => err(
                        "type-failed-argument",
                        format!("`{target}` is not a resource"),
                        *tspan,
                    ),
                };
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
            if records::is_record_call(name, shapes) {
                return records::infer_record(name, args, *span, scope, shapes);
            }
            if let Some((params, ret)) = shapes.fns.get(name) {
                // A `fn` (LLP 1017 P5): typed like a roster call.
                if args.len() != params.len() {
                    return err(
                        "type-arity",
                        checks::call_arity(name, args.len(), params),
                        *span,
                    );
                }
                for (i, (arg, want)) in args.iter().zip(params).enumerate() {
                    let t = choices::given(want, arg, scope, shapes)?;
                    if !want.accepts(&t) {
                        return err(
                            "type-argument",
                            format!(
                                "argument {} of `{name}` expects `{want}`, given `{t}`{}",
                                i + 1,
                                choices::hint(want, &t)
                            ),
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
                    let t = choices::given(pt, arg, scope, shapes)?;
                    if !pt.accepts(&t) {
                        return err(
                            "type-argument",
                            format!(
                                "`{name}` expects `{pt}`, given `{t}`{}",
                                choices::hint(pt, &t)
                            ),
                            arg.span(),
                        );
                    }
                }
                // A bare `action` prop types nothing it is given: each
                // argument is still checked as an expression on its own.
                for arg in args.iter().skip(params.len()) {
                    infer(arg, scope, shapes)?;
                }
                Ty::Action(params.iter().skip(args.len()).cloned().collect())
            } else if let Some(f) = Stdlib::from_name(name).filter(|f| lists::is_list_op(*f)) {
                return lists::infer_call(f, args, *span, scope, shapes);
            } else if let Some(f) = Stdlib::from_name(name) {
                routes::require_table(f, shapes, *span)?;
                geometry::check_call(f, args, scope, *span)?;
                if args.len() != f.arity() {
                    return err(
                        "type-arity",
                        checks::call_arity(
                            name,
                            args.len(),
                            f.params().iter().map(|spec| roster_spelling(f, spec)),
                        ),
                        *span,
                    );
                }
                let mut given = Vec::with_capacity(args.len());
                for (i, (arg, spec)) in args.iter().zip(f.params()).enumerate() {
                    if spec.starts_with('"') {
                        literal_argument(name, i, spec, arg)?;
                        given.push(Ty::String);
                        continue;
                    }
                    let t = infer(arg, scope, shapes)?;
                    given.push(t.clone());
                    if !roster_accepts(f, spec, &t) {
                        return err(
                            "type-argument",
                            format!(
                                "argument {} of `{name}` expects `{}`, given `{t}`",
                                i + 1,
                                roster_spelling(f, spec)
                            ),
                            arg.span(),
                        );
                    }
                }
                routes::location(f, args, shapes)?;
                match (f, given.first()) {
                    // `first(list<T>)` is `option<T>` (LLP 1054.000 C4).
                    (Stdlib::First, Some(Ty::List(item))) => Ty::Option(item.clone()),
                    // `at(list<T>, number)` is `option<T>` (LLP 1006 §3).
                    (Stdlib::At, Some(Ty::List(item))) => Ty::Option(item.clone()),
                    _ => Ty::from_roster(f.returns()),
                }
            } else {
                return Err(checks::unknown_function(name, scope, shapes, *span));
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
                    (a, b) if a.is_text() && b.is_text() => Ty::String,
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
                    choices::compared(&ta, b)?;
                    choices::compared(&tb, a)?;
                    if ta.join(&tb).is_none() {
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
        Expr::Ternary(..) | Expr::Match { .. } => {
            let (ta, tb) = arms(e, scope, shapes, infer)?;
            ta.join(&tb).ok_or_else(|| disagree(e, &ta, &tb))?
        }
        Expr::Case {
            subject,
            all,
            checked,
            span,
            ..
        } => choices::case(subject, all.as_deref(), *checked, *span, scope, shapes)?,
        Expr::Let {
            name, value, body, ..
        } => {
            let t = infer(value, scope, shapes)?;
            let mut inner = scope.clone();
            inner.push(vec![(name.clone(), Ref::Local(0), t)]);
            infer(body, &inner, shapes)?
        }
        Expr::Arrow { span, .. } => {
            return err(
                "type-arrow-position",
                "an arrow function is only the second argument of `map` or `filter`: `map(list, (item, index) => …)`",
                *span,
            )
        }
    })
}

/// The two arms of a ternary or a `match`, each typed by `leaf` once the
/// condition is a bool or the subject an option.
pub(crate) fn arms(
    e: &Expr,
    scope: &Scope,
    shapes: &Shapes,
    leaf: fn(&Expr, &Scope, &Shapes) -> Result<Ty, TypeError>,
) -> Result<(Ty, Ty), TypeError> {
    match e {
        Expr::Ternary(c, a, b, _) => {
            // Naming the type defers a condition still `?` (a derive the fixpoint has
            // not settled) the way `Binary` does: the message carries `?`, the
            // round skips it, and the strict pass reports what never types.
            let tc = infer(c, scope, shapes)?;
            if tc != Ty::Bool {
                return err(
                    "type-condition",
                    format!("a condition must be a bool, given `{tc}`"),
                    c.span(),
                );
            }
            Ok((leaf(a, scope, shapes)?, leaf(b, scope, shapes)?))
        }
        Expr::Match {
            subject,
            var,
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
            inner_scope.push(vec![(var.clone(), Ref::Local(0), (*inner).clone())]);
            Ok((
                leaf(some, &inner_scope, shapes)?,
                leaf(none, scope, shapes)?,
            ))
        }
        _ => unreachable!("a ternary or a `match`"),
    }
}

/// Two arm types where one was needed.
pub(crate) fn disagree(e: &Expr, ta: &Ty, tb: &Ty) -> TypeError {
    let what = match e {
        Expr::Match { .. } => "`match` arms disagree",
        _ => "branches disagree",
    };
    TypeError {
        id: "type-branches",
        message: format!("{what}: `{ta}` and `{tb}`"),
        span: e.span(),
    }
}

/// Check shared shapes and functions, including a module without a root component.
/// Navigation uses the same declaration rules as executable compilation.
pub fn check_declarations(file: &File) -> Result<Shapes, TypeError> {
    let mut shapes = Shapes::default();
    routes::declare(file, &mut shapes)?;
    selection::declare(&mut shapes);
    geometry::declare(&mut shapes);
    for s in &file.shapes {
        if shapes.map.contains_key(&s.name) {
            return err(
                "type-duplicate-shape",
                format!("shape `{}` declared twice", s.name),
                s.span,
            );
        }
        shapes.map.insert(s.name.clone(), Vec::new());
        shapes.declared.insert(s.name.clone());
    }
    check_shape_cycles(file, &shapes)?;
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
        // @ref LLP 1035.005.000 D3 — `Name(…)` builds a declared shape.
        if shapes.declared.contains(&f.name) {
            return err(
                "type-fn-shape-name",
                format!(
                    "`fn {}` has a shape's name, and `{}(field=…)` builds that shape: pick another",
                    f.name, f.name
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
        let (params, ret) = &shapes.fns[&f.name];
        let mut scope = Scope::default();
        scope.push(
            f.params
                .iter()
                .enumerate()
                .map(|(i, p)| (p.name.clone(), Ref::Local(i as u32), params[i].clone()))
                .collect(),
        );
        let t = choices::given(ret, &f.body, &scope, &shapes)?;
        if !ret.accepts(&t) {
            return err(
                "type-fn-return",
                format!(
                    "`fn {}` declares `{ret}` but its body is `{t}`{}",
                    f.name,
                    choices::hint(ret, &t)
                ),
                f.body.span(),
            );
        }
    }
    checks::check_function_cycles(file)?;
    Ok(shapes)
}

/// Check a file: shared declarations, then every component. The first
/// refusal, as [`check_all`] orders them.
pub fn check(file: &File, style_attr: fn(&str) -> bool) -> Result<Checked<'_>, TypeError> {
    check_with_sites(file, false, style_attr, None).map_err(|mut all| all.swap_remove(0))
}

/// Check a file and report every independent refusal (at most
/// [`MAX_REFUSALS`]), call sites first; `mapped` retains source provenance.
/// `style_attr` says which attribute names set style rows; `strings` are the
/// app's tables, which `t(...)` is checked against.
pub fn check_all(
    file: &File,
    mapped: bool,
    style_attr: fn(&str) -> bool,
    strings: Option<Arc<strings::Strings>>,
) -> Result<Checked<'_>, Vec<TypeError>> {
    check_with_sites(file, mapped, style_attr, strings)
}

fn check_with_sites(
    file: &File,
    capture_sites: bool,
    style_attr: fn(&str) -> bool,
    strings: Option<Arc<strings::Strings>>,
) -> Result<Checked<'_>, Vec<TypeError>> {
    if file.components.is_empty() {
        return Err(vec![TypeError {
            id: "analyze-no-component",
            message: "a file needs a component".into(),
            span: Span::point(1, 1),
        }]);
    }
    let mut shapes = check_declarations(file).map_err(|e| vec![e])?;
    shapes.style_attr = Some(style_attr);
    shapes.strings = strings;
    let mut types = Types {
        shapes,
        components: Vec::new(),
    };
    let root = &file.components[0];
    // The root's entry stands in until the root itself is checked.
    types.components.push(ComponentTypes {
        name: root.name.clone(),
        props: root
            .props
            .iter()
            .chain(&root.injects)
            .map(|p| {
                p.ty.as_ref()
                    .and_then(|t| types.shapes.resolve(t).ok())
                    .unwrap_or(Ty::Unknown)
            })
            .collect(),
        ..ComponentTypes::default()
    });
    let mut sink = Sink::default();
    posts::check_targets(file, &mut sink);
    check_children(file, &mut types, &mut sink);
    let children = sink.errors.len();
    // The root is checked against its inlined view, so a handler's real call
    // site (behind a child's prop) types the action's parameters.
    // The expanded root (LLP 1017 P4c): the inlined view plus every stateful
    // child's own declarations, lifted in — what lowering will lower.
    // A use that cannot be expanded is refused and left out; the rest of the
    // root is still checked, what it lacked reading as `?`.
    let (expanded, refused) = contract_syntax::expand_all(file, capture_sites);
    for e in refused {
        sink.push(TypeError {
            id: e.id,
            message: e.message,
            span: e.span,
        });
    }
    check_root(file, &mut types, &expanded, &mut sink);
    if sink.errors.is_empty() {
        Ok(Checked {
            file,
            types,
            expanded,
        })
    } else {
        Err(prefer_call_sites(
            sink.errors,
            children,
            file,
            &types,
            &expanded,
        ))
    }
}

/// Children are views over their props, checked standalone before the
/// root inlines them, so an error in a child is reported in its own terms.
fn check_children(file: &File, types: &mut Types, sink: &mut Sink) {
    // A child may own `state`, `derive`, and `action` (LLP 1017 P4c: its
    // instances' own), never a `resource`, `mutation`, or `task` — a row
    // must not open N requests, and only the root has a clock.
    for c in file.components.iter().skip(1) {
        if !c.resources.is_empty() || !c.mutations.is_empty() || !c.tasks.is_empty() {
            let span = c
                .resources
                .first()
                .map(|r| r.span)
                .or(c.mutations.first().map(|m| m.span))
                .or(c.tasks.first().map(|t| t.span))
                .unwrap_or(c.span);
            sink.push(TypeError {
                id: "type-child-resource",
                message: format!("component `{}` takes props: a resource, mutation, or task lives in the root (a child may own state, derives, and actions)", c.name),
                span,
            });
        }
    }
    for c in file.components.iter().skip(1) {
        let ct = component::check_component(c, types, None, sink);
        types.components.push(ct);
    }
}

/// Call sites before the views they expand into: the children's uses, then
/// the expanded root and its own uses and injects.
fn check_root(
    file: &File,
    types: &mut Types,
    expanded: &contract_syntax::Expanded,
    sink: &mut Sink,
) {
    for (c, ct) in file.components.iter().zip(&types.components).skip(1) {
        uses::check_uses(&c.view, &types.component_scope(c, ct), types, file, sink);
    }
    types.components[0] =
        component::check_component(&expanded.root, types, Some(&expanded.owners), sink);
    // @ref LLP 1048.000 D2 — each route's `pages=` source, in the root's seam.
    for row in file.routes.iter().flat_map(|r| &r.rows) {
        for field in row.fields.iter().filter(|f| f.name == "pages") {
            sink.keep_unit(routes::pages_source(row, field, &mut types.components[0]));
        }
    }
    let scope = types.component_scope(&expanded.root, &types.components[0]);
    let root = &file.components[0];
    uses::check_uses(&root.view, &scope, types, file, sink);
    for b in &root.provides {
        sink.keep(infer(&b.expr, &scope, &types.shapes));
    }
    sink.keep_unit(check_injects(
        &root.view,
        &root.provides,
        &scope,
        types,
        file,
    ));
}

/// Refusals are first checked against the call sites that lead to them: a
/// misspelled prop is named where it is written (never reported as the prop
/// it left missing), then a mistyped data argument at the root's uses (never
/// as what the substituted value broke inside the callee). When a call site
/// is at fault, what the expanded root found inside a child's lines is its
/// consequence and is left out; the root's own refusals and a child's own
/// (the first `children`) are independent and stay. Only a refusal pays for
/// this.
fn prefer_call_sites(
    errors: Vec<TypeError>,
    children: usize,
    file: &File,
    types: &Types,
    expanded: &contract_syntax::Expanded,
) -> Vec<TypeError> {
    let mut sites = Sink::default();
    // Uses whose missing prop is the one a misspelling there was meant as.
    let mut explained = Vec::new();
    for c in &file.components {
        uses::check_prop_names(&c.view, file, &mut sites, &mut explained);
    }
    uses::check_root_uses(&file.components[0], &expanded.root, types, file, &mut sites);
    if sites.errors.is_empty() {
        return errors;
    }
    for (i, e) in errors.into_iter().enumerate() {
        if i < children || !in_child(file, e.span) {
            sites.push(e);
        }
    }
    sites.errors.retain(|e| {
        !(matches!(e.id, "syntax-missing-prop" | "type-missing-prop")
            && explained.contains(&e.span))
    });
    sites.errors
}

/// Whether `span` lies in a child component's own lines: each component runs
/// from its header to the next component's header in the same file.
fn in_child(file: &File, span: Span) -> bool {
    file.components
        .iter()
        .enumerate()
        .filter(|(_, c)| c.span.source_id == span.source_id && c.span.line <= span.line)
        .max_by_key(|(_, c)| c.span.line)
        .is_some_and(|(i, _)| i > 0)
}
