//! What a shell answers its platform's accessibility services with (LLP
//! 1115.001 D13). Android's reader has one view, and what is in it is
//! painted, so TalkBack sees nothing unless the reader's view answers for
//! it: an `AccessibilityNodeProvider` whose virtual nodes are these
//! (`host/android/canvas`, `Access.kt`). Desktop Linux has no shell that
//! asks (LLP 1015 §7: no AT-SPI tree), and nothing here runs there.
//!
//! **Nothing is built until the shell asks.** The tree is made in
//! [`Presenter::reader_access`], which the reader calls only while an
//! accessibility service is on. It is made again only when a commit, a
//! paint, a scroll, a bar or the top layer moved on since the last one, and
//! told only when it differs from what was last told.
//!
//! **What a node is.** From the kernel's tree, as the Apple hosts read it
//! (`Accessibility.swift`): a pressable is one element named by its label,
//! else its words (those of a control inside it are that control's); a
//! control is its platform kind with its state; a text outside any of them
//! is itself, a heading where its role says so; an image is one only where
//! it is named; a scroll container that scrolls holds what is in it; a
//! `tablist` holds its tabs; a live region is kept so that its change is
//! spoken. Everything else is no node, and its children stand under the
//! node above. What is hidden, inert, `aria-hidden`, under a route that
//! does not show, or out of what its scrollers show is left out.
//!
//! **Where the bars and the top layer are.** A header the top app bar
//! replaced and the root `tablist` the navigation bar replaced are not in
//! the page (`bars.rs`): their nodes are read from the bars, first and
//! last, so the order is bar, content, navigation bar. While a surface of
//! the top layer is open (`top.rs`) the tree is that surface alone, in its
//! own window's space: the page under it cannot be reached.
//!
//! **Identity** ([`identity`]) is what a platform keeps its place by, so it
//! must outlive a commit, a scroll, and a row that leaves and returns. A
//! node's is its view id, which the runner keeps while the view lives.
//! Under a list's row it is derived from the list's own identity, the
//! row's item key and the node's place in the row, since a row's views are
//! given to another item when it scrolls away (LLP 1078).
//!
//! **A hosted text field** is the reader's own view (D8) and reports
//! itself: its node here says only where it stands in the order and what
//! its author named it.
use super::requests::string;
use super::scrollers::Scroller;
use super::top::shape::Plan;
use super::*;
use exact_kernel::{ControlKind, Display, Kernel, NodeRef};
use std::collections::BTreeSet;

/// What a node is, as the reader maps it to a class TalkBack knows.
pub mod role {
    /// A container with no role of its own.
    pub const GROUP: i32 = 0;
    /// Text.
    pub const TEXT: i32 = 1;
    /// A heading.
    pub const HEADING: i32 = 2;
    /// A button.
    pub const BUTTON: i32 = 3;
    /// A link.
    pub const LINK: i32 = 4;
    /// A checkbox.
    pub const CHECKBOX: i32 = 5;
    /// A switch.
    pub const SWITCH: i32 = 6;
    /// A radio button.
    pub const RADIO: i32 = 7;
    /// A slider: a range a person sets.
    pub const SLIDER: i32 = 8;
    /// A named image.
    pub const IMAGE: i32 = 9;
    /// A tab of a `tablist`, or a destination of the navigation bar.
    pub const TAB: i32 = 10;
    /// A scroll container.
    pub const SCROLLER: i32 = 11;
    /// Progress: a range a person only reads, or one with no value.
    pub const PROGRESS: i32 = 12;
    /// A text field the presenter paints.
    pub const EDIT: i32 = 13;
    /// A text field the shell hosts: its own view stands here.
    pub const FIELD: i32 = 14;
    /// A `tablist`, or the navigation bar.
    pub const TABLIST: i32 = 15;
    /// A `select`.
    pub const DROPDOWN: i32 = 16;
    /// A button that stays pressed (`aria-pressed`).
    pub const TOGGLE: i32 = 17;
    /// The top app bar's Up arrow, which the platform names.
    pub const UP: i32 = 18;
}

/// A node's flags.
pub mod flag {
    /// A press does something.
    pub const CLICK: i32 = 1;
    /// A held press opens a context menu.
    pub const LONG_CLICK: i32 = 1 << 1;
    /// It has a checked state.
    pub const CHECKABLE: i32 = 1 << 2;
    /// It is checked, on or pressed.
    pub const CHECKED: i32 = 1 << 3;
    /// It is selected.
    pub const SELECTED: i32 = 1 << 4;
    /// It is disabled, or says it is (`aria-disabled`).
    pub const DISABLED: i32 = 1 << 5;
    /// It says whether it is expanded.
    pub const EXPANDABLE: i32 = 1 << 6;
    /// It is expanded.
    pub const EXPANDED: i32 = 1 << 7;
    /// A scroller has more after what shows; a range can go up.
    pub const FORWARD: i32 = 1 << 8;
    /// A scroller has more before what shows; a range can go down.
    pub const BACKWARD: i32 = 1 << 9;
    /// A scroller that scrolls sideways.
    pub const SIDEWAYS: i32 = 1 << 10;
    /// A live region whose change is spoken when the speech is free.
    pub const POLITE: i32 = 1 << 11;
    /// A live region whose change is spoken at once.
    pub const ASSERTIVE: i32 = 1 << 12;
    /// Its range's three numbers mean something.
    pub const RANGE: i32 = 1 << 13;
}

/// What changed of a node since the last statement.
pub mod change {
    /// Its children, or where they are.
    pub const SUBTREE: i32 = 1;
    /// Its name or its value.
    pub const TEXT: i32 = 1 << 1;
    /// Its state.
    pub const STATE: i32 = 1 << 2;
    /// A scroller moved.
    pub const SCROLLED: i32 = 1 << 3;
    /// A live region changed, or an alert appeared: it is spoken.
    pub const LIVE: i32 = 1 << 4;
}

/// What a service asks of a node ([`Presenter::access_act`]).
pub mod act {
    /// Press it.
    pub const CLICK: i32 = 1;
    /// Hold it: its context menu.
    pub const LONG_CLICK: i32 = 2;
    /// Scroll a scroller on, or step a range up.
    pub const FORWARD: i32 = 3;
    /// Scroll a scroller back, or step a range down.
    pub const BACKWARD: i32 = 4;
    /// Set a range to the value given.
    pub const SET: i32 = 5;
}

