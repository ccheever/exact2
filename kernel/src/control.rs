//! Form controls by their HTML names (LLP 1069.001): which widget a
//! `Control` node is, the size a host shows before it reports its own, and a
//! `select`'s options as every host reads them.
//!
//! The rules here are HTML's, so the runner can hold every host to them: a
//! select's value is one of its enabled options' values.

use crate::generated::{NodeType, PropId};
use crate::id::ViewId;
use crate::kernel::{Kernel, NodeRef};
use crate::props::PropList;

/// Which platform control a `Control` node is, by its `type` prop (and a
/// checkbox's role).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    /// `input type="checkbox"`.
    Checkbox,
    /// `input type="checkbox" switch`.
    Switch,
    /// `input type="file"` (LLP 1069.002).
    File,
    /// `select`, its options the node's children.
    Select,
    /// `input type="range"`.
    Range,
    /// `input type="date"`: `yyyy-mm-dd`.
    Date,
    /// `input type="time"`: `hh:mm`, or with seconds.
    Time,
    /// `input type="datetime-local"`: a date, `T`, a time; no zone.
    DateTimeLocal,
}

impl ControlKind {
    /// The kind of a node, or `None` when it is not a `Control`.
    pub fn of(node_type: NodeType, props: &PropList) -> Option<ControlKind> {
        if node_type != NodeType::Control {
            return None;
        }
        Some(match props.str(PropId::Type) {
            Some("file") => ControlKind::File,
            Some("select") => ControlKind::Select,
            Some("range") => ControlKind::Range,
            Some("date") => ControlKind::Date,
            Some("time") => ControlKind::Time,
            Some("datetime-local") => ControlKind::DateTimeLocal,
            _ if props.str(PropId::AccessibilityRole) == Some("switch") => ControlKind::Switch,
            _ => ControlKind::Checkbox,
        })
    }

    /// The content size a host that reports none shows (LLP 1069.001 D3):
    /// Chrome's 13×13 checkbox, Safari's 38×22 desktop switch, and a select
    /// one line of Chrome's 13.33 px control font tall, Chrome's 129×16
    /// range, and the date types one line of it wide enough for their
    /// value. A host that knows its control's size reports it.
    pub fn default_size(self) -> (f32, f32) {
        match self {
            ControlKind::Checkbox | ControlKind::File => (13.0, 13.0),
            ControlKind::Switch => (38.0, 22.0),
            ControlKind::Select => (64.0, 19.0),
            ControlKind::Range => (129.0, 16.0),
            ControlKind::Date => (96.0, 19.0),
            ControlKind::Time => (64.0, 19.0),
            ControlKind::DateTimeLocal => (160.0, 19.0),
        }
    }

    /// Whether `value` is one a date control's kind takes, in HTML's value
    /// format (a valid date, time or local date and time string): the
    /// empty string, a cleared control's, included. Other kinds take any.
    pub fn valid_value(self, value: &str) -> bool {
        if value.is_empty() {
            return true;
        }
        match self {
            ControlKind::Date => valid_date(value),
            ControlKind::Time => valid_time(value),
            ControlKind::DateTimeLocal => value
                .split_once('T')
                .is_some_and(|(d, t)| valid_date(d) && valid_time(t)),
            _ => true,
        }
    }
}

fn digits(s: &str, n: usize) -> Option<u32> {
    (s.len() == n && s.bytes().all(|b| b.is_ascii_digit()))
        .then(|| s.parse().ok())
        .flatten()
}

/// `yyyy-mm-dd`, the year four or more digits and never 0, the day one the
/// month has (February's 29th in a leap year).
fn valid_date(s: &str) -> bool {
    let mut parts = s.rsplitn(3, '-');
    let (Some(d), Some(m), Some(y)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let (Some(d), Some(m)) = (digits(d, 2), digits(m, 2)) else {
        return false;
    };
    let Some(y) = (y.len() >= 4)
        .then(|| digits(y, y.len()))
        .flatten()
        .filter(|y| *y > 0)
    else {
        return false;
    };
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&d)
}

