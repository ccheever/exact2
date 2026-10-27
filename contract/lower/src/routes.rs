//! @ref LLP 1038 D2/D3 — the table and launch slot; paths use ordinary templates.

use super::*;

impl Lowerer<'_> {
    pub(super) fn declare_routes(&mut self, file: &File) {
        let Some(routes) = &file.routes else { return };
        for row in &routes.rows {
            let id = self.b.route(
                &row.name,
                &row.pattern,
                row.parent.map(|p| exact_plan::RoutesId(p as u32)),
                row.tab,
                row.notfound,
            );
            match route_policy(row) {
                Ok((render, activate, paint)) => {
                    self.b.set_route_policy(id, render, activate, paint)
                }
                Err(e) => self.errors.push(e),
            }
            // @ref LLP 1048.000 D2 — the source listing its pages, and its
            // arguments (values, as types checked).
            let pages = row.fields.iter().find(|f| f.name == "pages");
            if let Some(Expr::Call(source, args, _)) = pages.map(|f| &f.value) {
                let args: Vec<Value> = args
                    .iter()
                    .map(|arg| match arg {
                        Expr::Number(n, _) => Value::Number(*n),
                        Expr::Str(s, _) => Value::str(s),
                        Expr::Bool(b, _) => Value::Bool(*b),
                        _ => Value::Unit,
                    })
                    .collect();
                self.b.set_route_pages(id, source, &args);
            }
        }
        // @ref LLP 1116.009 D2 — the `notfound` the compiler writes: the
        // table `contract_types::routes` checked, in the plan.
        if contract_syntax::implicit_notfound(file) {
            self.b.route("notfound", "", None, false, true);
        }
        // expand inserted this root slot before all authored/lifted states.
        self.b.set_router(self.slots[0]);
    }

    pub(crate) fn path_expr(
        &self,
        args: &[Expr],
        span: Span,
        scope: &Scope,
    ) -> Result<Expr, LowerError> {
        contract_types::routes::expand_path(args, span, scope, &self.types.shapes).map_err(|e| {
            LowerError {
                id: e.id,
                message: e.message,
                span: e.span,
            }
        })
    }
}

/// A route's policy fields (LLP 1048.003 D5): `render=client|build|cached|
/// request`, undeclared `client`; `activate=idle|never|interaction`, inferred when
/// undeclared; `paint=settled|boot` (LLP 1048.005), undeclared `settled`. Each
/// value is a word, read by its spelling.
fn route_policy(
    row: &contract_syntax::RouteDecl,
) -> Result<
    (
        exact_plan::RenderPolicy,
        exact_plan::ActivatePolicy,
        exact_plan::PaintPolicy,
    ),
    LowerError,
> {
    use exact_plan::{ActivatePolicy, PaintPolicy, RenderPolicy};
    let mut render = RenderPolicy::Client;
    let mut activate = ActivatePolicy::Inferred;
    let mut paint = PaintPolicy::Settled;
    for field in &row.fields {
        let word = match &field.value {
            Expr::Ident(word, _) => word.as_str(),
            _ => "",
        };
        let refusal = match (field.name.as_str(), word) {
            ("render", "client") => {
                render = RenderPolicy::Client;
                continue;
            }
            ("render", "build") => {
                render = RenderPolicy::Build;
                continue;
            }
            ("render", "cached") => {
                render = RenderPolicy::Cached;
                continue;
            }
            ("render", "request") => {
                render = RenderPolicy::Request;
                continue;
            }
            ("activate", "idle") => {
                activate = ActivatePolicy::Idle;
                continue;
            }
            ("activate", "never") => {
                activate = ActivatePolicy::Never;
                continue;
            }
            ("activate", "interaction") => {
                activate = ActivatePolicy::Interaction;
                continue;
            }
            ("paint", "settled") => {
                paint = PaintPolicy::Settled;
                continue;
            }
            ("paint", "boot") => {
                paint = PaintPolicy::Boot;
                continue;
            }
            // Its source call is types' and `declare_routes`'.
            ("pages", _) => continue,
            ("render", _) => "`render` is a word: client, build, cached or request".to_owned(),
            ("activate", _) => "`activate` is a word: idle, never or interaction".to_owned(),
            ("paint", _) => "`paint` is a word: settled or boot".to_owned(),
            (other, _) => format!(
                "a route has no field `{other}`: it takes `render=`, `activate=`, `paint=` and `pages=`"
            ),
        };
        return err(
            "lower-route-field",
            format!("{refusal} (route `{}`)", row.name),
            field.span,
        );
    }
    // A parameterized route renders at build only the pages `pages=` lists
    // (LLP 1048.000 D2); `pages=` names pages a build or a server renders.
    let listed = row.fields.iter().any(|f| f.name == "pages");
    if render == RenderPolicy::Build
        && !listed
        && row.pattern.split('/').any(|s| s.starts_with(':'))
    {
        return err(
            "lower-route-field",
            format!(
                "route `{}` has parameters, so it renders at build only the pages `pages=source()` lists",
                row.name
            ),
            row.span,
        );
    }
    if listed && render == RenderPolicy::Client {
        return err(
            "lower-route-field",
            format!(
                "route `{}` renders on the client, so it has no pages to list: `pages=` goes with `render=build`, `cached` or `request`",
                row.name
            ),
            row.span,
        );
    }
    // A boot document is sent while a served page waits on its answers; a
    // page made at build, or on the client, waits on nothing a reader sees.
    if paint == PaintPolicy::Boot && !matches!(render, RenderPolicy::Cached | RenderPolicy::Request)
    {
        return err(
            "lower-route-field",
            format!(
                "route `{}` is not rendered per request, so it has no boot document to paint first: `paint=boot` goes with `render=cached` or `request`",
                row.name
            ),
            row.span,
        );
    }
    Ok((render, activate, paint))
}

