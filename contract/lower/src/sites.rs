//! The development map's half that only lowering knows (LLP 1035.005 D3,
//! 1035.002 D6): which plan node each element became and, per style row a
//! node binds, whether the binding came from the node's own attribute, from
//! a `class=` style, or from the tag itself. [`lower_all`](crate::lower_all)
//! returns it beside the plan; the plan's bytes never carry any of it.

use contract_syntax::{Expanded, Instance, Span};
use exact_kernel::StyleId;
use exact_plan::{BindingKind, BindingsRow};

/// Where every row the map describes came from, in plan order.
#[derive(Debug, Clone)]
pub struct Sites {
    /// Every component instantiation, the root first (`Expanded::instances`).
    pub instances: Vec<Instance>,
    /// One per plan node, in the `nodes` table's order.
    pub nodes: Vec<NodeSite>,
    /// The plan's slots: the declaring `state` or `mutation`.
    pub slots: Vec<Declared>,
    /// The plan's derives (a child's never reach the plan).
    pub derives: Vec<Declared>,
    /// The plan's resources, including a placeholder's owner declaration.
    pub resources: Vec<Declared>,
    /// The plan's actions, a child's lifted `name#N` included.
    pub actions: Vec<Declared>,
}

/// One plan node's declaration.
#[derive(Debug, Clone)]
pub struct NodeSite {
    /// The element in its component's view.
    pub span: Span,
    /// The instantiation it was expanded in (`Sites::instances`).
    pub instance: u32,
    /// Every style row the node binds, with where the binding came from —
    /// the winner when two bindings named one row.
    pub rows: Vec<(StyleId, Origin)>,
    /// Each host-known state's look rows (`hover`, `focus-visible`), each
    /// an [`Origin::State`] at its part's row (LLP 1122 queue item (1)).
    pub looks: Vec<(&'static str, Vec<(StyleId, Origin)>)>,
    /// The button's compile-time appearance decision, absent on other tags.
    pub button: Option<ButtonSite>,
    /// The part its element is named as (LLP 1139 D2, D5): what the
    /// selection report's `library` joins, since `part=` reaches no host.
    pub part: Option<String>,
    /// The declaring identity of the exported library component whose own
    /// view the element is of (LLP 1139 D0, D5); a slot fill's nodes are
    /// their caller's.
    pub library: Option<String>,
}

/// Development-only button admission and migration facts (LLP 1104 D2, D9).
#[derive(Debug, Clone)]
pub struct ButtonSite {
    /// Whether lowering chose the platform's control.
    pub native: bool,
    /// The first reason a default button became bare; explicit `none` has none.
    pub bare_reason: Option<String>,
    /// A default native button with authored rows beyond layout needs `none`
    /// to preserve its previous appearance during D9's source migration.
    pub migrate: bool,
}

/// A named declaration's site.
#[derive(Debug, Clone)]
pub struct Declared {
    /// The plan's name for it.
    pub name: String,
    /// Where it is declared.
    pub span: Span,
    /// The instantiation whose component declared it.
    pub instance: u32,
    /// Device facts hoisted once for a component, independent of its uses.
    pub component: Option<String>,
}

/// The attribute an author wrote that a row came from (LLP 1122 D6): its
/// span, and its name as written (a shorthand's for an expansion).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// The attribute's span.
    pub span: Span,
    /// Its name as written.
    pub name: String,
}

/// Where a bound row came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// The node's own attribute; none where the compiler derived the row.
    Own(Option<Written>),
    /// A `class=Name` style's row, and the style's attribute.
    Class(String, Option<Written>),
    /// The tag's fixed row (`row` sets `flex_direction`).
    Tag,
    /// A state part's row: `:active`'s lowered onto the node as its press
    /// paint (LLP 1121 D8), or a `:hover` or `:focus-visible` look's row;
    /// the node's or its class's, and where it is written.
    State {
        /// The node's or its class's.
        of: Box<Origin>,
        /// The state, as written after its colon.
        state: &'static str,
        /// The state part's row.
        span: Span,
    },
    /// A row a `:disabled` or `:checked` part folds into one value over
    /// its base (LLP 1122's queue item (2)): the base's attribute where an
    /// author wrote it, none for one the node takes unwritten, and each
    /// part folded over it (each an [`Origin::State`]).
    Folded {
        /// The base's origin and attribute; none where no author wrote it.
        base: Option<Box<Origin>>,
        /// Each part folded over it.
        parts: Vec<Origin>,
    },
}

