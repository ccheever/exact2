//! A button's `aria-keyshortcuts` against the keys a game's `Actions` bind
//! (LLP 1132 D11). A shown button's shortcut takes its chord, its key with
//! exactly its modifiers, before the focused world hears it, on every host,
//! so that press of a key both declare reaches the world only while no such
//! button shows; the key with other modifiers reaches it either way. And
//! the chords that never reach an app on Windows (LLP 1115.004 D9.6).

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
    let (modifiers, key) = split_chord(chord);
    if !modifiers
        .iter()
        .all(|m| matches!(*m, "Meta" | "Control" | "Alt" | "Shift"))
    {
        return (modifiers, Vec::new());
    }
    let shift = modifiers.contains(&"Shift");
    (modifiers, key_codes(key, shift))
}

/// A chord as ARIA spells it split into its modifiers and its key, `+` for
/// the plus key spelled `+` (`Control++`); `Plus` stays as written.
fn split_chord(chord: &str) -> (Vec<&str>, &str) {
    let (modifiers, key) = if chord == "+" {
        ("", "+")
    } else if let Some(held) = chord.strip_suffix("++") {
        (held, "+")
    } else {
        chord.rsplit_once('+').unwrap_or(("", chord))
    };
    (
        modifiers.split('+').filter(|m| !m.is_empty()).collect(),
        key,
    )
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

/// A button's `aria-keyshortcuts`: the button's attributes, the attribute,
/// and each literal value it can take with the span that wrote it.
struct ButtonShortcut<'a> {
    attrs: &'a [Attr],
    shortcut: &'a Attr,
    values: Vec<(&'a str, Span)>,
}

impl ButtonShortcut<'_> {
    /// Where a value passed through a component prop was written.
    fn related(&self, text: &str, written: Span) -> Vec<Related> {
        match self.shortcut.value {
            Expr::Str(..) => Vec::new(),
            _ => vec![Related {
                span: written,
                note: format!("\"{text}\" is passed here"),
            }],
        }
    }
}

/// Every button's `aria-keyshortcuts` in `file`, with the values it can
/// take: a literal, a conditional's two literals, and a component prop's
/// literal arguments at each use; a value computed otherwise has none.
fn button_shortcuts(file: &File) -> Vec<ButtonShortcut<'_>> {
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
    for component in &file.components {
        let mut stack: Vec<&Node> = component.view.iter().collect();
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
                    let Some(shortcut) = attrs.iter().find(|a| a.name == "aria-keyshortcuts")
                    else {
                        continue;
                    };
                    let mut values = Vec::new();
                    literals(
                        &shortcut.value,
                        &component.name,
                        &props,
                        &args,
                        0,
                        &mut values,
                    );
                    out.push(ButtonShortcut {
                        attrs,
                        shortcut,
                        values,
                    });
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
    }
    out
}

/// Every button whose `aria-keyshortcuts` names a key a surface's
/// `Actions` bind: `bound` maps each surface to its keys, each key to the
/// actions it drives. Values are found as [`button_shortcuts`] finds them.
/// Warnings, never refusals.
pub fn check_shortcut_keys(
    file: &File,
    bound: &BTreeMap<String, BTreeMap<String, Vec<String>>>,
) -> Vec<AnalyzeError> {
    let mut out = Vec::new();
    for button in button_shortcuts(file) {
        let own = button
            .attrs
            .iter()
            .find(|a| a.name == "action")
            .and_then(|a| match &a.value {
                Expr::Str(s, _) => Some(s.as_str()),
                _ => None,
            });
        for &(text, written) in &button.values {
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
                            format!(
                                "drop {code} from `Actions` or the shortcut, or bind another key"
                            )
                        };
                        let held = match modifiers.is_empty() {
                            true => "no modifier".to_owned(),
                            false => format!("exactly {}", modifiers.join("+")),
                        };
                        out.push(AnalyzeError {
                            id: "analyze-shortcut-world-key",
                            message: format!("`aria-keyshortcuts` \"{chord}\" takes {code} pressed with {held} held: while this button shows, surface `{surface}` does not hear that press as {names}, though {code} with other modifiers still reaches it; the shortcut presses the button first on every host (LLP 1132 D11); {fix}"),
                            span: button.shortcut.span,
                            related: button.related(text, written),
                        });
                    }
                }
            }
        }
    }
    out.sort_by_key(|e| (e.span.line, e.span.col));
    out.dedup_by(|a, b| a.span == b.span && a.message == b.message);
    out
}

