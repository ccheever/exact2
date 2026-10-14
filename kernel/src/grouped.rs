//! A grouped list's sections and rows (LLP 1084 D3, D4), read from its
//! tree as it now stands, for a host that draws the platform's own list.
//! Contract shaped the tree: the list's children are sections; a section is
//! an optional `header`, one group whose children are the rows, an optional
//! `footer`. A row whose children are a standard cell's parts is read as
//! data; any other row is custom, and the host carries its views.

use crate::arena::NodeArena;
use crate::control::ControlKind;
use crate::generated::{
    Display, FlexDirection, FontStyle, NodeType, Overflow, PropId, StyleId, StyleMask,
    TextOverflow, WhiteSpace,
};
use crate::id::ViewId;
use crate::kernel::{Kernel, NodeRef};
use crate::style::{ColorValue, Env};

/// A grouped list as a host draws it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GroupedList {
    /// `listStyle`: `inset-grouped`, `grouped` or `plain`.
    pub style: String,
    /// Its sections, in order.
    pub sections: Vec<GroupedSection>,
    /// The space under the last section, when the author set its
    /// `margin-bottom` (as `GroupedSection::space_above`).
    pub space_below: Option<f32>,
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
    /// How its header's text is set where its author said so.
    pub header_look: TextLook,
    /// How its footer's text is set where its author said so.
    pub footer_look: TextLook,
    /// Its rows, in order.
    pub rows: Vec<GroupedRow>,
    /// Whether its rows sit on a card: false when the section's group has a
    /// transparent background (`section background-color="transparent"`).
    pub card: bool,
    /// The space above it, in points, when the author wrote a margin at
    /// that boundary (its `margin-top`, or the previous section's
    /// `margin-bottom`): the web's, the two margins collapsed. `None` keeps
    /// the platform's own gap, which Contract's sheet stands in for (§2).
    pub space_above: Option<f32>,
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
    /// An `input type="radio"` at either end of the row, and whether it is
    /// checked: the platform's checkmark on the chosen row, and a tap on
    /// the row checks that radio (LLP 1115.000 D13.2).
    Radio(ViewId, bool),
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
    /// How the title is set where its author said so.
    pub title_look: TextLook,
    /// How the second text is set where its author said so.
    pub secondary_look: TextLook,
}

/// How a text the platform's list draws is set where its author said so
/// (LLP 1116.003 D7: author > platform, LLP 1115). The defaults are the
/// platform's: every line, wrapped, in the platform's colour and font.
/// "Said" is the row's own fact (LLP 1115.000 D2): the row that gives the
/// text its value was written by the author, on the text or on a box it
/// inherits from. Contract's sheet stands in for the platform there and
/// writes nothing, so its rows, like a value no one set, are unsaid.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TextLook {
    /// The most lines it shows, 0 for any number: its `line-clamp`, or 1
    /// where `white-space: nowrap` keeps it on one line.
    pub lines: u32,
    /// Whether a line cut short ends in "…" rather than at the edge: under
    /// a `line-clamp`, as CSS draws one, or `text-overflow: ellipsis` in a
    /// box that clips its overflow.
    pub ellipsis: bool,
    /// Its `color`, when its author wrote one.
    pub color: Option<ColorValue>,
    /// Its font as computed, when its author wrote its size, weight, style,
    /// family or numerals.
    pub font: Option<TextFont>,
}

/// A text's font as its computed style has it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextFont {
    /// `font-size` in points: a text style's already read at the root size.
    pub size: f32,
    /// `font-weight`.
    pub weight: u16,
    /// `font-style: italic`.
    pub italic: bool,
    /// `font-family`, by its id in the plan's catalog; 0 is the system's.
    pub family: u16,
    /// `font-variant-numeric`'s bits (1: `tabular-nums`).
    pub numeric: u8,
}

/// The rows a text's font is made of.
const FONT: [StyleId; 5] = [
    StyleId::FontSize,
    StyleId::FontWeight,
    StyleId::FontStyle,
    StyleId::FontFamily,
    StyleId::FontVariantNumeric,
];

