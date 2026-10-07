//! The VS Code grammar's word lists (`editors/vscode/syntaxes/
//! exact-contract.tmLanguage.json`) are the compiler's: the built-in tags,
//! the event handlers and the host commands. The grammar is written by hand;
//! a word the compiler gains or drops fails here, by name.

use std::collections::BTreeSet;

fn grammar() -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../editors/vscode/syntaxes/exact-contract.tmLanguage.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The words of the first `(…)` or `(?:…)` alternation in the `match` of the
/// rule in `repository.<rule>` whose scope `name` starts with `scope` (any
/// rule when `scope` is empty).
fn words(grammar: &serde_json::Value, rule: &str, scope: &str) -> BTreeSet<String> {
    let patterns = grammar["repository"][rule]["patterns"]
        .as_array()
        .unwrap_or_else(|| panic!("no rule {rule}"));
    let pattern = patterns
        .iter()
        .find(|p| p["name"].as_str().unwrap_or("").starts_with(scope))
        .unwrap_or_else(|| panic!("no {scope} pattern in {rule}"));
    let source = pattern["match"]
        .as_str()
        .unwrap_or_else(|| panic!("{rule}: no match"));
    let open = source
        .find("(?:")
        .map(|i| i + 3)
        .or_else(|| source.find('(').map(|i| i + 1))
        .expect("an alternation");
    let close = open + source[open..].find(')').expect("the alternation's end");
    source[open..close].split('|').map(str::to_string).collect()
}

fn same(what: &str, grammar: BTreeSet<String>, compiler: impl IntoIterator<Item = String>) {
    let compiler: BTreeSet<String> = compiler.into_iter().collect();
    let missing: Vec<_> = compiler.difference(&grammar).collect();
    let stale: Vec<_> = grammar.difference(&compiler).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "the VS Code grammar's {what}: missing {missing:?}, no longer the compiler's {stale:?}"
    );
}

#[test]
fn tags_are_the_compilers() {
    let tags = contract_lower::vocab::tags()
        .into_iter()
        .map(|(n, _)| n.to_string());
    same("built-in tags", words(&grammar(), "view-tags", ""), tags);
}

#[test]
fn events_are_the_compilers() {
    let handlers = contract_analyze::HANDLERS.iter().map(|h| h.to_string());
    same(
        "events",
        words(&grammar(), "attributes", "support.function.event"),
        handlers,
    );
}

#[test]
fn host_commands_are_the_compilers() {
    let commands = contract_syntax::HOST_COMMANDS.iter().map(|c| c.to_string());
    same(
        "host commands",
        words(&grammar(), "commands", "support.function.builtin"),
        commands,
    );
}
