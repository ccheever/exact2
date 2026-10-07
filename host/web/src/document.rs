//! The safe document projection: the page the live host builds, as HTML,
//! before browser layout.
//!
//! @ref LLP 1048 D1 (the document is the page) / LLP 1048.000 D1
//!
//! One walk over the kernel tree from the runner's roots. Each element's tag,
//! attributes and inline style come from the live host's own rules —
//! [`super::tag_for`], [`super::props_for`], [`crate::css`] and
//! [`super::host_css`] — and the rules `glue.js` applies imperatively when it
//! creates an element (`applyProps`, `attach`, the canvas wrapper,
//! `renderMarkup`, `navigation.project`) are restated here, each beside the
//! rule it mirrors. A change to one side without the other is what the parity
//! check (a parsed document against the live DOM, in Chrome) exists to catch.
//!
//! What the browser decides after layout is not in a document: font loading,
//! symbol masks from computed styles, focus
//! (`autofocus` is the focus controller's), scrolling, context positioning,
//! windows chosen from scrollport geometry, and controls the glue disables
//! until its module is ready.
//!
//! Output the HTML parser would restructure — a link inside a link, a button
//! inside a button, a NUL it drops — is a refusal naming the view, never
//! repaired: the page a reader gets without JavaScript is the page the live
//! host builds, or it is no page.

use super::element::{
    blocks, contents, css_style_of, folds, host_css_of, props_of, restricts_touch, svg_props_of,
    tag_of,
};
use super::{font_names, layers, Host};

#[path = "page.rs"]
mod page;
use crate::css;
use exact_kernel::paint_order::{self, Child, Facts};
use exact_kernel::SortedMap;
use exact_kernel::{NodeFacts, PropId, ViewId};
use exact_plan::EventKind;
use exact_runner::{DataSource, DocTree, Runner};
pub use page::{
    build_locations, canonical_location, checkpoint, digest, read_checkpoint, route_at,
    route_location, Site,
};
use std::borrow::Cow;
use std::fmt;

/// A projected document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// The resolved strings table, or unknown language without tables.
    pub lang: String,
    /// CSS direction compiled from the table locale.
    pub dir: String,
    /// What `#exact-root` holds: the roots' elements, in order, with no
    /// whitespace between elements (a parser keeps whitespace as text).
    pub root: String,
    /// The first root's `viewport-fit`, which the glue writes into the
    /// viewport meta (`syncViewportFit`).
    pub viewport_fit: Option<String>,
    /// The first root's `interactive-widget`, likewise.
    pub interactive_widget: Option<String>,
    /// The active head's fields: the page's `<head>` (LLP 1048.003 D1). A
    /// head node has no element in the root.
    pub head: exact_runner::Head,
    /// Every `@keyframes` rule an element's `animation` names, once each
    /// (LLP 1055 D7), for the head: a reader without JavaScript sees it play.
    pub keyframes: String,
    /// An element is the page's scroller (`scroll document`, LLP 1048.003
    /// D4): `<html data-scrolldocument>`, which the shell's rule reads.
    pub scroll_document: bool,
}

/// Why a tree has no document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentError {
    /// The view whose element the parser would restructure.
    pub view: ViewId,
    /// What it would do.
    pub reason: String,
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "view {}: {}", self.view, self.reason)
    }
}

impl std::error::Error for DocumentError {}

impl<D: DataSource> Host<D> {
    /// This page's document: [`project`] over the host's runner.
    pub fn document(&self) -> Result<Document, DocumentError> {
        project(&self.runner)
    }
}

/// The document of `runner`'s current tree.
pub fn project<D: DataSource>(runner: &Runner<D>) -> Result<Document, DocumentError> {
    walk(runner, None).map(|(document, _)| document)
}

/// [`project`], and what it computed for each view, for the first batch.
pub(crate) fn project_keeping<D: DataSource>(
    runner: &Runner<D>,
) -> Result<(Document, Computed), DocumentError> {
    walk(runner, Some(Computed::new())).map(|(document, computed)| {
        let mut computed = computed.unwrap_or_default();
        computed.reverse();
        (document, computed)
    })
}

/// A tree a document is written from: a kernel's nodes, or those a render
/// holds without one ([`DocTree`], LLP 1048.004). One walk writes both, so
/// the two documents differ only where the trees do.
pub trait Source {
    /// The facts of node `id`, when it is live.
    fn facts(&self, id: ViewId) -> Option<NodeFacts<'_>>;
    /// Its children, in order.
    fn children(&self, id: ViewId) -> Cow<'_, [ViewId]>;
    /// The element an SVG reference from `from` names.
    fn resolve(&self, from: ViewId, id: &str) -> Option<ViewId>;
}

struct KernelSource<'k>(&'k exact_kernel::Kernel);

impl Source for KernelSource<'_> {
    fn facts(&self, id: ViewId) -> Option<NodeFacts<'_>> {
        self.0.node(id).map(|n| n.facts())
    }
    fn children(&self, id: ViewId) -> Cow<'_, [ViewId]> {
        Cow::Owned(self.0.node(id).map(|n| n.children()).unwrap_or_default())
    }
    fn resolve(&self, from: ViewId, id: &str) -> Option<ViewId> {
        self.0.resolve_id(from, id)
    }
}

impl Source for DocTree {
    fn facts(&self, id: ViewId) -> Option<NodeFacts<'_>> {
        DocTree::facts(self, id)
    }
    fn children(&self, id: ViewId) -> Cow<'_, [ViewId]> {
        Cow::Borrowed(self.node(id).map_or(&[][..], |n| n.children.as_slice()))
    }
    fn resolve(&self, from: ViewId, id: &str) -> Option<ViewId> {
        self.resolve_id(from, id)
    }
}

/// How a document is written beyond its tree: in which form, and to whom
/// as it goes.
#[derive(Default)]
pub struct Writing<'w> {
    /// The runtime's form (`host/render` `page::for_runtime`): only a link
    /// keeps a `data-view`, which is empty, and each inline style goes
    /// through this, which writes it as the shell's classes carry it and
    /// says so (`false`: it stays inline). `None`: the document as the wasm
    /// runtime adopts it, ids and all.
    pub style: Option<StyleRewrite<'w>>,
    /// Handed each finished run of the root's HTML of at least
    /// [`STREAM_CHUNK`] bytes, as it is written: a server sends it on.
    pub sink: Option<&'w mut dyn FnMut(&str)>,
}

