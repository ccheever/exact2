//! Colours that name a platform colour (LLP 1095): a role from the schema's
//! table (D2), or `-exact-platform-color()` (D3). Each carries a fallback pair, so
//! the kernel and any host without the platform's colour always have a
//! deterministic RGBA; a host that has it resolves the name per view.

use super::{Color, ColorValue};
use crate::generated::{ColorRole, COLOR_ROLES};
use crate::gradient::ColorText;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

/// The role a keyword names, in any ASCII case: CSS's system colours by
/// their names, Exact's roles as `-exact-<role>` (LLP 1081 D2), WebKit's
/// `-apple-system-*` names for those it has, and the web form a role is
/// written as (`var(--exact-<role>, …)`), so canonical CSS reads back. An
/// Exact role's bare name is not a colour.
pub fn role(text: &str) -> Option<u8> {
    let text = text.trim();
    let lower = text.to_ascii_lowercase();
    // The tint as the web writes it, `var(--exact-accent, <name>)`, reads
    // back as the named role (`reference_css`).
    if let Some(name) = lower
        .strip_prefix("var(--exact-accent,")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return role(name).filter(|&id| role_of(id).is_some_and(web_accent));
    }
    let var = lower.strip_prefix("var(--exact-");
    let exact = match var {
        Some(rest) => Some(&rest[..rest.find([',', ')'])?]),
        None => lower.strip_prefix("-exact-"),
    };
    COLOR_ROLES
        .iter()
        .position(|r| match exact {
            // A CSS colour the browser draws in the accent is written under
            // its own name, `var(--exact-linktext, …)`, and only so.
            Some(name) if is_css_system(r) => {
                var.is_some() && web_accent(r) && r.name.eq_ignore_ascii_case(name)
            }
            Some(name) => r.name.eq_ignore_ascii_case(name),
            None => {
                (is_css_system(r) && r.name.eq_ignore_ascii_case(text))
                    || (!r.alias.is_empty() && r.alias.eq_ignore_ascii_case(text))
            }
        })
        .and_then(|i| u8::try_from(i).ok())
}

/// A role by id; `None` past the table. Every id [`role`], the wire decoder
/// and the schema give is inside it, and a host's report is checked
/// ([`is_known_reference`]); another id names no role (never a wrapped-onto
/// one), and its fallback is transparent.
pub fn role_of(id: u8) -> Option<&'static ColorRole> {
    COLOR_ROLES.get(usize::from(id))
}

/// Whether `c` names a role or an interned `-exact-platform-color()` that exists.
pub fn is_known_reference(c: ColorValue) -> bool {
    match c {
        ColorValue::Role(id) => usize::from(id) < COLOR_ROLES.len(),
        ColorValue::Platform(id) => platform(id).is_some(),
        _ => false,
    }
}

/// Whether a role is one of CSS's system colours (the web writes it as is).
pub fn is_css_system(role: &ColorRole) -> bool {
    role.name.starts_with(|c: char| c.is_ascii_uppercase())
}

/// The role table's two fallback columns (LLP 1115.000 D3.2): what a role
/// is where no host reported the platform's own colour for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    /// Apple's measured pairs: what an Apple host shows before it reports
    /// its own, and what every host shows until it says otherwise.
    Apple,
    /// The neutral pairs, no platform's palette: the web's under the
    /// neutral look (D4), a host's that paints its own controls (D17).
    Neutral,
}

/// The column this process shows. One platform per process, so one switch.
static NEUTRAL: AtomicBool = AtomicBool::new(false);

/// The one switch between the columns: a host says which it shows, once,
/// before it presents (LLP 1115.000 D4 part B for the web, D17 for a host
/// that paints its own controls). Every fallback follows: what the kernel
/// resolves itself, the pair an Exact role carries to the browser
/// (`var(--exact-<role>, …)`), what a painter reads. The look's metrics
/// follow too (`style::metrics::column`), so this is the look's one switch.
/// A reported colour still wins over either column. The web calls it, for a
/// page in the neutral look, which every web build draws since 2026-10-12
/// (host/web/look.mjs), and so do the Linux desktop and Windows, which have
/// no platform toolkit under their painter; every other host shows Apple's
/// pairs, in which a process starts.
pub fn set_column(column: Column) {
    NEUTRAL.store(column == Column::Neutral, Ordering::Release);
}

/// The column this process shows ([`set_column`]).
pub fn column() -> Column {
    if NEUTRAL.load(Ordering::Acquire) {
        Column::Neutral
    } else {
        Column::Apple
    }
}

/// A role's light and dark values in a column (transparent for no role).
pub fn role_pair(id: u8, column: Column) -> ColorValue {
    role_of(id).map_or(ColorValue::Fixed(Color::TRANSPARENT), |r| match column {
        Column::Apple => ColorValue::LightDark(Color(r.light), Color(r.dark)),
        Column::Neutral => ColorValue::LightDark(Color(r.neutral_light), Color(r.neutral_dark)),
    })
}

/// A role's fallback: its light and dark values in this process's column
/// (transparent for no role).
pub fn role_fallback(id: u8) -> ColorValue {
    role_pair(id, column())
}

/// One `-exact-platform-color()`: a native name per platform, and the fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformColor {
    /// `UIColor`'s class property, or `named:<Asset>`.
    pub ios: Option<Box<str>>,
    /// `NSColor`'s class property, or `named:<Asset>`.
    pub macos: Option<Box<str>>,
    /// A CSS colour for the web.
    pub web: Option<Box<str>>,
    /// What every other host shows: a colour or a `light-dark()` pair.
    pub fallback: ColorValue,
    /// The function as written, canonical: what the wire carries.
    pub text: Box<str>,
}

/// What a host reported each reference resolves to (LLP 1095 D1), by
/// reference and appearance: a host with the platform's colours resolves
/// every reference under the traits it is showing (style, Increased
/// Contrast) and reports the whole set again whenever they change, so what
/// the kernel resolves itself — paint motion, gradients, SVG scenes and
/// filters — is the platform's colour, not a snapshot of the fallback. The
/// Apple hosts report their named colours; Android reports the device's
/// Material colours, role by role (LLP 1115.001 D4). A host that reports
/// nothing (desktop Linux, headless) keeps the fallback pair.
/// One platform per process, so one table.
static REPORTED: RwLock<BTreeMap<(u8, u16, bool), Color>> = RwLock::new(BTreeMap::new());

fn reference_key(c: ColorValue) -> Option<(u8, u16)> {
    match c {
        ColorValue::Role(id) => Some((0, u16::from(id))),
        ColorValue::Platform(id) => Some((1, id)),
        _ => None,
    }
}

