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
//!
//! Beside it, `tamber-contrast` for the design's `AccentColor` under a
//! prominent button's white label (LLP 1116.019 D3), which `check` judges
//! only on a screen a drive reaches.

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

/// The contrast below which a control's label on its own fill is low:
/// WCAG 1.4.11's 3:1, as `check` judges it (scripts/agent-check.mjs).
const LABEL_CONTRAST: f64 = 3.0;

thread_local! {
    /// White's lowest contrast over the design's `AccentColor`, and whether
    /// it falls in dark, while the app's view is judged.
    static ACCENT_INK: std::cell::Cell<Option<(f64, bool)>> = const { std::cell::Cell::new(None) };
}

/// Note what white reads at on the `AccentColor` of the design lowered.
pub(crate) fn note_accent(ink: Option<(f64, bool)>) {
    ACCENT_INK.with(|a| a.set(ink));
}

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
    note_accent(None);
    crate::icons::clear(None);
    crate::theme::forget_roles();
}

/// Every unreachable override, once a target, control and parameter, and
/// one line for each declaration that sets it.
pub(crate) fn unreachable(view: &[Node], styles: &[StyleDecl], out: &mut Vec<LowerError>) {
    // In the table's host order, so a line names the hosts as the table does.
    let mut targets = crate::icons::shipped();
    targets.sort_by_key(|t| CONTROL_HOSTS.iter().position(|h| *h == t.name));
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
    // UIKit keeps a filled button's white label on any tint (LLP 1116.019 D3).
    let low = ACCENT_INK
        .with(|a| a.get())
        .filter(|(r, _)| *r < LABEL_CONTRAST);
    let filled = used
        .iter()
        .find(|u| u.control.starts_with("button-prominent") && u.accent.is_none());
    if let (Some((r, dark)), Some(u), Some(set)) = (low, filled, role("AccentColor")) {
        let scheme = if dark { "dark" } else { "light" };
        out.push(LowerError {
            id: "tamber-contrast",
            message: format!(
                "`AccentColor` is {:.2}:1 under a prominent button's white label in {scheme}: below {LABEL_CONTRAST}:1, a control label's ratio; UIKit keeps the label white on any tint, so leave the accent unsaid (the platform's blue) or deepen its {scheme} side (the first at line {}; LLP 1116.019 D3)",
                (r * 100.0).floor() / 100.0,
                u.at.line
            ),
            span: set,
        });
    }
    let mut said: Vec<(String, String, &str)> = Vec::new();
    let mut found: Vec<Found> = Vec::new();
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
            let f = match found
                .iter_mut()
                .find(|f| f.parameter == row.parameter && f.span == set)
            {
                Some(f) => f,
                None => {
                    found.push(Found {
                        parameter: row.parameter,
                        span: set,
                        line: first.line,
                        parts: Vec::new(),
                        targets: Vec::new(),
                    });
                    found.last_mut().expect("just pushed")
                }
            };
            f.line = f.line.min(first.line);
            if !f.targets.contains(&target) {
                f.targets.push(target);
            }
            // A control and its state that the host leaves alike (a range
            // and a disabled range) are one part, and so are the hosts that
            // leave the same controls alike.
            let host = host_name(&target.name);
            let named = plural(&control);
            if let Some(p) = f
                .parts
                .iter_mut()
                .find(|p| p.why == why && p.hosts == [host])
            {
                p.controls.push(named);
            } else if let Some(p) = f
                .parts
                .iter_mut()
                .find(|p| p.why == why && p.controls == [named.as_str()])
            {
                p.hosts.push(host);
            } else {
                f.parts.push(Part {
                    hosts: vec![host],
                    controls: vec![named],
                    why,
                });
            }
        }
    }
    // One line a declaration (LLP 1121 D9, amended 2026-10-30): an accent
    // that six controls leave alone is one thing to know, not six.
    for f in found {
        out.push(LowerError {
            id: "tamber-unreachable",
            message: f.message(),
            span: f.span,
        });
    }
}

/// One line of `unreachable`: a parameter one declaration sets, and every
/// host and control that does not honour it.
struct Found<'t> {
    parameter: &'static str,
    span: Span,
    line: u32,
    parts: Vec<Part<'t>>,
    targets: Vec<&'t crate::icons::Target>,
}

/// Controls on hosts that leave the parameter alone for one reason.
struct Part<'t> {
    hosts: Vec<&'t str>,
    controls: Vec<String>,
    why: &'static str,
}

impl Found<'_> {
    fn message(&self) -> String {
        let mut hosts: Vec<&str> = Vec::new();
        for h in self.parts.iter().flat_map(|p| &p.hosts) {
            if !hosts.contains(h) {
                hosts.push(h);
            }
        }
        let body = match &self.parts[..] {
            [p] => format!(
                "{} on {} {}: {}",
                self.parameter,
                and(&p.hosts),
                p.controls.join(" and "),
                p.why
            ),
            // One host names itself once; several name each part's.
            parts => {
                let each = parts
                    .iter()
                    .map(|p| match hosts.len() {
                        1 => format!("{} ({})", p.controls.join(" and "), p.why),
                        _ => format!("{} {} ({})", and(&p.hosts), p.controls.join(" and "), p.why),
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                format!(
                    "{} on {}: the platform's look stands on {each}",
                    self.parameter,
                    and(&hosts).replace("the web's", "the web")
                )
            }
        };
        format!(
            "{body} (the first at line {}; LLP 1121 D9){}",
            self.line,
            self.stop()
        )
    }

    /// What ships each native target and how to stop. An app made by
    /// `exact new` ships the Mac once a verb names it, so the line says so.
    /// The web stays: it is the dev loop's and the agent's.
    fn stop(&self) -> String {
        let native: Vec<_> = self.targets.iter().filter(|t| t.name != "web").collect();
        match &native[..] {
            [] => String::new(),
            [t] => match crate::icons::without(&t.name) {
                Some(step) => format!(
                    ". The app ships {}; not shipping it? {step} removes it",
                    t.described()
                ),
                None => format!(
                    ". The app ships {}; not shipping it? delete those keys",
                    t.described()
                ),
            },
            several => {
                let ships: Vec<String> = several.iter().map(|t| t.described()).collect();
                let mut steps: Vec<String> = several
                    .iter()
                    .filter_map(|t| {
                        crate::icons::without(&t.name)
                            .map(|step| format!("{step} removes {}", t.name))
                    })
                    .collect();
                if steps.len() < several.len() {
                    steps.push(match steps.is_empty() {
                        true => "delete its keys".into(),
                        false => "otherwise delete its keys".into(),
                    });
                }
                format!(
                    ". The app ships {}; not shipping one? {}",
                    and(&ships),
                    steps.join("; ")
                )
            }
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

/// `a`, `a and b`, `a, b and c`.
fn and<S: AsRef<str>>(names: &[S]) -> String {
    match names {
        [one] => one.as_ref().to_string(),
        [rest @ .., last] => {
            let rest: Vec<&str> = rest.iter().map(AsRef::as_ref).collect();
            format!("{} and {}", rest.join(", "), last.as_ref())
        }
        [] => String::new(),
    }
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
