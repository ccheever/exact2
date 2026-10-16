//! `tamber fmt` (LLP 1121 §3, D13): one canonical layout, so an editor's
//! edit is a one-line diff in a file a person can still edit. Blocks keep
//! their order; a block's entries are indented two spaces under its name and
//! a token's `when` lines two more; `name = value` takes one space each side,
//! metadata (and a dial's range) follow the value after two spaces; a
//! trailing comment follows after two spaces; a comment line keeps its own
//! indent's level (one before a block's first entry takes the entry's), so an
//! edit around it moves it nowhere. One blank line separates the header and each
//! block; inside a block a run of blank lines is one, and none opens or
//! closes it. Each value's text is kept as written, as Contract's formatter
//! keeps a literal's spelling. Formatting a formatted design changes nothing.

use crate::lex::{self, Line};
use crate::Diagnostic;

/// `src` in the canonical layout, or the parse's refusals: a design that
/// does not parse has no layout to give.
pub fn format(src: &str) -> Result<String, Vec<Diagnostic>> {
    crate::parse(src)?;
    let raw: Vec<&str> = src.lines().collect();
    // Each line: blank, a comment alone, or code with its trailing comment.
    enum Kind<'a> {
        Blank,
        Comment(&'a str),
        Code(String),
    }
    let mut items: Vec<(Kind, usize)> = Vec::new();
    let (lines, _) = lex::lines(src);
    let mut code = lines.iter().peekable();
    let mut block = "";
    let mut entry_indent = None;
    for text in &raw {
        let text = text.strip_suffix('\r').unwrap_or(text);
        let (body, comment) = lex::split_comment(text);
        if body.trim().is_empty() {
            match comment {
                // A comment keeps its own place: its indent's level, so an
                // edit to the line after it moves it nowhere.
                Some(c) => {
                    let indent = text.len() - text.trim_start_matches(' ').len();
                    let level = match (indent, entry_indent) {
                        (0, _) => 0,
                        (i, Some(e)) if i <= e => 1,
                        (_, Some(_)) => 2,
                        (_, None) => usize::MAX,
                    };
                    items.push((Kind::Comment(c.trim_end()), level));
                }
                None => items.push((Kind::Blank, 0)),
            }
            continue;
        }
        let Some(l) = code.next() else { break };
        let level = if l.indent == 0 {
            block = l.text.split(' ').next().unwrap_or("");
            entry_indent = None;
            0
        } else if entry_indent.is_none_or(|e| e == l.indent) {
            entry_indent = Some(l.indent);
            1
        } else {
            2
        };
        let mut out = match (level, block) {
            (0, "tamber") => format!("tamber {}", l.text["tamber".len()..].trim()),
            (0, _) => l.text.clone(),
            (1, "axes") => axis(l),
            (1, "dials") => dial(l),
            (1, "theme") => token(l),
            (2, "theme") => when(l),
            _ => l.text.clone(),
        };
        if let Some(c) = comment {
            out.push_str("  ");
            out.push_str(c.trim_end());
        }
        items.push((Kind::Code(out), level));
    }
    // Blank lines at the end are none.
    while matches!(items.last(), Some((Kind::Blank, _))) {
        items.pop();
    }
    // A comment before a block's first entry, whose indent no entry has
    // given a level yet, takes the level of the line after it (an entry's,
    // never the next block's).
    let mut next_level = 1;
    for (kind, level) in items.iter_mut().rev() {
        match kind {
            Kind::Code(_) => next_level = (*level).max(1),
            Kind::Comment(_) if *level == usize::MAX => *level = next_level,
            _ => {}
        }
    }
    // A top-level line and the comments right above it open a group, which
    // one blank line separates from what is before.
    let mut out = String::new();
    let mut blank = false;
    let mut started = false;
    let mut opened = false;
    for (i, (kind, level)) in items.iter().enumerate() {
        let opens = *level == 0
            && match kind {
                Kind::Code(_) => true,
                // A top-level comment opens its own group (the block after
                // it, when it is right above one).
                Kind::Comment(_) => true,
                Kind::Blank => false,
            }
            && !matches!(items.get(i.wrapping_sub(1)), Some((Kind::Comment(_), 0)) if i > 0);
        match kind {
            Kind::Blank => {
                blank = true;
                continue;
            }
            _ if !started => {}
            _ if opens => out.push('\n'),
            // Inside a block: one blank where any were, none at its start.
            _ if blank && !opened => out.push('\n'),
            _ => {}
        }
        blank = false;
        started = true;
        let text = match kind {
            Kind::Code(t) => t.as_str(),
            Kind::Comment(c) => c,
            Kind::Blank => unreachable!(),
        };
        out.push_str(&"  ".repeat(*level));
        out.push_str(text);
        out.push('\n');
        opened = matches!(kind, Kind::Code(_)) && *level == 0;
    }
    // What it prints parses, or it prints nothing: a layout never breaks a
    // design.
    if let Err(all) = crate::parse(&out) {
        return Err(all
            .into_iter()
            .map(|d| Diagnostic::new("tamber-fmt", format!("tamber fmt would break this design here ({}): leave it as it is and report it", d.message), d.line, d.col))
            .collect());
    }
    Ok(out)
}

