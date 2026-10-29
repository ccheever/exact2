//! A component's anatomy, checked (LLP 1139 D2, D6; stage 12 C1): each part
//! a node of the component's own view, named once and declared; each
//! group one the table knows; each row a `class` brings to a part's node in
//! that part's groups, in every branch of a conditional class and every
//! state layer; and a native control a part with the `control` group alone,
//! its rows (its own and its classes') the control's.
//!
//! The checks read [`contract_syntax::anatomy::Anatomy`], the data whatever
//! its spelling (LLP 1139 §9 Q7): a change of spelling never reaches them.
//! They run on every component that says anything of parts; which
//! components must have parts is the library rules' (C3).

pub(crate) mod library;
mod platform;
pub mod slot;

use crate::{tags, LowerError, Lowerer};
use contract_syntax::anatomy::{Anatomy, Mark, PartDecl};
use contract_syntax::{Attr, Component, Expr, File, Node, Span};
use exact_kernel::StyleId;

/// The row groups a part may take (LLP 1139 D2's table), by name.
pub const GROUPS: [&str; 5] = ["paint", "text", "shape", "space", "control"];

/// Whether group `group` holds row `id`; `control`'s rows are the control's
/// ([`control_rows`]).
fn holds(group: &str, id: StyleId) -> bool {
    use StyleId::*;
    match group {
        "paint" => matches!(
            id,
            BackgroundColor
                | BorderColorTop
                | BorderColorRight
                | BorderColorBottom
                | BorderColorLeft
                | BorderWidthTop
                | BorderWidthRight
                | BorderWidthBottom
                | BorderWidthLeft
                | BoxShadow
                | Opacity
                | TintColor
                | PressScale
        ),
        "text" => matches!(
            id,
            TextColor
                | FontFamily
                | FontSize
                | FontWeight
                | FontStyle
                | FontVariantNumeric
                | LineHeight
                | LetterSpacing
                | TextAlign
                | TextTransform
                | TextDecorationLine
                | WhiteSpace
        ),
        "shape" => matches!(
            id,
            BorderRadiusTopLeft
                | BorderRadiusTopRight
                | BorderRadiusBottomRight
                | BorderRadiusBottomLeft
        ),
        "space" => matches!(
            id,
            PaddingTop
                | PaddingRight
                | PaddingBottom
                | PaddingLeft
                | RowGap
                | ColumnGap
                | AlignItems
                | JustifyContent
        ),
        _ => false,
    }
}

/// The rows a native control's part takes (the `control` group, LLP 1139
/// D2, D6): its accent, and on a native `button` the rows a native button's
/// box carries (`controls::NATIVE_ROWS`).
fn control_rows(button: bool, id: StyleId) -> bool {
    id == StyleId::AccentColor || button && crate::controls::native_row(id)
}

/// Whether a part's element is a native control (LLP 1139 D6): an `input`
/// of any type, a `textarea`, a `select`, a `progress`, or a `button` that
/// says so (`appearance="auto"`) or is named for the `control` group. Any
/// other button is the authored pressable D9's exception keeps (the Post
/// button), which takes the other groups.
fn native(tag: &str, rows: &[&Attr], control: bool) -> bool {
    match tag {
        "input" | "textarea" | "select" | "progress" => true,
        "button" => control || appearance(rows) == Some("auto"),
        _ => false,
    }
}

/// The attributes of `class` (its rows) that reach the node: those whose
/// longhands the node's own attributes don't all set in the same state
/// layer (`class::rows`' rule, by longhand: a node's `margin` overrides a
/// class's `margin-top`).
pub(crate) fn surviving<'a>(class: &'a [Attr], own: &[Attr]) -> Vec<&'a Attr> {
    let layer = |a: &Attr| contract_syntax::state::state_of(&a.name).map(|(s, _)| s.to_string());
    class
        .iter()
        .filter(|a| {
            let ids = rows_of(a);
            let set: Vec<StyleId> = own
                .iter()
                .filter(|o| layer(o) == layer(a))
                .flat_map(rows_of)
                .collect();
            let by_name = own.iter().any(|o| o.name == a.name);
            !(by_name || !ids.is_empty() && ids.iter().all(|id| set.contains(id)))
        })
        .collect()
}

