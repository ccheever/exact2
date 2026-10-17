//! The build's one evaluator (LLP 1121 D7, §3 "Folding"): a colour value
//! over literals, dials and other tokens, folded in f64 to a form the
//! kernel already has. Only a legacy result is quantised, once, to 8-bit
//! sRGB; every other result keeps its own space (LLP 1100).

use crate::color_fn;
use crate::css::Item;
use exact_color::{Parsed, Rgba8, Space, Wide};

/// One colour, as CSS would serialise it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Half {
    /// A legacy sRGB colour: `Fixed` in the kernel, 8-bit, beside the
    /// unquantised value a further fold starts from (§3: only the final
    /// legacy result is quantised).
    Legacy(Rgba8, Wide),
    /// A colour in its own space: `Wide`.
    Wide(Wide),
}

/// A host value: what the build cannot fold, since a host resolves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Host {
    /// A colour role, as a row writes it (`-exact-label`, `Canvas`).
    Role(String),
    /// A `-exact-platform-color(…)` with its fallback folded, and that
    /// fallback's light and dark pair.
    Platform(String, [String; 2]),
}

/// A folded colour value.
#[derive(Debug, Clone, PartialEq)]
pub enum Folded {
    /// One colour whatever the scheme.
    One(Half),
    /// `light-dark()`: the light side and the dark side.
    Pair(Half, Half),
    /// A host value.
    Host(Host),
}

/// Why a value does not fold: a refusal's id and message.
#[derive(Debug, Clone, PartialEq)]
pub struct FoldError {
    /// Stable id (`tamber-residual`, `tamber-unknown-token` …).
    pub id: &'static str,
    /// What and why.
    pub message: String,
}

pub(crate) fn fail<T>(id: &'static str, message: impl Into<String>) -> Result<T, FoldError> {
    Err(FoldError {
        id,
        message: message.into(),
    })
}

/// What a name in a value is.
pub(crate) enum Named {
    /// A token or colour dial's value.
    Color(Folded),
    /// A number dial's or number token's value.
    Number(f64),
    /// A length dial's or length token's value, which no colour takes.
    Length(crate::measure::Length),
    /// A duration token's milliseconds.
    Duration(f64),
    /// An easing token's text.
    Easing(String),
    /// A shadow token's value, which only a whole shadow value names.
    Shadow(crate::shadow::Shadows),
    /// A colour role no token overrides.
    Role(String),
    /// Nothing the design declares.
    Unknown,
}

/// The names a value may use, answered by the checker.
pub(crate) trait Names {
    fn named(&mut self, name: &str) -> Result<Named, FoldError>;
}

impl Half {
    /// An 8-bit legacy colour, exact as it is.
    pub(crate) fn legacy(c: Rgba8) -> Half {
        Half::Legacy(
            c,
            Wide {
                space: Space::Srgb,
                c: [c.r, c.g, c.b].map(|v| f64::from(v) / 255.0),
                alpha: c.a,
            },
        )
    }

    /// As a colour in its own space; a legacy colour in `srgb`, unquantised.
    pub fn wide(self) -> Wide {
        match self {
            Half::Wide(w) | Half::Legacy(_, w) => w,
        }
    }

    /// The text the kernel reads back to this form: `#rrggbbaa` for a
    /// legacy colour, as `style::wide` writes a legacy half; else CSS's
    /// serialisation.
    pub fn css(self) -> String {
        match self {
            Half::Legacy(c, _) => hex(c),
            Half::Wide(w) => w.css(),
        }
    }

    /// The 8-bit sRGB clip, as a fallback.
    pub fn clip(self) -> String {
        match self {
            Half::Legacy(c, _) => hex(c),
            // Of the colour the kernel reads back, which clamps what an
            // unclamped relative colour carries (a negative chroma).
            Half::Wide(w) => match exact_color::parse(&w.css()) {
                Some(Parsed::Wide(read)) => hex(read.srgb8()),
                _ => hex(w.srgb8()),
            },
        }
    }
}

fn hex(c: Rgba8) -> String {
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        c.r,
        c.g,
        c.b,
        (c.a * 255.0).round() as u8
    )
}

impl Folded {
    /// The side a scheme shows, or the host value that stops a fold.
    pub(crate) fn side(&self, dark: bool) -> Result<Half, &Host> {
        match self {
            Folded::One(h) => Ok(*h),
            Folded::Pair(l, d) => Ok(if dark { *d } else { *l }),
            Folded::Host(h) => Err(h),
        }
    }