/// What Windows does with a chord holding no Windows key (`Meta`) instead
/// of giving it to the app, for the chords it keeps; `key` as
/// [`split_chord`] gives it.
fn windows_keeps(modifiers: &[&str], key: &str) -> Option<&'static str> {
    let held = |m: &str| modifiers.contains(&m);
    let only = |ms: &[&str]| modifiers.len() == ms.len() && ms.iter().all(|m| held(m));
    match key.to_ascii_lowercase().as_str() {
        "tab" if held("Alt") => Some("switching windows"),
        "escape" if only(&["Control", "Shift"]) => Some("opening Task Manager"),
        "escape" if only(&["Control"]) => Some("opening Start"),
        "delete" if only(&["Control", "Alt"]) => Some("its security screen"),
        "f4" if only(&["Alt"]) => Some("closing the window"),
        "space" if only(&["Alt"]) => Some("the window's system menu"),
        _ => None,
    }
}

/// The chord with `Control` held where it holds `Meta`: its key and other
/// modifiers kept, and `Meta` only dropped where `Control` is held too.
fn control_sibling(modifiers: &[&str], key: &str) -> String {
    let mut chord: Vec<&str> = Vec::new();
    for m in modifiers {
        let m = if *m == "Meta" { "Control" } else { m };
        if !chord.contains(&m) {
            chord.push(m);
        }
    }
    chord.push(if key == "+" { "Plus" } else { key });
    chord.join("+")
}

/// Whether two chords are one: the same modifiers in any order and the
/// key compared without case, as the hosts match it.
fn same_chord(a: &str, b: &str) -> bool {
    let ((a_held, a_key), (b_held, b_key)) = (split_chord(a), split_chord(b));
    let key = |k: &str| match k {
        "+" => "plus".to_owned(),
        k => k.to_ascii_lowercase(),
    };
    a_held.len() == b_held.len()
        && a_held.iter().all(|m| b_held.contains(m))
        && key(a_key) == key(b_key)
}

/// Every button `aria-keyshortcuts` chord that never presses its button on
/// Windows (LLP 1115.004 D9.6), for an app with a Windows host. Each host
/// matches a chord as written, and on Windows `Meta` is the Windows key,
/// whose chords the shell takes first; so a `Meta` chord whose list holds
/// no `Control` sibling is `analyze-shortcut-meta-only`, naming the sibling
/// to add, and a chord Windows keeps for itself without the Windows key
/// (Alt+Tab, Alt+F4, Control+Escape…) is `analyze-shortcut-windows-reserved`.
/// Values are found as [`button_shortcuts`] finds them. Warnings, never
/// refusals.
pub fn check_windows_shortcuts(file: &File) -> Vec<AnalyzeError> {
    let mut out = Vec::new();
    for button in button_shortcuts(file) {
        for &(text, written) in &button.values {
            let chords: Vec<&str> = text.split_whitespace().collect();
            for &chord in &chords {
                let (modifiers, key) = split_chord(chord);
                if !modifiers
                    .iter()
                    .all(|m| matches!(*m, "Meta" | "Control" | "Alt" | "Shift"))
                {
                    continue;
                }
                let (id, message) = if modifiers.contains(&"Meta") {
                    let sibling = control_sibling(&modifiers, key);
                    if chords.iter().any(|c| same_chord(c, &sibling)) {
                        continue;
                    }
                    let (sibling_held, sibling_key) = split_chord(&sibling);
                    let advice = match windows_keeps(&sibling_held, sibling_key) {
                        Some(kept) => format!("its Control chord, \"{sibling}\", Windows keeps for {kept}, so add another"),
                        None => {
                            let list = chords
                                .iter()
                                .flat_map(|&c| match c == chord {
                                    true => vec![c, sibling.as_str()],
                                    false => vec![c],
                                })
                                .collect::<Vec<_>>()
                                .join(" ");
                            format!("add \"{sibling}\" beside it: \"{list}\"")
                        }
                    };
                    (
                        "analyze-shortcut-meta-only",
                        format!("`aria-keyshortcuts` \"{chord}\" has no Control chord beside it: on Windows `Meta` is the Windows key, whose chords the shell takes before the app hears them, and a host matches a chord as written, so this shortcut never fires there; {advice} (LLP 1115.004 D9.6)"),
                    )
                } else if let Some(kept) = windows_keeps(&modifiers, key) {
                    (
                        "analyze-shortcut-windows-reserved",
                        format!("`aria-keyshortcuts` \"{chord}\" is a chord Windows keeps for {kept}, so on Windows it never presses this button; choose another (LLP 1115.004 D9.6)"),
                    )
                } else {
                    continue;
                };
                out.push(AnalyzeError {
                    id,
                    message,
                    span: button.shortcut.span,
                    related: button.related(text, written),
                });
            }
        }
    }
    out.sort_by_key(|e| (e.span.line, e.span.col));
    out.dedup_by(|a, b| a.span == b.span && a.message == b.message);
    out
}