/// Where an element sits relative to a navigation root (LLP 1038 D6, LLP
/// 1075.003 §3.7). Every host finds a root's routes among its own children
/// and among its tabpanels' children, and nowhere else: a route behind a
/// wrapper was ignored, with only a runtime log naming its key (hn-reader
/// F5, shop F9), so the compiler refuses it.
#[derive(Clone, Debug, Default)]
pub(crate) enum NavPlace {
    /// No navigation root encloses it.
    #[default]
    Outside,
    /// A root's child: the tags from the root down.
    RootChild(Vec<String>),
    /// A child of a root that holds tabs, whose routes are its tabpanels'
    /// children only (LLP 1075.003 §3.7).
    TabbedRootChild(Vec<String>),
    /// A tabpanel's child, the panel in a root.
    PanelChild(Vec<String>),
    /// Deeper inside a root, behind a wrapper, outside any route.
    InRoot(Vec<String>),
    /// Inside a route.
    InRoute,
}

/// @ref LLP 1116.006 D2 — the attribute marking a box as the split's own placeholder, written as
/// a literal: a wide window's split shows it in the detail pane while nothing is selected, and
/// it is no box anywhere else (its `display` is `none`, which the web shell overrides there).
pub(crate) fn split_placeholder(attrs: &[Attr]) -> Option<&Attr> {
    attrs.iter().find(|a| {
        a.name == "navigationSplit" && matches!(&a.value, Expr::Str(s, _) if s == "placeholder")
    })
}

impl NavPlace {
    /// @ref LLP 1116 D6, LLP 1115 D5 — the first root carrying a
    /// `navigationKey` is the navigation root with or without
    /// `navigationBack`. One that names no Back control is given the empty
    /// name, which no element's `id` carries: every host and the runner
    /// find the root by that attribute, and the platform's Back from a
    /// route is then the root's `navigate`, else the router's own `back`.
    pub(crate) fn unnamed(
        &self,
        first_root: bool,
        attrs: &[Attr],
        span: Span,
    ) -> Option<Vec<Attr>> {
        let has = |name: &str| attrs.iter().any(|a| a.name == name);
        (first_root
            && matches!(self, NavPlace::Outside)
            && has("navigationKey")
            && !has("navigationBack"))
        .then(|| {
            let mut attrs = attrs.to_vec();
            attrs.push(Attr {
                name: "navigationBack".into(),
                value: Expr::Str(String::new(), span),
                span,
                written: crate::IMPLIED,
            });
            attrs
        })
    }

