//! The `.tamber` grammar (LLP 1121 §3) into a [`Design`]: blocks, their
//! lines, and each value as its CSS text, read later by `css.rs`.

use crate::lex::{self, Line};
use crate::Diagnostic;

/// A parsed design, before checking.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Design {
    /// The name after `tamber`.
    pub name: String,
    /// App axes, as declared (D5).
    pub axes: Vec<Axis>,
    /// Dials, as declared (D4).
    pub dials: Vec<Dial>,
    /// Tokens, as declared (D3).
    pub tokens: Vec<Token>,
    /// The `exports` block, when there is one (D7's retention).
    pub exports: Option<Vec<At<String>>>,
    /// Icon tokens, the `icons` block's (D10).
    pub icons: Vec<Token>,
}

/// Something with the place it was written.
#[derive(Debug, Clone, PartialEq)]
pub struct At<T> {
    /// What.
    pub value: T,
    /// Its line, 1-based.
    pub line: u32,
    /// Its column, 1-based.
    pub col: u32,
}

/// `dark-style = black | dim  under scheme=dark`.
#[derive(Debug, Clone, PartialEq)]
pub struct Axis {
    /// The axis.
    pub name: At<String>,
    /// Its values, the first the default.
    pub values: Vec<String>,
    /// `under scheme=dark`: it matters only where the host shows dark.
    pub under_dark: bool,
}

/// A dial's type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialKind {
    /// `color`
    Color,
    /// `length`
    Length,
    /// `number`
    Number,
}

impl DialKind {
    /// As written.
    pub fn name(self) -> &'static str {
        match self {
            DialKind::Color => "color",
            DialKind::Length => "length",
            DialKind::Number => "number",
        }
    }
}

/// `accent: color = #1083fe`, `roundness: number = 1  in 0..2 step=0.1`.
#[derive(Debug, Clone, PartialEq)]
pub struct Dial {
    /// The dial.
    pub name: At<String>,
    /// Its type.
    pub kind: DialKind,
    /// Its default, as CSS text.
    pub value: At<String>,
    /// Its closed range, as written.
    pub range: Option<(String, String)>,
    /// Editor metadata (`step=`, `label=` …), in order.
    pub meta: Vec<(String, String)>,
}

/// `surface.page = light-dark(#fff, #000)` and its `when` lines.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// The name as written: dotted, or a role's spelling.
    pub name: At<String>,
    /// The kind written after `:`, when there is one.
    pub kind: Option<String>,
    /// The value, as CSS text.
    pub value: At<String>,
    /// Its `when` lines, in order.
    pub whens: Vec<When>,
    /// Metadata after the value (`scheme=fixed`, `label=…`), in order.
    pub meta: Vec<(String, String)>,
}

/// `when dark-style=dim: light-dark(#fff, #161e27)`.
#[derive(Debug, Clone, PartialEq)]
pub struct When {
    /// `axis=value` conditions, in order.
    pub conds: Vec<(String, String)>,
    /// The value, as CSS text.
    pub value: At<String>,
    /// Where the line starts.
    pub line: u32,
    /// Its column.
    pub col: u32,
}

