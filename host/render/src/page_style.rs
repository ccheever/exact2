//! A rendered page's own stylesheet (LLP 1048.007 D4), as
//! `host/web-js/page-style.mjs` selects it: of the rules the build analyzed
//! (`.gen/page-style.json`, each with what its selector needs of a
//! document), those the page's tokens can meet, in the sheet's order.

use std::collections::HashSet;

/// One rule of the shell's sheet: the grouping at-rules around it, each
/// opened (`@media …{`), the rule, and its needs (any one alternative of
/// tokens, whole, lets it match).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Rule {
    wrap: String,
    rule: String,
    needs: Vec<Vec<String>>,
}

/// The rules the build wrote, or `None` when they don't parse.
pub(crate) fn parse(json: &str) -> Option<Vec<Rule>> {
    let rows: Vec<(String, String, Vec<Vec<String>>)> = serde_json::from_str(json).ok()?;
    Some(
        rows.into_iter()
            .map(|(wrap, rule, needs)| Rule { wrap, rule, needs })
            .collect(),
    )
}

/// The tokens a document's markup offers: `t:` a tag, `a:` an attribute
/// name, `c:` a class, `i:` an id (`page-style.mjs` `tokens`).
pub(crate) fn tokens(html: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let b = html.as_bytes();
    let mut i = 0;
    while let Some(at) = html[i..].find('<') {
        i += at + 1;
        let name_end = b[i..]
            .iter()
            .position(|c| !(c.is_ascii_alphanumeric() || *c == b'-' || *c == b'_'))
            .map_or(b.len(), |n| i + n);
        if name_end == i || !b[i].is_ascii_alphabetic() {
            continue;
        }
        out.insert(format!("t:{}", html[i..name_end].to_ascii_lowercase()));
        i = name_end;
        // Its attributes, to the `>` outside a quoted value.
        while i < b.len() && b[i] != b'>' {
            while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
                i += 1;
            }
            let start = i;
            while i < b.len() && !b[i].is_ascii_whitespace() && !b"\"'=<>/".contains(&b[i]) {
                i += 1;
            }
            if i == start {
                if i < b.len() && b[i] != b'>' {
                    i += 1;
                }
                continue;
            }
            let attr = html[start..i].to_lowercase();
            let mut j = i;
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            let mut value = "";
            if j < b.len() && b[j] == b'=' {
                j += 1;
                while j < b.len() && b[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < b.len() && (b[j] == b'"' || b[j] == b'\'') {
                    let end = html[j + 1..].find(b[j] as char).map(|e| j + 1 + e);
                    value = &html[j + 1..end.unwrap_or(b.len())];
                    i = end.map_or(b.len(), |e| e + 1);
                } else {
                    let end = b[j..]
                        .iter()
                        .position(|c| c.is_ascii_whitespace() || *c == b'>')
                        .map_or(b.len(), |e| j + e);
                    value = &html[j..end];
                    i = end;
                }
            }
            if attr == "class" {
                out.extend(value.split_ascii_whitespace().map(|c| format!("c:{c}")));
            }
            if attr == "id" {
                out.insert(format!("i:{value}"));
            }
            if attr == "style" {
                out.extend(vars(value).map(|v| format!("v:{v}")));
            }
            out.insert(format!("a:{attr}"));
        }
    }
    out
}

/// The custom properties `css` reads (`var(--name`).
fn vars(css: &str) -> impl Iterator<Item = &str> {
    css.match_indices("var(").filter_map(move |(at, _)| {
        let rest = css[at + 4..].trim_start();
        let len = rest
            .bytes()
            .position(|b| !(b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
            .unwrap_or(rest.len());
        rest.starts_with("--").then(|| &rest[..len])
    })
}

const ROOT: &str = ":root{";

/// A `:root{…}` rule's declarations, split at the semicolons outside
/// parentheses, brackets and braces.
fn declarations(rule: &str) -> Vec<&str> {
    let body = &rule[ROOT.len()..rule.len() - 1];
    let (mut out, mut depth, mut at, mut quote) = (Vec::new(), 0i32, 0, None);
    let b = body.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match quote {
            Some(_) if c == b'\\' => i += 1,
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                b'"' | b'\'' => quote = Some(c),
                b'\\' => i += 1,
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b';' if depth == 0 => {
                    out.push(body[at..i].trim());
                    at = i + 1;
                }
                _ => {}
            },
        }
        i += 1;
    }
    out.push(body[at..].trim());
    out.retain(|d| !d.is_empty());
    out
}

