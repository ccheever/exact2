//! The `animation` shorthand and `@keyframes` bodies, in CSS's grammar.
//!
//! @ref LLP 1055 D5; CSS Animations 1 §3 (`@keyframes`), §4.10 (the shorthand)
//!
//! `animation: <name> || <duration> || <easing> || <delay> || <count> ||
//! <direction> || <fill-mode> || <play-state>, …` — the first time is the
//! duration and the second the delay; a keyword that could belong to another
//! longhand goes there first, and the name is what is left. A keyframes body
//! is `from{r:3;opacity:.5}50%,75%{…}to{…}`, the form Contract's `keyframes`
//! declaration lowers to and the web host emits.

use super::{Animation, AnimationError, Animations, Direction, FillMode, Keyframe, Keyframes};
use crate::easing::Easing;
use crate::parse::{easing, split_top_level, time, ParseError};
use crate::property::{Property, Value};
use crate::transition::TimingFunction;
use exact_num::Shortest;
use std::fmt::Write as _;

/// A grammar: the value, or why CSS refuses the text.
type Grammar<T> = fn(&str) -> Result<T, ParseError>;

/// The two grammars a link registers.
struct Grammars {
    animations: Grammar<Animations>,
    keyframes: Grammar<Keyframes>,
}

const GRAMMARS: Grammars = Grammars {
    animations: Animations::grammar,
    keyframes: Keyframes::grammar,
};

/// The grammars, once linked ([`link`]). Only a wasm artifact reads it.
#[cfg(target_arch = "wasm32")]
static LINKED: std::sync::OnceLock<&'static Grammars> = std::sync::OnceLock::new();

/// Link the `animation` shorthand's and `@keyframes`' grammars (LLP 1047
/// D2, linked by use): ~5 KiB of a web core that none of Caltrain, RealWorld
/// or the video player animates with. A web artifact links them when its
/// plan declares keyframes or binds `animation` or `exit-animation`, and a
/// plan that does so unlinked is refused at boot (D6), so an unlinked
/// artifact never parses one. Native artifacts and the compiler parse
/// without it.
pub fn link() {
    #[cfg(target_arch = "wasm32")]
    let _ = LINKED.set(&GRAMMARS);
}

/// `text` by the grammar `pick` names, on the web once linked.
fn parse_linked<T>(text: &str, pick: fn(&Grammars) -> Grammar<T>) -> Result<T, ParseError> {
    #[cfg(target_arch = "wasm32")]
    return LINKED.get().map_or_else(
        || {
            Err(ParseError::BadShape(
                "CSS animations are not linked into this artifact (LLP 1047 D6)".into(),
            ))
        },
        |grammars| pick(grammars)(text),
    );
    #[cfg(not(target_arch = "wasm32"))]
    pick(&GRAMMARS)(text)
}

impl Animations {
    /// Parse the shorthand. Names stay unresolved (no keyframes) until
    /// [`Animations::resolve`]. On the web, once linked ([`link`]).
    pub fn parse(text: &str) -> Result<Animations, ParseError> {
        parse_linked(text, |g| g.animations)
    }

    fn grammar(text: &str) -> Result<Animations, ParseError> {
        let text = text.trim();
        if text.is_empty() || text == "none" {
            return Ok(Animations::NONE);
        }
        let mut out = Vec::new();
        for decl in split_top_level(text, ',') {
            let decl = decl.trim();
            let parts: Vec<&str> = split_top_level(decl, ' ')
                .into_iter()
                .filter(|p| !p.is_empty())
                .collect();
            if parts.is_empty() || parts.len() > 8 {
                return Err(ParseError::BadShape(decl.to_string()));
            }
            let mut a = Animation::default();
            let (mut times, mut eased, mut counted) = (0, false, false);
            let (mut directed, mut filled, mut stated, mut named) = (false, false, false, false);
            for part in parts {
                if let Ok(t) = time(part) {
                    match times {
                        0 => a.duration = t,
                        1 => a.delay = t,
                        _ => return Err(ParseError::BadShape(decl.to_string())),
                    }
                    times += 1;
                } else if !eased && easing_keyword(part).is_some() {
                    a.easing = easing_keyword(part).unwrap()?;
                    eased = true;
                } else if !counted && count(part).is_some() {
                    a.iterations = count(part).unwrap();
                    counted = true;
                } else if !directed && direction(part).is_some() {
                    a.direction = direction(part).unwrap();
                    directed = true;
                } else if !filled && fill(part).is_some() {
                    a.fill = fill(part).unwrap();
                    filled = true;
                } else if !stated && matches!(part, "running" | "paused") {
                    a.paused = part == "paused";
                    stated = true;
                } else if !named && is_name(part) {
                    a.name = part.trim_matches(|c| c == '"' || c == '\'').to_string();
                    named = true;
                } else {
                    return Err(ParseError::BadShape(decl.to_string()));
                }
            }
            a.validate().map_err(ParseError::InvalidAnimation)?;
            if named && a.name != "none" {
                out.push(a);
            }
        }
        if out.len() > super::MAX_ANIMATIONS {
            return Err(ParseError::InvalidAnimation(AnimationError::TooMany));
        }
        Ok(Animations(out))
    }

