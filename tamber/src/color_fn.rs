//! CSS's colour functions over literals, dials and tokens (CSS Color 4
//! and 5): absolute (`rgb()`, `oklch()`, `color()` …), relative (`from`),
//! and `color-mix()`, with `calc()` typed as CSS types it.

use crate::css::Item;
use crate::fold::{color_item, fail, per_side, FoldError, Folded, Half, Named, Names};
use exact_color::{HueMethod, Interpolation, MixSpace, Parsed, Space, Wide};

/// A typed calc value (CSS Values 4 §10.9): a number, an angle in degrees,
/// or a percentage not yet resolved.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Num {
    N(f64),
    Angle(f64),
    Pct(f64),
}

/// What a component slot takes.
#[derive(Debug, Clone, Copy)]
enum Slot {
    /// A number, or a percentage of this reference.
    Channel(f64),
    /// A number or an angle, in degrees.
    Hue,
    /// A percentage, as in `color-mix()`.
    Percentage,
}

fn syntax(message: impl Into<String>) -> FoldError {
    FoldError {
        id: "tamber-syntax",
        message: message.into(),
    }
}

/// A relative colour's channel keywords and their values.
struct Channels<'a> {
    names: [&'a str; 3],
    values: [f64; 3],
    /// A powerless channel (CSS Color 4 §4.4: an achromatic colour's hue),
    /// which a further fold would need to carry as missing.
    missing: [bool; 3],
    alpha: Option<f64>,
}

const NO_CHANNELS: Channels<'static> = Channels {
    names: ["", "", ""],
    values: [0.0; 3],
    missing: [false; 3],
    alpha: None,
};

impl Channels<'_> {
    fn get(&self, word: &str) -> Option<Result<f64, FoldError>> {
        if word.eq_ignore_ascii_case("alpha") {
            return self.alpha.map(Ok);
        }
        let i = self
            .names
            .iter()
            .position(|n| !n.is_empty() && n.eq_ignore_ascii_case(word))?;
        if self.missing[i] {
            return Some(Err(syntax(format!(
                "`{word}` is missing (powerless) in an achromatic colour, which is not folded yet; write its value"
            ))));
        }
        Some(Ok(self.values[i]))
    }
}

fn is_math(f: &str) -> bool {
    matches!(f, "calc" | "min" | "max" | "clamp" | "")
}

/// One component, typed for its slot, as a number (degrees for a hue, a
/// percentage's number for a percentage slot).
fn component(
    item: &Item,
    slot: Slot,
    ch: &Channels,
    names: &mut dyn Names,
) -> Result<f64, FoldError> {
    let value = match (operand(item, ch, names)?, slot) {
        (Num::Pct(p), Slot::Channel(reference)) => Num::N(p / 100.0 * reference),
        (v, _) => v,
    };
    match (value, slot) {
        (Num::N(v), Slot::Channel(_) | Slot::Hue)
        | (Num::Angle(v), Slot::Hue)
        | (Num::Pct(v), Slot::Percentage) => {
            if v.is_finite() {
                Ok(v)
            } else {
                Err(syntax("a component that is not finite"))
            }
        }
        (Num::Angle(_), _) => Err(syntax("an angle where a colour takes a number")),
        (Num::Pct(_), _) => Err(syntax(
            "a percentage where a hue takes a number or an angle",
        )),
        (Num::N(_), Slot::Percentage) => {
            Err(syntax("a number where `color-mix()` takes a percentage"))
        }
    }
}

/// One operand, typed; a percentage stays one until its slot resolves it
/// (CSS Values 4 §10.9), so `calc(10 + 50%)` is a sum of two types.
fn operand(item: &Item, ch: &Channels, names: &mut dyn Names) -> Result<Num, FoldError> {
    Ok(match item {
        Item::Number(n) => Num::N(*n),
        Item::Percent(p) => Num::Pct(*p),
        Item::Dim(n, unit) => match unit.as_str() {
            "deg" => Num::Angle(*n),
            "grad" => Num::Angle(n * 0.9),
            "rad" => Num::Angle(n.to_degrees()),
            "turn" => Num::Angle(n * 360.0),
            _ => return Err(syntax(format!("`{n}{unit}`: a length in a colour"))),
        },
        Item::Ident(w) if w.eq_ignore_ascii_case("none") => {
            return fail(
                "tamber-syntax",
                "a missing component (`none`) is not folded yet; write its value",
            )
        }
        Item::Ident(w) => match ch.get(w) {
            Some(v) => Num::N(v?),
            None => match names.named(w)? {
                Named::Number(n) => Num::N(n),
                _ => return fail("tamber-kind", format!("`{w}` is not a number here")),
            },
        },
        Item::Func(f, args, _) if is_math(f) => math(f, args, ch, names)?,
        _ => return Err(syntax("not a number")),
    })
}

