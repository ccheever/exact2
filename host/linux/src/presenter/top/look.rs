//! Material 3's basic dialog and its menu, measured and painted (LLP
//! 1115.001 D10, D1.3): the two surfaces the top layer shows on Android.
//! Painted, not built of framework widgets: the window is the platform's
//! (`Dialog`, `PopupWindow`), and what is in it is drawn here from the
//! reference's own metrics, the Compose Material 3 library's (1.4:
//! `AlertDialog`, `DropdownMenu`, `DropdownMenuItem`), in the device's
//! colours.
//!
//! Lengths are dp, which are this host's points. Type is sp, a point times
//! the person's font scale, taken linearly: Android's own scaling is
//! non-linear above 100%, and comes with the reported type ramp (D4, S2).
//! The face is the page's default family.
use super::shape::{Plan, Row};
use crate::paint::{rgba, Backend, Ink, Rect4, Shape, SvgPaint};
use crate::text::{Paragraph, RunPaint, TextEngine};
use exact_kernel::style::roles;
use exact_kernel::{ColorValue, StyleProps, TextAlign, ViewId, WhiteSpace};
use std::rc::Rc;
use tiny_skia::Transform;

/// A Material colour role, read through the role of Exact's that LLP
/// 1115.001 D4's table gives it (`presenter/palette.rs`): the device's
/// colour where its shell reports a palette, the table's pair elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::presenter) enum Role {
    Surface,
    SurfaceContainer,
    SurfaceContainerHigh,
    OnSurface,
    OnSurfaceVariant,
    /// `onSurface` at 38%: Material's disabled content.
    Disabled,
    Primary,
    SecondaryContainer,
    Error,
    OutlineVariant,
}

impl Role {
    pub(in crate::presenter) fn color(self, dark: bool) -> [u8; 4] {
        let name = match self {
            Role::Surface => "Canvas",
            Role::SurfaceContainer => "-exact-secondary-grouped-background",
            Role::SurfaceContainerHigh => "-exact-tertiary-grouped-background",
            Role::OnSurface => "-exact-label",
            Role::OnSurfaceVariant => "-exact-secondary-label",
            Role::Disabled => "-exact-tertiary-label",
            Role::Primary => "AccentColor",
            Role::SecondaryContainer => "ButtonFace",
            Role::Error => "-exact-system-red",
            Role::OutlineVariant => "-exact-separator",
        };
        roles::role(name).map_or([0, 0, 0, 255], |id| {
            rgba(ColorValue::Role(id).resolve(dark))
        })
    }
}

/// A Material type style: size and line height in sp, weight, tracking in sp.
#[derive(Clone, Copy)]
pub(in crate::presenter) struct Type(pub f32, pub f32, pub u16, pub f32);
const HEADLINE_SMALL: Type = Type(24.0, 32.0, 400, 0.0);
const BODY_MEDIUM: Type = Type(14.0, 20.0, 400, 0.25);
pub(in crate::presenter) const LABEL_LARGE: Type = Type(14.0, 20.0, 500, 0.1);

/// The dialog's padding on every side, and the space under its headline
/// and under its supporting text.
const DIALOG_PADDING: f32 = 24.0;
const DIALOG_TITLE_GAP: f32 = 16.0;
const DIALOG_TEXT_GAP: f32 = 24.0;
/// The dialog is 280 to 560 wide, and keeps this far from the window's sides.
const DIALOG_WIDTH: (f32, f32) = (280.0, 560.0);
const DIALOG_MARGIN: f32 = 24.0;
/// A text button: at least 58 by 40, its label 12 from its sides; 8 between
/// two on a line, 12 between two lines.
const BUTTON_MIN: (f32, f32) = (58.0, 40.0);
const BUTTON_PADDING: f32 = 12.0;
const BUTTON_GAP: (f32, f32) = (8.0, 12.0);
/// The menu is 112 to 280 wide, 8 above its first row and under its last;
/// a row is 48 tall, its content 12 from its sides, a symbol 24 with 12
/// after it; a separator is a 1 dp line with 8 above and under it.
const MENU_WIDTH: (f32, f32) = (112.0, 280.0);
const MENU_PADDING: f32 = 8.0;
const MENU_ROW: f32 = 48.0;
const MENU_INSET: f32 = 12.0;
const MENU_SYMBOL: f32 = 24.0;
const MENU_RULE: (f32, f32) = (1.0, 8.0);
/// Material's state layer under a press or a keyboard's focus.
pub(in crate::presenter) const STATE_LAYER: f32 = 0.10;