/// Parse a `.tamber` source. Every line with a refusal reports one.
pub fn parse(src: &str) -> Result<Design, Vec<Diagnostic>> {
    let (lines, mut errors) = lex::lines(src);
    let mut design = Design::default();
    let mut rest = lines.as_slice();
    match rest.first() {
        Some(l) if l.indent == 0 && l.text.starts_with("tamber ") => {
            let name = l.text["tamber ".len()..].trim();
            if lex::is_ident(name) {
                design.name = name.to_string();
            } else {
                errors.push(Diagnostic::new(
                    "tamber-header",
                    format!("`tamber {name}`: the design's name is an identifier"),
                    l.line,
                    l.col,
                ));
            }
            rest = &rest[1..];
        }
        first => {
            let (line, col) = first.map_or((1, 1), |l| (l.line, l.col));
            errors.push(Diagnostic::new(
                "tamber-header",
                "a design starts `tamber <name>`",
                line,
                col,
            ));
            return Err(errors);
        }
    }
    let mut seen: Vec<&str> = Vec::new();
    while let Some((head, tail)) = rest.split_first() {
        let end = tail
            .iter()
            .position(|l| l.indent == 0)
            .unwrap_or(tail.len());
        let (body, after) = tail.split_at(end);
        rest = after;
        if head.indent != 0 {
            errors.push(Diagnostic::new(
                "tamber-indent",
                "a block's lines are indented under its name",
                head.line,
                head.col,
            ));
            continue;
        }
        let word = head.text.split(' ').next().unwrap_or("");
        if ["axes", "dials", "theme", "exports"].contains(&word) {
            if head.text != word {
                errors.push(Diagnostic::new(
                    "tamber-block",
                    format!("`{word}` stands alone on its line"),
                    head.line,
                    head.col,
                ));
                continue;
            }
            if seen.contains(&word) {
                errors.push(Diagnostic::new(
                    "tamber-block",
                    format!("a second `{word}` block: a design has one of each"),
                    head.line,
                    head.col,
                ));
                continue;
            }
            seen.push(word);
        }
        match word {
            "axes" => entries(body, &mut errors, |l, e| {
                axis(l, e).map(|a| design.axes.push(a))
            }),
            "dials" => entries(body, &mut errors, |l, e| {
                dial(l, e).map(|d| design.dials.push(d))
            }),
            "theme" => tokens(body, &mut design.tokens, &mut errors),
            "exports" => {
                let mut names = Vec::new();
                entries(body, &mut errors, |l, e| {
                    if !lex::is_name(&l.text) && role_spelling(&l.text).is_none() {
                        e.push(Diagnostic::new(
                            "tamber-syntax",
                            format!("`{}`: an export is a token's name", l.text),
                            l.line,
                            l.col,
                        ));
                        return None;
                    }
                    names.push(At {
                        value: l.text.clone(),
                        line: l.line,
                        col: l.col,
                    });
                    Some(())
                });
                design.exports = Some(names);
            }
            "icons" => tokens(body, &mut design.icons, &mut errors),
            "component" => errors.push(Diagnostic::new(
                "tamber-stage",
                "component params (D15) arrive in stage 7",
                head.line,
                head.col,
            )),
            _ => errors.push(Diagnostic::new(
                "tamber-block",
                format!(
                    "`{}`: a block is `axes`, `dials`, `theme`, `icons`, `exports` or `component`",
                    head.text
                ),
                head.line,
                head.col,
            )),
        }
    }
    if errors.is_empty() {
        Ok(design)
    } else {
        errors.sort_by_key(|d| (d.line, d.col));
        Err(errors)
    }
}

/// `-exact-secondary-label` or a bare role key's name, without the prefix;
/// whether it names a role is the checker's question.
pub(crate) fn role_spelling(name: &str) -> Option<&str> {
    name.strip_prefix("-exact-").filter(|n| lex::is_ident(n))
}

/// A block's lines, one entry each at one indent.
fn entries(
    body: &[Line],
    errors: &mut Vec<Diagnostic>,
    mut each: impl FnMut(&Line, &mut Vec<Diagnostic>) -> Option<()>,
) {
    let Some(indent) = body.first().map(|l| l.indent) else {
        return;
    };
    for l in body {
        if l.indent != indent {
            errors.push(Diagnostic::new(
                "tamber-indent",
                "this block's lines share one indent",
                l.line,
                l.col,
            ));
            continue;
        }
        let _ = each(l, errors);
    }
}

fn syntax(l: &Line, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("tamber-syntax", message, l.line, l.col)
}

/// The text after `at` in `l`, with its column.
fn value_at(l: &Line, at: usize) -> At<String> {
    let text = &l.text[at..];
    let trimmed = text.trim_start();
    At {
        value: trimmed.trim_end().to_string(),
        line: l.line,
        col: l.col + (at + text.len() - trimmed.len()) as u32,
    }
}

