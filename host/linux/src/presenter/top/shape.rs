//! What an authored subtree is to the top layer (LLP 1115.001 D10): the
//! shape rule, read from the kernel's tree.
//!
//! **An alert** (LLP 1115 D6) is an `alertdialog`, a `dialog` or a popover,
//! whose rows are explanatory text, actions (a `button` with a `press` that
//! also closes it) and one cancel (a closing `button` with no `press`), with
//! at most three buttons: the rule the Apple hosts follow
//! (`ConfirmationShape.swift`, `AlertsMac.swift`'s `plan`). Its title is the
//! `aria-label`, else its first line; its message is the rest.
//!
//! **A menu** (LLP 1021 D3) is a popover that is no confirmation: each
//! `button` row with a `press` is an item (its title, its symbol, whether it
//! is destructive, disabled or checked), a row that opens another menu is
//! that menu's opener, and any other row is a separator (`hr`), as
//! `MenusIOS.swift`'s `items(of:)` reads it.
//!
//! The rule is written twice, here and in Swift, because the Swift reads
//! its own view mirror. It belongs in shared Rust beside
//! `kernel::grouped_list`, read by both hosts.
use exact_kernel::{ControlKind, Kernel, NodeRef, NodeType, PropId, ViewId};

/// The alert shape: explanatory text, one cancel, and at most three buttons
/// with it (`ConfirmationShape.isAlert`).
pub fn is_alert(texts: usize, actions: usize, cancels: usize) -> bool {
    texts > 0 && cancels == 1 && actions + cancels <= 3
}

/// An alert's words: its title is the label, else its first line; its
/// message is the rest, a line each (`ConfirmationShape.alertText`).
pub fn alert_text(label: Option<&str>, lines: &[String]) -> (Option<String>, Option<String>) {
    let mut rest = lines;
    let title = match label.filter(|l| !l.is_empty()) {
        Some(label) => Some(label.to_string()),
        None => rest.split_first().map(|(first, others)| {
            rest = others;
            first.clone()
        }),
    };
    (title, (!rest.is_empty()).then(|| rest.join("\n")))
}

/// One row a person can choose: an alert's button or a menu's item.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The authored `button`.
    pub node: ViewId,
    /// What it says.
    pub title: String,
    /// Its symbol role (`delete`), for a menu's leading symbol.
    pub symbol: Option<String>,
    /// `destructive`.
    pub destructive: bool,
    /// Not `disabled`.
    pub enabled: bool,
    /// The alert's cancel: it closes and runs nothing.
    pub cancel: bool,
    /// `aria-checked`.
    pub checked: bool,
    /// The `id` of the menu it opens, for a row that opens one.
    pub opens: Option<String>,
}

/// What the top layer shows for one authored subtree.
#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    /// Material's basic dialog.
    Alert {
        /// The headline.
        title: Option<String>,
        /// The supporting text.
        message: Option<String>,
        /// The buttons as presented: the cancel, then the actions in the
        /// order the author wrote them.
        rows: Vec<Row>,
        /// Back and Escape cancel it (`closedby` is not `none`).
        cancellable: bool,
    },
    /// Material's menu: its items, `None` where the author wrote a separator.
    Menu(Vec<Option<Row>>),
}

impl Plan {
    /// The rows a person can choose, in order.
    pub fn rows(&self) -> impl Iterator<Item = &Row> {
        match self {
            Plan::Alert { rows, .. } => rows.iter().collect::<Vec<_>>(),
            Plan::Menu(entries) => entries.iter().flatten().collect(),
        }
        .into_iter()
    }
}

fn is_button(node: &NodeRef<'_>) -> bool {
    node.node_type == NodeType::Pressable
        || ControlKind::of(node.node_type, node.props) == Some(ControlKind::Button)
}

fn is_dialog(node: &NodeRef<'_>) -> bool {
    node.props.str(PropId::SemanticTag) == Some("dialog")
}

/// Whether `node` is presented as a confirmation: a `dialog`, or an
/// `alertdialog` popover.
pub fn is_confirmation(node: &NodeRef<'_>) -> bool {
    is_dialog(node) || node.props.str(PropId::AccessibilityRole) == Some("alertdialog")
}

/// The `id` a button names: its `commandfor`, else its `popovertarget`.
fn target<'a>(node: &NodeRef<'a>) -> Option<&'a str> {
    let named = |id| node.props.str(id).filter(|name: &&str| !name.is_empty());
    named(PropId::Commandfor).or_else(|| named(PropId::Popovertarget))
}

/// Whether pressing `row` closes `pop`: `commandfor` with `command="close"`
/// for a dialog; for a popover its `popovertarget` with
/// `popovertargetaction="hide"`, or `command="hide-popover"`.
fn closes(row: &NodeRef<'_>, pop: &NodeRef<'_>) -> bool {
    let id = pop.props.str(PropId::Id);
    let command = row.props.str(PropId::Command);
    if is_dialog(pop) {
        return row.props.str(PropId::Commandfor) == id && id.is_some() && command == Some("close");
    }
    target(row) == id
        && id.is_some()
        && (command == Some("hide-popover")
            || row.props.str(PropId::Popovertargetaction) == Some("hide"))
}

