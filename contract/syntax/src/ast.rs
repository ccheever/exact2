//! The abstract syntax tree.

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
    /// The app's router declaration. @ref LLP 1038 D2/D3.
    pub routes: Option<RoutesDecl>,
    /// `use Name from "./file.contract"` declarations, in order (LLP 1017 P8);
    /// resolved by the driver, which merges the used file's declarations in.
    pub uses: Vec<UseDecl>,
    /// `font "Name"` declarations, in order (LLP 1019 D1).
    pub fonts: Vec<FontDecl>,
    /// `shape` declarations, in order.
    pub shapes: Vec<ShapeDecl>,
    /// `style` declarations, in order (LLP 1017 P6).
    pub styles: Vec<StyleDecl>,
    /// `keyframes` declarations, in order (LLP 1055 D5): CSS `@keyframes`,
    /// global by name as in CSS.
    pub keyframes: Vec<KeyframesDecl>,
    /// `fn` declarations, in order (LLP 1017 P5).
    pub fns: Vec<FnDecl>,
    /// `test` declarations, in order (LLP 1017 P7) — normally in a file of
    /// their own beside the app, `app.test.contract`.
    pub tests: Vec<TestDecl>,
    /// `component` declarations, in order. The first is the root.
    pub components: Vec<Component>,
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

/// `routes <slot>` with rows in declaration order. @ref LLP 1038 D2.
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
    /// written (LLP 1048.003 D5); lowering reads them.
    pub fields: Vec<Attr>,
    /// Where.
    pub span: Span,
}

/// A declared font family. Faces are static in v1: one path, weight, style.
#[derive(Debug, Clone, PartialEq)]
pub struct FontDecl {
    /// The Contract alias; hosts bind this name to these bytes.
    pub name: String,
    /// Its static faces.
    pub faces: Vec<FontFaceDecl>,
    /// Where.
    pub span: Span,
}

/// One static face in a [`FontDecl`].
#[derive(Debug, Clone, PartialEq)]
pub struct FontFaceDecl {
    /// CSS weight 1–1000.
    pub weight: u16,
    /// Italic, rather than normal.
    pub italic: bool,
    /// App-relative TTF/OTF source.
    pub source: String,
    /// Where.
    pub span: Span,
}

/// `test "name"` with steps: the agent API's own operations (LLP 1012) and
/// `expect` lines that read their replies — compiled to a script the agent
/// driver runs against the real hosts; never a second evaluator (LLP 1017 P7).
#[derive(Debug, Clone, PartialEq)]
pub struct TestDecl {
    /// The name.
    pub name: String,
    /// The steps, in order.
    pub steps: Vec<Step>,
    /// Where.
    pub span: Span,
}

/// One step of a `test`.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// `tap "testId"` (`hover` for a pointer over).
    Tap {
        /// The node, by `testId`.
        target: String,
        /// `hover` instead of a press.
        hover: bool,
        /// Where.
        span: Span,
    },
    /// `tap "testId" drag dx dy [from x y] [mouse] [press ms] [over ms]
    /// [hold ms]`: one whole drag from the node's middle, or from `from` in
    /// its box, a finger's or the left button's; the driver's `tap … drag`
    /// (kanban F18, files diary F10).
    Drag {
        /// The node, by `testId`.
        target: String,
        /// Points across.
        dx: f64,
        /// Points down.
        dy: f64,
        /// Where it starts, an offset from the node's top left; its middle
        /// when `None`.
        from: Option<(f64, f64)>,
        /// The left button rather than a finger, where a carrier has both.
        mouse: bool,
        /// Milliseconds held before the move.
        press: Option<f64>,
        /// Milliseconds the move takes.
        over: Option<f64>,
        /// Milliseconds held after the move.
        hold: Option<f64>,
        /// Where.
        span: Span,
    },
    /// `size 1200x800`: the viewport the test's session opens at, its first
    /// step (the driver's `--size`; kanban F18, paint F5).
    Size {
        /// Points across.
        width: f64,
        /// Points down.
        height: f64,
        /// Where.
        span: Span,
    },
    /// `type "testId" "text"`.
    Type {
        /// The field, by `testId`.
        target: String,
        /// The text.
        text: String,
        /// Where.
        span: Span,
    },
    /// `type "testId" key "Enter"`.
    Key {
        /// The field, by `testId`.
        target: String,
        /// The key's web name.
        key: String,
        /// Where.
        span: Span,
    },
    /// `clock settle`, `clock +ms`, `clock ms`.
    Clock {
        /// The argument as the agent takes it.
        arg: String,
        /// Where.
        span: Span,
    },
    /// `screenshot "file.png"`.
    Screenshot {
        /// The file.
        path: String,
        /// Where.
        span: Span,
    },
    /// `expect tree has "testId"` / `expect tree missing "testId"`.
    ExpectTree {
        /// The node, by `testId`.
        target: String,
        /// Present, or absent.
        present: bool,
        /// Where.
        span: Span,
    },
    /// `expect text "testId" == "value"`: the node's text — its `text` prop,
    /// else its descendants' text in order (the web's `textContent`), else a
    /// field's value.
    ExpectText {
        /// The node, by `testId`.
        target: String,
        /// The text.
        value: String,
        /// Where.
        span: Span,
    },
    /// `expect state name == literal`: a slot, derive, or resource from the
    /// `state` reply, compared to a number, string, bool, or `none`.
    ExpectState {
        /// The declaration's name.
        name: String,
        /// The literal.
        value: Expr,
        /// Where.
        span: Span,
    },
}

