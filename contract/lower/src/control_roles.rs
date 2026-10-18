//! `tamber-unreachable` (LLP 1121 D9, stage 8 C2): for each target the app
//! ships, each control parameter the app sets that the target's host does
//! not honour there, from the schema's control-role table
//! (`exact_kernel::generated::CONTROLS`). A lint: the platform's look is
//! the answer there. `best-effort` says nothing (theme2's ruling (2)).
//!
//! A parameter is set by an `accent-color` row on the control, an element
//! around it or a class of either, or by the design's `AccentColor`; a
//! field's by the design's `Field`, `FieldText` or `placeholder`, unless
//! the field writes its own row (a node's own row wins over the role).

use crate::LowerError;
use contract_syntax::{Attr, Expr, Node, Span, StyleDecl};
use exact_kernel::generated::{Fidelity, CONTROLS, CONTROL_HOSTS};

/// A field's role and the row of its own that wins over it.
const FIELD_ROWS: [(&str, &[&str]); 3] = [
    ("Field", &["background-color", "background"]),
    ("FieldText", &["color"]),
    ("placeholder", &["::placeholder color"]),
];

/// The prominent button styles: the accent fills them.
const PROMINENT: [&str; 4] = [
    "filled",
    "bordered-prominent",
    "prominent-glass",
    "prominent-clear-glass",
];

/// A control the app draws, its first use, and where its accent comes from.
struct Used {
    control: String,
    at: Span,
    accent: Option<Span>,
    own: Vec<&'static str>,
}

/// Unlowered, nothing read the app's targets or its design: forget an
/// earlier compile's, so no `tamber-unreachable` reads them.
pub(crate) fn unlowered() {
    crate::icons::clear(None);
    crate::theme::forget_roles();
}

/// Every unreachable override, once a target, control and parameter.
pub(crate) fn unreachable(view: &[Node], styles: &[StyleDecl], out: &mut Vec<LowerError>) {
    let targets = crate::icons::shipped();
    let roles = crate::theme::overridden_roles();
    let role = |name: &str| roles.iter().find(|(r, _)| r == name).map(|(_, s)| *s);
    let mut used: Vec<Used> = Vec::new();
    let mut stack: Vec<(&Node, Option<Span>)> = view.iter().rev().map(|n| (n, None)).collect();
    while let Some((node, around)) = stack.pop() {
        let children: Vec<&Node> = match node {
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                span,
                ..
            } => {
                // Each set of classes the node may take is judged on its own
                // (its control, its accent and its field rows together).
                let mut accent = None;
                for rows in Rows::of(attrs, styles).split() {
                    // The accent's row that holds (the node's own over its
                    // classes', cited): `auto` the platform's again, `none`
                    // or none written the one around it.
                    let here = match rows.holds("accent-color")[0] {
                        Some(a) if authored(&a.value) => Some(a.span),
                        Some(a) if !clears(&a.value) => None,
                        _ => around,
                    };
                    accent = accent.or(here);
                    for control in control(tag, positional, &rows) {
                        let own = FIELD_ROWS
                            .iter()
                            .filter(|(_, mine)| mine.iter().any(|m| rows.always(m)))
                            .map(|(r, _)| *r)
                            .collect();
                        used.push(Used {
                            control,
                            at: *span,
                            accent: here,
                            own,
                        });
                    }
                }
                stack.extend(children.iter().rev().map(|c| (c, accent)));
                continue;
            }
            Node::When {
                then, otherwise, ..
            } => then.iter().chain(otherwise).collect(),
            Node::Each { body, .. } => body.iter().collect(),
            Node::Match { some, none, .. } => some.1.iter().chain(none).collect(),
            Node::Use { children, .. } => children.iter().collect(),
            Node::Children { .. } => Vec::new(),
        };
        stack.extend(children.into_iter().rev().map(|c| (c, around)));
    }
    let mut said: Vec<(String, String, &str)> = Vec::new();
    for target in &targets {
        // A target with no column (tvOS, watchOS: not D9's hosts) is not
        // judged; nor the terminal, which draws no colour at all (its
        // column says so, for the table's reader, not for every app).
        let Some(host) = CONTROL_HOSTS
            .iter()
            .position(|h| *h == target.name && *h != "terminal")
        else {
            continue;
        };
        for row in CONTROLS {
            let control = match row.state {
                Some(state) => format!("{}:{state}", row.control),
                None => row.control.to_string(),
            };
            let (fidelity, why) = row.hosts[host];
            if fidelity != Fidelity::None {
                continue;
            }
            // The first use the parameter reaches, and where it is set.
            // A timed progress is the web's bar and every other host's spinner.
            let timed = match (control.as_str(), target.name.as_str()) {
                ("progress", "web")
                | ("spinner", "ios" | "macos" | "linux" | "windows" | "android") => {
                    Some("progress:timed")
                }
                _ => None,
            };
            let reached = used
                .iter()
                .filter(|u| u.control == control || Some(u.control.as_str()) == timed)
                .find_map(|u| {
                    match row.parameter {
                        "accent-color" => u.accent.or(role("AccentColor")),
                        field if u.own.contains(&field) => None,
                        field => role(field),
                    }
                    .map(|set| (u.at, set))
                });
            let Some((first, set)) = reached else {
                continue;
            };
            let key = (target.name.clone(), control.clone(), row.parameter);
            if said.contains(&key) {
                continue;
            }
            said.push(key);
            out.push(LowerError {
                id: "tamber-unreachable",
                message: format!(
                    "{} on {} {}: {why} (the first at line {}; LLP 1121 D9)",
                    row.parameter,
                    host_name(&target.name),
                    plural(&control),
                    first.line
                ),
                span: set,
            });
        }
    }
}

