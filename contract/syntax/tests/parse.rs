//! The parser on the constructs the v1 app uses, and its rejections by id.

use contract_syntax::{parse, BinOp, Expr, Node, Stmt, TemplatePart, TypeExpr};

const APP: &str = r#"
// A slice of the Caltrain app.
shape Departure
  id: string
  train: number
  at: number

component App
  state stationId = none
  state query = ""
  derive selected = match stationId { case some(id) => id, case none => "mv" }
  resource board = departures(selected) as shape list<Departure>
  derive count = length(board)

  action selectStation(id) writes stationId, query
    stationId = some(id)
    query = ""
  action setDark
    setScheme("dark")

  task ticker mount
    every(1000, tick)

  contract
    has text "Caltrain"
    when query == "" then has button "x"

  view
    column gap=16 testId="main"
      text `${count} trains` font-size=24 font-weight=700
      when query == "" and count > 0
        each d in board key=d.id
          Row(dep=d, press=selectStation)
      else
        text "searching"
      match stationId
        case some(id)
          text `at ${id}`
        case none
          text "nearest" color=colors.text

component Row
  props
    dep: Departure
    press: string
  view
    button press=press(dep.id) aria-label=`Train ${dep.train}`
      text formatCountdownMinutes(dep.at, nowMs) font-size=(1 + 2) * 3
"#;

#[test]
fn the_app_slice_parses_to_the_expected_tree() {
    let file = parse(APP).unwrap();
    assert_eq!(file.shapes.len(), 1);
    assert_eq!(file.shapes[0].fields.len(), 3);
    assert_eq!(file.components.len(), 2);
    let app = &file.components[0];
    assert_eq!(app.name, "App");
    assert_eq!(app.states.len(), 2);
    assert_eq!(app.derives.len(), 2);
    assert_eq!(app.resources.len(), 1);
    assert!(matches!(app.resources[0].shape, TypeExpr::List(..)));
    assert_eq!(app.actions.len(), 2);
    assert_eq!(app.actions[0].writes.len(), 2);
    assert!(
        matches!(app.actions[0].body[0], Stmt::Assign { ref target, .. } if target == "stationId")
    );
    assert!(
        matches!(app.actions[1].body[0], Stmt::Command { ref name, .. } if name == "setScheme")
    );
    assert_eq!(app.tasks[0].every.1, "tick");
    assert!(matches!(app.derives[0].expr, Expr::Match { .. }));

    let Node::Element {
        tag,
        attrs,
        children,
        ..
    } = &app.view[0]
    else {
        panic!()
    };
    assert_eq!(tag, "column");
    assert_eq!(
        attrs.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
        ["gap", "testId"]
    );
    assert_eq!(children.len(), 3);
    let Node::Element { positional, .. } = &children[0] else {
        panic!()
    };
    let Expr::Template(parts, _) = &positional[0] else {
        panic!()
    };
    assert!(matches!(&parts[0], TemplatePart::Expr(Expr::Ident(n, _)) if n == "count"));
    assert!(matches!(&parts[1], TemplatePart::Text(t) if t == " trains"));
    let Node::When {
        cond,
        then,
        otherwise,
        ..
    } = &children[1]
    else {
        panic!()
    };
    assert!(matches!(cond, Expr::Binary(BinOp::And, ..)));
    assert!(matches!(&then[0], Node::Each { var, .. } if var == "d"));
    let Node::Each { body, .. } = &then[0] else {
        panic!()
    };
    assert!(matches!(&body[0], Node::Use { name, args, .. } if name == "Row" && args.len() == 2));
    assert_eq!(otherwise.len(), 1);
    let Node::Match { some, none, .. } = &children[2] else {
        panic!()
    };
    assert_eq!(some.0, "id");
    assert_eq!(none.len(), 1);

    let row = &file.components[1];
    assert_eq!(row.props.len(), 2);
    let Node::Element {
        attrs, children, ..
    } = &row.view[0]
    else {
        panic!()
    };
    assert!(matches!(&attrs[0].value, Expr::Call(n, args, _) if n == "press" && args.len() == 1));
    let Node::Element { attrs, .. } = &children[0] else {
        panic!()
    };
    // (1 + 2) * 3 parses with the parenthesized sum on the left.
    assert!(
        matches!(&attrs[0].value, Expr::Binary(BinOp::Mul, l, _, _) if matches!(**l, Expr::Binary(BinOp::Add, ..)))
    );
}

#[test]
fn static_font_declarations_parse_in_shorthand_and_block_forms() {
    let file = parse(
        "font \"Body\" = \"assets/Body.ttf\"\nfont \"Display\"\n  400 = \"assets/Display.ttf\"\n  700 = \"assets/Display-Bold.otf\"\n  400 italic = \"assets/Display-Italic.ttf\"\ncomponent App\n  view\n    text \"hello\"\n",
    )
    .unwrap();
    assert_eq!(file.fonts.len(), 2);
    assert_eq!(file.fonts[0].faces[0].weight, 400);
    assert!(!file.fonts[0].faces[0].italic);
    assert_eq!(file.fonts[1].faces.len(), 3);
    assert!(file.fonts[1].faces[2].italic);

    let error = parse("font \"Bad\"\n  1001 = \"assets/Bad.ttf\"\n").unwrap_err();
    assert_eq!(error.id, "syntax-font-weight");
}