/// How an inline style is written in the runtime's form ([`Writing::style`]).
pub type StyleRewrite<'w> = &'w dyn Fn(&str, &mut String) -> bool;

/// The run of HTML [`Writing::sink`] is handed at a time.
pub const STREAM_CHUNK: usize = 16 << 10;

/// The document of a render's own tree ([`exact_runner::Runner::document_tree`]),
/// written as [`project`] writes a kernel's, and in `writing`'s form. The
/// head, handlers and first root's policies are the tree's.
pub fn project_tree<D: DataSource>(
    tree: &DocTree,
    runner: &Runner<D>,
    writing: Writing<'_>,
) -> Result<(Document, String), DocumentError> {
    let handlers = tree.handlers(runner.plan());
    let (out, keyframes, scroll_document, _, sent) = write(
        tree,
        tree.roots(),
        handlers,
        font_names(runner.plan()),
        None,
        writing,
    )?;
    let document = Document {
        lang: runner.resolved_locale().into(),
        dir: runner.direction().into(),
        viewport_fit: first_prop(tree, tree.roots(), PropId::ViewportFit),
        interactive_widget: first_prop(tree, tree.roots(), PropId::InteractiveWidget),
        head: tree.head(),
        keyframes,
        scroll_document,
        root: out,
    };
    Ok((document, sent))
}

/// What a document's head and body need before its root is written, from
/// its tree alone (a streamed page sends them first): the `@keyframes`
/// rules its elements' animations name, and whether an element is the
/// page's scroller — as the walk finds them.
pub fn before_root(tree: &DocTree) -> (String, bool) {
    let mut keyframes = SortedMap::new();
    let mut scroll = false;
    for node in tree.nodes() {
        if node.node_type.is_metadata() {
            continue;
        }
        animations(&node.style, &mut keyframes);
        // `props_for` names it `data-scrolldocument` but on a filter's
        // elements, which take every prop by its own name.
        scroll |= !matches!(
            node.node_type,
            exact_kernel::NodeType::SvgFe | exact_kernel::NodeType::SvgFilter
        ) && node.props.bool(PropId::ScrollDocument) == Some(true);
    }
    (keyframes.values().map(String::as_str).collect(), scroll)
}

fn first_prop<S: Source>(src: &S, roots: &[ViewId], id: PropId) -> Option<String> {
    roots
        .first()
        .and_then(|root| src.facts(*root))
        .and_then(|n| n.props.str(id).map(str::to_owned))
}

fn walk<D: DataSource>(
    runner: &Runner<D>,
    computed: Option<Computed>,
) -> Result<(Document, Option<Computed>), DocumentError> {
    let src = KernelSource(runner.kernel());
    let roots = runner.roots();
    let (out, keyframes, scroll_document, computed, _) = write(
        &src,
        &roots,
        runner.handlers(),
        font_names(runner.plan()),
        computed,
        Writing::default(),
    )?;
    let document = Document {
        lang: runner.resolved_locale().into(),
        dir: runner.direction().into(),
        root: out,
        viewport_fit: first_prop(&src, &roots, PropId::ViewportFit),
        interactive_widget: first_prop(&src, &roots, PropId::InteractiveWidget),
        head: runner.head(),
        keyframes,
        scroll_document,
    };
    Ok((document, computed))
}

/// The roots' elements; the `@keyframes`, whether an element is the page's
/// scroller, what was computed, and what of the HTML the sink has not been
/// handed yet.
#[allow(clippy::type_complexity)]
fn write<S: Source>(
    src: &S,
    roots: &[ViewId],
    handlers: SortedMap<ViewId, Vec<EventKind>>,
    fonts: Vec<String>,
    computed: Option<Computed>,
    writing: Writing<'_>,
) -> Result<(String, String, bool, Option<Computed>, String), DocumentError> {
    let mut walk = Walk {
        src,
        isolation: isolation(src, roots),
        computed,
        fonts,
        handlers,
        routes: SortedMap::new(),
        keyframes: SortedMap::new(),
        out: String::new(),
        links: 0,
        buttons: 0,
        touch: 0,
        select: None,
        scroll_document: false,
        css: std::collections::HashMap::new(),
        style: writing.style,
        sink: writing.sink,
        sent: 0,
    };
    for root in roots {
        walk.element(*root, None, 16., false)?;
    }
    let rest = walk.out[walk.sent..].to_string();
    Ok((
        walk.out,
        walk.keyframes.values().map(String::as_str).collect(),
        walk.scroll_document,
        walk.computed,
        rest,
    ))
}

/// Decide every list before an opening tag can be streamed. This source may
/// be a DocTree with no kernel; the rule is still the kernel's pure functions.
fn isolation<S: Source>(src: &S, roots: &[ViewId]) -> SortedMap<ViewId, Child> {
    fn walk<S: Source>(
        src: &S,
        id: ViewId,
        beside_exclusion: bool,
        parent_display: Option<exact_kernel::Display>,
        out: &mut SortedMap<ViewId, Child>,
    ) -> Child {
        let node = src.facts(id).expect("live document node");
        let children = src.children(id);
        let exclusion = children.iter().any(|c| {
            src.facts(*c).is_some_and(|c| {
                c.style.position_type == exact_kernel::PositionType::Absolute
                    && c.style.wrap_flow == exact_kernel::WrapFlow::Both
            })
        });
        let own = paint_order::own_from(Facts {
            style: node.style,
            props: node.props,
            kind: node.node_type,
            root: node.is_root,
            parent_display,
            beside_exclusion,
            holds_layout_transition: children.iter().any(|c| {
                src.facts(*c)
                    .is_some_and(|c| c.style.mask.has(exact_kernel::StyleId::LayoutTransition))
            }),
        });
        let mut records: Vec<_> = children
            .iter()
            .map(|c| walk(src, *c, exclusion, Some(node.style.display), out))
            .collect();
        let facts: Vec<_> = records.iter().map(|c| (c.own, c.potentials)).collect();
        for ((id, child), isolated) in children
            .iter()
            .zip(&mut records)
            .zip(paint_order::decide(&facts))
        {
            child.isolated = isolated;
            out.insert(*id, *child);
        }
        Child {
            own,
            isolated: false,
            potentials: paint_order::potentials(&records),
        }
    }
    let mut out = SortedMap::new();
    for root in roots {
        let child = walk(src, *root, false, None, &mut out);
        out.insert(*root, child);
    }
    out
}

