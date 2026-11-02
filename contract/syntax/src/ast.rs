//! The abstract syntax tree.
pub use crate::colours::{ColorProfileDecl, DesignSource, ThemeDecl};
pub use crate::tap::TapForm;
pub use crate::test_steps::{Step, TestDecl};
use crate::Span;
use std::collections::BTreeMap;
/// Identifier ranges that are distinct from their AST construct's diagnostic span.
/// Compiler tooling consumes these; they never enter an executable plan.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NameSpans {
    /// Declaration, local binding, or target name, keyed by its construct.
    pub names: BTreeMap<Span, Span>,
    /// Data source name in a resource or send, keyed by its construct.
    pub sources: BTreeMap<Span, Span>,
}
impl NameSpans {
    /// The exact identifier range, or the construct when it already names a token.
    pub fn name(&self, span: Span) -> Span {
        self.names.get(&span).copied().unwrap_or(span)
    }
}
/// One source file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct File {
    /// Parser-owned identifier ranges for navigation.
    pub names: NameSpans,
    /// The app's router declaration.
    pub routes: Option<RoutesDecl>,
    /// `use … from "…"` declarations, in order; resolved by
    /// the driver, which scopes each file's names.
    pub uses: Vec<UseDecl>,
    /// `font "Name"` declarations, in order.
    pub fonts: Vec<FontDecl>,
    /// `sound "assets/…wav"` declarations, in order.
    pub sounds: Vec<SoundDecl>,
    /// `shape` declarations, in order.
    pub shapes: Vec<ShapeDecl>,
    /// `style` declarations, in order.
    pub styles: Vec<StyleDecl>,
    /// Named textarea decoration rows.
    pub decorations: Vec<StyleDecl>,
    /// Loaded files' visible decoration aliases, by source identity.
    pub decoration_scopes: BTreeMap<u32, Vec<(String, String)>>,
    /// `atom` declarations: the inline atoms' source forms.
    pub atoms: Vec<StyleDecl>,
    /// Loaded files' visible atom aliases, by source identity.
    pub atom_scopes: BTreeMap<u32, Vec<(String, String)>>,
    /// `keyframes` declarations, in order: CSS `@keyframes`,
    /// scoped to their file as in CSS Modules.
    pub keyframes: Vec<KeyframesDecl>,
    /// `timeline` declarations, in order: clock timelines
    /// that `animation-timeline=Name` puts animations on.
    pub timelines: Vec<TimelineDecl>,
    /// `color-profile` declarations, in order.
    pub color_profiles: Vec<ColorProfileDecl>,
    /// The app's `theme`: the root file's only.
    pub theme: Option<ThemeDecl>,
    /// The app's design, `app.tamber`, set by the driver.
    pub design: Option<DesignSource>,
    /// The designs of the packages the app's sources link,
    /// each `<name>.tamber` beside its package's entry, set by the driver.
    pub package_designs: Vec<DesignSource>,
    /// The identities of the exported library components:
    /// each component a private package that ships a design declares in
    /// its entry, or re-exports there; set by the driver.
    pub library: Vec<String>,
    /// The names of the app's library packages: each linked
    /// package that is `"private": true` and ships a design; set by the
    /// driver.
    pub library_packages: Vec<String>,
    /// `fn` declarations, in order.
    pub fns: Vec<FnDecl>,
    /// Native modules' sources and sends, read from packages.
    pub modules: Vec<crate::ModuleDecl>,
    /// `test` declarations, in order — normally in a file of
    /// their own beside the app, `app.test.contract`.
    pub tests: Vec<TestDecl>,
    /// Properties checked against test observations, never compiled.
    pub properties: Vec<PropertyDecl>,
    /// A test file's launch lines (`size`, `epoch`, `time-zone`, `locale`,
    /// `seed`, `before data`), which every test in the file opens with unless it names its
    /// own.
    pub launch: Vec<Step>,
    /// `component` declarations, in order. The first is the root.
    pub components: Vec<Component>,
    /// A members file's `members Name` block: members of
    /// the component `Name`, which lists this file.
    pub members: Option<Component>,
    /// Refusals left to the loader.
    pub deferred: Vec<crate::members::Deferred>,
}
impl File {
    /// Describe an unknown component and the merged declarations in source order.
    /// Only refusal paths call this; valid uses allocate no diagnostic list.
    pub fn unknown_component_message(&self, name: &str) -> String {
        let names = self
            .components
            .iter()
            .map(|component| format!("`{}`", component.name))
            .collect::<Vec<_>>()
            .join(", ");
        let choices = if names.is_empty() {
            "no components are declared".to_owned()
        } else {
            format!("declared components: {names}")
        };
        format!("unknown component `{name}`; {choices}")
    }
}
/// `routes <slot>` with rows in declaration order.
#[derive(Debug, Clone, PartialEq)]
pub struct RoutesDecl {
    /// The root slot filled at launch.
    pub slot: String,
    /// The table, flattened from indentation.
    pub rows: Vec<RouteDecl>,
    /// Where.
    pub span: Span,
}
/// One route line. Parent indices refer to earlier rows of the same table.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteDecl {
    /// Route name, or `notfound` for the fallback.
    pub name: String,
    /// Absolute pattern; empty for `notfound`.
    pub pattern: String,
    /// Enclosing route, if any.
    pub parent: Option<usize>,
    /// A tab root.
    pub tab: bool,
    /// The bare fallback line.
    pub notfound: bool,
    /// Policy fields after the pattern, `render=build activate=idle`, as
    /// written; lowering reads them.
    pub fields: Vec<Attr>,
    /// Where.
    pub span: Span,
}
/// A declared font family: its faces, each one file at a weight or a weight
/// range, and a style.
#[derive(Debug, Clone, PartialEq)]
pub struct FontDecl {
    /// The Contract alias; hosts bind this name to these bytes.
    pub name: String,
    /// Its faces.
    pub faces: Vec<FontFaceDecl>,
    /// Where.
    pub span: Span,
}
/// A declared sound: a WAV under the app's `assets/`, which
/// `playSound` and `playSounds` name by its path.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundDecl {
    /// App-relative WAV source, as `playSound` names it.
    pub source: String,
    /// Where.
    pub span: Span,
}
/// One face in a [`FontDecl`].
#[derive(Debug, Clone, PartialEq)]
pub struct FontFaceDecl {
    /// CSS weights 1–1000, low and high, as `@font-face`'s `font-weight`
    /// descriptor: one weight is both. `None` is the descriptor's `auto`, the
    /// file's own range (`font "Name" = "path"`).
    pub weight: Option<[u16; 2]>,
    /// Italic, rather than normal.
    pub italic: bool,
    /// App-relative TTF/OTF source.
    pub source: String,
    /// Where.
    pub span: Span,
}
/// A property in a test file.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyDecl {
    /// Declaration location.
    pub span: Span,
    /// When this property is checked.
    pub kind: PropertyKind,
    /// Authored name; for `never refuses`, the action's name.
    pub name: String,
    /// Predicates in source order.
    pub lines: Vec<PropertyLine>,
}
/// The observations a property quantifies over.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyKind {
    /// Every printed configuration.
    Always,
    /// An input committing this handler, followed by its landing.
    After(String),
    /// A clock advance or an independent landing.
    Clock,
    /// Every attempted commit of the named action.
    NeverRefuses,
}
/// One predicate and its authored location.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyLine {
    /// Line location.
    pub span: Span,
    /// Parsed predicate.
    pub form: PropertyForm,
}
/// A property line's parsed form.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyForm {
    /// A whole root state or derive value.
    Hold {
        /// Bare root name.
        name: String,
        /// Test of its observation.
        test: PropertyTest,
    },
    /// Every changed root slot belongs to this set; empty means `writes none`.
    OnlyWrites(Vec<String>),
    /// A host command emitted by the step or pair.
    Command {
        /// Command name.
        name: String,
        /// Literal arguments in order.
        args: Vec<PropertyValue>,
    },
}
/// A literal in the property vocabulary.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    /// A binary64 number.
    Number(f64),
    /// A string.
    String(String),
    /// A boolean.
    Bool(bool),
    /// An absent option.
    None,
    /// Any present option; admitted only in `in`.
    Some,
    /// An empty list.
    EmptyList,
}
/// A test of one value, optionally against its previous observation.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyTest {
    /// Exact printed value.
    Equal(PropertyValue),
    /// Exact previous printed value.
    Old,
    /// Previous number plus this number, in binary64.
    Add(f64),
    /// Previous number minus this number, in binary64.
    Subtract(f64),
    /// Negation of the previous boolean.
    NotOld,
    /// Membership by value or option kind.
    In(Vec<PropertyValue>),
    /// IEEE greater than or equal.
    AtLeast(f64),
    /// IEEE less than or equal.
    AtMost(f64),
    /// A finite integer.
    Whole,
    /// Once this value is seen, the next observation retains it.
    Stays(PropertyValue),
}
/// `fn name(param: type, …): type = expr` — a pure function written in
/// Contract: one expression over its parameters and the roster, expanded
/// inline wherever it is called. No recursion, no loops, no
/// state: the escape for a price string or a palette choice, not for a
/// traversal, which is the data crate's.
#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    /// The name.
    pub name: String,
    /// Typed parameters.
    pub params: Vec<Param>,
    /// The declared result type.
    pub ret: TypeExpr,
    /// The body, one expression.
    pub body: Expr,
    /// Where.
    pub span: Span,
}
/// `use A, B as C from "./file.contract"` — declarations of another Contract
/// file, named one by one, each optionally renamed in this file; never
/// anything else (`contract-no-imports`).
#[derive(Debug, Clone, PartialEq)]
pub struct UseDecl {
    /// The names, in order; at least one.
    pub names: Vec<UseName>,
    /// The file, relative to this one.
    pub path: String,
    /// Where.
    pub span: Span,
}
/// One name a `use` brings: `Card`, or `Card as UiCard`.
#[derive(Debug, Clone, PartialEq)]
pub struct UseName {
    /// The declaration's name in the used file.
    pub name: String,
    /// The name in this file, when `as` renames it.
    pub alias: Option<String>,
    /// The declaration's name as written.
    pub span: Span,
}
impl UseName {
    /// The name this file reads it by.
    pub fn local(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.name)
    }
}
/// `style Name` with lines of `attr=literal` — a named set of style rows a
/// node applies with `class=Name`; its own attributes win.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleDecl {
    /// The name.
    pub name: String,
    /// The rows, as attributes with literal values.
    pub attrs: Vec<Attr>,
    /// Where.
    pub span: Span,
}
/// `keyframes Name`: CSS `@keyframes`.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyframesDecl {
    /// The name `animation-name` refers to.
    pub name: String,
    /// The keyframes, in source order.
    pub frames: Vec<KeyframeDecl>,
    /// Where.
    pub span: Span,
}
/// `timeline Name`: a clock timeline. Every animation on
/// it starts in step with the others.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineDecl {
    /// The name `animation-timeline` refers to.
    pub name: String,
    /// Where.
    pub span: Span,
}
/// One line of a `keyframes` declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyframeDecl {
    /// Selectors as percentages: `from` is 0, `to` is 100.
    pub selectors: Vec<f64>,
    /// The values, literal.
    pub attrs: Vec<Attr>,
    /// Where.
    pub span: Span,
}
/// `shape Name` with its fields.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeDecl {
    /// The name.
    pub name: String,
    /// Fields in declaration order.
    pub fields: Vec<Field>,
    /// Where it was declared.
    pub span: Span,
}
/// One shape field.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// Name.
    pub name: String,
    /// Type.
    pub ty: TypeExpr,
    /// Where.
    pub span: Span,
}
/// A written type: `number`, `string`, `bool`, a shape name, `option<T>`, `T[]`.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    /// A finite string vocabulary, `enum<"a", "b">`.
    Enum(Vec<String>, Span),
    /// A named type: a primitive or a shape.
    Named(String, Span),
    /// `option<T>`.
    Option(Box<TypeExpr>, Span),
    /// `T[]`.
    List(Box<TypeExpr>, Span),
}
impl TypeExpr {
    /// Whether this type carries a finite string vocabulary.
    pub fn contains_enum(&self) -> bool {
        match self {
            Self::Enum(..) => true,
            Self::Option(t, _) | Self::List(t, _) => t.contains_enum(),
            Self::Named(..) => false,
        }
    }
    /// Where it was written.
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Enum(_, s)
            | TypeExpr::Named(_, s)
            | TypeExpr::Option(_, s)
            | TypeExpr::List(_, s) => *s,
        }
    }
}
/// A component.
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    /// The name.
    pub name: String,
    /// `props` (empty for the root).
    pub props: Vec<Param>,
    /// `inject` declarations: typed names a use site does not pass — the
    /// nearest enclosing component's `provide` section fills them.
    pub injects: Vec<Param>,
    /// The `provide` section: each binding fills the
    /// same-named `inject` of every component used in this component's view,
    /// unless a nearer component's section provides it too. A bare name is
    /// `name = name`; the span is the name's.
    pub provides: Vec<Binding>,
    /// Whether the component declares `slot`: the nodes indented under a use
    /// of it fill its `children` node.
    pub slot: bool,
    /// `state` declarations.
    pub states: Vec<Binding>,
    /// The states that say `persist`, in declaration order.
    pub persist: Vec<Persist>,
    /// `derive` declarations.
    pub derives: Vec<Binding>,
    /// `resource` declarations.
    pub resources: Vec<ResourceDecl>,
    /// `mutation` declarations.
    pub mutations: Vec<MutationDecl>,
    /// `action` declarations.
    pub actions: Vec<Action>,
    /// `task` declarations.
    pub tasks: Vec<Task>,
    /// `sequence` declarations, the root's only.
    pub sequences: Vec<crate::sequence::Sequence>,
    /// `tool` lines, the root's only.
    pub tools: Vec<crate::tool::Tool>,
    /// The view's top-level nodes.
    pub view: Vec<Node>,
    /// The `params` section and the component's identity.
    pub params: crate::params::Params,
    /// Its parts and the nodes named as them.
    pub anatomy: crate::anatomy::Anatomy,
    /// Its `members` lines.
    pub members: crate::members::Members,
    /// Where.
    pub span: Span,
}
impl Component {
    /// Describe every required prop absent from a use, in declaration order.
    /// Called only after a missing argument is found; valid uses allocate nothing.
    pub fn missing_props_message(&self, args: &[Attr]) -> String {
        let missing = self
            .props
            .iter()
            .filter(|prop| !args.iter().any(|arg| arg.name == prop.name))
            .map(|prop| format!("`{}`", prop.name))
            .collect::<Vec<_>>()
            .join(", ");
        format!("`{}` needs {missing}", self.name)
    }
}
/// `state name = expr persist`: a root state the host keeps
/// across launches; the state's name, and where `persist` is written.
#[derive(Debug, Clone, PartialEq)]
pub struct Persist {
    /// The state's name.
    pub name: String,
    /// Where `persist` is written.
    pub span: Span,
}
impl Component {
    /// Whether the state `name` says `persist`.
    pub fn persists(&self, name: &str) -> bool {
        self.persist.iter().any(|p| p.name == name)
    }
}
/// `name = expr`.
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    /// Name.
    pub name: String,
    /// Initializer or body.
    pub expr: Expr,
    /// Where.
    pub span: Span,
}
/// `resource name = source(args) [with context, …] as shape T [else source(args)]`.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceDecl {
    /// Name.
    pub name: String,
    /// The data source's name.
    pub source: String,
    /// Call arguments followed by request context expressions.
    pub args: Vec<Expr>,
    /// Number of call arguments when `with` is present; otherwise all
    /// arguments identify the answer.
    pub identity: Option<usize>,
    /// The declared shape.
    pub shape: TypeExpr,
    /// `else source(args)`: what shows while the source
    /// hasn't answered.
    pub placeholder: Option<Placeholder>,
    /// Where.
    pub span: Span,
}
/// A resource's placeholder: a source call, as a resource's value is (a
/// record comes from a source), whose arguments are values — it reads no
/// state, so the build answers it once for every launch.
#[derive(Debug, Clone, PartialEq)]
pub struct Placeholder {
    /// The data source's name.
    pub source: String,
    /// Argument expressions.
    pub args: Vec<Expr>,
    /// Where.
    pub span: Span,
}
/// `mutation name as shape T`: an `option<T>` slot, `none` at
/// boot, that a `send` fills from an action.
#[derive(Debug, Clone, PartialEq)]
pub struct MutationDecl {
    /// Name.
    pub name: String,
    /// The reply's shape, `T`.
    pub shape: TypeExpr,
    /// `queue`: one send in flight, later sends wait their turn in order;
    /// otherwise the newest send wins.
    pub queue: bool,
    /// `refreshes a, b`: resources the runner re-asks, forced, when a send
    /// to this mutation runs and when its reply lands.
    pub refreshes: Vec<(String, Span)>,
    /// `then action`: run after each answer lands, and where.
    pub then: Option<(String, Span)>,
    /// Where.
    pub span: Span,
}
/// A typed parameter (`props` entry or action parameter).
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// Name.
    pub name: String,
    /// Type, when written (action parameters may omit it).
    pub ty: Option<TypeExpr>,
    /// Where.
    pub span: Span,
}
/// `action name(params)` with a body of statements.
#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    /// Name.
    pub name: String,
    /// Parameters.
    pub params: Vec<Param>,
    /// Statements.
    pub body: Vec<Stmt>,
    /// Where.
    pub span: Span,
}
/// One slot an action body assigns (`x = …`) or sends (`send m = …`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Effect<'a> {
    /// The state or mutation.
    pub target: &'a str,
    /// The statement.
    pub span: Span,
    /// `send`, not an assignment.
    pub send: bool,
    /// The outermost call it is made through (the callee, the call's
    /// span), when a called action makes it.
    pub call: Option<(&'a str, Span)>,
}
impl Action {
    /// Every slot the body assigns or sends, through every branch of its
    /// `if`s and `match`es, in statement order with repeats. An action's
    /// effects are inferred, never declared.
    ///
    /// A call's are its callee's, through every call it makes in turn:
    /// the union is the plan's write allowlist, which the VM holds
    /// every store and send to.
    pub fn effects(&self) -> Vec<Effect<'_>> {
        fn walk<'a>(stmts: &'a [Stmt], call: Option<(&'a str, Span)>, out: &mut Vec<Effect<'a>>) {
            for stmt in stmts {
                match stmt {
                    Stmt::Assign { target, span, .. } => out.push(Effect {
                        target,
                        span: *span,
                        send: false,
                        call,
                    }),
                    Stmt::Send { target, span, .. } => out.push(Effect {
                        target,
                        span: *span,
                        send: true,
                        call,
                    }),
                    Stmt::If {
                        then, otherwise, ..
                    } => {
                        walk(then, call, out);
                        walk(otherwise, call, out);
                    }
                    Stmt::Match { some, none, .. } => {
                        walk(&some.1, call, out);
                        walk(none, call, out);
                    }
                    Stmt::Call {
                        action, body, span, ..
                    } => walk(body, call.or(Some((action, *span))), out),
                    Stmt::Command { .. } | Stmt::Refresh { .. } | Stmt::Let { .. } => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.body, None, &mut out);
        out
    }
}
/// A statement in an action body.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `let name = expr`: an immutable local, read by
    /// the statements after it in its block and the blocks nested there.
    Let {
        /// The local.
        name: String,
        /// Its value, evaluated once where the statement stands.
        expr: Expr,
        /// The name.
        span: Span,
    },
    /// `slot = expr`.
    Assign {
        /// The slot.
        target: String,
        /// The value.
        expr: Expr,
        /// Where.
        span: Span,
    },
    /// `name(args)` — a command (capability call).
    Command {
        /// The capability.
        name: String,
        /// Arguments.
        args: Vec<Expr>,
        /// Where.
        span: Span,
    },
    /// `send target = source(args)` — the mutation's request.
    Send {
        /// The mutation.
        target: String,
        /// The data source.
        source: String,
        /// Arguments.
        args: Vec<Expr>,
        /// Where.
        span: Span,
    },
    /// `refresh target` — re-request a resource with its current arguments.
    Refresh {
        /// The resource.
        target: String,
        /// Where.
        span: Span,
    },
    /// `if cond` … `else` … — a branch of statements.
    If {
        /// The condition, a bool.
        cond: Expr,
        /// When true.
        then: Vec<Stmt>,
        /// When false; may be empty.
        otherwise: Vec<Stmt>,
        /// Where.
        span: Span,
    },
    /// `match subject` with `case some(x)` and `case none` blocks of
    /// statements.
    Match {
        /// The option.
        subject: Expr,
        /// The bound name and the `some` block.
        some: (String, Vec<Stmt>),
        /// The `none` block.
        none: Vec<Stmt>,
        /// Where.
        span: Span,
    },
    /// A call of an action, expanded in place: the
    /// callee's statements, run where the call stands in the caller's one
    /// commit, reading the state the action started with. The parser never
    /// makes one; expansion turns a `name(args)` statement naming an action
    /// of the same component, an `action` prop or an injected action into
    /// one ([`crate::inline::calls`]).
    Call {
        /// The callee: an action of the expanded root.
        action: String,
        /// The callee's whole argument list, in its parameters' order: a
        /// lifted callee's capture parameters, the arguments curried where
        /// the action was passed, then the call's own (the last
        /// `authored`).
        args: Vec<Expr>,
        /// The callee's statements, every name it binds renamed apart, its
        /// parameters first as `let`s of `args`. One block: its locals drop
        /// before the statement after the call.
        body: Vec<Stmt>,
        /// How many arguments the call itself writes.
        authored: usize,
        /// How many of the callee's own parameters were curried where it
        /// was passed (`close=dismiss("photo")`); 0 for a same-component call.
        curried: usize,
        /// The prop's or the `provide`'s binding (`go=move(id)`), for a
        /// prop or inject call.
        binding: Option<Span>,
        /// Where.
        span: Span,
    },
}

