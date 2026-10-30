//! End-to-end migration of retired dialect spellings.

use super::diagnostics::{diagnostics, App};
use std::process::Command;

#[test]
fn logical_words_are_refused_and_migrate_without_changing_meaning_or_literals() {
    let before = r#"// and or not stay in prose.
shape Words
  and: bool
  or: bool
  not: bool
component App
  state words = Words(and=true, or=false, not=false)
  derive truth = not words.not and words.and or words.or
  derive guarded = false and (1 / 0 > 0) or true
  view
    column
      text `é and ${not false and truth} or ${words.or}`
      text "and or not"
"#;
    let app = App::new("logical-words");
    let path = app.write("app.contract", before);
    let all = diagnostics(&app.run(&["app.contract", "--json"]), 1);
    assert_eq!(all.len(), 7, "{all:?}");
    assert!(all.iter().all(|e| e["id"] == "syntax-operator-spelling"));
    let migrated = Command::new(env!("CARGO_BIN_EXE_contract"))
        .arg("migrate")
        .arg(&path)
        .output()
        .unwrap();
    assert!(migrated.status.success(), "{migrated:?}");
    let after = std::fs::read_to_string(&path).unwrap();
    let expected = before
        .replace(
            "not words.not and words.and or words.or",
            "! words.not && words.and || words.or",
        )
        .replace(
            "false and (1 / 0 > 0) or true",
            "false && (1 / 0 > 0) || true",
        )
        .replace("${not false and truth}", "${! false && truth}");
    assert_eq!(after, expected);
    assert_eq!(contract_syntax::migrate::migrate(&after).unwrap(), after);
    contract::compile(&after).unwrap();
    assert!(diagnostics(&app.run(&["app.contract", "--json"]), 0).is_empty());
}

#[test]
fn retired_spellings_are_reported_together_and_migrated_in_one_command() {
    let app = App::new("migrate");
    let before = r#"// list<string>, trim(x), when and each in comments stay.
shape Board
  values: list<list<string>>
component App
  state value = " each "
  state xs: list<string> = ["a"]
  derive count = length(xs)
  derive title = trim(toUpperCase(value))
  view
    column
      when count > 0
        each x in xs key=x
          text `${trim(x)} when list<string>`
      else when count == 0
        text "trim(x)"
"#;
    let path = app.write("app.contract", before);
    let all = diagnostics(&app.run(&["app.contract", "--json"]), 1);
    assert_eq!(all.len(), 10, "{all:?}");
    for id in [
        "syntax-list-type",
        "syntax-method-form",
        "syntax-view-keyword",
        "syntax-else-keyword",
    ] {
        assert!(all.iter().any(|e| e["id"] == id), "{all:?}");
    }
    let migrated = Command::new(env!("CARGO_BIN_EXE_contract"))
        .arg("migrate")
        .arg(&path)
        .output()
        .unwrap();
    assert!(migrated.status.success(), "{migrated:?}");
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.contains("// list<string>, trim(x), when and each in comments stay."));
    assert!(after.contains("values: string[][]"));
    assert!(after.contains(" when list<string>`"));
    let expected = r#"shape Board
  values: string[][]
component App
  state value = " each "
  state xs: string[] = ["a"]
  derive count = xs.length
  derive title = value.toUpperCase().trim()
  view
    column
      if count > 0
        for x in xs key=x
          text `${x.trim()} when list<string>`
      else if count == 0
        text "trim(x)"
"#;
    assert_eq!(
        contract::compile(&after).unwrap().encode(),
        contract::compile(expected).unwrap().encode()
    );
    assert!(diagnostics(&app.run(&["app.contract", "--json"]), 0).is_empty());
    assert_eq!(contract_syntax::migrate::migrate(&after).unwrap(), after);
    // A custom function is a function, including inside a template.
    let custom =
        "fn trim(x: string): string = x\ncomponent App\n  view\n    text `${trim(\"a\")}`\n";
    assert_eq!(contract_syntax::migrate::migrate(custom).unwrap(), custom);
    // Another malformed construct prevents a write even if earlier edits were safe.
    app.write("app.contract", &(before.to_owned() + "broken\n"));
    let failed = Command::new(env!("CARGO_BIN_EXE_contract"))
        .arg("migrate")
        .arg(&path)
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        before.to_owned() + "broken\n"
    );
}

#[test]
fn retired_spellings_are_not_cut_off_at_the_diagnostic_limit() {
    let app = App::new("many-retired");
    let mut src = "component App\n".to_string();
    for i in 0..35 {
        src.push_str(&format!("  derive value{i} = trim(\"a\")\n"));
    }
    src.push_str("  view\n    text value0\n");
    app.write("app.contract", &src);
    let all = diagnostics(&app.run(&["app.contract", "--json"]), 1);
    assert_eq!(all.len(), 35);
    assert!(all.iter().all(|e| e["id"] == "syntax-method-form"));
    contract::compile(&contract_syntax::migrate::migrate(&src).unwrap()).unwrap();
}

#[test]
fn migration_preserves_compound_receivers_multiline_arguments_and_utf8_templates() {
    let before = r#"component App
  state text = "é a"
  derive clipped = slice(
    true ? trim(text) : "x",
    0, 1)
  derive joined = join(map(["a", "b"], x => trim(x)), ",")
  view
    text `é ${length(trim(text))} ${includes(text, "a")}`
"#;
    let expected = r#"component App
  state text = "é a"
  derive clipped = (true ? text.trim() : "x").slice(0, 1)
  derive joined = ["a", "b"].map(x => x.trim()).join(",")
  view
    text `é ${text.trim().length} ${text.includes("a")}`
"#;
    let after = contract_syntax::migrate::migrate(before).unwrap();
    assert_eq!(
        contract::compile(&after).unwrap().encode(),
        contract::compile(expected).unwrap().encode()
    );
}

#[test]
fn migration_preserves_call_and_type_trivia_and_accepts_trailing_commas() {
    let before = r#"fn first(xs: list // type-name trivia
  <list // inner-type-name trivia
    <string>>): string = (trim // function-name trivia
      (" é ", // receiver trailing trivia
      ))
component App
  state value = " é "
  derive size = length(value,)
  derive nested = (length // outer-name trivia
    (trim // inner-name trivia
      (value, // inner trailing trivia
      ), // outer trailing trivia
    ))
  derive piece = slice(value, 0, 1, // argument trailing trivia
  )
  view
    text `${size} ${nested} ${piece}`
"#;
    let expected = r#"fn first(xs: string[][]): string = " é ".trim()
component App
  state value = " é "
  derive size = value.length
  derive nested = value.trim().length
  derive piece = value.slice(0, 1)
  view
    text `${size} ${nested} ${piece}`
"#;
    let after = contract_syntax::migrate::migrate(before).unwrap();
    let comments = |source: &str| {
        source
            .lines()
            .filter_map(|line| line.split_once("//").map(|(_, text)| text.to_owned()))
            .collect::<Vec<_>>()
    };
    assert_eq!(comments(&after), comments(before));
    assert_eq!(
        contract::compile(&after).unwrap().encode(),
        contract::compile(expected).unwrap().encode()
    );
    assert_eq!(contract_syntax::migrate::migrate(&after).unwrap(), after);
}