/// `name = a | b  under scheme=dark`.
fn axis(l: &Line) -> String {
    let Some((name, rest)) = l.text.split_once('=') else {
        return l.text.clone();
    };
    // The clause as the parser finds it: an `under` with no `|` after it.
    let (values, under) = match rest
        .rsplit_once(" under ")
        .filter(|(_, after)| !after.trim().is_empty() && !after.contains('|'))
    {
        Some((v, u)) => (v, Some(u.trim())),
        None => (rest, None),
    };
    let values: Vec<&str> = values.split('|').map(str::trim).collect();
    let mut out = format!("{} = {}", name.trim(), values.join(" | "));
    if let Some(u) = under {
        out.push_str("  under ");
        out.push_str(&lex::tight_equals(u));
    }
    out
}

/// `name: kind = value  in low..high  meta=…`.
fn dial(l: &Line) -> String {
    dial_value(l, None)
}

/// [`dial`] with its value replaced by `value` (an edit, `set-dial`).
pub(crate) fn dial_with(l: &Line, value: &str) -> String {
    dial_value(l, Some(value))
}

fn dial_value(l: &Line, with: Option<&str>) -> String {
    let Some((lhs, value)) = lex::split_top(&l.text, '=') else {
        return l.text.clone();
    };
    let (name, kind) = lhs.split_once(':').unwrap_or((lhs, ""));
    let value = value.trim();
    let words = lex::words(value);
    let end = words
        .iter()
        .position(|(_, w)| *w == "in" || meta(w))
        .unwrap_or(words.len());
    let mut out = format!(
        "{}: {} = {}",
        name.trim(),
        kind.trim(),
        with.unwrap_or(value_text(value, &words, end))
    );
    let mut i = end;
    if words.get(i).is_some_and(|(_, w)| *w == "in") {
        let from = i + 1;
        i = words[from..]
            .iter()
            .position(|(_, w)| meta(w))
            .map_or(words.len(), |p| from + p);
        out.push_str("  in ");
        out.push_str(&words[from..i].iter().map(|(_, w)| *w).collect::<String>());
    }
    metadata(&mut out, &words[i..]);
    out
}

/// `name = value  meta=…`, `name: kind = value`.
fn token(l: &Line) -> String {
    token_value(l, None)
}

/// [`token`] with its value replaced by `value` (an edit, `set-token`).
pub(crate) fn token_with(l: &Line, value: &str) -> String {
    token_value(l, Some(value))
}

fn token_value(l: &Line, with: Option<&str>) -> String {
    let Some((lhs, value)) = lex::split_top(&l.text, '=') else {
        return l.text.clone();
    };
    let lhs = match lhs.split_once(':') {
        Some((n, k)) => format!("{}: {}", n.trim(), k.trim()),
        None => lhs.trim().to_string(),
    };
    let value = value.trim();
    let words = lex::words(value);
    let end = (0..words.len())
        .rev()
        .take_while(|&i| meta(words[i].1))
        .last()
        .unwrap_or(words.len());
    let mut out = format!("{lhs} = {}", with.unwrap_or(value_text(value, &words, end)));
    metadata(&mut out, &words[end..]);
    out
}

/// `when a=b c=d: value`.
fn when(l: &Line) -> String {
    let Some(rest) = l.text.strip_prefix("when ") else {
        return l.text.clone();
    };
    let Some((conds, value)) = lex::split_top(rest, ':') else {
        return l.text.clone();
    };
    let conds = lex::tight_equals(conds);
    let conds: Vec<&str> = conds.split_whitespace().collect();
    format!("when {}: {}", conds.join(" "), value.trim())
}

/// A metadata word, `name=value`.
fn meta(w: &str) -> bool {
    w.split_once('=')
        .is_some_and(|(k, v)| lex::is_ident(k) && !v.is_empty())
}

/// The value's text as written, up to word `end`.
fn value_text<'a>(value: &'a str, words: &[(usize, &str)], end: usize) -> &'a str {
    match words.get(end) {
        Some(&(at, _)) => value[..at].trim_end(),
        None => value,
    }
}

/// Words joined by one space.
fn join(words: &[(usize, &str)]) -> String {
    words.iter().map(|(_, w)| *w).collect::<Vec<_>>().join(" ")
}

/// Metadata after the value: two spaces, then each word.
fn metadata(out: &mut String, words: &[(usize, &str)]) {
    if !words.is_empty() {
        out.push_str("  ");
        out.push_str(&join(words));
    }
}
