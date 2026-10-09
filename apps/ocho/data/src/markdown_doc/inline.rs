//! Inline Markdown (markdown.rs `InlineText::append` over pulldown-cmark's
//! inline events): emphasis, strikethrough, code spans, links, images,
//! autolinks, escapes, entities, soft and hard breaks, and math.

use super::math::{self, Formula};
use super::{link_allowed, MathKind, Run};

#[derive(Clone, Default)]
struct Style {
    bold: bool,
    italic: bool,
    strike: bool,
    url: Option<String>,
    /// Inside a link or image label: upstream protects the whole link from
    /// `\(`/`\[` conversion.
    in_link: bool,
}

/// Parse inline marks in `text`.
pub fn inline(text: &str) -> Vec<Run> {
    let chars: Vec<char> = text.chars().collect();
    let mut runs = Vec::new();
    append(&chars, &Style::default(), &mut runs);
    linkify(runs)
}

/// GitHub's extended autolinks: a bare `https://…`, `http://…` or `www.…` in
/// plain text is a link (agents write URLs bare more often than not). Code
/// spans, formulas and text already in a link stay as they are. Trailing
/// punctuation and an unbalanced closing parenthesis end the link, as GFM's do.
fn linkify(runs: Vec<Run>) -> Vec<Run> {
    let mut out = Vec::with_capacity(runs.len());
    for run in runs {
        if run.url.is_some() || run.code || run.math != MathKind::default() {
            out.push(run);
            continue;
        }
        let text = run.text.clone();
        let mut rest = text.as_str();
        let mut found = false;
        while let Some((start, end)) = bare_url(rest) {
            found = true;
            let target = &rest[start..end];
            let url = if target.starts_with("www.") {
                format!("https://{target}")
            } else {
                target.to_string()
            };
            if start > 0 {
                out.push(Run {
                    text: rest[..start].to_string(),
                    ..run.clone()
                });
            }
            let allowed = link_allowed(&url);
            out.push(Run {
                text: target.to_string(),
                url: allowed.then_some(url),
                ..run.clone()
            });
            rest = &rest[end..];
        }
        if !found {
            out.push(run);
        } else if !rest.is_empty() {
            out.push(Run {
                text: rest.to_string(),
                ..run
            });
        }
    }
    out
}

/// The byte range of the first bare URL in `text`, if any.
fn bare_url(text: &str) -> Option<(usize, usize)> {
    let lower = text.to_ascii_lowercase();
    let mut search = 0;
    loop {
        let start = ["https://", "http://", "www."]
            .iter()
            .filter_map(|p| lower[search..].find(p).map(|i| i + search))
            .min()?;
        // At a word's start: not inside `foo.www.bar` or `xhttps://`.
        let before = text[..start].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || c == '/' || c == '.') {
            search = start + 1;
            continue;
        }
        let mut end = start
            + text[start..]
                .find(|c: char| c.is_whitespace() || c == '<')
                .unwrap_or(text.len() - start);
        // Trailing punctuation is the sentence's, not the URL's.
        loop {
            let tail = text[..end].chars().next_back();
            match tail {
                Some('.' | ',' | ':' | ';' | '!' | '?' | '"' | '\'' | '*' | '_' | '~') => end -= 1,
                Some(')')
                    if text[start..end].matches('(').count()
                        < text[start..end].matches(')').count() =>
                {
                    end -= 1
                }
                _ => break,
            }
        }
        let body = &text[start..end];
        let host = body
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("HTTPS://")
            .trim_start_matches("HTTP://");
        if host.contains('.') && host.len() > 3 {
            return Some((start, end));
        }
        search = end.max(start + 1);
        if search >= text.len() {
            return None;
        }
    }
}

fn push(runs: &mut Vec<Run>, text: &str, style: &Style, code: bool) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = runs.last_mut() {
        if !code
            && !last.code
            && last.math == MathKind::None
            && last.bold == style.bold
            && last.italic == style.italic
            && last.strike == style.strike
            && last.url == style.url
        {
            last.text.push_str(text);
            return;
        }
    }
    runs.push(Run {
        text: text.to_string(),
        bold: style.bold,
        italic: style.italic,
        code,
        strike: style.strike,
        url: style.url.clone(),
        math: MathKind::None,
    });
}

