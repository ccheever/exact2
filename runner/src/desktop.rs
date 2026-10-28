//! A button's presentations past the window (LLP 1133.025 D4, D5): a
//! `-exact-menu="status"` button is the app's status item on macOS and
//! Windows, and one with `dock` among its ids an item of the Dock menu.
//! The agent reads both from the session's tree on every host, as
//! `state.statusItems` and `state.dockMenu`, so a drive sees what a Mac or
//! Windows host would place, and the buttons stay ordinary where no host
//! places them.

use crate::{DataSource, Runner};
use exact_kernel::{Display, Kernel, NodeType, PropId, PropList, ViewId};
use serde_json::{json, Value as Json};

/// Whether `props`' `-exact-menu` names `id` among its ids.
pub fn presents(props: &PropList, id: &str) -> bool {
    (props.str(PropId::MenuPlacement)).is_some_and(|p| p.split_whitespace().any(|w| w == id))
}

/// The buttons presented as `id` (`status` or `dock`), in tree order, that
/// are mounted and not `display: none` (a `hidden` button places nothing).
pub fn buttons<D: DataSource>(runner: &Runner<D>, id: &str) -> Vec<ViewId> {
    let kernel = runner.kernel();
    if !kernel.has_prop(PropId::MenuPlacement) {
        return Vec::new();
    }
    kernel
        .preorder_where(&kernel.roots(), |_, props| presents(props, id))
        .into_iter()
        .filter(|id| shown(kernel, *id))
        .collect()
}

fn shown(kernel: &Kernel, id: ViewId) -> bool {
    let mut at = Some(id);
    while let Some(id) = at {
        let Some(node) = kernel.node(id) else {
            return false;
        };
        if node.style.display == Display::None {
            return false;
        }
        at = node.parent;
    }
    true
}

/// A button's label: its `aria-label`, else its text.
fn label(kernel: &Kernel, id: ViewId) -> String {
    let Some(node) = kernel.node(id) else {
        return String::new();
    };
    if let Some(label) = node.props.str(PropId::AccessibilityLabel) {
        return label.to_owned();
    }
    let mut text = String::new();
    for d in kernel.preorder_where(&[id], |t, _| t == NodeType::Text) {
        if let Some(n) = kernel.node(d) {
            text.extend(n.read_runs().iter().map(|r| r.to_string()));
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The image of a status item: its `img` child's source, or `svg` for a
/// drawn one.
fn image(kernel: &Kernel, id: ViewId) -> Json {
    let found = kernel.preorder_where(&[id], |t, _| matches!(t, NodeType::Image | NodeType::Svg));
    let Some(node) = found.first().and_then(|d| kernel.node(*d)) else {
        return Json::Null;
    };
    match node.props.str(PropId::ImageSource) {
        Some(src) => json!(src),
        None => json!("svg"),
    }
}

/// The rows of the menu-shaped popover `button`'s `popovertarget` names
/// (LLP 1021 D1): each button's label, `disabled`, `aria-checked` and
/// `testId`, and `"—"` for an `hr`; `None` when it names none.
fn menu(kernel: &Kernel, button: ViewId) -> Option<Vec<Json>> {
    let target = kernel
        .node(button)?
        .props
        .str(PropId::Popovertarget)?
        .to_owned();
    let popover = kernel
        .preorder_where(&kernel.roots(), |_, p| {
            p.str(PropId::Id) == Some(target.as_str()) && p.str(PropId::Popover).is_some()
        })
        .into_iter()
        .next()?;
    let rows = kernel.preorder_where(&[popover], |t, p| {
        is_button(t, p) || p.str(PropId::SemanticTag) == Some("hr")
    });
    Some(
        rows.into_iter()
            .filter(|id| shown(kernel, *id))
            .filter_map(|id| {
                let node = kernel.node(id)?;
                if node.props.str(PropId::SemanticTag) == Some("hr") {
                    return Some(json!("—"));
                }
                Some(item(kernel, id))
            })
            .collect(),
    )
}

/// Whether a node is a `button`: a pressable, or the control a `button`
/// element lowers to.
fn is_button(t: NodeType, props: &PropList) -> bool {
    t == NodeType::Pressable
        || (t == NodeType::Control && props.str(PropId::Type) == Some("button"))
}

/// A button as a menu shows it.
fn item(kernel: &Kernel, id: ViewId) -> Json {
    let node = kernel.node(id);
    let props = node.as_ref().map(|n| n.props);
    json!({
        "label": label(kernel, id),
        "disabled": props.and_then(|p| p.bool(PropId::Disabled)) == Some(true),
        "checked": props.and_then(|p| p.str(PropId::AccessibilityChecked)),
        "testId": props.and_then(|p| p.str(PropId::TestId)),
        "id": id,
    })
}

/// `state.statusItems`: each status item the host would place, with its
/// menu, the host's quit item ending an accessory app's (D7) left to it.
pub fn status_items<D: DataSource>(runner: &Runner<D>) -> Vec<Json> {
    let kernel = runner.kernel();
    buttons(runner, "status")
        .into_iter()
        .map(|id| {
            let mut v = item(kernel, id);
            v["image"] = image(kernel, id);
            v["menu"] = menu(kernel, id).map_or(Json::Null, Json::from);
            v
        })
        .collect()
}

/// `state.dockMenu`: the Dock menu's items, in declaration order.
pub fn dock_menu<D: DataSource>(runner: &Runner<D>) -> Vec<Json> {
    let kernel = runner.kernel();
    (buttons(runner, "dock").into_iter())
        .map(|id| item(kernel, id))
        .collect()
}

/// `,"statusItems":[…],"dockMenu":[…]`, the agent's state.
pub(crate) fn state<D: DataSource>(runner: &Runner<D>, s: &mut String) {
    s.push_str(",\"statusItems\":");
    s.push_str(&Json::from(status_items(runner)).to_string());
    s.push_str(",\"dockMenu\":");
    s.push_str(&Json::from(dock_menu(runner)).to_string());
}
