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

mod separator;
pub use separator::GroupedSeparator;

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
    /// The leading symbol's Apple name ("" for an icon token with none).
    pub symbol: Option<String>,
    /// The leading symbol's icon token form (after `symbol:`) when it has
    /// no Apple name but a template path, which the host draws as the
    /// cell's template image (LLP 1121 D10, stage 9 A2); `None` otherwise.
    pub icon: Option<String>,
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
    /// Whether it is an action cell: a `button` (no `href`) of a title, a
    /// symbol before it at most, and nothing at its trailing edge, whose
    /// title the platform's list draws in the tint (LLP 1116.019 D6), as
    /// the sheet's `grouped-action-color` does (`contract/lower/src/grouped.rs`).
    /// A row with `aria-selected` is a choice, never one.
    pub action: bool,
    /// `destructive`.
    pub destructive: bool,
    /// `disabled`.
    pub disabled: bool,
    /// How the title is set where its author said so.
    pub title_look: TextLook,
    /// How the second text is set where its author said so.
    pub secondary_look: TextLook,
    /// Whether its author wrote the edge under it (its bottom edge or its
    /// background image: `Kernel::grouped_separator`'s rule), so the
    /// platform's separator gives way there, to the author's border or to
    /// none.
    pub authored_edge: bool,
    /// Whether the next shown row's author wrote its top edge, which the
    /// platform's separator above it gives way to.
    pub next_authored_top: bool,
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
    /// Which of the five its author said, by [`TextFont::SAID`]'s bits: the
    /// host keeps the platform's own for the rest (LLP 1115.000 D2), so a
    /// family written on the page leaves a cell's size and weight UIKit's.
    pub said: u8,
}

impl TextFont {
    /// The bits of `said`, in [`FONT`]'s order: size, weight, style, family
    /// and numerals.
    pub const SAID: [u8; 5] = [1, 2, 4, 8, 16];
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
        let said = FONT
            .iter()
            .zip(TextFont::SAID)
            .filter(|(row, _)| node.says(**row))
            .fold(0, |said, (_, bit)| said | bit);
        let font = TextFont {
            size: s.font_size,
            weight: s.font_weight,
            italic: s.font_style == FontStyle::Italic,
            family: s.font_family,
            numeric: s.font_variant_numeric,
            said,
        };
        let mut look = TextLook {
            color: node.says(StyleId::TextColor).then_some(s.text_color),
            font: (said != 0).then_some(font),
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
    // An icon token's SF Symbol too, or "" for one whose path the host
    // draws (LLP 1121 D10); an empty name is no symbol, as an empty `sf/`
    // one is (stage 9 H).
    let name = crate::apple_symbol_name(role);
    (!name.is_empty() || role.is_empty() || role.starts_with("sf/") || drawn_icon(role).is_some())
        .then(|| name.to_owned())
}

/// The icon token form `role` is when it has no SF Symbol and a template
/// path: the host draws that path where Apple has no symbol (stage 9 A2).
fn drawn_icon(role: &str) -> Option<&str> {
    let icon = crate::Icon::parse(role)?;
    (icon.sf.is_empty() && !icon.path.is_empty()).then_some(role)
}

/// The icon token form a symbol image draws by its path (`drawn_icon`).
fn icon_of(node: &NodeRef<'_>) -> Option<String> {
    let role = node
        .props
        .str(PropId::ImageSource)?
        .strip_prefix("symbol:")?;
    drawn_icon(role).map(str::to_owned)
}

/// A text node's text, its white space collapsed as its `white-space` has
/// the web collapse it (LLP 1116.004 D2): `normal` makes a line feed a
/// space, and `pre-line` keeps it, so a footer the author set `pre-line` is
/// two lines in UIKit's cell as on the page. `None` when blank.
pub(crate) fn text_of(node: &NodeRef<'_>) -> Option<String> {
    if node.node_type != NodeType::Text {
        return None;
    }
    let text_runs = node.read_runs();
    let runs: Vec<&str> = text_runs.iter().map(|r| &**r).collect();
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
            authored_edge: self.grouped_edge_owners(row).0,
            next_authored_top: self.grouped_edge_owners(row).1,
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
                out.icon = icon_of(first);
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
        // UIKit's content configuration draws none of the row's box: a row
        // whose author wrote an edge of it, or its background image, is its
        // own views, carried, as every other host draws it (LLP 1084 D7).
        let w = &row.style.written;
        if out.authored_edge
            || [
                (StyleId::BorderWidthTop, StyleId::BorderStyleTop),
                (StyleId::BorderWidthRight, StyleId::BorderStyleRight),
                (StyleId::BorderWidthBottom, StyleId::BorderStyleBottom),
                (StyleId::BorderWidthLeft, StyleId::BorderStyleLeft),
            ]
            .iter()
            .any(|&(width, line)| w.has(width) || w.has(line))
        {
            out.custom = true;
        }
        // A row that says whether it is selected (`aria-selected`, true or
        // false) is a choice, as a checkmark list's rows are: its title is
        // the label's whether or not its mark shows now, never an action's.
        // The sheet's word, where it gave one (`groupedAction`), wins over
        // the shape that shows now: an unpicked choice row is no action.
        out.action = out.pressable
            && row.props.bool(PropId::GroupedAction) != Some(false)
            && row.props.str(PropId::Href).is_none()
            && !row.props.contains(PropId::AccessibilitySelected)
            && out.accessory == Accessory::None
            && out.secondary.is_none()
            && !out.subtitle
            && !out.custom;
        if out.custom {
            // A custom row is its views, accessory and all.
            out = GroupedRow {
                view: out.view,
                custom: true,
                pressable: out.pressable,
                destructive: out.destructive,
                disabled: out.disabled,
                authored_edge: out.authored_edge,
                next_authored_top: out.next_authored_top,
                ..GroupedRow::default()
            };
        }
        out
    }
}