/// `fn name(param: type, …): type = expr` — a pure function written in
/// Contract: one expression over its parameters and the roster, expanded
/// inline wherever it is called (LLP 1017 P5). No recursion, no loops, no
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

/// `use Name from "./file.contract"` — a component, shape, or style from
/// another Contract file; never anything else (`contract-no-imports`).
#[derive(Debug, Clone, PartialEq)]
pub struct UseDecl {
    /// The declaration's name.
    pub name: String,
    /// The file, relative to this one.
    pub path: String,
    /// Where.
    pub span: Span,
}

/// `style Name` with lines of `attr=literal` — a named set of style rows a
/// node applies with `class=Name`; its own attributes win (LLP 1017 P6).
#[derive(Debug, Clone, PartialEq)]
pub struct StyleDecl {
    /// The name.
    pub name: String,
    /// The rows, as attributes with literal values.
    pub attrs: Vec<Attr>,
    /// Where.
    pub span: Span,
}

/// `keyframes Name`: CSS `@keyframes` (LLP 1055 D5).
#[derive(Debug, Clone, PartialEq)]
pub struct KeyframesDecl {
    /// The name `animation-name` refers to.
    pub name: String,
    /// The keyframes, in source order.
    pub frames: Vec<KeyframeDecl>,
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

/// A written type: `number`, `string`, `bool`, a shape name, `option<T>`, `list<T>`.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    /// A named type: a primitive or a shape.
    Named(String, Span),
    /// `option<T>`.
    Option(Box<TypeExpr>, Span),
    /// `list<T>`.
    List(Box<TypeExpr>, Span),
}

impl TypeExpr {
    /// Where it was written.
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Named(_, s) | TypeExpr::Option(_, s) | TypeExpr::List(_, s) => *s,
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
    /// nearest enclosing component's `provide` section fills them (LLP 1017
    /// P4a, LLP 1035.005.000 D9).
    pub injects: Vec<Param>,
    /// The `provide` section (LLP 1035.005.000 D9): each binding fills the
    /// same-named `inject` of every component used in this component's view,
    /// unless a nearer component's section provides it too. A bare name is
    /// `name = name`; the span is the name's.
    pub provides: Vec<Binding>,
    /// Whether the component declares `slot`: the nodes indented under a use
    /// of it fill its `children` node (LLP 1017 P4b).
    pub slot: bool,
    /// `state` declarations.
    pub states: Vec<Binding>,
    /// `derive` declarations.
    pub derives: Vec<Binding>,
    /// `resource` declarations.
    pub resources: Vec<ResourceDecl>,
    /// `mutation` declarations (LLP 1016).
    pub mutations: Vec<MutationDecl>,
    /// `action` declarations.
    pub actions: Vec<Action>,
    /// `task` declarations.
    pub tasks: Vec<Task>,
    /// The view's top-level nodes.
    pub view: Vec<Node>,
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
    /// arguments identify the answer. @ref LLP 1027.005 D6.
    pub identity: Option<usize>,
    /// The declared shape.
    pub shape: TypeExpr,
    /// `else source(args)` (LLP 1048.003 D6): what shows while the source
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

/// `mutation name as shape T` (LLP 1016): an `option<T>` slot, `none` at
/// boot, that a `send` fills from an action.
#[derive(Debug, Clone, PartialEq)]
pub struct MutationDecl {
    /// Name.
    pub name: String,
    /// The reply's shape, `T`.
    pub shape: TypeExpr,
    /// `refreshes a, b`: resources the runner re-asks, forced, when a send
    /// to this mutation runs and when its reply lands (LLP 1054.000.000 D1).
    pub refreshes: Vec<(String, Span)>,
    /// `then action`: run after each answer lands (LLP 1016.001), and where.
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
}

impl Action {
    /// Every slot the body assigns or sends, through every branch of its
    /// `if`s and `match`es, in statement order with repeats. An action's
    /// effects are inferred, never declared (LLP 1035.005.000 D1).
    pub fn effects(&self) -> Vec<Effect<'_>> {
        fn walk<'a>(stmts: &'a [Stmt], out: &mut Vec<Effect<'a>>) {
            for stmt in stmts {
                match stmt {
                    Stmt::Assign { target, span, .. } => out.push(Effect {
                        target,
                        span: *span,
                        send: false,
                    }),
                    Stmt::Send { target, span, .. } => out.push(Effect {
                        target,
                        span: *span,
                        send: true,
                    }),
                    Stmt::If {
                        then, otherwise, ..
                    } => {
                        walk(then, out);
                        walk(otherwise, out);
                    }
                    Stmt::Match { some, none, .. } => {
                        walk(&some.1, out);
                        walk(none, out);
                    }
                    Stmt::Command { .. } | Stmt::Refresh { .. } | Stmt::Let { .. } => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.body, &mut out);
        out
    }
}

/// A statement in an action body.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `let name = expr` (LLP 1035.005.000 D2): an immutable local, read by
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
    /// `send target = source(args)` — the mutation's request (LLP 1016).
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
    /// `if cond` … `else` … — a branch of statements (LLP 1017 P2).
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
    /// statements (LLP 1017 P2).
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
}

