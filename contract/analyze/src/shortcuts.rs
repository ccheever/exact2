//! A button's `aria-keyshortcuts` against the keys a game's `Actions` bind
//! (LLP 1132 D11). A shown button's shortcut takes its key before the
//! focused world hears it, on every host, so a key both declare reaches the
//! world only while no such button shows.

use crate::{AnalyzeError, Related};
use contract_syntax::{Attr, Expr, File, Node, Span};
use std::collections::BTreeMap;

/// The `KeyboardEvent.code`s a chord as ARIA spells it (`1`, `Shift+A`,
/// `Space`, `Control+Plus`) can arrive as: the physical keys that type its
/// key on a US layout, and the keypad's.
pub fn chord_codes(chord: &str) -> Vec<String> {
    let key = if chord == "+" || chord.ends_with("++") {
        "+"
    } else {
        chord.rsplit('+').next().unwrap_or_default()
    };
    let key = match key {
        "Plus" => "+",
        "Space" => " ",
        k => k,
    };
    let mut chars = key.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        let function = key
            .strip_prefix('F')
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=35).contains(&n));
        return match key {
            "Enter" => vec!["Enter".into(), "NumpadEnter".into()],
            "Tab" | "Escape" | "Backspace" | "Delete" | "Insert" | "ArrowUp" | "ArrowDown"
            | "ArrowLeft" | "ArrowRight" | "Home" | "End" | "PageUp" | "PageDown" => {
                vec![key.into()]
            }
            _ if function => vec![key.into()],
            _ => Vec::new(),
        };
    };
    let shifted = ")!@#$%^&*(";
    match c {
        'a'..='z' | 'A'..='Z' => vec![format!("Key{}", c.to_ascii_uppercase())],
        '0'..='9' => vec![format!("Digit{c}"), format!("Numpad{c}")],
        ' ' => vec!["Space".into()],
        _ if shifted.contains(c) => vec![format!("Digit{}", shifted.find(c).unwrap_or_default())],
        _ => {
            let (code, numpad) = match c {
                '-' | '_' => ("Minus", Some("NumpadSubtract")),
                '=' => ("Equal", None),
                '+' => ("Equal", Some("NumpadAdd")),
                '[' | '{' => ("BracketLeft", None),
                ']' | '}' => ("BracketRight", None),
                '\\' | '|' => ("Backslash", None),
                ';' | ':' => ("Semicolon", None),
                '\'' | '"' => ("Quote", None),
                '`' | '~' => ("Backquote", None),
                ',' | '<' => ("Comma", None),
                '.' | '>' => ("Period", Some("NumpadDecimal")),
                '/' | '?' => ("Slash", Some("NumpadDivide")),
                '*' => ("Digit8", Some("NumpadMultiply")),
                _ => return Vec::new(),
            };
            std::iter::once(code)
                .chain(numpad)
                .map(str::to_owned)
                .collect()
        }
    }
}