/// Add the rules `style`'s animations name, once each by name, for the
/// head: a reader without JavaScript sees them play (LLP 1055 D7).
fn animations(style: &exact_kernel::StyleProps, keyframes: &mut SortedMap<String, String>) {
    if let Some(link) = crate::link::linked().animations {
        let press = css::press_composes(style);
        for a in style.animation.0.iter().chain(&style.rare.exit_animation.0) {
            let name = (link.name)(a, press);
            if keyframes.get(&name).is_none() {
                let rule = format!("@keyframes {}{{{}}}", name, (link.body)(a, press));
                keyframes.insert(name, rule);
            }
        }
    }
}

/// What the router's projection (`navigation.project`) sets on a route.
#[derive(Debug, Clone, Copy)]
struct Route {
    hidden: bool,
    inert: bool,
}

/// What the projection computed for each view's element: its tag, props and
/// CSS (before the document's own additions), which the host's first batch
/// would compute again for the same views of the same tree (`Host::create`).
/// Last visited first: the first batch creates views in the order the
/// projection visits them, and takes each from the end.
pub(crate) type Computed = Vec<(ViewId, String, SortedMap<String, String>, String)>;

/// A navigation root's routes as `navigation.project` marks them when the
/// root has tabs (LLP 1075.003 §3.7), the `tabs` capability's walk:
/// `(view, hidden, inert)` for each tabpanel and each route. Every panel but
/// the selected one is hidden and inert, and each stack shows its own route;
/// without tabs the stack is the root's own rows. Empty when the key names
/// no route, which leaves the stack as it is.
pub type TabRoutes = fn(&dyn Source, &[ViewId], Option<&str>) -> Vec<(ViewId, bool, bool)>;

/// [`TabRoutes`]: the panels are those the root's own tablist's tabs name
/// with `aria-controls`, in tab order — `navigation.js`'s `panelsOf`. A
/// tablist inside a route is that route's, never the root's.
pub fn tab_routes(
    src: &dyn Source,
    children: &[ViewId],
    key: Option<&str>,
) -> Vec<(ViewId, bool, bool)> {
    // The root's tablists and tabpanels in tree order, none inside a route.
    let (mut lists, mut found) = (Vec::new(), Vec::new());
    let mut todo: Vec<ViewId> = children.iter().rev().copied().collect();
    while let Some(id) = todo.pop() {
        let Some(f) = src.facts(id) else { continue };
        match f.props.str(PropId::AccessibilityRole) {
            Some("tablist") => lists.push(id),
            Some("tabpanel") => found.push(id),
            _ => {}
        }
        if f.props.str(PropId::NavigationKey).is_none() {
            todo.extend(src.children(id).iter().rev());
        }
    }
    let mut panels = Vec::new();
    for list in lists {
        for tab in src.children(list).iter() {
            let Some(t) = src.facts(*tab) else { continue };
            let Some(target) = t.props.str(PropId::AccessibilityControls) else {
                continue;
            };
            if t.props.str(PropId::AccessibilityRole) != Some("tab") {
                continue;
            }
            let id = |p: &ViewId| {
                src.facts(*p)
                    .is_some_and(|f| f.props.str(PropId::Id) == Some(target))
            };
            panels.extend(found.iter().find(|p| id(p)));
        }
        if !panels.is_empty() {
            break;
        }
    }
    // Each stack's routes: (view, named by the key, modal).
    let mut stacks = Vec::new();
    let mut stack = |ids: &[ViewId]| {
        let mut routes = Vec::new();
        for id in ids {
            let Some(f) = src.facts(*id) else { continue };
            if let Some(k) = f.props.str(PropId::NavigationKey) {
                let modal = f.props.str(PropId::NavigationPresentation) == Some("modal");
                routes.push((*id, Some(k) == key, modal));
            }
        }
        stacks.push(routes);
    };
    if panels.is_empty() {
        stack(children);
    }
    for panel in &panels {
        stack(&src.children(*panel));
    }
    let mut out = Vec::new();
    let Some(at) = stacks.iter().position(|routes| routes.iter().any(|r| r.1)) else {
        return out;
    };
    for (index, panel) in panels.iter().enumerate() {
        out.push((*panel, index != at, index != at));
    }
    for (index, routes) in stacks.iter().enumerate() {
        let selected = if index == at {
            routes.iter().position(|r| r.1).unwrap_or(0)
        } else {
            routes.len().saturating_sub(1)
        };
        let modal = routes.get(selected).is_some_and(|r| r.2);
        for (i, route) in routes.iter().enumerate() {
            let active = i == selected;
            let shown = active || (modal && i + 1 == selected);
            out.push((route.0, !shown, !active));
        }
    }
    out
}

struct Walk<'r, 'w, S: Source> {
    src: &'r S,
    isolation: SortedMap<ViewId, Child>,
    computed: Option<Computed>,
    fonts: Vec<String>,
    handlers: SortedMap<ViewId, Vec<EventKind>>,
    routes: SortedMap<ViewId, Route>,
    keyframes: SortedMap<String, String>,
    out: String,
    /// Open `a` and `button` elements: the parser closes an open one when a
    /// second starts inside it, and a button's containers are `<span>`s.
    links: u32,
    buttons: u32,
    /// Open boxes that restrict touch (`element::restricts_touch`): a folded
    /// text under one is an inline box.
    touch: u32,
    /// The open `select`'s value: the option that carries it is `selected`.
    select: Option<String>,
    scroll_document: bool,
    /// A shared style's CSS, by the style's address: the nodes of a
    /// repeated template share one style, and its CSS is computed once.
    css: std::collections::HashMap<usize, String>,
    /// [`Writing::style`].
    style: Option<StyleRewrite<'w>>,
    /// [`Writing::sink`], and how much of `out` it has been handed.
    sink: Option<&'w mut dyn FnMut(&str)>,
    sent: usize,
}

