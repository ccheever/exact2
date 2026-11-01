//! The driver's names for keys, the web's `code` and `key` (`type … key`,
//! LLP 1012 §1): a chord's modifiers and the key it ends with.

const LETTERS: [&str; 26] = [
    "KeyA", "KeyB", "KeyC", "KeyD", "KeyE", "KeyF", "KeyG", "KeyH", "KeyI", "KeyJ", "KeyK", "KeyL",
    "KeyM", "KeyN", "KeyO", "KeyP", "KeyQ", "KeyR", "KeyS", "KeyT", "KeyU", "KeyV", "KeyW", "KeyX",
    "KeyY", "KeyZ",
];
const LOWER: [&str; 26] = [
    "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s",
    "t", "u", "v", "w", "x", "y", "z",
];
const UPPER: [&str; 26] = [
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z",
];
const DIGITS: [&str; 10] = [
    "Digit0", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8",
    "Digit9",
];
const DIGIT_KEYS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
const FKEYS: [&str; 24] = [
    "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "F13", "F14", "F15",
    "F16", "F17", "F18", "F19", "F20", "F21", "F22", "F23", "F24",
];

/// A chord's modifiers in the order it names them (`Shift+Control+x` is
/// Shift, then Control), as `(code, key)`; a modifier alone is the key.
pub(super) fn chord_modifiers(chord: &str) -> Vec<(&'static str, &'static str)> {
    const PREFIXES: [(&str, (&str, &str)); 4] = [
        ("Shift+", ("ShiftLeft", "Shift")),
        ("Control+", ("ControlLeft", "Control")),
        ("Alt+", ("AltLeft", "Alt")),
        ("Meta+", ("MetaLeft", "Meta")),
    ];
    let (mut held, mut rest) = (Vec::new(), chord);
    while let Some((prefix, modifier)) = PREFIXES
        .iter()
        .find(|(prefix, _)| rest.len() > prefix.len() && rest.starts_with(prefix))
    {
        held.push(*modifier);
        rest = &rest[prefix.len()..];
    }
    held
}

/// A driver's key as `(KeyboardEvent.code, KeyboardEvent.key)`, the web
/// `cdpKey` vocabulary: `p` and `KeyP` are one key, `7` and `Digit7` too,
/// and `End` stays a name. `None` refuses an unknown name instead of
/// delivering it as text (notes mac-agent-named-keys, platformer canvas-keys).
pub(super) fn driver_key(name: &str) -> Option<(&'static str, &'static str)> {
    if let Some(rest) = name.strip_prefix('F') {
        if let Ok(n) = rest.parse::<usize>() {
            if (1..=24).contains(&n) && rest.len() == n.to_string().len() {
                return Some((FKEYS[n - 1], FKEYS[n - 1]));
            }
        }
    }
    if let Some(rest) = name.strip_prefix("Key") {
        if rest.len() == 1 {
            if let Some(index) = letter_index(rest) {
                return Some((LETTERS[index], LOWER[index]));
            }
        }
    }
    if let Some(rest) = name.strip_prefix("Digit") {
        if rest.len() == 1 {
            if let Some(index) = digit_index(rest) {
                return Some((DIGITS[index], DIGIT_KEYS[index]));
            }
        }
    }
    if name.len() == 1 {
        let ch = name.chars().next()?;
        if let Some(index) = (ch.is_ascii_alphabetic())
            .then(|| letter_index(name))
            .flatten()
        {
            let key = if ch.is_ascii_uppercase() {
                UPPER[index]
            } else {
                LOWER[index]
            };
            return Some((LETTERS[index], key));
        }
        if let Some(index) = digit_index(name) {
            return Some((DIGITS[index], DIGIT_KEYS[index]));
        }
        return match ch {
            ' ' => Some(("Space", " ")),
            '-' | '_' => Some(("Minus", if ch == '-' { "-" } else { "_" })),
            '=' | '+' => Some(("Equal", if ch == '=' { "=" } else { "+" })),
            '[' | '{' => Some(("BracketLeft", if ch == '[' { "[" } else { "{" })),
            ']' | '}' => Some(("BracketRight", if ch == ']' { "]" } else { "}" })),
            '\\' | '|' => Some(("Backslash", if ch == '\\' { "\\" } else { "|" })),
            ';' | ':' => Some(("Semicolon", if ch == ';' { ";" } else { ":" })),
            '\'' | '"' => Some(("Quote", if ch == '\'' { "'" } else { "\"" })),
            '`' | '~' => Some(("Backquote", if ch == '`' { "`" } else { "~" })),
            ',' | '<' => Some(("Comma", if ch == ',' { "," } else { "<" })),
            '.' | '>' => Some(("Period", if ch == '.' { "." } else { ">" })),
            '/' | '?' => Some(("Slash", if ch == '/' { "/" } else { "?" })),
            '!' => Some(("Digit1", "!")),
            '@' => Some(("Digit2", "@")),
            '#' => Some(("Digit3", "#")),
            '$' => Some(("Digit4", "$")),
            '%' => Some(("Digit5", "%")),
            '^' => Some(("Digit6", "^")),
            '&' => Some(("Digit7", "&")),
            '*' => Some(("Digit8", "*")),
            '(' => Some(("Digit9", "(")),
            ')' => Some(("Digit0", ")")),
            _ => None,
        };
    }
    Some(match name {
        "Space" => ("Space", " "),
        "Enter" => ("Enter", "Enter"),
        "NumpadEnter" => ("NumpadEnter", "Enter"),
        "Escape" => ("Escape", "Escape"),
        "Tab" => ("Tab", "Tab"),
        "Backspace" => ("Backspace", "Backspace"),
        "Delete" => ("Delete", "Delete"),
        "Insert" => ("Insert", "Insert"),
        "Home" => ("Home", "Home"),
        "End" => ("End", "End"),
        "PageUp" => ("PageUp", "PageUp"),
        "PageDown" => ("PageDown", "PageDown"),
        "ArrowUp" => ("ArrowUp", "ArrowUp"),
        "ArrowDown" => ("ArrowDown", "ArrowDown"),
        "ArrowLeft" => ("ArrowLeft", "ArrowLeft"),
        "ArrowRight" => ("ArrowRight", "ArrowRight"),
        "CapsLock" => ("CapsLock", "CapsLock"),
        "Shift" | "ShiftLeft" => ("ShiftLeft", "Shift"),
        "ShiftRight" => ("ShiftRight", "Shift"),
        "Control" | "ControlLeft" => ("ControlLeft", "Control"),
        "ControlRight" => ("ControlRight", "Control"),
        "Alt" | "AltLeft" => ("AltLeft", "Alt"),
        "AltRight" => ("AltRight", "Alt"),
        "Meta" | "MetaLeft" => ("MetaLeft", "Meta"),
        "MetaRight" => ("MetaRight", "Meta"),
        _ => return None,
    })
}

fn letter_index(name: &str) -> Option<usize> {
    let ch = name.chars().next()?;
    ch.is_ascii_alphabetic()
        .then(|| (ch.to_ascii_uppercase() as usize) - ('A' as usize))
}

fn digit_index(name: &str) -> Option<usize> {
    let ch = name.chars().next()?;
    ch.is_ascii_digit().then(|| (ch as usize) - ('0' as usize))
}
