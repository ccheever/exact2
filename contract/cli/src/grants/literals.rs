//! The string literals of a data-module source, which is where a grant line
//! is written, and the keys each `device.<name> <key>` in them names. A
//! warning fires only on what is certain, so every form this does not model
//! makes the answer unknown rather than a guess: a regex literal that does not
//! close on its line, a Rust raw string, an unterminated literal or comment,
//! and a grant line joined to
//! more text (`+`, a literal beside it on the same statement, a template's
//! `${…}`, a Rust `concat!` argument or a `format!` string with a
//! placeholder) or ending without its key.

/// A string literal as written (escapes kept), and whether more text is
/// joined to its end: `+ …` or another literal after it (past a comment; in
/// TypeScript a literal after a line break is the next statement), a
/// template's `${…}`, or a Rust macro that builds a string of it.
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

/// Words whose parenthesised condition a statement follows: a `/` after its
/// `)` begins a regex (`if (x) /re/.test(s)`), where one after any other `)`
/// divides.
const CONTROL: &[&str] = &["if", "while", "for", "with"];

/// How a Rust macro joins the literals among its arguments.
#[derive(Clone, Copy, PartialEq)]
enum Join {
    No,
    /// `concat!`: every literal is a piece of one string.
    All,
    /// `format!`: its format string, still to come, is extended by a
    /// placeholder; a plain literal argument is as written.
    Format,
}

/// What a `(` opened.
struct Paren {
    /// A control word's condition.
    control: bool,
    joins: Join,
}

/// Past the regex literal opening at `i` and its flags (ECMA-262 §12.9.5:
/// an escape takes the next character, and a `/` inside a class `[…]` does
/// not close it), or `None` when it does not close on its line.
fn regex_end(s: &[char], mut i: usize) -> Option<usize> {
    let mut class = false;
    i += 1;
    loop {
        let c = *s.get(i)?;
        if line_terminator(c) {
            return None;
        }
        match c {
            '\\' => i += 1,
            '[' => class = true,
            ']' => class = false,
            '/' if !class => break,
            _ => {}
        }
        i += 1;
    }
    i += 1;
    while s.get(i).is_some_and(|c| c.is_alphanumeric()) {
        i += 1;
    }
    Some(i)
}

/// ECMAScript's line terminators (ECMA-262 §12.3), each a statement boundary
/// for automatic semicolon insertion and the end of a line comment.
fn line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// Past the block comment opening at `i` (Rust's nest, TypeScript's do
/// not), or `None` when it never closes.
fn block_comment_end(s: &[char], mut i: usize, typescript: bool) -> Option<usize> {
    let mut depth = 0;
    loop {
        if i + 1 >= s.len() {
            return None;
        }
        if s[i] == '/' && s[i + 1] == '*' && (depth == 0 || !typescript) {
            depth += 1;
            i += 2;
        } else if s[i] == '*' && s[i + 1] == '/' {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return Some(i);
            }
        } else {
            i += 1;
        }
    }
}