impl<S: Source> Walk<'_, '_, S> {
    /// The element and its subtree, with isolation already decided.
    fn element(
        &mut self,
        id: ViewId,
        up: Option<ViewId>,
        inherited_font: f32,
        folded: bool,
    ) -> Result<(), DocumentError> {
        let src = self.src;
        let node = src.facts(id).expect("the runner's tree names live views");
        let above = up.and_then(|u| src.facts(u));
        if node.node_type.is_metadata() {
            // The page's `<head>`, never an element (as the live host).
            return Ok(());
        }
        let refuse = |reason: &str| DocumentError {
            view: id,
            reason: reason.to_owned(),
        };
        let font = if node.style.mask.has(exact_kernel::StyleId::FontSize) {
            node.style.font_size
        } else {
            inherited_font
        };
        let tag = tag_of(&node, self.buttons > 0);
        match tag {
            "a" if self.links > 0 => return Err(refuse("a link inside a link")),
            "button" if self.buttons > 0 => return Err(refuse("a button inside a button")),
            _ => {}
        }
        let resolve = |from: ViewId, target: &str| src.resolve(from, target);
        let mut props = props_of(&node);
        svg_props_of(&resolve, &node, &mut props);
        self.scroll_document |= props
            .get("data-scrolldocument")
            .is_some_and(|v| v == "true");
        let text = match css_style_of(&resolve, &node) {
            Cow::Borrowed(style) => {
                let fonts = &self.fonts;
                self.css
                    .entry(style as *const exact_kernel::StyleProps as usize)
                    .or_insert_with(|| css::css_text(style, fonts).0)
                    .clone()
            }
            Cow::Owned(style) => css::css_text(&style, &self.fonts).0,
        };
        animations(node.style, &mut self.keyframes);
        let children = src.children(id);
        let paint = self.isolation.get(&id).copied().unwrap_or_default();
        let isolated = paint.isolated || paint.own.policy;
        let holds = children.len() == 1
            && src.facts(children[0]).is_some_and(|c| {
                let handled = self
                    .handlers
                    .get(&children[0])
                    .is_some_and(|k| !k.is_empty());
                folds(&c, &node, above.as_ref(), true, handled)
            });
        let css = blocks(
            contents(
                host_css_of(&node, above.as_ref(), text, tag),
                folded,
                self.touch > 0,
            ),
            holds,
        );
        let mut style = layers::with_isolation(css, isolated);
        let kept = self.computed.is_some().then(|| style.clone());
        // `glue.js` create: a canvas is a `div` holding the surface element.
        let element = if tag == "canvas" { "div" } else { tag };
        self.route_children(&node, &children);
        let chosen = (element == "select").then(|| props.get("value").cloned());
        // A waiting keyed row is opaque to adoption. Its complete paint
        // summary lets the JS target decide siblings without visiting it.
        // Its parent and siblings can change before it adopts: keep inputs,
        // not their current effects on z applicability or text-flow policy.
        let waiting = paint_order::own_from(Facts {
            style: node.style,
            props: node.props,
            kind: node.node_type,
            root: node.is_root,
            parent_display: None,
            beside_exclusion: false,
            holds_layout_transition: children.iter().any(|c| {
                src.facts(*c)
                    .is_some_and(|c| c.style.mask.has(exact_kernel::StyleId::LayoutTransition))
            }),
        });
        // Bits mirror paint.js's template facts, plus descendant potentials.
        let flags = u16::from(waiting.positioned)
            | (u16::from(waiting.stacks && !waiting.policy) << 1)
            | (u16::from(waiting.policy) << 2)
            | (u16::from(waiting.outside) << 3)
            | (u16::from(paint.potentials.z) << 4)
            | (u16::from(paint.potentials.level0) << 5)
            | (u16::from(node.style.isolation == exact_kernel::Isolation::Isolate) << 6)
            | (u16::from(node.is_root) << 7)
            | (u16::from(node.style.mask.has(exact_kernel::StyleId::LayoutTransition)) << 8)
            | (u16::from(node.style.position_type == exact_kernel::PositionType::Absolute) << 9)
            | (u16::from(node.style.wrap_flow == exact_kernel::WrapFlow::Both) << 10)
            | (u16::from(node.node_type == exact_kernel::NodeType::Text) << 11)
            | (u16::from(matches!(
                node.style.display,
                exact_kernel::Display::Flex | exact_kernel::Display::Grid
            )) << 12);
        let z = node.style.mask.has(exact_kernel::StyleId::ZIndex).then(|| {
            node.style
                .z_index
                .clamp(-paint_order::Z_MAX, paint_order::Z_MAX)
        });
        let z = z.map_or_else(|| "null".into(), |z| z.to_string());
        let mut attrs: Vec<(String, Option<String>)> = Vec::new();
        let mut content: Option<String> = None;
        let mut markup: Option<String> = None;
        for (name, value) in &props {
            match name.as_str() {
                // Browser-owned state the glue keeps in JavaScript.
                "scrollFollowEnd" | "scrollTop" | "scrollLeft" | "autofocus" => {}
                // A sized, transparent source supplies the natural box before
                // JavaScript. The live renderer adds the portable role's mask.
                "src" if element == "img" && value.starts_with("symbol:") => {
                    attrs.push((
                        name.clone(),
                        Some(format!("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='{font}' height='{font}'/%3E")),
                    ));
                }
                // `el.textContent = value` while it has no element children:
                // a canvas already holds its surface.
                "text" => {
                    if tag != "canvas" && !value.is_empty() {
                        content = Some(value.clone());
                    }
                }
                "markupPieces" => markup = Some(value.clone()),
                "data-action" => {
                    attrs.push((name.clone(), Some(value.clone())));
                    style.push_str("touch-action:none;");
                }
                // `writeValue`: an input's value and a button's (a reflected
                // attribute) are the element's; a textarea's is its text.
                "value" => match element {
                    "input" | "button" | "option" => {
                        if element == "option" && self.select.as_ref() == Some(value) {
                            attrs.push(("selected".into(), None));
                        }
                        attrs.push((name.clone(), Some(value.clone())));
                    }
                    "textarea" => content = Some(value.clone()),
                    _ => {}
                },
                "checked" | "inert" | "disabled" | "readonly" | "multiple" => {
                    if value == "true" {
                        attrs.push((name.clone(), None));
                    }
                }
                "autoplay"
                | "controls"
                | "loop"
                | "muted"
                | "playsinline"
                | "disablepictureinpicture"
                | "disableremoteplayback"
                    if element == "video" || element == "audio" =>
                {
                    if value == "true" {
                        attrs.push((name.clone(), None));
                    }
                }
                // `navigates` + `navigableURL`: a refused link loses its
                // href; a refused frame shows about:blank.
                "href" if !navigable(value) => {}
                "src" if element == "iframe" && !navigable(value) => {
                    attrs.push((name.clone(), Some("about:blank".into())));
                }
                _ => attrs.push((name.clone(), Some(value.clone()))),
            }
        }
        if let (Some(computed), Some(css)) = (self.computed.as_mut(), kept) {
            computed.push((id, tag.into(), props, css));
        }
        // `navigation.project`: routes other than the selected one (and the
        // one under a selected modal) are hidden; every route but the
        // selected one is inert.
        if let Some(route) = self.routes.get(&id).copied() {
            if route.hidden {
                style.push_str("visibility:hidden;");
            }
            if route.inert && !attrs.iter().any(|(n, _)| n == "inert") {
                attrs.push(("inert".into(), None));
            }
        }
        // `attach`: the view id, and a tab stop for an element that hears
        // focus, blur or keys and is not one already.
        attrs.push(("data-view".into(), Some(id.to_string())));
        if let Some(kinds) = self.handlers.get(&id).filter(|kinds| !kinds.is_empty()) {
            attrs.push((
                "data-exact-on".into(),
                Some({
                    let mut names = String::new();
                    for (i, kind) in kinds.iter().enumerate() {
                        if i > 0 {
                            names.push(' ');
                        }
                        names.push_str(kind.name());
                    }
                    names
                }),
            ));
        }
        let hears = self.handlers.get(&id).is_some_and(|kinds| {
            kinds
                .iter()
                .any(|k| matches!(k, EventKind::Focus | EventKind::Blur | EventKind::Key))
        });
        // An authored `tabindex` is explicit and wins, a negative one
        // included (LLP 1088 D7.3), as `glue.js`'s `hasAttribute` check does.
        if hears
            && !matches!(element, "input" | "button")
            && !attrs.iter().any(|(n, _)| n == "tabindex")
        {
            attrs.push(("tabindex".into(), Some("0".into())));
        }
        if !style.is_empty() {
            attrs.push(("style".into(), Some(style)));
        }
        attrs.push(("data-exact-paint".into(), Some(format!("[{flags},{z}]"))));
        self.open(id, element, &attrs)?;
        if matches!(element, "img" | "input" | "hr") {
            // Void: no content, no end tag.
            return Ok(());
        }
        if tag == "canvas" {
            if self.style.is_some() {
                self.open(
                    id,
                    "canvas",
                    &[
                        ("data-surface".into(), Some(String::new())),
                        ("style".into(), Some(SURFACE_STYLE.into())),
                    ],
                )?;
                self.out.push_str("</canvas>");
            } else {
                self.out.push_str(SURFACE);
            }
        }
        if let (Some(json), true) = (&markup, children.is_empty()) {
            // `renderMarkup`; a node's children replace its pieces.
            self.markup(id, json)?;
        } else if let Some(text) = &content {
            // The parser drops a newline right after `<textarea>`.
            if element == "textarea" && text.starts_with('\n') {
                self.out.push('\n');
            }
            escape(&mut self.out, text, false).map_err(refuse)?;
        }
        let (link, button) = (element == "a", element == "button");
        self.links += u32::from(link);
        self.buttons += u32::from(button);
        let touch = restricts_touch(&node);
        self.touch += u32::from(touch);
        let outer = chosen.map(|value| std::mem::replace(&mut self.select, value));
        let only = children.len() == 1;
        for child in children.iter().copied() {
            let handled = self.handlers.get(&child).is_some_and(|k| !k.is_empty());
            let fold = only
                && src
                    .facts(child)
                    .is_some_and(|c| folds(&c, &node, above.as_ref(), true, handled));
            self.element(child, Some(id), font, fold)?;
        }
        if let Some(outer) = outer {
            self.select = outer;
        }
        self.links -= u32::from(link);
        self.buttons -= u32::from(button);
        self.touch -= u32::from(touch);
        self.out.push_str("</");
        self.out.push_str(element);
        self.out.push('>');
        self.stream();
        Ok(())
    }