/// Where the platform's own list puts a grouped list's cards and their
/// content, in points: each card's inset from the list's content-box edges
/// (excluding CSS padding), and each cell's layout margins, left and right.
/// UIKit chooses them by the device, the list's width and what lies beside
/// it (16 or 20 points, and none beside the iPhone Duo's side strip). The host
/// reads them from its list (`GroupedPlacingIOS.swift`), and the sheet's stand-ins
/// (`grouped-card-inset`, `grouped-row-inset`, `grouped-row-padding-end`)
/// give way to them: a custom row's content then starts where a standard
/// row's text does, at the cell's margin, as UIKit's own content
/// configurations put it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroupedMargins {
    /// The card's left and right insets from the list's content-box edges.
    pub card: [f32; 2],
    /// The cell's left and right layout margins.
    pub cell: [f32; 2],
}

/// The margins of the list `slot` holds the rows of, when it is a section's
/// group (not its `header` or `footer`) and the host gave the list some.
fn group_margins(arena: &NodeArena, slot: u32) -> Option<GroupedMargins> {
    let margins = arena.grouped_margins(arena.parent(arena.parent(slot)?)?)?;
    (!is_label(arena, slot)).then_some(margins)
}

/// Whether a section's part is its `header` or `footer`.
fn is_label(arena: &NodeArena, slot: u32) -> bool {
    matches!(
        arena.props(slot).str(PropId::SemanticTag),
        Some("header" | "footer")
    )
}

/// Whether the swipe `scroll` is a `delete=` row's, which spans the card and
/// holds the row's insets in its content (`swipe_rows::grouped`): its left
/// margin is the sheet's swipe margin and it has no padding. A swipe written
/// by hand is styled as any row, its insets its scroll's padding.
fn inset_inside(arena: &NodeArena, scroll: u32) -> bool {
    let style = arena.style(scroll);
    arena.props(scroll).str(PropId::SwipeContent).is_some()
        && !style.mask.has(StyleId::PaddingLeft)
        && !style.written.has(StyleId::MarginLeft)
        && style.margin_left
            == crate::style::Dimension::Points(sheet("-exact-grouped-swipe-margin"))
}