/// What a surface's look depends on besides what it says.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Inputs {
    /// The scheme is dark.
    pub dark: bool,
    /// The person's font scale.
    pub font_scale: f32,
    /// The window it is shown over, points.
    pub window: (f32, f32),
}

/// One thing a surface draws, in its own points.
enum Item {
    Text(Rc<Paragraph>, (f32, f32), [u8; 4], ViewId),
    Rule(Rect4, [u8; 4]),
    Symbol(String, Rect4, [u8; 4]),
}

/// A row as it can be touched: its box, the authored node it stands for,
/// and the state layer a press shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    /// Its box in the surface, points.
    pub rect: Rect4,
    /// The authored `button`.
    pub node: ViewId,
    /// Whether it takes a press.
    pub enabled: bool,
    layer: (Shape, [u8; 4]),
}

/// A surface measured: its size, its container and what it draws.
pub struct Layout {
    /// Width and height, points.
    pub size: (f32, f32),
    /// The container's corner radius, and its elevation (a shadow the
    /// window casts): 28 and none for a dialog, 4 and 3 for a menu.
    pub radius: f32,
    /// See `radius`.
    pub elevation: f32,
    container: [u8; 4],
    items: Vec<Item>,
    /// The rows, in the plan's order.
    pub targets: Vec<Target>,
}

pub(in crate::presenter) fn paragraph(
    engine: &mut TextEngine,
    text: &str,
    Type(size, line, weight, tracking): Type,
    scale: f32,
    width: Option<f32>,
) -> Rc<Paragraph> {
    let style = StyleProps {
        font_size: size * scale,
        font_weight: weight,
        line_height: exact_kernel::LineHeight::Length(line * scale),
        letter_spacing: tracking * scale,
        white_space: match width {
            Some(_) => WhiteSpace::Normal,
            None => WhiteSpace::Nowrap,
        },
        text_align: TextAlign::Left,
        ..Default::default()
    };
    engine.paragraph(&crate::paint::text_spec(&style, text), width)
}

/// One line, cut with an ellipsis where it is wider than `max`.
fn label(engine: &mut TextEngine, text: &str, scale: f32, max: f32) -> Rc<Paragraph> {
    let line = paragraph(engine, text, LABEL_LARGE, scale, None);
    line.ellipsized(max).unwrap_or(line)
}

impl Layout {
    /// Each text it draws, by the node it stands for, with its box in the
    /// surface, points: what an accessibility service reads there.
    pub fn texts(&self) -> Vec<(ViewId, Rect4)> {
        (self.items.iter())
            .filter_map(|item| match item {
                Item::Text(p, at, _, source) => Some((*source, (at.0, at.1, p.width, p.height))),
                _ => None,
            })
            .collect()
    }

    /// Measure `plan` for `inputs`.
    pub fn of(engine: &mut TextEngine, plan: &Plan, pop: ViewId, inputs: &Inputs) -> Layout {
        match plan {
            Plan::Alert {
                title,
                message,
                rows,
                ..
            } => alert(
                engine,
                (title.as_deref(), message.as_deref()),
                rows,
                pop,
                inputs,
            ),
            Plan::Menu(entries) => menu(engine, entries, inputs),
        }
    }

    /// The row at a point of the surface.
    pub fn target_at(&self, x: f32, y: f32) -> Option<usize> {
        self.targets.iter().position(|t| {
            x >= t.rect.0 && y >= t.rect.1 && x < t.rect.0 + t.rect.2 && y < t.rect.1 + t.rect.3
        })
    }

