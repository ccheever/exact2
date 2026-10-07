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
    /// A tabpanel's child, the panel in a root.
    PanelChild(Vec<String>),
    /// Deeper inside a root, behind a wrapper, outside any route.
    InRoot(Vec<String>),
    /// Inside a route.
    InRoute,
}

impl NavPlace {
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
        if has("navigationKey") && has("navigationBack") {
            return Ok(NavPlace::RootChild(vec![tag.to_string()]));
        }
        if has("navigationKey") {
            let shown = match self {
                NavPlace::InRoot(p) => p.join(" > "),
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
                format!("this `{tag}` has a `navigationKey` but sits at `{shown} > {tag}`; the hosts find a route only as a child of the navigation root (the element with `navigationBack`) or of a `role=\"tabpanel\"` in it, so this one is never shown. Move the wrapper's layout onto the route or inside it (docs/contract-for-agents.md, \"Tabs and stacks\")"),
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
            NavPlace::RootChild(p) | NavPlace::InRoot(p) if panel => NavPlace::PanelChild(path(p)),
            NavPlace::RootChild(p) | NavPlace::PanelChild(p) | NavPlace::InRoot(p) => {
                NavPlace::InRoot(path(p))
            }
            other => other.clone(),
        })
    }
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

/// Whether a box provably never scrolls on y: a plain box, or a `scroll` or
/// `list` (whose block axis scrolls unless a row says otherwise), whose
/// literal overflow rows leave y visible or hidden, CSS's computation
/// included (a visible y beside a non-visible x computes to auto). A class,
/// a style or a computed row may say otherwise, so they are doubt.
fn still_on_y(tag: &str, attrs: &[Attr]) -> bool {
    const PLAIN: &[&str] = &[
        "view", "box", "column", "row", "main", "header", "nav", "section", "footer", "article",
        "aside",
    ];
    let scrolls = matches!(tag, "scroll" | "list");
    if !scrolls && !PLAIN.contains(&tag) {
        return false;
    }
    let (mut x, mut y) = ("visible", if scrolls { "scroll" } else { "visible" });
    for a in attrs {
        let name = a.name.as_str();
        if matches!(name, "class" | "className" | "style") || name.starts_with("ua:") {
            return false;
        }
        let (to_x, to_y) = match name {
            "overflow" => (true, true),
            "overflow-x" | "overflowX" => (true, false),
            "overflow-y" | "overflowY" => (false, true),
            _ => continue,
        };
        let Expr::Str(v, _) = &a.value else {
            return false;
        };
        let v = match v.as_str() {
            "visible" => "visible",
            "hidden" | "clip" => "hidden",
            _ => "scroll",
        };
        if to_x {
            x = v;
        }
        if to_y {
            y = v;
        }
    }
    y == "hidden" || (y == "visible" && x == "visible")
}
