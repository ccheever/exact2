//! `postMessage(text, "world")`: the inverse of a canvas's `message=`. Every
//! call reaches the host as its own command, in order, even when two presses
//! land between frames with the same text (Grow a Garden kept only the second
//! through a live argument).
use exact_runner::{DataError, DataSource, Runner, Value};

const APP: &str = r#"component App
  state count = 0
  action buy(seed: string)
    count = count + 1
    postMessage(`buy ${seed}`, "world")
  view
    column
      canvas surface=world() width=200 height=200
      button press=buy("carrot") testId="buy"
        text "Buy"
"#;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.to_string()))
    }
}

#[test]
fn each_post_is_its_own_command_in_order() {
    let mut r = Runner::boot(
        contract::compile(APP).unwrap(),
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.take_commands();
    for seed in ["carrot", "carrot", "tomato"] {
        r.act("buy", vec![Value::str(seed)]).unwrap();
    }
    let posted: Vec<_> = r
        .take_commands()
        .into_iter()
        .map(|c| (c.name, c.args))
        .collect();
    let expected = |seed: &str| {
        (
            "postMessage".to_string(),
            vec![Value::str(&format!("buy {seed}")), Value::str("world")],
        )
    };
    assert_eq!(
        posted,
        [expected("carrot"), expected("carrot"), expected("tomato")]
    );
}

#[test]
fn the_command_is_checked_by_name() {
    for statement in [
        r#"postMessage("world")"#,
        r#"postMessage(seed, seed)"#,
        r#"postMessage(count, "world")"#,
        // A typo in the surface's name is a compile error, not a dropped post.
        r#"postMessage(seed, "wrold")"#,
        r#"postMessage("world", seed)"#,
    ] {
        let source = APP.replace(r#"postMessage(`buy ${seed}`, "world")"#, statement);
        let error = contract::compile(&source).unwrap_err().to_string();
        assert!(error.contains("type-post-message"), "{statement}: {error}");
    }
}

#[test]
fn iframe_posts_use_live_ids_without_entering_the_canvas_queue() {
    let source = r#"component App
  state target = "guest-a"
  action reply(text: string)
    postMessage(text, target)
    postMessage(text, "guest-a")
  view
    iframe id="guest-a" src="https://example.test/a" sandbox="allow-scripts"
"#;
    let mut runner = Runner::boot(
        contract::compile(source).unwrap(),
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    runner.take_commands();
    runner.act("reply", vec![Value::str("pong")]).unwrap();
    let commands = runner.take_commands();
    assert_eq!(commands.len(), 2);
    for command in commands {
        assert_eq!(command.name, "postMessage");
        assert_eq!(
            command.args,
            vec![
                Value::str("pong"),
                Value::str("guest-a"),
                Value::str("iframe")
            ]
        );
    }
}

#[test]
fn iframe_targets_are_checked_and_static_name_collisions_are_refused() {
    for source in [
        "component App\n  action reply\n    postMessage(\"x\", 4)\n  view\n    iframe id=\"guest\" src=\"https://example.test\"\n",
        "component App\n  action reply\n    postMessage(\"x\", \"absent\")\n  view\n    iframe id=\"guest\" src=\"https://example.test\"\n",
        "component App\n  action reply\n    postMessage(\"x\", \"world\")\n  view\n    column\n      canvas surface=world() width=200 height=200\n      iframe id=\"world\" src=\"https://example.test\"\n",
    ] {
        let error = contract::compile(source).unwrap_err().to_string();
        assert!(error.contains("type-post-message"), "{error}");
    }
}

#[test]
fn an_unmounted_declared_canvas_keeps_the_existing_post_protocol() {
    let source = APP.replace(
        "canvas surface=world() width=200 height=200",
        "text \"No world mounted\"",
    )
        + "\ncomponent Unused\n  view\n    canvas surface=world() width=200 height=200\n";
    let mut runner = Runner::boot(
        contract::compile(&source).unwrap(),
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    runner.take_commands();
    runner.act("buy", vec![Value::str("carrot")]).unwrap();
    assert_eq!(runner.take_commands()[0].args.len(), 2);
}