/// The colour a host reported for a reference under an appearance.
pub fn reported(c: ColorValue, dark: bool) -> Option<Color> {
    let (kind, id) = reference_key(c)?;
    REPORTED.read().ok()?.get(&(kind, id, dark)).copied()
}

/// How many times the reported table has changed. The table is the
/// process's, but what was resolved from it is each session's: a session
/// re-presents when the generation moved since it last looked, so a second
/// session's identical report still refreshes it.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The reported table's generation (see [`set_reported`]).
pub fn reported_generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

/// Whether a role is the tint itself on an Apple platform (`@tint`, at full
/// strength: on iOS `AccentColor`, a link and a button's text; on the Mac
/// `AccentColor`), which resolves to the nearest `accent-color` a node or an
/// ancestor sets, else the platform's (LLP 1116.001 D2; LLP 1115.000 D3.4
/// for the Mac), as UIKit's inherited `tintColor` does: iOS sets an
/// authored accent as the view's tint, and the Mac's presenter reads the
/// nearest one up the view chain. The web's own two are [`web_accent`].
pub fn follows_accent(id: u8, macos: bool) -> bool {
    role_of(id).is_some_and(|r| if macos { r.macos } else { r.ios } == "@tint")
}

/// Whether the browser draws a role in the accent: CSS's `AccentColor`,
/// and `LinkText` (LLP 1115.000 D3.5), so a link is one blue on the web,
/// the plain button's and the tab bar's, where the browser's own
/// `LinkText` is a second; and an Exact role that is the tint on iOS
/// (`-exact-link`, and `-exact-symbol`, D3.9: the web draws a symbol nobody
/// tinted in its accent). Each reads the nearest `accent-color`
/// (`--exact-accent`, LLP 1116.001 D2), else the browser's accent. CSS's
/// `ButtonText` is the tint on iOS and the browser's own keyword here.
pub fn web_accent(r: &ColorRole) -> bool {
    matches!(r.name, "AccentColor" | "LinkText") || (!is_css_system(r) && r.ios == "@tint")
}

/// Whether a role is the app's tint (`@tint` on a platform: `AccentColor`,
/// `Highlight`), which a host reports app-wide, one tint per process.
pub fn is_tint_role(id: u8) -> bool {
    role_of(id).is_some_and(|r| r.ios.starts_with("@tint") || r.macos.starts_with("@tint"))
}

/// Replace every reported resolution with `entries` (a reference, an
/// appearance, its colour); the table's generation after, which moves only
/// when anything resolves differently. A report naming no tint role keeps
/// that role's previous entries: a view with no tint to read has nothing to
/// say about the app's.
pub fn set_reported(entries: impl IntoIterator<Item = (ColorValue, bool, Color)>) -> u64 {
    let mut next: BTreeMap<(u8, u16, bool), Color> = entries
        .into_iter()
        .filter(|(c, ..)| is_known_reference(*c))
        .filter_map(|(c, dark, color)| reference_key(c).map(|(k, id)| ((k, id, dark), color)))
        .collect();
    let Ok(mut table) = REPORTED.write() else {
        return reported_generation();
    };
    let kept: Vec<_> = table
        .iter()
        .filter(|((kind, id, _), _)| {
            *kind == 0
                && u8::try_from(*id).is_ok_and(is_tint_role)
                && !next.keys().any(|(k, i, _)| *k == 0 && i == id)
        })
        .map(|(key, color)| (*key, *color))
        .collect();
    next.extend(kept);
    if *table != next {
        *table = next;
        GENERATION.fetch_add(1, Ordering::AcqRel);
    }
    reported_generation()
}

/// Every reference a host may resolve, with its native name on this
/// platform: each role, then each interned `-exact-platform-color()` that names one.
pub fn references(macos: bool) -> Vec<(ColorValue, Box<str>)> {
    let mut out: Vec<(ColorValue, Box<str>)> = COLOR_ROLES
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            let name = if macos { r.macos } else { r.ios };
            let id = u8::try_from(i).ok()?;
            (!name.is_empty()).then(|| (ColorValue::Role(id), name.into()))
        })
        .collect();
    if let Ok(table) = PLATFORM.lock() {
        for (i, p) in table.iter().enumerate() {
            let name = if macos {
                p.macos.clone()
            } else {
                p.ios.clone()
            };
            if let (Some(name), Ok(id)) = (name, u16::try_from(i)) {
                out.push((ColorValue::Platform(id), name));
            }
        }
    }
    out
}

mod session;
pub use session::{
    bind, bound, generation, install, installed, overrides, set_overrides, Installed, Roles,
    RolesScope,
};

impl ColorValue {
    /// What a host is sent for this colour: a role the theme replaces is
    /// what replaces it (LLP 1095 D4), anything else itself.
    ///
    /// A role an Apple platform draws in the view's tint (`AccentColor`,
    /// `LinkText`, a symbol's default) stays itself: a host makes the theme's
    /// colour the app's tint, under which an authored `accent-color` still
    /// wins for its subtree (LLP 1095 D1, the author over the design).
    #[track_caller]
    pub fn themed(self) -> ColorValue {
        match self {
            ColorValue::Role(id) if tint(id) => self,
            ColorValue::Role(id) => overridden(id).filter(|o| *o != self).unwrap_or(self),
            other => other,
        }
    }
}

/// The web's reach (LLP 1095 D4): the overridden roles as `:root` custom
/// properties the host's own sheet reads (`--exact-<role>`, a CSS system
/// colour's as `--exact-<name>` in lower case, `AccentColor` as
/// `--exact-accent`, which an authored `accent-color` redeclares below),
/// and the page's colour and ground where `CanvasText` and `Canvas` are
/// replaced. Empty without overrides.
#[track_caller]
pub fn theme_css() -> String {
    let mut out = String::new();
    for id in 0..=u8::MAX {
        let Some(role) = role_of(id) else { break };
        let Some(o) = overridden(id) else { continue };
        out.push_str(&theme_property(role));
        out.push(':');
        crate::gradient::color_css(&mut out, o);
        out.push(';');
        match role.name {
            // A `color` computed here is the root scheme's side, and a
            // `color-scheme` card would inherit it; the card re-reads the
            // entry through `--exact-scheme-color` ([`SCHEME_COLOR`]), which
            // an authored colour unsets ([`scheme_color`]).
            "CanvasText" => out.push_str(
                "color:var(--exact-canvas-text);--exact-scheme-color:var(--exact-canvas-text);",
            ),
            "Canvas" => out.push_str("background-color:var(--exact-canvas);"),
            _ => {}
        }
    }
    if out.is_empty() {
        return out;
    }
    // A scheme the element's own style writes (the wasm host's, and a bound
    // one on the JS target's; its classes are listed by the emitter).
    let scheme = match overridden_canvas_text() {
        true => format!("#exact-root [style*=color-scheme]{{{SCHEME_COLOR}}}"),
        false => String::new(),
    };
    format!(":root{{{out}}}{scheme}{}", brand_states())
}