    /// Give each entry the keyframes its name resolves to. An entry whose
    /// name matches no rule is dropped: CSS starts no animation for it
    /// (CSS Animations 1 §3). Returns the names dropped.
    pub fn resolve<'k>(
        &mut self,
        mut lookup: impl FnMut(&str) -> Option<&'k Keyframes>,
    ) -> Vec<String> {
        let mut dropped = Vec::new();
        self.0.retain_mut(|a| match lookup(&a.name) {
            Some(k) => {
                a.keyframes = k.clone();
                true
            }
            None => {
                dropped.push(std::mem::take(&mut a.name));
                false
            }
        });
        dropped
    }

    /// The shorthand, every longhand spelled out, as CSS reads it.
    pub fn css(&self) -> String {
        self.css_named(&|a| std::borrow::Cow::Borrowed(&a.name))
    }

    /// [`Animations::css`], each entry naming the rule `name` gives it (a
    /// host's variant of a rule, say).
    pub fn css_named(&self, name: &dyn Fn(&Animation) -> std::borrow::Cow<'_, str>) -> String {
        if self.0.is_empty() {
            return "none".into();
        }
        let mut out = String::new();
        for (i, a) in self.0.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            let _ = write!(
                out,
                "{}s {} {}s ",
                Shortest(a.duration),
                easing_css(&a.easing),
                Shortest(a.delay)
            );
            if a.iterations.is_infinite() {
                out.push_str("infinite");
            } else {
                let _ = write!(out, "{}", Shortest(a.iterations));
            }
            let _ = write!(
                out,
                " {} {} {} {}",
                a.direction.name(),
                a.fill.name(),
                if a.paused { "paused" } else { "running" },
                css_ident(&name(a))
            );
        }
        out
    }
}

/// The eight longhands `animation` sets, in CSS's order.
pub const LONGHANDS: [&str; 8] = [
    "animation-name",
    "animation-duration",
    "animation-timing-function",
    "animation-delay",
    "animation-iteration-count",
    "animation-direction",
    "animation-fill-mode",
    "animation-play-state",
];

impl Animation {
    /// Set one longhand from one item of its list (CSS Animations 1 §4).
    pub fn set_longhand(&mut self, name: &str, value: &str) -> Result<(), ParseError> {
        let value = value.trim();
        let bad = || ParseError::BadValue(format!("{name}: {value}"));
        match name {
            "animation-name" if is_name(value) => {
                self.name = value.trim_matches(|c| c == '"' || c == '\'').to_string()
            }
            "animation-duration" => self.duration = time(value)?,
            "animation-delay" => self.delay = time(value)?,
            "animation-timing-function" => self.easing = easing_only(value)?,
            "animation-iteration-count" => self.iterations = count(value).ok_or_else(bad)?,
            "animation-direction" => self.direction = direction(value).ok_or_else(bad)?,
            "animation-fill-mode" => self.fill = fill(value).ok_or_else(bad)?,
            "animation-play-state" => {
                self.paused = match value {
                    "paused" => true,
                    "running" => false,
                    _ => return Err(bad()),
                }
            }
            _ => return Err(bad()),
        }
        self.validate().map_err(ParseError::InvalidAnimation)
    }
}

impl Keyframes {
    /// Parse a keyframes body: `<selectors>{<property>:<value>;…}…`. Keyframes
    /// sharing an offset and timing function merge (a later value wins); the
    /// result is in offset order, stable for equal offsets.
    /// On the web, once linked ([`link`]).
    pub fn parse(text: &str) -> Result<Keyframes, ParseError> {
        parse_linked(text, |g| g.keyframes)
    }

