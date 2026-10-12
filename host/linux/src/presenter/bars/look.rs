//! Material 3's small top app bar and its navigation bar, measured and
//! painted (LLP 1115.001 D11, D1.3). Painted, not built of framework
//! widgets: each bar is a view of the reader's, and what is in it is drawn
//! here from the reference's own metrics, the Compose Material 3 library's
//! (1.4: `TopAppBar` with `TopAppBarSmallTokens`, `NavigationBar` and
//! `NavigationBarItem` with `NavigationBarTokens`), in the device's colours.
//!
//! Lengths are dp, which are this host's points. Type is sp, a point times
//! the person's font scale, taken linearly, as the top layer's is
//! (`top/look.rs`).
//!
//! **The top app bar** is 64 tall under the status bar, whose area it
//! fills with its own colour: `surface`, and `surfaceContainer` once the
//! route's content has scrolled under it (Material's "on scroll" colour).
//! It does not collapse. The Up arrow is a 24 icon in a 48 target, 4 from
//! the edge; the title is title-large, 16 from the edge, or straight after
//! what leads it (52 from the edge after the arrow); the header's buttons
//! are at the trailing edge, 4 from it: an icon button where the button
//! has a symbol, else a text button.
//!
//! **The navigation bar** is 80 tall over the gesture bar, whose area it
//! fills with its own colour, `surfaceContainer`. Its destinations share
//! its width: a 24 symbol with its label under it (label-medium), and
//! behind the selected one's symbol the active indicator, a 64 by 32 pill
//! in `secondaryContainer`. The selected destination is `onSurface`, the
//! others `onSurfaceVariant`.
use crate::paint::{Backend, Rect4, Shape};
use crate::presenter::top::look::{glyph, paragraph, symbol, Role, Type, LABEL_LARGE, STATE_LAYER};
use crate::text::{Paragraph, RunPaint, TextEngine};
use exact_kernel::ViewId;
use std::rc::Rc;
use tiny_skia::Transform;

const TITLE_LARGE: Type = Type(22.0, 28.0, 400, 0.0);
const LABEL_MEDIUM: Type = Type(12.0, 16.0, 500, 0.5);

/// The top app bar's height, under the status bar.
pub const TOP_HEIGHT: f32 = 64.0;
/// What leads and what trails keep this far from the bar's sides.
const TOP_EDGE: f32 = 4.0;
/// The title's distance from the side with nothing leading it.
const TOP_TITLE_INSET: f32 = 16.0;
/// An icon button: a 24 icon in a 48 target, its state layer a 40 circle.
const ICON: f32 = 24.0;
const ICON_BUTTON: f32 = 48.0;
const ICON_LAYER: f32 = 40.0;
/// A text button: at least 58 by 40, its label 12 from its sides.
const TEXT_BUTTON: (f32, f32) = (58.0, 40.0);
const TEXT_BUTTON_PADDING: f32 = 12.0;

/// The navigation bar's height, over the gesture bar, where its labels fit.
const NAV_HEIGHT: f32 = 80.0;
/// The active indicator, and the room it leaves above and under the symbol.
const NAV_INDICATOR: (f32, f32) = (64.0, 32.0);
const NAV_INDICATOR_PADDING: f32 = 4.0;
/// Between the indicator and the label.
const NAV_LABEL_GAP: f32 = 4.0;

/// Material Symbols' `arrow_back`, on a 24 grid: the Up arrow (D14). The
/// one symbol of Material's own this stage needs; the others are still the
/// symbol table's paths.
const ARROW_BACK: &str = "M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z";

/// A header's button as the top bar shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Button {
    /// The authored `button`.
    pub node: ViewId,
    /// What it says: its title, else its label.
    pub title: String,
    /// Its symbol role, which makes it an icon button.
    pub symbol: Option<String>,
    /// Not `disabled`.
    pub enabled: bool,
    /// `destructive`.
    pub destructive: bool,
    /// The colour its author wrote on it, as it resolves now.
    pub tint: Option<[u8; 4]>,
}

/// The top app bar, as said.
#[derive(Debug, Clone, PartialEq)]
pub struct Top {
    /// The route whose header it is.
    pub route: ViewId,
    /// The heading.
    pub heading: ViewId,
    /// The title.
    pub title: String,
    /// Whether it has the Up arrow: the route is pushed.
    pub up: bool,
    /// The buttons before the heading, and those after it.
    pub leading: Vec<Button>,
    /// See `leading`.
    pub trailing: Vec<Button>,
    /// Whether the route's content has scrolled under it.
    pub scrolled: bool,
}