/// @ref LLP 1129 D2, D3 — what a theme's accent and selection colours
/// reach on the web beyond the controls. The keyboard's ring is drawn in
/// the theme's accent, as iOS draws its focus halo in the tint the theme
/// sets (LLP 1095 D1), and solid: Chrome's `auto` ring ignores its offset
/// and hugs the control, so in the accent it vanishes on a control the
/// accent fills. Selected text takes the theme's `Highlight` pair; unsaid,
/// no `::selection` rule is written, since one changes how an engine draws
/// an inactive selection.
fn brand_states() -> String {
    let said = |name: &str| role(name).is_some_and(|id| overridden(id).is_some());
    let mut out = String::new();
    if overridden(accent()).is_some() {
        // Two ids and nothing else: over the shell's `outline: revert` on
        // its controls (one id, index.html) and under the author's classes
        // (two ids and a class), so an authored `outline` wins. An invalid
        // field keeps its red ring. The bar's Back, and a platform button
        // the accent fills, which the shell gives no offset, take one.
        out.push_str(
            "#exact-root#exact-root :where(:focus-visible:not([aria-invalid=true])),\
             #exact-root#exact-root [navigationkey]>header::part(back):focus-visible\
             {outline:2px solid var(--exact-accent)}\
             #exact-root#exact-root :where(button[data-native]:is([data-button-style=filled],\
             [data-button-style=bordered-prominent]):focus-visible){outline-offset:2px}",
        );
    }
    if said("Highlight") || said("HighlightText") {
        out.push_str(
            "#exact-root ::selection{background-color:var(--exact-highlight,Highlight);\
             color:var(--exact-highlight-text,HighlightText)}",
        );
    }
    out
}

/// @ref LLP 1095 D4 — a theme's `CanvasText` is a `color` on the web's root,
/// which the browser computes on the root's scheme; a `color-scheme` card
/// reads the entry again on its own, as Apple's does. Written at an id and
/// a class's specificity: over the page's control reset (`all: unset`, so a
/// button with a scheme reads it too), under the look's own control colours
/// and an authored colour on the card.
pub const SCHEME_COLOR: &str = "color:var(--exact-scheme-color,currentcolor)";

/// The JS target's rule for its classes `.c1`… (`classes[0]`…) that set a
/// scheme: [`SCHEME_COLOR`] on them. None without a theme.
pub fn scheme_classes(classes: &[String]) -> String {
    let schemed: Vec<String> = classes
        .iter()
        .enumerate()
        .filter(|(_, c)| c.contains("color-scheme:light") || c.contains("color-scheme:dark"))
        .map(|(i, _)| format!(".c{}", i + 1))
        .collect();
    match overridden_canvas_text() && !schemed.is_empty() {
        true => format!("#exact-root :is({}){{{SCHEME_COLOR}}}", schemed.join(",")),
        false => String::new(),
    }
}

/// The custom property a host's own sheet declares `role`'s override as
/// ([`theme_css`]): `--exact-accent` for `AccentColor`, a CSS system
/// colour's in kebab case as the look's sheets spell them
/// (`--exact-gray-text`), an Exact role's by its name.
pub fn theme_property(role: &ColorRole) -> String {
    let name = match role.name {
        "AccentColor" => "accent".to_string(),
        n if is_css_system(role) => n.chars().enumerate().fold(String::new(), |mut k, (i, c)| {
            if c.is_ascii_uppercase() && i > 0 {
                k.push('-');
            }
            k.push(c.to_ascii_lowercase());
            k
        }),
        n => n.to_string(),
    };
    format!("--exact-{name}")
}

thread_local! {
    static LIVE_PROPERTIES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// While the guard lives, an overridden role crosses into CSS as its sheet
/// property over its value (`var(--exact-label, #…)`), so a development
/// build's live edit of the override, which sets the property, reaches it
/// (the JS target, LLP 1121 stage 5). A release build crosses the value.
pub fn live_properties(on: bool) -> LiveProperties {
    LiveProperties(LIVE_PROPERTIES.with(|l| l.replace(on)))
}

/// [`live_properties`]' scope: the setting before, restored when dropped.
pub struct LiveProperties(bool);

impl Drop for LiveProperties {
    fn drop(&mut self) {
        let _ = LIVE_PROPERTIES.try_with(|l| l.set(self.0));
    }
}

/// What an authored colour writes beside itself under a theme that replaces
/// `CanvasText`: [`SCHEME_COLOR`] unset, so its subtree inherits it.
#[track_caller]
pub fn scheme_color(style: &super::StyleProps) -> &'static str {
    match style.mask.has(super::StyleId::TextColor) && overridden_canvas_text() {
        true => "--exact-scheme-color:initial;",
        false => "",
    }
}

/// Whether the theme replaces `CanvasText`, the page's unsaid text.
#[track_caller]
pub fn overridden_canvas_text() -> bool {
    static ID: std::sync::OnceLock<u8> = std::sync::OnceLock::new();
    overridden(*ID.get_or_init(|| role("CanvasText").unwrap_or(u8::MAX))).is_some()
}

/// A role an Apple platform draws in the view's tint (`AccentColor`,
/// `LinkText`, `-exact-link`, `-exact-symbol`, `ButtonText`). Of these a
/// theme may replace only `AccentColor`, which is the app's tint.
pub fn tint(id: u8) -> bool {
    follows_accent(id, false) || follows_accent(id, true)
}

thread_local! {
    /// The authored `accent-color` around what a painting host paints now
    /// ([`authored_accent`]).
    static AUTHORED: std::cell::Cell<Option<ColorValue>> = const { std::cell::Cell::new(None) };
}

/// A painting host's scope: the nearest `accent-color` its node or an
/// ancestor authored (the inherited row), which the roles the browser draws
/// in the accent ([`web_accent`]: `AccentColor`, `LinkText`, `-exact-link`,
/// `-exact-symbol`) resolve to while it lasts, as the web's
/// `var(--exact-accent, AccentColor)` reads the nearest one. Restored when
/// the scope ends.
///
/// `accent-color="AccentColor"` keeps the one around, as css.rs leaves
/// `--exact-accent` inherited for exactly that and Apple's tint stays the
/// ancestor's; another role (`LinkText`) is the platform's colour, read with
/// no scope ([`painted_accent`]).
///
/// The scope holds a copy of a wide colour id, unmarked (LLP 1121 stage 4):
/// it lasts only inside one paint call, and the id is the painted node's,
/// which the kernel the host borrows to paint marks for that whole time.
pub fn authored_accent(accent: Option<ColorValue>) -> AccentScope {
    let accent = match accent {
        Some(ColorValue::Role(id)) if id == self::accent() => AUTHORED.with(|a| a.get()),
        other => other,
    };
    AccentScope(
        AUTHORED.with(|a| a.replace(accent)),
        std::marker::PhantomData,
    )
}

