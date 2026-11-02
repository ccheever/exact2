//! What of the viewport a plan reads (LLP 1048.006 D4), and the media
//! queries that tell a browser's viewport apart as the plan does (LLP
//! 1048.007 D2).
//!
//! Every code body is walked for `exactViewport` field reads. A field read
//! only to be compared with a constant (`viewport.width >= 640`) is
//! recorded as those comparisons; one read any other way (arithmetic, a
//! record passed whole) as its whole value. Two viewports are then one
//! class when every recorded reading gives the same answer for both, so a
//! page rendered for one is the page for the other.

use crate::viewport::{Viewport, FIELDS, SOURCE};
use crate::vm::{instructions, Instruction};
use exact_plan::{Opcode, Plan, ResourcesId, StrId, TypeKind, Value};
use std::collections::BTreeSet;

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
pub enum Constant {
    /// A number (a size).
    Number(f64),
    /// A keyword (`"dark"`, `"coarse"`).
    Str(String),
    /// A boolean preference.
    Bool(bool),
}

/// What of the viewport a plan reads, fact by fact (in [`FIELDS`]' order).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reads {
    facts: Vec<Option<Read>>,
}

/// One comparison the plan makes, as a media query that matches exactly
/// when the comparison holds in the browser's viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct Media {
    /// The query, for `matchMedia`.
    pub query: String,
    /// The fact compared (one of [`FIELDS`]).
    pub fact: &'static str,
    /// The comparison, the fact on its left.
    pub op: Opcode,
    /// What the fact is compared with.
    pub constant: Constant,
}

impl Reads {
    /// Walk every code body: each `exactViewport` resource's field read is a
    /// comparison with a constant, or not.
    pub fn of(plan: &Plan) -> Reads {
        let mut reads = Reads {
            facts: vec![None; FIELDS.len()],
        };
        // Each viewport resource's fields: field index -> fact index.
        let fields: Vec<(ResourcesId, Vec<Option<usize>>)> = plan
            .resources
            .iter()
            .enumerate()
            .filter(|(_, row)| plan.str(row.source) == SOURCE)
            .map(|(i, row)| {
                let ty = plan.type_(row.ty);
                let names = if ty.kind == TypeKind::Record {
                    ty.fields
                        .iter()
                        .map(|f| {
                            let name = plan.str(plan.field(f).name);
                            FIELDS.iter().position(|fact| *fact == name)
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
                let constant = |n: &Instruction| match n.op {
                    Opcode::Number => Some(Constant::Number(n.number)),
                    Opcode::Str => Some(Constant::Str(plan.str(StrId(n.args[0] as u32)).into())),
                    Opcode::Bool => Some(Constant::Bool(n.args[0] != 0)),
                    _ => None,
                };
                let landed = |n: &Instruction| targets.contains(&n.pc);
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
    pub fn any(&self) -> bool {
        self.facts.iter().any(Option::is_some)
    }

    /// Whether the plan reads `fact` (one of [`FIELDS`]).
    pub fn reads(&self, fact: &str) -> bool {
        FIELDS
            .iter()
            .position(|f| *f == fact)
            .is_some_and(|at| self.facts[at].is_some())
    }

    /// Each fact the plan reads in `viewport`, as the plan can tell it
    /// apart: equal for two viewports of one class. Empty for a plan that
    /// reads no viewport.
    pub fn key(&self, viewport: &Viewport) -> String {
        let mut key = String::new();
        for (name, read) in FIELDS.iter().zip(&self.facts) {
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

    /// Every comparison the plan makes, each as a media query, or `None`
    /// when some reading has none: a fact read whole, a fact no media
    /// feature carries (`gpuTier`, a fold's segments, a gamut, which a
    /// wider one also matches), or an order on a keyword. A fact the web
    /// holds constant needs none: `accessibilityTextSize` is false in every
    /// browser (`facts.js`), as in the render, so `exact:content`'s
    /// `EmptyState`, which reads it, leaves a page free to wait.
    pub fn media(&self) -> Option<Vec<Media>> {
        let mut out = Vec::new();
        for (fact, read) in FIELDS.iter().zip(&self.facts) {
            match read {
                None => {}
                Some(_) if *fact == "accessibilityTextSize" => {}
                Some(Read::Whole) => return None,
                Some(Read::Compared(all)) => {
                    for (op, constant) in all {
                        out.push(Media {
                            query: query(fact, *op, constant)?,
                            fact,
                            op: *op,
                            constant: constant.clone(),
                        });
                    }
                }
            }
        }
        Some(out)
    }
}

impl Media {
    /// Whether the comparison holds in `viewport`, as the VM compares.
    pub fn holds(&self, viewport: &Viewport) -> bool {
        holds(
            self.op,
            &viewport.field(self.fact).unwrap_or(Value::Unit),
            &self.constant,
        )
    }
}

/// The media query that matches exactly when `fact <op> constant` holds, as
/// the web host reads each fact (`navigation.js` `preferences`, `facts.js`).
fn query(fact: &str, op: Opcode, constant: &Constant) -> Option<String> {
    let not = |q: String, holds_when_matches: bool| {
        if holds_when_matches {
            q
        } else {
            format!("not all and {q}")
        }
    };
    let eq = match op {
        Opcode::Eq => true,
        Opcode::Ne => false,
        _ => {
            // Only a size is ordered: `width` is `innerWidth`, the layout
            // viewport with any scrollbar, as the media feature measures.
            let Constant::Number(n) = constant else {
                return None;
            };
            if !matches!(fact, "width" | "height") {
                return None;
            }
            let op = match op {
                Opcode::Lt => "<",
                Opcode::Le => "<=",
                Opcode::Gt => ">",
                _ => ">=",
            };
            return Some(format!("({fact} {op} {n}px)"));
        }
    };
    let feature = |name: &str, value: &str, values: &[&str]| {
        values
            .contains(&value)
            .then(|| format!("({name}: {value})"))
    };
    match (fact, constant) {
        ("width" | "height", Constant::Number(n)) => Some(not(format!("({fact} = {n}px)"), eq)),
        ("prefersReducedMotion", Constant::Bool(b)) => {
            Some(not("(prefers-reduced-motion: reduce)".into(), eq == *b))
        }
        ("prefersReducedTransparency", Constant::Bool(b)) => Some(not(
            "(prefers-reduced-transparency: reduce)".into(),
            eq == *b,
        )),
        ("prefersColorScheme", Constant::Str(s)) => match s.as_str() {
            "dark" => Some(not("(prefers-color-scheme: dark)".into(), eq)),
            "light" => Some(not("(prefers-color-scheme: dark)".into(), !eq)),
            _ => None,
        },
        ("prefersContrast", Constant::Str(s)) => feature(
            "prefers-contrast",
            s,
            &["no-preference", "more", "less", "custom"],
        )
        .map(|q| not(q, eq)),
        ("pointer", Constant::Str(s)) => {
            feature("pointer", s, &["fine", "coarse", "none"]).map(|q| not(q, eq))
        }
        ("hover", Constant::Str(s)) => feature("hover", s, &["hover", "none"]).map(|q| not(q, eq)),
        ("displayMode", Constant::Str(s)) => feature(
            "display-mode",
            s,
            &[
                "standalone",
                "browser",
                "fullscreen",
                "minimal-ui",
                "window-controls-overlay",
                "picture-in-picture",
                "tabbed",
            ],
        )
        .map(|q| not(q, eq)),
        _ => None,
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