/// One destination of the navigation bar.
#[derive(Debug, Clone, PartialEq)]
pub struct Tab {
    /// The authored tab.
    pub node: ViewId,
    /// Its label.
    pub label: String,
    /// Its symbol role.
    pub symbol: Option<String>,
    /// `aria-selected`.
    pub selected: bool,
    /// Not `disabled`.
    pub enabled: bool,
}

/// The navigation bar, as said.
#[derive(Debug, Clone, PartialEq)]
pub struct Bottom {
    /// The authored `tablist`.
    pub tablist: ViewId,
    /// Its destinations, in order.
    pub tabs: Vec<Tab>,
}

/// What a bar's look depends on besides what it says.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Inputs {
    /// The scheme is dark.
    pub dark: bool,
    /// The person's font scale.
    pub font_scale: f32,
    /// The window's width, which the bar spans.
    pub width: f32,
    /// The system's bar the bar reaches under and fills with its colour:
    /// the status bar above the top bar, the gesture bar under the
    /// navigation bar.
    pub edge: f32,
    /// The window's safe insets at its left and right (a cutout in
    /// landscape): what is in the bar keeps inside them.
    pub sides: (f32, f32),
}

/// What a touch in a bar is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum What {
    /// The Up arrow: Back, in its order (D6).
    Up,
    /// A header's button, by its authored node.
    Item(ViewId),
    /// A destination, by its authored tab.
    Tab(ViewId),
}

/// One thing in a bar that can be touched.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    /// Its box in the bar, points.
    pub rect: Rect4,
    /// What it is.
    pub what: What,
    /// Whether it takes a press.
    pub enabled: bool,
    layer: (Shape, [u8; 4]),
}

enum Item {
    Text(Rc<Paragraph>, (f32, f32), [u8; 4], ViewId),
    Fill(Shape, [u8; 4]),
    Symbol(String, Rect4, [u8; 4]),
    Glyph(&'static str, Rect4, [u8; 4]),
}

/// A bar measured: its size, its colour and what it draws.
pub struct Layout {
    /// Width and height, points: the system's bar it reaches under included.
    pub size: (f32, f32),
    container: [u8; 4],
    items: Vec<Item>,
    /// What can be touched, in order.
    pub targets: Vec<Target>,
}

/// One line of `style`, cut with an ellipsis where it is wider than `max`.
fn line(engine: &mut TextEngine, text: &str, style: Type, scale: f32, max: f32) -> Rc<Paragraph> {
    let line = paragraph(engine, text, style, scale, None);
    line.ellipsized(max.max(1.0)).unwrap_or(line)
}

/// The navigation bar's height at a font scale: 80, or what its symbol, its
/// label and the indicator's own padding need where that is more (as
/// `NavigationBarItem` places them).
pub fn nav_height(font_scale: f32) -> f32 {
    let content = ICON + NAV_INDICATOR_PADDING + NAV_LABEL_GAP + LABEL_MEDIUM.1 * font_scale;
    let padding = ((NAV_HEIGHT - content) / 2.0).max(NAV_INDICATOR_PADDING);
    content + 2.0 * padding
}

impl Layout {
    /// Each text it draws, by the node it stands for, with its box in the
    /// bar, points: what an accessibility service reads there.
    pub fn texts(&self) -> Vec<(ViewId, Rect4)> {
        (self.items.iter())
            .filter_map(|item| match item {
                Item::Text(p, at, _, source) => Some((*source, (at.0, at.1, p.width, p.height))),
                _ => None,
            })
            .collect()
    }

    /// The target at a point of the bar.
    pub fn target_at(&self, x: f32, y: f32) -> Option<usize> {
        self.targets.iter().position(|t| {
            x >= t.rect.0 && y >= t.rect.1 && x < t.rect.0 + t.rect.2 && y < t.rect.1 + t.rect.3
        })
    }