    /// Hand the sink what has been written since, once it is a chunk.
    fn stream(&mut self) {
        if let Some(sink) = self.sink.as_mut() {
            if self.out.len() - self.sent >= STREAM_CHUNK {
                sink(&self.out[self.sent..]);
                self.sent = self.out.len();
            }
        }
    }

    fn open(
        &mut self,
        id: ViewId,
        element: &str,
        attrs: &[(String, Option<String>)],
    ) -> Result<(), DocumentError> {
        self.out.push('<');
        self.out.push_str(element);
        let runtime = self.style;
        let has_class = runtime.is_some() && attrs.iter().any(|(n, _)| n == "class");
        for (name, value) in attrs {
            // The runtime's form (`page::for_runtime`): a link keeps an
            // empty `data-view`, no other element one; a style goes as the
            // shell's classes carry it.
            if let Some(rewrite) = runtime {
                match (name.as_str(), value) {
                    ("data-view", _) if element == "a" => {
                        self.out.push_str(" data-view");
                        continue;
                    }
                    ("data-view", _) => continue,
                    ("style", Some(css)) if !has_class && rewrite(css, &mut self.out) => continue,
                    _ => {}
                }
            }
            self.out.push(' ');
            self.out.push_str(name);
            if let Some(value) = value {
                self.out.push_str("=\"");
                escape(&mut self.out, value, true).map_err(|reason| DocumentError {
                    view: id,
                    reason: format!("attribute `{name}`: {reason}"),
                })?;
                self.out.push('"');
            }
        }
        self.out.push('>');
        Ok(())
    }

    /// Mark the routes under a navigation root as `navigation.project` does;
    /// with tabs, as the `tabs` capability does ([`tab_routes`]).
    fn route_children(&mut self, node: &NodeFacts<'_>, children: &[ViewId]) {
        if node.props.str(PropId::NavigationBack).is_none() {
            return;
        }
        let src = self.src;
        let key = node.props.str(PropId::NavigationKey);
        if let Some(tabs) = crate::link::linked().tabs {
            for (id, hidden, inert) in tabs(src, children, key) {
                self.routes.insert(id, Route { hidden, inert });
            }
            return;
        }
        let routes: Vec<NodeFacts<'_>> = children
            .iter()
            .filter_map(|c| src.facts(*c))
            .filter(|c| c.props.str(PropId::NavigationKey).is_some())
            .collect();
        // A key that names no route leaves the stack as it is.
        let Some(selected) = routes
            .iter()
            .position(|r| r.props.str(PropId::NavigationKey) == key)
        else {
            return;
        };
        let modal = routes[selected].props.str(PropId::NavigationPresentation) == Some("modal");
        for (index, route) in routes.iter().enumerate() {
            let active = index == selected;
            let shown = active || (modal && index + 1 == selected);
            self.routes.insert(
                route.id,
                Route {
                    hidden: !shown,
                    inert: !active,
                },
            );
        }
    }

