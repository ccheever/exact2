//! CSS `transition` shorthand → [`Transitions`].
//!
//! @ref LLP 1002 §2 (the web is the standard: the authored form is CSS's own
//! `transition` shorthand; `spring(stiffness, damping, mass)` is the one
//! declared extension)
//!
//! `transition: <property> || <duration> || <easing> || <delay>, …` — each
//! part in CSS's own grammar, with `spring(k, d, m)` admitted where an easing
//! goes. The first time is the duration and the second is the delay, wherever
//! the other components occur. What CSS's parser would reject, this rejects,
//! by name.

use crate::animation::{
    Animation, AnimationError, Animations, Direction, FillMode, KeyframeBlock, Keyframes, PlayState,
};
use crate::easing::{Easing, LinearStop, StepPosition};
use crate::property::{Property, Value};
use crate::spring::SpringConfig;
use crate::transition::{
    TimingFunction, Transition, TransitionError, TransitionProperty, Transitions,
};

/// Why a shorthand was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// An empty declaration.
    Empty,
    /// A property name outside `all` and the supported [`Property`] names.
    UnknownProperty(String),
    /// A time without `s`/`ms`, or not a number.
    BadTime(String),
    /// An easing name or function this parser does not know.
    BadEasing(String),
    /// A declaration with too many or too few parts.
    BadShape(String),
    /// Parsed, but the evaluator refuses it.
    Invalid(TransitionError),
    /// An `animation` named keyframes the text does not declare.
    UnknownKeyframes(String),
    /// A `@keyframes` rule that does not parse.
    BadKeyframes(String),
    /// `spring()` is a `transition` extension; keyframes take CSS easings.
    SpringInAnimation,
    /// Parsed, but the sampler refuses it.
    InvalidAnimation(AnimationError),
}

