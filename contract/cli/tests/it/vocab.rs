//! LLP 1086 D3: `contract vocab` lists what the compiler admits, from its
//! own lookups, and refuses a name as the compiler would.

use serde_json::Value;
use std::process::{Command, Output};

fn vocab(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_contract"))
        .arg("vocab")
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn a_style_shows_its_rows_codec_and_default() {
    let out = stdout(&vocab(&["padding"]));
    assert!(out.contains("padding: style attribute"), "{out}");
    assert!(out.contains("padding_top"), "{out}");
    assert!(out.contains("dimension, default 0"), "{out}");
}

#[test]
fn a_handler_and_a_tag_say_what_they_are() {
    assert!(stdout(&vocab(&["press"])).contains("press: handler attribute"));
    let column = stdout(&vocab(&["column"]));
    assert!(column.contains("column: tag, View"), "{column}");
    assert!(column.contains("flex_direction: column"), "{column}");
}

#[test]
fn an_unknown_name_gets_the_compilers_suggestion_and_exit_1() {
    let output = vocab(&["paddin"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let err = String::from_utf8(output.stderr).unwrap();
    assert!(err.contains("did you mean `padding`?"), "{err}");
    let output = vocab(&["fontSize"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("`font-size`"));
}

#[test]
fn json_lists_everything_and_one_entry() {
    let doc: Value = serde_json::from_str(&stdout(&vocab(&["--json"]))).unwrap();
    let names = |key: &str| -> Vec<String> {
        doc[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["name"].as_str().unwrap().to_owned())
            .collect()
    };
    assert!(names("tags").contains(&"column".to_owned()));
    assert!(!names("tags").contains(&"flex".to_owned()));
    assert!(names("attributes").contains(&"padding".to_owned()));
    assert!(names("renamed").contains(&"fontSize".to_owned()));
    let one: Value = serde_json::from_str(&stdout(&vocab(&["--json", "align-items"]))).unwrap();
    assert_eq!(one["attribute"]["kind"], "style");
    assert_eq!(one["attribute"]["rows"][0]["default"], "normal");
    assert!(
        one["attribute"]["rows"][0]["values"]
            .as_array()
            .unwrap()
            .len()
            > 3
    );
}

/// The listed contextual restrictions are the compiler's: each is refused
/// on a tag outside its list.
#[test]
fn contextual_attributes_are_refused_elsewhere() {
    for (tag, attr) in [
        ("view", "sandbox=\"allow-scripts\""),
        ("view", "src=\"https://example.com\""),
        ("view", "document=true"),
        ("input", "text-transform=\"uppercase\""),
        ("view", "robots=\"x\""),
    ] {
        let source = format!("component App\n  view\n    {tag} {attr}\n");
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, "lower-attr-tag", "{tag} {attr}: {error}");
    }
}
