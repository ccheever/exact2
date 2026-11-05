//! What a node's style rows and an arm's state ask of the whole plan,
//! answered once per emission and looked up per row. Each answer is a walk of
//! the plan's nodes, slots or theme; asked per row, it would make emission
//! quadratic in the plan. Each depends only on the plan and on the
//! thread-local state `emit` installs before it writes the view
//! (`modes::every_for`, `roles::live_properties`,
//! `bridge::set_role_overrides`; `Modes::project` has undone its token edits
//! by then). Writing a row changes neither, so the first row to ask gets what
//! every row would.

use exact_kernel::style::theme::Kind;
use exact_kernel::{NodeType, PropId, StyleId};
use exact_plan::{BindingKind, Plan};
use std::collections::{HashMap, HashSet};

type Tokens = (Vec<(String, String)>, Vec<(String, StyleId, String)>);

struct StatusSites {
    bindings: HashMap<(u32, u16), Vec<u32>>,
    heads: Vec<bool>,
    regions: Vec<Vec<u32>>,
}

pub(super) struct Answers<'a> {
    plan: &'a Plan,
    /// Whether a type or family token varies by an app axis (`rows.rs`).
    pub varying: bool,
    /// Whether the plan has modes on the web ([`crate::modes::active`]).
    pub modes: bool,
    /// Each arm's own slots, in plan order (`regions.rs`).
    slots: Vec<Vec<usize>>,
    /// Navigation bindings and regions that can change the page's status.
    status: StatusSites,
    /// [`Answers::styles`].
    styles: Option<Vec<(String, String)>>,
    /// [`Answers::accent`].
    accent: Option<String>,
    /// The tokens' properties ([`crate::modes::properties`]) and the type
    /// tokens' ([`crate::measures::type_properties`]), which a measured
    /// row's map reads.
    tokens: Option<Tokens>,
    /// Each measured row's map ([`crate::measures::map`]), by its row and
    /// its `attribute` and `container` arguments.
    maps: HashMap<(StyleId, bool, bool), Option<String>>,
}

