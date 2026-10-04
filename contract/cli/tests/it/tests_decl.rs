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
    let src = "test \"board\"\n  size 1280x800\n  tap \"card\" drag 300 -60 hold 100\n  tap \"card\" drag -4.5 0\n";
    let tests = contract::tests(src).unwrap();
    let t = &tests[0];
    assert!(
        matches!(&t.steps[0], Step::Size { width, height, .. } if *width == 1280.0 && *height == 800.0)
    );
    assert!(
        matches!(&t.steps[1], Step::Drag { dx, dy, hold: Some(h), press: None, .. } if *dx == 300.0 && *dy == -60.0 && *h == 100.0)
    );
    assert!(matches!(&t.steps[2], Step::Drag { dx, .. } if *dx == -4.5));
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
    // The session opens at its size: a later `size` is refused, as is an
    // option given twice, or a size not written as the driver's flag is.
    let e = contract::tests("test \"t\"\n  tap \"a\"\n  size 800x600\n").unwrap_err();
    assert_eq!((e.id.as_str(), e.span.line), ("syntax-expected-step", 3));
    let e = contract::tests("test \"t\"\n  tap \"a\" drag 1 2 over 10 over 20\n").unwrap_err();
    assert_eq!(e.id, "syntax-expected-step");
    let e = contract::tests("test \"t\"\n  size 800 600\n").unwrap_err();
    assert_eq!(e.id, "syntax-expected-step");
}

#[test]
fn a_file_and_a_test_carry_their_launch_facts() {
    // habits F7, calendar F13: the date, zone, locale, seed and viewport a
    // test needs are written in the file, not remembered as driver flags.
    // The file's lines lead every test that does not name its own.
    let src = "epoch \"2026-09-21T12:00:00Z\"\ntime-zone \"America/New_York\"\nsize 1200x800\n\ntest \"a\"\n  locale \"fr-FR\"\n  seed 7\n  tap \"x\"\n\ntest \"b\"\n  epoch 1790000000000\n  size 420x900\n  expect tree has \"y\"\n";
    let tests = contract::tests(src).unwrap();
    let json = contract::tests_json(&tests);
    assert!(json.starts_with("[{\"name\":\"a\",\"steps\":[{\"op\":\"epoch\",\"value\":\"2026-09-21T12:00:00Z\",\"line\":1},{\"op\":\"time-zone\",\"value\":\"America/New_York\",\"line\":2},{\"op\":\"size\",\"width\":1200,\"height\":800,\"line\":3},{\"op\":\"locale\",\"value\":\"fr-FR\",\"line\":6},{\"op\":\"seed\",\"value\":7,\"line\":7},{\"op\":\"tap\""), "{json}");
    assert!(json.contains("{\"name\":\"b\",\"steps\":[{\"op\":\"time-zone\",\"value\":\"America/New_York\",\"line\":2},{\"op\":\"epoch\",\"value\":\"1790000000000\",\"line\":11},{\"op\":\"size\",\"width\":420,\"height\":900,\"line\":12},{\"op\":\"expect-tree\""), "{json}");
    // Each leads its test's steps and is named once; the values are the
    // driver's: an ISO date or whole milliseconds, a whole seed.
    for (src, line) in [
        ("test \"t\"\n  tap \"a\"\n  seed 7\n", 3),
        ("test \"t\"\n  locale \"fr\"\n  locale \"de\"\n", 3),
        ("seed 1\nseed 2\n", 2),
        ("test \"t\"\n  epoch \"Sept 21\"\n", 2),
        ("test \"t\"\n  seed 1.5\n", 2),
        ("test \"t\"\n  time-zone UTC\n", 2),
    ] {
        let e = contract::tests(src).unwrap_err();
        assert_eq!(e.span.line, line, "{src}: {e}");
    }
}