impl TextLook {
    /// The look the text node `node` asks for, as the web lays it out and
    /// paints it.
    fn of(node: &NodeRef<'_>) -> TextLook {
        let mut rows = StyleMask::EMPTY;
        for row in [StyleId::WhiteSpace, StyleId::TextColor]
            .into_iter()
            .chain(FONT)
        {
            rows.set(row);
        }
        let s = node.computed_style(rows);
        let font = TextFont {
            size: s.font_size,
            weight: s.font_weight,
            italic: s.font_style == FontStyle::Italic,
            family: s.font_family,
            numeric: s.font_variant_numeric,
        };
        let mut look = TextLook {
            color: node.says(StyleId::TextColor).then_some(s.text_color),
            font: FONT.iter().any(|row| node.says(*row)).then_some(font),
            ..TextLook::default()
        };
        let clamp = node.style.line_clamp;
        if clamp > 0 {
            look.lines = clamp;
            look.ellipsis = true;
        } else if s.white_space == WhiteSpace::Nowrap {
            look.lines = 1;
            look.ellipsis = node.style.text_overflow == TextOverflow::Ellipsis
                && node.style.overflow_x != Overflow::Visible;
        }
        look
    }
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

/// A text node's text, its white space collapsed as its `white-space` has
/// the web collapse it (LLP 1116.004 D2): `normal` makes a line feed a
/// space, and `pre-line` keeps it, so a footer the author set `pre-line` is
/// two lines in UIKit's cell as on the page. `None` when blank.
pub(crate) fn text_of(node: &NodeRef<'_>) -> Option<String> {
    if node.node_type != NodeType::Text {
        return None;
    }
    let text_runs = node.text_runs();
    let runs: Vec<&str> = text_runs.iter().map(|r| &*r.text).collect();
    let mut rows = StyleMask::EMPTY;
    rows.set(StyleId::WhiteSpace);
    let white_space = node.computed_style(rows).white_space.model();
    let text = match exact_textflow::collapse(&runs, white_space) {
        Some(collapsed) => collapsed.runs.concat(),
        None => runs.concat(),
    };
    (!text.trim().is_empty()).then_some(text)
}

impl GroupedRow {
    /// A standard cell's title and second text, and how each is set: the
    /// second a subtitle under the title, else a value beside it.
    fn texts(&mut self, title: &NodeRef<'_>, second: Option<&NodeRef<'_>>, subtitle: bool) {
        self.title = text_of(title);
        self.title_look = TextLook::of(title);
        self.secondary = second.and_then(text_of);
        self.secondary_look = second.map(TextLook::of).unwrap_or_default();
        self.subtitle = subtitle;
    }
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
        let label = |n: &NodeRef<'_>| {
            nodes(n)
                .iter()
                .find_map(|t| text_of(t).map(|text| (text, TextLook::of(t))))
        };
        let shown = nodes(&list);
        let sections = shown
            .iter()
            .map(|section| {
                let mut out = GroupedSection {
                    view: section.id,
                    card: true,
                    ..GroupedSection::default()
                };
                for part in nodes(section) {
                    match part.props.str(PropId::SemanticTag) {
                        Some("header") => {
                            (out.header, out.header_look) = label(&part)
                                .map_or((None, TextLook::default()), |(t, l)| (Some(t), l))
                        }
                        Some("footer") => {
                            (out.footer, out.footer_look) = label(&part)
                                .map_or((None, TextLook::default()), |(t, l)| (Some(t), l))
                        }
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
            .collect::<Vec<GroupedSection>>();
        let mut list = GroupedList {
            style,
            sections,
            space_below: None,
        };
        // A section's percentage margin is of the list's content box
        let basis = crate::svg::scene::content_box(self, &self.node(view)?).2;
        authored_space(&mut list, &shown, self.arena().env(), basis);
        Some(list)
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
        // A row that lays out as a column (a `column`, LLP 1116 D2) stacks
        // its parts: one or two texts are the subtitle cell's lines, and
        // anything else is its own layout.
        if row.style.display == Display::Flex
            && matches!(
                row.style.flex_direction,
                FlexDirection::Column | FlexDirection::ColumnReverse
            )
        {
            if (1..=2).contains(&parts.len()) && parts.iter().all(|p| p.node_type == NodeType::Text)
            {
                out.texts(&parts[0], parts.get(1), true);
            }
            if out.title.is_none() {
                return GroupedRow {
                    custom: true,
                    ..out
                };
            }
            return out;
        }
        let accessory = |apple: &str| match apple {
            "chevron.forward" | "chevron.right" => Some(Accessory::Disclosure),
            "checkmark" => Some(Accessory::Checkmark),
            _ => None,
        };
        let mut rest = &parts[..];
        // A radio's row is a choice: the radio is its accessory wherever its
        // author put it, before the label as a web form has it or after.
        let radio = |n: &NodeRef<'_>| {
            matches!(
                ControlKind::of(n.node_type, n.props),
                Some(ControlKind::Radio)
            )
            .then(|| Accessory::Radio(n.id, n.props.bool(PropId::Checked) == Some(true)))
        };
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
                radio(last).or_else(|| apple_symbol(last).and_then(|s| accessory(&s)))
            };
            if let Some(found) = found {
                out.accessory = found;
                rest = before;
            }
        }
        if let (Accessory::None, Some((first, after))) = (out.accessory, rest.split_first()) {
            if let Some(found) = radio(first) {
                out.accessory = found;
                rest = after;
            }
        }
        if let Some((first, after)) = rest.split_first() {
            if let Some(symbol) = apple_symbol(first) {
                out.symbol = Some(symbol);
                rest = after;
            }
        }
        match rest {
            [title] if title.node_type == NodeType::Text => out.texts(title, None, false),
            [title, value]
                if title.node_type == NodeType::Text && value.node_type == NodeType::Text =>
            {
                out.texts(title, Some(value), false)
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
                    out.texts(&lines[0], lines.get(1), true);
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

impl NodeArena {
    /// Whether `slot` is a native text field (an unsaid `input` or
    /// `textarea`, LLP 1104 D2) in a grouped list's row, at any depth: the
    /// field a platform draws in a list's cell (LLP 1116.005 D2), which on
    /// iOS is borderless, the cell being the field. A search field keeps its
    /// own chrome. Read from the tree as it stands: a list with `listStyle`,
    /// a section, its rows' group (not its `header` or `footer`), a row.
    pub(crate) fn is_cell_field(&self, slot: u32) -> bool {
        use crate::FieldKind;
        if !self.is_native_text_control(slot)
            || FieldKind::from_props(self.props(slot)) == FieldKind::SearchField
        {
            return false;
        }
        let mut at = slot;
        while let Some(group) = self.parent(at) {
            let list = self.parent(group).and_then(|section| self.parent(section));
            if list.is_some_and(|l| self.props(l).str(PropId::ListStyle).is_some())
                && !matches!(
                    self.props(group).str(PropId::SemanticTag),
                    Some("header" | "footer")
                )
            {
                return true;
            }
            at = group;
        }
        false
    }
}

impl NodeRef<'_> {
    /// Whether this is a native text field in a grouped list's row, which a
    /// host draws as its platform draws a field in a list's cell
    /// (`NodeArena::is_cell_field`, LLP 1116.005 D2).
    pub fn is_cell_field(&self) -> bool {
        self.arena.is_cell_field(self.slot)
    }
}

/// Two adjoining vertical margins as CSS collapses them: the largest
/// positive plus the most negative (CSS 2 §8.3.1).
fn collapse(a: f32, b: f32) -> f32 {
    a.max(b).max(0.0) + a.min(b).min(0.0)
}

/// Each boundary where the author wrote a margin takes the web's space: the
/// margins that meet there collapsed, as block margins do (§6.4). A margin
/// is the author's when its row says so (LLP 1115.000 D2), whatever its
/// number; the sheet's (LLP 1084 §2) stands in for the platform's gap.
fn authored_space(list: &mut GroupedList, shown: &[NodeRef<'_>], env: &Env, basis: f32) {
    use crate::style::Dimension;
    let mut below: Option<(f32, bool)> = None;
    for (section, node) in list.sections.iter_mut().zip(shown) {
        let points = |d: Dimension| d.at_basis(env, basis);
        let top = points(node.style.margin_top);
        let authored = node.style.written.has(StyleId::MarginTop);
        section.space_above = match below {
            Some((bottom, mine)) if mine || authored => Some(collapse(top, bottom)),
            None if authored => Some(top),
            _ => None,
        };
        let bottom = points(node.style.margin_bottom);
        below = Some((bottom, node.style.written.has(StyleId::MarginBottom)));
    }
    list.space_below = below.and_then(|(bottom, mine)| mine.then_some(bottom));
}