fn axis(l: &Line, errors: &mut Vec<Diagnostic>) -> Option<Axis> {
    let Some((name, rest)) = l.text.split_once('=') else {
        errors.push(syntax(l, "an axis is `name = value | value …`"));
        return None;
    };
    let name = name.trim();
    // The clause follows the values: an `under` with no `|` after it (a
    // value may be called `under`).
    let clause = rest
        .rsplit_once(" under ")
        .filter(|(_, after)| !after.trim().is_empty() && !after.contains('|'));
    let (values, under_dark) = match clause {
        Some((values, under)) if lex::tight_equals(under.trim()) == "scheme=dark" => (values, true),
        Some((_, under)) => {
            errors.push(syntax(
                l,
                format!(
                    "`under {}`: an axis may be `under scheme=dark` only",
                    under.trim()
                ),
            ));
            return None;
        }
        None => (rest, false),
    };
    let values: Vec<String> = values.split('|').map(|v| v.trim().to_string()).collect();
    if !lex::is_ident(name) || values.iter().any(|v| !lex::is_ident(v)) {
        errors.push(syntax(l, "an axis and its values are identifiers"));
        return None;
    }
    Some(Axis {
        name: At {
            value: name.to_string(),
            line: l.line,
            col: l.col,
        },
        values,
        under_dark,
    })
}

fn dial(l: &Line, errors: &mut Vec<Diagnostic>) -> Option<Dial> {
    let shape = "a dial is `name: color|length|number = value [in low..high] [meta=…]`";
    let (Some((name, rest)), true) = (l.text.split_once(':'), l.text.contains('=')) else {
        errors.push(syntax(l, shape));
        return None;
    };
    let Some((kind, value)) = rest.split_once('=') else {
        errors.push(syntax(l, shape));
        return None;
    };
    let kind = match kind.trim() {
        "color" => DialKind::Color,
        "length" => DialKind::Length,
        "number" => DialKind::Number,
        other => {
            errors.push(syntax(l, format!("`{other}`: {shape}")));
            return None;
        }
    };
    let name = name.trim();
    if !lex::is_ident(name) {
        errors.push(syntax(
            l,
            format!("`{name}`: a dial's name is an identifier"),
        ));
        return None;
    }
    let start = l.text.len() - value.len();
    // The value ends, outside parentheses, at `in` or the first `name=`.
    let words = lex::words(value);
    let end = words
        .iter()
        .position(|(_, w)| *w == "in" || meta_word(w).is_some())
        .unwrap_or(words.len());
    let Some(&(last, last_word)) = end.checked_sub(1).and_then(|i| words.get(i)) else {
        errors.push(syntax(l, format!("`{name}` has no value: {shape}")));
        return None;
    };
    let mut parsed = Dial {
        name: At {
            value: name.to_string(),
            line: l.line,
            col: l.col,
        },
        kind,
        value: value_at(l, start),
        range: None,
        meta: Vec::new(),
    };
    parsed.value.value = value[..last + last_word.len()].trim().to_string();
    let mut i = end;
    if words.get(i).is_some_and(|(_, w)| *w == "in") {
        let from = i + 1;
        i = words[from..]
            .iter()
            .position(|(_, w)| meta_word(w).is_some())
            .map_or(words.len(), |p| from + p);
        let range: String = words[from..i].iter().map(|(_, w)| *w).collect();
        match range.split_once("..") {
            Some((low, high)) if !low.is_empty() && !high.is_empty() => {
                parsed.range = Some((low.to_string(), high.to_string()));
            }
            _ => {
                errors.push(syntax(l, format!("`in {range}`: a range is `low..high`")));
                return None;
            }
        }
    }
    for (_, w) in &words[i..] {
        match meta_word(w) {
            Some((k, v)) => match lex::unquote(v) {
                Ok(v) => parsed.meta.push((k.to_string(), v)),
                Err(why) => {
                    errors.push(syntax(l, format!("`{k}=`: {why}")));
                    return None;
                }
            },
            None => {
                errors.push(syntax(l, format!("`{w}`: editor metadata is `name=value`")));
                return None;
            }
        }
    }
    Some(parsed)
}

/// `step=0.1`, `label="Accent"`.
fn meta_word(w: &str) -> Option<(&str, &str)> {
    let (k, v) = w.split_once('=')?;
    (lex::is_ident(k) && !v.is_empty()).then_some((k, v))
}