/// Whether the literal `text` that ends at index `j` is joined to more text:
/// by the macro around it, or by what follows past white space and
/// comments: `+`, or another literal on the same statement (TypeScript ends
/// a statement at a line terminator before one, a comment's included).
fn joined(s: &[char], j: usize, typescript: bool, parens: &mut [Paren], text: &str) -> bool {
    if let Some(p) = parens.last_mut() {
        if p.joins == Join::Format {
            p.joins = Join::No;
            return text.contains('{');
        }
    }
    if parens.iter().any(|p| p.joins == Join::All) {
        return true;
    }
    let (mut j, mut broke_line) = (j, false);
    loop {
        while j < s.len() && s[j].is_whitespace() {
            broke_line |= line_terminator(s[j]);
            j += 1;
        }
        match (s.get(j), s.get(j + 1)) {
            (Some('/'), Some('/')) => {
                while j < s.len() && !(line_terminator(s[j]) && (typescript || s[j] == '\n')) {
                    j += 1;
                }
            }
            (Some('/'), Some('*')) => match block_comment_end(s, j, typescript) {
                Some(end) => {
                    broke_line |= s[j..end].iter().copied().any(line_terminator);
                    j = end;
                }
                None => return true,
            },
            (Some('+'), _) => return true,
            (Some('"'), _) => return !(typescript && broke_line),
            (Some('\'' | '`'), _) => return typescript && !broke_line,
            _ => return false,
        }
    }
}

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
    // Each `(` open, and whether the last significant character closed a
    // control word's condition.
    let mut parens: Vec<Paren> = Vec::new();
    let mut after_control = false;
    // The last significant character of code, the word it ends, and the
    // word before the last punctuation (`concat` at `concat!(`).
    let (mut last, mut word, mut before) = (None::<char>, String::new(), String::new());
    // `last` ends `++` or `--`: an operand's postfix, after which `/` divides
    // (a prefix one cannot take a regex).
    let mut postfix = false;
    let mut i = 0;
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
    let template = |i: &mut usize, lits: &mut Vec<Lit>, parens: &mut [Paren]| -> Option<bool> {
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
            let joined = joined(&s, *i, typescript, parens, &text);
            lits.push(Lit { text, joined });
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
            while i < n && !(line_terminator(s[i]) && (typescript || s[i] == '\n')) {
                i += 1;
            }
            continue;
        }
        if c == '/' && next == Some('*') {
            i = block_comment_end(&s, i, typescript)?;
            continue;
        }
        let keyword = after_control || BEFORE_REGEX.contains(&word.as_str());
        let regex = typescript
            && c == '/'
            && ((last.is_none_or(|l| "(,=:[!&|?{};+-*%<>~^}".contains(l)) && !postfix) || keyword);
        if regex {
            // After a `}` a block's end begins a regex and an object's or a
            // function expression's divides: which, this does not model.
            if last == Some('}') && !keyword {
                return None;
            }
            i = regex_end(&s, i)?;
            (last, word) = (Some('"'), String::new());
            continue;
        }
        if !c.is_whitespace() {
            after_control = false;
        }
        // A raw string (`r"…"`, `r#"…"#`, `br"…"`, `cr"…"`), or a raw identifier.
        let raw_at = |j: usize| matches!(s.get(j), Some('"' | '#'));
        let after_ident = i > 0 && ident(s[i - 1]);
        if !typescript
            && !after_ident
            && ((c == 'r' && raw_at(i + 1))
                || (matches!(c, 'b' | 'c') && next == Some('r') && raw_at(i + 2)))
        {
            return None;
        }
        match c {
            '"' => {
                let text = quoted(&mut i)?;
                let joined = joined(&s, i, typescript, &mut parens, &text);
                lits.push(Lit { text, joined });
                (last, word) = (Some('"'), String::new());
                continue;
            }
            '\'' if typescript => {
                let text = quoted(&mut i)?;
                let joined = joined(&s, i, typescript, &mut parens, &text);
                lits.push(Lit { text, joined });
                (last, word) = (Some('"'), String::new());
                continue;
            }
            '(' => parens.push(Paren {
                control: CONTROL.contains(&word.as_str()),
                joins: match before.as_str() {
                    _ if typescript || last != Some('!') => Join::No,
                    "concat" => Join::All,
                    "format" => Join::Format,
                    _ => Join::No,
                },
            }),
            ')' => after_control = parens.pop().is_some_and(|p| p.control),
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
                if !template(&mut i, &mut lits, &mut parens)? {
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
                if !template(&mut i, &mut lits, &mut parens)? {
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
            } else if !word.is_empty() {
                before = std::mem::take(&mut word);
            }
            postfix = matches!(c, '+' | '-') && last == Some(c) && i > 0 && s[i - 1] == c;
            last = Some(c);
        }
        i += 1;
    }
    holes.is_empty().then_some(lits)
}