/// `hh:mm`, `hh:mm:ss` or `hh:mm:ss.sss` (one to three fraction digits).
fn valid_time(s: &str) -> bool {
    let mut parts = s.splitn(3, ':');
    let (Some(h), Some(m)) = (parts.next(), parts.next()) else {
        return false;
    };
    let hm = digits(h, 2).is_some_and(|h| h < 24) && digits(m, 2).is_some_and(|m| m < 60);
    match parts.next() {
        None => hm,
        Some(sec) => {
            let (whole, fraction) = sec.split_once('.').unwrap_or((sec, ""));
            hm && digits(whole, 2).is_some_and(|s| s < 60)
                && (fraction.is_empty() && !sec.ends_with('.')
                    || (1..=3).contains(&fraction.len())
                        && fraction.bytes().all(|b| b.is_ascii_digit()))
        }
    }
}

/// A range's `min`, `max` and `step` by HTML's rules (the value
/// sanitization algorithm of `input type=range`): `min` 0 and `max` 100 by
/// default, a `max` below `min` is `min`, `step` 1 by default and none for
/// `any`, an unusable one the default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    /// The least value.
    pub min: f64,
    /// The greatest value.
    pub max: f64,
    /// The step from `min`; `None` for `any`.
    pub step: Option<f64>,
}

fn number(s: Option<&str>) -> Option<f64> {
    s.and_then(|s| s.trim().parse::<f64>().ok())
        .filter(|n| n.is_finite())
}

impl Range {
    /// A range's bounds from its props.
    pub fn of(props: &PropList) -> Range {
        let min = number(props.str(PropId::Min)).unwrap_or(0.0);
        let max = number(props.str(PropId::Max)).unwrap_or(100.0).max(min);
        let step = match props.str(PropId::Step) {
            Some(s) if s.trim().eq_ignore_ascii_case("any") => None,
            s => Some(number(s).filter(|n| *n > 0.0).unwrap_or(1.0)),
        };
        Range { min, max, step }
    }

    /// `value` clamped and snapped as HTML sanitizes it: to the nearest
    /// step from `min` (half up), within `min` and `max`.
    pub fn sanitize(&self, value: f64) -> f64 {
        let clamped = value.clamp(self.min, self.max);
        let Some(step) = self.step else {
            return clamped;
        };
        let mut snapped = self.min + ((clamped - self.min) / step + 0.5).floor() * step;
        if snapped > self.max {
            snapped -= step;
        }
        // Tidy the float: a step of 0.01 lands on 0.85, not 0.8500000000000001.
        let tidy = (snapped * 1e9).round() / 1e9;
        tidy.clamp(self.min, self.max)
    }

    /// The value a range shows: its `value` sanitized, or with none (or an
    /// unreadable one) the midpoint, as HTML's default is.
    pub fn shown(&self, props: &PropList) -> f64 {
        let value =
            number(props.str(PropId::Value)).unwrap_or(self.min + (self.max - self.min) / 2.0);
        self.sanitize(value)
    }
}

/// One `option` of a `select`, as a menu shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// The option's node.
    pub view: ViewId,
    /// Its `value`, or its label when it has none (HTML).
    pub value: String,
    /// Its text, white space stripped and collapsed (HTML's `option.text`).
    pub label: String,
    /// Whether it may not be chosen.
    pub disabled: bool,
}

/// Whether `node` is an `option`: a text node the `option` tag made.
pub fn is_option(node: &NodeRef<'_>) -> bool {
    node.node_type == NodeType::Text && node.props.str(PropId::SemanticTag) == Some("option")
}

impl Kernel {
    /// A select's options in order, from its children (LLP 1069.001 D2);
    /// empty for any other node.
    pub fn select_choices(&self, view: ViewId) -> Vec<Choice> {
        let Some(select) = self.node(view) else {
            return Vec::new();
        };
        if ControlKind::of(select.node_type, select.props) != Some(ControlKind::Select) {
            return Vec::new();
        }
        let disabled = select.props.bool(PropId::Disabled) == Some(true);
        select
            .children()
            .into_iter()
            .filter_map(|id| self.node(id))
            .filter(is_option)
            .map(|option| {
                let text: String = option.text_runs().iter().map(|r| &*r.text).collect();
                let label = text.split_whitespace().collect::<Vec<_>>().join(" ");
                Choice {
                    view: option.id,
                    value: option
                        .props
                        .str(PropId::Value)
                        .map_or_else(|| label.clone(), str::to_owned),
                    label,
                    disabled: disabled || option.props.bool(PropId::Disabled) == Some(true),
                }
            })
            .collect()
    }