    /// `renderMarkup`: each piece a `span`, or an `a` when it is a link to
    /// a navigable destination; newlines are `<br>`s. A list item's
    /// paragraph is a block `span` padded by its indent, its marker hung in
    /// the gutter by a negative `text-indent` (LLP 1045 D4).
    fn markup(&mut self, id: ViewId, json: &str) -> Result<(), DocumentError> {
        let refuse = |reason: String| DocumentError { view: id, reason };
        // At a paragraph's start; inside a list item's block.
        let (mut start, mut block) = (true, false);
        for piece in markup_pieces(json).map_err(refuse)? {
            let link = piece.flags & 8 != 0 && !piece.href.is_empty() && navigable(&piece.href);
            if link && self.links > 0 {
                return Err(refuse("a Markdown link inside a link".into()));
            }
            let hang = piece.flags & 32 != 0;
            let mut style = String::new();
            if piece.scale != "1" {
                style.push_str(&format!("font-size:{}em;", piece.scale));
            }
            if piece.weight != "0" {
                style.push_str(&format!("font-weight:{};", piece.weight));
            }
            if piece.flags & 1 != 0 {
                style.push_str("font-style:italic;");
            }
            if piece.flags & 2 != 0 {
                style.push_str("font-family:ui-monospace, monospace;");
            }
            if piece.flags & 4 != 0 {
                style.push_str("text-decoration:line-through;");
            }
            if piece.flags & 16 != 0 {
                style.push_str("opacity:0.62;");
            }
            if hang {
                style.push_str(
                    "display:inline-flex;justify-content:flex-end;width:40px;white-space:pre;text-indent:0;",
                );
            }
            let element = if link { "a" } else { "span" };
            let mut attrs = Vec::new();
            if !style.is_empty() {
                attrs.push(("style".to_owned(), Some(style)));
            }
            if link {
                attrs.push(("href".to_owned(), Some(piece.href.clone())));
            }
            let lines: Vec<&str> = if hang {
                vec![piece.text.as_str()]
            } else {
                piece.text.split('\n').collect()
            };
            // One element per paragraph the piece is in, as the page makes them.
            let mut open = false;
            for (n, line) in lines.into_iter().enumerate() {
                if n > 0 {
                    if !open {
                        self.paragraph(id, (&mut start, &mut block), &piece.indent, hang)?;
                        self.open(id, element, &attrs)?;
                    }
                    self.out.push_str("<br></");
                    self.out.push_str(element);
                    self.out.push('>');
                    (open, start) = (false, true);
                }
                if !line.is_empty() {
                    if !open {
                        self.paragraph(id, (&mut start, &mut block), &piece.indent, hang)?;
                        self.open(id, element, &attrs)?;
                        open = true;
                    }
                    escape(&mut self.out, line, false).map_err(|e| refuse(e.to_owned()))?;
                }
            }
            if open {
                self.out.push_str("</");
                self.out.push_str(element);
                self.out.push('>');
            }
        }
        if block {
            self.out.push_str("</span>");
        }
        Ok(())
    }

    /// A paragraph begins at `state.0`: a list item's (a nonzero indent) is
    /// a block, closing the one before it, and a hung marker pulls its first
    /// line back by the gutter.
    fn paragraph(
        &mut self,
        id: ViewId,
        state: (&mut bool, &mut bool),
        indent: &str,
        hang: bool,
    ) -> Result<(), DocumentError> {
        let (start, block) = state;
        if !std::mem::take(start) {
            return Ok(());
        }
        if std::mem::take(block) {
            self.out.push_str("</span>");
        }
        if indent != "0" {
            let mut css = format!("display:block;padding-left:{indent}px;");
            if hang {
                css.push_str("text-indent:-40px;");
            }
            self.open(id, "span", &[("style".to_owned(), Some(css))])?;
            *block = true;
        }
        Ok(())
    }
}

/// The surface element `glue.js` puts first in a canvas's `div`.
const SURFACE: &str = "<canvas data-surface=\"\" style=\"position:absolute;inset:0;width:100%;height:100%;display:block;z-index:-1\"></canvas>";
/// [`SURFACE`]'s style.
const SURFACE_STYLE: &str =
    "position:absolute;inset:0;width:100%;height:100%;display:block;z-index:-1";

/// Append `text` escaped for HTML text (`attribute` false) or a
/// double-quoted attribute value. A carriage return is a reference (the
/// parser folds a literal one into a newline); a NUL has no spelling the
/// parser keeps.
fn escape(out: &mut String, text: &str, attribute: bool) -> Result<(), &'static str> {
    // Every byte that needs a reference is ASCII, so the runs between them
    // end on character boundaries and are copied whole.
    let mut run = 0;
    for (at, b) in text.bytes().enumerate() {
        let reference = match b {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' if attribute => "&quot;",
            b'\r' => "&#13;",
            0 => return Err("a NUL character, which the HTML parser drops"),
            _ => continue,
        };
        out.push_str(&text[run..at]);
        out.push_str(reference);
        run = at + 1;
    }
    out.push_str(&text[run..]);
    Ok(())
}