    /// Paint the bar into `backend`, its top left at the origin: its
    /// colour over all of it, the state layer of the target a finger holds,
    /// then what it says.
    pub fn paint(&self, backend: &mut dyn Backend, engine: &mut TextEngine, lit: Option<usize>) {
        let ts = Transform::identity();
        let all = Shape::rect((0.0, 0.0, self.size.0, self.size.1));
        backend.fill(&all, self.container, ts);
        // The indicator is under the state layer, and both under the symbol.
        for item in &self.items {
            if let Item::Fill(shape, color) = item {
                backend.fill(shape, *color, ts);
            }
        }
        if let Some(target) = lit.and_then(|i| self.targets.get(i)) {
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
                Item::Fill(..) => {}
                Item::Symbol(role, rect, color) => symbol(backend, role, *rect, *color),
                Item::Glyph(d, rect, color) => glyph(backend, d, true, *rect, *color),
            }
        }
    }

    /// Material's small top app bar for `top`.
    pub fn top(engine: &mut TextEngine, top: &Top, inputs: &Inputs) -> Layout {
        let (dark, scale) = (inputs.dark, inputs.font_scale);
        let (y0, h) = (inputs.edge, TOP_HEIGHT);
        let (mut items, mut targets) = (Vec::new(), Vec::new());
        let middle = |size: f32| y0 + (h - size) / 2.0;
        let circle = |x: f32| {
            let at = (x + (ICON_BUTTON - ICON_LAYER) / 2.0, middle(ICON_LAYER));
            Shape::new((at.0, at.1, ICON_LAYER, ICON_LAYER), [ICON_LAYER / 2.0; 4])
        };
        let mut left = inputs.sides.0 + TOP_EDGE;
        let mut right = inputs.width - inputs.sides.1 - TOP_EDGE;
        if top.up {
            let ink = Role::OnSurface.color(dark);
            let icon = (left + (ICON_BUTTON - ICON) / 2.0, middle(ICON), ICON, ICON);
            items.push(Item::Glyph(ARROW_BACK, icon, ink));
            targets.push(Target {
                rect: (left, middle(ICON_BUTTON), ICON_BUTTON, ICON_BUTTON),
                what: What::Up,
                enabled: true,
                layer: (circle(left), ink),
            });
            left += ICON_BUTTON;
        }
        // A button's box is its 48 target; what it draws sits in the middle.
        let mut button = |b: &Button, x: Option<f32>, right: &mut f32, left: &mut f32| {
            let ink = match (b.enabled, b.destructive, b.tint, b.symbol.is_some()) {
                (false, ..) => Role::Disabled.color(dark),
                (true, _, Some(tint), _) => tint,
                (true, true, ..) => Role::Error.color(dark),
                (true, false, None, true) => Role::OnSurfaceVariant.color(dark),
                (true, false, None, false) => Role::Primary.color(dark),
            };
            let label = (b.symbol.is_none()).then(|| {
                let room = (inputs.width / 2.0 - 2.0 * TEXT_BUTTON_PADDING).max(1.0);
                line(engine, &b.title, LABEL_LARGE, scale, room)
            });
            let w = match &label {
                Some(p) => (p.width + 2.0 * TEXT_BUTTON_PADDING).max(TEXT_BUTTON.0),
                None => ICON_BUTTON,
            };
            // Leading: placed at `x` and the line moves on. Trailing: it
            // ends where the one after it begins.
            let at = match x {
                Some(x) => {
                    *left = x + w;
                    x
                }
                None => {
                    *right -= w;
                    *right
                }
            };
            let layer = match (&b.symbol, &label) {
                (Some(role), _) => {
                    let icon = (at + (ICON_BUTTON - ICON) / 2.0, middle(ICON), ICON, ICON);
                    items.push(Item::Symbol(role.clone(), icon, ink));
                    circle(at)
                }
                (None, Some(p)) => {
                    let tall = (p.height + 20.0).max(TEXT_BUTTON.1);
                    let text = (at + (w - p.width) / 2.0, middle(p.height));
                    items.push(Item::Text(p.clone(), text, ink, b.node));
                    Shape::new((at, middle(tall), w, tall), [tall / 2.0; 4])
                }
                (None, None) => circle(at),
            };
            targets.push(Target {
                rect: (at, middle(ICON_BUTTON), w, ICON_BUTTON),
                what: What::Item(b.node),
                enabled: b.enabled,
                layer: (layer, ink),
            });
        };
        let led = top.up || !top.leading.is_empty();
        for b in &top.leading {
            let x = left;
            button(b, Some(x), &mut right, &mut left);
        }
        for b in top.trailing.iter().rev() {
            button(b, None, &mut right, &mut left);
        }
        // The trailing buttons were measured last to first: back in the
        // order the author wrote them, after the leading ones.
        let first = targets.len() - top.trailing.len();
        targets[first..].reverse();
        // After an icon button the title starts where its target ends (the
        // target's own 12 is the gap); a leading text button gets 4 more.
        let text_led = top.leading.last().is_some_and(|b| b.symbol.is_none());
        let x = match (led, text_led) {
            (false, _) => inputs.sides.0 + TOP_TITLE_INSET,
            (true, false) => left,
            (true, true) => left + TOP_EDGE,
        };
        let room = right - x - if top.trailing.is_empty() { 12.0 } else { 0.0 };
        if !top.title.is_empty() && room > 0.0 {
            let p = line(engine, &top.title, TITLE_LARGE, scale, room);
            let at = (x, middle(p.height));
            items.push(Item::Text(p, at, Role::OnSurface.color(dark), top.heading));
        }
        let container = match top.scrolled {
            true => Role::SurfaceContainer,
            false => Role::Surface,
        };
        Layout {
            size: (inputs.width, y0 + h),
            container: container.color(dark),
            items,
            targets,
        }
    }

