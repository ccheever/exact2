//! The parser on the constructs the v1 app uses, and its rejections by id.

use contract_syntax::{parse, BinOp, Expr, Node, Stmt, TaskKind, TemplatePart, TypeExpr};

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

  action selectStation(id)
    stationId = some(id)
    query = ""
  action setDark
    setScheme("dark")

  task ticker mount
    every(1000, tick)

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
      text countdownText(dep.at, nowMs) font-size=(1 + 2) * 3
"#;

#[test]
fn resource_context_appends_expressions_after_the_call_arguments() {
    let src = r#"component App
  resource plain = source(car, minute) as shape Status
  resource single = source(car) with minute as shape Status
  resource empty = source() with minute as shape Status
  resource several = source(car, account) with max(minute, 1), (ready ? 2 : 3), some(clock) as shape Status else preview("cached")
  view
    text "ready"
"#;
    let file = parse(src).unwrap();
    let resources = &file.components[0].resources;
    assert_eq!(resources[0].identity, None);
    assert_eq!(resources[0].args.len(), 2);
    assert_eq!(resources[1].identity, Some(1));
    assert_eq!(resources[1].args.len(), 2);
    assert_eq!(resources[2].identity, Some(0));
    assert_eq!(resources[2].args.len(), 1);
    let r = &resources[3];
    assert_eq!(r.identity, Some(2));
    assert_eq!(r.args.len(), 5);
    assert!(matches!(&r.args[0], Expr::Ident(name, _) if name == "car"));
    assert!(matches!(&r.args[1], Expr::Ident(name, _) if name == "account"));
    assert!(matches!(&r.args[2], Expr::Call(name, args, _) if name == "max" && args.len() == 2));
    assert!(matches!(&r.args[3], Expr::Ternary(..)));
    assert!(matches!(&r.args[4], Expr::Some(..)));
    let placeholder = r.placeholder.as_ref().unwrap();
    assert_eq!(placeholder.source, "preview");
    assert_eq!(placeholder.args.len(), 1);
}

