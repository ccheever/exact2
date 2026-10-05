//! A grouped list's sections and rows (LLP 1084 D3, D4), read from its
//! tree as it now stands, for a host that draws the platform's own list.
//! Contract shaped the tree: the list's children are sections; a section is
//! an optional `header`, one group whose children are the rows, an optional
//! `footer`. A row whose children are a standard cell's parts is read as
//! data; any other row is custom, and the host carries its views.

use crate::control::ControlKind;
use crate::generated::{Display, FlexDirection, NodeType, PropId};
use crate::id::ViewId;
use crate::kernel::{Kernel, NodeRef};

/// A grouped list as a host draws it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GroupedList {
    /// `listStyle`: `inset-grouped`, `grouped` or `plain`.
    pub style: String,
    /// Its sections, in order.
    pub sections: Vec<GroupedSection>,
}

/// A section: its texts and its rows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GroupedSection {
    /// The `section` node.
    pub view: ViewId,
    /// Its `header`'s text.
    pub header: Option<String>,
    /// Its `footer`'s text.
    pub footer: Option<String>,
    /// Its rows, in order.
    pub rows: Vec<GroupedRow>,
    /// Whether its rows sit on a card: false when the section's group has a
    /// transparent background (`section background-color="transparent"`).
    pub card: bool,
    /// Its `margin-top` and `margin-bottom` in points: Contract's sheet
    /// writes UIKit's gaps (§2), and an author's override (Signal's 20-pt
    /// sections) is the space a host leaves above and below it. `None` for
    /// a margin that is not points.
    pub margins: (Option<f32>, Option<f32>),
}

/// What a row shows at its trailing edge (D4).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Accessory {
    /// Nothing.
    #[default]
    None,
    /// A trailing chevron image: UIKit's disclosure indicator.
    Disclosure,
    /// A trailing checkmark image.
    Checkmark,
    /// A trailing checkbox or switch control: a switch, which flips that control.
    Toggle(ViewId),
    /// A trailing button holding only an `info.circle` symbol: the detail
    /// button, which presses that button.
    Detail(ViewId),
}

/// A row. A custom row carries nothing but its node, its press and its
/// state: the host shows its views as they are.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GroupedRow {
    /// The row's node.
    pub view: ViewId,
    /// Whether its children are anything but a standard cell's parts.
    pub custom: bool,
    /// The leading symbol's Apple name.
    pub symbol: Option<String>,
    /// The title.
    pub title: Option<String>,
    /// The second text: a value beside the title, or a subtitle under it.
    pub secondary: Option<String>,
    /// Whether `secondary` is under the title (a `column` of two texts).
    pub subtitle: bool,
    /// The trailing accessory.
    pub accessory: Accessory,
    /// Whether the row is a button, so a tap presses it.
    pub pressable: bool,
    /// `destructive`.
    pub destructive: bool,
    /// `disabled`.
    pub disabled: bool,
}

/// A symbol image's Apple name: an `sf/` name as written, a role by the
/// symbol table; `None` for any other node or source.
fn apple_symbol(node: &NodeRef<'_>) -> Option<String> {
    if node.node_type != NodeType::Image {
        return None;
    }
    let role = node
        .props
        .str(PropId::ImageSource)?
        .strip_prefix("symbol:")?;
    Some(match role.strip_prefix("sf/") {
        Some(name) => name.to_owned(),
        None => crate::generated::symbol(role)?.0.to_owned(),
    })
}