/// Every button whose `aria-keyshortcuts` names a key a surface's
/// `Actions` bind: `bound` maps each surface to its keys, each key to the
/// actions it drives. A literal value is checked, a conditional's two
/// literals, and a component prop's literal arguments at each use; a value
/// computed otherwise is not. Warnings, never refusals.
pub fn check_shortcut_keys(
    file: &File,
    bound: &BTreeMap<String, BTreeMap<String, Vec<String>>>,
) -> Vec<AnalyzeError> {
    // Each component prop's arguments: (component, prop) → (using component, value).
    let mut args: BTreeMap<(&str, &str), Vec<(&str, &Expr)>> = BTreeMap::new();
    fn uses<'a>(
        nodes: &'a [Node],
        user: &'a str,
        args: &mut BTreeMap<(&'a str, &'a str), Vec<(&'a str, &'a Expr)>>,
    ) {
        for node in nodes {
            match node {
                Node::Use {
                    name,
                    args: given,
                    children,
                    ..
                } => {
                    for Attr {
                        name: prop, value, ..
                    } in given
                    {
                        args.entry((name, prop)).or_default().push((user, value));
                    }
                    uses(children, user, args);
                }
                Node::Element { children, .. } => uses(children, user, args),
                Node::Each { body, .. } => uses(body, user, args),
                Node::When {
                    then, otherwise, ..
                } => {
                    uses(then, user, args);
                    uses(otherwise, user, args);
                }
                Node::Match { some, none, .. } => {
                    uses(&some.1, user, args);
                    uses(none, user, args);
                }
                Node::Children { .. } => {}
            }
        }
    }
    for component in &file.components {
        uses(&component.view, &component.name, &mut args);
    }
    let props: BTreeMap<&str, Vec<&str>> = file
        .components
        .iter()
        .map(|c| {
            (
                c.name.as_str(),
                c.props.iter().map(|p| p.name.as_str()).collect(),
            )
        })
        .collect();
    // The literal values `value` can take in `component`, each with the
    // span that wrote it.
    fn literals<'a>(
        value: &'a Expr,
        component: &'a str,
        props: &BTreeMap<&str, Vec<&str>>,
        args: &BTreeMap<(&'a str, &'a str), Vec<(&'a str, &'a Expr)>>,
        depth: u8,
        out: &mut Vec<(&'a str, Span)>,
    ) {
        match value {
            Expr::Str(text, span) => out.push((text, *span)),
            Expr::Ternary(_, a, b, _) => {
                literals(a, component, props, args, depth, out);
                literals(b, component, props, args, depth, out);
            }
            Expr::Ident(name, _)
                if depth < 8
                    && props
                        .get(component)
                        .is_some_and(|p| p.contains(&name.as_str())) =>
            {
                for (user, value) in args.get(&(component, name.as_str())).into_iter().flatten() {
                    literals(value, user, props, args, depth + 1, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    let mut walk = |component: &str, nodes: &[Node]| {
        let mut stack: Vec<&Node> = nodes.iter().collect();
        while let Some(node) = stack.pop() {
            match node {
                Node::Element {
                    tag,
                    attrs,
                    children,
                    ..
                } => {
                    stack.extend(children);
                    if tag != "button" {
                        continue;
                    }
                    let own =
                        attrs
                            .iter()
                            .find(|a| a.name == "action")
                            .and_then(|a| match &a.value {
                                Expr::Str(s, _) => Some(s.as_str()),
                                _ => None,
                            });
                    let Some(shortcut) = attrs.iter().find(|a| a.name == "aria-keyshortcuts")
                    else {
                        continue;
                    };
                    let mut values = Vec::new();
                    literals(&shortcut.value, component, &props, &args, 0, &mut values);
                    for (text, written) in values {
                        for chord in text.split_whitespace() {
                            for code in chord_codes(chord) {
                                for (surface, keys) in bound {
                                    let Some(actions) = keys.get(&code) else {
                                        continue;
                                    };
                                    let names = actions
                                        .iter()
                                        .map(|a| format!("`{a}`"))
                                        .collect::<Vec<_>>()
                                        .join(", ");
                                    let same = own.is_some_and(|o| actions.iter().any(|a| a == o));
                                    let fix = if same {
                                        format!("the button's own `action` is one of them, so the key already reaches the world through it while it shows; drop {code} from `Actions` or the shortcut")
                                    } else {
                                        format!("drop {code} from `Actions` or the shortcut, or bind another key")
                                    };
                                    out.push(AnalyzeError {
                                        id: "analyze-shortcut-world-key",
                                        message: format!("`aria-keyshortcuts` \"{chord}\" takes {code} while this button shows, so surface `{surface}` never hears it as {names}: the shortcut presses the button first on every host (LLP 1132 D11); {fix}"),
                                        span: shortcut.span,
                                        related: match shortcut.value {
                                            Expr::Str(..) => Vec::new(),
                                            _ => vec![Related {
                                                span: written,
                                                note: format!("\"{text}\" is passed here"),
                                            }],
                                        },
                                    });
                                }
                            }
                        }
                    }
                }
                Node::Use { children, .. } => stack.extend(children),
                Node::Each { body, .. } => stack.extend(body),
                Node::When {
                    then, otherwise, ..
                } => {
                    stack.extend(then);
                    stack.extend(otherwise);
                }
                Node::Match { some, none, .. } => {
                    stack.extend(&some.1);
                    stack.extend(none);
                }
                Node::Children { .. } => {}
            }
        }
    };
    for component in &file.components {
        walk(&component.name, &component.view);
    }
    out.sort_by_key(|e| (e.span.line, e.span.col));
    out.dedup_by(|a, b| a.span == b.span && a.message == b.message);
    out
}
