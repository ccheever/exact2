//! The card (LLP 1116.010 D1): `contract vocab` with no name. Round 12's
//! builders made eleven to fifteen lookups each, one call at a time, for what
//! a React builder already knows: which functions exist, what a tag takes,
//! what a part's call looks like. The card is that in one read, at most
//! 8 KB, generated from the tables so it cannot drift: the
//! functions from the roster and the grammar's tables (`functions.rs`), the
//! parts from the calls beside `exact:content`'s components, the test steps
//! from the recipe's table, the agent's operations from the help text, the
//! symbols from the schema. What no table says is `card.md`, which the tests
//! check against them.

use contract::functions::{self, Function};
use std::collections::BTreeMap;

const SOURCE: &str = include_str!("card.md");
const TESTS: &str = include_str!("../../../../docs/recipes/tests.md");
const HELP: &str = include_str!("../../../../scripts/help.mjs");

/// A section of `card.md`: its body, after `## <name>`.
fn section(name: &str) -> &'static str {
    SOURCE
        .split("\n## ")
        .skip(1)
        .find_map(|s| {
            let (head, body) = s.split_once('\n')?;
            (head.trim() == name).then_some(body.trim())
        })
        .unwrap_or_else(|| panic!("card.md has no section `{name}`"))
}

/// `name: text` lines of the `functions` section.
fn overrides() -> BTreeMap<&'static str, &'static str> {
    section("functions")
        .lines()
        .filter_map(|l| l.split_once(": "))
        .collect()
}

/// One function's line: the grammar's call and the roster's result (else the
/// type its rule opens with), or the card's own words in its place or after it.
fn function_line(f: &Function, over: &BTreeMap<&str, &str>) -> String {
    let words = over.get(f.name.as_str());
    if let Some(text) = words.filter(|t| !t.starts_with('(')) {
        return (*text).to_owned();
    }
    let row = &f.rows[0];
    let call = row.call.replace('`', "");
    let signature = f.signature.as_deref().unwrap_or_default();
    let mut result = signature
        .rsplit_once(" -> ")
        .map_or("", |(_, r)| r)
        .to_owned();
    if result == "any" {
        let head = row.rule.split([';', ':']).next().unwrap_or_default();
        if head.starts_with('`') && head.ends_with('`') {
            result = head.replace('`', "");
        }
    }
    let mut line = format!("{call} -> {result}");
    if let Some(text) = words {
        line += &format!("  {text}");
    }
    line
}

/// Every standard function's line, in name order, and the intrinsics' calls.
pub fn function_lines() -> (Vec<String>, String) {
    let over = overrides();
    let (mut standard, intrinsic): (Vec<Function>, Vec<Function>) = functions::functions()
        .into_iter()
        .partition(|f| f.signature.is_some());
    standard.sort_by_key(|f| f.name.to_lowercase());
    let lines = standard.iter().map(|f| function_line(f, &over)).collect();
    let intrinsics = intrinsic
        .iter()
        .filter_map(|f| f.rows.first())
        .map(|r| r.call.replace('`', ""))
        .collect::<Vec<_>>()
        .join("  ");
    (lines, intrinsics)
}

/// A `tags` line: the tags, their attributes, their events.
pub struct TagLine {
    pub tags: Vec<&'static str>,
    pub attrs: Vec<&'static str>,
    pub events: Vec<&'static str>,
}

pub fn tag_lines() -> Vec<TagLine> {
    let words = |s: Option<&'static str>| s.unwrap_or_default().split_whitespace().collect();
    section("tags")
        .lines()
        .map(|l| {
            let mut cells = l.split('|');
            TagLine {
                tags: words(cells.next()),
                attrs: words(cells.next()),
                events: words(cells.next()),
            }
        })
        .collect()
}

/// The test steps: the first cell of each row of the recipe's table.
pub fn steps() -> Vec<String> {
    TESTS
        .lines()
        .skip_while(|l| !l.starts_with("| Step |"))
        .skip(2)
        .take_while(|l| l.starts_with('|'))
        .map(|l| {
            let cell = l.trim_start_matches('|').split(" | ").next().unwrap_or(l);
            cell.trim().replace('`', "")
        })
        .collect()
}