/// A text node's text, white space collapsed; `None` when blank.
fn text_of(node: &NodeRef<'_>) -> Option<String> {
    if node.node_type != NodeType::Text {
        return None;
    }
    let text: String = node.text_runs().iter().map(|r| &*r.text).collect();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

impl Kernel {
    /// The grouped list `view` is, as it now stands; `None` for a node
    /// without `listStyle`.
    pub fn grouped_list(&self, view: ViewId) -> Option<GroupedList> {
        let list = self.node(view)?;
        let style = list.props.str(PropId::ListStyle)?.to_owned();
        // What `display: none` hides on the web is no section or row here.
        let nodes = |n: &NodeRef<'_>| -> Vec<NodeRef<'_>> {
            n.children()
                .into_iter()
                .filter_map(|id| self.node(id))
                .filter(|c| c.style.display != Display::None)
                .collect()
        };
        let label = |n: &NodeRef<'_>| nodes(n).iter().find_map(text_of);
        let sections = nodes(&list)
            .iter()
            .map(|section| {
                let points = |d: crate::style::Dimension| match d {
                    crate::style::Dimension::Points(p) => Some(p),
                    _ => None,
                };
                let mut out = GroupedSection {
                    view: section.id,
                    card: true,
                    margins: (
                        points(section.style.margin_top),
                        points(section.style.margin_bottom),
                    ),
                    ..GroupedSection::default()
                };
                for part in nodes(section) {
                    match part.props.str(PropId::SemanticTag) {
                        Some("header") => out.header = label(&part),
                        Some("footer") => out.footer = label(&part),
                        _ => {
                            out.card = part.style.background_color.is_none_or(|c| {
                                c.resolve(false).a() != 0 || c.resolve(true).a() != 0
                            });
                            out.rows = nodes(&part).iter().map(|r| self.grouped_row(r)).collect()
                        }
                    }
                }
                out
            })
            .collect();
        Some(GroupedList { style, sections })
    }

    fn grouped_row(&self, row: &NodeRef<'_>) -> GroupedRow {
        let mut out = GroupedRow {
            view: row.id,
            pressable: row.node_type == NodeType::Pressable,
            destructive: row.props.bool(PropId::Destructive) == Some(true),
            disabled: row.props.bool(PropId::Disabled) == Some(true),
            ..GroupedRow::default()
        };
        // A native button is the platform's control: its views, carried.
        if ControlKind::of(row.node_type, row.props).is_some() {
            out.custom = true;
            return out;
        }
        let shown = |n: &NodeRef<'_>| -> Vec<NodeRef<'_>> {
            n.children()
                .into_iter()
                .filter_map(|id| self.node(id))
                .filter(|c| c.style.display != Display::None)
                .collect()
        };
        let parts = shown(row);
        let accessory = |apple: &str| match apple {
            "chevron.forward" | "chevron.right" => Some(Accessory::Disclosure),
            "checkmark" => Some(Accessory::Checkmark),
            _ => None,
        };
        let mut rest = &parts[..];
        // The trailing accessory, then the leading symbol, then the texts.
        if let Some((last, before)) = rest.split_last() {
            let toggle = matches!(
                ControlKind::of(last.node_type, last.props),
                Some(ControlKind::Switch | ControlKind::Checkbox)
            );
            let detail = last.node_type == NodeType::Pressable && {
                let inner = last.children();
                inner.len() == 1
                    && self
                        .node(inner[0])
                        .and_then(|i| apple_symbol(&i))
                        .is_some_and(|s| s == "info.circle" || s == "info.circle.fill")
            };
            let found = if toggle {
                Some(Accessory::Toggle(last.id))
            } else if detail {
                Some(Accessory::Detail(last.id))
            } else {
                apple_symbol(last).and_then(|s| accessory(&s))
            };
            if let Some(found) = found {
                out.accessory = found;
                rest = before;
            }
        }
        if let Some((first, after)) = rest.split_first() {
            if let Some(symbol) = apple_symbol(first) {
                out.symbol = Some(symbol);
                rest = after;
            }
        }
        match rest {
            [title] if title.node_type == NodeType::Text => out.title = text_of(title),
            [title, value]
                if title.node_type == NodeType::Text && value.node_type == NodeType::Text =>
            {
                out.title = text_of(title);
                out.secondary = text_of(value);
            }
            // A `column`, as Contract's sheet styles one; a `row` or a box
            // of texts is the row's own layout.
            [stack]
                if stack.node_type == NodeType::View
                    && stack.style.display == Display::Flex
                    && stack.style.flex_direction == FlexDirection::Column =>
            {
                let lines = shown(stack);
                if lines.iter().all(|l| l.node_type == NodeType::Text)
                    && (1..=2).contains(&lines.len())
                {
                    out.title = text_of(&lines[0]);
                    out.secondary = lines.get(1).and_then(text_of);
                    out.subtitle = true;
                } else {
                    out.custom = true;
                }
            }
            _ => out.custom = true,
        }
        if out.title.is_none() {
            out.custom = true;
        }
        if out.custom {
            // A custom row is its views, accessory and all.
            out = GroupedRow {
                view: out.view,
                custom: true,
                pressable: out.pressable,
                destructive: out.destructive,
                disabled: out.disabled,
                ..GroupedRow::default()
            };
        }
        out
    }
}