impl Stmt {
    /// The statement's span.
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let { span, .. }
            | Stmt::Assign { span, .. }
            | Stmt::Command { span, .. }
            | Stmt::Send { span, .. }
            | Stmt::Refresh { span, .. }
            | Stmt::If { span, .. }
            | Stmt::Match { span, .. }
            | Stmt::Call { span, .. } => *span,
        }
    }
}

/// `task name mount` (or `when cond [key=expr]`, or `key=expr`) with
/// `every(ms, action)`, `every(frame, action)` or `after(ms, action)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// Name.
    pub name: String,
    /// Whether the timer repeats or fires once.
    pub kind: TaskKind,
    /// `when cond`: the timer exists while `cond` holds;
    /// `None` for `mount` and for `key=` alone (`when true`).
    pub gate: Option<Expr>,
    /// `key=expr`: a new key restarts the timer, as a new `for` key makes
    /// a new row.
    pub key: Option<Expr>,
    /// `(ms, action)` and the entry's span.
    pub timer: (Expr, String, Span),
    /// Named scheduling options, validated by lowering.
    pub options: Vec<(String, Expr, Span)>,
    /// Where.
    pub span: Span,
}

/// A task's schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    /// `every(ms, action)`: fires at boot+ms, then every ms.
    Every,
    /// `after(ms, action)`: fires once at boot+ms, then is spent.
    After,
    /// `every(frame, action)`: fires once per presented frame;
    /// the timer's expression is a placeholder `0`.
    Frame,
}

