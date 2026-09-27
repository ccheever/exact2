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

use crate::easing::{Easing, LinearStop, StepPosition};

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
    /// A keyframe value outside its property's grammar (LLP 1055 D6).
    BadValue(String),
    /// Parsed, but the animation sampler refuses it (LLP 1055 D5).
    InvalidAnimation(crate::animation::AnimationError),
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
                } else if let Some(named) = TransitionProperty::from_name(part) {
                    if property.is_some() {
                        return Err(ParseError::BadShape(decl.trim().to_string()));
                    }
                    property = Some(named);
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

pub(crate) fn time(s: &str) -> Result<f64, ParseError> {
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

pub(crate) fn easing(s: &str) -> Result<TimingFunction, ParseError> {
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
pub(crate) fn split_top_level(s: &str, sep: char) -> Vec<&str> {
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
    use crate::property::Property;

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