/// `calc()`, `min()`, `max()`, `clamp()` or a group.
fn math(
    f: &str,
    args: &[Vec<Item>],
    ch: &Channels,
    names: &mut dyn Names,
) -> Result<Num, FoldError> {
    let mut each = Vec::new();
    for arg in args {
        each.push(sum(arg, ch, names)?);
    }
    let same = each
        .windows(2)
        .all(|w| std::mem::discriminant(&w[0]) == std::mem::discriminant(&w[1]));
    if !same {
        return Err(syntax(format!("`{f}()` over values of different types")));
    }
    let v = |n: &Num| match *n {
        Num::N(v) | Num::Angle(v) | Num::Pct(v) => v,
    };
    let pick = |x: f64| match each[0] {
        Num::N(_) => Num::N(x),
        Num::Angle(_) => Num::Angle(x),
        Num::Pct(_) => Num::Pct(x),
    };
    match (f, each.as_slice()) {
        ("calc" | "", [one]) => Ok(*one),
        ("min", [_, ..]) => Ok(pick(each.iter().map(v).fold(f64::INFINITY, f64::min))),
        ("max", [_, ..]) => Ok(pick(each.iter().map(v).fold(f64::NEG_INFINITY, f64::max))),
        ("clamp", [lo, x, hi]) => Ok(pick(v(x).min(v(hi)).max(v(lo)))),
        _ => Err(syntax(format!("`{f}()` has the wrong number of arguments"))),
    }
}

/// Terms joined by `+` and `-`, each factors joined by `*` and `/`: a sum
/// takes one type, a product a number on one side, a quotient a number
/// below.
fn sum(items: &[Item], ch: &Channels, names: &mut dyn Names) -> Result<Num, FoldError> {
    let missing = || syntax("an operator without an operand");
    let mut total: Option<Num> = None;
    let mut sign = 1.0;
    let (mut term, mut pending): (Option<Num>, Option<char>) = (None, None);
    let add = |total: Option<Num>, sign: f64, t: Num| -> Result<Num, FoldError> {
        let t = scale(t, sign);
        match (total, t) {
            (None, t) => Ok(t),
            (Some(Num::N(a)), Num::N(b)) => Ok(Num::N(a + b)),
            (Some(Num::Angle(a)), Num::Angle(b)) => Ok(Num::Angle(a + b)),
            (Some(Num::Pct(a)), Num::Pct(b)) => Ok(Num::Pct(a + b)),
            _ => Err(syntax("a sum of values of different types")),
        }
    };
    for item in items {
        match item {
            Item::Op(c @ ('+' | '-')) if pending.is_none() => {
                total = Some(add(total, sign, term.take().ok_or_else(missing)?)?);
                sign = if *c == '-' { -1.0 } else { 1.0 };
            }
            Item::Op('*') if term.is_some() && pending.is_none() => pending = Some('*'),
            Item::Slash if term.is_some() && pending.is_none() => pending = Some('/'),
            Item::Op(_) | Item::Slash => return Err(missing()),
            _ => {
                let v = operand(item, ch, names)?;
                term = Some(match (term, pending.take(), v) {
                    (None, None, v) => v,
                    (Some(Num::N(a)), Some('*'), v) => scale(v, a),
                    (Some(t), Some('*'), Num::N(b)) => scale(t, b),
                    (Some(t), Some('/'), Num::N(b)) => scale(t, 1.0 / b),
                    (Some(_), Some(_), _) => {
                        return Err(syntax(
                            "a product needs a number on one side, a quotient a number below",
                        ))
                    }
                    _ => return Err(syntax("two operands without an operator")),
                });
            }
        }
    }
    if pending.is_some() {
        return Err(missing());
    }
    add(total, sign, term.ok_or_else(missing)?)
}