    /// Paint the surface into `backend`, its top left at the origin:
    /// the container, the state layer of each row in `lit` (the one
    /// pressed, the one a keyboard focused), then what it says.
    pub fn paint(&self, backend: &mut dyn Backend, engine: &mut TextEngine, lit: &[usize]) {
        let ts = Transform::identity();
        let container = Shape::new((0.0, 0.0, self.size.0, self.size.1), [self.radius; 4]);
        backend.fill(&container, self.container, ts);
        for target in lit.iter().filter_map(|i| self.targets.get(*i)) {
            let (shape, mut tint) = target.layer;
            tint[3] = (f32::from(tint[3]) * STATE_LAYER).round() as u8;
            backend.fill(&shape, tint, ts);
        }
        for item in &self.items {
            match item {
                Item::Text(paragraph, at, color, source) => {
                    let paint = [RunPaint {
                        color: *color,
                        source: *source,
                    }];
                    backend.text(engine, paragraph, &paint, *at, ts);
                }
                Item::Rule(rect, color) => backend.fill(&Shape::rect(*rect), *color, ts),
                Item::Symbol(role, rect, color) => symbol(backend, role, *rect, *color),
            }
        }
    }
}

/// A symbol role's path in `rect`, as the page draws one at a regular
/// weight (`paint/svg.rs`): stroked, or filled for a `-fill` role.
pub(in crate::presenter) fn symbol(
    backend: &mut dyn Backend,
    role: &str,
    rect: Rect4,
    color: [u8; 4],
) {
    // A role, an icon token's form, or `sf/` a role's Apple name (LLP 1121 D10).
    let Some(path) = exact_kernel::symbol_path(role) else {
        return;
    };
    for d in path.d.split(';') {
        glyph_in(
            backend,
            d,
            path.filled,
            path.even_odd,
            path.view_box,
            rect,
            color,
        );
    }
}

/// A 24-unit path in `rect`: stroked as a symbol role's is, or filled.
pub(in crate::presenter) fn glyph(
    backend: &mut dyn Backend,
    d: &str,
    filled: bool,
    rect: Rect4,
    color: [u8; 4],
) {
    glyph_in(backend, d, filled, true, "0 0 24 24", rect, color);
}

/// A path in `rect` under its viewBox, stroked in a role's 24 units, or filled.
fn glyph_in(
    backend: &mut dyn Backend,
    d: &str,
    filled: bool,
    even_odd: bool,
    view_box: &str,
    rect: Rect4,
    color: [u8; 4],
) {
    let [_, _, bw, _] = crate::paint::view_box(view_box);
    let path = exact_kernel::svg::parse_d(d);
    let paint = SvgPaint {
        path: &path,
        fill: filled.then_some(Ink::Solid(color)),
        even_odd,
        stroke: (!filled).then_some(Ink::Solid(color)),
        order: [0, 1, 2],
        width: (1.1 + 300.0 / 400.0) * bw / 24.0,
        cap: 1,
        join: 1,
        miter: 4.0,
        dash: Vec::new(),
        phase: 0.0,
    };
    let ts = crate::paint::fitted(view_box, rect);
    backend.svg_path(&paint, ts);
}