/// A node's rows as they may hold: its own, and each set of classes it
/// may take (a conditional `class` gives one set a branch), its classes'
/// rows in order.
struct Rows<'a> {
    own: &'a [Attr],
    branches: Vec<Vec<&'a Attr>>,
}

impl<'a> Rows<'a> {
    fn of(attrs: &'a [Attr], styles: &'a [StyleDecl]) -> Rows<'a> {
        let mut branches: Vec<Vec<&'a Attr>> = vec![Vec::new()];
        for a in attrs.iter().filter(|a| a.name == "class") {
            let mut names = Vec::new();
            alternatives(&a.value, &mut names);
            let given = |n: &&'a str| -> Vec<&'a Attr> {
                n.split_whitespace()
                    .flat_map(|n| styles.iter().filter(move |s| s.name == n))
                    .flat_map(|s| &s.attrs)
                    .collect()
            };
            branches = branches
                .iter()
                .flat_map(|b| {
                    names
                        .iter()
                        .map(move |n| b.iter().copied().chain(given(n)).collect())
                })
                .collect();
            if branches.is_empty() {
                branches.push(Vec::new());
            }
        }
        Rows {
            own: attrs,
            branches,
        }
    }

    /// One `Rows` a branch.
    fn split(self) -> Vec<Rows<'a>> {
        let own = self.own;
        self.branches
            .into_iter()
            .map(|b| Rows {
                own,
                branches: vec![b],
            })
            .collect()
    }

    /// The row named `name` that holds in each branch: the node's own, else
    /// its classes' last; `None` where none is written.
    fn holds(&self, name: &str) -> Vec<Option<&'a Attr>> {
        let own = self.own.iter().rfind(|a| a.name == name);
        self.branches
            .iter()
            .map(|b| own.or_else(|| b.iter().rfind(|a| a.name == name).copied()))
            .collect()
    }

    /// Every value `name` may hold, in any branch, lower case.
    fn values(&self, name: &str) -> Vec<String> {
        let mut out = Vec::new();
        for a in self.holds(name).into_iter().flatten() {
            alternatives(&a.value, &mut out);
        }
        out.into_iter().map(str::to_ascii_lowercase).collect()
    }

    /// Whether `name` always holds a value: written in every branch, and no
    /// value of it `none`.
    fn always(&self, name: &str) -> bool {
        self.holds(name)
            .into_iter()
            .all(|a| a.is_some_and(|a| !clears(&a.value)))
    }

    /// Whether every value `name` may hold is `value` (and it is written).
    fn only(&self, name: &str, value: &str) -> bool {
        self.holds(name).into_iter().all(|a| {
            a.is_some_and(|a| {
                let mut v = Vec::new();
                alternatives(&a.value, &mut v);
                !v.is_empty()
                    && !clears(&a.value)
                    && v.iter().all(|v| v.eq_ignore_ascii_case(value))
            })
        })
    }
}