/// A view node.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// `tag positional attr=expr …` with children.
    Element {
        /// The tag.
        tag: String,
        /// Positional arguments (a `text` literal, for instance).
        positional: Vec<Expr>,
        /// Named attributes.
        attrs: Vec<Attr>,
        /// Children.
        children: Vec<Node>,
        /// Where.
        span: Span,
        /// The inliner's: the index into
        /// [`Expanded::instances`](crate::Expanded::instances) of the
        /// component instantiation this element was expanded in — the
        /// root's own elements are 0, a used component's are its use's;
        /// 0 as parsed. The development map and refusal diagnostics read it.
        instance: u32,
    },
    /// `Name(arg=expr, …)`, with the nodes indented under it filling the
    /// component's `slot`.
    Use {
        /// The component.
        name: String,
        /// Named arguments.
        args: Vec<Attr>,
        /// The slot's fill; empty when nothing is indented under the use.
        children: Vec<Node>,
        /// Where.
        span: Span,
    },
    /// `children` — where a `slot` component's use puts the nodes indented
    /// under it.
    Children {
        /// Where.
        span: Span,
    },
    /// `if cond … else …`. `tag` is the inliner's, as on `Each`: its
    /// arms own the state of the children used in them; 0 as parsed.
    When {
        /// The inliner's tag.
        tag: u32,
        /// Condition.
        cond: Expr,
        /// Then-branch.
        then: Vec<Node>,
        /// Else-branch (possibly empty).
        otherwise: Vec<Node>,
        /// Where.
        span: Span,
    },
    /// `for x in list key=expr`, or `for x, i in list key=expr` binding
    /// the item's position too. `tag` is the inliner's: unique
    /// per region (`for`, `if`, `match`) in the expanded root, so a
    /// lifted slot can name the region that owns it before regions exist;
    /// 0 as parsed.
    Each {
        /// The inliner's tag.
        tag: u32,
        /// The item variable.
        var: String,
        /// The position variable, a number from 0, when named.
        index: Option<String>,
        /// The list.
        list: Expr,
        /// The key expression (may name `var`).
        key: Expr,
        /// Body.
        body: Vec<Node>,
        /// Where.
        span: Span,
    },
    /// `match subject` with `case some(x)` and `case none` arms; `tag` as
    /// on `When`.
    Match {
        /// The inliner's tag.
        tag: u32,
        /// The subject.
        subject: Expr,
        /// The bound name and body of `case some(x)`.
        some: (String, Vec<Node>),
        /// The body of `case none`.
        none: Vec<Node>,
        /// Where.
        span: Span,
    },
}