/// The key each `device.<name> <key>` in `lits` names. A line built
/// otherwise (joined to more text, its key missing or cut short) is left
/// out; `None` when no certain key remains and such a line was seen, since
/// the grant may be in it.
pub(super) fn keys(lits: &[Lit], name: &str) -> Option<Vec<String>> {
    let key_char = |c: char| c.is_alphanumeric() || matches!(c, '.' | '-' | '_');
    let mut keys = Vec::new();
    let mut uncertain = false;
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
                uncertain = true;
                continue;
            }
            if device != name {
                continue;
            }
            let Some(spelled) = after.strip_prefix([' ', '\t']) else {
                uncertain = true;
                continue;
            };
            let spelled = spelled.trim_start_matches([' ', '\t']);
            let key: String = spelled.chars().take_while(|c| key_char(*c)).collect();
            let end = &spelled[key.len()..];
            let certain = !key.is_empty()
                && !key.ends_with('.')
                && !(end.is_empty() && lit.joined)
                && end
                    .chars()
                    .next()
                    .is_none_or(|c| c.is_whitespace() || c == '\\');
            if !certain {
                uncertain = true;
                continue;
            }
            keys.push(key);
        }
    }
    if keys.is_empty() && uncertain {
        return None;
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
        // A Rust macro's arguments are one built string, however the grant
        // line is cut; a literal beside another and a `+` behind a comment
        // join too (review rounds 1 to 3).
        for built in [
            "const G: &str = concat!(\"device.midi purpose.\", \"midi\");",
            "const G: &str = concat!(\"device.midi purpose.midi\", \"-v2\");",
            "const G: &str = concat!(\"device.midi purpose.midi\");",
            "let g = format!(\"device.midi {}\", key);",
            "let g = format!(\"device.midi purpose.midi{}\", suffix(1));",
            "m!(\"device.midi purpose.midi\" \"-v2\")",
            "const G: &str = \"device.midi purpose.midi\" /* v2 */ + \"x\";",
        ] {
            assert_eq!(read(built, false), None, "{built}");
        }
        for built in [
            "export const grants = 'device.midi purpose.midi' // v2\n  + '-v2';",
            "export const grants = 'device.midi purpose.midi' /* v2 */ + '-v2';",
            "f('device.midi purpose.midi' '-v2')",
            "f('device.midi purpose.midi' `-v2`)",
        ] {
            assert_eq!(read(built, true), None, "{built}");
        }
        // Only a `format!` string with a placeholder is extended; a plain
        // literal argument, or a format string without one, is as written.
        assert_eq!(
            read(
                "let g = format!(\"{}\", \"device.midi purpose.midi\");",
                false
            ),
            key("purpose.midi")
        );
        assert_eq!(
            read("let g = format!(\"device.midi purpose.midi\");", false),
            key("purpose.midi")
        );
        assert_eq!(
            read(
                "let g = format!(\"device.midi purpose.{}\", \"midi\");",
                false
            ),
            None
        );
        // In TypeScript a literal after a line break is the next statement.
        assert_eq!(
            read(
                "export const grants = 'device.midi purpose.midi'\n'unrelated';",
                true
            ),
            key("purpose.midi")
        );
        // Rust's block comments nest on the way to a `+` too; TypeScript's
        // end at the first `*/`.
        assert_eq!(
            read(
                "fn grants() -> &'static str { \"device.midi purpose.midi\" /* a /* b */ + x */ }",
                false
            ),
            key("purpose.midi")
        );
        assert_eq!(
            read(
                "const g = 'device.midi purpose.midi' /* a /* b */ + '-v2';",
                true
            ),
            None
        );
        // A line terminator inside a comment, or any ECMAScript line
        // terminator, ends the statement too.
        assert_eq!(
            read(
                "export const grants = 'device.midi purpose.midi' /*\n*/ 'x';",
                true
            ),
            key("purpose.midi")
        );
        assert_eq!(
            read(
                "export const grants = 'device.midi purpose.midi'\u{2028}'x';",
                true
            ),
            key("purpose.midi")
        );
        assert_eq!(
            read(
                "export const grants = 'device.midi purpose.midi' /* a */ 'x';",
                true
            ),
            None
        );
        // A built line beside a written one, in another literal or the same,
        // leaves the written one's key.
        assert_eq!(
            read(
                "const g = 'device.midi purpose.midi\\ndevice.midi ' + name;",
                true
            ),
            key("purpose.midi")
        );
        assert_eq!(
            read(
                "const shown = 'device.midi ' + state;\nexport const grants = 'device.midi purpose.midi';",
                true
            ),
            key("purpose.midi")
        );
        // The `+` is read past a comment, so one in a comment alone is no join.
        assert_eq!(
            read(
                "export const grants = 'device.midi purpose.midi'; // + more\nconst n = 1;",
                true
            ),
            key("purpose.midi")
        );
        assert_eq!(
            read(
                "const g = ['device.midi purpose.midi', 'secret.keep k'];",
                true
            ),
            key("purpose.midi")
        );
        // A macro that builds no string leaves its argument as written.
        assert_eq!(
            read(
                "fn grants() -> &'static str { trim!(\"device.midi purpose.midi\") }",
                false
            ),
            key("purpose.midi")
        );
    }

    #[test]
    fn a_join_after_a_line_comment_uses_the_sources_line_terminators() {
        for terminator in ['\n', '\r', '\u{2028}', '\u{2029}'] {
            let source = format!("\"device.midi purpose.midi\" // comment{terminator}+ \"x\"\n;");
            assert_eq!(read(&source, true), None, "TypeScript: {source:?}");
            let expected = if terminator == '\n' {
                None
            } else {
                key("purpose.midi")
            };
            assert_eq!(read(&source, false), expected, "Rust: {source:?}");
        }
    }

    #[test]
    fn a_form_not_modelled_is_unknown() {
        for (source, typescript) in [
            (
                "const r = /a[/\nb/; export const grants = 'device.midi purpose.midi';",
                true,
            ),
            // After a `}`, a block's end or an object's: which is unknown.
            (
                "if (x) { y() } /[/*]/.test(s); export const grants = 'device.midi purpose.midi'; // */",
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
            (
                "const R: &CStr = cr#\"a /* b\"#; const G: &str = \"device.midi purpose.midi\";",
                false,
            ),
            ("const s = 'unterminated", true),
            ("/* never closed", true),
        ] {
            assert_eq!(read(source, typescript), None, "{source}");
        }
        assert_eq!(
            read(
                "const h = (a + b) / 2; export const grants = 'device.midi purpose.midi';",
                true
            ),
            key("purpose.midi")
        );
        // A regex is read past, a `/` in its class or escaped included
        // (euxy's app.ts matched a share link with one, which hid that it
        // granted no `device.midi`); one straight after a control word's
        // condition too, where a `/` after any other `)` divides.
        for source in [
            "const r = /[/*]/; export const grants = 'device.midi purpose.midi'; // */",
            "const m = raw.match(/[?&]d=([^&]+)/) ?? raw.match(/p\\/([^/?#]+)\\/?$/i);\nexport const grants = 'device.midi purpose.midi';",
            "if (x) /[/*]/.test(s); export const grants = 'device.midi purpose.midi'; // */",
            "while (ok(x)) /\\/*/.exec(s); export const grants = 'device.midi purpose.midi'; // */",
            "const q = /'/g; export const grants = 'device.midi purpose.midi';",
            // A `/` after a postfix `++` or `--` divides (review round 1:
            // read as a regex, it swallowed the grant).
            "const q=n++ / 2; export const grants=\"device.midi purpose.midi\"; const r=/x/;",
            "const q=n-- /2; export const grants=\"device.midi purpose.midi\"; const r=/x/;",
            // A unary `+` before a regex, and `+ +` that is no `++`.
            "const t = +/1/.test(s); export const grants = 'device.midi purpose.midi';",
            "const t = a + +/1/.test(s); export const grants = 'device.midi purpose.midi';",
        ] {
            assert_eq!(read(source, true), key("purpose.midi"), "{source}");
        }
        assert_eq!(
            read("const m = s.match(/x/);\nexport const grants = '';", true),
            Some(Vec::new())
        );
        assert_eq!(
            read(
                "if (x) { y = (a) / 2 } export const grants = 'device.midi purpose.midi';",
                true
            ),
            key("purpose.midi")
        );
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