fn scale(n: Num, by: f64) -> Num {
    match n {
        Num::N(v) => Num::N(v * by),
        Num::Angle(v) => Num::Angle(v * by),
        Num::Pct(v) => Num::Pct(v * by),
    }
}

/// A function's space, its channel keywords and slots, and whether its
/// channels are 0–255 (`rgb()`).
fn spec(name: &str) -> Option<(MixSpace, [&'static str; 3], [Slot; 3])> {
    use Slot::{Channel as C, Hue};
    Some(match name {
        "rgb" | "rgba" => (MixSpace::Srgb, ["r", "g", "b"], [C(255.0); 3]),
        "hsl" | "hsla" => (MixSpace::Hsl, ["h", "s", "l"], [Hue, C(100.0), C(100.0)]),
        "hwb" => (MixSpace::Hwb, ["h", "w", "b"], [Hue, C(100.0), C(100.0)]),
        "lab" => (
            MixSpace::Lab,
            ["l", "a", "b"],
            [C(100.0), C(125.0), C(125.0)],
        ),
        "lch" => (MixSpace::Lch, ["l", "c", "h"], [C(100.0), C(150.0), Hue]),
        "oklab" => (MixSpace::Oklab, ["l", "a", "b"], [C(1.0), C(0.4), C(0.4)]),
        "oklch" => (MixSpace::Oklch, ["l", "c", "h"], [C(1.0), C(0.4), Hue]),
        _ => return None,
    })
}

/// `color()`'s space after its leading identifier.
fn color_space(id: &str) -> Option<(MixSpace, [&'static str; 3])> {
    let space = Interpolation::parse(&["in", &id.to_ascii_lowercase()])?
        .0
        .space;
    match space {
        MixSpace::Hsl
        | MixSpace::Hwb
        | MixSpace::Lab
        | MixSpace::Lch
        | MixSpace::Oklab
        | MixSpace::Oklch => None,
        MixSpace::XyzD50 | MixSpace::XyzD65 => Some((space, ["x", "y", "z"])),
        _ => Some((space, ["r", "g", "b"])),
    }
}

/// The result space of a mix or a relative colour (hsl and hwb are sRGB).
fn space_of(mix: MixSpace) -> Space {
    match mix {
        MixSpace::Srgb | MixSpace::Hsl | MixSpace::Hwb => Space::Srgb,
        MixSpace::SrgbLinear => Space::SrgbLinear,
        MixSpace::DisplayP3 => Space::DisplayP3,
        MixSpace::DisplayP3Linear => Space::DisplayP3Linear,
        MixSpace::A98Rgb => Space::A98Rgb,
        MixSpace::ProphotoRgb => Space::ProphotoRgb,
        MixSpace::Rec2020 => Space::Rec2020,
        MixSpace::Lab => Space::Lab,
        MixSpace::Oklab => Space::Oklab,
        MixSpace::XyzD50 => Space::XyzD50,
        MixSpace::XyzD65 => Space::XyzD65,
        MixSpace::Lch => Space::Lch,
        MixSpace::Oklch => Space::Oklch,
    }
}

/// A colour in `space`'s components; in its own space, as it is.
fn components(w: Wide, space: MixSpace) -> [f64; 3] {
    if space_of(space) == w.space && !matches!(space, MixSpace::Hsl | MixSpace::Hwb) {
        return w.c;
    }
    space.from_linear_srgb(w.linear_srgb())
}

/// Components in `space` as a colour, hsl and hwb as sRGB. A relative
/// colour's components stay unclamped, so a fold over it starts from them
/// (CSS Color 5 §4); the kernel clamps what it shows when it parses them.
fn wide_from(space: MixSpace, c: [f64; 3], alpha: f64) -> Wide {
    let c = match space {
        MixSpace::Hsl | MixSpace::Hwb => MixSpace::Srgb.from_linear_srgb(space.to_linear_srgb(c)),
        _ => c,
    };
    let c = if matches!(space, MixSpace::Lch | MixSpace::Oklch) {
        [c[0], c[1], c[2].rem_euclid(360.0)]
    } else {
        c
    };
    Wide {
        space: space_of(space),
        c,
        alpha: alpha.clamp(0.0, 1.0),
    }
}

/// The three channels and the alpha of a space-separated component list.
fn channels<'i>(
    items: &'i [Item],
    text: &str,
) -> Result<(&'i [Item], Option<&'i Item>), FoldError> {
    let (main, alpha) = match items.iter().position(|i| *i == Item::Slash) {
        Some(at) => match &items[at + 1..] {
            [a] => (&items[..at], Some(a)),
            _ => return Err(syntax(format!("`{text}`: one alpha after `/`"))),
        },
        None => (items, None),
    };
    if main.len() != 3 {
        return Err(syntax(format!(
            "`{text}`: three channels and an optional `/ alpha`"
        )));
    }
    Ok((main, alpha))
}

/// `rgb(…)`, `hsl(…)`, `oklch(…)`, `color(<space> …)`: each component
/// typed for its slot (dials and `calc()` put in), then read by
/// `exact_color` from its numbers as the kernel reads the literal. A
/// legacy result keeps its unquantised sRGB for a further fold.
pub(crate) fn absolute(
    name: &str,
    args: &[Vec<Item>],
    text: &str,
    names: &mut dyn Names,
) -> Result<Folded, FoldError> {
    let not = || fail("tamber-kind", format!("`{text}` is not a colour"));
    let (space, slots, items): (MixSpace, [Slot; 3], Vec<&Item>) = if name == "color" {
        let [arg] = args else { return not() };
        let Some((Item::Ident(id), rest)) = arg.split_first() else {
            return not();
        };
        if hdr_space(id).is_some() {
            let (main, alpha) = channels(rest, text)?;
            let mut c = [0.0; 3];
            for i in 0..3 {
                c[i] = component(&main[i], Slot::Channel(1.0), &NO_CHANNELS, names)?;
            }
            let a = match alpha {
                Some(a) => component(a, Slot::Channel(1.0), &NO_CHANNELS, names)?,
                None => 1.0,
            };
            let n = exact_color::number_text;
            return match exact_color::parse(&format!(
                "color({id} {} {} {} / {})",
                n(c[0]),
                n(c[1]),
                n(c[2]),
                n(a)
            )) {
                Some(Parsed::Wide(w)) => Ok(Folded::One(Half::Wide(w))),
                _ => not(),
            };
        }
        let Some((space, _)) = color_space(id) else {
            return fail("tamber-syntax", format!("`{id}` is not a `color()` space"));
        };
        (space, [Slot::Channel(1.0); 3], rest.iter().collect())
    } else {
        let (space, _, slots) = spec(name).ok_or_else(|| syntax(format!("`{name}()`")))?;
        match args {
            [one] => (space, slots, one.iter().collect()),
            // CSS's legacy comma syntax: rgb() and hsl() only, one item each.
            [a, b, c, rest @ ..]
                if rest.len() <= 1 && matches!(name, "rgb" | "rgba" | "hsl" | "hsla") =>
            {
                let mut all: Vec<&Item> = Vec::new();
                for arg in [a, b, c] {
                    let [one] = arg.as_slice() else { return not() };
                    all.push(one);
                }
                if let [alpha] = rest {
                    let [one] = alpha.as_slice() else {
                        return not();
                    };
                    all.push(&Item::Slash);
                    all.push(one);
                }
                // Typed after calc(): rgb() all numbers or all percentages,
                // hsl() percentages after its hue.
                let mut pct = [false; 3];
                for i in 0..3 {
                    pct[i] = matches!(operand(all[i], &NO_CHANNELS, names)?, Num::Pct(_));
                }
                let legal = if name.starts_with("rgb") {
                    pct == [false; 3] || pct == [true; 3]
                } else {
                    pct[1] && pct[2]
                };
                if !legal {
                    return not();
                }
                (space, slots, all)
            }
            _ => return not(),
        }
    };
    let owned: Vec<Item> = items.into_iter().cloned().collect();
    let (main, alpha) = channels(&owned, text)?;
    let mut c = [0.0; 3];
    for i in 0..3 {
        c[i] = component(&main[i], slots[i], &NO_CHANNELS, names)?;
    }
    let a = match alpha {
        Some(a) => component(a, Slot::Channel(1.0), &NO_CHANNELS, names)?,
        None => 1.0,
    };
    let n = exact_color::number_text;
    let canonical = match space {
        MixSpace::Hsl | MixSpace::Hwb => {
            format!("{name}({} {}% {}% / {})", n(c[0]), n(c[1]), n(c[2]), n(a))
        }
        _ if name == "color" => {
            let [Item::Ident(id), ..] = args[0].as_slice() else {
                return not();
            };
            format!("color({id} {} {} {} / {})", n(c[0]), n(c[1]), n(c[2]), n(a))
        }
        _ => format!("{name}({} {} {} / {})", n(c[0]), n(c[1]), n(c[2]), n(a)),
    };
    match exact_color::parse(&canonical) {
        Some(Parsed::Legacy(c8)) => {
            let exact = match space {
                MixSpace::Srgb => Wide {
                    space: Space::Srgb,
                    c: c.map(|v| (v / 255.0).clamp(0.0, 1.0)),
                    alpha: a.clamp(0.0, 1.0),
                },
                _ => {
                    let s = [c[1].clamp(0.0, 100.0), c[2].clamp(0.0, 100.0)];
                    wide_from(space, [c[0].rem_euclid(360.0), s[0], s[1]], a)
                }
            };
            Ok(Folded::One(Half::Legacy(c8, exact)))
        }
        Some(Parsed::Wide(w)) => Ok(Folded::One(Half::Wide(w))),
        _ => not(),
    }
}

/// `rgb(from X r g b / alpha)`, `oklch(from X l c h)`, `color(from X
/// display-p3 r g b)` … (CSS Color 5 §4): X's channels in the function's
/// space (0–255 for `rgb()`), each component typed for its slot. The result
/// is `Wide` (LLP 1121 D7).
pub(crate) fn relative(
    name: &str,
    args: &[Vec<Item>],
    text: &str,
    names: &mut dyn Names,
) -> Result<Folded, FoldError> {
    let [arg] = args else {
        return fail(
            "tamber-syntax",
            format!("`{text}`: a relative colour has no commas"),
        );
    };
    let Some(source) = arg.get(1) else {
        return fail("tamber-syntax", format!("`{text}`: `from` takes a colour"));
    };
    let mut rest = &arg[2..];
    let (space, keys, slots) = match spec(name) {
        Some(s) => s,
        None => {
            let Some((Item::Ident(id), tail)) = rest.split_first() else {
                return fail(
                    "tamber-syntax",
                    format!("`{text}`: `color(from X <space> …)`"),
                );
            };
            if let Some(hdr) = hdr_space(id) {
                return relative_hdr(hdr, source, tail, text, names);
            }
            let Some((space, keys)) = color_space(id) else {
                return fail("tamber-syntax", format!("`{id}` is not a `color()` space"));
            };
            rest = tail;
            (space, keys, [Slot::Channel(1.0); 3])
        }
    };
    let bytes = matches!(name, "rgb" | "rgba");
    let (main, alpha) = channels(rest, text)?;
    let source = color_item(source, names)?;
    per_side(&[&source], text, |s| {
        let src = s[0].wide();
        let mut values = components(src, space);
        if bytes {
            values = values.map(|v| v * 255.0);
        }
        let polar = [
            matches!(space, MixSpace::Hsl | MixSpace::Hwb),
            false,
            matches!(space, MixSpace::Lch | MixSpace::Oklch),
        ];
        let rgb = components(src, MixSpace::Srgb);
        // CSS Color 4's thresholds: an hsl or hwb grey, a chroma under
        // 0.0015 in lch, under 0.000004 in oklch (§9.3, §9.5).
        let gray = match space {
            MixSpace::Hsl | MixSpace::Hwb => {
                rgb[0].max(rgb[1]).max(rgb[2]) - rgb[0].min(rgb[1]).min(rgb[2]) < 1e-6
            }
            MixSpace::Lch => values[1].abs() < 0.0015,
            _ => values[1].abs() < 0.000_004,
        };
        let ch = Channels {
            names: keys,
            values,
            missing: polar.map(|p| p && gray),
            alpha: Some(src.alpha),
        };
        let mut c = [0.0; 3];
        for i in 0..3 {
            c[i] = component(&main[i], slots[i], &ch, names)?;
        }
        let a = match alpha {
            Some(a) => component(a, Slot::Channel(1.0), &ch, names)?,
            None => src.alpha,
        };
        if bytes {
            c = c.map(|v| v / 255.0);
        }
        Ok(Half::Wide(wide_from(space, c, a)))
    })
}

/// A Rec. 2100 space `color()` names, which no interpolation method does.
fn hdr_space(id: &str) -> Option<Space> {
    match id.to_ascii_lowercase().as_str() {
        "rec2100-pq" => Some(Space::Rec2100Pq),
        "rec2100-hlg" => Some(Space::Rec2100Hlg),
        "rec2100-linear" => Some(Space::Rec2100Linear),
        _ => None,
    }
}

/// `color(from X rec2100-pq r g b)`: X's channels in the HDR space.
fn relative_hdr(
    space: Space,
    source: &Item,
    rest: &[Item],
    text: &str,
    names: &mut dyn Names,
) -> Result<Folded, FoldError> {
    let (main, alpha) = channels(rest, text)?;
    let source = color_item(source, names)?;
    per_side(&[&source], text, |s| {
        let src = s[0].wide();
        let values = if src.space == space {
            src.c
        } else {
            exact_color::rec2100_from_linear_srgb(space, src.linear_srgb()).unwrap_or(src.c)
        };
        let ch = Channels {
            names: ["r", "g", "b"],
            values,
            missing: [false; 3],
            alpha: Some(src.alpha),
        };
        let mut c = [0.0; 3];
        for i in 0..3 {
            c[i] = component(&main[i], Slot::Channel(1.0), &ch, names)?;
        }
        let a = match alpha {
            Some(a) => component(a, Slot::Channel(1.0), &ch, names)?,
            None => src.alpha,
        };
        Ok(Half::Wide(Wide {
            space,
            c,
            alpha: a.clamp(0.0, 1.0),
        }))
    })
}

/// `color-mix([in <space>,] a [p], b [q], …)` (CSS Color 5 §2): the method
/// is optional (oklab); one or more colours, their percentages normalised
/// (an omitted one takes an equal share of what is left, a sum over 100%
/// scales down, a sum under it is an alpha multiplier, 0% throughout an
/// equal mix at alpha 0), interpolated premultiplied in the mix's space
/// from each colour's exact components, one after another; the result is
/// `Wide` in that space even in sRGB (LLP 1121 D7).
pub(crate) fn mix(
    args: &[Vec<Item>],
    text: &str,
    names: &mut dyn Names,
) -> Result<Folded, FoldError> {
    let shape = "`color-mix([in <space>,] <colour> [<percentage>], …)`";
    let explicit = matches!(args.first().and_then(|a| a.first()), Some(Item::Ident(w)) if w.eq_ignore_ascii_case("in"));
    let (how, colours) = match args.split_first() {
        Some((first, rest)) if explicit => {
            let words: Vec<String> = first
                .iter()
                .map(|i| match i {
                    Item::Ident(w) => Ok(w.to_ascii_lowercase()),
                    _ => Err(syntax(shape)),
                })
                .collect::<Result<_, _>>()?;
            let words: Vec<&str> = words.iter().map(String::as_str).collect();
            let Some((how, _)) = Interpolation::parse(&words).filter(|(_, n)| *n == words.len())
            else {
                return fail("tamber-syntax", shape);
            };
            (how, rest)
        }
        _ => (Interpolation::OKLAB, args),
    };
    if colours.is_empty() {
        return fail(
            "tamber-syntax",
            format!("`{text}`: {shape} takes at least one colour"),
        );
    }
    let mut folded = Vec::new();
    let mut given = Vec::new();
    for arg in colours {
        let (color, pct): (Vec<&Item>, Vec<&Item>) = arg.iter().partition(|i| {
            !matches!(i, Item::Percent(_)) && !matches!(i, Item::Func(f, ..) if is_math(f))
        });
        let [color] = color.as_slice() else {
            return fail("tamber-syntax", shape);
        };
        let pct = match pct.as_slice() {
            [] => None,
            [p] => Some(component(p, Slot::Percentage, &NO_CHANNELS, names)?),
            _ => return fail("tamber-syntax", shape),
        };
        if pct.is_some_and(|p| !(0.0..=100.0).contains(&p)) {
            return fail("tamber-syntax", format!("`{text}`: percentages are 0–100%"));
        }
        folded.push(color_item(color, names)?);
        given.push(pct);
    }
    let (weights, multiplier) = normalise(&given);
    let refs: Vec<&Folded> = folded.iter().collect();
    per_side(&refs, text, |s| {
        // One colour after another: the running mix toward the next, by its
        // share of the weight so far (CSS Color 5's n-ary mix). Exact in a
        // rectangular space; in a polar one each step takes its own shorter
        // or longer hue, which can differ a little from one n-way average.
        let mut acc = end(s[0].wide(), how.space);
        let mut total = weights[0];
        for (h, &w) in s.iter().zip(&weights).skip(1) {
            total += w;
            // All 0% so far: an even step, as CSS Color 5 mixes them.
            let t = if total > 0.0 { w / total } else { 0.5 };
            acc = step(acc, end(h.wide(), how.space), t, how);
        }
        Ok(Half::Wide(wide_from(
            how.space,
            acc.c,
            acc.alpha * multiplier,
        )))
    })
}

/// CSS Color 5's percentage normalisation: the weights, summing to 1 (all
/// 0 when every percentage is), and the alpha multiplier.
fn normalise(given: &[Option<f64>]) -> (Vec<f64>, f64) {
    let specified: f64 = given.iter().flatten().sum();
    let omitted = given.iter().filter(|p| p.is_none()).count();
    let share = if omitted > 0 {
        (100.0 - specified).max(0.0) / omitted as f64
    } else {
        0.0
    };
    let all: Vec<f64> = given.iter().map(|p| p.unwrap_or(share)).collect();
    let sum: f64 = all.iter().sum();
    if sum <= 0.0 {
        return (all, 0.0);
    }
    (
        all.iter().map(|w| w / sum).collect(),
        (sum / 100.0).min(1.0),
    )
}

/// A colour at one end of an interpolation in a space: its components, its
/// alpha, and whether its hue is powerless (an achromatic colour's).
#[derive(Clone, Copy)]
struct End {
    c: [f64; 3],
    alpha: f64,
    gray: bool,
}

fn polar(space: MixSpace) -> Option<usize> {
    match space {
        MixSpace::Hsl | MixSpace::Hwb => Some(0),
        MixSpace::Lch | MixSpace::Oklch => Some(2),
        _ => None,
    }
}

fn end(w: Wide, space: MixSpace) -> End {
    let c = components(w, space);
    let gray = match space {
        MixSpace::Hsl | MixSpace::Hwb => {
            let rgb = components(w, MixSpace::Srgb);
            rgb[0].max(rgb[1]).max(rgb[2]) - rgb[0].min(rgb[1]).min(rgb[2]) < 1e-6
        }
        MixSpace::Lch | MixSpace::Oklch => c[1].abs() < 1e-6,
        _ => false,
    };
    End {
        c,
        alpha: w.alpha,
        gray,
    }
}

/// One interpolation between two ends (CSS Color 4 §12): premultiplied in
/// `how`'s space, a powerless hue taking the other's, the hue going round by
/// its method. The result's hue is powerless only where both ends' were, so
/// a running n-ary mix keeps the hue it has.
fn step(mut a: End, mut b: End, t: f64, how: Interpolation) -> End {
    let hue = polar(how.space);
    if let Some(hue) = hue {
        match (a.gray, b.gray) {
            (true, false) => a.c[hue] = b.c[hue],
            (false, true) => b.c[hue] = a.c[hue],
            _ => {}
        }
        let (h1, h2) = (a.c[hue], b.c[hue]);
        let d = h2 - h1;
        b.c[hue] = match how.hue {
            HueMethod::Shorter if d > 180.0 => h2 - 360.0,
            HueMethod::Shorter if d < -180.0 => h2 + 360.0,
            HueMethod::Longer if d > 0.0 && d < 180.0 => h2 - 360.0,
            // As `exact_color::mix` takes the bound, so a transition agrees.
            HueMethod::Longer if (-180.0..=0.0).contains(&d) => h2 + 360.0,
            HueMethod::Increasing if d < 0.0 => h2 + 360.0,
            HueMethod::Decreasing if d > 0.0 => h2 - 360.0,
            _ => h2,
        };
    }
    let alpha = a.alpha + (b.alpha - a.alpha) * t;
    let c = std::array::from_fn(|i| {
        if hue == Some(i) {
            return (a.c[i] + (b.c[i] - a.c[i]) * t).rem_euclid(360.0);
        }
        let (pa, pb) = (a.c[i] * a.alpha, b.c[i] * b.alpha);
        let v = pa + (pb - pa) * t;
        if alpha > 0.0 {
            v / alpha
        } else {
            v
        }
    });
    End {
        c,
        alpha,
        gray: a.gray && b.gray,
    }
}