    /// @ref LLP 1116.007 D5 — a route whose scroll is a grouped list is on
    /// the grouped background, as a hand-built `UITableViewController`'s
    /// screen is, bar to bottom edge: its unsaid `background-color` is the
    /// role `-exact-grouped-background`, so what sits between the `header`
    /// and the list (a segmented switcher, a summary) is on the list's grey,
    /// not the page's white. The row is the compiler's, set and not written
    /// (LLP 1115.000 D2), and pushed before the author's, so a
    /// `background-color` on the route (or its class) wins; a box between the
    /// header and the list paints its own, over it. Each host resolves the
    /// role its own way (`systemGroupedBackground` on iOS). `None` where this
    /// is no route, or the route's scroll is anything else or cannot be told.
    pub(crate) fn grouped_page(
        &self,
        tag: &str,
        attrs: &[Attr],
        children: &[Node],
        span: Span,
    ) -> Option<Vec<Attr>> {
        let has = |name: &str| attrs.iter().any(|a| a.name == name);
        if !self.is_route(attrs)
            || has("background-color")
            || !scrolls_a_grouped_list(tag, attrs, children)
        {
            return None;
        }
        let role = Attr {
            name: "background-color".into(),
            value: Expr::Str("-exact-grouped-background".into(), span),
            span,
            written: crate::IMPLIED,
        };
        Some([&[role][..], attrs].concat())
    }

    /// @ref LLP 1116.009 D3 — a sheet is never transparent: a route that
    /// `navigationPresentation="modal"` presents is on the page's own ground,
    /// `-exact-background`, unless its author said otherwise. A hand-built
    /// UIKit sheet is never see-through: it has the system background, or
    /// below its large detent UIKit's glass, to which the iOS host leaves
    /// this ground (LLP 1141.001 D3); a web or Mac host
    /// keeps the covered route in view, so a form whose author wrote no
    /// background was see-through there (the bake-off's habit-tracker, rounds
    /// 10 and 11). The row is the compiler's, set and not written (LLP
    /// 1115.000 D2) and pushed before the author's, as `grouped_page`'s is,
    /// so a `background-color` on the route (or its class) wins; and it runs
    /// after it, so a sheet that scrolls a grouped list is on that grey. A
    /// presentation that is computed is not told, and `None` is a route that
    /// is not a sheet, or already says its ground.
    pub(crate) fn sheet_page(&self, attrs: &[Attr], span: Span) -> Option<Vec<Attr>> {
        let has = |name: &str| attrs.iter().any(|a| a.name == name);
        let route = self.is_route(attrs);
        let modal = attrs
            .iter()
            .rev()
            .find(|a| a.name == "navigationPresentation");
        if !route
            || has("background-color")
            || !matches!(modal.map(|a| &a.value), Some(Expr::Str(s, _)) if s == "modal")
        {
            return None;
        }
        let role = Attr {
            name: "background-color".into(),
            value: Expr::Str("-exact-background".into(), span),
            span,
            written: crate::IMPLIED,
        };
        Some([&[role][..], attrs].concat())
    }

    /// Whether an element here with `attrs` is a route: a child of the
    /// navigation root or of a `role="tabpanel"` in it, with a `navigationKey`.
    fn is_route(&self, attrs: &[Attr]) -> bool {
        let has = |name: &str| attrs.iter().any(|a| a.name == name);
        matches!(self, NavPlace::RootChild(_) | NavPlace::PanelChild(_))
            && has("navigationKey")
            && !has("navigationBack")
    }

