//! The styling a file carries (LLP 1011.001 D2): `style=""` on an element,
//! and a `style` element's rules with type, class and id selectors and no
//! combinators, which is what a designer's export writes (`.cls-1{fill:…}`).
//! A rule with anything else in its selector is dropped, with a warning.
//! Each declaration is applied as the presentation attribute of the same
//! name: the resolver sees one value per property, in the cascade's order
//! (attribute, then rules by specificity, then the style attribute).

/// A simple selector: every part given must match.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Selector {
    pub tag: Option<String>,
    pub classes: Vec<String>,
    pub id: Option<String>,
}

impl Selector {
    /// (ids, classes, tags): CSS specificity.
    fn specificity(&self) -> (usize, usize, usize) {
        (
            usize::from(self.id.is_some()),
            self.classes.len(),
            usize::from(self.tag.is_some()),
        )
    }

    fn matches(&self, tag: &str, id: Option<&str>, classes: &[&str]) -> bool {
        self.tag.as_deref().is_none_or(|t| t == tag)
            && self.id.as_deref().is_none_or(|i| Some(i) == id)
            && self.classes.iter().all(|c| classes.contains(&c.as_str()))
    }

    /// `tag.class#id` and any order of those; `None` for a selector with a
    /// combinator, a pseudo-class, an attribute test or anything else.
    fn parse(text: &str) -> Option<Selector> {
        let text = text.trim();
        if text.is_empty()
            || text.contains(|c: char| {
                c.is_whitespace() || matches!(c, '>' | '+' | '~' | ':' | '[' | '*' | '(' | ')')
            })
        {
            return None;
        }
        let mut out = Selector::default();
        let mut rest = text;
        while !rest.is_empty() {
            let (kind, body) = match rest.chars().next()? {
                '.' => ('.', &rest[1..]),
                '#' => ('#', &rest[1..]),
                _ => ('t', rest),
            };
            let end = body.find(['.', '#']).unwrap_or(body.len());
            let name = &body[..end];
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
            {
                return None;
            }
            match kind {
                '.' => out.classes.push(name.to_string()),
                '#' => {
                    if out.id.is_some() {
                        return None;
                    }
                    out.id = Some(name.to_string());
                }
                _ => {
                    if out.tag.is_some() {
                        return None;
                    }
                    out.tag = Some(name.to_string());
                }
            }
            rest = &body[end..];
        }
        Some(out)
    }
}

/// One rule of a `style` element.
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub selector: Selector,
    pub declarations: Vec<(String, String)>,
    /// The rule's place in the sheet, for the cascade among equals.
    order: usize,
}

/// The rules of a `style` element's text, in sheet order; `dropped` gets
/// each selector the reader does not take.
pub fn rules(text: &str, dropped: &mut Vec<String>) -> Vec<Rule> {
    let mut out = Vec::new();
    let mut rest = strip_comments(text);
    let mut order = 0;
    loop {
        // An at-rule with no block (`@import url(x);`, `@charset`): an
        // external reference or nothing a picture reads, dropped by name.
        let lead = rest.trim_start();
        if lead.starts_with('@') {
            let semicolon = lead.find(';');
            let brace = lead.find('{');
            if let Some(end) = semicolon.filter(|s| brace.is_none_or(|b| *s < b)) {
                dropped.push(lead[..end].trim().to_string());
                rest = lead[end + 1..].to_string();
                continue;
            }
        }
        let Some(open) = rest.find('{') else { break };
        let selectors = rest[..open].trim().to_string();
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let body = rest[open + 1..open + close].to_string();
        rest = rest[open + close + 1..].to_string();
        // An at-rule (`@import`, `@font-face`, `@media`) is not read: an
        // import is an external reference, and the rest has no place in a
        // picture.
        if selectors.starts_with('@') {
            dropped.push(selectors);
            continue;
        }
        let declarations = declarations(&body);
        for part in selectors.split(',') {
            match Selector::parse(part) {
                Some(selector) => out.push(Rule {
                    selector,
                    declarations: declarations.clone(),
                    order,
                }),
                None => dropped.push(part.trim().to_string()),
            }
            order += 1;
        }
    }
    out
}

/// `a: b; c: d` as pairs, names lowercased and values trimmed; `!important`
/// is read as the value without it.
pub fn declarations(text: &str) -> Vec<(String, String)> {
    text.split(';')
        .filter_map(|d| {
            let (name, value) = d.split_once(':')?;
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim().trim_end_matches("!important").trim();
            (!name.is_empty() && !value.is_empty()).then(|| (name, value.to_string()))
        })
        .collect()
}

/// The declarations of every rule matching an element, in cascade order
/// (specificity, then sheet order), so a later entry overrides an earlier.
pub fn matching<'r>(
    rules: &'r [Rule],
    tag: &str,
    id: Option<&str>,
    classes: &[&str],
) -> Vec<&'r (String, String)> {
    let mut hits: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.selector.matches(tag, id, classes))
        .collect();
    hits.sort_by_key(|r| (r.selector.specificity(), r.order));
    hits.iter().flat_map(|r| r.declarations.iter()).collect()
}

fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("/*") {
        out.push_str(&rest[..i]);
        match rest[i + 2..].find("*/") {
            Some(j) => rest = &rest[i + 2 + j + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exports_classes_cascade_by_specificity_then_order() {
        let mut dropped = Vec::new();
        let sheet = rules(".cls-1{fill:#f00;stroke:none}\n/* c */ path{fill:#0f0}\n#mark{fill:#00f !important}\n.cls-1.cls-2{fill:#ff0}\ng > path{fill:#000}\n@import url(x.css);", &mut dropped);
        assert_eq!(
            dropped,
            vec!["g > path".to_string(), "@import url(x.css)".to_string()]
        );
        let hits = matching(&sheet, "path", Some("mark"), &["cls-1", "cls-2"]);
        let fills: Vec<&str> = hits
            .iter()
            .filter(|(n, _)| n == "fill")
            .map(|(_, v)| v.as_str())
            .collect();
        assert_eq!(fills, vec!["#0f0", "#f00", "#ff0", "#00f"]);
        assert_eq!(matching(&sheet, "rect", None, &["cls-1"]).len(), 2);
        assert!(matching(&sheet, "rect", None, &[]).is_empty());
    }

    #[test]
    fn a_style_attribute_splits_into_declarations() {
        assert_eq!(
            declarations("fill:red; stroke-width: 2 ;;bad;"),
            vec![
                ("fill".to_string(), "red".to_string()),
                ("stroke-width".to_string(), "2".to_string())
            ]
        );
    }
}