    fn grammar(text: &str) -> Result<Keyframes, ParseError> {
        let mut frames: Vec<Keyframe> = Vec::new();
        let mut rest = text.trim();
        while !rest.is_empty() {
            let open = rest
                .find('{')
                .ok_or_else(|| ParseError::BadShape(rest.to_string()))?;
            let close = rest[open..]
                .find('}')
                .map(|i| open + i)
                .ok_or_else(|| ParseError::BadShape(rest.to_string()))?;
            let selectors = &rest[..open];
            let body = &rest[open + 1..close];
            let mut easing_here = None;
            let mut values: Vec<(Property, Value)> = Vec::new();
            let mut dark: Vec<(Property, Value)> = Vec::new();
            for decl in split_top_level(body, ';')
                .into_iter()
                .map(str::trim)
                .filter(|d| !d.is_empty())
            {
                let (name, value) = decl
                    .split_once(':')
                    .ok_or_else(|| ParseError::BadShape(decl.to_string()))?;
                let (name, value) = (name.trim(), value.trim());
                if name == "animation-timing-function" {
                    easing_here = Some(easing_only(value)?);
                    continue;
                }
                let property = Property::from_name(name)
                    .filter(|p| *p != Property::Height)
                    .ok_or_else(|| ParseError::UnknownProperty(name.to_string()))?;
                for (property, light, night) in keyframe_values(property, value)? {
                    values.retain(|(p, _)| *p != property);
                    dark.retain(|(p, _)| *p != property);
                    values.push((property, light));
                    if let Some(night) = night {
                        dark.push((property, night));
                    }
                }
            }
            for selector in selectors.split(',').map(str::trim) {
                let offset = match selector {
                    "from" => 0.0,
                    "to" => 1.0,
                    s => s
                        .strip_suffix('%')
                        .and_then(|n| exact_num::parse_f64(n.trim()).ok())
                        .filter(|n| (0.0..=100.0).contains(n))
                        .map(|n| n / 100.0)
                        .ok_or_else(|| ParseError::BadShape(s.to_string()))?,
                };
                match frames
                    .iter_mut()
                    .find(|f| f.offset == offset && f.easing == easing_here)
                {
                    Some(frame) => {
                        for (p, v) in &values {
                            frame.values.retain(|(q, _)| q != p);
                            frame.dark.retain(|(q, _)| q != p);
                            frame.values.push((*p, *v));
                        }
                        frame.dark.extend(dark.iter().copied());
                    }
                    None => frames.push(Keyframe {
                        offset,
                        easing: easing_here.clone(),
                        values: values.clone(),
                        dark: dark.clone(),
                    }),
                }
            }
            rest = rest[close + 1..].trim_start();
        }
        frames.sort_by(|a, b| a.offset.total_cmp(&b.offset));
        let keyframes = Keyframes(frames);
        keyframes.validate().map_err(ParseError::InvalidAnimation)?;
        Ok(keyframes)
    }

    /// The body as CSS (percentages, CSS units), for a web host's
    /// `@keyframes name { … }`.
    pub fn css(&self) -> String {
        let mut out = String::new();
        for frame in &self.0 {
            let _ = write!(out, "{}%{{", Shortest(frame.offset * 100.0));
            if let Some(e) = &frame.easing {
                let _ = write!(out, "animation-timing-function:{};", easing_css(e));
            }
            // The shadow's colour is written in its `box-shadow`.
            for (p, v) in frame
                .values
                .iter()
                .filter(|(p, _)| *p != Property::ShadowColor)
            {
                let colour = |p: Property, v: Value| match frame.dark.iter().find(|(q, _)| *q == p)
                {
                    Some((_, night)) => format!(
                        "light-dark({}, {})",
                        crate::color::css(v),
                        crate::color::css(*night)
                    ),
                    None => crate::color::css(v),
                };
                let text = match p {
                    p if p.is_color() => colour(*p, *v),
                    Property::BoxShadow => {
                        let shade = frame
                            .values
                            .iter()
                            .find(|(q, _)| *q == Property::ShadowColor)
                            .map_or(Value::ZERO, |(_, c)| *c);
                        format!(
                            "{} {}",
                            value_css(*p, *v),
                            colour(Property::ShadowColor, shade)
                        )
                    }
                    _ => value_css(*p, *v),
                };
                let _ = write!(out, "{}:{};", p.css_name(), text);
            }
            out.push('}');
        }
        out
    }
}