    /// The place of `tag`'s children, or the refusal of a route `tag` that
    /// no host would find here.
    pub(crate) fn enter(
        &self,
        tag: &str,
        attrs: &[Attr],
        children: &[Node],
        span: Span,
    ) -> Result<NavPlace, LowerError> {
        let has = |name: &str| attrs.iter().any(|a| a.name == name);
        let path = |p: &[String]| [p, &[tag.to_string()]].concat();
        if let Some(a) = attrs.iter().rev().find(|a| a.name == "-exact-safe-area") {
            if !self.is_route(attrs) {
                return err(
                    "lower-attr-value",
                    "`-exact-safe-area` belongs on a content owner's authored root (a route with a `navigationKey`), not the session root or a descendant",
                    a.span,
                );
            }
            // A computed value the host reads; an invalid one is `auto` there, journalled once.
            let mut literals = Vec::new();
            crate::theme::literal_leaves(&a.value, &mut literals);
            for (word, span) in literals {
                let edges: Vec<_> = word.split_whitespace().collect();
                if word != "auto"
                    && word != "none"
                    && (edges.is_empty()
                        || edges.iter().enumerate().any(|(i, edge)| {
                            !matches!(*edge, "top" | "right" | "bottom" | "left")
                                || edges[..i].contains(edge)
                        }))
                {
                    return err(
                        "lower-attr-value",
                        "`-exact-safe-area` takes \"auto\", \"none\", or a space-separated list of top, right, bottom and left without duplicates",
                        span,
                    );
                }
            }
        }
        // @ref LLP 1127 — an inspector is a sheet a wide split shows beside its
        // detail, so it is said on a sheet route.
        if let Some(a) = attrs.iter().rev().find(|a| a.name == "navigationInspector") {
            // A bound presentation may be a sheet; a literal one must be.
            let sheet = match attrs
                .iter()
                .rev()
                .find(|a| a.name == "navigationPresentation")
                .map(|a| &a.value)
            {
                Some(Expr::Str(word, _)) => word == "modal",
                Some(_) => true,
                None => false,
            };
            if !self.is_route(attrs) || !sheet {
                return err(
                    "lower-attr-value",
                    "`navigationInspector` belongs on a sheet route (a route with `navigationPresentation=\"modal\"`): an expanded split on iOS shows that sheet as its inspector column",
                    a.span,
                );
            }
        }
        // @ref LLP 1115.000 D5.2 — the bar is each route's, so `navigationBar`
        // is a route's word: on the root or on a box inside a screen no host
        // reads it, and the screen would keep the bar it meant to drop.
        if has("navigationBar") && (!has("navigationKey") || has("navigationBack")) {
            let on = if has("navigationKey") {
                "the navigation root"
            } else {
                "an element with no `navigationKey`"
            };
            return err(
                "lower-route-bar",
                format!("`navigationBar` is on {on}, where no host reads it: it belongs to a route (an element with a `navigationKey`, a child of the navigation root or of a `role=\"tabpanel\"` in it) and says whether that one screen shows the platform's navigation bar. Move it onto each route that draws its own chrome (docs/recipes/navigation-stack.md)"),
                attrs
                    .iter()
                    .find(|a| a.name == "navigationBar")
                    .map_or(span, |a| a.span),
            );
        }
        // A sheet opens at one of its own heights, which its route says; a
        // list the compiler cannot read is given the benefit of the doubt
        // (iOS journals a miss).
        if let Some(a) = attrs
            .iter()
            .rev()
            .find(|a| a.name == "navigationInitialDetent")
        {
            if !self.is_route(attrs) {
                return err(
                    "lower-attr-value",
                    "`navigationInitialDetent` is read only on a route (an element with a `navigationKey`, a child of the navigation root or of a `role=\"tabpanel\"` in it), beside its `navigationDetent`: move both onto the sheet's route",
                    a.span,
                );
            }
            // Empty, as `navigationDetent=""` is, it is unsaid: the first.
            if matches!(&a.value, Expr::Str(word, _) if !word.is_empty() && !crate::vocab::detent_word(word))
            {
                return err(
                    "lower-attr-value",
                    "`navigationInitialDetent` takes one word of the route's `navigationDetent`, the height its sheet opens at",
                    a.span,
                );
            }
            let detents = attrs.iter().rev().find(|a| a.name == "navigationDetent");
            // The words, and whether the route wrote them.
            let heights = match detents.map(|a| &a.value) {
                Some(Expr::Str(s, _)) if !s.trim().is_empty() => Some((s.as_str(), true)),
                Some(Expr::Str(..)) | None => Some(("large", false)),
                Some(_) => None,
            };
            if let (Expr::Str(word, _), Some((heights, written))) = (&a.value, heights) {
                if !word.is_empty() && !heights.split(' ').any(|w| w == word) {
                    let said = if written {
                        format!("`navigationDetent=\"{heights}\"`")
                    } else {
                        "`large` alone, as the route says no `navigationDetent`".into()
                    };
                    return err(
                        "lower-attr-value",
                        format!("`navigationInitialDetent=\"{word}\"` is not one of this sheet's heights, {said}; it names the one of them the sheet opens at"),
                        a.span,
                    );
                }
            }
        }
        // @ref LLP 1116.006 D2 — the split's placeholder is a box in the root route; a route
        // marked so would never show.
        if let Some(a) = split_placeholder(attrs).filter(|_| has("navigationKey")) {
            return err(
                "lower-attr-value",
                "`navigationSplit=\"placeholder\"` marks a box inside the stack's root route, which a wide window shows in the detail pane while nothing is selected; on a route it would hide the route. Put it on a box in the route (docs/recipes/navigation-stack.md)",
                a.span,
            );
        }
        if has("navigationKey") && has("navigationBack") {
            let path = vec![tag.to_string()];
            return Ok(if holds_tabs(children) {
                NavPlace::TabbedRootChild(path)
            } else {
                NavPlace::RootChild(path)
            });
        }
        if has("navigationKey") {
            let shown = match self {
                NavPlace::InRoot(p) => p.join(" > "),
                // @ref LLP 1116.020 D2 — round 18's contacts took its add sheet out of the tab's
                // stack and drew it beside the tabs: no host presented it, and its header sat
                // under the status bar with Save and Cancel dead.
                NavPlace::TabbedRootChild(_) => {
                    let modal = attrs.iter().rev().find(|a| a.name == "navigationPresentation");
                    let sheet = if modal.is_some() {
                        " (a `navigationPresentation=\"modal\"` here is no sheet: it is drawn over the page, its header under the status bar)"
                    } else {
                        ""
                    };
                    return err(
                        "lower-route-place",
                        format!("this `{tag}` has a `navigationKey` beside the root's tabpanels; with tabs every host finds routes only among a tabpanel's children, so this one is never a route{sheet}. Render it in the tab's stack, in the panel's `for e in t.stack` with the other routes and not filtered out of it: a modal route there is presented as a sheet over the tabs (docs/contract-for-agents.md, \"Tabs and stacks\")"),
                        modal.map_or(span, |a| a.span),
                    );
                }
                NavPlace::InRoute => {
                    return err(
                        "lower-route-place",
                        format!("this `{tag}` has a `navigationKey` inside another route; a route is a child of the navigation root or of a `role=\"tabpanel\"` in it, never of a route, so no host would show it"),
                        span,
                    )
                }
                NavPlace::Outside => return Ok(NavPlace::Outside),
                _ => {
                    check_scroll(tag, attrs, children)?;
                    return Ok(NavPlace::InRoute);
                }
            };
            return err(
                "lower-route-place",
                format!("this `{tag}` has a `navigationKey` but sits at `{shown} > {tag}`; the hosts find a route only as a child of the navigation root (the first root with a `navigationKey`) or of a `role=\"tabpanel\"` in it, so this one is never shown. Move the wrapper's layout onto the route or inside it (docs/contract-for-agents.md, \"Tabs and stacks\")"),
                span,
            );
        }
        // A dynamic `role` may be a tabpanel: its children are given the benefit.
        let panel = attrs
            .iter()
            .rev()
            .find(|a| a.name == "role")
            .is_some_and(|a| match &a.value {
                Expr::Str(v, _) => v == "tabpanel",
                _ => true,
            });
        Ok(match self {
            NavPlace::RootChild(p) | NavPlace::TabbedRootChild(p) | NavPlace::InRoot(p)
                if panel =>
            {
                NavPlace::PanelChild(path(p))
            }
            NavPlace::RootChild(p)
            | NavPlace::TabbedRootChild(p)
            | NavPlace::PanelChild(p)
            | NavPlace::InRoot(p) => NavPlace::InRoot(path(p)),
            other => other.clone(),
        })
    }
}