impl Node {
    /// Where.
    pub fn span(&self) -> Span {
        match self {
            Node::Element { span, .. }
            | Node::Use { span, .. }
            | Node::Children { span }
            | Node::When { span, .. }
            | Node::Each { span, .. }
            | Node::Match { span, .. } => *span,
        }
    }
}

/// Whether `positional` is the word `document` in `scroll document`: the
/// view's scroll container is the page's. It is a word,
/// never a name in scope, so checking and lowering read it by spelling.
pub fn is_scroll_document(tag: &str, positional: &Expr) -> bool {
    tag == "scroll" && matches!(positional, Expr::Ident(word, _) if word == "document")
}

/// Whether `positional` is the word `switch` in `input type="checkbox"
/// switch`: HTML's boolean attribute, a checkbox drawn as a switch.
/// A word like `scroll document`, never a name in scope.
pub fn is_input_switch(tag: &str, positional: &Expr) -> bool {
    tag == "input" && matches!(positional, Expr::Ident(word, _) if word == "switch")
}

/// Whether `positional` is the word `multiple` in `input type="file"
/// multiple`: HTML's boolean attribute, a word like
/// `switch`, never a name in scope. `multiple=true` says the same.
pub fn is_input_multiple(tag: &str, positional: &Expr) -> bool {
    tag == "input" && matches!(positional, Expr::Ident(word, _) if word == "multiple")
}