/// What [`authored_accent`] restores; the thread's, so it stays on it.
pub struct AccentScope(
    Option<ColorValue>,
    std::marker::PhantomData<std::rc::Rc<()>>,
);

impl Drop for AccentScope {
    fn drop(&mut self) {
        AUTHORED.with(|a| a.set(self.0));
    }
}

/// `c` with a role drawn in the accent replaced by `accent`, where the
/// node's accent is one it or an ancestor authored: what a host keeps to
/// resolve later, outside the painting scope ([`authored_accent`]).
pub fn accented(c: ColorValue, accent: Option<ColorValue>) -> ColorValue {
    let _scope = authored_accent(accent);
    match (c, AUTHORED.with(|a| a.get())) {
        (ColorValue::Role(id), Some(a)) if role_of(id).is_some_and(web_accent) => a,
        _ => c,
    }
}

/// `c` under the scope's authored accent, when it is a role drawn in the
/// accent; the accent itself is resolved with none around (the scope never
/// holds such a role, [`authored_accent`]).
pub(crate) fn painted_accent(c: ColorValue, dark: bool) -> Option<super::Color> {
    let ColorValue::Role(id) = c else { return None };
    if !role_of(id).is_some_and(web_accent) {
        return None;
    }
    let accent = AUTHORED.with(|a| a.get())?;
    let _none = authored_accent(None);
    Some(accent.resolve(dark))
}

/// What a painting host resolves for `c` under the theme: a replaced
/// role's colour, and for a tint role the theme's `AccentColor` (an
/// authored accent is the host's to apply).
#[track_caller]
pub fn painted_override(c: ColorValue) -> Option<ColorValue> {
    let ColorValue::Role(id) = c else { return None };
    let accent = accent();
    let id = if id != accent && tint(id) && overridden(accent).is_some() {
        accent
    } else {
        id
    };
    match overridden(id).filter(|o| *o != ColorValue::Role(id))? {
        // A role replaced by a tint role is the tint: the theme's when it sets
        // one. The tint itself never names one (the compiler refuses it; a
        // built plan that does is read as no override, so nothing recurses).
        ColorValue::Role(r) if tint(r) => match id == accent {
            true => None,
            false => overridden(accent)
                .filter(|a| !matches!(a, ColorValue::Role(t) if tint(*t)))
                .or(Some(ColorValue::Role(r))),
        },
        o => Some(o),
    }
}

/// `AccentColor`'s id.
pub fn accent() -> u8 {
    static ID: std::sync::OnceLock<u8> = std::sync::OnceLock::new();
    *ID.get_or_init(|| role("AccentColor").unwrap_or(u8::MAX))
}

/// What the theme replaces a role with, if it does.
#[track_caller]
pub fn overridden(id: u8) -> Option<ColorValue> {
    let roles = bound();
    if roles.is_empty() {
        return None;
    }
    // One step: an override naming another overridden role reads that one's.
    match roles.find(id)? {
        ColorValue::Role(next) => roles.find(next).or(Some(ColorValue::Role(next))),
        other => Some(other),
    }
}

/// Interned `-exact-platform-color()`s. A plan names a few; the runner admits one
/// only as a plan literal (LLP 1095 D3), and the cap bounds the table
/// whatever else parses one.
static PLATFORM: Mutex<Vec<Arc<PlatformColor>>> = Mutex::new(Vec::new());
const PLATFORM_CAP: usize = 1024;

/// A `-exact-platform-color()` by id.
pub fn platform(id: u16) -> Option<Arc<PlatformColor>> {
    PLATFORM.lock().ok()?.get(usize::from(id)).cloned()
}

