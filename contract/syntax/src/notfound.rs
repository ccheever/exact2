//! @ref LLP 1116.009 D2 — the `notfound` route and screen the compiler writes
//! for an app that declares none.
//!
//! A location no route matches then keeps its URL: the router value is valid
//! (the not-found entry is a stack entry like any other), and the screen the
//! app draws for that entry is "Page not found" with a way to the first
//! screen. Both the route and the screen are ordinary Contract, written here
//! as an author would, so the runner, the web JS target and the semantics
//! need to know nothing of them.
//!
//! Where the screen goes is the app's own structure: the `each` over a stack
//! that draws a route per entry. Its body is wrapped, `when entry.name ==
//! "notfound"` the screen, `else` what the author wrote, so an app whose route
//! element stands outside any `when` (a blank screen for an entry no branch
//! names) and one whose every route is in a `when` are served alike, and a
//! tab's stack as the root's.

use crate::ast::*;
use crate::{Span, VisitSpans};

/// Whether the compiler writes the `notfound` route and its screen for this
/// app: it declares `routes` with no `notfound` line, and its view opens on a
/// navigation root (the first node, an element with a `navigationKey`, which
/// every host finds the routes under) that draws a route per stack entry (an
/// `each` over a stack, `each e in stack(nav)` or `t.stack`). An app that
/// writes `notfound` draws its own, and one with no navigation root has no
/// screen to draw one in. The first tab's root route has no parameter, so the
/// way to it is a location ([`root_pattern`]).
pub fn implicit_notfound(file: &File) -> bool {
    let Some(routes) = &file.routes else {
        return false;
    };
    !routes.rows.iter().any(|r| r.notfound)
        && root_pattern(routes).is_some_and(|p| !p.contains(':'))
        && matches!(
            file.components.first().and_then(|c| c.view.first()),
            Some(Node::Element { attrs, children, .. })
                if attrs.iter().any(|a| a.name == "navigationKey")
                    && children.iter().any(|n| holds_route_each(n, &routes.slot))
        )
}

/// The first tab's root route's pattern: the first `tab` row, else the first.
fn root_pattern(routes: &RoutesDecl) -> Option<&str> {
    let row = routes.rows.iter().find(|r| r.tab).or(routes.rows.first())?;
    Some(row.pattern.as_str())
}

/// An `each` that draws a route per stack entry: over `stack(<the router>)`
/// or a tab's `.stack`, its body holding (directly, or in a `when` or `match`
/// arm) an element with a `navigationKey`.
fn is_route_each(node: &Node, slot: &str) -> bool {
    let Node::Each { list, body, .. } = node else {
        return false;
    };
    let stack = match list {
        Expr::Call(f, args, _) => {
            f == "stack" && matches!(args.as_slice(), [Expr::Ident(name, _)] if name == slot)
        }
        Expr::Member(_, field, _) => field == "stack",
        _ => false,
    };
    stack && holds_route(body)
}

fn holds_route(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Element { attrs, .. } => attrs.iter().any(|a| a.name == "navigationKey"),
        Node::When {
            then, otherwise, ..
        } => holds_route(then) || holds_route(otherwise),
        Node::Match { some, none, .. } => holds_route(&some.1) || holds_route(none),
        _ => false,
    })
}

/// Whether `node`, or something inside it that is not a use, is a route `each`.
fn holds_route_each(node: &Node, slot: &str) -> bool {
    is_route_each(node, slot)
        || match node {
            Node::Element { children, .. } => children.iter().any(|n| holds_route_each(n, slot)),
            Node::Each { body, .. } => body.iter().any(|n| holds_route_each(n, slot)),
            Node::When {
                then, otherwise, ..
            } => then
                .iter()
                .chain(otherwise)
                .any(|n| holds_route_each(n, slot)),
            Node::Match { some, none, .. } => {
                some.1.iter().chain(none).any(|n| holds_route_each(n, slot))
            }
            Node::Use { .. } | Node::Children { .. } => false,
        }
}

/// The root component's view with every route `each` of the navigation root
/// wrapped (see [`implicit_notfound`]).
pub(crate) fn implicit_notfound_view(routes: &RoutesDecl, view: &[Node]) -> Option<Vec<Node>> {
    let mut view = view.to_vec();
    let Some(Node::Element { children, .. }) = view.first_mut() else {
        return None;
    };
    wrap(children, routes).then_some(view)
}

/// Wrap each route `each` in `nodes`; whether any was.
fn wrap(nodes: &mut [Node], routes: &RoutesDecl) -> bool {
    let mut wrapped = false;
    for node in nodes {
        if is_route_each(node, &routes.slot) {
            let Node::Each { var, body, .. } = node else {
                continue;
            };
            if let Some(mut screen) = screen(routes, var) {
                if let Node::When { otherwise, .. } = &mut screen {
                    *otherwise = std::mem::take(body);
                }
                *body = vec![screen];
                wrapped = true;
            }
            continue;
        }
        wrapped |= match node {
            Node::Element { children, .. } => wrap(children, routes),
            Node::Each { body, .. } => wrap(body, routes),
            Node::When {
                then, otherwise, ..
            } => wrap(then, routes) | wrap(otherwise, routes),
            Node::Match { some, none, .. } => wrap(&mut some.1, routes) | wrap(none, routes),
            Node::Use { .. } | Node::Children { .. } => false,
        };
    }
    wrapped
}

/// The screen for the entry bound to `entry`: a `when` with no `else` yet.
/// "Go Home" is a link to the first screen, so it works where the page is a
/// document (a server's not-found page) and goes where any link to a route
/// goes (the root's `navigate`, else the router's own `go`). Read as Contract,
/// as an author would write it, and given the `routes` line's position.
fn screen(routes: &RoutesDecl, entry: &str) -> Option<Node> {
    let home = root_pattern(routes)?;
    let text = format!(
        r#"component NotFound
  view
    when {entry}.name == "notfound"
      column navigationKey=`${{{entry}.id}}` position="absolute" inset=0 display="flex" flex-direction="column" testId="not-found"
        header display="flex" align-items="center" padding="8px 16px"
          text "Not Found" role="heading" aria-level=2
        column flex=1 min-height=0 align-items="center" justify-content="center" gap=8 padding="48px 24px" text-align="center"
          image "symbol:alert" width=48 height=48 object-fit="contain" margin-bottom=8 aria-hidden=true
            -exact-tint-color="-exact-secondary-label"
          text "Page not found" font="-exact-title3" testId="not-found-title"
          text `There is no page at ${{{entry}.url}}.` color="-exact-secondary-label" max-width=360 testId="not-found-message"
          link href="{home}" margin-top=8 testId="not-found-home"
            text "Go Home"
"#
    );
    let mut parsed = crate::parse(&text).ok()?;
    let mut node = parsed.components.pop()?.view.pop()?;
    let at = routes.span;
    node.visit_spans(&mut |s: &mut Span| *s = at);
    Some(node)
}