/// The form control an `input` is, by its literal `type`:
/// `Some("checkbox")` for a checkbox (a switch is one too), whose `change`
/// and `input` carry a bool; `Some("radio")` for a radio, whose carry its
/// `value`; `Some("file")` for a file input, whose `change` carries a
/// `Picked[]`; `Some("select")` for a
/// `select`, whose `change` and `input` carry the chosen option's value;
/// `None` for a text field or any other element.
pub fn input_control(tag: &str, attrs: &[Attr]) -> Option<&'static str> {
    if tag == "select" {
        return Some("select");
    }
    if tag != "input" {
        return None;
    }
    attrs
        .iter()
        .find(|a| a.name == "type")
        .and_then(|a| match &a.value {
            Expr::Str(t, _) => control_type(t),
            _ => None,
        })
}

/// [`input_control`], or `Some("field")` for a text field whose `select`
/// is HTML's: an `input` that is no control, or a
/// `textarea` that is not the Markdown editor (no `markup`, or literally
/// `none`), whose `select` carries the field's `InputEvent`; or
/// `Some("markdown")` for a textarea that may be the editor, whose `select`
/// carries its `MarkdownSelection` and whose other field events are a text
/// field's, over its source. What a handler's payload is typed by.
pub fn payload_control(tag: &str, attrs: &[Attr]) -> Option<&'static str> {
    match tag {
        "iframe" => Some("iframe"), // its `navigate=` is the guest's
        "input" if input_control(tag, attrs).is_none() => Some("field"),
        "textarea" => Some(match attrs.iter().find(|a| a.name == "markup") {
            Some(a) if !matches!(&a.value, Expr::Str(m, _) if m == "none") => "markdown",
            _ => "field",
        }),
        _ => input_control(tag, attrs),
    }
}