/// Whether a navigation root's own subtree, outside its routes, holds a
/// literal `tablist` and `tabpanel`: its routes are then its tabpanels'
/// children, never its own (`host/web/navigation.js` `stacksOf`, the iOS and
/// Mac hosts' `NavigationTabs`, the runner's `tab_panels`). What the compiler
/// cannot see (a computed role, a component's use) is not counted.
fn holds_tabs(children: &[Node]) -> bool {
    let (mut list, mut panel) = (false, false);
    let mut stack: Vec<&Node> = children.iter().collect();
    while let Some(node) = stack.pop() {
        match node {
            Node::Element {
                attrs, children, ..
            } => {
                if attrs.iter().any(|a| a.name == "navigationKey") {
                    continue;
                }
                match attrs
                    .iter()
                    .rev()
                    .find(|a| a.name == "role")
                    .map(|a| &a.value)
                {
                    Some(Expr::Str(r, _)) if r == "tablist" => list = true,
                    Some(Expr::Str(r, _)) if r == "tabpanel" => panel = true,
                    _ => {}
                }
                stack.extend(children);
            }
            Node::When {
                then, otherwise, ..
            } => stack.extend(then.iter().chain(otherwise)),
            Node::Each { body, .. } => stack.extend(body),
            Node::Match { some, none, .. } => stack.extend(some.1.iter().chain(none)),
            Node::Use { .. } | Node::Children { .. } => {}
        }
    }
    list && panel
}