/// The agent's operations: `agent --help`'s `Ops` block, from its source.
pub fn operations() -> String {
    let start = HELP
        .find("Ops      ")
        .expect("scripts/help.mjs has the agent's operations");
    let block = &HELP[start..];
    let end = block.find("`,").expect("the operations end the text");
    block[..end].replacen("Ops      ", "ops: ", 1)
}

/// The `exact:content` parts, each as its call.
pub fn part_calls() -> Vec<(String, String)> {
    super::PARTS
        .iter()
        .flat_map(|(_, source)| source.lines())
        .filter_map(|l| l.strip_prefix("component "))
        .filter_map(|name| {
            let call = super::part(name)?.call?;
            Some((name.to_owned(), call))
        })
        .collect()
}

pub fn card() -> String {
    let (functions, intrinsics) = function_lines();
    let mut out = format!("{}\n\n", section("intro"));
    out += "functions: call -> result\n";
    for l in &functions {
        out += &format!("  {l}\n");
    }
    out += &format!("  {intrinsics}\n\n");
    out += "tags: attributes; on events. Every tag takes CSS's style attributes (flex gap padding font-size …) and colour roles (-exact-label); `contract vocab <name>` gives a style's values\n";
    for t in tag_lines() {
        // A tag with a positional value (`text "…"`, `image "…"`) is written with it.
        let names: Vec<String> = t
            .tags
            .iter()
            .map(|n| match contract_lower::tags::tag(n) {
                Some(tag) if tag.positional.is_some() => format!("{n} \"…\""),
                _ => (*n).to_owned(),
            })
            .collect();
        out += &format!("  {}: {}", names.join(" "), t.attrs.join(" "));
        if !t.events.is_empty() {
            out += &format!("; on {}", t.events.join(" "));
        }
        out += "\n";
    }
    out += "\nparts: use X from \"exact:content\"; an option is none or some(…)\n";
    for (_, call) in part_calls() {
        out += &format!("  {call}\n");
    }
    out += "\ntest steps (app.test.contract; every other form: docs/contract-grammar.md)\n";
    for s in steps() {
        out += &format!("  {s}\n");
    }
    out += &format!("\n{}\n\n", operations());
    out += &format!("rules\n{}\n", section("rules"));
    out += &format!(
        "idioms, whole with their code (`contract vocab <name>`): {}\n",
        super::concepts::concepts()
            .iter()
            .map(|c| c.name)
            .collect::<Vec<_>>()
            .join(" ")
    );
    out += &format!(
        "symbol roles: {}\n",
        exact_kernel::generated::SYMBOL_ROLES.join(" ")
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vocab::concepts::Concept;
    use contract_lower::{tags, vocab};

    /// The card's size, in bytes: a builder's context is re-sent at every step.
    const LIMIT: usize = 8 * 1024;

    #[test]
    fn the_card_holds_its_size() {
        let card = card();
        assert!(
            card.len() <= LIMIT,
            "the card is {} bytes, over {LIMIT}: cut words, not sections",
            card.len()
        );
        // A card that is a few lines is not the card.
        assert!(card.len() > LIMIT / 2, "{} bytes", card.len());
    }

    #[test]
    fn every_function_every_part_every_step_and_the_operations_are_on_it() {
        let (lines, intrinsics) = function_lines();
        let standard: Vec<_> = functions::functions()
            .into_iter()
            .filter(|f| f.signature.is_some())
            .collect();
        assert_eq!(lines.len(), standard.len());
        for f in &standard {
            assert!(
                lines.iter().any(|l| l.starts_with(&format!("{}(", f.name))),
                "{} has no line: {lines:#?}",
                f.name
            );
        }
        // A result of `any` says nothing: the card's own words give the type.
        for l in &lines {
            assert!(!l.contains("-> any"), "{l}: give it a line in card.md");
        }
        for f in functions::functions()
            .iter()
            .filter(|f| f.signature.is_none())
        {
            assert!(intrinsics.contains(&format!("{}(", f.name)), "{}", f.name);
        }
        // Each override names a function there is.
        for name in overrides().keys() {
            assert!(
                standard.iter().any(|f| f.name == *name),
                "card.md rewrites `{name}`, which is no standard function"
            );
        }
        let source = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../lib/content.contract"
        ))
        .unwrap();
        let components = source
            .lines()
            .filter(|l| l.starts_with("component "))
            .count();
        assert_eq!(part_calls().len(), components);
        let steps = steps();
        for verb in [
            "tap",
            "type",
            "expect text",
            "clock data",
            "reload",
            "back",
            "fail fetch",
            "request",
        ] {
            assert!(
                steps.iter().any(|s| s.contains(verb)),
                "no step `{verb}`: {steps:#?}"
            );
        }
        let ops = operations();
        for op in [
            "tree",
            "tap <target>",
            "type <target>",
            "clock data",
            "screenshot",
        ] {
            assert!(ops.contains(op), "no operation `{op}`: {ops}");
        }
        let card = card();
        for c in super::super::concepts::concepts() {
            assert!(card.contains(c.name), "{}", c.name);
        }
    }

    #[test]
    fn every_tag_attribute_and_event_the_card_names_is_real() {
        let tag_names: Vec<&str> = vocab::tags().iter().map(|(n, _)| *n).collect();
        let attrs = vocab::attrs();
        let is_event = |n: &str| {
            attrs
                .iter()
                .any(|(a, t)| *a == n && matches!(t, tags::AttrTarget::Handler(_)))
        };
        let is_attr = |n: &str| {
            attrs
                .iter()
                .any(|(a, t)| *a == n && !matches!(t, tags::AttrTarget::Handler(_)))
        };
        let mut failures = Vec::new();
        for line in tag_lines() {
            assert!(!line.tags.is_empty());
            failures.extend(
                line.tags
                    .iter()
                    .filter(|t| !tag_names.contains(t))
                    .map(|t| format!("`{t}` is no tag")),
            );
            failures.extend(
                line.attrs
                    .iter()
                    .filter(|a| !is_attr(a))
                    .map(|a| format!("`{a}` is no attribute (an event? a typo?)")),
            );
            failures.extend(
                line.events
                    .iter()
                    .filter(|e| !is_event(e))
                    .map(|e| format!("`{e}` is no event")),
            );
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn the_names_the_card_says_are_absent_say_what_to_write() {
        let rule = section("rules")
            .lines()
            .find(|l| l.starts_with("- Not in Contract"))
            .expect("the rule that lists the web's names Contract lacks");
        let names = rule.split(": ").nth(1).unwrap().trim_end_matches('.');
        for name in names.split_whitespace() {
            assert!(
                contract_syntax::idioms::refusal(name).is_some(),
                "`{name}` has no refusal saying what to write"
            );
            assert!(
                functions::function(name).is_none() || name == "some",
                "`{name}` is a function now: take it off the card's list"
            );
        }
    }

    #[test]
    fn the_cards_example_compiles_with_no_diagnostic() {
        let rules = Concept {
            name: "card",
            title: "",
            names: Vec::new(),
            answer: section("rules"),
        };
        let blocks = rules.contract();
        assert!(!blocks.is_empty(), "the rules hold an example");
        let dir = std::env::temp_dir().join(format!("exact-card-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.contract");
        let mut failures = Vec::new();
        for body in &blocks {
            std::fs::write(&path, body).unwrap();
            let (compiled, warnings) = contract::compile_path_all_warned(&path, false);
            if let Err(all) = compiled {
                failures.extend(all.iter().map(|e| format!("[{}] {}", e.id, e.message)));
            }
            failures.extend(
                warnings
                    .iter()
                    .map(|w| format!("warning [{}] {}", w.id, w.message)),
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(failures.is_empty(), "{failures:#?}");
    }
}