/// The rules a document offering `have` can need, as one sheet
/// (`page-style.mjs` `select`): a `:root` rule keeps only the custom
/// properties the kept rules or the document's inline styles read, and
/// those they read in turn; one left with nothing goes.
pub(crate) fn select(rules: &[Rule], have: &HashSet<String>) -> String {
    let kept: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.needs.iter().any(|a| a.iter().all(|t| have.contains(t))))
        .collect();
    let mut read: HashSet<String> = have
        .iter()
        .filter_map(|t| t.strip_prefix("v:").map(str::to_owned))
        .collect();
    let mut declared: std::collections::HashMap<&str, String> = Default::default();
    for rule in &kept {
        if rule.rule.starts_with(ROOT) {
            for d in declarations(&rule.rule)
                .into_iter()
                .filter(|d| d.starts_with("--"))
            {
                let name = &d[..d.find(':').unwrap_or(d.len())];
                declared.entry(name).or_default().push_str(d);
            }
        } else {
            read.extend(vars(&rule.rule).map(str::to_owned));
        }
    }
    let mut queue: Vec<String> = read.iter().cloned().collect();
    while let Some(name) = queue.pop() {
        for v in declared
            .get(name.as_str())
            .map(|d| vars(d).collect::<Vec<_>>())
            .unwrap_or_default()
        {
            if read.insert(v.to_owned()) {
                queue.push(v.to_owned());
            }
        }
    }
    let depth = |wrap: &str| wrap.matches('{').count();
    let mut out = String::new();
    let mut open = "";
    for rule in kept {
        let text = if rule.rule.starts_with(ROOT) {
            let kept: Vec<&str> = declarations(&rule.rule)
                .into_iter()
                .filter(|d| {
                    !d.starts_with("--") || read.contains(&d[..d.find(':').unwrap_or(d.len())])
                })
                .collect();
            format!("{ROOT}{}}}", kept.join(";"))
        } else {
            rule.rule.clone()
        };
        if text == format!("{ROOT}}}") {
            continue;
        }
        if rule.wrap != open {
            out.push_str(&"}".repeat(depth(open)));
            out.push_str(&rule.wrap);
            open = &rule.wrap;
        }
        out.push_str(&text);
    }
    out.push_str(&"}".repeat(depth(open)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same sheet and documents as `page-style.test.mjs`: the two
    /// renderers select alike.
    #[test]
    fn a_page_keeps_the_rules_its_markup_can_meet_and_the_custom_properties_they_read() {
        let rules = parse(r#"[["",":root{--a:red;--b:var(--a);--c:blue;--d:green}",[[]]],["","p{color:var(--b)}",[["t:p"]]],["@media (hover:hover){:where(html:not([data-x])){","button:hover{color:var(--c)}",[["t:button"]]],["@media (hover:hover){:where(html:not([data-x])){","dialog{color:red}",[["t:dialog"]]],["",".c9{x:1}",[["c:c9"]]]]"#).unwrap();
        assert_eq!(
            select(&rules, &tokens(r#"<p title="a>b">x</p><button>y</button>"#)),
            ":root{--a:red;--b:var(--a);--c:blue}p{color:var(--b)}@media (hover:hover){:where(html:not([data-x])){button:hover{color:var(--c)}}}"
        );
        assert_eq!(
            select(&rules, &tokens(r#"<p style="margin:var(--d)">x</p>"#)),
            ":root{--a:red;--b:var(--a);--d:green}p{color:var(--b)}"
        );
        assert_eq!(select(&rules, &tokens(r#"<b class="c9">"#)), ".c9{x:1}");
    }
}