/// Whether a [`payload_control`] edits text: a text field's or the
/// Markdown editor's `input`, `selectionchange` and `beforeinput` carry the
/// field's `InputEvent`.
pub fn edits_text(control: Option<&str>) -> bool {
    matches!(control, Some("field" | "markdown"))
}

/// The control an `input`'s literal `type` names; `None` for a text field's
/// (`text`, `password`, `email`, …).
fn control_type(t: &str) -> Option<&'static str> {
    [
        "checkbox",
        "radio",
        "file",
        "range",
        "date",
        "time",
        "datetime-local",
    ]
    .into_iter()
    .find(|kind| t.eq_ignore_ascii_case(kind))
}

/// A non-text HTML `input` kind Exact has not admitted. HTML classifies the
/// `type` attribute ASCII-case-insensitively; return the authored spelling so
/// a refusal can name the value that would change the DOM's node kind.
pub fn unsupported_input_type(value: &Expr) -> Option<&str> {
    match value {
        Expr::Str(kind, _)
            if ["button", "submit", "reset", "image"]
                .into_iter()
                .any(|candidate| kind.eq_ignore_ascii_case(candidate)) =>
        {
            Some(kind)
        }
        Expr::Ternary(_, yes, no, _) => {
            unsupported_input_type(yes).or_else(|| unsupported_input_type(no))
        }
        Expr::Match { some, none, .. } => {
            unsupported_input_type(some).or_else(|| unsupported_input_type(none))
        }
        _ => None,
    }
}