impl<'a> Answers<'a> {
    pub fn new(plan: &'a Plan, sites: &super::Sites) -> Answers<'a> {
        let mut slots = vec![Vec::new(); plan.arms.len()];
        for (k, slot) in plan.slots.iter().enumerate() {
            if let Some(own) = slot.owner.and_then(|a| slots.get_mut(a.0 as usize)) {
                own.push(k);
            }
        }
        Answers {
            plan,
            varying: plan.theme.iter().any(|r| {
                r.slices.len > 0
                    && matches!(crate::measures::kind(r), Some(Kind::Type | Kind::Family))
            }),
            modes: crate::modes::active(plan),
            slots,
            status: status_sites(plan, sites),
            styles: None,
            accent: None,
            tokens: None,
            maps: HashMap::new(),
        }
    }

    /// Arm `arm`'s own slots.
    pub fn slots(&self, arm: u32) -> &[usize] {
        self.slots.get(arm as usize).map_or(&[], Vec::as_slice)
    }

    pub fn status_binding(&self, node: u32, prop: u16) -> Option<&[u32]> {
        self.status.bindings.get(&(node, prop)).map(Vec::as_slice)
    }

    pub fn status_region(&self, region: u32) -> Option<&[u32]> {
        let owners = &self.status.regions[region as usize];
        (self.status.heads[region as usize] || !owners.is_empty()).then_some(owners)
    }

    /// Row `id`'s bound map ([`crate::measures::map`]).
    pub fn map(&mut self, id: StyleId, attribute: bool, container: bool) -> Option<&str> {
        let (plan, tokens) = (self.plan, &mut self.tokens);
        self.maps
            .entry((id, attribute, container))
            .or_insert_with(|| {
                let (props, types) = tokens.get_or_insert_with(|| {
                    let every = crate::modes::every_token();
                    (
                        crate::modes::properties(plan),
                        crate::measures::type_properties(plan, every),
                    )
                });
                crate::measures::map(plan, props, types, id, attribute, container)
            })
            .as_deref()
    }

    /// The text styles a bound size's maps read (`text_style::style_map`):
    /// the style a type token's size names, by the token's size property and
    /// by its name.
    pub fn styles(&mut self) -> &[(String, String)] {
        let plan = self.plan;
        self.styles.get_or_insert_with(|| {
            let every = crate::modes::every_token();
            let mut styles = crate::measures::size_styles(plan, every);
            for (name, id, prop) in crate::measures::type_properties(plan, every) {
                if id != StyleId::FontSize {
                    continue;
                }
                if let Some((_, style)) = styles.iter().find(|(v, _)| *v == format!("var({prop})"))
                {
                    styles.push((name, style.clone()));
                }
            }
            styles
        })
    }

    /// The properties of the tokens whose value names the accent in some
    /// tuple, as JSON ([`crate::modes::accent_properties`]).
    pub fn accent(&mut self) -> &str {
        let plan = self.plan;
        self.accent.get_or_insert_with(|| {
            serde_json::to_string(&crate::modes::accent_properties(plan)).unwrap()
        })
    }
}

pub(super) fn status_owners(owners: &[u32]) -> String {
    format!(
        "[{}]",
        owners
            .iter()
            .map(|n| format!("e{n}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn property(plan: &Plan, node: u32, id: PropId) -> Option<exact_plan::BindingsRow> {
    plan.node(exact_plan::NodesId(node))
        .bindings
        .iter()
        .map(|b| plan.binding(b))
        .find(|b| b.kind == BindingKind::Prop && b.id == id as u16)
}

fn text(plan: &Plan, node: u32, id: PropId) -> Option<String> {
    property(plan, node, id)
        .and_then(|b| crate::style::literal(plan, plan.code(b.expr)))
        .and_then(|v| v.as_str().map(str::to_owned))
}

fn role_can(plan: &Plan, node: u32, roles: &[&str]) -> bool {
    property(plan, node, PropId::AccessibilityRole).is_some()
        && text(plan, node, PropId::AccessibilityRole)
            .as_deref()
            .is_none_or(|r| roles.contains(&r))
}

fn parent_site(plan: &Plan, site: super::Site) -> Option<super::Site> {
    use super::Site;
    let (parent, arm) = match site {
        Site::Node(n) => {
            let node = plan.node(exact_plan::NodesId(n));
            (node.parent, node.arm)
        }
        Site::Region(r) => {
            let region = &plan.regions[r as usize];
            (region.parent, region.arm)
        }
    };
    parent
        .map(|p| Site::Node(p.0))
        .or_else(|| arm.map(|a| Site::Region(plan.arms[a.0 as usize].region.0)))
}

// A panel or tablist keeps this owner while present; a conditional ancestor may prevent guaranteed presence.
fn fixed_owner(plan: &Plan, node: u32, owner: u32, always: bool) -> bool {
    use super::Site;
    let mut at = parent_site(plan, Site::Node(node));
    while let Some(site) = at {
        match site {
            Site::Node(n) if n == owner => return true,
            Site::Node(n) if property(plan, n, PropId::NavigationKey).is_some() => return false,
            Site::Region(_) if always => return false,
            _ => {}
        }
        at = parent_site(plan, site);
    }
    false
}

fn status_sites(plan: &Plan, sites: &super::Sites) -> StatusSites {
    use super::Site;
    let mut out = StatusSites {
        bindings: HashMap::new(),
        heads: vec![false; plan.regions.len()],
        regions: vec![Vec::new(); plan.regions.len()],
    };
    if !(0..plan.nodes.len()).any(|n| property(plan, n as u32, PropId::HeadStatus).is_some()) {
        return out;
    }
    let children = crate::style::element_children(plan, sites);
    let mut parents = vec![None; plan.nodes.len()];
    for (i, list) in children.iter().enumerate() {
        for &child in list {
            parents[child as usize] = Some(i);
        }
    }
    let keys: Vec<_> = (0..plan.nodes.len())
        .map(|i| text(plan, i as u32, PropId::NavigationKey))
        .collect();
    let mut order = Vec::new();
    let mut walk = vec![sites.root];
    while let Some(n) = walk.pop() {
        order.push(n);
        walk.extend(children[n as usize].iter().rev());
    }
    let mut tabs = vec![false; plan.nodes.len()];
    let mut first_list = vec![None; plan.nodes.len()];
    let mut unknown_ids = vec![false; plan.nodes.len()];
    let mut ids = HashMap::new();
    for &n in &order {
        if role_can(plan, n, &["tab", "tablist", "tabpanel"]) {
            let mut at = Some(n as usize);
            while let Some(i) = at {
                if std::mem::replace(&mut tabs[i], true) {
                    break;
                }
                at = parents[i];
            }
        }
        if role_can(plan, n, &["tablist"]) {
            let mut at = parents[n as usize];
            while let Some(i) = at {
                if first_list[i].is_some() {
                    break;
                }
                first_list[i] = Some(n);
                at = parents[i];
            }
        }
        if property(plan, n, PropId::Id).is_some() {
            if let Some(id) = text(plan, n, PropId::Id) {
                ids.entry(id).and_modify(|n| *n = None).or_insert(Some(n));
            } else {
                let mut at = parents[n as usize];
                while let Some(i) = at {
                    if std::mem::replace(&mut unknown_ids[i], true) {
                        break;
                    }
                    at = parents[i];
                }
            }
        }
    }
    let selected = |parent: u32, key: &str| {
        sites
            .of_node(parent)
            .iter()
            .any(|s| matches!(s, Site::Node(n) if keys[*n as usize].as_deref() == Some(key)))
    };
    // Exclude a literal route only while a fixed selection always names a matching route.
    // For tabs, the first possible tablist must always name a stable panel holding that route.
    let mut fixed = vec![None; plan.nodes.len()];
    for &n in &order {
        let Some(key) = keys[n as usize].as_deref() else {
            continue;
        };
        if text(plan, n, PropId::NavigationBack).is_none() {
            continue;
        }
        if !tabs[n as usize] {
            if selected(n, key) {
                fixed[n as usize] = Some(key.to_owned());
            }
            continue;
        }
        let Some(list) = first_list[n as usize] else {
            continue;
        };
        if unknown_ids[n as usize]
            || text(plan, list, PropId::AccessibilityRole).as_deref() != Some("tablist")
            || !fixed_owner(plan, list, n, true)
        {
            continue;
        }
        let mut panels = Vec::new();
        for site in sites.of_node(list) {
            let Site::Node(tab) = *site else {
                continue;
            };
            if text(plan, tab, PropId::AccessibilityRole).as_deref() != Some("tab") {
                continue;
            }
            let Some(id) = text(plan, tab, PropId::AccessibilityControls) else {
                continue;
            };
            let Some(Some(panel)) = ids.get(&id) else {
                continue;
            };
            if text(plan, *panel, PropId::AccessibilityRole).as_deref() == Some("tabpanel")
                && fixed_owner(plan, *panel, n, false)
            {
                panels.push(*panel);
            }
        }
        if panels
            .iter()
            .any(|&p| fixed_owner(plan, p, n, true) && selected(p, key))
        {
            for panel in panels {
                fixed[panel as usize] = Some(key.to_owned());
            }
        }
    }
    let mut nodes = vec![false; plan.nodes.len()];
    let mut heads = Vec::new();
    let mut walk = vec![(sites.root, false)];
    while let Some((n, off)) = walk.pop() {
        let off = off
            || parents[n as usize].is_some_and(|p| {
                fixed[p]
                    .as_deref()
                    .is_some_and(|key| keys[n as usize].as_deref().is_some_and(|k| k != key))
            });
        if !off
            && plan.node(exact_plan::NodesId(n)).node_type == NodeType::Head as u8
            && property(plan, n, PropId::HeadStatus).is_some()
        {
            heads.push(Site::Node(n));
        }
        walk.extend(children[n as usize].iter().map(|&c| (c, off)));
    }
    for mut site in heads {
        loop {
            let seen = match site {
                Site::Node(n) => std::mem::replace(&mut nodes[n as usize], true),
                Site::Region(r) => std::mem::replace(&mut out.heads[r as usize], true),
            };
            if seen {
                break;
            }
            let Some(parent) = parent_site(plan, site) else {
                break;
            };
            site = parent;
        }
    }
    // A literal key blocks ownership by outer navigation roots. A computed key may clear, so keep both possibilities.
    let mut walk = vec![(sites.root, Vec::<u32>::new())];
    let mut seen = HashSet::new();
    while let Some((n, owners)) = walk.pop() {
        let root = property(plan, n, PropId::NavigationBack).is_some();
        for prop in [
            PropId::NavigationKey,
            PropId::NavigationBack,
            PropId::AccessibilityRole,
            PropId::AccessibilityControls,
            PropId::Id,
        ] {
            if property(plan, n, prop).is_none() {
                continue;
            }
            let relevant = match prop {
                PropId::AccessibilityRole => role_can(plan, n, &["tab", "tablist", "tabpanel"]),
                PropId::AccessibilityControls => role_can(plan, n, &["tab"]),
                PropId::Id => role_can(plan, n, &["tabpanel"]),
                _ => true,
            };
            if !relevant {
                continue;
            }
            let mut targets: Vec<_> = owners
                .iter()
                .copied()
                .filter(|&owner| {
                    prop != PropId::NavigationBack
                        && (prop != PropId::NavigationKey
                            || tabs[owner as usize]
                            || parents[n as usize] == Some(owner as usize))
                })
                .collect();
            if root
                && nodes[n as usize]
                && matches!(prop, PropId::NavigationKey | PropId::NavigationBack)
            {
                targets.push(n);
            }
            if targets.is_empty() {
                continue;
            }
            for &owner in &targets {
                let mut site = Site::Node(n);
                loop {
                    if matches!(site, Site::Node(i) if i == owner) {
                        break;
                    }
                    let mark = match site {
                        Site::Node(i) => (owner, 0, i),
                        Site::Region(i) => (owner, 1, i),
                    };
                    if !seen.insert(mark) {
                        break;
                    }
                    if let Site::Region(r) = site {
                        out.regions[r as usize].push(owner);
                    }
                    let Some(parent) = parent_site(plan, site) else {
                        break;
                    };
                    site = parent;
                }
            }
            out.bindings.insert((n, prop as u16), targets);
        }
        let mut next = if keys[n as usize].is_some() {
            Vec::new()
        } else {
            owners
        };
        if root && nodes[n as usize] {
            next.push(n);
        }
        walk.extend(children[n as usize].iter().map(|&c| (c, next.clone())));
    }
    for owners in &mut out.regions {
        owners.sort_unstable();
        owners.dedup();
    }
    out
}