impl Origin {
    /// The map's spelling: `own`, `class:<Name>`, `tag` (a state part's row,
    /// its node's or class's).
    pub fn label(&self) -> String {
        match self {
            Origin::Own(_) => "own".into(),
            Origin::Class(name, _) => format!("class:{name}"),
            Origin::Tag => "tag".into(),
            Origin::State { of, .. } => of.label(),
            Origin::Folded { base, .. } => {
                base.as_ref().map_or_else(|| "own".into(), |b| b.label())
            }
        }
    }

    /// The attribute the author wrote for it, where there is one: an own
    /// row's or its class's (LLP 1122 D6). A state part's row says its own
    /// place (`State::span`).
    pub fn written(&self) -> Option<&Written> {
        match self {
            Origin::Own(w) | Origin::Class(_, w) => w.as_ref(),
            Origin::Tag | Origin::State { .. } => None,
            Origin::Folded { base, .. } => base.as_ref().and_then(|b| b.written()),
        }
    }
}

impl Sites {
    /// The declarations' sites from the expansion; nodes are pushed as the
    /// lowering emits them.
    pub(crate) fn declared(ex: &Expanded) -> Sites {
        let root = &ex.root;
        let declared = |name: &str, span: Span, instance: u32| Declared {
            name: name.to_string(),
            span,
            instance,
            component: None,
        };
        let resource = |name: &str, r: &contract_syntax::ResourceDecl| {
            let mut site = declared(name, r.span, 0);
            site.component = r
                .name
                .rsplit_once('@')
                .map(|(_, component)| component.to_string());
            site
        };
        Sites {
            instances: ex.instances.clone(),
            nodes: Vec::new(),
            slots: root
                .states
                .iter()
                .zip(&ex.state_instances)
                .map(|(s, i)| declared(&s.name, s.span, *i))
                .chain(root.mutations.iter().map(|m| declared(&m.name, m.span, 0)))
                .collect(),
            derives: root
                .derives
                .iter()
                .map(|d| declared(&d.name, d.span, 0))
                .collect(),
            resources: root
                .resources
                .iter()
                .map(|r| resource(&r.name, r))
                .chain(root.resources.iter().filter_map(|r| {
                    r.placeholder
                        .as_ref()
                        .filter(|p| p.source != contract_types::placeholder::EMPTY)
                        .map(|_| resource(&format!("{}#else", r.name), r))
                }))
                .collect(),
            actions: root
                .actions
                .iter()
                .zip(&ex.action_instances)
                .map(|(a, i)| declared(&a.name, a.span, *i))
                .collect(),
        }
    }
}

/// A node's site: its element, every style row it binds, by origin, and
/// its looks' rows.
pub(crate) fn node_site(
    span: Span,
    instance: u32,
    button: Option<ButtonSite>,
    bindings: &[BindingsRow],
    origins: &[Origin],
    looks: Vec<(&'static str, Vec<(StyleId, Origin)>)>,
) -> NodeSite {
    NodeSite {
        span,
        instance,
        rows: style_rows(bindings, origins),
        looks,
        button,
        part: None,
        library: None,
    }
}

/// The style rows of `bindings`, each with its origin.
pub(crate) fn style_rows(bindings: &[BindingsRow], origins: &[Origin]) -> Vec<(StyleId, Origin)> {
    bindings
        .iter()
        .zip(origins)
        .filter(|(b, _)| b.kind == BindingKind::Style)
        .filter_map(|(b, o)| StyleId::from_bit(b.id as u32).map(|row| (row, o.clone())))
        .collect()
}