    /// The text the plan carries, which the kernel parses to the form.
    pub fn css(&self) -> String {
        match self {
            Folded::One(h) => h.css(),
            Folded::Pair(l, d) => format!("light-dark({}, {})", l.css(), d.css()),
            Folded::Host(Host::Role(r)) => r.clone(),
            Folded::Host(Host::Platform(p, _)) => p.clone(),
        }
    }
}

impl Host {
    fn text(&self) -> String {
        Folded::Host(self.clone()).css()
    }
}

/// `f` of each side, as one colour when the inputs are, else a pair.
pub(crate) fn per_side(
    inputs: &[&Folded],
    what: &str,
    mut f: impl FnMut(&[Half]) -> Result<Half, FoldError>,
) -> Result<Folded, FoldError> {
    let mut sides = [Vec::new(), Vec::new()];
    for (dark, side) in sides.iter_mut().enumerate() {
        for input in inputs {
            match input.side(dark == 1) {
                Ok(h) => side.push(h),
                Err(host) => {
                    return fail(
                        "tamber-residual",
                        format!(
                            "`{what}` over the host value `{}` does not fold at build, and a device evaluates none until LLP 1095 D10's mixes are built",
                            host.text()
                        ),
                    )
                }
            }
        }
    }
    let light = f(&sides[0])?;
    if inputs.iter().all(|i| matches!(i, Folded::One(_))) {
        return Ok(Folded::One(light));
    }
    Ok(Folded::Pair(light, f(&sides[1])?))
}

/// Fold a whole value as a colour.
pub(crate) fn color(items: &[Item], names: &mut dyn Names) -> Result<Folded, FoldError> {
    match items {
        [one] => color_item(one, names),
        _ if items.iter().any(|i| {
            matches!(
                i,
                Item::Number(_) | Item::Dim(..) | Item::Str(_) | Item::Slash
            )
        }) =>
        {
            not_color_kind()
        }
        _ => fail("tamber-kind", "a colour is one value"),
    }
}

pub(crate) fn not_color_kind<T>() -> Result<T, FoldError> {
    fail(
        "tamber-stage",
        "type and family tokens arrive with stage 6's commit C",
    )
}

pub(crate) fn color_item(item: &Item, names: &mut dyn Names) -> Result<Folded, FoldError> {
    match item {
        Item::Hash(h) => match exact_color::parse(&format!("#{h}")) {
            Some(Parsed::Legacy(c)) => Ok(Folded::One(Half::legacy(c))),
            _ => fail("tamber-kind", format!("`#{h}` is not a colour")),
        },
        Item::Ident(name) => ident(name, names),
        Item::Func(name, args, text) => func(name, args, text, names),
        Item::Number(_) | Item::Percent(_) | Item::Dim(..) | Item::Str(_) => not_color_kind(),
        Item::Slash | Item::Op(_) => fail("tamber-kind", "not a colour"),
    }
}

fn ident(name: &str, names: &mut dyn Names) -> Result<Folded, FoldError> {
    match names.named(name)? {
        Named::Color(f) => return Ok(f),
        Named::Number(_) => {
            return fail("tamber-kind", format!("`{name}` is a number, not a colour"))
        }
        Named::Length(_) => {
            return fail("tamber-kind", format!("`{name}` is a length, not a colour"))
        }
        Named::Shadow(_) => {
            return fail(
                "tamber-kind",
                format!("`{name}` is a shadow token, not a colour"),
            )
        }
        Named::Duration(_) | Named::Easing(_) => {
            return fail(
                "tamber-kind",
                format!("`{name}` is a motion token, not a colour"),
            )
        }
        Named::Role(r) => return Ok(Folded::Host(Host::Role(r))),
        Named::Unknown => {}
    }
    match exact_color::parse(name) {
        Some(Parsed::Legacy(c)) => Ok(Folded::One(Half::legacy(c))),
        Some(Parsed::Current) => fail(
            "tamber-kind",
            "`currentcolor` is the element's own colour, which no token can hold",
        ),
        _ => fail(
            "tamber-unknown-token",
            format!("`{name}` is no token, dial, role or CSS colour"),
        ),
    }
}

/// One colour argument: a lone item.
fn one_color(arg: &[Item], what: &str, names: &mut dyn Names) -> Result<Folded, FoldError> {
    match arg {
        [one] => color_item(one, names),
        _ => fail("tamber-syntax", format!("`{what}` takes a colour here")),
    }
}