impl Transitions {
    /// Parse the CSS shorthand.
    pub fn parse(text: &str) -> Result<Transitions, ParseError> {
        let text = text.trim();
        if text.is_empty() || text == "none" {
            return Ok(Transitions::NONE);
        }
        let mut out = Vec::new();
        for decl in split_top_level(text, ',') {
            let parts: Vec<&str> = split_top_level(decl.trim(), ' ')
                .into_iter()
                .filter(|p| !p.is_empty())
                .collect();
            if parts.is_empty() || parts.len() > 4 {
                return Err(ParseError::BadShape(decl.trim().to_string()));
            }
            let mut property = None;
            let mut duration = 0.0;
            let mut delay = 0.0;
            let mut timing = None;
            let mut times = 0;
            for part in &parts {
                if let Ok(t) = time(part) {
                    match times {
                        0 => duration = t,
                        1 => delay = t,
                        _ => return Err(ParseError::BadShape(decl.trim().to_string())),
                    }
                    times += 1;
                } else if *part == "all" || Property::from_name(part).is_some() {
                    if property.is_some() {
                        return Err(ParseError::BadShape(decl.trim().to_string()));
                    }
                    property = Some(match *part {
                        "all" => TransitionProperty::All,
                        name => TransitionProperty::Property(Property::from_name(name).unwrap()),
                    });
                } else {
                    match easing(part) {
                        Ok(value) if timing.is_none() => timing = Some(value),
                        Ok(_) => return Err(ParseError::BadShape(decl.trim().to_string())),
                        Err(_) if property.is_none() => {
                            return Err(ParseError::UnknownProperty((*part).to_string()))
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
            let t = Transition {
                property: property.unwrap_or(TransitionProperty::All),
                duration,
                delay,
                timing: timing.unwrap_or(TimingFunction::Easing(Easing::Ease)),
            };
            t.validate().map_err(ParseError::Invalid)?;
            out.push(t);
        }
        let ts = Transitions(out);
        ts.validate().map_err(ParseError::Invalid)?;
        Ok(ts)
    }
}

impl Animations {
    /// Parse the row's text form: CSS's `animation` shorthand list, then the
    /// `@keyframes` rules it names, in CSS syntax —
    /// `pulse 1.6s ease-in-out infinite @keyframes pulse{0%{opacity:0.4}50%{opacity:1}}`.
    /// A compiled plan carries this (LLP 1057 D3): the compiler appends each
    /// rule an authored name resolves to, so nothing is looked up later.
    pub fn parse(text: &str) -> Result<Animations, ParseError> {
        let (head, rules) = match text.find("@keyframes") {
            Some(at) => (&text[..at], keyframes_rules(&text[at..])?),
            None => (text, Vec::new()),
        };
        let head = head.trim();
        if head.is_empty() || head == "none" {
            return Ok(Animations::NONE);
        }
        let mut out = Vec::new();
        for entry in split_top_level(head, ',') {
            let entry = entry.trim();
            let bad = || ParseError::BadShape(entry.to_string());
            let (mut duration, mut delay, mut easing, mut iterations) = (None, None, None, None);
            let (mut direction, mut fill, mut play_state, mut name) = (None, None, None, None);
            for part in split_top_level(entry, ' ')
                .into_iter()
                .filter(|p| !p.is_empty())
            {
                let direction_word = Direction::ALL.into_iter().find(|d| d.name() == part);
                let fill_word = FillMode::ALL.into_iter().find(|f| f.name() == part);
                let play_word = PlayState::ALL.into_iter().find(|p| p.name() == part);
                // CSS §3.11: a word that fits a longhand not yet set is that
                // longhand's, so the first `none` is the fill mode and only a
                // second one is the name.
                if let Ok(t) = time(part) {
                    match (duration, delay) {
                        (None, _) => duration = Some(t),
                        (Some(_), None) => delay = Some(t),
                        _ => return Err(bad()),
                    }
                } else if easing.is_none() && is_easing(part) {
                    easing = Some(Easing::parse(part)?);
                } else if iterations.is_none() && part == "infinite" {
                    iterations = Some(f64::INFINITY);
                } else if iterations.is_none() && exact_num::parse_f64(part).is_ok() {
                    iterations = exact_num::parse_f64(part).ok();
                } else if direction.is_none() && direction_word.is_some() {
                    direction = direction_word;
                } else if fill.is_none() && fill_word.is_some() {
                    fill = fill_word;
                } else if play_state.is_none() && play_word.is_some() {
                    play_state = play_word;
                } else if name.is_none() && is_ident(part) {
                    name = Some(part);
                } else {
                    return Err(bad());
                }
            }
            let Some(name) = name.filter(|n| *n != "none") else {
                continue;
            };
            let keyframes = rules
                .iter()
                .rev()
                .find(|k| k.name == name)
                .cloned()
                .ok_or_else(|| ParseError::UnknownKeyframes(name.to_string()))?;
            out.push(Animation {
                keyframes,
                duration: duration.unwrap_or(0.0),
                easing: easing.unwrap_or(Easing::Ease),
                delay: delay.unwrap_or(0.0),
                iterations: iterations.unwrap_or(1.0),
                direction: direction.unwrap_or_default(),
                fill: fill.unwrap_or_default(),
                play_state: play_state.unwrap_or_default(),
            });
        }
        let animations = Animations(out);
        animations
            .validate()
            .map_err(ParseError::InvalidAnimation)?;
        Ok(animations)
    }
}

impl Easing {
    /// Parse one CSS `<easing-function>`. `spring()`, a `transition`
    /// extension, is refused.
    pub fn parse(text: &str) -> Result<Easing, ParseError> {
        match easing(text.trim())? {
            TimingFunction::Easing(e) => Ok(e),
            TimingFunction::Spring(_) => Err(ParseError::SpringInAnimation),
        }
    }
}

impl Keyframes {
    /// Parse one `@keyframes name{…}` rule in CSS syntax. Blocks sort by
    /// offset; blocks at one offset merge, the later value winning (CSS
    /// Animations §3: keyframes at the same offset cascade).
    pub fn parse(text: &str) -> Result<Keyframes, ParseError> {
        let mut all = keyframes_rules(text)?;
        match all.len() {
            1 => Ok(all.remove(0)),
            _ => Err(ParseError::BadKeyframes(text.trim().to_string())),
        }
    }
}

/// Every `@keyframes` rule in `text`, which holds nothing else.
fn keyframes_rules(text: &str) -> Result<Vec<Keyframes>, ParseError> {
    let mut rules = Vec::new();
    let mut rest = text.trim_start();
    while !rest.is_empty() {
        let bad = || ParseError::BadKeyframes(rest.chars().take(64).collect());
        let body = rest.strip_prefix("@keyframes").ok_or_else(bad)?;
        let open = body.find('{').ok_or_else(bad)?;
        let name = body[..open].trim();
        if !is_ident(name) {
            return Err(bad());
        }
        // The rule ends at the brace that closes its own.
        let mut depth = 0;
        let mut close = None;
        for (i, c) in body[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(open + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let close = close.ok_or_else(bad)?;
        rules.push(keyframe_blocks(name, &body[open + 1..close])?);
        rest = body[close + 1..].trim_start();
    }
    Ok(rules)
}

fn keyframe_blocks(name: &str, inner: &str) -> Result<Keyframes, ParseError> {
    let bad = |what: &str| ParseError::BadKeyframes(format!("@keyframes {name}: {what}"));
    let mut blocks: Vec<KeyframeBlock> = Vec::new();
    let mut rest = inner.trim();
    while !rest.is_empty() {
        let open = rest.find('{').ok_or_else(|| bad(rest))?;
        let close = rest.find('}').ok_or_else(|| bad(rest))?;
        if close < open {
            return Err(bad(rest));
        }
        let mut block = KeyframeBlock {
            offset: 0.0,
            easing: None,
            values: Vec::new(),
        };
        for decl in rest[open + 1..close].split(';') {
            let decl = decl.trim();
            if decl.is_empty() {
                continue;
            }
            let (property, value) = decl.split_once(':').ok_or_else(|| bad(decl))?;
            let (property, value) = (property.trim(), value.trim());
            if property == "animation-timing-function" {
                block.easing = Some(Easing::parse(value)?);
                continue;
            }
            let property = Property::from_name(property)
                .ok_or_else(|| ParseError::UnknownProperty(property.to_string()))?;
            let value = keyframe_value(property, value).ok_or_else(|| bad(decl))?;
            block.values.retain(|(p, _)| *p != property);
            block.values.push((property, value));
        }
        for selector in rest[..open].split(',') {
            let offset = match selector.trim() {
                "from" => 0.0,
                "to" => 1.0,
                s => s
                    .strip_suffix('%')
                    .and_then(|n| exact_num::parse_f64(n.trim()).ok())
                    .map(|n| n / 100.0)
                    .ok_or_else(|| bad(s))?,
            };
            blocks.push(KeyframeBlock {
                offset,
                ..block.clone()
            });
        }
        rest = rest[close + 1..].trim_start();
    }
    Keyframes::new(name, blocks).map_err(ParseError::InvalidAnimation)
}

/// A keyframe's value in CSS's units: `translate` one or two lengths (`px`,
/// or a unitless number), `rotate` an angle (`deg`, or a unitless number of
/// degrees), `scale` and `opacity` numbers.
fn keyframe_value(property: Property, text: &str) -> Option<Value> {
    let number = |s: &str, unit: &str| exact_num::parse_f64(s.strip_suffix(unit).unwrap_or(s)).ok();
    match property {
        Property::Translate => {
            let mut parts = text.split_whitespace();
            let x = number(parts.next()?, "px")?;
            let y = parts.next().map_or(Some(0.0), |y| number(y, "px"))?;
            parts.next().is_none().then_some(Value::new(x, y))
        }
        Property::Rotate => number(text, "deg").map(Value::scalar),
        _ => exact_num::parse_f64(text).ok().map(Value::scalar),
    }
}

fn is_easing(s: &str) -> bool {
    matches!(
        s,
        "linear" | "ease" | "ease-in" | "ease-out" | "ease-in-out" | "step-start" | "step-end"
    ) || call(s).is_some()
}

/// A CSS `<custom-ident>` as Contract names are spelled.
fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with(|c: char| c.is_ascii_digit())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn time(s: &str) -> Result<f64, ParseError> {
    let (num, scale) = if let Some(n) = s.strip_suffix("ms") {
        (n, 0.001)
    } else if let Some(n) = s.strip_suffix('s') {
        (n, 1.0)
    } else {
        return Err(ParseError::BadTime(s.to_string()));
    };
    exact_num::parse_f64(num)
        .map(|n| n * scale)
        .map_err(|_| ParseError::BadTime(s.to_string()))
}

fn easing(s: &str) -> Result<TimingFunction, ParseError> {
    Ok(TimingFunction::Easing(match s {
        "linear" => Easing::Linear,
        "ease" => Easing::Ease,
        "ease-in" => Easing::EaseIn,
        "ease-out" => Easing::EaseOut,
        "ease-in-out" => Easing::EaseInOut,
        "step-start" => Easing::Steps {
            count: 1,
            position: StepPosition::JumpStart,
        },
        "step-end" => Easing::Steps {
            count: 1,
            position: StepPosition::JumpEnd,
        },
        _ => {
            let (name, args) = call(s).ok_or_else(|| ParseError::BadEasing(s.to_string()))?;
            match name {
                "cubic-bezier" => {
                    let n = numbers(&args, s)?;
                    if n.len() != 4 {
                        return Err(ParseError::BadEasing(s.to_string()));
                    }
                    Easing::CubicBezier {
                        x1: n[0],
                        y1: n[1],
                        x2: n[2],
                        y2: n[3],
                    }
                }
                "steps" => {
                    let count = args
                        .first()
                        .and_then(|a| a.trim().parse::<u16>().ok())
                        .ok_or_else(|| ParseError::BadEasing(s.to_string()))?;
                    let position = match args.get(1).map(|a| a.trim()) {
                        None | Some("jump-end") | Some("end") => StepPosition::JumpEnd,
                        Some("jump-start") | Some("start") => StepPosition::JumpStart,
                        Some("jump-none") => StepPosition::JumpNone,
                        Some("jump-both") => StepPosition::JumpBoth,
                        Some(_) => return Err(ParseError::BadEasing(s.to_string())),
                    };
                    Easing::Steps { count, position }
                }
                "linear" => Easing::PiecewiseLinear(linear_stops(&args, s)?),
                "spring" => {
                    let n = numbers(&args, s)?;
                    let config = match n.len() {
                        0 => SpringConfig::default(),
                        3 => SpringConfig {
                            stiffness: n[0],
                            damping: n[1],
                            mass: n[2],
                        },
                        _ => return Err(ParseError::BadEasing(s.to_string())),
                    };
                    return Ok(TimingFunction::Spring(config));
                }
                _ => return Err(ParseError::BadEasing(s.to_string())),
            }
        }
    }))
}

fn linear_stops(args: &[String], whole: &str) -> Result<Vec<LinearStop>, ParseError> {
    let bad = || ParseError::BadEasing(whole.to_string());
    let mut stops: Vec<(Option<f64>, f64)> = Vec::new();
    for arg in args {
        let fields: Vec<&str> = arg.split_whitespace().collect();
        if fields.is_empty() || fields.len() > 3 {
            return Err(bad());
        }
        let output = exact_num::parse_f64(fields[0]).map_err(|_| bad())?;
        if fields.len() == 1 {
            stops.push((None, output));
            continue;
        }
        for field in &fields[1..] {
            let input = field
                .strip_suffix('%')
                .and_then(|value| exact_num::parse_f64(value).ok())
                .map(|value| value / 100.0)
                .ok_or_else(&bad)?;
            stops.push((Some(input), output));
        }
    }
    if stops.is_empty() {
        return Err(bad());
    }

    // CSS Easing 2's fixup: default the endpoints, clamp authored positions
    // to the greatest preceding position, then evenly distribute each run of
    // omitted positions between its authored neighbours.
    if stops[0].0.is_none() {
        stops[0].0 = Some(0.0);
    }
    let last = stops.len() - 1;
    if stops[last].0.is_none() {
        stops[last].0 = Some(1.0);
    }
    let mut greatest = f64::NEG_INFINITY;
    for (input, _) in &mut stops {
        if let Some(value) = input {
            if *value < greatest {
                *value = greatest;
            }
            greatest = *value;
        }
    }
    let mut start = 0;
    while start + 1 < stops.len() {
        if stops[start + 1].0.is_some() {
            start += 1;
            continue;
        }
        let end = (start + 2..stops.len())
            .find(|i| stops[*i].0.is_some())
            .expect("the last linear stop has a position");
        let from = stops[start]
            .0
            .expect("the first linear stop has a position");
        let to = stops[end].0.unwrap();
        let width = (end - start) as f64;
        for (offset, stop) in stops[start + 1..end].iter_mut().enumerate() {
            stop.0 = Some(from + (to - from) * (offset + 1) as f64 / width);
        }
        start = end;
    }
    Ok(stops
        .into_iter()
        .map(|(input, output)| LinearStop {
            input: input.unwrap(),
            output,
        })
        .collect())
}

fn call(s: &str) -> Option<(&str, Vec<String>)> {
    let open = s.find('(')?;
    if !s.ends_with(')') {
        return None;
    }
    let name = &s[..open];
    let inner = &s[open + 1..s.len() - 1];
    let args = if inner.trim().is_empty() {
        Vec::new()
    } else {
        split_top_level(inner, ',')
            .into_iter()
            .map(|a| a.trim().to_string())
            .collect()
    };
    Some((name, args))
}

fn numbers(args: &[String], whole: &str) -> Result<Vec<f64>, ParseError> {
    args.iter()
        .map(|a| {
            exact_num::parse_f64(a.trim()).map_err(|_| ParseError::BadEasing(whole.to_string()))
        })
        .collect()
}

/// Split on `sep` outside parentheses.
fn split_top_level(s: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c == sep && depth == 0 => {
                out.push(&s[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_shorthand_parses_and_bad_forms_are_refused_by_name() {
        let t = Transitions::parse("opacity 250ms ease-in-out, all 0.5s cubic-bezier(0.4, 0, 0.2, 1) 100ms, translate spring(180, 12, 1)").unwrap();
        assert_eq!(t.0.len(), 3);
        assert_eq!(t.0[0].duration, 0.25);
        assert_eq!(t.0[0].timing, TimingFunction::Easing(Easing::EaseInOut));
        assert_eq!(t.0[1].delay, 0.1);
        assert!(
            matches!(t.0[1].timing, TimingFunction::Easing(Easing::CubicBezier { x1, .. }) if x1 == 0.4)
        );
        assert!(
            matches!(t.0[2].timing, TimingFunction::Spring(SpringConfig { stiffness, .. }) if stiffness == 180.0)
        );
        assert_eq!(t.0[2].duration, 0.0);
        // A bare duration is `all`; `ease` is the default easing.
        let t = Transitions::parse("0.3s").unwrap();
        assert_eq!(t.0[0].property, TransitionProperty::All);
        assert_eq!(t.0[0].timing, TimingFunction::Easing(Easing::Ease));
        let t = Transitions::parse("ease 1s").unwrap();
        assert_eq!(t.0[0].property, TransitionProperty::All);
        assert_eq!(t.0[0].duration, 1.0);
        assert_eq!(t.0[0].timing, TimingFunction::Easing(Easing::Ease));
        let t = Transitions::parse("linear 200ms").unwrap();
        assert_eq!(t.0[0].property, TransitionProperty::All);
        assert_eq!(t.0[0].duration, 0.2);
        let t = Transitions::parse("1s opacity").unwrap();
        assert_eq!(
            t.0[0].property,
            TransitionProperty::Property(Property::Opacity)
        );
        assert_eq!(t.0[0].duration, 1.0);
        assert_eq!(Transitions::parse("none").unwrap(), Transitions::NONE);
        assert_eq!(
            Transitions::parse("width 1s"),
            Err(ParseError::UnknownProperty("width".into()))
        );
        assert_eq!(
            Transitions::parse("opacity 1"),
            Err(ParseError::BadEasing("1".into())),
            "a unitless number is neither a time nor an easing"
        );
        assert_eq!(
            Transitions::parse("opacity 1s bounce"),
            Err(ParseError::BadEasing("bounce".into()))
        );
        assert!(matches!(
            Transitions::parse("opacity 1s ease linear"),
            Err(ParseError::BadShape(_))
        ));
        assert!(matches!(
            Transitions::parse("opacity scale 1s"),
            Err(ParseError::BadShape(_))
        ));
        assert!(matches!(
            Transitions::parse("opacity 1s spring(1,2,3)"),
            Err(ParseError::Invalid(TransitionError::SpringDeclaresDuration))
        ));
        assert!(matches!(
            Transitions::parse("opacity 1s steps(0)"),
            Err(ParseError::Invalid(_))
        ));
        let steps = Transitions::parse("opacity 1s steps(4, jump-both)").unwrap();
        assert!(matches!(
            steps.0[0].timing,
            TimingFunction::Easing(Easing::Steps {
                count: 4,
                position: StepPosition::JumpBoth
            })
        ));
        let lin = Transitions::parse("opacity 1s linear(0, 0.9 50%, 1)").unwrap();
        let TimingFunction::Easing(Easing::PiecewiseLinear(stops)) = &lin.0[0].timing else {
            panic!()
        };
        assert_eq!(stops.len(), 3);
        assert_eq!((stops[1].input, stops[1].output), (0.5, 0.9));

        let lin = Transitions::parse("opacity 1s linear(0, 0.2, 0.6 60%, 0.8, 1)").unwrap();
        let TimingFunction::Easing(Easing::PiecewiseLinear(stops)) = &lin.0[0].timing else {
            panic!()
        };
        assert_eq!(
            stops
                .iter()
                .map(|stop| (stop.input, stop.output))
                .collect::<Vec<_>>(),
            [(0.0, 0.0), (0.3, 0.2), (0.6, 0.6), (0.8, 0.8), (1.0, 1.0)]
        );
        let lin = Transitions::parse("opacity 1s linear(0 0% 20%, 1 80% 100%)").unwrap();
        let TimingFunction::Easing(Easing::PiecewiseLinear(stops)) = &lin.0[0].timing else {
            panic!()
        };
        assert_eq!(
            stops
                .iter()
                .map(|stop| (stop.input, stop.output))
                .collect::<Vec<_>>(),
            [(0.0, 0.0), (0.2, 0.0), (0.8, 1.0), (1.0, 1.0)]
        );
    }
}