#[test]
fn precedence_and_multiline_calls_hold() {
    let f = parse("component A\n  derive x = 1 + 2 * 3 == 7 and not false\n  derive y = f(\n    1,\n    2)\n  view\n    text \"a\"\n").unwrap();
    let Expr::Binary(BinOp::And, l, r, _) = &f.components[0].derives[0].expr else {
        panic!()
    };
    assert!(matches!(**l, Expr::Binary(BinOp::Eq, ..)));
    assert!(matches!(**r, Expr::Unary(..)));
    assert!(
        matches!(&f.components[0].derives[1].expr, Expr::Call(n, a, _) if n == "f" && a.len() == 2)
    );
}

#[test]
fn a_template_expression_balances_match_braces_and_string_braces() {
    let file = parse(
        "component A\n  state choice = some(\"yes\")\n  derive label = `value ${match choice { case some(value) => value, case none => \"}\" }}`\n  view\n    text label\n",
    )
    .unwrap();
    let Expr::Template(parts, _) = &file.components[0].derives[0].expr else {
        panic!()
    };
    assert!(matches!(&parts[1], TemplatePart::Expr(Expr::Match { .. })));
}

#[test]
fn if_when_and_their_explicit_else_need_non_empty_blocks() {
    let cases = [
        (
            "component A\n  state n = 0\n  action go writes n\n    if true\n    n = 1\n  view\n    text \"a\"\n",
            4,
        ),
        (
            "component A\n  state n = 0\n  action go writes n\n    if true\n      n = 1\n    else\n    n = 2\n  view\n    text \"a\"\n",
            4,
        ),
        (
            "component A\n  view\n    when true\n    text \"always\"\n",
            3,
        ),
        (
            "component A\n  view\n    when true\n      text \"then\"\n    else\n    text \"always\"\n",
            3,
        ),
    ];
    for (src, line) in cases {
        let error = parse(src).unwrap_err();
        assert_eq!(error.id, "syntax-empty-block", "{src:?}: {error}");
        assert_eq!(error.span.line, line, "{src:?}: {error}");
    }
}

#[test]
fn rejections_carry_stable_ids_and_spans() {
    let cases = [
        ("use theme from \"x\"\n", "contract-no-imports", 1),
        (
            "component A\n  state x = 1\n\tview\n",
            "syntax-tab-indent",
            3,
        ),
        (
            "component A\n  state x = \"open\n",
            "syntax-unterminated-string",
            2,
        ),
        (
            "component A\n  view\n    when x\n      text \"a\"\n   text \"b\"\n",
            "syntax-bad-dedent",
            5,
        ),
        (
            "component A\n  task t mount\n    later(1, x)\n",
            "contract-task-body",
            3,
        ),
        (
            "component A\n  view\n    match x\n      case some(y)\n        text \"a\"\n",
            "contract-match-arms",
            3,
        ),
        (
            "component A\n  derive x = (1 + \n",
            "syntax-expected-expression",
            3,
        ),
        (
            "component A\n  derive x = `a ${1 + }`\n",
            "syntax-expected-expression",
            2,
        ),
        (
            "shape S\n  a: string\nwhatever\n",
            "syntax-expected-declaration",
            3,
        ),
    ];
    for (src, id, line) in cases {
        let err = parse(src).unwrap_err();
        assert_eq!(err.id, id, "{src:?} → {err}");
        assert_eq!(err.span.line, line, "{src:?} → {err}");
    }
}

#[test]
fn an_elements_attributes_continue_on_deeper_lines_that_begin_with_name_equals() {
    // LLP 1035.005 D1: `name=` on a deeper line continues the attribute
    // list; a child begins with a tag, so one-token lookahead decides.
    let src = "component A\n  state n = 0\n  action go writes n\n    n = 1\n  view\n    button press=go\n      testId=\"go\" aria-label=\"Go\"\n      width=40\n      text \"a\"\n        font-size=12\n      text \"b\"\n";
    let file = parse(src).unwrap();
    let Node::Element {
        attrs, children, ..
    } = &file.components[0].view[0]
    else {
        panic!()
    };
    assert_eq!(
        attrs.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
        ["press", "testId", "aria-label", "width"]
    );
    assert_eq!(children.len(), 2);
    let Node::Element { tag, attrs, .. } = &children[0] else {
        panic!()
    };
    assert_eq!(tag, "text");
    assert_eq!(attrs[0].name, "font-size");

    // At the element's own depth (or shallower) `name=` is refused.
    let e = parse("component A\n  view\n    column gap=8\n      text \"a\"\n    width=40\n")
        .unwrap_err();
    assert_eq!(e.id, "syntax-continuation-indent");
    assert_eq!(e.span.line, 5);
    // A continued line holds attributes only.
    let e = parse("component A\n  view\n    text \"a\"\n      width=40 \"b\"\n").unwrap_err();
    assert_eq!(e.id, "syntax-expected-attr");
    // A duplicate across lines is the same refusal as on one line.
    let e = parse("component A\n  view\n    text \"a\" width=1\n      width=2\n").unwrap_err();
    assert_eq!(e.id, "syntax-duplicate-attr");
}

#[test]
fn a_component_uses_arguments_may_span_lines_inside_its_parentheses() {
    let src = "component A\n  view\n    Row(\n      a=1,\n      b=\"x\",\n    )\n      text \"filled\"\ncomponent Row\n  props\n    a: number\n    b: string\n  slot\n  view\n    children\n";
    let file = parse(src).unwrap();
    let Node::Use {
        name,
        args,
        children,
        ..
    } = &file.components[0].view[0]
    else {
        panic!()
    };
    assert_eq!(name, "Row");
    assert_eq!(
        args.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(children.len(), 1);
}