/// `task name mount` with `every(ms, action)`, `every(frame, action)` or
/// `after(ms, action)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// Name.
    pub name: String,
    /// Whether the timer repeats or fires once.
    pub kind: TaskKind,
    /// `(ms, action)` and the entry's span.
    pub timer: (Expr, String, Span),
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
    /// `every(frame, action)`: fires once per presented frame (LLP 1073);
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
        /// The inliner's (LLP 1035.005 D3): the index into
        /// [`Expanded::instances`](crate::Expanded::instances) of the
        /// component instantiation this element was expanded in — the
        /// root's own elements are 0, a used component's are its use's;
        /// 0 as parsed. The development map and refusal diagnostics read it.
        instance: u32,
    },
    /// `Name(arg=expr, …)`, with the nodes indented under it filling the
    /// component's `slot` (LLP 1017 P4b).
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
    /// under it (LLP 1017 P4b).
    Children {
        /// Where.
        span: Span,
    },
    /// `when cond … else …`. `tag` is the inliner's, as on `Each`: its
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
    /// `each x in list key=expr`, or `each x, i in list key=expr` binding
    /// the item's position too (LLP 1062 D8). `tag` is the inliner's: unique
    /// per region (`each`, `when`, `match`) in the expanded root, so a
    /// lifted slot can name the region that owns it before regions exist
    /// (LLP 1017 P4c); 0 as parsed.
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
/// view's scroll container is the page's (LLP 1048.003 D4). It is a word,
/// never a name in scope, so checking and lowering read it by spelling.
pub fn is_scroll_document(tag: &str, positional: &Expr) -> bool {
    tag == "scroll" && matches!(positional, Expr::Ident(word, _) if word == "document")
}

/// Whether `positional` is the word `switch` in `input type="checkbox"
/// switch`: HTML's boolean attribute, a checkbox drawn as a switch (LLP
/// 1069.001 D1). A word like `scroll document`, never a name in scope.
pub fn is_input_switch(tag: &str, positional: &Expr) -> bool {
    tag == "input" && matches!(positional, Expr::Ident(word, _) if word == "switch")
}

/// Whether `positional` is the word `multiple` in `input type="file"
/// multiple`: HTML's boolean attribute (LLP 1069.002 D1), a word like
/// `switch`, never a name in scope. `multiple=true` says the same.
pub fn is_input_multiple(tag: &str, positional: &Expr) -> bool {
    tag == "input" && matches!(positional, Expr::Ident(word, _) if word == "multiple")
}

/// The form control an `input` is, by its literal `type` (LLP 1069.001 D1):
/// `Some("checkbox")` for a checkbox (a switch is one too), whose `change`
/// and `input` carry a bool; `Some("file")` for a file input (LLP 1069.002
/// D1), whose `change` carries a `list<Picked>`; `Some("select")` for a
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

/// The control an `input`'s literal `type` names; `None` for a text field's
/// (`text`, `password`, `email`, …).
fn control_type(t: &str) -> Option<&'static str> {
    [
        "checkbox",
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
            if ["radio", "button", "submit", "reset", "image"]
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
/// control the compiler has to know (LLP 1069.001 D1).
pub fn text_input_type(value: &Expr) -> bool {
    match value {
        Expr::Str(t, _) => control_type(t).is_none() && unsupported_input_type(value).is_none(),
        Expr::Ternary(_, a, b, _) => text_input_type(a) && text_input_type(b),
        _ => false,
    }
}

/// `name=expr`.
#[derive(Debug, Clone, PartialEq)]
pub struct Attr {
    /// Name.
    pub name: String,
    /// Value.
    pub value: Expr,
    /// Where.
    pub span: Span,
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
    /// `[]`: the empty list. Its element type comes from where it is
    /// written (the other arm of a `match` or `?:`, a declared `list<T>`,
    /// a write into the state it initializes); Contract has no list literal
    /// with items (LLP 1017.003 D4).
    EmptyList(Span),
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
    /// of `map` or `filter` (LLP 1017.003 D1). Its zero to two parameters
    /// are the item and its index.
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
            | Expr::EmptyList(s)
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