/// Material's basic dialog: the headline, the supporting text, and the
/// buttons as text buttons flowing from the trailing edge, a second line
/// where they do not fit one.
fn alert(
    engine: &mut TextEngine,
    (title, message): (Option<&str>, Option<&str>),
    rows: &[Row],
    pop: ViewId,
    inputs: &Inputs,
) -> Layout {
    let (dark, scale) = (inputs.dark, inputs.font_scale);
    let room = inputs.window.0 - 2.0 * DIALOG_MARGIN;
    let width = room.clamp(DIALOG_WIDTH.0.min(inputs.window.0), DIALOG_WIDTH.1);
    let inner = (width - 2.0 * DIALOG_PADDING).max(1.0);
    let mut items = Vec::new();
    let mut y = DIALOG_PADDING;
    for (text, style, role, gap) in [
        (title, HEADLINE_SMALL, Role::OnSurface, DIALOG_TITLE_GAP),
        (
            message,
            BODY_MEDIUM,
            Role::OnSurfaceVariant,
            DIALOG_TEXT_GAP,
        ),
    ] {
        let Some(text) = text else { continue };
        let p = paragraph(engine, text, style, scale, Some(inner));
        let height = p.height;
        items.push(Item::Text(p, (DIALOG_PADDING, y), role.color(dark), pop));
        y += height + gap;
    }
    // Each button's label and its box's size, then lines of them.
    let buttons: Vec<(Rc<Paragraph>, f32, f32)> = (rows.iter())
        .map(|row| {
            let p = label(engine, &row.title, scale, inner - 2.0 * BUTTON_PADDING);
            let w = (p.width + 2.0 * BUTTON_PADDING).max(BUTTON_MIN.0);
            let h = (p.height + 20.0).max(BUTTON_MIN.1);
            (p, w.min(inner), h)
        })
        .collect();
    let mut lines: Vec<Vec<usize>> = vec![Vec::new()];
    let mut used = 0.0;
    for (i, (_, w, _)) in buttons.iter().enumerate() {
        let fits = used + w + if used > 0.0 { BUTTON_GAP.0 } else { 0.0 } <= inner + 0.01;
        if !fits && used > 0.0 {
            lines.push(Vec::new());
            used = 0.0;
        }
        used += w + if used > 0.0 { BUTTON_GAP.0 } else { 0.0 };
        lines.last_mut().expect("a line").push(i);
    }
    let mut targets = vec![None; rows.len()];
    for (n, line) in lines.iter().enumerate() {
        let total: f32 = line.iter().map(|i| buttons[*i].1).sum::<f32>()
            + BUTTON_GAP.0 * line.len().saturating_sub(1) as f32;
        let tall = line.iter().map(|i| buttons[*i].2).fold(0.0, f32::max);
        let mut x = DIALOG_PADDING + inner - total;
        y += if n > 0 { BUTTON_GAP.1 } else { 0.0 };
        for &i in line {
            let (p, w, h) = &buttons[i];
            let row = &rows[i];
            let rect = (x, y + (tall - h) / 2.0, *w, *h);
            let ink = match (row.enabled, row.destructive) {
                (false, _) => Role::Disabled,
                (true, true) => Role::Error,
                (true, false) => Role::Primary,
            }
            .color(dark);
            let at = (rect.0 + (w - p.width) / 2.0, rect.1 + (h - p.height) / 2.0);
            items.push(Item::Text(p.clone(), at, ink, row.node));
            targets[i] = Some(Target {
                rect,
                node: row.node,
                enabled: row.enabled,
                layer: (Shape::new(rect, [h / 2.0; 4]), ink),
            });
            x += w + BUTTON_GAP.0;
        }
        y += tall;
    }
    Layout {
        size: (width, y + DIALOG_PADDING),
        radius: 28.0,
        elevation: 0.0,
        container: Role::SurfaceContainerHigh.color(dark),
        items,
        targets: targets.into_iter().flatten().collect(),
    }
}