/// Whether `value`, an `input`'s `type`, is a text field's whatever it
/// evaluates to: a text field's literal, or a choice between such literals
/// (`shown ? "text" : "password"`, a password's show and hide). The node is
/// a text field either way, so the choice can be made as the app runs, as
/// the attribute can be set on the web; any other expression could name a
/// control the compiler has to know.
pub fn text_input_type(value: &Expr) -> bool {
    match value {
        Expr::Str(t, _) => control_type(t).is_none() && unsupported_input_type(value).is_none(),
        Expr::Ternary(_, a, b, _) => text_input_type(a) && text_input_type(b),
        _ => false,
    }
}

pub use crate::part::{part_attr, PLACEHOLDER_COLOR};

/// `name=expr`.
#[derive(Debug, Clone, PartialEq)]
pub struct Attr {
    /// Name.
    pub name: String,
    /// Value.
    pub value: Expr,
    /// Where.
    pub span: Span,
    /// Whether the author wrote it: the parser's are; a
    /// row the compiler adds to an element (a sheet's, an implied one) is not.
    pub written: bool,
}

/// A binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Rem,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `and` / `&&`
    And,
    /// `or` / `||`
    Or,
}

/// A unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    /// `-`
    Neg,
    /// `not` / `!`
    Not,
}

