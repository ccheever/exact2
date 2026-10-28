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
//! the edge; the title is title-large, 16 from the edge, or 4 after what
//! leads it (56 from the edge after the arrow); the header's buttons are at
//! the trailing edge, 4 from it: an icon button where the button has a
//! symbol, else a text button. Its numbers are the `top-` rows.
//!
//! **The navigation bar** is 80 tall over the gesture bar, whose area it
//! fills with its own colour, `surfaceContainer`. Its destinations share
//! its width, 8 apart: a 24 symbol with its label under it (label-medium),
//! and behind the selected one's symbol the active indicator, a 56 by 32
//! pill in `secondaryContainer`. The selected destination's symbol is
//! `onSecondaryContainer` and its label `secondary`, the others'
//! `onSurfaceVariant`, and a disabled one's that at 38 %
//! (`NavigationBarItemDefaults.colors`). Its numbers are the schema's
//! Material column (the `nav-` rows; `host/android/reference/rows.json`
//! names each one's symbol).
use crate::paint::{Backend, Rect4, Shape};
use crate::presenter::palette::{material, Material};
use crate::presenter::top::look::{paragraph, symbol, Role, Type, LABEL_LARGE, STATE_LAYER};
use crate::text::{Paragraph, RunPaint, TextEngine};
use exact_kernel::generated::metric;
use exact_kernel::style::metrics::{self, Column};
use exact_kernel::ViewId;
use std::rc::Rc;
use tiny_skia::Transform;

const TITLE_LARGE: Type = Type(22.0, 28.0, 400, 0.0);
const LABEL_MEDIUM: Type = Type(12.0, 16.0, 500, 0.5);

/// The top app bar's height, under the status bar.
pub fn top_height() -> f32 {
    dp(metric::TOP_HEIGHT)
}

/// An icon button: a 48 target (the minimum interactive size), its state
/// layer a 40 circle.
const ICON_BUTTON: f32 = 48.0;
const ICON_LAYER: f32 = 40.0;
/// A text button: at least 58 by 40, its label 12 from its sides.
const TEXT_BUTTON: (f32, f32) = (58.0, 40.0);
const TEXT_BUTTON_PADDING: f32 = 12.0;

/// A length of the Material look, dp.
fn dp(index: usize) -> f32 {
    metrics::length_in(index, Column::Material, 0.0, 0.0).expect("a Material row")
}

/// The navigation bar's numbers: its height over the gesture bar, where its
/// labels fit; its symbol; the active indicator, and the room it leaves
/// above and under the symbol; between the indicator and the label; and
/// between two destinations.
struct Nav {
    height: f32,
    icon: f32,
    indicator: (f32, f32),
    indicator_padding: f32,
    label_gap: f32,
    item_gap: f32,
}

fn nav() -> Nav {
    let (icon, indicator) = (
        dp(metric::NAV_ICON),
        (
            dp(metric::NAV_INDICATOR_WIDTH),
            dp(metric::NAV_INDICATOR_HEIGHT),
        ),
    );
    Nav {
        height: dp(metric::NAV_HEIGHT),
        icon,
        indicator,
        indicator_padding: (indicator.1 - icon) / 2.0,
        label_gap: dp(metric::NAV_LABEL_GAP),
        item_gap: dp(metric::NAV_ITEM_GAP),
    }
}

