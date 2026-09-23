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
    /// `expect text "testId" == "value"`: the node's `text` prop.
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
    /// nearest enclosing `provide name = expr` fills them (LLP 1017 P4a).
    pub injects: Vec<Param>,
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

/// `resource name = source(args) as shape T`.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceDecl {
    /// Name.
    pub name: String,
    /// The data source's name.
    pub source: String,
    /// Argument expressions.
    pub args: Vec<Expr>,
    /// The declared shape.
    pub shape: TypeExpr,
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

/// `action name(params) writes a, b` with a body of statements.
#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    /// Name.
    pub name: String,
    /// Parameters.
    pub params: Vec<Param>,
    /// The `writes` list.
    pub writes: Vec<(String, Span)>,
    /// Statements.
    pub body: Vec<Stmt>,
    /// Where.
    pub span: Span,
}

/// A statement in an action body.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
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

/// `task name mount` with `every(ms, action)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// Name.
    pub name: String,
    /// `every(interval, action)`, the one v1 body.
    pub every: (Expr, String, Span),
    /// Where.
    pub span: Span,
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
    /// `provide name = expr` with children: every component used below that
    /// declares `inject name` reads `expr`, the innermost `provide` winning;
    /// the compiler fills it at inlining — no runtime lookup (LLP 1017 P4a).
    Provide {
        /// The provided name.
        name: String,
        /// The value, an expression in the providing scope.
        expr: Expr,
        /// The subtree it covers.
        body: Vec<Node>,
        /// Where.
        span: Span,
    },
    /// `children` — where a `slot` component's use puts the nodes indented
    /// under it (LLP 1017 P4b).
    Children {
        /// Where.
        span: Span,
    },
    /// `when cond … else …`.
    When {
        /// Condition.
        cond: Expr,
        /// Then-branch.
        then: Vec<Node>,
        /// Else-branch (possibly empty).
        otherwise: Vec<Node>,
        /// Where.
        span: Span,
    },
    /// `each x in list key=expr`. `tag` is the inliner's: unique per `each`
    /// in the expanded root, so a row slot can name the `each` that owns it
    /// before regions exist (LLP 1017 P4c); 0 as parsed.
    Each {
        /// The inliner's tag.
        tag: u32,
        /// The item variable.
        var: String,
        /// The list.
        list: Expr,
        /// The key expression (may name `var`).
        key: Expr,
        /// Body.
        body: Vec<Node>,
        /// Where.
        span: Span,
    },
    /// `match subject` with `case some(x)` and `case none` arms.
    Match {
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
            | Node::Provide { span, .. }
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
            | Expr::Some(_, s)
            | Expr::Ident(_, s)
            | Expr::Member(_, _, s)
            | Expr::Call(_, _, s)
            | Expr::NamedArg(_, _, s)
            | Expr::Unary(_, _, s)
            | Expr::Binary(_, _, _, s)
            | Expr::Ternary(_, _, _, s)
            | Expr::Match { span: s, .. }
            | Expr::Let { span: s, .. } => *s,
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