    /// The option a select shows: the one whose value its `value` names;
    /// with no `value`, the first enabled option, as an uncontrolled HTML
    /// select starts. `None` when its value names no option.
    pub fn select_chosen(&self, view: ViewId) -> Option<Choice> {
        let choices = self.select_choices(view);
        match self.node(view)?.props.str(PropId::Value) {
            Some(value) => choices.into_iter().find(|c| c.value == value),
            None => choices.into_iter().find(|c| !c.disabled),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::Op;
    use crate::PropValue;

    fn select() -> Kernel {
        let mut k = Kernel::with_monospace();
        let mut ops = vec![
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 2,
                node_type: NodeType::Control,
            },
            Op::SetProp {
                id: 2,
                prop: PropId::Type,
                value: PropValue::Str("select".into()),
            },
        ];
        for (id, value, text) in [(3, Some("s"), "  System\n default "), (4, None, "Dark")] {
            ops.push(Op::CreateView {
                id,
                node_type: NodeType::Text,
            });
            ops.push(Op::SetProp {
                id,
                prop: PropId::SemanticTag,
                value: PropValue::Str("option".into()),
            });
            ops.push(Op::SetProp {
                id,
                prop: PropId::Text,
                value: PropValue::Str(text.into()),
            });
            if let Some(v) = value {
                ops.push(Op::SetProp {
                    id,
                    prop: PropId::Value,
                    value: PropValue::Str(v.into()),
                });
            }
        }
        ops.push(Op::SetProp {
            id: 4,
            prop: PropId::Disabled,
            value: PropValue::Bool(true),
        });
        ops.push(Op::SetChildren {
            id: 2,
            children: vec![3, 4],
        });
        ops.push(Op::SetChildren {
            id: 1,
            children: vec![2],
        });
        ops.push(Op::AttachRoot { id: 1 });
        k.apply(0, 1, &ops).unwrap();
        k
    }

    #[test]
    fn date_values_are_htmls_formats() {
        use ControlKind::*;
        for (kind, ok) in [
            (Date, "2026-09-27"),
            (Date, "2024-02-29"),
            (Date, ""),
            (Time, "14:30"),
            (Time, "14:30:15"),
            (Time, "14:30:15.5"),
            (DateTimeLocal, "2026-09-27T14:30"),
        ] {
            assert!(kind.valid_value(ok), "{kind:?} {ok}");
        }
        for (kind, bad) in [
            (Date, "2026-13-01"),
            (Date, "2025-02-29"),
            (Date, "26-09-27"),
            (Date, "0000-01-01"),
            (Time, "24:00"),
            (Time, "9:30"),
            (Time, "14:30:15."),
            (DateTimeLocal, "2026-09-27 14:30"),
            (DateTimeLocal, "2026-09-27"),
        ] {
            assert!(!kind.valid_value(bad), "{kind:?} {bad}");
        }
    }

    #[test]
    fn a_range_clamps_and_snaps_as_html_does() {
        let mut props = PropList::default();
        let r = Range::of(&props);
        assert_eq!((r.min, r.max, r.step), (0.0, 100.0, Some(1.0)));
        assert_eq!(r.shown(&props), 50.0);
        assert_eq!(r.sanitize(40.5), 41.0);
        assert_eq!(r.sanitize(140.0), 100.0);
        props.set(PropId::Min, PropValue::Str("0.85".into()));
        props.set(PropId::Max, PropValue::Str("1.25".into()));
        props.set(PropId::Step, PropValue::Str("0.01".into()));
        let r = Range::of(&props);
        assert_eq!(r.sanitize(1.123), 1.12);
        assert_eq!(r.sanitize(0.1), 0.85);
        props.set(PropId::Step, PropValue::Str("any".into()));
        assert_eq!(Range::of(&props).sanitize(1.123), 1.123);
        props.set(PropId::Step, PropValue::Str("0.3".into()));
        props.set(PropId::Min, PropValue::Str("0".into()));
        props.set(PropId::Max, PropValue::Str("1".into()));
        assert_eq!(
            Range::of(&props).sanitize(1.0),
            0.9,
            "a step past max steps back"
        );
    }

    #[test]
    fn a_selects_options_are_its_children_by_htmls_rules() {
        let k = select();
        let choices = k.select_choices(2);
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].value, "s");
        assert_eq!(choices[0].label, "System default");
        assert_eq!(choices[1].value, "Dark", "no value: the label");
        assert!(choices[1].disabled);
        assert_eq!(k.select_chosen(2).map(|c| c.value), Some("s".into()));
        assert!(k.select_choices(1).is_empty());
        assert_eq!(
            ControlKind::of(NodeType::Control, k.node(2).unwrap().props),
            Some(ControlKind::Select)
        );
    }
}
