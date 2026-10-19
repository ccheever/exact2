//! A button's `aria-keyshortcuts` against the keys a game's `Actions` bind
//! (LLP 1132 D11). A shown button's shortcut takes its chord, its key with
//! exactly its modifiers, before the focused world hears it, on every host,
//! so that press of a key both declare reaches the world only while no such
//! button shows; the key with other modifiers reaches it either way.

use crate::{AnalyzeError, Related};
use contract_syntax::{Attr, Expr, File, Node, Span};
use std::collections::BTreeMap;

/// The modifiers a chord as ARIA spells it holds (`Shift` in `Shift+J`)
/// and the `KeyboardEvent.code`s that type its key with exactly those held,
/// on a US layout: a host matches the chord's key (`KeyboardEvent.key`)
/// with exactly its modifiers, so `*` is the keypad's alone (Digit8 types
/// it only with Shift) and `Control+Plus` likewise. No codes for a chord no
/// host matches.
pub fn chord_codes(chord: &str) -> (Vec<&str>, Vec<String>) {
    let (modifiers, key) = if chord == "+" {
        ("", "+")
    } else if let Some(held) = chord.strip_suffix("++") {
        (held, "+")
    } else {
        chord.rsplit_once('+').unwrap_or(("", chord))
    };
    let modifiers: Vec<&str> = modifiers.split('+').filter(|m| !m.is_empty()).collect();
    if !modifiers
        .iter()
        .all(|m| matches!(*m, "Meta" | "Control" | "Alt" | "Shift"))
    {
        return (modifiers, Vec::new());
    }
    let shift = modifiers.contains(&"Shift");
    (modifiers, key_codes(key, shift))
}

fn key_codes(key: &str, shift: bool) -> Vec<String> {
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
    // Each printable key's main-row code with the Shift state that types
    // it, and the keypad's, which types it either way.
    let (main, shifted, numpad) = match c {
        'a'..='z' | 'A'..='Z' => {
            return vec![format!("Key{}", c.to_ascii_uppercase())];
        }
        ' ' => return vec!["Space".into()],
        '0'..='9' if shift => return Vec::new(),
        '0'..='9' => {
            return vec![format!("Digit{c}"), format!("Numpad{c}")];
        }
        ')' | '!' | '@' | '#' | '$' | '%' | '^' | '&' | '*' | '(' => {
            let digit = ")!@#$%^&*(".find(c).unwrap_or_default();
            let code = format!("Digit{digit}");
            let numpad = (c == '*').then_some("NumpadMultiply");
            return (shift.then_some(code))
                .into_iter()
                .chain(numpad.map(str::to_owned))
                .collect();
        }
        '-' => ("Minus", false, Some("NumpadSubtract")),
        '_' => ("Minus", true, None),
        '=' => ("Equal", false, None),
        '+' => ("Equal", true, Some("NumpadAdd")),
        '[' => ("BracketLeft", false, None),
        '{' => ("BracketLeft", true, None),
        ']' => ("BracketRight", false, None),
        '}' => ("BracketRight", true, None),
        '\\' => ("Backslash", false, None),
        '|' => ("Backslash", true, None),
        ';' => ("Semicolon", false, None),
        ':' => ("Semicolon", true, None),
        '\'' => ("Quote", false, None),
        '"' => ("Quote", true, None),
        '`' => ("Backquote", false, None),
        '~' => ("Backquote", true, None),
        ',' => ("Comma", false, None),
        '<' => ("Comma", true, None),
        '.' => ("Period", false, Some("NumpadDecimal")),
        '>' => ("Period", true, None),
        '/' => ("Slash", false, Some("NumpadDivide")),
        '?' => ("Slash", true, None),
        _ => return Vec::new(),
    };
    (shifted == shift)
        .then_some(main)
        .into_iter()
        .chain(numpad)
        .map(str::to_owned)
        .collect()
}

/// Whether any button in `file` declares `aria-keyshortcuts`: only then
/// does a stale or missing key declaration matter.
pub fn declares_shortcuts(file: &File) -> bool {
    fn any(nodes: &[Node]) -> bool {
        nodes.iter().any(|node| match node {
            Node::Element {
                tag,
                attrs,
                children,
                ..
            } => {
                (tag == "button" && attrs.iter().any(|a| a.name == "aria-keyshortcuts"))
                    || any(children)
            }
            Node::Use { children, .. } => any(children),
            Node::Each { body, .. } => any(body),
            Node::When {
                then, otherwise, ..
            } => any(then) || any(otherwise),
            Node::Match { some, none, .. } => any(&some.1) || any(none),
            Node::Children { .. } => false,
        })
    }
    file.components.iter().any(|c| any(&c.view))
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
                            let (modifiers, codes) = chord_codes(chord);
                            for code in codes {
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
                                        message: {
                                            let held = match modifiers.is_empty() {
                                                true => "no modifier".to_owned(),
                                                false => format!("exactly {}", modifiers.join("+")),
                                            };
                                            format!("`aria-keyshortcuts` \"{chord}\" takes {code} pressed with {held} held: while this button shows, surface `{surface}` does not hear that press as {names}, though {code} with other modifiers still reaches it; the shortcut presses the button first on every host (LLP 1132 D11); {fix}")
                                        },
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