/// Whether `href` navigates somewhere the page allows: `navigableURL`'s one
/// scheme allowlist (http, https, mailto, tel), read the way the URL parser
/// reads a scheme — surrounding C0 controls and spaces stripped, tabs and
/// newlines removed anywhere, the scheme case-folded. A URL with no scheme is
/// relative to the page's https base; an http(s) authority whose host does
/// not parse is refused, as the parser's failure is (IDNA aside).
pub fn navigable(href: &str) -> bool {
    let trimmed = href.trim_matches(|c: char| c <= ' ');
    let url: std::borrow::Cow<'_, str> = if trimmed.contains(['\t', '\n', '\r']) {
        trimmed
            .chars()
            .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
            .collect::<String>()
            .into()
    } else {
        trimmed.into()
    };
    // A scheme is a letter, then letters, digits, `+`, `-` or `.`, then `:`.
    for (at, b) in url.bytes().enumerate() {
        match b {
            b':' if at > 0 => {
                let (scheme, rest) = (&url[..at], &url[at + 1..]);
                let is = |name: &str| scheme.eq_ignore_ascii_case(name);
                return if is("mailto") || is("tel") {
                    true
                } else if is("https") && !rest.starts_with("//") {
                    // The base's own scheme reads the rest as relative
                    // unless it starts `//`; another special scheme's
                    // authority follows whatever slashes there are.
                    true
                } else {
                    (is("http") || is("https")) && authority_parses(rest)
                };
            }
            b if b.is_ascii_alphabetic() => {}
            b if at > 0 && (b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.')) => {}
            _ => break,
        }
    }
    // Relative: a leading pair of slashes (either way) starts an authority.
    let mut slashes = url.chars().take_while(|c| matches!(c, '/' | '\\'));
    if slashes.next().is_some() && slashes.next().is_some() {
        return authority_parses(&url);
    }
    true
}

/// The special authority after any slashes: a host (a bracketed IPv6, an
/// IPv4 when its last label is numeric, or a domain free of forbidden code
/// points) and a port of at most 65535.
fn authority_parses(rest: &str) -> bool {
    let rest = rest.trim_start_matches(['/', '\\']);
    let authority = &rest[..rest.find(['/', '\\', '?', '#']).unwrap_or(rest.len())];
    let hostport = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let (host, port) = if let Some(inner) = hostport.strip_prefix('[') {
        let Some((v6, after)) = inner.split_once(']') else {
            return false;
        };
        if v6.is_empty()
            || !v6
                .chars()
                .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
        {
            return false;
        }
        match after {
            "" => (hostport, None),
            p => match p.strip_prefix(':') {
                Some(port) => (hostport, Some(port)),
                None => return false,
            },
        }
    } else {
        match hostport.split_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (hostport, None),
        }
    };
    if port.is_some_and(|p| {
        !p.is_empty()
            && (!p.bytes().all(|b| b.is_ascii_digit())
                || p.parse::<u32>().map_or(true, |n| n > 65535))
    }) {
        return false;
    }
    if host.starts_with('[') {
        return true;
    }
    let decoded = percent_decoded(host);
    if decoded.is_empty()
        || decoded.chars().any(|c| {
            c <= ' '
                || c == '\u{7f}'
                || matches!(
                    c,
                    '#' | '%' | '/' | ':' | '<' | '>' | '?' | '@' | '[' | '\\' | ']' | '^' | '|'
                )
        })
    {
        return false;
    }
    // A host whose last label is a number is an IPv4 address and must be one.
    let labels: Vec<&str> = decoded.trim_end_matches('.').split('.').collect();
    let number = |label: &str| -> Option<u64> {
        let lower = label.to_ascii_lowercase();
        match lower.strip_prefix("0x") {
            Some("") => Some(0),
            Some(hex) => u64::from_str_radix(hex, 16).ok(),
            None if lower.len() > 1 && lower.starts_with('0') => {
                u64::from_str_radix(&lower[1..], 8).ok()
            }
            None => lower.parse().ok(),
        }
    };
    let last = labels.last().copied().unwrap_or_default();
    let numeric = !last.is_empty()
        && (last.bytes().all(|b| b.is_ascii_digit())
            || last
                .to_ascii_lowercase()
                .strip_prefix("0x")
                .is_some_and(|h| h.bytes().all(|b| b.is_ascii_hexdigit())));
    if !numeric {
        return true;
    }
    let Some(parts) = labels
        .iter()
        .map(|l| number(l))
        .collect::<Option<Vec<u64>>>()
    else {
        return false;
    };
    let (init, tail) = parts.split_at(parts.len() - 1);
    parts.len() <= 4
        && init.iter().all(|n| *n <= 255)
        && tail[0] < 256u64.pow(5 - parts.len() as u32)
}