/// The Up arrow, an alert's headline and its supporting text stand for no
/// view: their identities are these.
const UP: i32 = 0x7fff_0001;
const HEADLINE: i32 = 0x7fff_0002;
const SUPPORTING: i32 = 0x7fff_0003;
/// The page itself, where the document scrolls and no scroller holds it.
const PAGE_NODE: i32 = 0x7fff_0004;
/// Identities under a list's row start here; a view's own is below it.
const ROWED: i32 = 0x4000_0000;
/// More changes than this are told as one, of the whole tree.
const MOST_CHANGES: usize = 12;

/// One node of the tree, in the order a person reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Its identity ([`identity`]): never 0.
    pub id: i32,
    /// The identity of the node it stands under; 0 for the shell's view.
    pub parent: i32,
    /// The kernel's view it stands for; 0 for the Up arrow and an alert's
    /// words.
    pub view: ViewId,
    /// What it is ([`role`]).
    pub role: i32,
    /// Its flags ([`flag`]).
    pub flags: i32,
    /// Where it shows, in points: in the window for the page and the bars,
    /// in its own window for a surface of the top layer.
    pub rect: [f32; 4],
    /// A range's least, greatest and present value; for a scroller, where
    /// it is along its axis and how far it goes.
    pub range: [f32; 3],
    /// Its place among its kind, from 1, and how many they are (a tab, a
    /// list's row); for a `tablist`, 0 and how many tabs. 0 and 0: none.
    pub set: (i32, i32),
    /// Its accessible name.
    pub name: String,
    /// Its value: a range's text, a `select`'s choice, a painted field's
    /// text; for a hosted field, its placeholder.
    pub value: String,
    /// Its description (`aria-description`, `aria-describedby`).
    pub hint: String,
}

/// What the presenter keeps of the tree its shell was told.
#[derive(Default)]
pub(super) struct State {
    /// What the last tree was built from: nothing is built while it stands.
    key: Option<Key>,
    /// The last tree told, which surface it was (0: the page), and its pane.
    nodes: Vec<Node>,
    host: u32,
    pane: (i32, String),
    told: bool,
    /// The page's tree and its route while a surface's is the tree told:
    /// what a live region of the page came to say meanwhile is spoken when
    /// the page is told again.
    page: Option<(Vec<Node>, i32)>,
    /// Counts the statements.
    serial: u32,
}

/// What a tree is built from, cheaply: when none of it moved, the tree
/// stands.
#[derive(Debug, Clone, PartialEq)]
struct Key {
    epoch: u64,
    boxes: u64,
    scroll: Vec<(ViewId, i32, i32)>,
    page: (i32, i32),
    surface: Option<(u32, usize)>,
    bars: u32,
    frame: [i32; 4],
}