/// A `theme` block: token lines, each with its deeper `when` lines.
fn tokens(body: &[Line], out: &mut Vec<Token>, errors: &mut Vec<Diagnostic>) {
    let Some(indent) = body.first().map(|l| l.indent) else {
        return;
    };
    let mut when_indent = None;
    for l in body {
        if l.indent > indent {
            let Some(token) = out
                .last_mut()
                .filter(|_| when_indent.is_none_or(|w| w == l.indent))
            else {
                errors.push(Diagnostic::new(
                    "tamber-indent",
                    "a deeper line is a `when` under the token above it",
                    l.line,
                    l.col,
                ));
                continue;
            };
            when_indent = Some(l.indent);
            if let Some(w) = when(l, errors) {
                token.whens.push(w);
            }
            continue;
        }
        when_indent = None;
        if l.indent < indent {
            errors.push(Diagnostic::new(
                "tamber-indent",
                "this block's tokens share one indent",
                l.line,
                l.col,
            ));
            continue;
        }
        // `when = …` is a token called `when`; `when axis=value: …` a when.
        if l.text.starts_with("when ") && !l.text[4..].trim_start().starts_with('=') {
            errors.push(syntax(l, "a `when` is indented under its token"));
            continue;
        }
        if let Some(t) = token(l, errors) {
            out.push(t);
        }
    }
}

fn token(l: &Line, errors: &mut Vec<Diagnostic>) -> Option<Token> {
    let Some((lhs, _)) = lex::split_top(&l.text, '=') else {
        errors.push(syntax(l, "a token is `name = value`"));
        return None;
    };
    let (name, kind) = match lhs.split_once(':') {
        Some((n, k)) => (n.trim(), Some(k.trim().to_string())),
        None => (lhs.trim(), None),
    };
    if !lex::is_name(name) && role_spelling(name).is_none() {
        errors.push(syntax(
            l,
            format!("`{name}`: a token's name is identifiers joined by `.`, or a role's"),
        ));
        return None;
    }
    let mut value = value_at(l, lhs.len() + 1);
    // Metadata words (`scheme=fixed`) end the line, after the value.
    let words = lex::words(&value.value);
    let end = (0..words.len())
        .rev()
        .take_while(|&i| meta_word(words[i].1).is_some())
        .last()
        .unwrap_or(words.len());
    let mut meta = Vec::new();
    for (_, w) in &words[end..] {
        let (k, v) = meta_word(w).expect("a metadata word");
        match lex::unquote(v) {
            Ok(v) => meta.push((k.to_string(), v)),
            Err(why) => {
                errors.push(syntax(l, format!("`{k}=`: {why}")));
                return None;
            }
        }
    }
    if let Some(&(at, _)) = words.get(end) {
        value.value = value.value[..at].trim_end().to_string();
    }
    if value.value.is_empty() {
        errors.push(syntax(l, format!("`{name}` has no value")));
        return None;
    }
    Some(Token {
        name: At {
            value: name.to_string(),
            line: l.line,
            col: l.col,
        },
        kind,
        value,
        whens: Vec::new(),
        meta,
    })
}

fn when(l: &Line, errors: &mut Vec<Diagnostic>) -> Option<When> {
    let shape = "a `when` is `when axis=value …: value`";
    let Some(rest) = l.text.strip_prefix("when ") else {
        errors.push(syntax(l, shape));
        return None;
    };
    let Some((conds, _)) = lex::split_top(rest, ':') else {
        errors.push(syntax(l, shape));
        return None;
    };
    let mut parsed = Vec::new();
    for c in lex::tight_equals(conds).split_whitespace() {
        match c.split_once('=') {
            Some((a, v)) if lex::is_ident(a) && lex::is_ident(v) => {
                parsed.push((a.to_string(), v.to_string()))
            }
            _ => {
                errors.push(syntax(l, format!("`{c}`: {shape}")));
                return None;
            }
        }
    }
    let value = value_at(l, "when ".len() + conds.len() + 1);
    if parsed.is_empty() || value.value.is_empty() {
        errors.push(syntax(l, shape));
        return None;
    }
    Some(When {
        conds: parsed,
        value,
        line: l.line,
        col: l.col,
    })
}