/// A route's literal `navigationScroll` names its content scroller by HTML
/// id (LLP 1075.003 §3.5), resolved inside the route as `navigationBack` is.
/// Refused only where that provably fails: no element of the route can carry
/// the id, or every one that does is a plain box that never scrolls on y.
/// Anything the compiler cannot see through — a computed id or overflow, a
/// class, an unexpanded use — is given the benefit.
fn check_scroll(tag: &str, attrs: &[Attr], children: &[Node]) -> Result<(), LowerError> {
    let Some(named) = attrs.iter().rev().find(|a| a.name == "navigationScroll") else {
        return Ok(());
    };
    let Expr::Str(name, _) = &named.value else {
        return Ok(());
    };
    if name.is_empty() {
        return Ok(());
    }
    let mut carriers: Vec<(&str, &[Attr])> = Vec::new();
    let mut doubt = false;
    // The route itself is inside the route, as the hosts resolve it.
    let mut elements = vec![(tag, attrs)];
    let mut stack: Vec<&Node> = children.iter().collect();
    while let Some(node) = stack.pop() {
        match node {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                elements.push((tag, attrs));
                stack.extend(children);
            }
            Node::When {
                then, otherwise, ..
            } => stack.extend(then.iter().chain(otherwise)),
            Node::Each { body, .. } => stack.extend(body),
            Node::Match { some, none, .. } => stack.extend(some.1.iter().chain(none)),
            Node::Use { .. } | Node::Children { .. } => doubt = true,
        }
    }
    for (tag, attrs) in elements {
        match attrs
            .iter()
            .rev()
            .find(|a| a.name == "id")
            .map(|a| &a.value)
        {
            Some(Expr::Str(id, _)) if id == name => carriers.push((tag, attrs)),
            Some(Expr::Str(..)) | None => {}
            Some(_) => doubt = true,
        }
    }
    if carriers.is_empty() && !doubt {
        return err(
            "lower-route-scroll",
            format!("this route's `navigationScroll=\"{name}\"` names no element: nothing in the route has `id=\"{name}\"`, so no host finds its content scroller (on iOS the large title stays still and nothing scrolls under the bar). Give the route's scroller, a `scroll` or `list` right after its `header`, `id=\"{name}\"`, or remove `navigationScroll` (docs/contract-for-agents.md, \"Routes and web documents\")"),
            named.span,
        );
    }
    match carriers.as_slice() {
        // A computed id elsewhere may name a scroller in another branch.
        [(tag, attrs), ..] if !doubt && carriers.iter().all(|(t, a)| still_on_y(t, a)) => err(
            "lower-route-scroll",
            format!("this route's `navigationScroll=\"{name}\"` names a `{tag}` that never scrolls on y: its overflow-y is visible or hidden, so it grows with or clips its content and the host has no scroller to follow (on iOS the large title stays still). Make `id=\"{name}\"` a `scroll` or `list` with `flex=1 min-height=0`, or give it `overflow-y=\"auto\"` and a bounded height (docs/contract-for-agents.md, \"Routes and web documents\")"),
            attrs.iter().find(|a| a.name == "id").map_or(named.span, |a| a.span),
        ),
        _ => Ok(()),
    }
}

/// The tags that are plain boxes: they scroll only where an `overflow` row says so.
const PLAIN: &[&str] = &[
    "view", "box", "column", "row", "main", "header", "nav", "section", "footer", "article",
    "aside",
];

/// Whether a box provably never scrolls on y: a plain box, or a `scroll` or
/// `list` (whose block axis scrolls unless a row says otherwise), whose
/// literal overflow rows leave y visible or hidden, CSS's computation
/// included (a visible y beside a non-visible x computes to auto). A class,
/// a style or a computed row may say otherwise, so they are doubt.
fn still_on_y(tag: &str, attrs: &[Attr]) -> bool {
    let scrolls = matches!(tag, "scroll" | "list");
    if !scrolls && !PLAIN.contains(&tag) {
        return false;
    }
    let doubt = |a: &Attr| {
        matches!(a.name.as_str(), "class" | "className" | "style") || a.name.starts_with("ua:")
    };
    if attrs.iter().any(doubt) {
        return false;
    }
    let y = if scrolls { "scroll" } else { "visible" };
    crate::overflow::Overflow::of(attrs, y).scrolls_y() == Some(false)
}