/// A formula, or its literal text when it would not typeset (`math_node`).
fn push_math(runs: &mut Vec<Run>, source: &str, display: bool, style: &Style) {
    match Formula::parse(source, display) {
        Some(formula) => runs.push(Run {
            text: formula.source,
            bold: style.bold,
            italic: style.italic,
            code: false,
            strike: style.strike,
            url: style.url.clone(),
            math: if display {
                MathKind::Display
            } else {
                MathKind::Inline
            },
        }),
        None => push(runs, &math::literal(source, display), style, false),
    }
}

fn run_len(chars: &[char], at: usize, c: char) -> usize {
    chars[at..].iter().take_while(|x| **x == c).count()
}

fn is_word(c: Option<&char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric())
}

/// The closing delimiter run for an opener of `want` characters `c` starting
/// the search at `from`: the index of the run and how many of its characters
/// close (a 3-run closes a 2-opener with its last two, leaving one inside).
/// Code spans and math spans are opaque.
fn closer(chars: &[char], from: usize, c: char, want: usize) -> Option<(usize, usize)> {
    let mut i = from;
    let mut code_ticks = 0;
    while i < chars.len() {
        let x = chars[i];
        if x == '\\' {
            i += 2;
            continue;
        }
        if x == '`' {
            let n = run_len(chars, i, '`');
            if code_ticks == 0 {
                code_ticks = n;
            } else if code_ticks == n {
                code_ticks = 0;
            }
            i += n;
            continue;
        }
        if code_ticks == 0 && x == '$' {
            if let Some((_, _, next, _)) = math::dollar_span(chars, i) {
                i = next;
                continue;
            }
        }
        if code_ticks == 0 && x == c {
            let n = run_len(chars, i, c);
            let before_ws = i == 0 || chars[i - 1].is_whitespace();
            let after_word = is_word(chars.get(i + n));
            let flanking = !before_ws && (c != '_' || !after_word);
            if flanking {
                if n == want {
                    return Some((i, want));
                }
                if want == 2 && n == 3 {
                    return Some((i + 1, 2));
                }
                if want == 1 && n == 3 {
                    return Some((i + 2, 1));
                }
                if n > want && want == 1 {
                    return Some((i, 1));
                }
            }
            i += n;
            continue;
        }
        i += 1;
    }
    None
}