/// `host`'s percent-escapes decoded as the host parser decodes them (bytes
/// that are not UTF-8 make it unparseable, as an empty host is).
fn percent_decoded(host: &str) -> String {
    let bytes = host.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (
            bytes[i],
            bytes.get(i + 1).and_then(|b| hex(*b)),
            bytes.get(i + 2).and_then(|b| hex(*b)),
        ) {
            (b'%', Some(h), Some(l)) => {
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            (b, _, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_default()
}

/// One piece of `markupPieces`, as the page reads it: numbers keep the
/// spelling the glue's template literals would give them.
#[derive(Debug, Clone, PartialEq)]
struct Piece {
    text: String,
    scale: String,
    weight: String,
    flags: u8,
    href: String,
    /// A list item's indent, CSS px; "0" outside a list.
    indent: String,
}

/// Read the host's own `markupPieces` back (`[[text, scale, weight, flags,
/// href, indent?], …]`), as the linked Markdown capability wrote them; anything else
/// is a defect.
fn markup_pieces(json: &str) -> Result<Vec<Piece>, String> {
    let mut p = Json::new(json);
    let mut pieces = Vec::new();
    p.expect(b'[')?;
    if p.peek() == Some(b']') {
        return Ok(pieces);
    }
    loop {
        p.expect(b'[')?;
        let text = p.string()?;
        p.expect(b',')?;
        let scale = p.number()?;
        p.expect(b',')?;
        let weight = p.number()?;
        p.expect(b',')?;
        let flags = p.number()?.parse::<u8>().map_err(|e| e.to_string())?;
        p.expect(b',')?;
        let href = p.string()?;
        let indent = if p.peek() == Some(b',') {
            p.next();
            p.number()?
        } else {
            "0".to_owned()
        };
        p.expect(b']')?;
        pieces.push(Piece {
            text,
            scale,
            weight,
            flags,
            href,
            indent,
        });
        match p.next() {
            Some(b',') => continue,
            Some(b']') => return Ok(pieces),
            other => return Err(format!("markup pieces: unexpected {other:?}")),
        }
    }
}

/// A `dataset` row's pairs, the `dataset` capability's reading (LLP 1075.003
/// §3.3): the runner's object of strings (`NativeProps`,
/// LLP 1024 §9), written without whitespace and with JSON's escapes for
/// `"`, `\\` and control characters only. Anything else yields what it
/// parsed so far.
pub fn dataset(json: &str) -> Vec<(String, String)> {
    let mut p = Json::new(json);
    let mut pairs = Vec::new();
    if p.expect(b'{').is_err() {
        return pairs;
    }
    while p.peek() == Some(b'"') {
        let Ok(key) = p.string() else { break };
        let (Ok(()), Ok(value)) = (p.expect(b':'), p.string()) else {
            break;
        };
        pairs.push((key, value));
        if p.peek() == Some(b',') {
            p.next();
        }
    }
    pairs
}

/// A reader of the JSON the host and the runner write themselves: the
/// `markupPieces`, and a `dataset` row ([`dataset`]).
struct Json<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Json<'a> {
    fn new(text: &'a str) -> Self {
        Json {
            bytes: text.as_bytes(),
            at: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let b = self.peek();
        self.at += 1;
        b
    }

    fn expect(&mut self, b: u8) -> Result<(), String> {
        match self.next() {
            Some(got) if got == b => Ok(()),
            got => Err(format!(
                "markup pieces: expected `{}`, found {got:?}",
                b as char
            )),
        }
    }

    fn number(&mut self) -> Result<String, String> {
        let start = self.at;
        while matches!(
            self.peek(),
            Some(b'0'..=b'9' | b'.' | b'-' | b'e' | b'E' | b'+')
        ) {
            self.at += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.at]).map_err(|e| e.to_string())?;
        // The glue's `${scale}` and `fontWeight = weight` spell the number
        // as JavaScript does; the host's `css::num` already writes it so.
        exact_num::parse_f64(text)
            .map(|_| text.to_owned())
            .map_err(|e| format!("markup pieces: number {text:?}: {e}"))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let start = self.at;
            while !matches!(self.peek(), Some(b'"' | b'\\') | None) {
                self.at += 1;
            }
            out.push_str(
                std::str::from_utf8(&self.bytes[start..self.at]).map_err(|e| e.to_string())?,
            );
            match self.next() {
                Some(b'"') => return Ok(out),
                Some(b'\\') => match self.next() {
                    Some(b'"') => out.push('"'),
                    Some(b'\\') => out.push('\\'),
                    Some(b'n') => out.push('\n'),
                    Some(b'r') => out.push('\r'),
                    Some(b't') => out.push('\t'),
                    Some(b'u') => {
                        let hex = self
                            .bytes
                            .get(self.at..self.at + 4)
                            .and_then(|h| std::str::from_utf8(h).ok())
                            .and_then(|h| u32::from_str_radix(h, 16).ok())
                            .and_then(char::from_u32)
                            .ok_or("markup pieces: bad \\u escape")?;
                        self.at += 4;
                        out.push(hex);
                    }
                    other => return Err(format!("markup pieces: bad escape {other:?}")),
                },
                _ => return Err("markup pieces: unterminated string".into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{escape, markup_pieces, navigable};

    #[test]
    fn schemes_are_read_as_the_url_parser_reads_them() {
        for allowed in [
            "https://e.dev/a",
            "HTTP://e.dev",
            "mailto:a@e.dev",
            "tel:+15555550100",
            " https://e.dev ",
            "/post/5",
            "post/5?q=1#x",
            "#top",
            "",
            "//e.dev/a",
            "?q=javascript:x",
        ] {
            assert!(navigable(allowed), "{allowed:?}");
        }
        for refused in [
            "javascript:alert(1)",
            "JaVaScRiPt:alert(1)",
            "java\tscript:alert(1)",
            "java\nscript:alert(1)",
            "\u{1}javascript:alert(1)",
            " javascript:alert(1)",
            "data:text/html,<b>x</b>",
            "vbscript:x",
            "file:///etc/passwd",
            "c:/x",
            "a+b.c-d:x",
            // Hosts the parser cannot read: the browser refuses the link.
            "http:",
            "http://",
            "//exa mple.com/",
            "//[::1",
            "https://a b/",
            "https://e.dev:99999/",
            "https://e.dev:8o/",
            "http://1.2.3.999/",
            "http://e%20v.dev/",
        ] {
            assert!(!navigable(refused), "{refused:?}");
        }
        // Read as the browser reads them (each checked against Bun's URL).
        for allowed in [
            "\u{a0}javascript:x",
            "\\\\e.dev\\x",
            "1http:x",
            "-x:y",
            "javascript\u{a0}:x",
            "https:post/5",
            "http:e.dev",
            "http://[::1]:8080/",
            "http://user@e.dev:80/a",
            "http://0x7f.1/",
            "http://e.dev./",
            "https://bücher.example/",
        ] {
            assert!(navigable(allowed), "{allowed:?}");
        }
        assert!(!navigable("javas\rcript:x"));
    }

    #[test]
    fn escaping_keeps_what_the_parser_would_fold_or_drop() {
        let mut out = String::new();
        escape(&mut out, "a<b>&\"c\"\r\n", true).unwrap();
        assert_eq!(out, "a&lt;b&gt;&amp;&quot;c&quot;&#13;\n");
        let mut out = String::new();
        escape(&mut out, "\"q\"", false).unwrap();
        assert_eq!(out, "\"q\"");
        assert!(escape(&mut String::new(), "a\0b", false).is_err());
    }

    #[test]
    fn markup_pieces_read_back_what_the_host_wrote() {
        // What `exact_web_capabilities::markdown::pieces` writes for
        // "# T \"q\"\n\n**b** [l](https://e.dev/a?b=1) `c`", as its test pins.
        let json = r#"[["T \"q\"",1.6,700,0,""],["\n",1,0,0,""],["\n",0.5,0,0,""],["b",1,700,0,""],[" ",1,0,0,""],["l",1,0,8,"https://e.dev/a?b=1"],[" ",1,0,0,""],["c",0.92,0,2,""]]"#;
        let pieces = markup_pieces(json).unwrap();
        assert_eq!(pieces[0].text, "T \"q\"");
        assert_eq!(pieces[0].scale, "1.6");
        assert_eq!(pieces[0].weight, "700");
        assert_eq!(pieces[5].flags, 8);
        assert_eq!(pieces[5].href, "https://e.dev/a?b=1");
        assert_eq!(pieces.last().unwrap().scale, "0.92");
        assert!(markup_pieces("[]").unwrap().is_empty());
        assert_eq!(
            markup_pieces(r#"[["a\\b\tc\u0001",1,0,0,""]]"#).unwrap()[0].text,
            "a\\b\tc\u{1}"
        );
    }
}
