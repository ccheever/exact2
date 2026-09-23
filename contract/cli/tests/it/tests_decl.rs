//! LLP 1017 P7: `test` blocks parse to the agent's steps and print as JSON
//! the driver reads (`scripts/agent.mjs --test`).

use contract_syntax::Step;

#[test]
fn a_test_block_parses_to_the_eight_operations_and_expects() {
    let src = "test \"login\"\n  tap \"title\"\n  type \"who\" \"alice\"\n  type \"password\" key \"Enter\"\n  clock settle\n  clock +500\n  screenshot \"after.png\"\n  expect tree has \"login-error\"\n  expect tree missing \"signed-in\"\n  expect text \"login-error\" == \"Wrong password\"\n  expect state password == \"\"\n  expect state attempt == 1\n  expect state ok == false\n  expect state picked == none\n";
    let tests = contract::tests(src).unwrap();
    assert_eq!(tests.len(), 1);
    let t = &tests[0];
    assert_eq!(t.name, "login");
    assert_eq!(t.steps.len(), 13);
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
