//! The viewport a served page is laid out for (LLP 1048.006).
//!
//! A page whose plan reads `exactViewport()` is rendered at the reader's
//! viewport: the browser's client hints when it sends them
//! (`Sec-CH-Viewport-Width` and `-Height`, the `Sec-CH-Prefers-*` facts),
//! else the class its user agent names — a phone's `pageViewport`, a
//! desktop's [`DESKTOP`]. A plan that reads no viewport renders as before,
//! and its pages say nothing of hints.
//!
//! A kept page is kept per **class**: two viewports are one class when
//! every comparison the plan makes on each fact it reads comes out the same
//! for both (`viewport.width <= 700` makes two classes), so the page is the
//! same page. A fact read any other way (a width in arithmetic, a record
//! passed whole) is keyed by its value.

use exact_plan::{Opcode, Plan, ResourcesId, StrId, TypeKind, Value};
use exact_runner::vm::instructions;
use exact_runner::Viewport;
use std::collections::BTreeSet;

/// The layout viewport of a desktop browser that sends no width: a 1280-wide
/// window, the most common desktop width class's lower edge.
pub(crate) const DESKTOP: (f64, f64) = (1280.0, 800.0);

/// The facts `exactViewport` fills, as the runner names them, with the
/// client hint that carries each (none: the browser has no hint for it).
const FACTS: [(&str, Option<&str>); 6] = [
    ("width", Some("Sec-CH-Viewport-Width")),
    ("height", Some("Sec-CH-Viewport-Height")),
    (
        "prefersReducedMotion",
        Some("Sec-CH-Prefers-Reduced-Motion"),
    ),
    (
        "prefersReducedTransparency",
        Some("Sec-CH-Prefers-Reduced-Transparency"),
    ),
    ("prefersContrast", None),
    ("prefersColorScheme", Some("Sec-CH-Prefers-Color-Scheme")),
];

/// How the plan reads one fact.
#[derive(Debug, Clone, PartialEq)]
enum Read {
    /// Only compared with constants: these comparisons, `fact <op> value`.
    Compared(Vec<(Opcode, Constant)>),
    /// Some other way: the page depends on its value.
    Whole,
}

/// A constant a fact is compared with.
#[derive(Debug, Clone, PartialEq)]
enum Constant {
    Number(f64),
    Str(String),
    Bool(bool),
}

/// What of the viewport a plan reads, fact by fact (in [`FACTS`]' order).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Reads {
    facts: [Option<Read>; 6],
}