/// Each string an expression may be, a branch each (`flag ? A : B`: both).
fn alternatives<'a>(e: &'a Expr, out: &mut Vec<&'a str>) {
    match e {
        Expr::Str(s, _) | Expr::Ident(s, _) => out.push(s),
        Expr::Ternary(_, a, b, _) => {
            alternatives(a, out);
            alternatives(b, out);
        }
        _ => {}
    }
}

/// Whether an accent value may be an authored colour: a branch that is
/// neither `auto` (the platform's) nor `none` (the one around it).
fn authored(e: &Expr) -> bool {
    match e {
        Expr::Str(v, _) => v != "auto",
        Expr::None(_) => false,
        Expr::Ternary(_, a, b, _) => authored(a) || authored(b),
        _ => true,
    }
}

/// Whether a value may be `none` (a branch that clears the row).
fn clears(e: &Expr) -> bool {
    match e {
        Expr::None(_) => true,
        Expr::Ternary(_, a, b, _) => clears(a) || clears(b),
        _ => false,
    }
}

/// The control-role table's control a node is, if it is one.
/// `attrs` are its rows, its classes' included: a style may set
/// `buttonStyle` or `appearance`; a conditional value counts any branch.
fn control(tag: &str, positional: &[Expr], rows: &Rows) -> Vec<String> {
    // A control always of the author's own (`appearance="none"`) paints its rows.
    if rows.only("appearance", "none") {
        return Vec::new();
    }
    let written = |name: &str| {
        rows.holds(name)
            .into_iter()
            .flatten()
            .any(|a| !matches!(a.value, Expr::Bool(false, _)))
    };
    let switch = positional
        .iter()
        .any(|p| contract_syntax::is_input_switch(tag, p))
        || rows.values("role").iter().any(|r| r == "switch");
    let kinds: Vec<&str> = match tag {
        "input" => {
            // Each `type` it may have (none written: a text field; a computed
            // one: no control this can name).
            let types = rows.values("type");
            if types.is_empty() && rows.holds("type").iter().any(Option::is_some) {
                return Vec::new();
            }
            let types = if types.is_empty() {
                vec![String::new()]
            } else {
                types
            };
            types
                .iter()
                .filter_map(|t| match t.as_str() {
                    "checkbox" if switch => Some("switch"),
                    "checkbox" => Some("checkbox"),
                    "radio" => Some("radio"),
                    "range" => Some("range"),
                    "" | "text" | "search" | "email" | "url" | "tel" | "password" | "number" => {
                        Some("field")
                    }
                    _ => None,
                })
                .collect()
        }
        "textarea" => vec!["textarea"],
        // A value: a bar. A timed progress (`start`, no value) is a bar on
        // the web and a spinner elsewhere (`progress:timed`, judged so).
        "progress" if written("value") => vec!["progress"],
        "progress" if written("start") => vec!["progress:timed"],
        "progress" => vec!["spinner"],
        "select" => vec!["select"],
        "button"
            if rows
                .values("buttonStyle")
                .iter()
                .any(|s| PROMINENT.contains(&s.as_str())) =>
        {
            vec!["button-prominent"]
        }
        _ => Vec::new(),
    };
    // A control that writes `disabled` is its `:disabled` row's, where the
    // table has one.
    let mut out: Vec<String> = kinds
        .into_iter()
        .map(|c| {
            let disabled = CONTROLS
                .iter()
                .any(|r| r.control == c && r.state == Some("disabled"));
            match written("disabled") && disabled {
                true => format!("{c}:disabled"),
                false => c.to_string(),
            }
        })
        .collect();
    out.dedup();
    out
}

fn host_name(host: &str) -> &str {
    match host {
        "ios" => "iOS",
        "macos" => "macOS",
        "web" => "the web's",
        "linux" => "Linux",
        "windows" => "Windows",
        "terminal" => "the terminal's",
        "android" => "Android",
        other => other,
    }
}

fn plural(control: &str) -> String {
    if let Some(c) = control.strip_suffix(":disabled") {
        return format!("disabled {}", plural(c));
    }
    match control {
        "switch" => "switches".into(),
        "checkbox" => "checkboxes".into(),
        "progress" => "progress bars".into(),
        "button-prominent" => "prominent buttons".into(),
        other => format!("{other}s"),
    }
}