/// Whether a route's content scroller is a grouped list (`list
/// appearance="auto"`, `listStyle` `inset-grouped` or `grouped`: the two that
/// draw the grouped grey), resolved as the hosts resolve it (LLP 1115.000
/// D11.1): the element a literal `navigationScroll` names by `id`, else the
/// shallowest one that scrolls on y, eight levels down at most, in tree
/// order at each depth. False where the compiler cannot tell (a computed
/// name or id, a class or a computed row on a box that may scroll): the
/// platform's default is then not supplied, as a doubt is never the author's
/// loss.
fn scrolls_a_grouped_list(tag: &str, attrs: &[Attr], children: &[Node]) -> bool {
    type El<'a> = (&'a str, &'a [Attr], &'a [Node]);
    // The elements `nodes` hold, through `if`, `for` and `match` but not
    // through an element, in order.
    fn level<'a>(nodes: &'a [Node], out: &mut Vec<El<'a>>) {
        for node in nodes {
            match node {
                Node::Element {
                    tag,
                    attrs,
                    children,
                    ..
                } => out.push((tag, attrs, children)),
                Node::When {
                    then, otherwise, ..
                } => {
                    level(then, out);
                    level(otherwise, out);
                }
                Node::Each { body, .. } => level(body, out),
                Node::Match { some, none, .. } => {
                    level(&some.1, out);
                    level(none, out);
                }
                Node::Use { .. } | Node::Children { .. } => {}
            }
        }
    }
    let grouped = |(tag, attrs, _): &El| {
        matches!(
            crate::grouped::style(tag, attrs),
            Ok(Some("inset-grouped" | "grouped"))
        )
    };
    let named = attrs.iter().rev().find(|a| a.name == "navigationScroll");
    let named = match named.map(|a| &a.value) {
        None => None,
        Some(Expr::Str(name, _)) if name.is_empty() => None,
        Some(Expr::Str(name, _)) => Some(name),
        Some(_) => return false,
    };
    if let Some(name) = named {
        // Every element of the route: the carriers of the id must all be
        // grouped lists, and a computed id may name another.
        let (mut carriers, mut all, mut stack) = (Vec::new(), Vec::new(), Vec::new());
        level(children, &mut stack);
        while let Some(el) = stack.pop() {
            all.push(el);
            level(el.2, &mut stack);
        }
        for el in all {
            match el.1.iter().rev().find(|a| a.name == "id").map(|a| &a.value) {
                Some(Expr::Str(id, _)) if id == name => carriers.push(el),
                Some(Expr::Str(..)) | None => {}
                Some(_) => return false,
            }
        }
        return !carriers.is_empty() && carriers.iter().all(grouped);
    }
    // The route itself comes first, as the hosts' search starts at it.
    let mut row: Vec<El> = vec![(tag, attrs, children)];
    for _ in 0..8 {
        for el in &row {
            match scrolls_y(el.0, el.1) {
                Some(false) => {}
                Some(true) => return grouped(el),
                None => return false,
            }
        }
        let mut next = Vec::new();
        for el in &row {
            level(el.2, &mut next);
        }
        if next.is_empty() {
            break;
        }
        row = next;
    }
    false
}

/// Whether a box scrolls on y by its literal rows, `None` where a class, a
/// style or a computed row may say otherwise: a `scroll` or `list` unless an
/// overflow row stills it, a plain box only by `overflow` `auto` or `scroll`.
fn scrolls_y(tag: &str, attrs: &[Attr]) -> Option<bool> {
    let scroller = matches!(tag, "scroll" | "list");
    if !scroller && !PLAIN.contains(&tag) {
        return Some(false);
    }
    for a in attrs {
        match a.name.as_str() {
            "class" | "style" => return None,
            "flex-direction" if scroller => return None,
            name if name.starts_with("ua:") => return None,
            _ => {}
        }
    }
    let y = if scroller { "scroll" } else { "visible" };
    crate::overflow::Overflow::of(attrs, y).scrolls_y()
}