fn append(chars: &[char], style: &Style, runs: &mut Vec<Run>) {
    let mut text = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' if i + 1 < chars.len() => {
                if !style.in_link {
                    if let Some((source, display, end)) = math::bracket_span(chars, i) {
                        push(runs, &text, style, false);
                        text.clear();
                        push_math(runs, &source, display, style);
                        i = end;
                        continue;
                    }
                }
                if chars[i + 1] == '\n' {
                    text.push('\n');
                } else if chars[i + 1].is_ascii_punctuation() {
                    text.push(chars[i + 1]);
                } else {
                    text.push('\\');
                    text.push(chars[i + 1]);
                }
                i += 2;
            }
            '$' => match math::dollar_span(chars, i) {
                Some((start, end, next, display)) => {
                    push(runs, &text, style, false);
                    text.clear();
                    let source: String = chars[start..end].iter().collect();
                    push_math(runs, &source, display, style);
                    i = next;
                }
                None => {
                    text.push('$');
                    i += 1;
                }
            },
            '&' => match entity(chars, i) {
                Some((decoded, next)) => {
                    text.push_str(&decoded);
                    i = next;
                }
                None => {
                    text.push('&');
                    i += 1;
                }
            },
            '\n' => {
                // Two trailing spaces make a hard break; a soft break is a space.
                let hard = text.ends_with("  ");
                let trimmed = text.trim_end_matches(' ').len();
                text.truncate(trimmed);
                text.push(if hard { '\n' } else { ' ' });
                i += 1;
                while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                    i += 1;
                }
            }
            '`' => {
                let n = run_len(chars, i, '`');
                let mut j = i + n;
                let mut found = None;
                while j < chars.len() {
                    if chars[j] == '`' {
                        let m = run_len(chars, j, '`');
                        if m == n {
                            found = Some(j);
                            break;
                        }
                        j += m;
                    } else {
                        j += 1;
                    }
                }
                match found {
                    Some(end) => {
                        push(runs, &text, style, false);
                        text.clear();
                        let mut code: String = chars[i + n..end].iter().collect();
                        code = code.replace('\n', " ");
                        if code.len() > 1
                            && code.starts_with(' ')
                            && code.ends_with(' ')
                            && !code.trim().is_empty()
                        {
                            code = code[1..code.len() - 1].to_string();
                        }
                        push(runs, &code, style, true);
                        i = end + n;
                    }
                    None => {
                        text.extend(std::iter::repeat_n('`', n));
                        i += n;
                    }
                }
            }
            '*' | '_' | '~' => {
                let n = run_len(chars, i, c);
                let before_word = i > 0 && is_word(chars.get(i - 1));
                let after_ws = chars.get(i + n).is_none_or(|x| x.is_whitespace());
                let can_open = !after_ws && (c != '_' || !before_word);
                let want = if c == '~' {
                    if n == 2 {
                        2
                    } else {
                        0
                    }
                } else if n >= 2 {
                    2
                } else {
                    1
                };
                let mut done = false;
                if can_open && want > 0 {
                    let inner_from = i + want;
                    if let Some((at, len)) = closer(chars, inner_from, c, want) {
                        push(runs, &text, style, false);
                        text.clear();
                        let mut next = style.clone();
                        match (c, want) {
                            ('~', _) => next.strike = true,
                            (_, 2) => next.bold = true,
                            _ => next.italic = true,
                        }
                        append(&chars[inner_from..at], &next, runs);
                        i = at + len;
                        done = true;
                    }
                }
                if !done {
                    text.push(c);
                    i += 1;
                }
            }
            '!' if chars.get(i + 1) == Some(&'[') => match link(chars, i + 1) {
                Some((label, _url, end)) => {
                    push(runs, &text, style, false);
                    text.clear();
                    push(runs, "[Image: ", style, false);
                    let inner = Style {
                        in_link: true,
                        ..style.clone()
                    };
                    append(&label, &inner, runs);
                    push(runs, "]", style, false);
                    i = end;
                }
                None => {
                    text.push('!');
                    i += 1;
                }
            },
            '[' => match link(chars, i) {
                Some((label, url, end)) => {
                    push(runs, &text, style, false);
                    text.clear();
                    let mut next = style.clone();
                    next.in_link = true;
                    // Only web, mail and local-file links become clickable.
                    if link_allowed(&url) {
                        next.url = Some(url);
                    }
                    append(&label, &next, runs);
                    i = end;
                }
                None => {
                    text.push('[');
                    i += 1;
                }
            },
            '<' => match autolink(chars, i) {
                Some((shown, url, next)) => {
                    push(runs, &text, style, false);
                    text.clear();
                    let mut linked = style.clone();
                    if link_allowed(&url) {
                        linked.url = Some(url);
                    }
                    push(runs, &shown, &linked, false);
                    i = next;
                }
                None => {
                    text.push('<');
                    i += 1;
                }
            },
            _ => {
                text.push(c);
                i += 1;
            }
        }
    }
    push(runs, &text, style, false);
}

