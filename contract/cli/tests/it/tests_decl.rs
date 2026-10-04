//! LLP 1017 P7: `test` blocks parse to the agent's steps and print as JSON
//! the driver reads (`scripts/agent.mjs --test`).

use contract_syntax::Step;

#[test]
fn a_test_block_parses_to_the_eight_operations_and_expects() {
    let src = "test \"login\"\n  tap \"title\"\n  type \"who\" \"alice\"\n  type \"password\" key \"Enter\"\n  clock settle\n  clock +500\n  screenshot \"after.png\"\n  expect tree has \"login-error\"\n  expect tree missing \"signed-in\"\n  expect text \"login-error\" == \"Wrong password\"\n  expect state password == \"\"\n  expect state attempt == 1\n  expect state ok == false\n  expect state picked == none\n  expect state tags == []\n";
    let tests = contract::tests(src).unwrap();
    assert_eq!(tests.len(), 1);
    let t = &tests[0];
    assert_eq!(t.name, "login");
    assert_eq!(t.steps.len(), 14);
    assert!(matches!(&t.steps[0], Step::Tap { target, hover: false, .. } if target == "title"));
    assert!(matches!(&t.steps[2], Step::Key { key, .. } if key == "Enter"));
    assert!(matches!(&t.steps[4], Step::Clock { arg, .. } if arg == "+500"));
    assert!(matches!(
        &t.steps[7],
        Step::ExpectTree { present: false, .. }
    ));
    assert!(matches!(&t.steps[8], Step::ExpectText { .. }));
    let json = contract::tests_json(&tests);
    assert!(json.starts_with("[{\"name\":\"login\",\"steps\":[{\"op\":\"tap\",\"target\":\"title\",\"hover\":false,\"line\":2}"), "{json}");
    assert!(
        json.contains("{\"op\":\"expect-state\",\"name\":\"attempt\",\"value\":1,\"line\":12}"),
        "{json}"
    );
    assert!(
        json.contains("{\"op\":\"expect-state\",\"name\":\"picked\",\"value\":null,\"line\":14}"),
        "{json}"
    );
    // `[]` is the empty list on the wire too, never `null`.
    assert!(
        json.contains("{\"op\":\"expect-state\",\"name\":\"tags\",\"value\":[],\"line\":15}"),
        "{json}"
    );
    assert!(
        json.contains(
            "\"op\":\"expect-text\",\"target\":\"login-error\",\"value\":\"Wrong password\""
        ),
        "{json}"
    );
}

#[test]
fn a_step_that_is_not_an_operation_is_refused() {
    let e = contract::tests("test \"t\"\n  jump \"a\"\n").unwrap_err();
    assert_eq!(e.id, "syntax-expected-step");
    let e = contract::tests("test \"t\"\n  expect state n == 1 + 1\n").unwrap_err();
    assert_eq!(e.id, "syntax-expected-step");
}

#[test]
fn the_apps_tests_parse() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../apps/caltrain/app.test.contract"
    ))
    .unwrap();
    let tests = contract::tests(&src).unwrap();
    assert_eq!(tests.len(), 3);
}

#[test]
fn a_test_drags_and_opens_at_its_size() {
    // kanban F18, paint F5: a drag, as the driver's `tap … drag`, and the
    // viewport a test's session opens at, as its `--size`.
    let src = "test \"board\"\n  size 1280x800\n  tap \"card\" drag 300 -60 hold 100\n  tap \"card\" drag -4.5 0\n  tap \"row\" drag 0 -100 from 12 8 mouse over 300\n";
    let tests = contract::tests(src).unwrap();
    let t = &tests[0];
    assert!(
        matches!(&t.steps[0], Step::Size { width, height, .. } if *width == 1280.0 && *height == 800.0)
    );
    assert!(
        matches!(&t.steps[1], Step::Drag { dx, dy, hold: Some(h), press: None, .. } if *dx == 300.0 && *dy == -60.0 && *h == 100.0)
    );
    assert!(matches!(&t.steps[2], Step::Drag { dx, .. } if *dx == -4.5));
    // Where in the node it starts, and the left button (files diary F10).
    assert!(
        matches!(&t.steps[3], Step::Drag { from: Some((x, y)), mouse: true, over: Some(o), .. } if *x == 12.0 && *y == 8.0 && *o == 300.0)
    );
    let json = contract::tests_json(&tests);
    assert!(
        json.contains("{\"op\":\"size\",\"width\":1280,\"height\":800,\"line\":2}"),
        "{json}"
    );
    assert!(
        json.contains(
            "{\"op\":\"drag\",\"target\":\"card\",\"dx\":300,\"dy\":-60,\"hold\":100,\"line\":3}"
        ),
        "{json}"
    );
    assert!(
        json.contains(
            "{\"op\":\"drag\",\"target\":\"row\",\"dx\":0,\"dy\":-100,\"from\":[12,8],\"mouse\":true,\"over\":300,\"line\":5}"
        ),
        "{json}"
    );
    // The session opens at its size: a later `size` is refused, as is an
    // option given twice, or a size not written as the driver's flag is.
    let e = contract::tests("test \"t\"\n  tap \"a\"\n  size 800x600\n").unwrap_err();
    assert_eq!((e.id.as_str(), e.span.line), ("syntax-expected-step", 3));
    let e = contract::tests("test \"t\"\n  tap \"a\" drag 1 2 over 10 over 20\n").unwrap_err();
    assert_eq!(e.id, "syntax-expected-step");
    let e = contract::tests("test \"t\"\n  tap \"a\" drag 1 2 mouse mouse\n").unwrap_err();
    assert_eq!(e.id, "syntax-expected-step");
    let e = contract::tests("test \"t\"\n  size 800 600\n").unwrap_err();
    assert_eq!(e.id, "syntax-expected-step");
}