/// An expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A number literal.
    Number(f64, Span),
    /// A string literal.
    Str(String, Span),
    /// A template string: literal and expression parts in order.
    Template(Vec<TemplatePart>, Span),
    /// `true` / `false`.
    Bool(bool, Span),
    /// `none`.
    None(Span),
    /// `[a, b, c]`: a list of its items, which unify as the arms of `?:`
    /// do. `[]`'s element type comes from where it is
    /// written (the other arm of a `match` or `?:`, a declared `T[]`,
    /// a write into the state it initializes).
    List(Vec<Expr>, Span),
    /// `some(expr)`.
    Some(Box<Expr>, Span),
    /// A name.
    Ident(String, Span),
    /// `expr.field`.
    Member(Box<Expr>, String, Span),
    /// `name(args)`.
    Call(String, Vec<Expr>, Span),
    /// An authored named surface argument.
    NamedArg(String, Box<Expr>, Span),
    /// A unary operation.
    Unary(UnOp, Box<Expr>, Span),
    /// A binary operation.
    Binary(BinOp, Box<Expr>, Box<Expr>, Span),
    /// `cond ? a : b`.
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>, Span),
    /// `match subject { case some(x) => a, case none => b }`.
    Match {
        /// Subject.
        subject: Box<Expr>,
        /// Bound name in the `some` arm.
        var: String,
        /// `some` arm.
        some: Box<Expr>,
        /// `none` arm.
        none: Box<Expr>,
        /// Where.
        span: Span,
    },
    /// `(item, index) => body`: a callback, only ever the second argument
    /// of `map`, `filter` or `sortBy`. Its
    /// zero to two parameters are the item and its index.
    Arrow {
        /// Parameter names, in order.
        params: Vec<String>,
        /// The one expression it returns.
        body: Box<Expr>,
        /// Where.
        span: Span,
    },
    /// `value` evaluated once and bound to `name` in `body`. Compiler-only:
    /// no surface syntax spells it. Expansion introduces it so a child's
    /// derive read twice, or a `fn` call repeated, is computed and emitted
    /// once rather than copied into every place it is read.
    Let {
        /// The bound name.
        name: String,
        /// Evaluated first, once.
        value: Box<Expr>,
        /// Where `name` is in scope.
        body: Box<Expr>,
        /// Where.
        span: Span,
    },
    /// `value` as the declared type it fills. Compiler-only: no surface
    /// syntax spells it. Expansion wraps a use's argument that holds a
    /// `none` or a `[]` in the prop's declared type, so `C(o=none)` for
    /// `o: option<number>` is an `option<number>` wherever the child reads it.
    Typed(Box<Expr>, TypeExpr, Span),
}

/// One part of a template string.
#[derive(Debug, Clone, PartialEq)]
pub enum TemplatePart {
    /// Literal text.
    Text(String),
    /// `${expr}`.
    Expr(Expr),
}

impl Expr {
    /// Where.
    pub fn span(&self) -> Span {
        match self {
            Expr::Number(_, s)
            | Expr::Str(_, s)
            | Expr::Template(_, s)
            | Expr::Bool(_, s)
            | Expr::None(s)
            | Expr::List(_, s)
            | Expr::Some(_, s)
            | Expr::Ident(_, s)
            | Expr::Member(_, _, s)
            | Expr::Call(_, _, s)
            | Expr::NamedArg(_, _, s)
            | Expr::Unary(_, _, s)
            | Expr::Binary(_, _, _, s)
            | Expr::Ternary(_, _, _, s)
            | Expr::Match { span: s, .. }
            | Expr::Arrow { span: s, .. }
            | Expr::Let { span: s, .. }
            | Expr::Typed(_, _, s) => *s,
        }
    }
}

/// The one candidate a misspelled `name` most plausibly meant: the same
/// letters in another case, else one spelling edit away. `None` when no
/// candidate fits or two different ones do — a refusal never guesses.
pub fn suggestion<'a>(
    name: &str,
    candidates: impl IntoIterator<Item = &'a str> + Clone,
) -> Option<&'a str> {
    let unique = |fits: &dyn Fn(&str) -> bool| {
        let mut found = None;
        for candidate in candidates.clone() {
            if candidate != name && fits(candidate) {
                if found.is_some_and(|f| f != candidate) {
                    return Err(());
                }
                found = Some(candidate);
            }
        }
        Ok(found)
    };
    match unique(&|c| c.eq_ignore_ascii_case(name)) {
        Ok(Some(found)) => Some(found),
        Err(()) => None,
        Ok(None) => unique(&|c| one_spelling_edit(name.as_bytes(), c.as_bytes()))
            .ok()
            .flatten(),
    }
}

/// One ASCII insertion, deletion, substitution, or adjacent transposition.
pub fn one_spelling_edit(a: &[u8], b: &[u8]) -> bool {
    if !a.is_ascii()
        || !b.is_ascii()
        || a.len() > 64
        || b.len() > 64
        || a.len().abs_diff(b.len()) > 1
        || a == b
    {
        return false;
    }
    let i = a.iter().zip(b).take_while(|(a, b)| a == b).count();
    match a.len().cmp(&b.len()) {
        std::cmp::Ordering::Less => a[i..] == b[i + 1..],
        std::cmp::Ordering::Greater => a[i + 1..] == b[i..],
        std::cmp::Ordering::Equal => {
            a[i + 1..] == b[i + 1..]
                || (i + 1 < a.len()
                    && a[i] == b[i + 1]
                    && a[i + 1] == b[i]
                    && a[i + 2..] == b[i + 2..])
        }
    }
}
