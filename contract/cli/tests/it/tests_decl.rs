//! LLP 1017 P7: `test` blocks parse to the agent's steps and print as JSON
//! the driver reads (`scripts/agent.mjs --test`).

use contract_syntax::{Expr, Step, TapForm};

#[test]
fn a_test_block_parses_to_the_eight_operations_and_expects() {
    let src = "test \"login\"\n  tap \"title\"\n  type \"who\" \"alice\"\n  type \"password\" key \"Enter\"\n  clock settle\n  clock +500\n  clock +250 real\n  screenshot \"after.png\"\n  expect tree has \"login-error\"\n  expect tree missing \"signed-in\"\n  expect text \"login-error\" == \"Wrong password\"\n  expect state password == \"\"\n  expect state attempt == 1\n  expect state ok == false\n  expect state picked == none\n  expect state tags == []\n";
    let tests = contract::tests(src).unwrap();
    assert_eq!(tests.len(), 1);
    let t = &tests[0];
    assert_eq!(t.name, "login");
    assert_eq!(t.steps.len(), 15);
    assert!(
        matches!(&t.steps[0], Step::Tap { target, form: TapForm::Press, .. } if target == "title")
    );
    assert!(matches!(&t.steps[2], Step::Key { key, .. } if key == "Enter"));
    assert!(matches!(&t.steps[4], Step::Clock { arg, .. } if arg == "+500"));
    assert!(matches!(&t.steps[5], Step::Clock { arg, .. } if arg == "+250 real"));
    assert!(matches!(
        &t.steps[8],
        Step::ExpectTree { present: false, .. }
    ));
    assert!(matches!(&t.steps[9], Step::ExpectText { .. }));
    let json = contract::tests_json(&tests);
    assert!(json.starts_with("[{\"name\":\"login\",\"steps\":[{\"op\":\"tap\",\"target\":\"title\",\"form\":\"press\",\"line\":2}"), "{json}");
    assert!(
        json.contains("{\"op\":\"expect-state\",\"name\":\"attempt\",\"value\":1,\"line\":13}"),
        "{json}"
    );
    assert!(
        json.contains("{\"op\":\"expect-state\",\"name\":\"picked\",\"value\":null,\"line\":15}"),
        "{json}"
    );
    // `[]` is the empty list on the wire too, never `null`.
    assert!(
        json.contains("{\"op\":\"expect-state\",\"name\":\"tags\",\"value\":[],\"line\":16}"),
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

#[test]
fn a_test_pinches_holds_a_key_and_indexes_state() {
    // stocks: pinch. platformer R7: a key held on the virtual clock, and a
    // drag that holds. drums R7–R8: `during`, a list index, a negative number.
    let src = "test \"play\"\n  tap \"chart\" pinch 2\n  tap \"map\" pinch 1.5 at 4 -2\n  type \"board\" key \"KeyW\" down\n  type \"board\" key \"KeyW\" up\n  type \"board\" key \"Space\" for 40\n  tap \"card\" drag 10 0 hold 100 during \"clock +500\"\n  expect state items.0 == -3\n  expect state song.patterns.0.rows.0 == 3\n";
    let tests = contract::tests(src).unwrap();
    let t = &tests[0];
    assert!(matches!(
        &t.steps[0],
        Step::Tap { form: TapForm::Pinch { scale, at: None }, .. } if *scale == 2.0
    ));
    assert!(matches!(
        &t.steps[1],
        Step::Tap { form: TapForm::Pinch { scale, at: Some((x, y)) }, .. } if *scale == 1.5 && *x == 4.0 && *y == -2.0
    ));
    assert!(matches!(
        &t.steps[2],
        Step::Key { key, phase: Some(p), duration: None, .. } if key == "KeyW" && p == "down"
    ));
    assert!(matches!(
        &t.steps[3],
        Step::Key { phase: Some(p), .. } if p == "up"
    ));
    assert!(matches!(
        &t.steps[4],
        Step::Key { key, phase: None, duration: Some(ms), .. } if key == "Space" && *ms == 40.0
    ));
    assert!(matches!(
        &t.steps[5],
        Step::Drag { hold: Some(h), during, .. } if *h == 100.0 && during.as_slice() == ["clock +500"]
    ));
    assert!(matches!(
        &t.steps[6],
        Step::ExpectState { name, value: Expr::Number(n, _), .. } if name == "items.0" && *n == -3.0
    ));
    assert!(matches!(
        &t.steps[7],
        Step::ExpectState { name, .. } if name == "song.patterns.0.rows.0"
    ));
    let json = contract::tests_json(&tests);
    assert!(
        json.contains(
            "{\"op\":\"tap\",\"target\":\"chart\",\"form\":\"pinch\",\"scale\":2,\"line\":2}"
        ),
        "{json}"
    );
    assert!(
        json.contains("\"form\":\"pinch\",\"scale\":1.5,\"at\":[4,-2]"),
        "{json}"
    );
    assert!(
        json.contains(
            "{\"op\":\"key\",\"target\":\"board\",\"key\":\"KeyW\",\"phase\":\"down\",\"line\":4}"
        ),
        "{json}"
    );
    assert!(
        json.contains(
            "{\"op\":\"key\",\"target\":\"board\",\"key\":\"Space\",\"for\":40,\"line\":6}"
        ),
        "{json}"
    );
    assert!(
        json.contains("\"hold\":100,\"during\":[\"clock +500\"]"),
        "{json}"
    );
    assert!(
        json.contains("{\"op\":\"expect-state\",\"name\":\"items.0\",\"value\":-3,\"line\":8}"),
        "{json}"
    );
    // `pinch` is not a leftover word on the line (stocks: syntax-expected-newline).
    let e = contract::tests("test \"t\"\n  tap \"chart\" pinch\n").unwrap_err();
    assert_eq!(e.id, "syntax-expected-step");
    let e = contract::tests("test \"t\"\n  tap \"a\" drag 1 0 during \"clock +1\" hold 10\n")
        .unwrap_err();
    assert_eq!(e.id, "syntax-expected-step");
    let e = contract::tests("test \"t\"\n  tap \"a\" drag 1 0 during \"tap b\"\n").unwrap_err();
    assert_eq!(e.id, "syntax-expected-step");
    let e = contract::tests("test \"t\"\n  type \"a\" key \"Space\" down for 10\n").unwrap_err();
    assert_eq!(e.id, "syntax-expected-step");
    let e = contract::tests("test \"t\"\n  expect state n == -1 + 2\n").unwrap_err();
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

#[test]
fn a_test_waits_for_the_apps_data_unless_it_says_before_data() {
    // habits, pomodoro, kanban: every test's first step waits for the app's
    // data, as `clock data` does; `before data` does not wait, and
    // `clock data` lands what a step started without moving the clock.
    let src = "before data\n\ntest \"a\"\n  expect text \"x\" == \"Opening…\"\n  clock data\n\ntest \"b\"\n  before data\n  size 420x900\n  tap \"y\"\n";
    let tests = contract::tests(src).unwrap();
    assert!(matches!(&tests[0].steps[0], Step::BeforeData { .. }));
    assert!(matches!(&tests[0].steps[2], Step::Clock { arg, .. } if arg == "data"));
    let json = contract::tests_json(&tests);
    assert!(
        json.starts_with("[{\"name\":\"a\",\"steps\":[{\"op\":\"before-data\",\"line\":1},"),
        "{json}"
    );
    assert!(
        json.contains("{\"op\":\"clock\",\"arg\":\"data\",\"line\":5}"),
        "{json}"
    );
    assert!(
        json.contains(
            "{\"name\":\"b\",\"steps\":[{\"op\":\"before-data\",\"line\":8},{\"op\":\"size\""
        ),
        "{json}"
    );
    for (src, line) in [
        ("test \"t\"\n  tap \"a\"\n  before data\n", 3),
        ("test \"t\"\n  before \"a\"\n", 2),
        ("before data\nbefore data\n", 2),
    ] {
        let e = contract::tests(src).unwrap_err();
        assert_eq!(e.span.line, line, "{src}: {e}");
    }
}

#[test]
fn a_test_resizes_the_window_mid_test() {
    // reader: repagination on resize is driven, not polled for by hand.
    let tests =
        contract::tests("test \"t\"\n  size 1200x800\n  tap \"a\"\n  resize 800x600\n").unwrap();
    assert!(
        matches!(&tests[0].steps[2], Step::Resize { width, height, .. } if *width == 800.0 && *height == 600.0)
    );
    let json = contract::tests_json(&tests);
    assert!(
        json.contains("{\"op\":\"resize\",\"width\":800,\"height\":600,\"line\":4}"),
        "{json}"
    );
    for src in ["test \"t\"\n  resize 800\n", "test \"t\"\n  resize 0x600\n"] {
        let e = contract::tests(src).unwrap_err();
        assert!(e.message.starts_with("`resize` takes"), "{e}");
    }
}

#[test]
fn a_test_closes_the_window_as_its_close_button_does() {
    // studio diary R17: `beforeunload` and "Save changes?" are driven.
    let tests = contract::tests("test \"t\"\n  type \"note\" \"x\"\n  close\n").unwrap();
    assert!(matches!(&tests[0].steps[1], Step::Close { .. }));
    let json = contract::tests_json(&tests);
    assert!(json.contains("{\"op\":\"close\",\"line\":3}"), "{json}");
}

#[test]
fn a_test_answers_a_held_picker_by_its_node() {
    // files F11: a picker's hold is answered by the node its answer
    // arrives at, never by a ticket a test cannot predict.
    let src = "test \"import\"\n  tap \"choose\"\n  pick \"folder-input\" \"fixtures/notes\"\n  pick \"files\" \"a.txt\" \"b.txt\"\n  pick \"open-directory\" cancel\n";
    let tests = contract::tests(src).unwrap();
    let t = &tests[0];
    assert!(
        matches!(&t.steps[1], Step::Pick { target, paths, .. } if target == "folder-input" && paths == &["fixtures/notes"])
    );
    assert!(matches!(&t.steps[2], Step::Pick { paths, .. } if paths.len() == 2));
    assert!(matches!(&t.steps[3], Step::Pick { paths, .. } if paths.is_empty()));
    let json = contract::tests_json(&tests);
    assert!(
        json.contains("{\"op\":\"pick\",\"target\":\"folder-input\",\"paths\":[\"fixtures/notes\"],\"line\":3}"),
        "{json}"
    );
    assert!(
        json.contains("{\"op\":\"pick\",\"target\":\"open-directory\",\"paths\":[],\"line\":5}"),
        "{json}"
    );
    let e = contract::tests("test \"t\"\n  pick \"folder-input\"\n").unwrap_err();
    assert_eq!(e.id, "syntax-expected-string");
}

/// Spreadsheet F6: the clipboard's events are test steps, a paste carrying
/// its text as the clipboard's.
#[test]
fn clipboard_steps_parse_and_print() {
    let tests = contract::tests(
        "test \"grid\"\n  type \"grid\" paste \"a\\tb\"\n  type \"grid\" copy\n  type \"grid\" cut\n  tap \"b3\" modifiers \"Shift\"\n",
    )
    .unwrap();
    let steps = &tests[0].steps;
    assert!(
        matches!(&steps[0], Step::Clipboard { edit, text, .. } if edit == "paste" && text == "a\tb")
    );
    assert!(
        matches!(&steps[1], Step::Clipboard { edit, text, .. } if edit == "copy" && text.is_empty())
    );
    let json = contract::tests_json(&tests);
    assert!(
        json.contains(
            "{\"op\":\"clipboard\",\"target\":\"grid\",\"edit\":\"cut\",\"text\":\"\",\"line\":4}"
        ),
        "{json}"
    );
    // Gallery F20: a press with keys held.
    assert!(
        json.contains(
            "{\"op\":\"tap\",\"target\":\"b3\",\"form\":\"press\",\"modifiers\":\"Shift\",\"line\":5}"
        ),
        "{json}"
    );
}

#[test]
fn a_test_double_clicks_scrolls_a_list_appends_reloads_and_reads_a_field() {
    // feed F8/F10, mail F19, kanban F25: the driver's `tap` forms, a list
    // brought to a row by its key, typing after a prefill, a restart on the
    // same store, and a field of a record.
    let src = "test \"feed\"\n  tap \"post-1\" dblclick\n  tap \"post-1\" contextmenu\n  tap \"post-1\" hover\n  tap \"timeline\" into \"s2981\"\n  type \"reply\" \"agree\" append\n  reload\n  expect state s.queued == 1\n  expect state feed.page.next == \"b\"\n";
    let tests = contract::tests(src).unwrap();
    let t = &tests[0];
    assert!(matches!(
        &t.steps[0],
        Step::Tap {
            form: TapForm::Dblclick,
            ..
        }
    ));
    assert!(matches!(
        &t.steps[1],
        Step::Tap {
            form: TapForm::Contextmenu,
            ..
        }
    ));
    assert!(matches!(
        &t.steps[2],
        Step::Tap {
            form: TapForm::Hover,
            ..
        }
    ));
    assert!(matches!(&t.steps[3], Step::Tap { form: TapForm::Into(key), .. } if key == "s2981"));
    assert!(matches!(&t.steps[4], Step::Type { append: true, .. }));
    assert!(matches!(&t.steps[5], Step::Reload { .. }));
    assert!(matches!(&t.steps[6], Step::ExpectState { name, .. } if name == "s.queued"));
    let json = contract::tests_json(&tests);
    for want in [
        "{\"op\":\"tap\",\"target\":\"post-1\",\"form\":\"dblclick\",\"line\":2}",
        "{\"op\":\"tap\",\"target\":\"timeline\",\"form\":\"into\",\"key\":\"s2981\",\"line\":5}",
        "{\"op\":\"type\",\"target\":\"reply\",\"text\":\"agree\",\"append\":true,\"line\":6}",
        "{\"op\":\"reload\",\"line\":7}",
        "{\"op\":\"expect-state\",\"name\":\"feed.page.next\",\"value\":\"b\",\"line\":9}",
    ] {
        assert!(json.contains(want), "{want} in {json}");
    }
    // `into` names its key; a path ends on a field name.
    let e = contract::tests("test \"t\"\n  tap \"list\" into\n").unwrap_err();
    assert_eq!(e.id, "syntax-expected-string");
    assert!(contract::tests("test \"t\"\n  expect state a. == 1\n").is_err());
}