#[test]
fn resource_context_requires_an_expression_after_with_and_each_comma() {
    for context in ["", ", minute", "minute,", "minute,, clock"] {
        let src = format!(
            "component App\n  resource value = source() with {context} as shape number\n  view\n    text value\n"
        );
        assert!(parse(&src).is_err(), "accepted {context:?}");
    }
}

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
    assert_eq!(app.actions[0].effects().len(), 2);
    assert!(
        matches!(app.actions[0].body[0], Stmt::Assign { ref target, .. } if target == "stationId")
    );
    assert!(
        matches!(app.actions[1].body[0], Stmt::Command { ref name, .. } if name == "setScheme")
    );
    assert_eq!(app.tasks[0].kind, TaskKind::Every);
    assert_eq!(app.tasks[0].timer.1, "tick");
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
fn a_task_fires_once_with_after() {
    let file = parse(
        "component A\n  state launching = true\n  action arrived\n    launching = false\n  task launch mount\n    after(60, arrived)\n  view\n    text \"a\"\n",
    )
    .unwrap();
    let task = &file.components[0].tasks[0];
    assert_eq!(task.name, "launch");
    assert_eq!(task.kind, TaskKind::After);
    assert!(matches!(task.timer.0, Expr::Number(ms, _) if ms == 60.0));
    assert_eq!(task.timer.1, "arrived");
    // One entry per task, whichever word.
    let e = parse(
        "component A\n  task t mount\n    after(60, a)\n    every(1000, b)\n  view\n    text \"a\"\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "syntax-duplicate-declaration");
}

// @ref LLP 1073 D1 — `frame` in `every`'s first place is a word, even
// beside a slot of that name; `after(frame, …)` is refused.
#[test]
fn a_task_fires_each_frame_with_every_frame() {
    let file = parse(
        "component A\n  state frame = 0\n  action step\n    frame = frame + 1\n  task ticker mount\n    every(frame, step)\n  view\n    text \"a\"\n",
    )
    .unwrap();
    let task = &file.components[0].tasks[0];
    assert_eq!(task.kind, TaskKind::Frame);
    assert_eq!(task.timer.1, "step");
    // An expression that starts with the word is still an interval.
    let file = parse(
        "component A\n  state frame = 16\n  action step\n    frame = 16\n  task t mount\n    every(frame + 1, step)\n  view\n    text \"a\"\n",
    )
    .unwrap();
    assert_eq!(file.components[0].tasks[0].kind, TaskKind::Every);
    let e = parse("component A\n  task t mount\n    after(frame, a)\n  view\n    text \"a\"\n")
        .unwrap_err();
    assert_eq!(e.id, "contract-task-body");
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
            "component A\n  state n = 0\n  action go\n    if true\n    n = 1\n  view\n    text \"a\"\n",
            4,
        ),
        (
            "component A\n  state n = 0\n  action go\n    if true\n      n = 1\n    else\n    n = 2\n  view\n    text \"a\"\n",
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
        // `[]` is the empty list; a list literal with items is not Contract.
        ("component A\n  derive xs = [1, 2]\n", "syntax-expected", 2),
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
    let src = "component A\n  state n = 0\n  action go\n    n = 1\n  view\n    button press=go\n      testId=\"go\" aria-label=\"Go\"\n      width=40\n      text \"a\"\n        font-size=12\n      text \"b\"\n";
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

#[test]
fn source_identity_reaches_nested_ast_ranges_without_changing_syntax() {
    use contract_syntax::{parse_source, VisitSpans};
    let expected = parse(APP).unwrap();
    let mut tagged = parse_source(APP, 29).unwrap();
    let mut count = 0;
    tagged.visit_spans(&mut |span| {
        assert_eq!(span.source_id, 29, "{span:?}");
        assert!(span.end_col >= span.col, "{span:?}");
        span.source_id = 0;
        count += 1;
    });
    assert!(count > 60, "{count}");
    assert_eq!(tagged, expected);
}

#[test]
fn send_is_a_name_everywhere_but_where_the_send_statement_starts() {
    let src = "component A\n  props\n    send: action\n  state count = 0\n  mutation session as shape Session\n  action go\n    send()\n    send(count)\n    send session = login(send=count)\n    count = send\n  view\n    Row(send=go)\n";
    let file = parse(src).unwrap();
    let a = &file.components[0];
    assert_eq!(a.props[0].name, "send");
    assert!(matches!(a.props[0].ty, Some(TypeExpr::Named(ref t, _)) if t == "action"));
    let body = &a.actions[0].body;
    assert!(
        matches!(&body[0], Stmt::Command { name, args, .. } if name == "send" && args.is_empty())
    );
    assert!(
        matches!(&body[1], Stmt::Command { name, args, .. } if name == "send" && args.len() == 1)
    );
    assert!(matches!(
        &body[2],
        Stmt::Send { target, source, args, .. }
            if target == "session" && source == "login"
                && matches!(&args[0], Expr::NamedArg(n, _, _) if n == "send")
    ));
    assert!(matches!(&body[3], Stmt::Assign { target, expr, .. }
        if target == "count" && matches!(expr, Expr::Ident(n, _) if n == "send")));
    assert!(matches!(&a.view[0], Node::Use { args, .. } if args[0].name == "send"));

    let src = "component A\n  state send = 0\n  action go\n    send = send + 1\n  view\n    text `${send}`\n";
    let file = parse(src).unwrap();
    let a = &file.components[0];
    assert_eq!(a.states[0].name, "send");
    assert!(matches!(&a.actions[0].body[0], Stmt::Assign { target, .. } if target == "send"));
}

#[test]
fn let_starts_a_statement_only_before_a_name_and_records_build_with_named_arguments() {
    // LLP 1035.005.000 D2 and D3.
    let src = "component A\n  state let = 0\n  action go\n    let next = F(base, title=\"a\")\n    if next.pinned\n      let word = \"b\"\n    let = 1\n  view\n    text \"a\"\n";
    let file = parse(src).unwrap();
    let body = &file.components[0].actions[0].body;
    assert!(
        matches!(&body[0], Stmt::Let { name, expr: Expr::Call(shape, args, _), span }
        if name == "next" && shape == "F" && (span.line, span.col) == (4, 9)
            && matches!(&args[0], Expr::Ident(b, _) if b == "base")
            && matches!(&args[1], Expr::NamedArg(f, _, _) if f == "title"))
    );
    assert!(matches!(&body[1], Stmt::If { then, .. }
        if matches!(&then[0], Stmt::Let { name, .. } if name == "word")));
    assert!(matches!(&body[2], Stmt::Assign { target, .. } if target == "let"));
    let e =
        parse("component A\n  action go\n    let in = 1\n  view\n    text \"a\"\n").unwrap_err();
    assert_eq!(e.id, "syntax-expected-name", "{e:?}");
    let e = parse("component A\n  action go\n    let a 1\n  view\n    text \"a\"\n").unwrap_err();
    assert_eq!(e.id, "syntax-expected", "{e:?}");
}

#[test]
fn template_text_decodes_the_escapes_a_string_does_and_escaped_interpolation_is_literal() {
    fn parts(template: &str) -> Vec<TemplatePart> {
        let src = format!("component A\n  state x = 1\n  view\n    text {template}\n");
        let file = parse(&src).unwrap();
        let Node::Element { positional, .. } = &file.components[0].view[0] else {
            panic!()
        };
        let Expr::Template(parts, _) = &positional[0] else {
            panic!("{:?}", positional[0])
        };
        parts.clone()
    }
    fn text(template: &str) -> String {
        match parts(template).as_slice() {
            [TemplatePart::Text(t)] => t.clone(),
            other => panic!("{template}: {other:?}"),
        }
    }
    assert_eq!(text(r"`a\${x}`"), "a${x}");
    assert_eq!(text(r"`a\tb\nc`"), "a\tb\nc");
    assert_eq!(text(r#"`\`\\\"\$`"#), "`\\\"$");
    // `\\` before `${` is an escaped backslash; the interpolation still runs.
    let p = parts(r"`a\\${x}\`b`");
    assert!(
        matches!(&p[0], TemplatePart::Text(t) if t == "a\\"),
        "{p:?}"
    );
    assert!(
        matches!(&p[1], TemplatePart::Expr(Expr::Ident(n, _)) if n == "x"),
        "{p:?}"
    );
    assert!(matches!(&p[2], TemplatePart::Text(t) if t == "`b"), "{p:?}");
    // The same escape table as a string, and the same refusal past it.
    let e = parse("component A\n  view\n    text `ok \\q`\n").unwrap_err();
    assert_eq!(e.id, "syntax-bad-escape", "{e:?}");
    assert_eq!((e.span.line, e.span.col), (3, 14), "{e:?}");
}