/// A button's `appearance`, the last its rows say.
fn appearance<'a>(rows: &[&'a Attr]) -> Option<&'a str> {
    rows.iter()
        .rev()
        .find(|a| a.name == "appearance")
        .and_then(|a| match &a.value {
            Expr::Str(v, _) => Some(v.as_str()),
            _ => None,
        })
}

/// Whether a part's name is a lower-case word (LLP 1139 D2).
fn word(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The element of `nodes` written at `span`, at any depth: its tag, its
/// attributes and its children.
fn element(nodes: &[Node], span: Span) -> Option<(&str, &[Attr], &[Node])> {
    nodes.iter().find_map(|n| match n {
        Node::Element {
            tag,
            attrs,
            children,
            span: s,
            ..
        } => (*s == span)
            .then_some((tag.as_str(), attrs.as_slice(), children.as_slice()))
            .or_else(|| element(children, span)),
        Node::Use { children, .. } | Node::Each { body: children, .. } => element(children, span),
        Node::When {
            then, otherwise, ..
        } => element(then, span).or_else(|| element(otherwise, span)),
        Node::Match { some, none, .. } => element(&some.1, span).or_else(|| element(none, span)),
        Node::Children { .. } => None,
    })
}

/// Every element's span in `nodes`, at any depth (a fill this view passes
/// to a component it uses among them: it writes them).
fn element_spans(nodes: &[Node], out: &mut Vec<Span>) {
    for n in nodes {
        match n {
            Node::Element { children, span, .. } => {
                out.push(*span);
                element_spans(children, out);
            }
            Node::Use { children, .. } | Node::Each { body: children, .. } => {
                element_spans(children, out)
            }
            Node::When {
                then, otherwise, ..
            } => {
                element_spans(then, out);
                element_spans(otherwise, out);
            }
            Node::Match { some, none, .. } => {
                element_spans(&some.1, out);
                element_spans(none, out);
            }
            Node::Children { .. } => {}
        }
    }
}

/// The styles a node's `class` names: one, or each side of a choice.
fn classes(attrs: &[Attr]) -> Vec<(&str, Span)> {
    fn names<'e>(e: &'e Expr, out: &mut Vec<(&'e str, Span)>) {
        match e {
            Expr::Ident(name, span) => out.push((name, *span)),
            Expr::Ternary(_, yes, no, _) => {
                names(yes, out);
                names(no, out);
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    if let Some(c) = attrs.iter().find(|a| a.name == "class") {
        names(&c.value, &mut out);
    }
    // A style both sides name is one.
    let mut seen: Vec<&str> = Vec::new();
    out.retain(|(name, _)| {
        let first = !seen.contains(name);
        seen.push(name);
        first
    });
    out
}

/// The style rows an attribute writes, a state part's as its row's
/// (`:disabled background-color` is `background-color`'s).
fn rows_of(a: &Attr) -> Vec<StyleId> {
    let row = contract_syntax::state::state_of(&a.name).map_or(a.name.as_str(), |(_, row)| row);
    tags::rows(&Attr {
        name: row.to_string(),
        ..a.clone()
    })
}

/// Each refusal of `c`'s anatomy, its styles found in `file`.
pub(crate) fn check(c: &Component, file: &File) -> Vec<LowerError> {
    let Anatomy { parts, marks } = &c.anatomy;
    let mut out = Vec::new();
    let refuse = |id: &'static str, message: String, span: Span| LowerError { id, message, span };
    let comp = &c.name;
    for p in parts {
        if !word(&p.name) {
            out.push(refuse(
                "lower-anatomy-part",
                format!("part `{}` of `{comp}`: a part's name is a lower-case word (`surface`, `header`)", p.name),
                p.span,
            ));
        }
        for (g, span) in &p.groups {
            if !GROUPS.contains(&g.as_str()) {
                out.push(refuse(
                    "lower-anatomy-group",
                    format!(
                        "part `{}` of `{comp}`: `{g}` is no row group ({})",
                        p.name,
                        GROUPS.join(", ")
                    ),
                    *span,
                ));
            }
        }
        if !marks.iter().any(|m| m.part == p.name) {
            out.push(refuse(
                "lower-anatomy-part",
                format!(
                    "part `{}` of `{comp}` names no node: write `{}` on a node of its view",
                    p.name,
                    contract_syntax::anatomy::written(&p.name)
                ),
                p.span,
            ));
        }
    }
    for (i, m) in marks.iter().enumerate() {
        if m.part.is_empty() {
            out.push(refuse(
                "lower-anatomy-part",
                format!(
                    "a part of `{comp}` is named by a non-empty string literal (`{}`)",
                    contract_syntax::anatomy::written("surface")
                ),
                m.span,
            ));
            continue;
        }
        if !m.own {
            out.push(refuse(
                "lower-anatomy-part",
                format!("`{}` is on a node of a used component's slot fill, not of `{comp}`'s own view: a part is the component's own node", contract_syntax::anatomy::written(&m.part)),
                m.span,
            ));
            continue;
        }
        // One node, one part: D5's selection names the part a node is.
        if let Some(first) = marks[..i]
            .iter()
            .find(|f| f.node == m.node && f.part != m.part && !f.part.is_empty())
        {
            out.push(LowerError {
                id: "lower-anatomy-part",
                message: format!(
                    "one node of `{comp}` is named both part `{}` and part `{}`: a node is one part",
                    first.part, m.part
                ),
                span: m.span,
            });
            continue;
        }
        if let Some(first) = marks[..i].iter().find(|f| f.part == m.part && f.own) {
            out.push(LowerError {
                id: "lower-anatomy-part",
                message: format!(
                    "two nodes of `{comp}` are part `{}` (the first at line {}): a part is one node",
                    m.part, first.span.line
                ),
                span: m.span,
            });
            continue;
        }
        let Some(decl) = parts.iter().find(|p| p.name == m.part) else {
            out.push(refuse(
                "lower-anatomy-part",
                format!(
                    "`{}`: `{comp}` declares no part `{}` (its `{}`: {})",
                    contract_syntax::anatomy::written(&m.part),
                    m.part,
                    contract_syntax::anatomy::SECTION,
                    names(parts)
                ),
                m.span,
            ));
            continue;
        };
        out.extend(rows(c, file, decl, m));
    }
    out
}

fn names(parts: &[PartDecl]) -> String {
    match parts.is_empty() {
        true => "none".into(),
        false => parts
            .iter()
            .map(|p| format!("`{}`", p.name))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

/// What part `decl`'s node, marked by `m`, is refused for: a class row
/// outside its groups, or a native control's part out of the `control`
/// group's rules. A native button's part is asked again by lowering, which
/// knows when the platform's button is demoted ([`Lowerer::check_anatomy`]).
fn rows(c: &Component, file: &File, decl: &PartDecl, m: &Mark) -> Vec<LowerError> {
    let mut out = Vec::new();
    let Some((tag, attrs, _)) = element(&c.view, m.node) else {
        return out;
    };
    let comp = &c.name;
    let part = &decl.name;
    let groups: Vec<&str> = decl.groups.iter().map(|(g, _)| g.as_str()).collect();
    let said = groups.join(" ");
    // The style a name finds is the one lowering keeps (the last), and a
    // class row the node sets itself never reaches it (`class::rows`).
    let styles: Vec<(&str, Vec<&Attr>)> = classes(attrs)
        .into_iter()
        .filter_map(|(name, _)| {
            let s = file.styles.iter().rev().find(|s| s.name == name)?;
            Some((name, surviving(&s.attrs, attrs)))
        })
        .collect();
    let all: Vec<&Attr> = styles
        .iter()
        .flat_map(|(_, rows)| rows.iter().copied())
        .chain(attrs)
        .collect();
    let control = groups.contains(&"control");
    if native(tag, &all, control) {
        if !groups.iter().all(|g| *g == "control") {
            out.push(LowerError {
                id: "lower-anatomy-control",
                message: format!("part `{part}` of `{comp}` is a native `{tag}`: a native control's part takes the `control` group alone (`{}`), not `{said}`{} (LLP 1139 D6)", contract_syntax::anatomy::declared(part, &["control"]), if tag == "button" { "; a `button` with `appearance=\"none\"` is a bare box, which takes the other groups" } else { "" }),
                span: decl.span,
            });
            return out;
        }
        let button = tag == "button";
        let from = |a: &Attr| {
            styles
                .iter()
                .find(|(_, rows)| rows.iter().any(|r| std::ptr::eq(*r, a)))
                .map_or_else(
                    || "its own row".to_string(),
                    |(s, _)| format!("`style {s}`"),
                )
        };
        for a in &all {
            if rows_of(a).iter().any(|id| !control_rows(button, *id)) {
                out.push(LowerError {
                    id: "lower-anatomy-control",
                    message: format!("`{}` ({}) on part `{part}` of `{comp}`, a native `{tag}`: the `control` group takes {} (LLP 1139 D2, D6)", a.name, from(a), if button { "`accent-color` and the rows a native button carries" } else { "`accent-color` alone" }),
                    span: a.span,
                });
            }
        }
        // A corner keeps a button native only with `appearance="auto"`:
        // without it the button is demoted to a bare box (`buttons.rs`).
        let radius = all.iter().find(|a| {
            rows_of(a).iter().any(|id| {
                matches!(
                    id,
                    StyleId::BorderRadiusTopLeft
                        | StyleId::BorderRadiusTopRight
                        | StyleId::BorderRadiusBottomRight
                        | StyleId::BorderRadiusBottomLeft
                )
            })
        });
        if let (true, Some(a), false) = (button, radius, appearance(&all) == Some("auto")) {
            out.push(LowerError {
                id: "lower-anatomy-control",
                message: format!("`{}` on part `{part}` of `{comp}`: a corner keeps a native button native only with `appearance=\"auto\"`, else it draws as a bare box (LLP 1139 D6)", a.name),
                span: a.span,
            });
        }
        return out;
    }
    if control {
        out.push(LowerError {
            id: "lower-anatomy-control",
            message: format!("part `{part}` of `{comp}` is a `{tag}`, no native control: the `control` group is a native control's alone (LLP 1139 D6)"),
            span: decl.span,
        });
        return out;
    }
    for (style, rows) in &styles {
        for a in rows.iter() {
            if rows_of(a)
                .iter()
                .any(|id| !groups.iter().any(|g| holds(g, *id)))
            {
                out.push(LowerError {
                    id: "lower-anatomy-row",
                    message: format!("`{}` in `style {style}`, which `class` brings to part `{part}` of `{comp}`: the part takes `{said}`, and a class brings a part only its groups' rows (LLP 1139 D2)", a.name),
                    span: a.span,
                });
            }
        }
    }
    out
}

/// Each line of the design's `params` block a library's names refuse (LLP
/// 1139 D8), with its line and column: one naming a param under a library
/// package that has none (not warned of, as an app's own is), and a
/// library component's param by its short name.
pub(crate) fn param_lines(
    design: &exact_tamber::Design,
    params: &[crate::params::ParamToken],
    identities: &[String],
    file: &File,
) -> Vec<(u32, u32, LowerError)> {
    let under_library = |name: &str| {
        file.library_packages
            .iter()
            .any(|p| name.starts_with(&format!("{p}/")))
    };
    // A short name's component (`Card` of `Card.gap`).
    let short = |name: &str| name.split_once('.').map_or(name, |(c, _)| c).to_string();
    let exported = |name: &str| {
        !name.contains('#')
            && !name.contains('/')
            && file
                .library
                .iter()
                .any(|i| i.rsplit_once('#').is_some_and(|(_, c)| c == short(name)))
    };
    let mut out = Vec::new();
    let resolver = crate::params::Resolver::new(params, identities);
    for entry in &design.params {
        let name = &entry.name.value;
        let error = match resolver.resolve(name) {
            Err(e) if e.id == "tamber-param-unknown" && under_library(name) => LowerError {
                id: "tamber-package-unknown",
                message: format!("`{name}` in the design's `params` block names no param of the library: its names are its API, so a renamed or removed one fails the build (LLP 1139 D8)"),
                span: Span::default(),
            },
            // Short, naming no param, as a library component's (`Card.gap`
            // once `gap` is gone): a rename never falls back to the warning.
            Err(e) if e.id == "tamber-param-unknown" && exported(name) => LowerError {
                id: "tamber-param-short-library",
                message: format!("`{name}` names no param, and `{}` is a library component: a library's params are named in full (`<package>/<file>#{}.<param>`), so a renamed or removed one fails the build (LLP 1139 D8)", short(name), short(name)),
                span: Span::default(),
            },
            Ok(p) if !name.contains('#') && file.library.contains(&p.identity) => LowerError {
                id: "tamber-param-short-library",
                message: format!("`{name}` names a library component's param by its short name: write it in full, `{}` (LLP 1139 D8)", p.full),
                span: Span::default(),
            },
            _ => continue,
        };
        out.push((entry.name.line, entry.name.col, error));
    }
    out
}

/// The design's `params` lines naming no param of the app
/// (`tamber-param-unknown`, a warning), less those a library's names
/// refuse ([`param_lines`]), never both.
pub(crate) fn unknown_params(
    design: &exact_tamber::Design,
    params: &[crate::params::ParamToken],
    file: &File,
    source_id: u32,
) -> Vec<LowerError> {
    let identities = crate::params::identities(file);
    let refused: Vec<(u32, u32)> = param_lines(design, params, &identities, file)
        .into_iter()
        .map(|(line, col, _)| (line, col))
        .collect();
    crate::params::unknown(design, params, &identities, source_id)
        .into_iter()
        .filter(|w| !refused.contains(&(w.span.line, w.span.col)))
        .collect()
}

/// Each host read in a component of a library package (LLP 1139 D6).
pub(crate) fn platform(file: &File) -> Vec<LowerError> {
    let components: Vec<(&Component, &str)> = file
        .components
        .iter()
        .filter_map(|c| {
            let i = c.params.identity.as_ref()?;
            let p = file
                .library_packages
                .iter()
                .find(|p| i.starts_with(&format!("{p}/")))?;
            Some((c, p.as_str()))
        })
        .collect();
    platform::check(file, &components)
}

/// A `button` named as a part (LLP 1139 D6), which lowering judges where it
/// decides whether the platform draws it.
#[derive(Debug, Clone)]
pub(crate) struct ControlPart {
    /// The button's node, as written (an instance's keeps it).
    pub node: Span,
    /// Where it is named as the part.
    pub mark: Span,
    /// The part's name and its component's.
    pub part: String,
    pub component: String,
    /// Whether the part is named for the `control` group (alone).
    pub control: bool,
}

impl Lowerer<'_> {
    /// Every component's anatomy, checked ([`check`]); and each native
    /// button's `control` part noted, which lowering refuses where it draws
    /// the button as a bare box (a row's value, `align-items: stretch` or
    /// `display: block`, a transition, the button's context), as it decides
    /// for that instance ([`Lowerer::bare_control`]).
    pub(crate) fn check_anatomy(&mut self, file: &File) {
        for e in platform(file) {
            self.push(e);
        }
        for c in &file.components {
            // An exported library component's slot (LLP 1139 D0, D4).
            let library = c
                .params
                .identity
                .as_ref()
                .is_some_and(|i| file.library.contains(i));
            if library {
                let identity = c.params.identity.clone().unwrap_or_default();
                let mut spans = Vec::new();
                element_spans(&c.view, &mut spans);
                self.library_nodes
                    .extend(spans.into_iter().map(|s| (s, identity.clone())));
            }
            // The library rules (LLP 1139 D1, D6): an exported component's
            // knobs and controls; any component of a library package's host
            // reads.
            if library {
                let controls = library::control(c);
                for e in library::literal(c, file)
                    .into_iter()
                    .chain(controls.refused)
                {
                    self.push(e);
                }
                self.library_buttons.extend(controls.buttons);
            }

            if library && c.slot {
                // A token's values over the tuples (a param's is its
                // token's, the rows already name it), each base name or
                // expression over one in every column of the scale.
                let columns = |v: &str| {
                    crate::scale::ranged(v)
                        .map(|cols| cols.into_iter().map(|(_, n)| crate::scale::px(n)).collect())
                };
                let fold = |name: &str| -> Option<Vec<String>> {
                    // The design's own token first (a base name it sets
                    // among them); a base name it leaves unset, or an
                    // expression over one, in every column.
                    let Some((_, values)) = crate::theme::measure_written(name) else {
                        return columns(name);
                    };
                    Some(
                        values
                            .into_iter()
                            .flat_map(|v| columns(&v).unwrap_or_else(|| vec![v]))
                            .collect(),
                    )
                };
                for e in slot::check(c, file, &fold) {
                    self.push(e);
                }
            }
            if c.anatomy.is_empty() {
                continue;
            }
            let refused = check(c, file);
            let flagged = |m: &Mark| {
                let said = format!("part `{}` of `{}`", m.part, c.name);
                refused.iter().any(|e| {
                    e.span == m.span || e.id == "lower-anatomy-control" && e.message.contains(&said)
                })
            };
            self.part_names.extend(
                c.anatomy
                    .marks
                    .iter()
                    .filter(|m| m.own && c.anatomy.parts.iter().any(|p| p.name == m.part))
                    .map(|m| (m.node, m.part.clone())),
            );
            for m in c.anatomy.marks.iter().filter(|m| m.own && !flagged(m)) {
                let Some(decl) = c.anatomy.parts.iter().find(|p| p.name == m.part) else {
                    continue;
                };
                let control = decl.groups.iter().all(|(g, _)| g == "control");
                if let Some(("button", ..)) = element(&c.view, m.node) {
                    self.control_parts.push(ControlPart {
                        node: m.node,
                        mark: m.span,
                        part: m.part.clone(),
                        component: c.name.clone(),
                        control,
                    });
                }
            }
            for e in refused {
                self.push(e);
            }
        }
    }

    /// A native button's `control` part that lowering, deciding at `node`,
    /// draws as a bare box (`reason`): refused (LLP 1139 D6).
    /// A `button` part as lowering decides it at `node` (`native`, or a
    /// bare box for `reason`): a `control` part drawn as a bare box, or any
    /// other part the platform draws as its own button, refused (LLP 1139
    /// D6: the native button's part is the `control` group's alone, and a
    /// library control is refused where an app's button would be demoted).
    pub(crate) fn button_part(&mut self, node: Span, native: bool, reason: Option<&str>) {
        // A library button with a control's role, drawn as a bare box (D6).
        if !native {
            if let Some(i) = self.library_buttons.iter().position(|e| e.span == node) {
                let e = self.library_buttons.swap_remove(i);
                self.push(e);
            }
        }
        let Some(p) = self.control_parts.iter().find(|p| p.node == node).cloned() else {
            return;
        };
        let message = match (p.control, native) {
            (true, false) => {
                let why =
                    reason.map_or_else(|| "its `appearance` is `none`".to_string(), str::to_string);
                format!("part `{}` of `{}` is a native button the platform draws as a bare box here: {why} (LLP 1139 D6: a library control is refused where an app's button would be demoted)", p.part, p.component)
            }
            (false, true) => format!("part `{}` of `{}` is a button the platform draws as its own here: a native button's part takes the `control` group alone (`{}`); write `appearance=\"none\"` for the authored pressable, which takes the other groups (LLP 1139 D6)", p.part, p.component, contract_syntax::anatomy::declared(&p.part, &["control"])),
            _ => return,
        };
        self.push(LowerError {
            id: "lower-anatomy-control",
            message,
            span: p.mark,
        });
    }
}

/// What the Tamber IR says of a component's anatomy (LLP 1139 D5).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Described {
    /// Each part: its name, its groups, and whether its node holds the
    /// slot's `children`.
    pub parts: Vec<(String, Vec<String>, bool)>,
    /// Each variant prop (an `enum<…>` or `bool` prop a `class` condition,
    /// or a row's `?:` or `match`, of the component's view reads), with its
    /// options.
    pub variants: Vec<(String, Vec<String>)>,
    /// Whether it is an exported library component (LLP 1139 D0).
    pub library: bool,
    /// An exported library component's slot chain (LLP 1139 D4), root first: each node's tag and the
    /// rows of the deny-list its classes (every branch and state layer) and
    /// its own attributes write, each `(row, value)` as written (a `?:`'s
    /// every leaf; `?` for one no literal says). What the design server
    /// asks [`slot::denied`] of on a candidate fold.
    pub chain: Vec<(String, Vec<(String, String)>)>,
}

/// [`Described`] for `c`, its styles found in `file`.
pub fn described(c: &Component, file: &File) -> Described {
    let parts = c
        .anatomy
        .parts
        .iter()
        .map(|p| {
            let slot = c
                .anatomy
                .marks
                .iter()
                .filter(|m| m.part == p.name && m.own)
                .any(|m| holds_children(&c.view, m.node));
            (
                p.name.clone(),
                p.groups.iter().map(|(g, _)| g.clone()).collect(),
                slot,
            )
        })
        .collect();
    let mut read: Vec<String> = Vec::new();
    conditions(&c.view, &mut read);
    let variants = c
        .props
        .iter()
        .filter(|p| read.contains(&p.name))
        .filter_map(|p| {
            let options = match p.ty.as_ref()? {
                contract_syntax::TypeExpr::Enum(options, _) => options.clone(),
                contract_syntax::TypeExpr::Named(n, _) if n == "bool" => {
                    vec!["false".into(), "true".into()]
                }
                _ => return None,
            };
            Some((p.name.clone(), options))
        })
        .collect();
    let library = c
        .params
        .identity
        .as_ref()
        .is_some_and(|i| file.library.contains(i));
    let chain = if c.slot && library {
        slot::chain(c, file)
    } else {
        Vec::new()
    };
    Described {
        parts,
        variants,
        library,
        chain,
    }
}

/// Whether the element written at `span` holds `children` among its own.
fn holds_children(nodes: &[Node], span: Span) -> bool {
    nodes.iter().any(|n| match n {
        Node::Element {
            children, span: s, ..
        } if *s == span => children.iter().any(|c| matches!(c, Node::Children { .. })),
        Node::Element { children, .. }
        | Node::Use { children, .. }
        | Node::Each { body: children, .. } => holds_children(children, span),
        Node::When {
            then, otherwise, ..
        } => holds_children(then, span) || holds_children(otherwise, span),
        Node::Match { some, none, .. } => {
            holds_children(&some.1, span) || holds_children(none, span)
        }
        Node::Children { .. } => false,
    })
}

/// The names a `?:` or a `match` reads in a `class` or a style row of
/// `nodes`, at any depth, but those a `for` or a `match` arm binds there:
/// what makes a prop a variant (LLP 1139 D5).
fn conditions(nodes: &[Node], out: &mut Vec<String>) {
    walk_conditions(nodes, &mut Vec::new(), out);
}

/// The names `e` reads inside a `?:` or a `match` (`inside`, once one is
/// entered), but those bound there: a `match` arm's, an arrow's, a `let`'s.
fn reads(e: &Expr, bound: &mut Vec<String>, inside: bool, out: &mut Vec<String>) {
    match e {
        Expr::Ident(name, _) => {
            if inside && !bound.contains(name) && !out.contains(name) {
                out.push(name.clone());
            }
        }
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(..) => {}
        Expr::Ternary(a, b, c, _) => {
            for x in [a, b, c] {
                reads(x, bound, true, out);
            }
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => {
            reads(subject, bound, true, out);
            bound.push(var.clone());
            reads(some, bound, true, out);
            bound.pop();
            reads(none, bound, true, out);
        }
        Expr::Arrow { params, body, .. } => {
            let mark = bound.len();
            bound.extend(params.iter().cloned());
            reads(body, bound, inside, out);
            bound.truncate(mark);
        }
        Expr::Let {
            name, value, body, ..
        } => {
            reads(value, bound, inside, out);
            bound.push(name.clone());
            reads(body, bound, inside, out);
            bound.pop();
        }
        Expr::Template(parts, _) => {
            for p in parts {
                if let contract_syntax::TemplatePart::Expr(x) = p {
                    reads(x, bound, inside, out);
                }
            }
        }
        Expr::List(items, _) | Expr::Call(_, items, _) => {
            for x in items {
                reads(x, bound, inside, out);
            }
        }
        Expr::Some(x, _)
        | Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Typed(x, _, _)
        | Expr::Unary(_, x, _) => reads(x, bound, inside, out),
        Expr::Binary(_, a, b, _) => {
            reads(a, bound, inside, out);
            reads(b, bound, inside, out);
        }
    }
}

fn walk_conditions(nodes: &[Node], bound: &mut Vec<String>, out: &mut Vec<String>) {
    for n in nodes {
        match n {
            Node::Element {
                attrs, children, ..
            } => {
                for a in attrs
                    .iter()
                    .filter(|a| a.name == "class" || !rows_of(a).is_empty())
                {
                    reads(&a.value, bound, false, out);
                }
                walk_conditions(children, bound, out);
            }
            Node::Use { children, .. } => walk_conditions(children, bound, out),
            Node::Each {
                var, index, body, ..
            } => {
                let mark = bound.len();
                bound.push(var.clone());
                bound.extend(index.clone());
                walk_conditions(body, bound, out);
                bound.truncate(mark);
            }
            Node::When {
                then, otherwise, ..
            } => {
                walk_conditions(then, bound, out);
                walk_conditions(otherwise, bound, out);
            }
            Node::Match { some, none, .. } => {
                let mark = bound.len();
                bound.push(some.0.clone());
                walk_conditions(&some.1, bound, out);
                bound.truncate(mark);
                walk_conditions(none, bound, out);
            }
            Node::Children { .. } => {}
        }
    }
}