impl Reads {
    /// Walk every code body: each `exactViewport` resource's field read is a
    /// comparison with a constant, or not.
    pub(crate) fn of(plan: &Plan) -> Reads {
        let mut reads = Reads::default();
        // Each viewport resource's fields: field index -> fact index.
        let fields: Vec<(ResourcesId, Vec<Option<usize>>)> = plan
            .resources
            .iter()
            .enumerate()
            .filter(|(_, row)| plan.str(row.source) == exact_runner::viewport::SOURCE)
            .map(|(i, row)| {
                let ty = plan.type_(row.ty);
                let names = if ty.kind == TypeKind::Record {
                    ty.fields
                        .iter()
                        .map(|f| {
                            let name = plan.str(plan.field(f).name);
                            FACTS.iter().position(|(fact, _)| *fact == name)
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                (ResourcesId(i as u32), names)
            })
            .collect();
        if fields.is_empty() {
            return reads;
        }
        plan.each_code(&mut |code| {
            let Ok(code) = instructions(plan.code(code)).collect::<Result<Vec<_>, _>>() else {
                // A body the VM would refuse reads nothing; the runner never runs it.
                return;
            };
            // Where a jump can land: a value there may come from elsewhere.
            let targets: BTreeSet<usize> = code
                .iter()
                .filter(|i| {
                    matches!(
                        i.op,
                        Opcode::Jump
                            | Opcode::JumpIfFalse
                            | Opcode::JumpIfNone
                            | Opcode::Map
                            | Opcode::Filter
                    )
                })
                .map(|i| i.args[0] as usize)
                .collect();
            for (at, i) in code.iter().enumerate() {
                if i.op != Opcode::LoadResource {
                    continue;
                }
                let Some((_, names)) = fields.iter().find(|(r, _)| r.0 as u64 == i.args[0]) else {
                    continue;
                };
                // The record itself, not one field: every fact it holds.
                let Some(field) = code.get(at + 1).filter(|n| n.op == Opcode::Field) else {
                    for fact in names.iter().flatten() {
                        reads.facts[*fact] = Some(Read::Whole);
                    }
                    continue;
                };
                let Some(fact) = names.get(field.args[0] as usize).copied().flatten() else {
                    continue;
                };
                let constant = |n: &exact_runner::vm::Instruction| match n.op {
                    Opcode::Number => Some(Constant::Number(n.number)),
                    Opcode::Str => Some(Constant::Str(plan.str(StrId(n.args[0] as u32)).into())),
                    Opcode::Bool => Some(Constant::Bool(n.args[0] != 0)),
                    _ => None,
                };
                let landed = |n: &exact_runner::vm::Instruction| targets.contains(&n.pc);
                // `fact <op> constant`, or `constant <op> fact`.
                let after = code.get(at + 2).zip(code.get(at + 3)).and_then(|(c, op)| {
                    let value = constant(c).filter(|_| !landed(c) && !landed(op))?;
                    comparison(op.op).map(|op| (op, value))
                });
                let before = at
                    .checked_sub(1)
                    .and_then(|b| code.get(b))
                    .zip(code.get(at + 2))
                    .and_then(|(c, op)| {
                        let value = constant(c).filter(|_| !landed(i) && !landed(op))?;
                        comparison(op.op).map(|op| (mirror(op), value))
                    });
                match (after.or(before), &mut reads.facts[fact]) {
                    (Some(_), Some(Read::Whole)) => {}
                    (Some(compared), Some(Read::Compared(all))) => {
                        if !all.contains(&compared) {
                            all.push(compared);
                        }
                    }
                    (Some(compared), slot) => *slot = Some(Read::Compared(vec![compared])),
                    (None, slot) => *slot = Some(Read::Whole),
                }
            }
        });
        reads
    }

    /// Whether the plan reads any viewport fact.
    pub(crate) fn any(&self) -> bool {
        self.facts.iter().any(Option::is_some)
    }

    /// Whether it reads the size, which a browser without hints is guessed at.
    fn size(&self) -> bool {
        self.facts[0].is_some() || self.facts[1].is_some()
    }

    /// The header lines a page adds: `Accept-CH` naming the hints for the
    /// facts it reads, and `Vary` naming what its viewport was chosen by.
    /// Empty for a plan that reads none.
    pub(crate) fn headers(&self) -> String {
        let hints: Vec<&str> = FACTS
            .iter()
            .zip(&self.facts)
            .filter(|(_, read)| read.is_some())
            .filter_map(|((_, hint), _)| *hint)
            .collect();
        let mut vary = hints.clone();
        if self.size() {
            vary.extend(["Sec-CH-UA-Mobile", "User-Agent"]);
        }
        let mut out = String::new();
        if !hints.is_empty() {
            out.push_str(&format!("Accept-CH: {}\r\n", hints.join(", ")));
        }
        if !vary.is_empty() {
            out.push_str(&format!("Vary: {}\r\n", vary.join(", ")));
        }
        out
    }

    /// The viewport a request with `hints` is rendered at: `page` (the
    /// manifest's `pageViewport`) for a plan that reads none.
    pub(crate) fn viewport(&self, hints: &Hints, page: Viewport) -> Viewport {
        if self.any() {
            hints.viewport(page)
        } else {
            page
        }
    }

    /// The class a kept page of a request with `hints` is kept under beside
    /// its target: empty for a plan that reads no viewport.
    pub(crate) fn class(&self, hints: &Hints, page: Viewport) -> String {
        self.key(&self.viewport(hints, page))
    }

    /// Each fact the plan reads in `viewport`, as the plan can tell it apart.
    fn key(&self, viewport: &Viewport) -> String {
        let mut key = String::new();
        for ((name, _), read) in FACTS.iter().zip(&self.facts) {
            let Some(read) = read else { continue };
            let value = viewport.field(name).unwrap_or(Value::Unit);
            key.push_str(name);
            key.push('=');
            match read {
                Read::Whole => key.push_str(&format!("{value:?}")),
                Read::Compared(all) => {
                    for (op, constant) in all {
                        key.push(if holds(*op, &value, constant) {
                            '1'
                        } else {
                            '0'
                        });
                    }
                }
            }
            key.push(';');
        }
        key
    }
}

/// A comparison opcode, if `op` is one.
fn comparison(op: Opcode) -> Option<Opcode> {
    matches!(
        op,
        Opcode::Lt | Opcode::Le | Opcode::Gt | Opcode::Ge | Opcode::Eq | Opcode::Ne
    )
    .then_some(op)
}

/// `a <op> b` as `b <mirror(op)> a`.
fn mirror(op: Opcode) -> Opcode {
    match op {
        Opcode::Lt => Opcode::Gt,
        Opcode::Le => Opcode::Ge,
        Opcode::Gt => Opcode::Lt,
        Opcode::Ge => Opcode::Le,
        other => other,
    }
}

/// Whether `value <op> constant` holds, as the VM compares.
fn holds(op: Opcode, value: &Value, constant: &Constant) -> bool {
    let order = match (value, constant) {
        (Value::Number(a), Constant::Number(b)) => a.partial_cmp(b),
        (Value::Bool(a), Constant::Bool(b)) => Some(a.cmp(b)),
        (_, Constant::Str(b)) => value.as_str().map(|a| a.cmp(b.as_str())),
        _ => None,
    };
    let Some(order) = order else {
        return op == Opcode::Ne;
    };
    match op {
        Opcode::Lt => order.is_lt(),
        Opcode::Le => order.is_le(),
        Opcode::Gt => order.is_gt(),
        Opcode::Ge => order.is_ge(),
        Opcode::Eq => order.is_eq(),
        _ => order.is_ne(),
    }
}

/// What a request says of its reader's viewport: its client hints, and
/// whether its user agent is a phone's.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Hints {
    width: Option<f64>,
    height: Option<f64>,
    /// A phone (`Sec-CH-UA-Mobile: ?1`, or a user agent that names one), a
    /// desktop (`?0`, or a browser's user agent that names none), or `None`
    /// when neither tells (a client that is not a browser).
    mobile: Option<bool>,
    dark: Option<bool>,
    reduced_motion: Option<bool>,
    reduced_transparency: Option<bool>,
}

impl Hints {
    /// Read from a request's header lines (`name`, `value`).
    pub(crate) fn parse(headers: &[(&str, &str)]) -> Hints {
        let get = |name: &str| {
            headers
                .iter()
                .find(|(n, _)| n.trim().eq_ignore_ascii_case(name))
                .map(|(_, v)| v.trim().trim_matches('"'))
        };
        // A layout viewport a browser could have, in CSS pixels.
        let size = |v: &str| {
            v.parse::<f64>()
                .ok()
                .filter(|n| (100.0..=10_000.0).contains(n))
        };
        // A phone by either sign wins: an emulated phone in a desktop
        // browser may still say `?0`. A desktop needs one sign of its own.
        let ua = get("user-agent").filter(|ua| ua.starts_with("Mozilla/"));
        let phone_ua = ua.map(|ua| {
            ["Mobi", "Android", "iPhone", "iPod"]
                .iter()
                .any(|word| ua.contains(word))
        });
        let mobile = match (get("sec-ch-ua-mobile"), phone_ua) {
            (Some("?1"), _) | (_, Some(true)) => Some(true),
            (Some("?0"), _) | (_, Some(false)) => Some(false),
            _ => None,
        };
        Hints {
            width: get("sec-ch-viewport-width")
                .or_else(|| get("viewport-width"))
                .and_then(size),
            height: get("sec-ch-viewport-height").and_then(size),
            mobile,
            dark: get("sec-ch-prefers-color-scheme").map(|v| v == "dark"),
            reduced_motion: get("sec-ch-prefers-reduced-motion").map(|v| v == "reduce"),
            reduced_transparency: get("sec-ch-prefers-reduced-transparency").map(|v| v == "reduce"),
        }
    }

    /// The viewport to render at: the hints, else the class the user agent
    /// names — `page` (the manifest's `pageViewport`) for a phone or a
    /// client that says nothing, [`DESKTOP`] for a desktop browser.
    pub(crate) fn viewport(&self, page: Viewport) -> Viewport {
        let desktop = self.mobile == Some(false);
        let (width, height) = if desktop {
            DESKTOP
        } else {
            (page.width, page.height)
        };
        let mut viewport = Viewport {
            width: self.width.unwrap_or(width),
            height: self.height.unwrap_or(height),
            preferences: page.preferences,
            fold: page.fold,
        };
        let p = &mut viewport.preferences;
        p.dark = self.dark.unwrap_or(p.dark);
        p.reduced_motion = self.reduced_motion.unwrap_or(p.reduced_motion);
        p.reduced_transparency = self.reduced_transparency.unwrap_or(p.reduced_transparency);
        viewport
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reads(src: &str) -> Reads {
        Reads::of(&contract::compile(src).unwrap())
    }

    const GRID: &str = "shape Viewport\n  width: number\ncomponent A\n  resource viewport = exactViewport() as shape Viewport\n  derive narrow = viewport.width <= 700\n  view\n    column flex-direction=(narrow ? \"column\" : \"row\")\n      text \"a\"\n";

    #[test]
    fn a_width_compared_with_constants_is_one_class_per_side() {
        let r = reads(GRID);
        assert!(r.any());
        let at = |w| r.key(&Viewport::sized(w, 900.0));
        assert_eq!(at(390.0), at(700.0));
        assert_eq!(at(1280.0), at(701.0));
        assert_ne!(at(700.0), at(701.0));
        assert_eq!(
            r.headers(),
            "Accept-CH: Sec-CH-Viewport-Width\r\nVary: Sec-CH-Viewport-Width, Sec-CH-UA-Mobile, User-Agent\r\n"
        );
        // Constant first reads the same.
        let flipped = reads(&GRID.replace("viewport.width <= 700", "700 >= viewport.width"));
        assert_eq!(flipped.key(&Viewport::sized(700.0, 1.0)), at(700.0));
        assert_ne!(flipped.key(&Viewport::sized(701.0, 1.0)), at(700.0));
    }

    #[test]
    fn a_width_used_otherwise_is_keyed_by_its_value_and_no_read_is_no_key() {
        let r = reads(&GRID.replace("viewport.width <= 700", "viewport.width / 2 <= 350"));
        assert_ne!(
            r.key(&Viewport::sized(390.0, 1.0)),
            r.key(&Viewport::sized(391.0, 1.0))
        );
        let none = reads("component A\n  view\n    text \"a\"\n");
        assert!(!none.any());
        assert_eq!(none.headers(), "");
        assert_eq!(none.key(&Viewport::sized(390.0, 1.0)), "");
    }

    #[test]
    fn hints_else_the_user_agent_choose_the_viewport() {
        let page = Viewport::default();
        let at = |headers: &[(&str, &str)]| Hints::parse(headers).viewport(page);
        let hinted = at(&[
            ("Sec-CH-Viewport-Width", "1440"),
            ("Sec-CH-Viewport-Height", "900"),
            ("Sec-CH-Prefers-Color-Scheme", "\"dark\""),
        ]);
        assert_eq!((hinted.width, hinted.height), (1440.0, 900.0));
        assert!(hinted.preferences.dark);
        let desktop = at(&[("Sec-CH-UA-Mobile", "?0")]);
        assert_eq!((desktop.width, desktop.height), DESKTOP);
        let safari = at(&[(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Version/18.0 Safari/605.1.15",
        )]);
        assert_eq!(safari.width, DESKTOP.0);
        let phone = at(&[(
            "User-Agent",
            "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 Mobile/15E148",
        )]);
        assert_eq!((phone.width, phone.height), (page.width, page.height));
        // An emulated phone that still says `?0` is a phone.
        let emulated = at(&[
            ("Sec-CH-UA-Mobile", "?0"),
            (
                "User-Agent",
                "Mozilla/5.0 (Linux; Android 11; moto g power (2022)) Mobile Safari/537.36",
            ),
        ]);
        assert_eq!(emulated.width, page.width);
        // A client that says nothing, or nonsense, gets the page viewport.
        assert_eq!(at(&[("Sec-CH-Viewport-Width", "abc")]), page);
        assert_eq!(at(&[]), page);
    }
}
