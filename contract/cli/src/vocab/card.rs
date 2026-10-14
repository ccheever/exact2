//! The card (LLP 1116.010 D1): `contract vocab` with no name. Round 12's
//! builders made eleven to fifteen lookups each, one call at a time, for what
//! a React builder already knows: which functions exist, what a tag takes,
//! what a part's call looks like. The card is that in one read, at most
//! 12 KB, generated from the tables so it cannot drift: the
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

/// One entry of `card.md`'s `how` section: a title, its rule, and one
/// fenced snippet with the fence's kind (`contract`, `contract-test`, `sh`).
pub struct How {
    pub title: String,
    pub rule: String,
    /// Read by the tests, which check each kind another way.
    #[cfg_attr(not(test), allow(dead_code))]
    pub kind: String,
    pub code: String,
}

pub fn hows() -> Vec<How> {
    section("how")
        .split("\n### ")
        .map(|entry| entry.trim_start_matches("### "))
        .filter(|entry| !entry.trim().is_empty())
        .map(|entry| {
            let (title, rest) = entry.split_once('\n').expect("a how entry has a rule");
            let (mut rule, mut kind, mut code) = (Vec::new(), String::new(), String::new());
            let mut open = false;
            for line in rest.lines() {
                match (open, line.strip_prefix("```")) {
                    (false, Some(info)) => {
                        assert!(kind.is_empty(), "how `{title}` has two snippets");
                        kind = info.trim().to_owned();
                        open = true;
                    }
                    (true, Some("")) => open = false,
                    (true, _) => code += &format!("{line}\n"),
                    (false, None) => rule.push(line.trim()),
                }
            }
            assert!(
                !open && !kind.is_empty(),
                "how `{title}` has no closed snippet"
            );
            How {
                title: title.trim().to_owned(),
                rule: rule.join(" "),
                kind,
                code,
            }
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
    out += "how (snippets compile)\n";
    for h in hows() {
        out += &format!("{}: {}\n", h.title, h.rule);
        for line in h.code.lines() {
            out += &format!("  {line}\n");
        }
    }
    out += &format!("\nrules\n{}\n", section("rules"));
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

    /// The card's size, in bytes: a builder's context is re-sent at every step
    /// (12 KB since LLP 1116.011 D1 added the how-to).
    const LIMIT: usize = 12 * 1024;

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

    /// The lines of a `how` snippet as a whole app: the card shows the lines
    /// that teach, and this puts the frame back. A top-level line opening
    /// `routes`, `shape`, `fn` or `use` stays at the file's level; `state`,
    /// `action`, `resource`, `mutation`, `task` and `derive` go in the
    /// component; any other is the view (one root, else a `column`'s children).
    fn framed(code: &str) -> String {
        let (mut file, mut body, mut view) = (String::new(), String::new(), Vec::new());
        let (mut roots, mut at) = (0, 0);
        for line in code.lines().filter(|l| !l.trim().is_empty()) {
            if !line.starts_with(' ') {
                at = match line.split([' ', '(']).next().unwrap_or_default() {
                    "routes" | "shape" | "fn" | "use" => 0,
                    "state" | "action" | "resource" | "mutation" | "task" | "derive" => 1,
                    _ => {
                        roots += 1;
                        2
                    }
                };
            }
            match at {
                0 => file += &format!("{line}\n"),
                1 => body += &format!("  {line}\n"),
                _ => view.push(line),
            }
        }
        let shown = match roots {
            0 => "    text \"-\"\n".to_owned(),
            1 => view.iter().map(|l| format!("    {l}\n")).collect(),
            _ => format!(
                "    column\n{}",
                view.iter()
                    .map(|l| format!("      {l}\n"))
                    .collect::<String>()
            ),
        };
        format!("{file}\ncomponent App\n{body}  view\n{shown}")
    }

    /// Whether `app` compiles with no error and no warning.
    fn problems(app: &str) -> Vec<String> {
        let dir = std::env::temp_dir().join(format!("exact-how-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.contract");
        std::fs::write(&path, app).unwrap();
        let (compiled, warnings) = contract::compile_path_all_warned(&path, false);
        std::fs::remove_dir_all(&dir).unwrap();
        let mut out: Vec<String> = Vec::new();
        if let Err(all) = compiled {
            out.extend(
                all.iter()
                    .map(|e| format!("[{}] {} (line {})", e.id, e.message, e.span.line)),
            );
        }
        out.extend(
            warnings
                .iter()
                .map(|w| format!("warning [{}] {}", w.id, w.message)),
        );
        out
    }

    #[test]
    fn every_how_to_snippet_compiles_parses_or_names_real_flags() {
        let hows = hows();
        assert!(hows.len() >= 9, "{} how-tos", hows.len());
        let mut failures = Vec::new();
        for h in &hows {
            match h.kind.as_str() {
                "contract" => {
                    let app = framed(&h.code);
                    failures.extend(
                        problems(&app)
                            .into_iter()
                            .map(|p| format!("`{}`: {p}\n{app}", h.title)),
                    );
                }
                "contract-test" => {
                    if let Err(e) = contract::tests(&h.code) {
                        failures.push(format!("`{}`: [{}] {}", h.title, e.id, e.message));
                    }
                }
                other => failures.push(format!("`{}`: a `{other}` snippet has no check", h.title)),
            }
        }
        // The flags a rule names are the verb's.
        let verify = HELP
            .split("verify: {")
            .nth(1)
            .and_then(|s| s.split("usage: '").nth(1))
            .and_then(|s| s.split('\'').next())
            .expect("scripts/help.mjs gives verify's usage");
        let rule = hows
            .iter()
            .find(|h| h.title.contains("verify"))
            .map(|h| h.rule.as_str());
        let rule = rule.expect("a how-to for verify");
        for flag in ["verify ios", "--shots <dir>"] {
            let usage = flag.replace("verify ios", "[web|ios]");
            assert!(rule.contains(flag), "the verify how-to names `{flag}`");
            assert!(verify.contains(&usage), "`{flag}` is not in `{verify}`");
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        // The how-tos the round-13 lookups ask for, by title.
        for want in [
            "screen",
            "sheet",
            "confirmation",
            "timer",
            "keeping a value",
            "colour",
            "data",
            "dated record",
            "test",
            "verify",
            "composing",
        ] {
            assert!(
                hows.iter().any(|h| h.title.contains(want)),
                "no how-to for `{want}`"
            );
        }
    }

    #[test]
    fn the_roles_the_how_to_names_are_colour_roles() {
        let (roles, _) = vocab::color_roles();
        let text: String = hows()
            .iter()
            .map(|h| format!("{}\n{}\n", h.rule, h.code))
            .collect();
        let named: Vec<&str> = text
            .split("-exact-")
            .skip(1)
            .map(|s| {
                s.split(|c: char| !(c.is_ascii_lowercase() || c == '-'))
                    .next()
                    .unwrap()
            })
            .collect();
        assert!(named.len() >= 3, "the colour how-to names roles: {named:?}");
        let bad: Vec<_> = named.iter().filter(|r| !roles.contains(r)).collect();
        assert!(bad.is_empty(), "{bad:?} are no colour role");
        // A role named bare in a rule (`secondary-label`) is one too.
        let colour = hows().into_iter().find(|h| h.title == "colour").unwrap();
        for word in colour.rule.split('`').skip(1).step_by(2) {
            assert!(
                roles.contains(&word)
                    || ["separator", "link"].contains(&word)
                    || word.contains(' '),
                "`{word}` in the colour rule is no role"
            );
        }
    }

    /// `docs/card.md` is the card checked in, for the builders that read
    /// files rather than run commands (LLP 1116.011 D1).
    #[test]
    fn docs_card_md_is_what_vocab_prints() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/card.md");
        let checked_in = std::fs::read_to_string(path).unwrap_or_default();
        assert!(
            checked_in == card(),
            "docs/card.md is not what `contract vocab` prints: regenerate it with\n    bun scripts/exact.mjs contract vocab > docs/card.md\nand stage it"
        );
    }
}