/// `<scheme:…>` or `<user@host>` at `chars[at] == '<'`: the text shown, the
/// destination, the index after `>`.
fn autolink(chars: &[char], at: usize) -> Option<(String, String, usize)> {
    let close = chars[at + 1..].iter().position(|x| *x == '>')? + at + 1;
    let inner: String = chars[at + 1..close].iter().collect();
    if inner.is_empty() || inner.contains(|c: char| c.is_whitespace() || c == '<') {
        return None;
    }
    if let Some((scheme, _)) = inner.split_once(':') {
        let mut letters = scheme.chars();
        let valid = (2..=32).contains(&scheme.len())
            && letters.next().is_some_and(|c| c.is_ascii_alphabetic())
            && letters.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'));
        return valid.then(|| (inner.clone(), inner.clone(), close + 1));
    }
    let (user, host) = inner.split_once('@')?;
    let email = !user.is_empty()
        && host.contains('.')
        && !host.starts_with('.')
        && !host.ends_with('.')
        && inner.chars().filter(|c| *c == '@').count() == 1;
    email.then(|| (inner.clone(), format!("mailto:{inner}"), close + 1))
}

/// An HTML entity at `chars[at] == '&'`: its text and the index after `;`.
fn entity(chars: &[char], at: usize) -> Option<(String, usize)> {
    let end = chars[at + 1..].iter().take(33).position(|c| *c == ';')? + at + 1;
    let name: String = chars[at + 1..end].iter().collect();
    let decoded = if let Some(number) = name.strip_prefix('#') {
        let code = match number.strip_prefix(['x', 'X']) {
            Some(hex) if !hex.is_empty() && hex.len() <= 6 => u32::from_str_radix(hex, 16).ok()?,
            Some(_) => return None,
            None if !number.is_empty() && number.len() <= 7 => number.parse().ok()?,
            None => return None,
        };
        let c = if code == 0 {
            '\u{fffd}'
        } else {
            char::from_u32(code).unwrap_or('\u{fffd}')
        };
        c.to_string()
    } else {
        match name.as_str() {
            "amp" => "&",
            "lt" => "<",
            "gt" => ">",
            "quot" => "\"",
            "apos" => "'",
            "nbsp" => "\u{a0}",
            "copy" => "©",
            "reg" => "®",
            "trade" => "™",
            "hellip" => "…",
            "mdash" => "—",
            "ndash" => "–",
            "larr" => "←",
            "rarr" => "→",
            "times" => "×",
            "middot" => "·",
            _ => return None,
        }
        .to_string()
    };
    Some((decoded, end + 1))
}

/// `[label](url "title")` at `chars[at] == '['`: the label's characters, the
/// url, and the index after the closing paren.
fn link(chars: &[char], at: usize) -> Option<(Vec<char>, String, usize)> {
    let mut depth = 0;
    let mut close = None;
    let mut i = at;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 1,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(i);
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }
    let close = close?;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let mut j = close + 2;
    while j < chars.len() && chars[j] == ' ' {
        j += 1;
    }
    let url: String;
    if chars.get(j) == Some(&'<') {
        let end = chars[j + 1..].iter().position(|x| *x == '>')? + j + 1;
        url = chars[j + 1..end].iter().collect();
        j = end + 1;
    } else {
        let start = j;
        let mut parens = 0;
        while j < chars.len() {
            match chars[j] {
                '\\' => j += 1,
                '(' => parens += 1,
                ')' if parens == 0 => break,
                ')' => parens -= 1,
                c if c.is_whitespace() => break,
                _ => {}
            }
            j += 1;
        }
        let raw: String = chars[start..j.min(chars.len())].iter().collect();
        url = unescape(&raw);
    }
    // An optional title, then the closing paren.
    while j < chars.len() && chars[j].is_whitespace() {
        j += 1;
    }
    if matches!(chars.get(j), Some('"') | Some('\'')) {
        let quote = chars[j];
        let end = chars[j + 1..].iter().position(|x| *x == quote)? + j + 1;
        j = end + 1;
        while j < chars.len() && chars[j].is_whitespace() {
            j += 1;
        }
    }
    if chars.get(j) != Some(&')') {
        return None;
    }
    Some((chars[at + 1..close].to_vec(), url, j + 1))
}

/// Backslash escapes in a link destination.
fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek().is_some_and(|n| n.is_ascii_punctuation()) {
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else {
            out.push(c);
        }
    }
    out
}