/// A property value in CSS, with the unit CSS requires.
pub fn value_css(property: Property, v: Value) -> String {
    match property {
        Property::Translate => {
            let axis = |px: f64, pct: f64| match (px, pct) {
                (_, 0.0) => format!("{}px", Shortest(px)),
                (0.0, _) => format!("{}%", Shortest(pct)),
                _ => format!("calc({}px + {}%)", Shortest(px), Shortest(pct)),
            };
            format!("{} {}", axis(v.x, v.z), axis(v.y, v.w))
        }
        Property::Rotate => format!("{}deg", Shortest(v.x)),
        Property::R
        | Property::Height
        | Property::Cx
        | Property::Cy
        | Property::X
        | Property::Y
        | Property::Rx
        | Property::Ry => format!("{}px", Shortest(v.x)),
        Property::Scale | Property::Opacity | Property::StrokeDashoffset => {
            format!("{}", Shortest(v.x))
        }
        // Offset and blur; the colour is `box-shadow`'s other half.
        Property::BoxShadow => format!(
            "{}px {}px {}px",
            Shortest(v.x),
            Shortest(v.y),
            Shortest(v.z)
        ),
        Property::Layout => format!(
            "{}px {}px {}px {}px",
            Shortest(v.x),
            Shortest(v.y),
            Shortest(v.z),
            Shortest(v.w)
        ),
        _ => crate::color::css(v),
    }
}

/// An easing in CSS.
pub fn easing_css(e: &Easing) -> String {
    match e {
        Easing::Linear => "linear".into(),
        Easing::Ease => "ease".into(),
        Easing::EaseIn => "ease-in".into(),
        Easing::EaseOut => "ease-out".into(),
        Easing::EaseInOut => "ease-in-out".into(),
        Easing::CubicBezier { x1, y1, x2, y2 } => format!(
            "cubic-bezier({}, {}, {}, {})",
            Shortest(*x1),
            Shortest(*y1),
            Shortest(*x2),
            Shortest(*y2)
        ),
        Easing::Steps { count, position } => format!("steps({count}, {})", position.name()),
        Easing::PiecewiseLinear(stops) => {
            let parts: Vec<String> = stops
                .iter()
                .map(|s| format!("{} {}%", Shortest(s.output), Shortest(s.input * 100.0)))
                .collect();
            format!("linear({})", parts.join(", "))
        }
    }
}

fn css_ident(name: &str) -> String {
    if name.is_empty() || !is_name(name) {
        format!("\"{}\"", name.replace('"', "\\\""))
    } else {
        name.to_string()
    }
}

fn easing_keyword(part: &str) -> Option<Result<Easing, ParseError>> {
    let known = matches!(
        part,
        "linear" | "ease" | "ease-in" | "ease-out" | "ease-in-out" | "step-start" | "step-end"
    ) || ["cubic-bezier(", "steps(", "linear(", "spring("]
        .iter()
        .any(|f| part.starts_with(f));
    known.then(|| easing_only(part))
}

fn easing_only(part: &str) -> Result<Easing, ParseError> {
    match easing(part)? {
        TimingFunction::Easing(e) => Ok(e),
        TimingFunction::Spring(_) => Err(ParseError::BadEasing(format!(
            "{part}: a spring is a transition timing function, not an animation's"
        ))),
    }
}

fn count(part: &str) -> Option<f64> {
    if part == "infinite" {
        return Some(f64::INFINITY);
    }
    exact_num::parse_f64(part).ok().filter(|n| *n >= 0.0)
}

fn direction(part: &str) -> Option<Direction> {
    Some(match part {
        "normal" => Direction::Normal,
        "reverse" => Direction::Reverse,
        "alternate" => Direction::Alternate,
        "alternate-reverse" => Direction::AlternateReverse,
        _ => return None,
    })
}

fn fill(part: &str) -> Option<FillMode> {
    Some(match part {
        "none" => FillMode::None,
        "forwards" => FillMode::Forwards,
        "backwards" => FillMode::Backwards,
        "both" => FillMode::Both,
        _ => return None,
    })
}

/// A CSS `<custom-ident>` (or a quoted string) that is not a CSS-wide keyword.
fn is_name(part: &str) -> bool {
    if part.len() >= 2
        && ((part.starts_with('"') && part.ends_with('"'))
            || (part.starts_with('\'') && part.ends_with('\'')))
    {
        return true;
    }
    let mut chars = part.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let starts = first.is_ascii_alphabetic()
        || first == '_'
        || (first == '-' && part.len() > 1 && !part[1..].starts_with(|c: char| c.is_ascii_digit()));
    starts
        && part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !matches!(
            part,
            "initial" | "inherit" | "unset" | "revert" | "revert-layer" | "default"
        )
}

