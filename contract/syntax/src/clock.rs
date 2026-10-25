//! `animation-timeline=Name` naming a `timeline` declaration (LLP 1055.002
//! D2): the attribute becomes the row's text, `"-exact-clock(Name)"`, before types
//! and lowering see it, so neither knows clock timelines exist.

use crate::ast::{Attr, Expr, File, Node};
use crate::parser::SyntaxError;
use std::collections::{HashMap, HashSet};

/// Rewrite every `animation-timeline` (or `animationTimeline`) whose value
/// is a bare name a `timeline` declares into `"-exact-clock(Name)"`, on view
/// elements and in `style` declarations. On an element any other value is
/// left as written (a bare name there may be a binding, and a component's
/// prop, inject, state, derive, resource, mutation, action, or a `for` or
/// `match` binder of the name shadows the timeline as it shadows any name);
/// in a style, where only a timeline's name is admitted, one no `timeline`
/// declares is refused. A quoted declared name, `animation-timeline="Name"`,
/// is refused with the unquoted spelling, unless it is a CSS keyword or a
/// local takes the name: as a string it would be a CSS timeline's name, and
/// CSS's are `--dashed`.
pub fn resolve_clock_timelines(file: &mut File) -> Result<(), SyntaxError> {
    let names = file
        .timelines
        .iter()
        .map(|t| (t.name.clone(), t.name.clone()))
        .collect();
    resolve_clock_timelines_in(file, &names)
}

/// [`resolve_clock_timelines`] through a file's scope (LLP 1091 D5): `names`
/// maps each timeline name the file may write, its own or one its `use`
/// lines bring, to the program-unique name of the declaration it means.
pub fn resolve_clock_timelines_in(
    file: &mut File,
    names: &HashMap<String, String>,
) -> Result<(), SyntaxError> {
    for style in &mut file.styles {
        attrs(names, &HashSet::new(), &mut style.attrs)?;
        if let Some(a) = style
            .attrs
            .iter()
            .find(|a| matches!(a.value, Expr::Ident(..)))
        {
            let Expr::Ident(name, _) = &a.value else {
                unreachable!()
            };
            return Err(SyntaxError {
                id: "contract-timeline-unknown",
                message: format!(
                    "`{}={name}` in `style {}`: no `timeline {name}` is declared or used",
                    a.name, style.name
                ),
                span: a.span,
            });
        }
    }
    if names.is_empty() {
        return Ok(());
    }
    for component in &mut file.components {
        let c = &*component;
        let locals = (c.props.iter().chain(&c.injects).map(|p| &p.name))
            .chain(c.states.iter().chain(&c.derives).map(|b| &b.name))
            .chain(c.resources.iter().map(|r| &r.name))
            .chain(c.mutations.iter().map(|m| &m.name))
            .chain(c.actions.iter().map(|a| &a.name))
            // Members its other files declare (LLP 1091.002 D5).
            .chain(c.members.outer.iter().map(|(n, _)| n))
            .filter(|n| names.contains_key(*n))
            .cloned()
            .collect();
        nodes(names, &locals, &mut component.view)?;
    }
    Ok(())
}

/// `animation-timeline`'s keywords, its own and CSS's for every property.
const CSS_KEYWORDS: &[&str] = &[
    "auto",
    "none",
    "inherit",
    "initial",
    "unset",
    "revert",
    "revert-layer",
];

fn is_timeline(a: &Attr) -> bool {
    a.name == "animation-timeline" || a.name == "animationTimeline"
}