/// The platform's margins in a box's engine style: a group spans its card,
/// and a row's unsaid inset at each end moves by what the platform's cell
/// margin has over the sheet's stand-in, on the box the sheet wrote it on: a
/// row's left and right paddings (after a leading symbol too), the row
/// spanning its card; a `delete=` row's content's, its swipe spanning the
/// card. A native button row is the platform's control: its left margin
/// and its widest (the card less its insets) move, and it keeps its own
/// padding. An end the author wrote stays the author's.
pub(crate) fn apply_margins(arena: &NodeArena, slot: u32, s: &mut taffy::style::Style) {
    use taffy::style::{LengthPercentage as Lp, LengthPercentageAuto as Lpa};
    use taffy::util::MaybeResolve;
    if !arena.has_grouped_margins() {
        return;
    }
    if let Some(m) = group_margins(arena, slot) {
        s.margin.left = Lpa::length(m.card[0]);
        s.margin.right = Lpa::length(m.card[1]);
        return;
    }
    let Some(parent) = arena.parent(slot) else {
        return;
    };
    let (m, content) = match group_margins(arena, parent) {
        Some(_) if inset_inside(arena, slot) => return,
        Some(m) => (m, false),
        None if inset_inside(arena, parent) => {
            let named =
                arena.props(parent).str(PropId::SwipeContent) == arena.props(slot).str(PropId::Id);
            match arena.parent(parent).and_then(|g| group_margins(arena, g)) {
                Some(m) if named => (m, true),
                _ => return,
            }
        }
        None => return,
    };
    // A padding moves and stays at zero or more; a margin moves as it is.
    let padded = |at: Option<f32>, by: f32| at.map(|v| (v + by).max(0.0));
    let written = &arena.style(slot).written;
    if content && !written.has(StyleId::PaddingLeft) {
        let at = s
            .padding
            .left
            .maybe_resolve(None, crate::style::resolve_calc);
        let by = m.cell[0] - sheet("-exact-grouped-swipe-padding");
        s.padding.left = padded(at, by).map_or(s.padding.left, Lp::length);
    }
    let control = ControlKind::of(arena.node_type(slot), arena.props(slot)).is_some();
    let by = m.cell[0] - sheet("-exact-grouped-row-inset");
    if !content && control && !written.has(StyleId::MarginLeft) {
        let at: Option<f32> = s
            .margin
            .left
            .maybe_resolve(None, crate::style::resolve_calc);
        s.margin.left = at.map_or(s.margin.left, |v| Lpa::length(v + by));
        // Its widest, the card less its insets, moves with its inset.
        if let crate::style::Dimension::Calc(percent, points) = arena.style(slot).max_width {
            if !written.has(StyleId::MaxWidth) {
                s.max_size.width =
                    crate::style::Dimension::Calc(percent, points - by).floor_lpa(arena.env());
            }
        }
    } else if !content && !control && !written.has(StyleId::PaddingLeft) {
        let at = s
            .padding
            .left
            .maybe_resolve(None, crate::style::resolve_calc);
        s.padding.left = padded(at, by).map_or(s.padding.left, Lp::length);
    }
    if !control && !written.has(StyleId::PaddingRight) && !written.has(StyleId::MarginRight) {
        let at = s
            .padding
            .right
            .maybe_resolve(None, crate::style::resolve_calc);
        let by = m.cell[1] - sheet("-exact-grouped-row-padding-end");
        s.padding.right = padded(at, by).map_or(s.padding.right, Lp::length);
    }
}

/// Whether the margins of a grouped list, or a cells list's (LLP 1138),
/// reach `slot` where it stands: a list four boxes up at most has them (a
/// `delete=` row's content is the deepest box `apply_margins` moves).
pub(crate) fn reached(arena: &NodeArena, slot: u32) -> bool {
    (arena.has_grouped_margins()
        && std::iter::successors(arena.parent(slot), |s| arena.parent(*s))
            .take(4)
            .any(|s| arena.grouped_margins(s).is_some()))
        || crate::rows::in_cells(arena, slot)
}

/// Style `slot` and what it holds again, as deep as the margins reach from
/// a section (a `delete=` row's content, three boxes down): what
/// `apply_margins` gives a box depends on where it stands, and a box's style
/// is otherwise derived only when its own rows change.
pub(crate) fn restyle(arena: &NodeArena, layout: &mut dyn crate::layout::LayoutMirror, slot: u32) {
    fn below(
        arena: &NodeArena,
        layout: &mut dyn crate::layout::LayoutMirror,
        slot: u32,
        depth: u8,
    ) {
        if let Some(node) = arena.taffy(slot) {
            layout.restyle(arena, slot, node);
        }
        if depth > 0 {
            for &child in arena.children(slot) {
                below(arena, layout, child, depth - 1);
            }
        }
    }
    below(arena, layout, slot, 3);
}

/// A sheet metric's points in this process's column; 0 where it has none.
fn sheet(name: &str) -> f32 {
    use crate::style::metrics;
    match metrics::named(name).map(|m| metrics::value(m, metrics::column())) {
        Some(crate::generated::MetricValue::Number(n)) => n as f32,
        _ => 0.0,
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