/// A node's text: a `text`'s own, white space collapsed; else its
/// children's, a space between them, less what is hidden from assistive
/// technology (a submenu row's `›`).
fn text_of(kernel: &Kernel, node: &NodeRef<'_>) -> String {
    if node.node_type == NodeType::Text {
        let text: String = node.text_runs().iter().map(|r| &*r.text).collect();
        return text.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    node.children()
        .into_iter()
        .filter_map(|id| kernel.node(id))
        .filter(|n| n.props.bool(PropId::AccessibilityElementsHidden) != Some(true))
        .map(|n| text_of(kernel, &n))
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A button's row: its face's title, else its label, else its text.
pub(in crate::presenter) fn row(kernel: &Kernel, node: &NodeRef<'_>, cancel: bool) -> Row {
    let face = kernel.press_face(node.id).unwrap_or_default();
    let title = face
        .title
        .or(face.label)
        .unwrap_or_else(|| text_of(kernel, node));
    Row {
        node: node.id,
        title,
        symbol: face.symbol,
        destructive: node.props.bool(PropId::Destructive) == Some(true),
        enabled: node.props.bool(PropId::Disabled) != Some(true),
        cancel,
        checked: node.props.bool(PropId::AccessibilityChecked) == Some(true),
        opens: None,
    }
}

/// The alert `pop` is, or `None` for any other shape. `pressed` says
/// whether a node has a `press` handler.
pub fn alert(kernel: &Kernel, pop: ViewId, pressed: &dyn Fn(ViewId) -> bool) -> Option<Plan> {
    let pop = kernel.node(pop)?;
    if pop.props.str(PropId::AccessibilityRole) != Some("alertdialog")
        || !(is_dialog(&pop) || pop.props.str(PropId::Popover).is_some())
    {
        return None;
    }
    let children: Vec<NodeRef<'_>> = (pop.children().into_iter())
        .filter_map(|id| kernel.node(id))
        .collect();
    let button = |n: &&NodeRef<'_>| is_button(n);
    let actions: Vec<&NodeRef<'_>> = (children.iter().filter(button))
        .filter(|n| pressed(n.id))
        .collect();
    let cancels: Vec<&NodeRef<'_>> = (children.iter().filter(button))
        .filter(|n| !pressed(n.id) && closes(n, &pop))
        .collect();
    let texts: Vec<&NodeRef<'_>> = (children.iter())
        .filter(|n| n.node_type == NodeType::Text)
        .collect();
    let enabled = |n: &&NodeRef<'_>| n.props.bool(PropId::Disabled) != Some(true);
    if children.len() != actions.len() + cancels.len() + texts.len()
        || !actions.iter().all(|n| closes(n, &pop))
        || !actions.iter().any(enabled)
    {
        return None;
    }
    let lines: Vec<String> = (texts.iter().map(|n| text_of(kernel, n)))
        .filter(|line| !line.is_empty())
        .collect();
    if !is_alert(lines.len(), actions.len(), cancels.len()) {
        return None;
    }
    let (title, message) = alert_text(pop.props.str(PropId::AccessibilityLabel), &lines);
    let mut rows = vec![row(kernel, cancels[0], true)];
    rows.extend(actions.iter().map(|n| row(kernel, n, false)));
    Some(Plan::Alert {
        title,
        message,
        rows,
        cancellable: pop.props.str(PropId::Closedby) != Some("none"),
    })
}

/// The menu `pop` is, or `None` for a popover that is a confirmation or has
/// no item. `pressed` says whether a node has a `press` handler.
pub fn menu(kernel: &Kernel, pop: ViewId, pressed: &dyn Fn(ViewId) -> bool) -> Option<Plan> {
    let pop = kernel.node(pop)?;
    pop.props.str(PropId::Popover)?;
    if is_confirmation(&pop) {
        return None;
    }
    let mut entries: Vec<Option<Row>> = Vec::new();
    for child in pop.children() {
        let Some(node) = kernel.node(child) else {
            continue;
        };
        // A context menu's preview and a submenu written inside its parent
        // are no rows of it.
        if node.props.bool(PropId::ContextPreview) == Some(true)
            || node.props.str(PropId::Popover).is_some()
        {
            continue;
        }
        // The menu a row opens: one its `popovertarget` names to show.
        let opens = (node.props.str(PropId::Popovertarget))
            .filter(|name| !name.is_empty() && Some(*name) != pop.props.str(PropId::Id))
            .filter(|_| node.props.str(PropId::Popovertargetaction) != Some("hide"));
        if is_button(&node) && (pressed(node.id) || opens.is_some()) {
            let mut item = row(kernel, &node, false);
            item.opens = opens.map(str::to_string);
            entries.push(Some(item));
        } else if entries.last().is_some_and(Option::is_some) {
            entries.push(None);
        }
    }
    if entries.last().is_some_and(Option::is_none) {
        entries.pop();
    }
    (!entries.is_empty()).then_some(Plan::Menu(entries))
}