fn func(
    name: &str,
    args: &[Vec<Item>],
    text: &str,
    names: &mut dyn Names,
) -> Result<Folded, FoldError> {
    match name {
        "light-dark" => {
            let [a, b] = args else {
                return fail("tamber-syntax", "`light-dark()` takes two colours");
            };
            let (a, b) = (one_color(a, text, names)?, one_color(b, text, names)?);
            let light = a.side(false).map_err(residual_pair)?;
            let dark = b.side(true).map_err(residual_pair)?;
            Ok(Folded::Pair(light, dark))
        }
        "color-mix" => color_fn::mix(args, text, names),
        "contrast-color" => {
            let [a] = args else {
                return fail("tamber-syntax", "`contrast-color()` takes one colour");
            };
            let a = one_color(a, text, names)?;
            per_side(&[&a], text, |s| Ok(Half::legacy(contrast(s[0]))))
        }
        "-exact-platform-color" => platform(args, text, names),
        "var" => fail(
            "tamber-var",
            "a token is named bare (`surface.card`), not through `var()` (LLP 1121 D3)",
        ),
        "rgb" | "rgba" | "hsl" | "hsla" | "hwb" | "lab" | "lch" | "oklab" | "oklch" | "color" => {
            let from = matches!(args.first().and_then(|a| a.first()), Some(Item::Ident(w)) if w.eq_ignore_ascii_case("from"));
            if from {
                color_fn::relative(name, args, text, names)
            } else {
                color_fn::absolute(name, args, text, names)
            }
        }
        "calc" | "min" | "max" | "clamp" | "round" => not_color_kind(),
        _ => fail(
            "tamber-kind",
            format!("`{name}()` is not a colour function"),
        ),
    }
}

fn residual_pair(host: &Host) -> FoldError {
    FoldError {
        id: "tamber-residual",
        message: format!(
            "`light-dark()` with the host value `{}` as a side has no form in the kernel until LLP 1095 D10",
            host.text()
        ),
    }
}

/// `-exact-platform-color(ios <name>, macos <name>, web <name>, <fallback>)`
/// (LLP 1095 D3), as the kernel reads it: each platform once, and a
/// fallback that is a legacy colour or a pair of them, which may name
/// tokens and is folded here.
fn platform(args: &[Vec<Item>], text: &str, names: &mut dyn Names) -> Result<Folded, FoldError> {
    let shape = "`-exact-platform-color(ios <name>, macos <name>, web <name>, <fallback>)`";
    let Some((fallback, parts)) = args.split_last().filter(|(_, p)| !p.is_empty()) else {
        return fail("tamber-syntax", format!("`{text}`: {shape}"));
    };
    // The platform names as written, which the kernel reads as text.
    let inner = &text[text.find('(').map_or(0, |o| o + 1)..text.len().saturating_sub(1)];
    let mut written = Vec::new();
    let mut rest = inner;
    while let Some((part, tail)) = crate::lex::split_top(rest, ',') {
        written.push(part.trim());
        rest = tail;
    }
    let mut out = String::from("-exact-platform-color(");
    let mut seen = Vec::new();
    for part in written.iter().take(parts.len()) {
        let (p, name) = part.split_once(char::is_whitespace).unwrap_or((part, ""));
        let name = name.trim();
        let ok = match p {
            "ios" | "macos" => native_name_ok(name),
            "web" => !name.is_empty() && !name.contains(['{', '}', ';']),
            _ => false,
        };
        if !ok || seen.contains(&p) {
            return fail(
                "tamber-syntax",
                format!("`{text}`: {shape}, each platform once; an iOS or macOS name is a UIColor/NSColor class property (`systemTealColor`) or `named:<asset>`"),
            );
        }
        seen.push(p);
        out.push_str(&format!("{p} {name}, "));
    }
    let fallback = one_color(fallback, text, names)?;
    let pair = match &fallback {
        Folded::One(h @ Half::Legacy(..)) => [h.clip(), h.clip()],
        Folded::Pair(l @ Half::Legacy(..), d @ Half::Legacy(..)) => [l.clip(), d.clip()],
        _ => {
            return fail(
                "tamber-kind",
                format!("`{text}`: a platform colour's fallback is a legacy colour or a `light-dark()` of two"),
            )
        }
    };
    out.push_str(&fallback.css());
    out.push(')');
    Ok(Folded::Host(Host::Platform(out, pair)))
}

/// The kernel's rule for an iOS or macOS colour name (`roles::native_name_ok`):
/// a class property ending `Color`, or `named:` and an asset catalogue name.
fn native_name_ok(name: &str) -> bool {
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

/// CSS Color 5's `contrast-color()`: white or black, whichever contrasts
/// more with the colour (WCAG 2), white on a tie.
fn contrast(h: Half) -> Rgba8 {
    let [r, g, b] = h.wide().linear_srgb().map(|c| c.clamp(0.0, 1.0));
    let l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let v = if 1.05 / (l + 0.05) >= (l + 0.05) / 0.05 {
        255
    } else {
        0
    };
    Rgba8 {
        r: v,
        g: v,
        b: v,
        a: 1.0,
    }
}
