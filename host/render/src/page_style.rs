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
            out.insert(format!("a:{attr}"));
        }
    }
    out
}

/// The rules a document offering `have` can need, as one sheet
/// (`page-style.mjs` `select`).
pub(crate) fn select(rules: &[Rule], have: &HashSet<String>) -> String {
    let depth = |wrap: &str| wrap.matches('{').count();
    let mut out = String::new();
    let mut open = "";
    for rule in rules {
        if !rule
            .needs
            .iter()
            .any(|a| a.iter().all(|t| have.contains(t)))
        {
            continue;
        }
        if rule.wrap != open {
            out.push_str(&"}".repeat(depth(open)));
            out.push_str(&rule.wrap);
            open = &rule.wrap;
        }
        out.push_str(&rule.rule);
    }
    out.push_str(&"}".repeat(depth(open)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_keeps_the_rules_its_markup_can_meet_in_order() {
        let rules = parse(
            r#"[["","p{a:1}",[[]]],["","button[data-x]{b:2}",[["t:button","a:data-x"]]],["@media (hover:hover){",".c1:hover{c:3}",[["c:c1"]]],["@media (hover:hover){","dialog{d:4}",[["t:dialog"]]]]"#,
        )
        .unwrap();
        let have = tokens(r#"<div class="c0 c1" title="a>b"><button data-x>Go</button></div>"#);
        assert!(have.contains("t:button") && have.contains("a:data-x") && have.contains("c:c1"));
        assert!(have.contains("a:title") && !have.contains("t:b"));
        assert_eq!(
            select(&rules, &have),
            "p{a:1}button[data-x]{b:2}@media (hover:hover){.c1:hover{c:3}}"
        );
        assert_eq!(select(&rules, &tokens("<p>")), "p{a:1}");
    }
}
