//! CSS `transition` shorthand → [`Transitions`].
//!
//! @ref LLP 1002 §2 (the web is the standard: the authored form is CSS's own
//! `transition` shorthand; `spring(stiffness, damping, mass)` is the one
//! declared extension)
//!
//! `transition: <property> <duration> [<easing>] [<delay>], …` — each part
//! in CSS's own grammar, with `spring(k, d, m)` admitted where an easing
//! goes. What CSS's parser would reject, this rejects, by name.

use crate::easing::{Easing, LinearStop, StepPosition};
use crate::property::Property;
use crate::spring::SpringConfig;
use crate::transition::{
    TimingFunction, Transition, TransitionError, TransitionProperty, Transitions,
};

/// Why a shorthand was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// An empty declaration.
    Empty,
    /// A property name outside `all`, `translate`, `scale`, `rotate`, `opacity`.
    UnknownProperty(String),
    /// A time without `s`/`ms`, or not a number.
    BadTime(String),
    /// An easing name or function this parser does not know.
    BadEasing(String),
    /// A declaration with too many or too few parts.
    BadShape(String),
    /// Parsed, but the evaluator refuses it.
    Invalid(TransitionError),
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
            let mut i = 0;
            let property = match parts[0] {
                "all" => {
                    i += 1;
                    TransitionProperty::All
                }
                p if Property::from_name(p).is_some() => {
                    i += 1;
                    TransitionProperty::Property(Property::from_name(p).unwrap())
                }
                p if time(p).is_ok() => TransitionProperty::All,
                p => return Err(ParseError::UnknownProperty(p.to_string())),
            };
            let mut duration = 0.0;
            let mut delay = 0.0;
            let mut timing = TimingFunction::Easing(Easing::Ease);
            let mut times = 0;
            for part in &parts[i..] {
                if let Ok(t) = time(part) {
                    match times {
                        0 => duration = t,
                        1 => delay = t,
                        _ => return Err(ParseError::BadShape(decl.trim().to_string())),
                    }
                    times += 1;
                } else {
                    timing = easing(part)?;
                }
            }
            let t = Transition {
                property,
                duration,
                delay,
                timing,
            };
            t.validate().map_err(ParseError::Invalid)?;
            out.push(t);
        }
        let ts = Transitions(out);
        ts.validate().map_err(ParseError::Invalid)?;
        Ok(ts)
    }
}

fn time(s: &str) -> Result<f64, ParseError> {
    let (num, scale) = if let Some(n) = s.strip_suffix("ms") {
        (n, 0.001)
    } else if let Some(n) = s.strip_suffix('s') {
        (n, 1.0)
    } else {
        return Err(ParseError::BadTime(s.to_string()));
    };
    num.parse::<f64>()
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
                "linear" => {
                    let mut stops = Vec::new();
                    for (i, a) in args.iter().enumerate() {
                        let fields: Vec<&str> = a.split_whitespace().collect();
                        let output: f64 = fields
                            .first()
                            .and_then(|f| f.parse().ok())
                            .ok_or_else(|| ParseError::BadEasing(s.to_string()))?;
                        let input = match fields.get(1) {
                            Some(p) => p
                                .strip_suffix('%')
                                .and_then(|v| v.parse::<f64>().ok())
                                .map(|v| v / 100.0)
                                .ok_or_else(|| ParseError::BadEasing(s.to_string()))?,
                            None => {
                                if args.len() == 1 {
                                    0.0
                                } else {
                                    i as f64 / (args.len() as f64 - 1.0)
                                }
                            }
                        };
                        stops.push(LinearStop { input, output });
                    }
                    Easing::PiecewiseLinear(stops)
                }
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
            a.trim()
                .parse::<f64>()
                .map_err(|_| ParseError::BadEasing(whole.to_string()))
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
    }
}
