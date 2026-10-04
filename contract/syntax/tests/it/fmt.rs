//! Source preservation is independent of AST equivalence: the AST discards
//! comments, and numbers lose their spelling.
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
                .map_or(line, |t| &line[t.span.end_col as usize - 1..]);
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
fn resource_context_preserves_the_call_boundary_nested_values_and_else() {
    use contract_syntax::{parse, Span, VisitSpans};
    let src = r#"component App
    resource plain=source(car,minute) as shape Status
    resource one=source(car) with minute as shape Status
    resource zero=source() with (minute+1) as shape Status
    resource many=source(car,account) with max(minute,1),some(clock) as shape Status else preview("cached") // stale
    resource nested=source(car) with max(
        minute, // current sample
        1
    ),(ready ? 2 : 3) as shape Status
    view
        text "ready"
"#;
    let formatted = preserved(src);
    assert!(formatted.contains("source() with (minute + 1) as shape Status"));
    assert!(formatted.contains("source(car, account) with max(minute, 1), some(clock) as shape Status else preview(\"cached\")"));
    let mut before = parse(src).unwrap();
    let mut after = parse(&formatted).unwrap();
    before.visit_spans(&mut |span| *span = Span::point(1, 1));
    after.visit_spans(&mut |span| *span = Span::point(1, 1));
    assert_eq!(before, after);
}

#[test]
fn comments_on_branches_continuations_and_arguments_keep_their_anchors() {
    let src = r#"
// header
component A
    state n=0 // state
    action go
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
fn raw_literals_survive() {
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
    view
        text "a\t\n\`\$" font-size=12
"#
    );
    let after = preserved(&src);
    assert!(after.contains("0001.00"));
    assert!(after.contains(&huge));
    assert!(after.contains("1 .field"));
    assert!(after.contains(r#"text "a\t\n\`\$" font-size=12"#));
    assert!(after.contains("`unchanged ${n+2} ${`nested ${\"}\"}`}`"));
}

#[test]
fn template_escapes_keep_their_spelling_and_meaning() {
    use contract_syntax::parse;
    let src =
        "component A\n  state x = 1\n  view\n    text   `a\\${x} \\` \\\\${x} \\t\\n\\\"\\$`\n";
    let after = preserved(src);
    assert!(
        after.contains(r#"text `a\${x} \` \\${x} \t\n\"\$`"#),
        "{after}"
    );
    // Only the spacing before the template changed; its escapes still decode.
    assert_eq!(after, src.replace("text   `", "text `"));
    let view = format!("{:?}", parse(&after).unwrap().components[0].view);
    assert!(view.contains(r#"Text("a${x} ` \\")"#), "{view}");
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
fn an_empty_list_is_spelled_without_a_space() {
    let src = "component A\n  state flag = true\n  derive xs = flag ? [ ] : []\n  action clear\n    picked = [   ]\n  state picked = []\n  view\n    text join(xs, \",\")\n";
    let after = preserved(src);
    assert!(!after.contains("[ "), "{after}");
    assert_eq!(after.matches("[]").count(), 4, "{after}");
    assert!(after.contains("    picked = []\n"), "{after}");
    // Only spans move; every `[]` is still the empty list.
    let empties = |src: &str| {
        let file = contract_syntax::parse(src).unwrap();
        let c = &file.components[0];
        (
            matches!(c.states[1].expr, contract_syntax::Expr::EmptyList(_)),
            matches!(
                &c.derives[0].expr,
                contract_syntax::Expr::Ternary(_, a, b, _)
                    if matches!(**a, contract_syntax::Expr::EmptyList(_))
                        && matches!(**b, contract_syntax::Expr::EmptyList(_))
            ),
        )
    };
    assert_eq!(empties(src), (true, true));
    assert_eq!(empties(&after), (true, true));
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
fn argument_equals_on_the_next_line_keeps_attribute_spacing() {
    let src = "component A\n  view\n    Row(\n      first\n      = \"雪\",\n      second\n      = 2\n    )\n";
    let expected = "component A\n  view\n    Row(\n      first\n      =\"雪\",\n      second\n      =2\n    )\n";
    assert_eq!(preserved(src), expected);
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

#[test]
fn send_as_a_name_and_the_send_statement_both_round_trip() {
    // `send` spaces like any other name: `who` beside it gets the same
    // treatment. A named argument's `=` stays against its name, in a call as
    // at a use (LLP 1035.005.000 D3: `Shape(field=value)`).
    let src = "component A\n  props\n    send: action\n  mutation session as shape Session\n  action go\n    send()\n    send session = login(who=1, send=2)\n  view\n    Row(who=go, send=go)\n";
    assert_eq!(preserved(src), src);
    let spaced = src.replace("login(who=1, send=2)", "login(who = 1, send = 2)");
    assert_eq!(preserved(&spaced), src);
}

#[test]
fn a_provide_section_spaces_like_bindings() {
    // LLP 1035.005.000 D9: a bare name stays bare; a binding spaces like a state's.
    let src = "component A\n  state a = 1\n  provide\n    // the value itself\n    a\n    b   =   a+1\n  view\n    text \"x\"\n";
    let expected = src.replace("b   =   a+1", "b = a + 1");
    assert_eq!(preserved(src), expected);
}