/// A Material role the shell told, else Material's baseline (Compose's
/// `lightColorScheme()` and `darkColorScheme()`), where the twelve roles
/// LLP 1115.001 D4 maps have none of their own for it.
fn told(role: Material, dark: bool, light: u32, night: u32) -> [u8; 4] {
    material(role, dark).unwrap_or_else(|| {
        let [_, r, g, b] = if dark { night } else { light }.to_be_bytes();
        [r, g, b, 255]
    })
}

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
    let nav = nav();
    let content = nav.icon + nav.indicator_padding + nav.label_gap + LABEL_MEDIUM.1 * font_scale;
    let padding = ((nav.height - content) / 2.0).max(nav.indicator_padding);
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
                // Material's shapes (D14): the Up arrow is its `back`.
                Item::Symbol(role, rect, color) => symbol(
                    backend,
                    role,
                    *rect,
                    *color,
                    exact_kernel::SymbolSet::Material,
                ),
            }
        }
    }

    /// Material's small top app bar for `top`.
    pub fn top(engine: &mut TextEngine, top: &Top, inputs: &Inputs) -> Layout {
        let (dark, scale) = (inputs.dark, inputs.font_scale);
        let (y0, h) = (inputs.edge, top_height());
        // What leads and what trails keep `edge` from the bar's sides, and
        // the title has `edge` of its own either side (`TopAppBarLayout`).
        let (edge, icon) = (dp(metric::TOP_EDGE), dp(metric::TOP_ICON));
        let (mut items, mut targets) = (Vec::new(), Vec::new());
        let middle = |size: f32| y0 + (h - size) / 2.0;
        let circle = |x: f32| {
            let at = (x + (ICON_BUTTON - ICON_LAYER) / 2.0, middle(ICON_LAYER));
            Shape::new((at.0, at.1, ICON_LAYER, ICON_LAYER), [ICON_LAYER / 2.0; 4])
        };
        let mut left = inputs.sides.0 + edge;
        let mut right = inputs.width - inputs.sides.1 - edge;
        if top.up {
            let ink = Role::OnSurface.color(dark);
            let at = (left + (ICON_BUTTON - icon) / 2.0, middle(icon), icon, icon);
            items.push(Item::Symbol("back".into(), at, ink));
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
                    let rect = (at + (ICON_BUTTON - icon) / 2.0, middle(icon), icon, icon);
                    items.push(Item::Symbol(role.clone(), rect, ink));
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
        // The title's box is what leads and what trails leave it, its text
        // `edge` inside it. It starts at the inset or after what leads,
        // whichever is further, and moves back from what trails where it
        // would reach it (`TopAppBarLayout`).
        let room = right - left - 2.0 * edge;
        if !top.title.is_empty() && room > 0.0 {
            let p = line(engine, &top.title, TITLE_LARGE, scale, room);
            let x = (inputs.sides.0 + dp(metric::TOP_TITLE_INSET)).max(left) + edge;
            let at = (x.min(right - edge - p.width), middle(p.height));
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
        let (dark, scale, nav) = (inputs.dark, inputs.font_scale, nav());
        let h = nav_height(scale);
        let content = nav.icon + nav.indicator_padding + nav.label_gap + LABEL_MEDIUM.1 * scale;
        let padding = (h - content) / 2.0;
        // The destinations share what the gaps between them leave (the
        // `Row`'s `Arrangement.spacedBy`, each item's `weight(1f)`).
        let span = (inputs.width - inputs.sides.0 - inputs.sides.1).max(0.0);
        let count = bottom.tabs.len().max(1) as f32;
        let cell = ((span - nav.item_gap * (count - 1.0)) / count).max(0.0);
        let inactive = told(Material::OnSurfaceVariant, dark, 0xFF49_454F, 0xFFCA_C4D0);
        let (mut items, mut targets) = (Vec::new(), Vec::new());
        for (index, tab) in bottom.tabs.iter().enumerate() {
            let x = inputs.sides.0 + (cell + nav.item_gap) * index as f32;
            let middle = x + cell / 2.0;
            let (ink, text) = match (tab.enabled, tab.selected) {
                (false, _) => {
                    let mut faded = inactive;
                    faded[3] = (255.0 * 0.38f32).round() as u8;
                    (faded, faded)
                }
                (true, true) => (
                    told(
                        Material::OnSecondaryContainer,
                        dark,
                        0xFF1D_192B,
                        0xFFE8_DEF8,
                    ),
                    told(Material::Secondary, dark, 0xFF62_5B71, 0xFFCC_C2DC),
                ),
                (true, false) => (inactive, inactive),
            };
            let label = (!tab.label.is_empty())
                .then(|| line(engine, &tab.label, LABEL_MEDIUM, scale, cell));
            let pill = (
                middle - nav.indicator.0 / 2.0,
                padding - nav.indicator_padding,
                nav.indicator.0,
                nav.indicator.1,
            );
            let mut layer = Shape::new(pill, [nav.indicator.1 / 2.0; 4]);
            match (&tab.symbol, &label) {
                (Some(role), _) => {
                    if tab.selected {
                        let fill = Role::SecondaryContainer.color(dark);
                        items.push(Item::Fill(layer, fill));
                    }
                    let icon = (middle - nav.icon / 2.0, padding, nav.icon, nav.icon);
                    items.push(Item::Symbol(role.clone(), icon, ink));
                    if let Some(p) = &label {
                        let y = padding + nav.icon + nav.indicator_padding + nav.label_gap;
                        items.push(Item::Text(
                            p.clone(),
                            (middle - p.width / 2.0, y),
                            text,
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
                    items.push(Item::Text(p.clone(), at, text, tab.node));
                }
                (None, None) => {}
            }
            // A touch between two destinations is the nearer one's, as a
            // destination narrower than 48 takes touches past its sides.
            let half = nav.item_gap / 2.0;
            let start = if index == 0 { inputs.sides.0 } else { x - half };
            let end = match index + 1 == bottom.tabs.len() {
                true => inputs.width - inputs.sides.1,
                false => x + cell + half,
            };
            targets.push(Target {
                rect: (start, 0.0, end - start, h),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Three destinations, the first selected and the last disabled, in a
    /// 412 wide window under the light scheme, no palette told.
    fn bar() -> Layout {
        let mut engine = TextEngine::from_font_files(
            &[std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/fixtures/fonts/assets"
            ))],
            "DejaVu Sans",
        );
        let tab = |node, label: &str, selected, enabled| Tab {
            node,
            label: label.into(),
            symbol: Some("star".into()),
            selected,
            enabled,
        };
        let bottom = Bottom {
            tablist: 1,
            tabs: vec![
                tab(2, "Home", true, true),
                tab(3, "Starred", false, true),
                tab(4, "Settings", false, false),
            ],
        };
        let inputs = Inputs {
            dark: false,
            font_scale: 1.0,
            width: 412.0,
            edge: 0.0,
            sides: (0.0, 0.0),
        };
        Layout::bottom(&mut engine, &bottom, &inputs)
    }

    #[test]
    fn a_title_has_the_room_what_leads_and_trails_leave_it() {
        let mut engine = TextEngine::from_font_files(
            &[std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/fixtures/fonts/assets"
            ))],
            "DejaVu Sans",
        );
        let title = "A title nearly as wide as its bar";
        let wide = paragraph(&mut engine, title, TITLE_LARGE, 1.0, None).width;
        let button = Button {
            node: 3,
            title: "Star".into(),
            symbol: Some("star".into()),
            enabled: true,
            destructive: false,
            tint: None,
        };
        let top = Top {
            route: 1,
            heading: 2,
            title: title.into(),
            up: false,
            leading: Vec::new(),
            trailing: vec![button],
            scrolled: false,
        };
        // Compose's room: the bar less 4 and a 48 button at its end, 4 at
        // its start, and the title's own 4 either side; 2 to spare. From
        // the inset (16) instead, the title would be cut.
        let width = wide + 4.0 + 48.0 + 4.0 + 8.0 + 2.0;
        let inputs = Inputs {
            dark: false,
            font_scale: 1.0,
            width,
            edge: 0.0,
            sides: (0.0, 0.0),
        };
        let bar = Layout::top(&mut engine, &top, &inputs);
        let (p, at) = (bar.items.iter())
            .find_map(|item| match item {
                Item::Text(p, at, _, 2) => Some((p.clone(), *at)),
                _ => None,
            })
            .expect("the title");
        assert_eq!(p.width, wide, "not cut");
        // Moved back from the button: it ends 4 before the button's target.
        assert_eq!(at.0 + p.width, width - 4.0 - 48.0 - 4.0);
    }

    #[test]
    fn the_navigation_bar_has_materials_indicator_and_colours() {
        let bar = bar();
        let mut texts = Vec::new();
        let mut symbols = Vec::new();
        let mut fills = Vec::new();
        for item in &bar.items {
            match item {
                Item::Text(_, _, color, node) => texts.push((*node, *color)),
                Item::Symbol(_, rect, color) => symbols.push((*rect, *color)),
                Item::Fill(shape, _) => fills.push(shape.rect),
            }
        }
        // Where no palette was told, Material's baseline: the selected
        // label `secondary`, its symbol `onSecondaryContainer`; the others
        // `onSurfaceVariant`, at 38 % when disabled.
        let variant = [0x49, 0x45, 0x4F, 0xFF];
        assert_eq!(
            texts,
            [
                (2, [0x62, 0x5B, 0x71, 0xFF]),
                (3, variant),
                (4, [0x49, 0x45, 0x4F, 97])
            ]
        );
        assert_eq!(symbols[0].1, [0x1D, 0x19, 0x2B, 0xFF]);
        assert_eq!(symbols[1].1, variant);
        // One indicator, 56 by 32, behind the first symbol: the first cell
        // is (412 − 16) / 3 = 132 wide.
        assert_eq!(fills.len(), 1);
        let (x, _, w, h) = fills[0];
        assert_eq!((w, h), (56.0, 32.0));
        assert_eq!(x + w / 2.0, 66.0);
        assert_eq!(symbols[2].0 .0 + 12.0, 280.0 + 66.0);
    }
}
