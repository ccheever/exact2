//! The string literals of a data-module source, which is where a grant line
//! is written, and the keys each `device.<name> <key>` in them names. A
//! warning fires only on what is certain, so every form this does not model
//! makes the answer unknown rather than a guess: a regex literal, a Rust raw
//! string, an unterminated literal or comment, and a grant line joined to
//! more text (`+`, a template's `${…}`) or ending without its key.

/// A string literal as written (escapes kept), and whether more text is
/// joined to its end: `+ …` after it, or a template's `${…}`.
#[derive(Debug, PartialEq)]
pub(super) struct Lit {
    pub text: String,
    pub joined: bool,
}

/// Words after which a `/` begins a regex literal, not a division.
const BEFORE_REGEX: &[&str] = &[
    "return", "typeof", "case", "do", "else", "in", "of", "new", "delete", "void", "throw",
    "yield", "await",
];

/// The string literals of `text`, TypeScript when `typescript` and Rust
/// otherwise, with comments left out; `None` when it holds a form this does
/// not model.
pub(super) fn literals(text: &str, typescript: bool) -> Option<Vec<Lit>> {
    let s: Vec<char> = text.chars().collect();
    let n = s.len();
    let ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let mut lits = Vec::new();
    // Each template `${` open: the depth of braces inside it.
    let mut holes: Vec<usize> = Vec::new();
    // The last significant character of code, and the word it ends.
    let (mut last, mut word) = (None::<char>, String::new());
    let mut i = 0;
    // `+` after index `j` (past white space) joins the literal before it.
    let joined_at = |j: usize| s[j..].iter().find(|c| !c.is_whitespace()) == Some(&'+');
    // A quoted literal from `i` (its quote) to past its close.
    let quoted = |i: &mut usize| -> Option<String> {
        let q = s[*i];
        let start = *i + 1;
        *i += 1;
        while *i < n && s[*i] != q {
            if s[*i] == '\\' {
                *i += 1;
            } else if s[*i] == '\n' && typescript {
                return None;
            }
            *i += 1;
        }
        if *i >= n {
            return None;
        }
        *i += 1;
        Some(s[start..*i - 1].iter().collect())
    };
    // A template's text from `i` to its close or its next `${`; whether it closed.
    let template = |i: &mut usize, lits: &mut Vec<Lit>| -> Option<bool> {
        let start = *i;
        while *i < n && s[*i] != '`' && !(s[*i] == '$' && s.get(*i + 1) == Some(&'{')) {
            *i += if s[*i] == '\\' { 2 } else { 1 };
        }
        if *i >= n {
            return None;
        }
        let text: String = s[start..*i].iter().collect();
        if s[*i] == '`' {
            *i += 1;
            lits.push(Lit {
                text,
                joined: joined_at(*i),
            });
            Some(true)
        } else {
            *i += 2;
            lits.push(Lit { text, joined: true });
            Some(false)
        }
    };
    while i < n {
        let c = s[i];
        let next = s.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < n && s[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && next == Some('*') {
            // Rust's block comments nest; TypeScript's do not.
            let mut depth = 0;
            loop {
                if i + 1 >= n {
                    return None;
                }
                if s[i] == '/' && s[i + 1] == '*' && (depth == 0 || !typescript) {
                    depth += 1;
                    i += 2;
                } else if s[i] == '*' && s[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            continue;
        }
        let regex = typescript
            && c == '/'
            && (last.is_none_or(|l| "(,=:[!&|?{};+-*%<>~^}".contains(l))
                || BEFORE_REGEX.contains(&word.as_str()));
        if regex {
            return None;
        }
        // A raw string (`r"…"`, `r#"…"#`, `br"…"`), or a raw identifier.
        let raw_at = |j: usize| matches!(s.get(j), Some('"' | '#'));
        let after_ident = i > 0 && ident(s[i - 1]);
        if !typescript
            && !after_ident
            && ((c == 'r' && raw_at(i + 1)) || (c == 'b' && next == Some('r') && raw_at(i + 2)))
        {
            return None;
        }
        match c {
            '"' => {
                let text = quoted(&mut i)?;
                lits.push(Lit {
                    text,
                    joined: joined_at(i),
                });
                (last, word) = (Some('"'), String::new());
                continue;
            }
            '\'' if typescript => {
                let text = quoted(&mut i)?;
                lits.push(Lit {
                    text,
                    joined: joined_at(i),
                });
                (last, word) = (Some('"'), String::new());
                continue;
            }
            // A Rust character (`'"'`, `'\''`), else a lifetime.
            '\'' => {
                if next == Some('\\') {
                    // Past the escaped character, which may itself be `'`.
                    let close = s.get(i + 3..)?.iter().take(12).position(|c| *c == '\'')?;
                    i += close + 4;
                } else if s.get(i + 2) == Some(&'\'') {
                    i += 3;
                } else {
                    i += 1;
                }
                (last, word) = (Some('\''), String::new());
                continue;
            }
            '`' if typescript => {
                i += 1;
                if !template(&mut i, &mut lits)? {
                    holes.push(0);
                }
                (last, word) = (Some('"'), String::new());
                continue;
            }
            '{' => {
                if let Some(depth) = holes.last_mut() {
                    *depth += 1;
                }
            }
            '}' if holes.last() == Some(&0) => {
                holes.pop();
                i += 1;
                if !template(&mut i, &mut lits)? {
                    holes.push(0);
                }
                (last, word) = (Some('"'), String::new());
                continue;
            }
            '}' => {
                if let Some(depth) = holes.last_mut() {
                    *depth -= 1;
                }
            }
            _ => {}
        }
        if !c.is_whitespace() {
            if ident(c) {
                if !last.is_some_and(ident) {
                    word.clear();
                }
                word.push(c);
            } else {
                word.clear();
            }
            last = Some(c);
        }
        i += 1;
    }
    holes.is_empty().then_some(lits)
}

/// The key each `device.<name> <key>` in `lits` names, or `None` when one is
/// built otherwise (joined to more text, its key missing or cut short).
pub(super) fn keys(lits: &[Lit], name: &str) -> Option<Vec<String>> {
    let key_char = |c: char| c.is_alphanumeric() || matches!(c, '.' | '-' | '_');
    let mut keys = Vec::new();
    for lit in lits {
        let t = &lit.text;
        for (at, _) in t.match_indices("device.") {
            // A grant line begins the literal, or follows white space or an escape (`\n`).
            let head = &t[..at];
            let begins = head.is_empty()
                || head.ends_with(char::is_whitespace)
                || head.ends_with("\\n")
                || head.ends_with("\\t");
            if !begins {
                continue;
            }
            let rest = &t[at + "device.".len()..];
            let device: String = rest.chars().take_while(|c| key_char(*c)).collect();
            let after = &rest[device.len()..];
            if after.is_empty() && lit.joined {
                return None;
            }
            if device != name {
                continue;
            }
            let spelled = after
                .strip_prefix([' ', '\t'])?
                .trim_start_matches([' ', '\t']);
            let key: String = spelled.chars().take_while(|c| key_char(*c)).collect();
            let end = &spelled[key.len()..];
            if key.is_empty() || key.ends_with('.') || (end.is_empty() && lit.joined) {
                return None;
            }
            match end.chars().next() {
                None => keys.push(key),
                Some(c) if c.is_whitespace() || c == '\\' => keys.push(key),
                Some(_) => return None,
            }
        }
    }
    Some(keys)
}

#[cfg(test)]
mod tests {
    use super::{keys, literals};

    fn read(source: &str, typescript: bool) -> Option<Vec<String>> {
        keys(&literals(source, typescript)?, "midi")
    }

    fn key(k: &str) -> Option<Vec<String>> {
        Some(vec![k.to_string()])
    }

    #[test]
    fn a_written_grant_is_its_key() {
        assert_eq!(
            read("export const grants = 'device.midi purpose.midi';", true),
            key("purpose.midi")
        );
        assert_eq!(
            read(
                "const g = 'net.fetch https://x/\\ndevice.midi purpose.midi\\nsecret.keep k';",
                true
            ),
            key("purpose.midi")
        );
        assert_eq!(
            read(
                "fn grants(&self) -> &str { \"device.midi purpose.midi\" }",
                false
            ),
            key("purpose.midi")
        );
        // The scaffold's template with a substitution, a division and a
        // Rust lifetime and character are modelled.
        let scaffold = "export const grants = 'device.midi purpose.midi';\nconst s = { greeting: ([name]) => ({ text: `Hello from ${name}.` }) };\nconst half = total / 2;";
        assert_eq!(read(scaffold, true), key("purpose.midi"));
        assert_eq!(
            read("fn f<'a>(x: &'a str) -> char { let q = '\"'; let e = '\\''; q }\nconst G: &str = \"device.midi purpose.midi\";", false),
            key("purpose.midi")
        );
        // A nested template is followed to its end.
        assert_eq!(
            read("const a = `x ${`y ${1}` + '/*'} z`;\nexport const grants = 'device.midi purpose.midi';", true),
            key("purpose.midi")
        );
    }

    #[test]
    fn a_built_grant_is_unknown() {
        for built in [
            "export const grants = `device.midi ${PURPOSE}`;",
            "export const grants = ['device.midi', 'purpose.midi'].join(' ');",
            "export const grants = 'device.midi ' + key;",
            "export const grants = `device.midi purpose.${name}`;",
            "export const grants = \"device.midi purpose.\" + \"midi\";",
            "export const grants = 'device.mi' + 'di purpose.midi';",
        ] {
            assert_eq!(read(built, true), None, "{built}");
        }
        assert_eq!(
            read(
                "const G: &str = concat!(\"device.midi purpose.\", \"midi\");",
                false
            ),
            None
        );
        assert_eq!(
            read("let g = format!(\"device.midi {}\", key);", false),
            None
        );
    }

    #[test]
    fn a_form_not_modelled_is_unknown() {
        for (source, typescript) in [
            (
                "const r = /[/*]/; export const grants = 'device.midi purpose.midi'; // */",
                true,
            ),
            (
                "const r = s.match(/[^/*]+/); export const grants = 'device.midi purpose.midi';",
                true,
            ),
            (
                "const R: &str = r#\"a /* b\"#; const G: &str = \"device.midi purpose.midi\";",
                false,
            ),
            (
                "const R: &str = r\"/*\"; const G: &str = \"device.midi purpose.midi\";",
                false,
            ),
            ("const s = 'unterminated", true),
            ("/* never closed", true),
        ] {
            assert_eq!(read(source, typescript), None, "{source}");
        }
    }

    #[test]
    fn comments_and_other_words_are_no_grant() {
        let none = Some(Vec::new());
        assert_eq!(
            read(
                "// device.midi grants instrument access\nexport const grants = '';",
                true
            ),
            none
        );
        assert_eq!(
            read(
                "/* device.midi purpose.midi */ export const grants = '';",
                true
            ),
            none
        );
        assert_eq!(
            read(
                "//! It grants device.midi purpose.midi\nfn grants() -> &'static str { \"\" }",
                false
            ),
            none
        );
        assert_eq!(
            read(
                "/* a /* nested */ device.midi purpose.midi */ fn f() {}",
                false
            ),
            none
        );
        assert_eq!(
            read(
                "const a = 'mydevice.midi x'; const b = 'device.midis x';",
                true
            ),
            none
        );
        assert_eq!(read("const p = 'Plug in the device.';", true), none);
        // A `//` in a string is no comment.
        assert_eq!(
            read(
                "const g = 'net.fetch https://x/ device.midi purpose.midi';",
                true
            ),
            key("purpose.midi")
        );
    }
}
