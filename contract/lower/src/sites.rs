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
}

/// Where a bound row came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// The node's own attribute.
    Own,
    /// A `class=Name` style's row.
    Class(String),
    /// The tag's fixed row (`row` sets `flex_direction`).
    Tag,
}

impl Origin {
    /// The map's spelling: `own`, `class:<Name>`, `tag`.
    pub fn label(&self) -> String {
        match self {
            Origin::Own => "own".into(),
            Origin::Class(name) => format!("class:{name}"),
            Origin::Tag => "tag".into(),
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
            actions: root
                .actions
                .iter()
                .zip(&ex.action_instances)
                .map(|(a, i)| declared(&a.name, a.span, *i))
                .collect(),
        }
    }
}

/// A node's site: its element and every style row it binds, by origin.
pub(crate) fn node_site(
    span: Span,
    instance: u32,
    bindings: &[BindingsRow],
    origins: &[Origin],
) -> NodeSite {
    let rows = bindings
        .iter()
        .zip(origins)
        .filter(|(b, _)| b.kind == BindingKind::Style)
        .filter_map(|(b, o)| StyleId::from_bit(b.id as u32).map(|row| (row, o.clone())))
        .collect();
    NodeSite {
        span,
        instance,
        rows,
    }
}