/// Material's menu: 48 dp rows, each a label with a leading symbol where
/// its button has one (a check where it is `aria-checked`), a destructive
/// row in the error role, and a line where the author wrote a separator.
fn menu(engine: &mut TextEngine, entries: &[Option<Row>], inputs: &Inputs) -> Layout {
    let (dark, scale) = (inputs.dark, inputs.font_scale);
    let widest = MENU_WIDTH.1.min(inputs.window.0);
    let lead = |row: &Row| match (&row.symbol, row.checked) {
        (Some(symbol), _) => Some(symbol.clone()),
        (None, true) => Some("checkmark".to_string()),
        (None, false) => None,
    };
    let indent = |row: &Row| match lead(row) {
        Some(_) => MENU_SYMBOL + MENU_INSET,
        None => 0.0,
    };
    let labels: Vec<Option<Rc<Paragraph>>> = (entries.iter())
        .map(|entry| {
            let row = entry.as_ref()?;
            let room = widest - 2.0 * MENU_INSET - indent(row);
            Some(label(engine, &row.title, scale, room.max(1.0)))
        })
        .collect();
    let content = (entries.iter().zip(&labels))
        .filter_map(|(entry, p)| Some(indent(entry.as_ref()?) + p.as_ref()?.width))
        .fold(0.0, f32::max);
    let width = (content + 2.0 * MENU_INSET).clamp(MENU_WIDTH.0.min(widest), widest);
    let (mut items, mut targets) = (Vec::new(), Vec::new());
    let mut y = MENU_PADDING;
    for (entry, p) in entries.iter().zip(labels) {
        let (Some(row), Some(p)) = (entry, p) else {
            let line = (0.0, y + MENU_RULE.1, width, MENU_RULE.0);
            items.push(Item::Rule(line, Role::OutlineVariant.color(dark)));
            y += MENU_RULE.0 + 2.0 * MENU_RULE.1;
            continue;
        };
        let tall = (p.height + 16.0).max(MENU_ROW);
        let (ink, tint) = match (row.enabled, row.destructive) {
            (false, _) => (Role::Disabled, Role::Disabled),
            (true, true) => (Role::Error, Role::Error),
            (true, false) => (Role::OnSurface, Role::OnSurfaceVariant),
        };
        if let Some(role) = lead(row) {
            let at = (MENU_INSET, y + (tall - MENU_SYMBOL) / 2.0);
            let rect = (at.0, at.1, MENU_SYMBOL, MENU_SYMBOL);
            items.push(Item::Symbol(role, rect, tint.color(dark)));
        }
        let at = (MENU_INSET + indent(row), y + (tall - p.height) / 2.0);
        items.push(Item::Text(p, at, ink.color(dark), row.node));
        let rect = (0.0, y, width, tall);
        targets.push(Target {
            rect,
            node: row.node,
            enabled: row.enabled,
            layer: (Shape::rect(rect), Role::OnSurface.color(dark)),
        });
        y += tall;
    }
    Layout {
        size: (width, y + MENU_PADDING),
        radius: 4.0,
        elevation: 3.0,
        container: Role::SurfaceContainer.color(dark),
        items,
        targets,
    }
}

/// The corner of a menu at `at` that its motion grows from: the one at its
/// anchor, where Compose's `DropdownMenu` puts its transform origin (to the
/// corner). Bit 0: its right side, for a menu that ends at its anchor's
/// end; bit 1: its bottom, for one that opens upward.
pub fn origin(anchor: Rect4, at: (f32, f32), size: (f32, f32)) -> u32 {
    let right = at.0 + size.0 / 2.0 < anchor.0 + anchor.2 / 2.0;
    let bottom = at.1 + size.1 / 2.0 < anchor.1 + anchor.3 / 2.0;
    u32::from(right) | u32::from(bottom) << 1
}

/// Where a menu of `size` goes for an anchor's box, all in the window's
/// points: Compose's `DropdownMenuPositionProvider`. Along the row it
/// starts at the anchor's start, else ends at its end, else sits against
/// the window's far side; down the page it hangs under the anchor, else
/// sits over it, else is centred on its top, else rests at the bottom,
/// never closer than `margins` (top, bottom) to the window's edges.
pub fn place(
    anchor: Rect4,
    size: (f32, f32),
    window: (f32, f32),
    margins: (f32, f32),
) -> (f32, f32) {
    let (w, h) = size;
    let across = [anchor.0, anchor.0 + anchor.2 - w, window.0 - w];
    let x = (across.iter().copied())
        .find(|x| *x >= 0.0 && x + w <= window.0)
        .unwrap_or(across[1]);
    let down = [
        (anchor.1 + anchor.3).max(margins.0),
        anchor.1 - h,
        anchor.1 - h / 2.0,
        window.1 - h - margins.1,
    ];
    let y = (down.iter().copied())
        .find(|y| *y >= margins.0 && y + h <= window.1 - margins.1)
        .unwrap_or(down[1]);
    (x.max(0.0), y.max(0.0))
}
