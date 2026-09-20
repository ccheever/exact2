//! Source preservation is independent of AST equivalence: the AST discards
//! comments and opaque contract bodies, and numbers lose their spelling.
use contract_syntax::{fmt::format, Lexer, TokenKind};

fn trivia(src: &str) -> Vec<String> {
    let tokens = Lexer::tokenize(src, 1).unwrap();
    let mut count = 0;
    let mut out = Vec::new();
    for (i, line) in src.lines().enumerate() {
        let code: Vec<_> = tokens
            .iter()
            .filter(|t| {
                t.span.line as usize == i + 1
                    && !matches!(
                        t.kind,
                        TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent | TokenKind::Eof
                    )
            })
            .collect();
        count += code.len();
        if line.trim().is_empty() {
            out.push(format!("blank:{count}"));
        } else {
            let suffix = code
                .last()
                .map_or(line, |t| &line[t.end_col as usize - 1..]);
            if suffix.trim_start().starts_with("//") {
                out.push(format!(
                    "comment:{count}:{}:{}",
                    !code.is_empty(),
                    suffix.trim_start()
                ));
            }
        }
    }
    out
}

fn preserved(src: &str) -> String {
    let after = format(src).unwrap();
    assert_eq!(
        trivia(src),
        trivia(&after),
        "comment anchors or blanks changed:\n{after}"
    );
    assert_eq!(format(&after).unwrap(), after, "not idempotent");
    after
}

#[test]
fn comments_on_branches_continuations_and_arguments_keep_their_anchors() {
    let src = r#"
// header
component A
    state n=0 // state
    action go writes n
        if n > 0
            n=0
        // the other case
        else // else comment
            n=1
        match some(n)
            case some(x) // some statement
                n=x
            case none // none statement
                n=2
    view
        when n > 0
            button "Go" press=go // header attribute
                testId="go" // continued attribute
                aria-label="A very long aria label that will make this element need attribute wrapping" // last attribute
                // child
                text "child"
        else // view else
            match some(n)
                case some(x) // some node
                    text x
                case none // none node
                    text "none"
        Row( // use header
            first="A rather long argument with a // string and escaped \" quote", // first argument
            // second argument's comment
            second="another long argument value that cannot fit on the same line" // second argument
        ) // close use

// trailer

"#;
    let after = preserved(src);
    assert!(after.contains("    else // else comment"));
    assert!(after.contains("      case some(x) // some statement"));
    assert!(after.contains("      testId=\"go\" // continued attribute"));
    assert!(after.contains("    ) // close use"));
}

#[test]
fn raw_literals_and_opaque_contract_bodies_survive() {
    let huge = "9".repeat(400); // lexer accepts an overflowing decimal
    let src = format!(
        r#"
font "Brand" = "fonts/brand.ttf"
routes screen
    home "/"
        story "/:id"
    notfound
component A
    state n = 0001.00
    derive enormous = {huge}
    derive sub = n - n
    derive property = 1 . field
    derive message = `unchanged ${{n+2}} ${{`nested ${{"}}"}}`}}`
    contract
        has   text "a\t\n\`\$"  // opaque spacing stays
        opaque   words
    view
        text "a\t\n\`\$" font-size=12
"#
    );
    let after = preserved(&src);
    assert!(after.contains("0001.00"));
    assert!(after.contains(&huge));
    assert!(after.contains("1 .field"));
    assert!(after.contains(
        "      has   text \"a\\t\\n\\`\\$\"  // opaque spacing stays\n      opaque   words\n"
    ));
    assert!(after.contains("`unchanged ${n+2} ${`nested ${\"}\"}`}`"));
}

#[test]
fn long_headers_break_only_at_parser_attribute_and_argument_boundaries() {
    let src = "component A\n  view\n    input value=\"\" placeholder=\"A long placeholder for a field\" aria-label=\"A long label for the field\" testId=\"field\"\n    Row(first=\"a long argument value here\", second=\"another long argument value\", third=\"and a third one\")\n";
    let expected = "component A\n  view\n    input value=\"\"\n      placeholder=\"A long placeholder for a field\"\n      aria-label=\"A long label for the field\"\n      testId=\"field\"\n    Row(\n      first=\"a long argument value here\",\n      second=\"another long argument value\",\n      third=\"and a third one\"\n    )\n";
    assert_eq!(preserved(src), expected);
    let multiline = "component A\n  view\n    text \"x\" width=(\n        1+2\n      )\n      height=30\n      opacity=(true ? 1 : 0) // last\n";
    preserved(multiline);
    preserved("component A\n  view\n    input value=\"a very long first attribute value to make this header wrap at one hundred columns\" width=(\n        1+2\n      ) height=30\n");
}

#[test]
fn empty_files_blank_groups_and_eof_are_stable() {
    for src in [
        "",
        "\n",
        "\n\n",
        "// only",
        "\n// only\n\n\n",
        "component A\n  view\n    text \"x\"",
    ] {
        let after = preserved(src);
        assert_eq!(
            after,
            if src.is_empty() || src.ends_with('\n') {
                src.to_owned()
            } else {
                format!("{src}\n")
            }
        );
    }
}

#[test]
fn nested_multiline_values_stay_stable_when_attribute_wrapping_adds_a_level() {
    for width in [2, 4, 7] {
        let indent = " ".repeat(width);
        for closes in [")))", ")\nCLOSE))"] {
            let src = format!("component A\n{indent}view\n{indent}{indent}text \"x\" label=\"{}\" width=(outer(\nDEEPinner(\nDEEPEST1+2 // sum\nCLOSE{closes} height=20\n", "long".repeat(30))
                .replace("DEEPEST", &indent.repeat(5))
                .replace("DEEP", &indent.repeat(4))
                .replace("CLOSE", &indent.repeat(3));
            preserved(&src);
        }
    }
}

#[test]
fn authored_test_steps_and_nested_types_keep_their_values() {
    let src = r#"shape Record
  values:list<option<string>>
fn choose(x:option<number>):number = match x { case some(n) => n, case none => -1 }
test "é test"
  tap "open" // click
  type "input" "😃\n\t"
  type "input" key "Enter"
  clock +120
  clock settle
  expect state value == 1
  expect text "result" == "hello"
  expect tree has "result"
  screenshot "result.png"
"#;
    let after = preserved(src);
    assert!(after.contains("values: list<option<string>>"));
    let original = contract_syntax::parse(src).unwrap();
    let formatted = contract_syntax::parse(&after).unwrap();
    assert_eq!(
        original.tests[0].steps.len(),
        formatted.tests[0].steps.len()
    );
}

#[test]
fn positionals_after_named_attributes_stay_on_the_elements_head() {
    let long = "A long accessible label that pushes this otherwise valid element header beyond one hundred columns";
    for tag in ["text", "button"] {
        let src = format!("component A\n  view\n    {tag} width=120 aria-label=\"{long}\" \"label\" height=40 opacity=1\n");
        let after = preserved(&src);
        assert!(
            after.contains(&format!(
                "{tag} width=120 aria-label=\"{long}\" \"label\"\n      height=40\n      opacity=1"
            )),
            "{after}"
        );
    }
}