    /// Material's navigation bar for `bottom`.
    pub fn bottom(engine: &mut TextEngine, bottom: &Bottom, inputs: &Inputs) -> Layout {
        let (dark, scale) = (inputs.dark, inputs.font_scale);
        let h = nav_height(scale);
        let content = ICON + NAV_INDICATOR_PADDING + NAV_LABEL_GAP + LABEL_MEDIUM.1 * scale;
        let padding = (h - content) / 2.0;
        let span = (inputs.width - inputs.sides.0 - inputs.sides.1).max(0.0);
        let cell = span / bottom.tabs.len().max(1) as f32;
        let (mut items, mut targets) = (Vec::new(), Vec::new());
        for (index, tab) in bottom.tabs.iter().enumerate() {
            let x = inputs.sides.0 + cell * index as f32;
            let middle = x + cell / 2.0;
            let ink = match (tab.enabled, tab.selected) {
                (false, _) => Role::Disabled,
                (true, true) => Role::OnSurface,
                (true, false) => Role::OnSurfaceVariant,
            }
            .color(dark);
            let label = (!tab.label.is_empty())
                .then(|| line(engine, &tab.label, LABEL_MEDIUM, scale, cell - 8.0));
            let pill = (
                middle - NAV_INDICATOR.0 / 2.0,
                padding - NAV_INDICATOR_PADDING,
                NAV_INDICATOR.0,
                NAV_INDICATOR.1,
            );
            let mut layer = Shape::new(pill, [NAV_INDICATOR.1 / 2.0; 4]);
            match (&tab.symbol, &label) {
                (Some(role), _) => {
                    if tab.selected {
                        let fill = Role::SecondaryContainer.color(dark);
                        items.push(Item::Fill(layer, fill));
                    }
                    let icon = (middle - ICON / 2.0, padding, ICON, ICON);
                    items.push(Item::Symbol(role.clone(), icon, ink));
                    if let Some(p) = &label {
                        let y = padding + ICON + NAV_INDICATOR_PADDING + NAV_LABEL_GAP;
                        items.push(Item::Text(
                            p.clone(),
                            (middle - p.width / 2.0, y),
                            ink,
                            tab.node,
                        ));
                    }
                }
                // A destination with no symbol is its label, in the middle.
                (None, Some(p)) => {
                    let at = (middle - p.width / 2.0, (h - p.height) / 2.0);
                    let around = (at.0 - 12.0, at.1 - 8.0, p.width + 24.0, p.height + 16.0);
                    layer = Shape::new(around, [around.3 / 2.0; 4]);
                    if tab.selected {
                        let fill = Role::SecondaryContainer.color(dark);
                        items.push(Item::Fill(layer, fill));
                    }
                    items.push(Item::Text(p.clone(), at, ink, tab.node));
                }
                (None, None) => {}
            }
            targets.push(Target {
                rect: (x, 0.0, cell, h),
                what: What::Tab(tab.node),
                enabled: tab.enabled,
                layer: (layer, Role::OnSurface.color(dark)),
            });
        }
        Layout {
            size: (inputs.width, h + inputs.edge),
            container: Role::SurfaceContainer.color(dark),
            items,
            targets,
        }
    }
}