fn length(value: &str) -> Option<f64> {
    let n = value.strip_suffix("px").unwrap_or(value);
    exact_num::parse_f64(n.trim()).ok()
}

/// One declaration's values: usually one property's, a `light-dark()`
/// colour's dark value beside it (LLP 1062 D9), and `box-shadow`'s two
/// halves, its geometry and its colour.
fn keyframe_values(
    property: Property,
    value: &str,
) -> Result<Vec<(Property, Value, Option<Value>)>, ParseError> {
    let bad = || ParseError::BadValue(format!("{}: {value}", property.name()));
    let colour = |text: &str| -> Result<(Value, Option<Value>), ParseError> {
        let text = text.trim();
        if let Some(body) = text
            .strip_prefix("light-dark(")
            .and_then(|b| b.strip_suffix(')'))
        {
            let parts = split_top_level(body, ',');
            let [light, night] = parts.as_slice() else {
                return Err(bad());
            };
            let light = crate::color::parse(light).ok_or_else(bad)?;
            let night = crate::color::parse(night).ok_or_else(bad)?;
            return Ok((light, Some(night)));
        }
        Ok((crate::color::parse(text).ok_or_else(bad)?, None))
    };
    Ok(match property {
        p if p.is_color() => {
            let (light, night) = colour(value)?;
            vec![(p, light, night)]
        }
        // `none` is CSS's transparent, zero-length shadow, which a shadow
        // interpolates from and to.
        Property::BoxShadow if value.trim() == "none" => vec![
            (Property::BoxShadow, Value::ZERO, None),
            (Property::ShadowColor, Value::ZERO, None),
        ],
        Property::BoxShadow => {
            // `<x> <y> [<blur>] <colour>`, one outer shadow, the colour last.
            let parts: Vec<&str> = split_top_level(value.trim(), ' ')
                .into_iter()
                .filter(|p| !p.is_empty())
                .collect();
            let (color, lengths) = parts.split_last().ok_or_else(bad)?;
            let lengths: Vec<f64> = lengths
                .iter()
                .map(|l| length(l).ok_or_else(bad))
                .collect::<Result<_, _>>()?;
            let (x, y, blur) = match lengths.as_slice() {
                [x, y] => (*x, *y, 0.0),
                [x, y, blur] if *blur >= 0.0 => (*x, *y, *blur),
                _ => return Err(bad()),
            };
            let (light, night) = colour(color)?;
            vec![
                (Property::BoxShadow, Value::four(x, y, blur, 0.0), None),
                (Property::ShadowColor, light, night),
            ]
        }
        p => vec![(p, keyframe_value(p, value)?, None)],
    })
}

fn keyframe_value(property: Property, value: &str) -> Result<Value, ParseError> {
    let bad = || ParseError::BadValue(format!("{}: {value}", property.name()));
    Ok(match property {
        // A length or a percentage of the box per axis (chess diary #4):
        // lengths in x and y, percentages in z and w.
        Property::Translate => {
            let axis = |part: &str| match part.strip_suffix('%') {
                Some(n) => exact_num::parse_f64(n.trim())
                    .ok()
                    .filter(|n| n.is_finite())
                    .map(|n| (0.0, n)),
                None => length(part).map(|l| (l, 0.0)),
            };
            let parts: Vec<&str> = value.split_whitespace().collect();
            let ((x, px), (y, py)) = match parts.as_slice() {
                [x] => (axis(x).ok_or_else(bad)?, (0.0, 0.0)),
                [x, y] => (axis(x).ok_or_else(bad)?, axis(y).ok_or_else(bad)?),
                _ => return Err(bad()),
            };
            Value::four(x, y, px, py)
        }
        Property::Rotate => {
            let n = value.strip_suffix("deg").unwrap_or(value);
            Value::scalar(exact_num::parse_f64(n.trim()).map_err(|_| bad())?)
        }
        Property::Scale | Property::Opacity => {
            let v = match value.strip_suffix('%') {
                Some(n) => exact_num::parse_f64(n.trim()).map_err(|_| bad())? / 100.0,
                None => exact_num::parse_f64(value).map_err(|_| bad())?,
            };
            Value::scalar(v)
        }
        Property::StrokeDashoffset
        | Property::R
        | Property::Height
        | Property::Cx
        | Property::Cy
        | Property::X
        | Property::Y
        | Property::Rx
        | Property::Ry => Value::scalar(length(value).ok_or_else(bad)?),
        p if p.is_color() => crate::color::parse(value).ok_or_else(bad)?,
        _ => return Err(bad()),
    })
}