/// Whether a native name is one a host may look up: a class colour property
/// (`[a-z][A-Za-z0-9]*Color`, so never a private `_` selector, `new` or
/// `alloc`), or an asset catalogue colour.
pub fn native_name_ok(name: &str) -> bool {
    if let Some(asset) = name.strip_prefix("named:") {
        return !asset.is_empty()
            && asset
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "_-./ ".contains(c));
    }
    name.len() > 5
        && name.ends_with("Color")
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Intern every `-exact-platform-color()` written in `text` (a plan literal: a
/// colour, or a composite value holding some), so a host's first report
/// resolves it before any branch selects it (LLP 1095 D9). One that does
/// not parse is left for the row's own parse to refuse.
pub fn intern_literals(text: &str) {
    let mut rest = text;
    while let Some(at) = rest.find("-exact-platform-color(") {
        let call = &rest[at..];
        let mut depth = 0usize;
        let close = call.char_indices().find_map(|(i, c)| {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
            None
        });
        let Some(close) = close else {
            return;
        };
        parse_platform(&call[..=close]);
        rest = &call[close + 1..];
    }
}

/// `-exact-platform-color(<platform> <name>, …, <fallback>)` (LLP 1095 D3), interned.
/// The fallback is a colour or a `light-dark()` pair, never a reference.
pub fn parse_platform(text: &str) -> Option<ColorValue> {
    let inner = text
        .trim()
        .strip_prefix("-exact-platform-color(")?
        .strip_suffix(')')?;
    let mut parts = Vec::new();
    let (mut depth, mut start) = (0i32, 0);
    for (i, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(inner[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(inner[start..].trim());
    let fallback_text = parts.pop()?;
    let fallback = super::ColorValue::parse_pair(fallback_text)
        .or_else(|| Color::parse(fallback_text).map(ColorValue::Fixed))?;
    let (mut ios, mut macos, mut web) = (None, None, None);
    for part in &parts {
        let (platform, name) = part.split_once(char::is_whitespace)?;
        let name = name.trim();
        let slot = match platform {
            "ios" if native_name_ok(name) => &mut ios,
            "macos" if native_name_ok(name) => &mut macos,
            "web" if !name.is_empty() && !name.contains(['{', '}', ';']) => &mut web,
            _ => return None,
        };
        if slot.replace(Box::<str>::from(name)).is_some() {
            return None;
        }
    }
    let mut canonical = String::from("-exact-platform-color(");
    for (platform, name) in [("ios", &ios), ("macos", &macos), ("web", &web)] {
        if let Some(n) = name {
            canonical.push_str(platform);
            canonical.push(' ');
            canonical.push_str(n);
            canonical.push_str(", ");
        }
    }
    crate::gradient::color_css(&mut canonical, fallback);
    canonical.push(')');
    let mut table = PLATFORM.lock().ok()?;
    if let Some(i) = table.iter().position(|p| *p.text == *canonical) {
        return Some(ColorValue::Platform(u16::try_from(i).ok()?));
    }
    if table.len() >= PLATFORM_CAP {
        return Some(fallback);
    }
    table.push(Arc::new(PlatformColor {
        ios,
        macos,
        web,
        fallback,
        text: canonical.into(),
    }));
    Some(ColorValue::Platform(u16::try_from(table.len() - 1).ok()?))
}

/// A reference's CSS for the browser and for canonical text: a CSS system
/// colour as is; an Exact role as `var(--exact-<role>, <fallback>)`, which a
/// page's sheet may define (WebKit's own dynamic colour) and which reads
/// back as the role; a `-exact-platform-color()` as its web colour, else its
/// fallback, and on the wire as itself. A role the browser draws in the
/// accent ([`web_accent`]) is the nearest `accent-color` for it,
/// `var(--exact-accent, AccentColor)` (the custom property the web host
/// writes beside `accent-color`), and its name on the wire; any other of
/// them is that under its own name, `var(--exact-linktext,
/// var(--exact-accent, AccentColor))`, a property no sheet defines, so it
/// reads back as itself and not as `AccentColor`, which another platform
/// resolves differently (the Mac's `linkColor` is not its accent, nor its
/// symbols' colour).
#[track_caller]
pub(crate) fn reference_css(out: &mut String, c: ColorValue, mode: ColorText) {
    match c {
        ColorValue::Role(id) => {
            let Some(r) = role_of(id) else {
                out.push_str("transparent");
                return;
            };
            // @ref LLP 1095 D4 — a role the theme replaces crosses as what
            // replaces it, on the wire and in CSS.
            // The browser's accent roles read the nearest `accent-color`
            // first, so an authored one still wins; the theme's colour is
            // what they fall back to.
            let accent = mode == ColorText::Css && web_accent(r);
            match overridden(id) {
                // The page's `--exact-accent` is the theme's (theme_css), so
                // an accent role keeps reading it, an authored one first.
                _ if accent || tint(id) => {}
                Some(ColorValue::Role(next)) if next != id => {
                    return reference_css(out, ColorValue::Role(next), mode)
                }
                Some(ColorValue::Role(_)) | None => {}
                Some(o) if mode == ColorText::Css && LIVE_PROPERTIES.with(|l| l.get()) => {
                    out.push_str("var(");
                    out.push_str(&theme_property(r));
                    out.push_str(", ");
                    crate::gradient::color_text(out, o, mode);
                    out.push(')');
                    return;
                }
                Some(o) => return crate::gradient::color_text(out, o, mode),
            }
            if accent {
                let named = r.name != "AccentColor";
                if named {
                    // An Exact role's name is lower case already.
                    out.push_str("var(--exact-");
                    out.push_str(&r.name.to_ascii_lowercase());
                    out.push_str(", ");
                }
                out.push_str("var(--exact-accent, AccentColor)");
                if named {
                    out.push(')');
                }
            } else if is_css_system(r) {
                out.push_str(r.name);
            } else {
                out.push_str("var(--exact-");
                out.push_str(r.name);
                out.push_str(", ");
                crate::gradient::color_css(out, role_fallback(id));
                out.push(')');
            }
        }
        ColorValue::Platform(id) => match platform(id) {
            Some(p) if mode == ColorText::Wire => out.push_str(&p.text),
            Some(p) => match &p.web {
                Some(web) => out.push_str(web),
                None => crate::gradient::color_css(out, p.fallback),
            },
            None => out.push_str("transparent"),
        },
        other => crate::gradient::color_css(out, other),
    }
}

impl ColorValue {
    /// What a host without the platform's colour, or with only sRGB, shows:
    /// a reference's fallback (LLP 1095 D1), a wide colour's sRGB clip.
    #[track_caller]
    pub fn fallback(self) -> ColorValue {
        match self {
            ColorValue::Role(id) => overridden(id).map_or_else(
                || role_fallback(id),
                |o| match o {
                    ColorValue::Role(next) => role_fallback(next),
                    o => o.fallback(),
                },
            ),
            ColorValue::Platform(id) => {
                platform(id).map_or(ColorValue::Fixed(Color::TRANSPARENT), |p| p.fallback)
            }
            ColorValue::Wide(id) => super::wide::wide(id)
                .map_or(ColorValue::Fixed(Color::TRANSPARENT), |w| w.fallback()),
            // A host that reaches here has refused it already (LLP 1100 D3).
            ColorValue::Profiled(_) => ColorValue::Fixed(Color::TRANSPARENT),
            ColorValue::Moving(..) => {
                let (c, a) = self.moving_linear().unwrap_or_default();
                ColorValue::Fixed(Color::from_linear_srgb(c, a))
            }
            other => other,
        }
    }

    /// A role's WebKit name (`-apple-system-label`), when it has one: what an
    /// Apple host draws vibrantly inside a material (LLP 1077 D13). `fill`
    /// has no WebKit name and answers as its author spelling, `-exact-fill`,
    /// UIKit's `.fill` vibrancy (LLP 1095 §12).
    pub fn system_name(self) -> Option<&'static str> {
        match self {
            ColorValue::Role(id) => {
                let role = role_of(id)?;
                match role.alias {
                    "" if role.name == "fill" => Some("-exact-fill"),
                    "" => None,
                    alias => Some(alias),
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_themes_accent_and_highlight_reach_the_ring_and_the_selection() {
        let pair = |l: u32, d: u32| ColorValue::LightDark(Color(l), Color(d));
        let id = |name: &str| role(name).unwrap();
        set_overrides(vec![(id("CanvasText"), pair(0x1f2a_22ff, 0xece7_daff))]);
        let plain = theme_css();
        assert!(
            !plain.contains(":focus-visible") && !plain.contains("::selection"),
            "{plain}"
        );
        set_overrides(vec![
            (id("AccentColor"), pair(0x2f6b_3fff, 0x7cc0_8aff)),
            (id("Highlight"), pair(0x2f6b_3fff, 0x7cc0_8aff)),
        ]);
        let branded = theme_css();
        assert!(
            branded.contains("#exact-root#exact-root :where(:focus-visible:not([aria-invalid=true])),#exact-root#exact-root [navigationkey]>header::part(back):focus-visible{outline:2px solid var(--exact-accent)}"),
            "{branded}"
        );
        assert!(branded.contains("#exact-root ::selection{background-color:var(--exact-highlight,Highlight);color:var(--exact-highlight-text,HighlightText)}"), "{branded}");
        set_overrides(Vec::new());
        assert_eq!(theme_css(), "");
    }

    #[test]
    fn a_reported_resolution_replaces_the_fallback_until_withdrawn() {
        // A name no other test reports: the table is process-wide.
        let c = ColorValue::parse_light_dark(
            "-exact-platform-color(ios rolesTestReportColor, light-dark(#010203, #040506))",
        )
        .unwrap();
        assert_eq!(c.resolve(true), Color(0x0405_06ff));
        assert!(references(false)
            .iter()
            .any(|(r, name)| *r == c && &**name == "rolesTestReportColor"));
        let before = reported_generation();
        let first = set_reported([(c, true, Color(0xff00_00ff))]);
        assert!(first > before);
        assert_eq!(c.resolve(true), Color(0xff00_00ff));
        assert_eq!(
            c.resolve(false),
            Color(0x0102_03ff),
            "light was not reported"
        );
        assert_eq!(
            set_reported([(c, true, Color(0xff00_00ff))]),
            first,
            "unchanged"
        );
        assert!(set_reported([]) > first);
        // An id past the table, or never interned, is no reference: dropped,
        // and it names no role (transparent, never another role's colours).
        let bad_role = ColorValue::Role(u8::try_from(COLOR_ROLES.len()).unwrap());
        assert!(!is_known_reference(bad_role));
        assert!(role_of(u8::MAX).is_none());
        assert_eq!(bad_role.fallback(), ColorValue::Fixed(Color::TRANSPARENT));
        assert_eq!(bad_role.resolve(false), Color::TRANSPARENT);
        assert_eq!(bad_role.system_name(), None);
        let mut css = String::new();
        reference_css(&mut css, bad_role, ColorText::Css);
        assert_eq!(css, "transparent");
        assert!(!is_known_reference(ColorValue::Platform(u16::MAX)));
        // A report without the tint keeps the last one (here its fallback,
        // so no other test sees a change).
        let accent = role("AccentColor").unwrap();
        assert!(is_tint_role(accent) && is_tint_role(role("Highlight").unwrap()));
        assert!(!is_tint_role(role("CanvasText").unwrap()));
        let ColorValue::LightDark(light, dark) = role_fallback(accent) else {
            panic!()
        };
        let a = ColorValue::Role(accent);
        set_reported([
            (a, false, light),
            (a, true, dark),
            (c, true, Color(0xff00_00ff)),
        ]);
        set_reported([(c, true, Color(0x00ff_00ff))]);
        assert_eq!(reported(a, false), Some(light), "the tint is kept");
        assert_eq!(reported(a, true), Some(dark));
        assert_eq!(reported(c, true), Some(Color(0x00ff_00ff)));
        set_reported([]);
        let empty = reported_generation();
        assert_eq!(
            set_reported([(bad_role, false, Color(0xff00_00ff))]),
            empty,
            "nothing known was reported"
        );
        assert_eq!(c.resolve(true), Color(0x0405_06ff));
    }

    #[test]
    fn roles_answer_to_css_and_webkit_names_and_their_own_css() {
        let id = role("-exact-secondary-label").unwrap();
        assert_eq!(role("-Exact-Secondary-Label"), Some(id));
        assert_eq!(
            role("secondary-label"),
            None,
            "an Exact role is spelled -exact- (LLP 1081)"
        );
        assert_eq!(
            role("-exact-canvastext"),
            None,
            "a CSS colour keeps its CSS name"
        );
        assert_eq!(role("-apple-system-secondary-label"), Some(id));
        assert_eq!(
            role("var(--exact-secondary-label, light-dark(#3c3c4399, #ebebf599))"),
            Some(id)
        );
        assert_eq!(role("canvastext"), role("CanvasText"));
        assert!(role("ButtonBorder").is_none() && role("secondary_label").is_none());
        assert_eq!(
            role_fallback(id),
            ColorValue::LightDark(Color(0x3c3c_4399), Color(0xebeb_f599))
        );
        let mut css = String::new();
        reference_css(&mut css, ColorValue::Role(id), ColorText::Css);
        assert_eq!(
            ColorValue::parse_light_dark(&css),
            Some(ColorValue::Role(id))
        );
        // The tint is the nearest accent for the browser, its name on the
        // wire, and reads back as itself (LLP 1116.001 D2). A link is the
        // accent there too, one blue (LLP 1115.000 D3.5), under its own
        // name, so it is still `LinkText` when read back.
        for (name, web) in [
            ("AccentColor", "var(--exact-accent, AccentColor)"),
            (
                "LinkText",
                "var(--exact-linktext, var(--exact-accent, AccentColor))",
            ),
        ] {
            let tint = ColorValue::Role(role(name).unwrap());
            css.clear();
            reference_css(&mut css, tint, ColorText::Css);
            assert_eq!(css, web);
            assert_eq!(ColorValue::parse_light_dark(&css), Some(tint));
            css.clear();
            reference_css(&mut css, tint, ColorText::Wire);
            assert_eq!(css, name);
        }
        assert_eq!(
            role("var(--exact-accent, LinkText)"),
            role("LinkText"),
            "as it was written before"
        );
        assert_eq!(role("-exact-linktext"), None, "no author's spelling");
        assert_eq!(role("var(--exact-canvastext, CanvasText)"), None);
        // An Exact role that is the tint on iOS is the accent for the browser
        // too, under its own name: a link named by its role, and a symbol
        // nobody tinted (LLP 1115.000 D3.9).
        for name in ["-exact-link", "-exact-symbol"] {
            let tint = ColorValue::Role(role(name).unwrap());
            css.clear();
            reference_css(&mut css, tint, ColorText::Css);
            assert_eq!(
                css,
                format!(
                    "var(--exact-{}, var(--exact-accent, AccentColor))",
                    &name["-exact-".len()..]
                )
            );
            assert_eq!(ColorValue::parse_light_dark(&css), Some(tint));
            // On the wire an Exact role is its own pair, as before.
            css.clear();
            reference_css(&mut css, tint, ColorText::Wire);
            assert!(
                css.ends_with(", light-dark(#0088ffff, #0091ffff))"),
                "{css}"
            );
            assert_eq!(ColorValue::parse_light_dark(&css), Some(tint));
        }
        for name in ["Highlight", "CanvasText", "ButtonText"] {
            css.clear();
            reference_css(
                &mut css,
                ColorValue::Role(role(name).unwrap()),
                ColorText::Css,
            );
            assert_eq!(css, name, "not the tint at full strength");
        }
        assert_eq!(role("var(--exact-accent, CanvasText)"), None);
    }

    /// LLP 1115.000 D3.2: the table holds two columns. Apple's are the
    /// measurements (the separator, the system red and blue and the tint,
    /// iOS 27); each host shows them until it throws the switch, which no
    /// host does yet (`kernel/tests/column.rs` throws it, in a process of
    /// its own).
    #[test]
    fn a_role_has_apples_pair_and_a_neutral_one_and_shows_apples() {
        let pair = |name: &str, column| role_pair(role(name).unwrap(), column);
        let both = |light, dark| ColorValue::LightDark(Color(light), Color(dark));
        for (name, apple, neutral) in [
            (
                "-exact-separator",
                both(0x3c3c_431f, 0x5454_5880),
                both(0x0000_001f, 0xffff_ff29),
            ),
            (
                "-exact-system-red",
                both(0xff38_3cff, 0xff42_45ff),
                both(0xc90f_1aff, 0xfe73_67ff),
            ),
            (
                "-exact-system-blue",
                both(0x0088_ffff, 0x0091_ffff),
                both(0x0b66_d6ff, 0x68a5_feff),
            ),
            (
                "Canvas",
                both(0xffff_ffff, 0x0000_00ff),
                both(0xffff_ffff, 0x1212_12ff),
            ),
        ] {
            assert_eq!(pair(name, Column::Apple), apple, "{name}");
            assert_eq!(pair(name, Column::Neutral), neutral, "{name}");
        }
        assert_eq!(column(), Column::Apple, "a process starts in Apple's");
        for id in 0..COLOR_ROLES.len() as u8 {
            assert_eq!(role_fallback(id), role_pair(id, Column::Apple));
        }
        assert_eq!(
            role_pair(u8::MAX, Column::Neutral),
            ColorValue::Fixed(Color::TRANSPARENT)
        );
        // The tint is one colour: a role that is the tint on iOS, or the
        // tint at an alpha (`Highlight`), falls back to `AccentColor`'s pair
        // in Apple's column, so a host without a tint shows one blue. LLP
        // 1115.000 D3.6: a button's text and a link are the tint there, as
        // UIKit draws them, so an authored accent reaches both, and
        // `-exact-link` and `LinkText` resolve alike.
        let named = |name: &str| COLOR_ROLES[usize::from(role_id(name))];
        let accent = named("AccentColor");
        let tints: Vec<&str> = COLOR_ROLES
            .iter()
            .filter(|r| r.ios.starts_with("@tint"))
            .map(|r| r.name)
            .collect();
        assert_eq!(
            tints,
            [
                "LinkText",
                "Highlight",
                "AccentColor",
                "ButtonText",
                "link",
                "symbol"
            ]
        );
        for r in COLOR_ROLES.iter().filter(|r| r.ios.starts_with("@tint")) {
            let alpha = if r.ios == "@tint" { 0xff } else { 0x33 };
            for (got, want) in [(r.light, accent.light), (r.dark, accent.dark)] {
                assert_eq!(got, want & 0xffff_ff00 | alpha, "{}: Apple's", r.name);
            }
            assert_eq!(
                follows_accent(role_id(r.name), false),
                r.ios == "@tint",
                "{}",
                r.name
            );
        }
        // LLP 1115.000 D3.4: on the Mac the accent alone is the view's tint,
        // the nearest authored `accent-color` and else the system's; a link
        // and a control's text are AppKit's own colours there.
        let mac: Vec<&str> = COLOR_ROLES
            .iter()
            .filter(|r| follows_accent(role_id(r.name), true))
            .map(|r| r.name)
            .collect();
        assert_eq!(mac, ["AccentColor"]);
        assert_eq!(named("link").ios, named("LinkText").ios);
        assert_eq!(
            named("link").macos,
            "linkColor",
            "the Mac's own link colour"
        );
        assert_eq!(named("ButtonText").macos, "controlTextColor");
        // In the neutral column a link is the accent too; a button's text is
        // the page's ink, as a browser's own `ButtonText` is.
        for name in ["LinkText", "link", "Highlight", "symbol"] {
            let r = named(name);
            for (got, want) in [
                (r.neutral_light, accent.neutral_light),
                (r.neutral_dark, accent.neutral_dark),
            ] {
                assert_eq!(got >> 8, want >> 8, "{name}: neutral");
            }
        }
        let (button, ink) = (named("ButtonText"), named("CanvasText"));
        assert_eq!(
            (button.neutral_light, button.neutral_dark),
            (ink.neutral_light, ink.neutral_dark)
        );
        // An unset page background is `-exact-background`, the window's on
        // the Mac; `Canvas` stays the document's white there.
        assert_eq!(named("background").macos, "windowBackgroundColor");
        assert_eq!(named("background").ios, "systemBackgroundColor");
        assert_eq!(named("Canvas").macos, "textBackgroundColor");
    }

    /// A role's id by the name the table gives it.
    fn role_id(name: &str) -> u8 {
        let at = COLOR_ROLES.iter().position(|r| r.name == name);
        u8::try_from(at.unwrap()).unwrap()
    }

    /// The neutral column's own rule (the schema's `_colors`): a text role
    /// reads at WCAG's 4.5:1 or better on every background role of its
    /// scheme, and an ink on the fill it is named for, over each of them.
    #[test]
    fn the_neutral_column_reads_at_4_5_to_1_on_its_grounds() {
        fn linear(c: u8) -> f64 {
            let c = f64::from(c) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        // A colour over an opaque ground, as channels.
        fn over(top: Color, ground: [f64; 3]) -> [f64; 3] {
            let a = f64::from(top.a()) / 255.0;
            let mix = |t: u8, g: f64| f64::from(t) * a + g * (1.0 - a);
            [
                mix(top.r(), ground[0]),
                mix(top.g(), ground[1]),
                mix(top.b(), ground[2]),
            ]
        }
        fn luminance(c: [f64; 3]) -> f64 {
            let l = |v: f64| linear(v.round() as u8);
            0.2126 * l(c[0]) + 0.7152 * l(c[1]) + 0.0722 * l(c[2])
        }
        fn contrast(a: [f64; 3], b: [f64; 3]) -> f64 {
            let (a, b) = (luminance(a), luminance(b));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        }
        let neutral = |name: &str, dark: bool| {
            let r = COLOR_ROLES[usize::from(role(name).unwrap_or_else(|| panic!("{name}")))];
            Color(if dark {
                r.neutral_dark
            } else {
                r.neutral_light
            })
        };
        const GROUNDS: &[&str] = &[
            "Canvas",
            "Field",
            "-exact-background",
            "-exact-secondary-background",
            "-exact-tertiary-background",
            "-exact-grouped-background",
            "-exact-secondary-grouped-background",
            "-exact-tertiary-grouped-background",
        ];
        const INKS: &[(&str, &str)] = &[
            ("AccentColorText", "AccentColor"),
            ("ButtonText", "ButtonFace"),
            ("HighlightText", "Highlight"),
            ("FieldText", "Field"),
        ];
        // Every other role is a text's colour: the labels, the links, the
        // tint and the hues (a destructive row's red, an error's). The ink
        // on the accent is no text on the page.
        let text: Vec<String> = COLOR_ROLES
            .iter()
            .filter(|r| {
                let name = r.name;
                !name.ends_with("background")
                    && !name.ends_with("fill")
                    && !name.ends_with("separator")
                    && !["Canvas", "Field", "ButtonFace", "Highlight"].contains(&name)
                    && name != "AccentColorText"
            })
            .map(|r| {
                if is_css_system(r) {
                    r.name.to_owned()
                } else {
                    format!("-exact-{}", r.name)
                }
            })
            .collect();
        assert!(text.len() >= 24, "{text:?}");
        for dark in [false, true] {
            for ground_name in GROUNDS {
                let ground = over(neutral(ground_name, dark), [0.0; 3]);
                for name in &text {
                    let ratio = contrast(over(neutral(name, dark), ground), ground);
                    assert!(
                        ratio >= 4.5,
                        "{name} on {ground_name} (dark: {dark}) is {ratio:.2}:1"
                    );
                }
                for (ink, fill) in INKS {
                    let fill = over(neutral(fill, dark), ground);
                    let ratio = contrast(over(neutral(ink, dark), fill), fill);
                    assert!(
                        ratio >= 4.5,
                        "{ink} on its fill over {ground_name} (dark: {dark}) is {ratio:.2}:1"
                    );
                }
            }
        }
    }

    #[test]
    fn a_platform_color_is_interned_with_a_required_fallback() {
        let a = parse_platform("-exact-platform-color(ios systemMintColor, macos systemMintColor, light-dark(#00c7be, #63e6e2))").unwrap();
        let b = parse_platform(
            "-exact-platform-color(macos systemMintColor,ios systemMintColor,light-dark(#00c7be,#63e6e2))",
        )
        .unwrap();
        assert_eq!(a, b, "one canonical form, one id");
        let ColorValue::Platform(id) = a else {
            panic!("{a:?}")
        };
        let p = platform(id).unwrap();
        assert_eq!(p.ios.as_deref(), Some("systemMintColor"));
        assert_eq!(a.resolve(true), Color(0x63e6_e2ff));
        assert!(
            parse_platform("-exact-platform-color(ios systemMintColor)").is_none(),
            "no fallback"
        );
        assert!(parse_platform("-exact-platform-color(ios _privateColor, #000)").is_none());
        assert!(parse_platform("-exact-platform-color(ios new, #000)").is_none());
        assert!(
            parse_platform("-exact-platform-color(ios labelColor, label)").is_none(),
            "a reference is no fallback"
        );
        assert!(parse_platform("-exact-platform-color(android x, #000)").is_none());
        assert!(parse_platform("-exact-platform-color(ios named:Brand, #c8102e)").is_some());
    }

    #[test]
    fn a_literals_platform_colours_are_interned_wherever_they_stand() {
        let a = "-exact-platform-color(ios rolesTestLiteralAColor, light-dark(#010203, #040506))";
        let b = "-exact-platform-color(ios rolesTestLiteralBColor, #070809)";
        intern_literals(&format!(
            "linear-gradient({a}, transparent), 0 1px {b}, -exact-platform-color("
        ));
        let names: Vec<_> = references(false).into_iter().map(|(_, n)| n).collect();
        for name in ["rolesTestLiteralAColor", "rolesTestLiteralBColor"] {
            assert!(names.iter().any(|n| &**n == name), "{name}");
        }
    }

    #[test]
    fn references_cross_the_wire_as_what_they_name() {
        use crate::wire::codec::{Reader, Writer};
        for text in [
            "-exact-secondary-label",
            "Canvas",
            "-exact-platform-color(ios systemTealColor, light-dark(#30b0c7, #40c8e0))",
        ] {
            let c = ColorValue::parse_light_dark(text).unwrap();
            let mut w = Writer::new();
            w.color_value(c);
            let bytes = w.into_vec();
            assert_eq!(Reader::new(&bytes).color_value().unwrap(), c, "{text}");
        }
    }

    #[test]
    fn composite_rows_keep_their_references_across_the_wire() {
        use crate::wire::codec::{Reader, Writer};
        use crate::{StyleId, StyleProps, StyleValue};
        // A web colour, so the browser's CSS differs from what was written.
        let p = "-exact-platform-color(ios systemOrangeColor, web orange, #010203)";
        let mut style = StyleProps::default();
        for (id, text) in [
            (
                StyleId::BackgroundImage,
                format!("linear-gradient({p}, -exact-system-orange)"),
            ),
            (
                StyleId::MaskImage,
                format!("linear-gradient({p}, transparent)"),
            ),
            (
                StyleId::BoxShadow,
                format!("0 1px 2px {p}, 0 0 4px -exact-secondary-label"),
            ),
            (StyleId::TextShadow, format!("1px 1px 2px {p}")),
            (StyleId::SymbolPalette, format!("{p} -exact-system-orange")),
            (StyleId::Fill, p.to_string()),
            (StyleId::Stroke, format!("url(#g) {p}")),
            (
                StyleId::Filter,
                format!("drop-shadow(0 2px 4px {p}) drop-shadow(1px 1px -exact-system-orange)"),
            ),
        ] {
            style
                .set_dynamic(id, &StyleValue::Text(text.as_str().into()))
                .unwrap_or_else(|e| panic!("{text}: {e:?}"));
        }
        assert!(
            !style.background_image.css().contains("platform-color"),
            "the browser's CSS"
        );
        let mut w = Writer::new();
        style.encode_patch(&mut w);
        let back = StyleProps::decode_patch(&mut Reader::new(w.as_slice())).unwrap();
        assert_eq!(back, style);
        let ColorValue::Platform(id) = back.background_image.layers()[0].stops[0].color else {
            panic!("{:?}", back.background_image)
        };
        assert_eq!(
            platform(id).unwrap().ios.as_deref(),
            Some("systemOrangeColor")
        );
    }
}
