//! `postMessage("world", text)`: the inverse of a canvas's `message=`. Every
//! call reaches the host as its own command, in order, even when two presses
//! land between frames with the same text (Grow a Garden kept only the second
//! through a live argument).
use exact_runner::{DataError, DataSource, Runner, Value};

const APP: &str = r#"component App
  state count = 0
  action buy(seed: string)
    count = count + 1
    postMessage("world", `buy ${seed}`)
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
            vec![Value::str("world"), Value::str(&format!("buy {seed}"))],
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
        r#"postMessage("world", count)"#,
    ] {
        let source = APP.replace(r#"postMessage("world", `buy ${seed}`)"#, statement);
        let error = contract::compile(&source).unwrap_err().to_string();
        assert!(error.contains("type-post-message"), "{statement}: {error}");
    }
}