fn attrs(
    names: &HashMap<String, String>,
    locals: &HashSet<String>,
    attrs: &mut [Attr],
) -> Result<(), SyntaxError> {
    for a in attrs {
        if !is_timeline(a) {
            continue;
        }
        match &a.value {
            Expr::Ident(name, span) => {
                if let Some(to) = names.get(name).filter(|_| !locals.contains(name)) {
                    a.value = Expr::Str(format!("-exact-clock({to})"), *span);
                }
            }
            // CSS's own keywords stay CSS's, whatever a `timeline` is named,
            // and where a local takes the name, the unquoted spelling is it.
            Expr::Str(text, span)
                if names.contains_key(text.trim())
                    && !locals.contains(text.trim())
                    && !CSS_KEYWORDS.contains(&text.trim().to_ascii_lowercase().as_str()) =>
            {
                let name = text.trim();
                return Err(SyntaxError {
                    id: "contract-timeline-quoted",
                    message: format!(
                        "`{}=\"{name}\"` is a string, not the declared `timeline {name}`: write it unquoted, `{}={name}`",
                        a.name, a.name
                    ),
                    span: *span,
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn nodes(
    names: &HashMap<String, String>,
    locals: &HashSet<String>,
    list: &mut [Node],
) -> Result<(), SyntaxError> {
    for node in list {
        match node {
            Node::Element {
                attrs: a, children, ..
            } => {
                attrs(names, locals, a)?;
                nodes(names, locals, children)?;
            }
            Node::Use { children, .. } => nodes(names, locals, children)?,
            Node::Children { .. } => {}
            Node::When {
                then, otherwise, ..
            } => {
                nodes(names, locals, then)?;
                nodes(names, locals, otherwise)?;
            }
            Node::Each {
                var, index, body, ..
            } => {
                let mut inner = locals.clone();
                inner.extend(std::iter::once(&*var).chain(index.as_ref()).cloned());
                nodes(names, &inner, body)?;
            }
            Node::Match { some, none, .. } => {
                let mut inner = locals.clone();
                inner.insert(some.0.clone());
                nodes(names, &inner, &mut some.1)?;
                nodes(names, locals, none)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timelines(src: &str) -> Vec<String> {
        fn walk(list: &[Node], out: &mut Vec<String>) {
            for node in list {
                match node {
                    Node::Element {
                        attrs, children, ..
                    } => {
                        for a in attrs.iter().filter(|a| a.name == "animation-timeline") {
                            out.push(match &a.value {
                                Expr::Str(s, _) => s.clone(),
                                Expr::Ident(n, _) => format!("ident {n}"),
                                _ => "other".into(),
                            });
                        }
                        walk(children, out);
                    }
                    Node::Each { body, .. } => walk(body, out),
                    Node::Match { some, none, .. } => {
                        walk(&some.1, out);
                        walk(none, out);
                    }
                    _ => {}
                }
            }
        }
        let mut file = crate::parse(src).unwrap();
        resolve_clock_timelines(&mut file).unwrap();
        let mut out = Vec::new();
        walk(&file.components[0].view, &mut out);
        out
    }

    #[test]
    fn a_binder_of_the_name_shadows_the_timeline_in_its_body_only() {
        let src = "timeline P\ncomponent App\n  state items = []\n  view\n    column\n      text \"a\" animation-timeline=P\n      for P in items key=P\n        text \"b\" animation-timeline=P\n      text \"c\" animation-timeline=P\n";
        assert_eq!(
            timelines(src),
            ["-exact-clock(P)", "ident P", "-exact-clock(P)"]
        );
    }

    #[test]
    fn a_quoted_declared_name_is_refused_with_its_unquoted_spelling() {
        for src in [
            "timeline P\ncomponent App\n  view\n    text \"a\" animation-timeline=\"P\"\n",
            "timeline P\nstyle s\n  animation-timeline=\" P\"\ncomponent App\n  view\n    text \"a\"\n",
        ] {
            let mut file = crate::parse(src).unwrap();
            let e = resolve_clock_timelines(&mut file).unwrap_err();
            assert_eq!(e.id, "contract-timeline-quoted", "{src}");
            assert!(e.message.contains("`animation-timeline=P`"), "{}", e.message);
        }
        let src = "timeline P\ncomponent App\n  view\n    text \"a\" animation-timeline=\"--p\"\n";
        assert_eq!(timelines(src), ["--p"]);
        // A timeline named like a CSS keyword leaves the keyword CSS's.
        for word in ["auto", "unset"] {
            let src = format!(
                "timeline {word}\ncomponent App\n  view\n    text \"a\" animation-timeline=\"{word}\"\n"
            );
            assert_eq!(timelines(&src), [word]);
        }
        // Where a binder takes the name, it is left for lowering to refuse as CSS.
        let src = "timeline P\ncomponent App\n  state items = []\n  view\n    for P in items key=P\n      text \"b\" animation-timeline=\"P\"\n";
        assert_eq!(timelines(src), ["P"]);
    }
}