fn fnv(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

const FNV: u64 = 0xcbf2_9ce4_8422_2325;

/// A row's hash: its list's identity and its item's key.
fn row_hash(list: i32, key: &str) -> u64 {
    fnv(fnv(fnv(FNV, &list.to_le_bytes()), key.as_bytes()), &[0xff])
}

/// The identity a hash under a row folds to.
fn rowed(hash: u64) -> i32 {
    ROWED + ((hash ^ (hash >> 32)) % 0x3ff0_0000) as i32
}

/// A node's identity, which a platform keeps its place by: its view id,
/// which the runner keeps for as long as the view lives, through every
/// commit and scroll. Under a list's row (a node whose ancestor carries a
/// `listItemKey`) the view is not the item's own: the row's views are
/// given to another item when it scrolls away (LLP 1078), and a row that
/// returns is built from others. So there it is a hash of the list's own
/// identity, the row's item key, and the node's place in the row (its
/// index among its siblings at each level down from the row).
pub fn identity(kernel: &Kernel, view: ViewId) -> i32 {
    let mut path: Vec<u32> = Vec::new();
    let mut at = view;
    while let Some(node) = kernel.node(at) {
        let key = node.props.str(PropId::ListItemKey);
        if let Some(key) = key.filter(|key| !key.is_empty()) {
            let list = node.parent.map_or(0, |list| identity(kernel, list));
            let mut hash = row_hash(list, key);
            for index in path.iter().rev() {
                hash = fnv(hash, &index.to_le_bytes());
            }
            return rowed(hash);
        }
        let Some(parent) = node.parent else { break };
        let siblings = kernel.node(parent).map(|p| p.children());
        let index = siblings.and_then(|s| s.iter().position(|child| *child == at));
        path.push(index.unwrap_or(0) as u32);
        at = parent;
    }
    (view & 0x3fff_ffff) as i32
}

fn intersect(a: Rect4, b: Rect4) -> Option<Rect4> {
    let (x0, y0) = (a.0.max(b.0), a.1.max(b.1));
    let (x1, y1) = ((a.0 + a.2).min(b.0 + b.2), (a.1 + a.3).min(b.1 + b.3));
    (x1 - x0 > 0.5 && y1 - y0 > 0.5).then_some((x0, y0, x1 - x0, y1 - y0))
}

fn number(text: Option<&str>) -> Option<f32> {
    text.and_then(|t| t.trim().parse::<f32>().ok())
        .filter(|n| n.is_finite())
}

/// What the walk carries down the tree.
#[derive(Clone, Copy)]
struct Scope {
    /// The identity of the node above.
    parent: i32,
    /// The identity hash under a list's row.
    hash: Option<u64>,
    /// How far what shows here has moved since it was painted: a scroller
    /// the shell moved by itself is painted a moment later.
    shift: (f32, f32),
    /// What shows of the page here, in the viewport.
    clip: Rect4,
    /// Inside a pressable: words here are its name, not nodes.
    inside: bool,
    /// The nearest `aria-disabled` said above.
    disabled: bool,
}

/// A tree being built.
struct Build {
    out: Vec<Node>,
    used: BTreeSet<i32>,
    /// Painted boxes by view: the box, its clip, and a scroller's offset as
    /// painted.
    boxes: BTreeMap<ViewId, PaintedBox>,
    /// The page's place in the window.
    origin: (f32, f32),
    /// A row's place, waiting for the first node emitted under it.
    item: (i32, i32),
    /// How far each list scrolls, read once.
    limits: BTreeMap<ViewId, f32>,
}

impl Build {
    /// An identity no node of this tree has yet: `id`, or the next free.
    fn unique(&mut self, mut id: i32) -> i32 {
        while id <= 0 || !self.used.insert(id) {
            id = if id >= 0x7ffe_ffff { ROWED } else { id + 1 };
        }
        id
    }
}

impl<D: DataSource> Presenter<D> {
    /// `aria-labelledby`'s words, else `aria-label`, unless empty.
    fn access_label(&self, node: &NodeRef<'_>) -> Option<String> {
        let kernel = self.host.kernel();
        let named = node.props.str(PropId::AccessibilityLabelledBy).map(|refs| {
            (refs.split_whitespace())
                .filter_map(|name| kernel.find_by_id(name).into_iter().next())
                .filter_map(|key| kernel.node_by_key(key))
                .map(|n| {
                    let own = n.props.str(PropId::AccessibilityLabel);
                    (own.filter(|l| !l.is_empty()).map(str::to_string))
                        .unwrap_or_else(|| self.access_words(&n, true))
                })
                .filter(|words| !words.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        });
        named.filter(|words| !words.is_empty()).or_else(|| {
            let label = node.props.str(PropId::AccessibilityLabel);
            label.filter(|l| !l.is_empty()).map(str::to_string)
        })
    }

    /// A node's description: the words of what `aria-describedby` names,
    /// else its `aria-description`.
    fn access_hint(&self, node: &NodeRef<'_>) -> String {
        let kernel = self.host.kernel();
        let described = node
            .props
            .str(PropId::AccessibilityDescribedBy)
            .map(|refs| {
                (refs.split_whitespace())
                    .filter_map(|name| kernel.find_by_id(name).into_iter().next())
                    .filter_map(|key| kernel.node_by_key(key))
                    .map(|n| self.access_name(&n))
                    .filter(|words| !words.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
            });
        (described.filter(|words| !words.is_empty()))
            .or_else(|| (node.props.str(PropId::AccessibilityHint)).map(str::to_string))
            .unwrap_or_default()
    }

    /// Whether a press on `node` is its own: a button, a link, a node with
    /// a `press` handler.
    fn access_pressable(&self, node: &NodeRef<'_>) -> bool {
        node.node_type == NodeType::Pressable
            || ControlKind::of(node.node_type, node.props) == Some(ControlKind::Button)
            || node.props.str(PropId::Href).is_some_and(|h| !h.is_empty())
            || (self.host.runner().handlers_of(node.id)).contains(&EventKind::Press)
    }

    /// A node's words: a text's own, an image's label, and for anything
    /// else its children's, a space between them, less what is hidden from
    /// assistive technology and, unless `all`, less the controls inside it,
    /// which are named apart.
    fn access_words(&self, node: &NodeRef<'_>, all: bool) -> String {
        let kernel = self.host.kernel();
        match node.node_type {
            NodeType::Text => return kernel.bar_text(node.id).unwrap_or_default(),
            NodeType::Image | NodeType::Svg => {
                let label = node.props.str(PropId::AccessibilityLabel);
                return label.unwrap_or_default().to_string();
            }
            NodeType::TextInput | NodeType::Control
                if ControlKind::of(node.node_type, node.props) != Some(ControlKind::Button) =>
            {
                return String::new()
            }
            _ => {}
        }
        (node.children().into_iter())
            .filter_map(|id| kernel.node(id))
            .filter(|n| {
                n.props.bool(PropId::AccessibilityElementsHidden) != Some(true)
                    && n.style.display != Display::None
                    && (all || !self.access_pressable(n))
            })
            .map(|n| match self.access_label(&n) {
                Some(label) => label,
                None => self.access_words(&n, all),
            })
            .filter(|words| !words.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// A node's accessible name: its label, else a button's face, else its
    /// words.
    fn access_name(&self, node: &NodeRef<'_>) -> String {
        if let Some(label) = self.access_label(node) {
            return label;
        }
        let face = self.host.kernel().press_face(node.id);
        if let Some(title) = face.and_then(|face| face.title.or(face.label)) {
            return title;
        }
        self.access_words(node, false)
    }

    /// How a live region speaks: its `aria-live`, else its role's
    /// (`status` polite, `alert` assertive).
    fn access_live(node: &NodeRef<'_>) -> i32 {
        let role = node.props.str(PropId::AccessibilityRole);
        let live = node.props.str(PropId::AccessibilityLive).or(match role {
            Some("status") => Some("polite"),
            Some("alert") => Some("assertive"),
            _ => None,
        });
        match live {
            Some("polite") => flag::POLITE,
            Some("assertive") => flag::ASSERTIVE,
            _ => 0,
        }
    }

    /// Add `node` to the tree where its box shows; its identity, or `None`
    /// where nothing of it shows (what is under it is then left out too).
    #[allow(clippy::too_many_arguments)]
    fn access_emit(
        &self,
        b: &mut Build,
        scope: &Scope,
        node: &NodeRef<'_>,
        role: i32,
        mut flags: i32,
        range: [f32; 3],
        (name, value): (String, String),
    ) -> Option<i32> {
        let kernel = self.host.kernel();
        if !self.display.allows(kernel, node.id) {
            return None;
        }
        let painted = *b.boxes.get(&node.id)?;
        let rect = (
            painted.rect.0 + scope.shift.0,
            painted.rect.1 + scope.shift.1,
            painted.rect.2,
            painted.rect.3,
        );
        let shows = intersect(rect, scope.clip)?;
        let id = b.unique(match scope.hash {
            Some(hash) => rowed(hash),
            None => (node.id & 0x3fff_ffff) as i32,
        });
        if node.props.bool(PropId::Disabled) == Some(true) {
            flags = (flags | flag::DISABLED) & !(flag::CLICK | flag::LONG_CLICK);
        } else if scope.disabled {
            flags |= flag::DISABLED;
        }
        flags |= Self::access_live(node);
        b.out.push(Node {
            id,
            parent: scope.parent,
            view: node.id,
            role,
            flags,
            rect: [shows.0 + b.origin.0, shows.1 + b.origin.1, shows.2, shows.3],
            range,
            set: std::mem::take(&mut b.item),
            name,
            value,
            hint: self.access_hint(node),
        });
        Some(id)
    }

    /// A control's node: its platform kind and its state.
    fn access_control(
        &self,
        node: &NodeRef<'_>,
        kind: ControlKind,
    ) -> (i32, i32, [f32; 3], String) {
        let checked = (node.props.bool(PropId::Checked))
            .or_else(|| self.controls.get(&node.id).copied())
            .unwrap_or(false);
        let check = flag::CLICK | flag::CHECKABLE | if checked { flag::CHECKED } else { 0 };
        let chosen = self.chosen.get(&node.id);
        let choice = crate::paint::control::choice(node, chosen);
        let value = || {
            choice
                .or(node.props.str(PropId::Value))
                .unwrap_or_default()
                .to_string()
        };
        match kind {
            ControlKind::Checkbox => (role::CHECKBOX, check, [0.0; 3], String::new()),
            ControlKind::Switch => (role::SWITCH, check, [0.0; 3], String::new()),
            ControlKind::Radio => (role::RADIO, check, [0.0; 3], String::new()),
            ControlKind::Range => {
                let range = exact_kernel::Range::of(node.props);
                let now = (choice.and_then(|c| number(Some(c))).map(f64::from))
                    .map_or_else(|| range.shown(node.props), |v| range.sanitize(v));
                let mut flags = flag::RANGE;
                if now < range.max {
                    flags |= flag::FORWARD;
                }
                if now > range.min {
                    flags |= flag::BACKWARD;
                }
                let text = node.props.str(PropId::AccessibilityValueText);
                (
                    role::SLIDER,
                    flags,
                    [range.min as f32, range.max as f32, now as f32],
                    text.unwrap_or_default().to_string(),
                )
            }
            ControlKind::ProgressBar => {
                let progress = exact_kernel::Progress::of(node.props);
                let range = [0.0, progress.max as f32, progress.value as f32];
                (role::PROGRESS, flag::RANGE, range, String::new())
            }
            ControlKind::Progress => (role::PROGRESS, 0, [0.0; 3], String::new()),
            ControlKind::Select => {
                let shown = self.host.kernel().select_chosen(node.id);
                let label = shown.map(|c| c.label).unwrap_or_else(value);
                (role::DROPDOWN, flag::CLICK, [0.0; 3], label)
            }
            _ => (role::BUTTON, flag::CLICK, [0.0; 3], value()),
        }
    }

    /// A pressable's role and state, by its ARIA role.
    fn access_pressed(&self, node: &NodeRef<'_>) -> (i32, i32) {
        let aria = node.props.str(PropId::AccessibilityRole);
        let checked = node.props.bool(PropId::AccessibilityChecked) == Some(true);
        let check = flag::CHECKABLE | if checked { flag::CHECKED } else { 0 };
        let pressed = node.props.str(PropId::AccessibilityPressed);
        let (role, mut flags) = match aria {
            Some("checkbox" | "menuitemcheckbox") => (role::CHECKBOX, check),
            Some("switch") => (role::SWITCH, check),
            Some("radio" | "menuitemradio") => (role::RADIO, check),
            Some("tab") => (role::TAB, 0),
            Some("link") => (role::LINK, 0),
            None | Some("button") if matches!(pressed, Some("true" | "false" | "mixed")) => {
                let on = if pressed == Some("true") {
                    flag::CHECKED
                } else {
                    0
                };
                (role::TOGGLE, flag::CHECKABLE | on)
            }
            _ if node.props.str(PropId::Href).is_some_and(|h| !h.is_empty()) => (role::LINK, 0),
            _ => (role::BUTTON, 0),
        };
        flags |= flag::CLICK;
        if node.props.bool(PropId::AccessibilitySelected) == Some(true) {
            flags |= flag::SELECTED;
        }
        if let Some(expanded) = node.props.bool(PropId::AccessibilityExpanded) {
            flags |= flag::EXPANDABLE | if expanded { flag::EXPANDED } else { 0 };
        }
        let menu = node.props.str(PropId::ContextPopover);
        if menu.is_some_and(|name| !name.is_empty())
            || (self.host.runner().handlers_of(node.id)).contains(&EventKind::Contextmenu)
        {
            flags |= flag::LONG_CLICK;
        }
        (role, flags)
    }

    /// A scroller's flags and where it is along the axis it scrolls on.
    fn access_scroll(s: &Scroller) -> (i32, [f32; 3]) {
        let sideways = s.axes.0 && !s.axes.1;
        let (at, max) = match sideways {
            true => (s.offset.0, s.max.0),
            false => (s.offset.1, s.max.1),
        };
        let mut flags = if sideways { flag::SIDEWAYS } else { 0 };
        if at < max - 0.5 {
            flags |= flag::FORWARD;
        }
        if at > 0.5 {
            flags |= flag::BACKWARD;
        }
        (flags, [at, max, 0.0])
    }

    /// Walk `id` and what is under it into the tree.
    fn access_walk(&self, b: &mut Build, id: ViewId, mut scope: Scope) {
        let kernel = self.host.kernel();
        let Some(node) = kernel.node(id) else { return };
        let (hidden, inert) = self.host.route_visibility(id);
        if hidden
            || inert
            || node.style.display == Display::None
            || node.props.bool(PropId::AccessibilityElementsHidden) == Some(true)
        {
            return;
        }
        if let Some(said) = node.props.bool(PropId::AccessibilityDisabled) {
            scope.disabled = said;
        }
        let position = |prop| node.props.get(prop).and_then(|v| v.as_int());
        if let Some(at) = position(PropId::AccessibilityPosInSet) {
            let size = position(PropId::AccessibilitySetSize).unwrap_or(0);
            b.item = (at as i32, size as i32);
        }
        let aria = node.props.str(PropId::AccessibilityRole);
        let control = ControlKind::of(node.node_type, node.props);
        // What is one node, with nothing under it.
        let leaf = match (node.node_type, control) {
            (NodeType::TextInput, _) => {
                let hosted = self.hosts_field(id);
                let role = if hosted { role::FIELD } else { role::EDIT };
                let name = self.access_label(&node).unwrap_or_default();
                let value = match hosted {
                    true => node
                        .props
                        .str(PropId::Placeholder)
                        .unwrap_or_default()
                        .into(),
                    false => self.field_text(id),
                };
                Some((role, flag::CLICK, [0.0; 3], (name, value)))
            }
            (_, Some(kind)) if kind != ControlKind::Button => {
                let (role, flags, range, value) = self.access_control(&node, kind);
                let name = self.access_label(&node).unwrap_or_default();
                Some((role, flags, range, (name, value)))
            }
            _ => None,
        };
        if let Some((role, flags, range, words)) = leaf {
            self.access_emit(b, &scope, &node, role, flags, range, words);
            return;
        }
        if self.access_pressable(&node) {
            let (role, flags) = self.access_pressed(&node);
            let name = (self.access_name(&node), String::new());
            let Some(own) = self.access_emit(b, &scope, &node, role, flags, [0.0; 3], name) else {
                return;
            };
            scope.parent = own;
            scope.inside = true;
        } else if matches!(
            node.node_type,
            NodeType::Text | NodeType::Image | NodeType::Svg
        ) {
            if scope.inside {
                return;
            }
            let label = self.access_label(&node);
            let text = node.node_type == NodeType::Text;
            let name = label.unwrap_or_else(|| match text {
                true => kernel.bar_text(id).unwrap_or_default(),
                false => String::new(),
            });
            let heading = aria == Some("heading")
                || node.props.get(PropId::AccessibilityHeadingLevel).is_some();
            let role = match (text, heading) {
                (true, true) => role::HEADING,
                (true, false) => role::TEXT,
                _ => role::IMAGE,
            };
            if !name.trim().is_empty() {
                self.access_emit(b, &scope, &node, role, 0, [0.0; 3], (name, String::new()));
            }
            return;
        } else if let Some(now) =
            number(node.props.str(PropId::AccessibilityValueNow)).filter(|_| {
                matches!(
                    aria,
                    Some("progressbar" | "slider" | "meter" | "scrollbar" | "spinbutton")
                )
            })
        {
            // A range role is one element, its children presentational.
            let min = number(node.props.str(PropId::AccessibilityValueMin)).unwrap_or(0.0);
            let max = number(node.props.str(PropId::AccessibilityValueMax)).unwrap_or(100.0);
            let role = if aria == Some("slider") {
                role::SLIDER
            } else {
                role::PROGRESS
            };
            let text = node
                .props
                .str(PropId::AccessibilityValueText)
                .unwrap_or_default();
            let words = (
                self.access_label(&node).unwrap_or_default(),
                text.to_string(),
            );
            self.access_emit(b, &scope, &node, role, flag::RANGE, [min, max, now], words);
            return;
        } else {
            // A swipe row is written as a scroller, and is the presenter's
            // own gesture: its actions are not a place to scroll to.
            let may = !scope.inside
                && node.props.str(PropId::SwipeContent).is_none()
                && (node.node_type.scrolls_by_default()
                    || matches!(node.style.overflow_x, Overflow::Scroll | Overflow::Auto)
                    || matches!(node.style.overflow_y, Overflow::Scroll | Overflow::Auto));
            let scroller = (may.then(|| self.scroller(id, &b.limits)).flatten())
                .filter(|s| s.axes.0 || s.axes.1);
            let label = self.access_label(&node);
            if let Some(s) = scroller {
                let (flags, range) = Self::access_scroll(&s);
                let words = (label.unwrap_or_default(), String::new());
                let painted = b.boxes.get(&id).copied();
                let Some(own) =
                    self.access_emit(b, &scope, &node, role::SCROLLER, flags, range, words)
                else {
                    return;
                };
                if let Some(painted) = painted {
                    // What is in it was painted at the offset it had then.
                    let was = painted.scroll.unwrap_or(s.offset);
                    let port = (
                        painted.rect.0 + scope.shift.0,
                        painted.rect.1 + scope.shift.1,
                        painted.rect.2,
                        painted.rect.3,
                    );
                    scope.clip = intersect(port, scope.clip).unwrap_or(scope.clip);
                    scope.shift.0 += was.0 - s.offset.0;
                    scope.shift.1 += was.1 - s.offset.1;
                }
                scope.parent = own;
            } else if aria == Some("tablist")
                || Self::access_live(&node) != 0
                || (label.is_some() && matches!(aria, Some("group" | "radiogroup" | "region")))
            {
                let role = if aria == Some("tablist") {
                    role::TABLIST
                } else {
                    role::GROUP
                };
                let words = (label.unwrap_or_default(), String::new());
                if let Some(own) = self.access_emit(b, &scope, &node, role, 0, [0.0; 3], words) {
                    scope.parent = own;
                }
            }
        }
        let own = match scope.hash {
            Some(hash) => rowed(hash),
            None => (id & 0x3fff_ffff) as i32,
        };
        for (index, child) in node.children().into_iter().enumerate() {
            let key = kernel
                .node(child)
                .and_then(|n| n.props.str(PropId::ListItemKey));
            let mut under = scope;
            under.hash = match key.filter(|key| !key.is_empty()) {
                Some(key) => Some(row_hash(own, key)),
                None => scope
                    .hash
                    .map(|hash| fnv(hash, &(index as u32).to_le_bytes())),
            };
            self.access_walk(b, child, under);
        }
    }

    /// A node a bar stands for, at `rect` in the bar at `frame`.
    #[allow(clippy::too_many_arguments)]
    fn access_bar_node(
        &self,
        b: &mut Build,
        view: ViewId,
        parent: i32,
        role: i32,
        flags: i32,
        name: String,
        (rect, frame): (Rect4, [f32; 4]),
    ) -> i32 {
        let kernel = self.host.kernel();
        let node = kernel.node(view);
        let said = node.as_ref().map(|n| {
            let mut at = Some(n.id);
            let mut disabled = None;
            while let (None, Some(id)) = (disabled, at) {
                let node = kernel.node(id);
                disabled = node
                    .as_ref()
                    .and_then(|n| n.props.bool(PropId::AccessibilityDisabled));
                at = node.and_then(|n| n.parent);
            }
            disabled.unwrap_or(false)
        });
        let id = b.unique(if view == 0 {
            UP
        } else {
            identity(kernel, view)
        });
        b.out.push(Node {
            id,
            parent,
            view,
            role,
            flags: flags
                | if said == Some(true) {
                    flag::DISABLED
                } else {
                    0
                },
            rect: [frame[0] + rect.0, frame[1] + rect.1, rect.2, rect.3],
            range: [0.0; 3],
            set: (0, 0),
            name,
            value: String::new(),
            hint: node.map(|n| self.access_hint(&n)).unwrap_or_default(),
        });
        id
    }

    /// The top app bar's nodes: the Up arrow, the leading buttons, the
    /// title as a heading, the trailing buttons. And the pane: the route
    /// and its title.
    fn access_top(&mut self, b: &mut Build) -> (i32, String) {
        let (top, _) = self.bar_plans();
        let Some(plan) = top.cloned() else {
            return (0, String::new());
        };
        let (bar, _) = self.bars();
        let Some(bar) = bar else {
            return (0, String::new());
        };
        let (frame, targets, texts) = (bar.frame, bar.targets.to_vec(), bar.texts);
        let target = |what: bars::What| targets.iter().find(|t| t.what == what).copied();
        let enabled = |on: bool| if on { flag::CLICK } else { flag::DISABLED };
        if let Some(up) = target(bars::What::Up).filter(|_| plan.up) {
            let place = (up.rect, frame);
            self.access_bar_node(b, 0, 0, role::UP, flag::CLICK, String::new(), place);
        }
        let button = |this: &Self, b: &mut Build, item: &bars::Button| {
            if let Some(t) = target(bars::What::Item(item.node)) {
                let name = (this.host.kernel().node(item.node))
                    .map(|n| this.access_name(&n))
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| item.title.clone());
                let place = (t.rect, frame);
                let flags = enabled(item.enabled);
                this.access_bar_node(b, item.node, 0, role::BUTTON, flags, name, place);
            }
        };
        for item in &plan.leading {
            button(self, b, item);
        }
        if let Some((_, rect)) = texts.iter().find(|(node, _)| *node == plan.heading) {
            let place = (*rect, frame);
            let title = plan.title.clone();
            self.access_bar_node(b, plan.heading, 0, role::HEADING, 0, title, place);
        }
        for item in &plan.trailing {
            button(self, b, item);
        }
        (identity(self.host.kernel(), plan.route), plan.title)
    }

    /// The navigation bar's nodes: the bar, and its destinations in it.
    fn access_bottom(&mut self, b: &mut Build) {
        let (_, bottom) = self.bar_plans();
        let Some(plan) = bottom.cloned() else { return };
        let (_, bar) = self.bars();
        let Some(bar) = bar else { return };
        let (frame, targets) = (bar.frame, bar.targets.to_vec());
        let whole = ((0.0, 0.0, frame[2], frame[3]), frame);
        let label = (self.host.kernel().node(plan.tablist))
            .and_then(|n| self.access_label(&n))
            .unwrap_or_default();
        let list = self.access_bar_node(b, plan.tablist, 0, role::TABLIST, 0, label, whole);
        for tab in &plan.tabs {
            let Some(t) = targets.iter().find(|t| t.what == bars::What::Tab(tab.node)) else {
                continue;
            };
            let name = (self.host.kernel().node(tab.node))
                .and_then(|n| self.access_label(&n))
                .unwrap_or_else(|| tab.label.clone());
            let mut flags = if tab.enabled {
                flag::CLICK
            } else {
                flag::DISABLED
            };
            if tab.selected {
                flags |= flag::SELECTED;
            }
            self.access_bar_node(b, tab.node, list, role::TAB, flags, name, (t.rect, frame));
        }
    }

    /// The open surface's nodes, in its own window's points: an alert's
    /// headline, its supporting text and its buttons; a menu's items.
    fn access_surface(&self, b: &mut Build) -> u32 {
        let Some(shown) = self.surface() else {
            return 0;
        };
        let whole = [0.0, 0.0, shown.size.0, shown.size.1];
        if let Plan::Alert { title, message, .. } = shown.plan {
            let words = [
                (HEADLINE, role::HEADING, title),
                (SUPPORTING, role::TEXT, message),
            ];
            let mut texts = shown.texts.iter().filter(|(node, _)| *node == shown.node);
            for (id, role, text) in words {
                let (Some(text), Some((_, rect))) = (text, texts.next()) else {
                    continue;
                };
                let id = b.unique(id);
                b.out.push(Node {
                    id,
                    parent: 0,
                    view: 0,
                    role,
                    flags: 0,
                    rect: [rect.0, rect.1, rect.2, rect.3],
                    range: [0.0; 3],
                    set: (0, 0),
                    name: text.clone(),
                    value: String::new(),
                    hint: String::new(),
                });
            }
        }
        for (row, target) in shown.plan.rows().zip(shown.targets) {
            let mut flags = if row.enabled {
                flag::CLICK
            } else {
                flag::DISABLED
            };
            if row.checked {
                flags |= flag::CHECKABLE | flag::CHECKED;
            }
            let place = (target.rect, whole);
            self.access_bar_node(
                b,
                row.node,
                0,
                role::BUTTON,
                flags,
                row.title.clone(),
                place,
            );
        }
        shown.serial
    }

    /// Each `tablist`'s tabs are counted and placed among themselves.
    fn access_sets(nodes: &mut [Node]) {
        let lists: Vec<i32> = (nodes.iter())
            .filter(|n| n.role == role::TABLIST)
            .map(|n| n.id)
            .collect();
        for list in lists {
            let count = (nodes.iter())
                .filter(|n| n.parent == list && n.role == role::TAB)
                .count() as i32;
            let mut at = 0;
            for node in nodes.iter_mut() {
                if node.id == list {
                    node.set = (0, count);
                } else if node.parent == list && node.role == role::TAB {
                    at += 1;
                    node.set = (at, count);
                }
            }
        }
    }

    /// What the tree is built from, read cheaply.
    fn access_key(&mut self) -> Key {
        let ten = |v: f32| (v * 10.0).round() as i32;
        let surface = self
            .surface()
            .map(|s| (s.serial, s.pressed.map_or(0, |p| p + 1)));
        let bars = self.bars_revision();
        Key {
            epoch: self.host.kernel().epoch(),
            boxes: self.boxes_serial,
            scroll: (self.scroll.iter())
                .map(|(id, at)| (*id, ten(at.0), ten(at.1)))
                .collect(),
            page: (ten(self.page.0), ten(self.page.1)),
            surface,
            bars,
            frame: self.chrome().frame.map(ten),
        }
    }

    /// The tree as it now stands: its nodes in reading order, which
    /// surface's window they are in (0: the page's), and the pane (the
    /// route that shows and its title).
    pub fn access_tree(&mut self) -> (Vec<Node>, u32, (i32, String)) {
        self.boxes();
        let frame = self.chrome().frame;
        let mut b = Build {
            out: Vec::new(),
            used: BTreeSet::new(),
            boxes: self.boxes.iter().map(|b| (b.id, *b)).collect(),
            origin: (frame[0], frame[1]),
            item: (0, 0),
            limits: self.collection_scroll_limits(),
        };
        if self.top_open() {
            let serial = self.access_surface(&mut b);
            return (b.out, serial, (0, String::new()));
        }
        let pane = self.access_top(&mut b);
        let page = self.page_scroller();
        let viewport = (0.0, 0.0, page.port[2], page.port[3]);
        let mut scope = Scope {
            parent: 0,
            hash: None,
            shift: (0.0, 0.0),
            clip: viewport,
            inside: false,
            disabled: false,
        };
        if page.axes.0 || page.axes.1 {
            // The page itself scrolls: it is the scroller everything is in.
            let (flags, range) = Self::access_scroll(&page);
            let id = b.unique(PAGE_NODE);
            b.out.push(Node {
                id,
                parent: 0,
                view: 0,
                role: role::SCROLLER,
                flags,
                rect: [frame[0], frame[1], page.port[2], page.port[3]],
                range,
                set: (0, 0),
                name: String::new(),
                value: String::new(),
                hint: String::new(),
            });
            scope.parent = id;
        }
        for root in self.host.runner().roots() {
            self.access_walk(&mut b, root, scope);
        }
        self.access_bottom(&mut b);
        Self::access_sets(&mut b.out);
        (b.out, 0, pane)
    }

    /// What changed between the tree last told and `now`, least first: for
    /// each changed node its identity and what of it changed
    /// ([`change`]), and for a scroller that moved, how far. A node whose
    /// children came, went or moved is one change, of its subtree; what is
    /// under such a node is not told apart, but for a live region, whose
    /// change is spoken, and a node that became selected or checked or
    /// stopped being so, whose state a service says.
    fn access_changes(before: &[Node], now: &[Node], new_pane: bool) -> Vec<(i32, i32, f32, f32)> {
        let old: BTreeMap<i32, &Node> = before.iter().map(|n| (n.id, n)).collect();
        let new: BTreeMap<i32, &Node> = now.iter().map(|n| (n.id, n)).collect();
        let children = |nodes: &[Node]| {
            let mut under: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
            for node in nodes {
                under.entry(node.parent).or_default().push(node.id);
            }
            under
        };
        let (were, are) = (children(before), children(now));
        let new = &new;
        let mut changed: BTreeMap<i32, (i32, f32, f32)> = BTreeMap::new();
        fn mark(changed: &mut BTreeMap<i32, (i32, f32, f32)>, id: i32, what: i32) {
            changed.entry(id).or_insert((0, 0.0, 0.0)).0 |= what;
        }
        for parent in were.keys().chain(are.keys()) {
            if were.get(parent) != are.get(parent) && (*parent == 0 || new.contains_key(parent)) {
                mark(&mut changed, *parent, change::SUBTREE);
            }
        }
        let live = |node: &Node| node.flags & (flag::POLITE | flag::ASSERTIVE) != 0;
        let mut spoken = Vec::new();
        // A node that became selected or checked, or stopped being so, is
        // told apart wherever it is: a service says so of the node it is on.
        let mut stated = Vec::new();
        for node in now {
            let Some(was) = old.get(&node.id) else {
                // An alert that appears is spoken, as a browser's is; a
                // polite region too, unless it came with a new screen.
                let speaks = node.flags & flag::ASSERTIVE != 0 || (live(node) && !new_pane);
                if speaks && !before.is_empty() {
                    spoken.push(node.id);
                }
                continue;
            };
            let mut what = 0;
            if was.name != node.name || was.value != node.value || was.hint != node.hint {
                what |= change::TEXT;
            }
            if was.flags != node.flags || was.set != node.set || was.role != node.role {
                what |= change::STATE;
            }
            if was.range != node.range {
                what |= match node.role {
                    role::SCROLLER => change::SCROLLED | change::SUBTREE,
                    _ => change::STATE,
                };
            }
            if what != 0 {
                mark(&mut changed, node.id, what);
            }
            if (was.flags ^ node.flags) & (flag::SELECTED | flag::CHECKED) != 0 {
                stated.push(node.id);
            }
            if what & change::SCROLLED != 0 {
                let by = node.range[0] - was.range[0];
                let sideways = node.flags & flag::SIDEWAYS != 0;
                let entry = changed.entry(node.id).or_insert((0, 0.0, 0.0));
                (entry.1, entry.2) = if sideways { (by, 0.0) } else { (0.0, by) };
            }
            if was.rect != node.rect {
                // Moved or sized: the node above shows its children anew.
                mark(&mut changed, node.parent, change::SUBTREE);
            }
        }
        // A change under a live region is that region's to speak.
        let above = |mut id: i32| {
            std::iter::from_fn(move || {
                id = new.get(&id).map(|n| n.parent)?;
                (id != 0).then_some(id)
            })
        };
        let region = |id: i32| {
            std::iter::once(id)
                .chain(above(id))
                .find(|id| new.get(id).is_some_and(|n| live(n)))
        };
        for (id, (what, ..)) in &changed {
            if what & (change::TEXT | change::SUBTREE) != 0 {
                spoken.extend(region(*id));
            }
        }
        for id in now
            .iter()
            .filter(|n| !old.contains_key(&n.id))
            .map(|n| n.id)
        {
            spoken.extend(above(id).find(|id| new.get(id).is_some_and(|n| live(n))));
        }
        // What is under a node whose subtree changed is not told apart.
        let covered = |id: i32| {
            above(id)
                .chain(std::iter::once(0))
                .any(|a| changed.get(&a).is_some_and(|c| c.0 & change::SUBTREE != 0))
        };
        let mut out: Vec<(i32, i32, f32, f32)> = (changed.iter())
            .filter(|(id, _)| **id == 0 || !covered(**id))
            .map(|(id, (what, dx, dy))| (*id, *what, *dx, *dy))
            .collect();
        let stated = stated.into_iter().filter(|id| covered(*id));
        out.extend(stated.map(|id| (id, change::STATE, 0.0, 0.0)));
        spoken.sort_unstable();
        spoken.dedup();
        out.extend(spoken.into_iter().map(|id| (id, change::LIVE, 0.0, 0.0)));
        out
    }

    /// What the shell is told of the accessibility tree, in its pixels
    /// (`scale` to a point), or `None` when that is what it was last told.
    /// `again`: the shell holds nothing (it was made again, or a service
    /// was just turned on) and is told the tree whole. The words:
    ///
    /// - the statement's number; the surface whose window the nodes are in
    ///   (its serial; 0: the page's window); 1 when the tree is told whole,
    ///   and the only changes listed are live regions that speak;
    /// - the pane: the identity of the route that shows (0: none) and its
    ///   title, a string (its bytes' count, then UTF-8 padded to whole
    ///   words), as the top app bar shows it;
    /// - a count of changes, then four words each: a node's identity (0:
    ///   the shell's view), what changed ([`change`]), and as float bits
    ///   how far a scroller moved (x, y);
    /// - a count of nodes, then for each, in reading order: its identity,
    ///   its parent's (0: the shell's view), its view, its role
    ///   ([`role`]), its flags ([`flag`]), its box (x, y, width, height),
    ///   as float bits its range's least, greatest and present value (a
    ///   scroller's offset along its axis and its farthest, in points),
    ///   its place in its set and the set's size, and three strings: its
    ///   name, its value and its description.
    pub fn reader_access(&mut self, scale: f32, again: bool) -> Option<Vec<i32>> {
        let key = self.access_key();
        if !again && self.access.told && self.access.key.as_ref() == Some(&key) {
            return None;
        }
        let (nodes, host, pane) = self.access_tree();
        self.access.key = Some(key);
        let same = self.access.told && host == self.access.host;
        if !again && same && nodes == self.access.nodes && pane == self.access.pane {
            return None;
        }
        let state = &mut self.access;
        let spoken = |c: Vec<(i32, i32, f32, f32)>| -> Vec<(i32, i32, f32, f32)> {
            c.into_iter().filter(|c| c.1 == change::LIVE).collect()
        };
        // Told whole: to a shell that holds nothing, when the tree is
        // another window's, and when too much changed to tell apart. What a
        // live region came to say is spoken even so, but to a shell that
        // holds nothing.
        let (changes, whole) = match (again, same, &state.page) {
            (true, ..) => (Vec::new(), true),
            (_, true, _) => {
                let new_pane = pane.0 != state.pane.0;
                let all = Self::access_changes(&state.nodes, &nodes, new_pane);
                match all.len() <= MOST_CHANGES {
                    true => (all, false),
                    false => (spoken(all), true),
                }
            }
            // The page again, after a surface closed.
            (_, _, Some((page, route))) if host == 0 => {
                let all = Self::access_changes(page, &nodes, pane.0 != *route);
                (spoken(all), true)
            }
            _ => (Vec::new(), true),
        };
        if again || host == 0 {
            state.page = None;
        } else if state.told && state.host == 0 {
            state.page = Some((std::mem::take(&mut state.nodes), state.pane.0));
        }
        state.serial += 1;
        let px = |v: f32| (v * scale).round() as i32;
        let mut out = vec![state.serial as i32, host as i32, i32::from(whole)];
        out.push(pane.0);
        string(&mut out, &pane.1);
        out.push(changes.len() as i32);
        for (id, what, dx, dy) in changes {
            let moved = [(dx * scale).to_bits() as i32, (dy * scale).to_bits() as i32];
            out.extend([id, what, moved[0], moved[1]]);
        }
        out.push(nodes.len() as i32);
        for node in &nodes {
            out.extend([
                node.id,
                node.parent,
                node.view as i32,
                node.role,
                node.flags,
            ]);
            out.extend(node.rect.map(px));
            out.extend(node.range.map(|v| v.to_bits() as i32));
            out.extend([node.set.0, node.set.1]);
            for text in [&node.name, &node.value, &node.hint] {
                string(&mut out, text);
            }
        }
        (state.nodes, state.host, state.pane, state.told) = (nodes, host, pane, true);
        Some(out)
    }

    /// The shell stopped asking (no service is on): what it was told is
    /// forgotten, and nothing is kept for it.
    pub fn access_reset(&mut self) {
        self.access = State::default();
    }

    /// The tree as the shell was last told it.
    pub fn access_told(&self) -> &[Node] {
        &self.access.nodes
    }

    /// A service acts on node `id` of the tree last told ([`act`]):
    /// whether anything took it. A press is the presenter's own, at the
    /// node's middle, as the agent's `tap` is; a held press opens the
    /// node's context menu, as a finger held on it does; a scroller scrolls
    /// by what it shows, less a little kept in view; a range steps, or is
    /// set to `value`.
    pub fn access_act(&mut self, id: i32, action: i32, value: f32, now_ms: f64) -> bool {
        let Some(node) = self.access.nodes.iter().find(|n| n.id == id).cloned() else {
            return false;
        };
        let live = |p: &Self| node.view != 0 && p.host.kernel().node(node.view).is_some();
        match (action, node.role) {
            (act::CLICK, role::UP) => self.system_back(now_ms),
            (act::CLICK, _) if node.flags & flag::CLICK != 0 && live(self) => {
                let done = self.tap(node.view);
                if let Err(e) = &done {
                    self.host.log(format!("accessibility: press: {e}"));
                }
                done.is_ok()
            }
            (act::LONG_CLICK, _) if node.flags & flag::LONG_CLICK != 0 && live(self) => {
                let Some(painted) = self.box_of(node.view) else {
                    return false;
                };
                let (x, y) = painted.center();
                self.long_press(x, y, now_ms)
            }
            (act::FORWARD | act::BACKWARD, role::SCROLLER) => {
                let view = if id == PAGE_NODE { PAGE } else { node.view };
                let limits = self.collection_scroll_limits();
                let Some(s) = self.scroller(view, &limits) else {
                    return false;
                };
                let sideways = node.flags & flag::SIDEWAYS != 0;
                let shows = if sideways { node.rect[2] } else { node.rect[3] };
                // A line of what showed stays in view, as the platform's
                // own lists leave one.
                let step = (shows - 48.0).max(shows / 2.0).max(1.0);
                let by = if action == act::FORWARD { step } else { -step };
                let to = match sideways {
                    true => (s.offset.0 + by, s.offset.1),
                    false => (s.offset.0, s.offset.1 + by),
                };
                self.scroll_to(view, to).is_some_and(|at| at != s.offset)
            }
            (act::FORWARD | act::BACKWARD | act::SET, role::SLIDER) if live(self) => {
                let range = (self.host.kernel().node(node.view))
                    .filter(|n| n.node_type == NodeType::Control)
                    .map(|n| exact_kernel::Range::of(n.props));
                let Some(range) = range else { return false };
                // A range with no step moves by a twentieth of its span.
                let step = range.step.unwrap_or((range.max - range.min) / 20.0);
                let now = f64::from(node.range[2]);
                let to = range.sanitize(match action {
                    act::FORWARD => now + step,
                    act::BACKWARD => now - step,
                    _ => f64::from(value),
                });
                to != now && self.set_control_value(node.view, &to.to_string()).is_ok()
            }
            _ => false,
        }
    }
}

#[cfg(test)]
#[path = "access_tests.rs"]
mod tests;
