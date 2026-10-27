//! LLP 1123 D11: `-exact-background-extension` on an `image`, `auto` or
//! `none`; on any other node, or with any other word, lowering refuses it.
use exact_kernel::{Kernel, PropId};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn app(node: &str, value: &str) -> String {
    format!("component App\n  state wide = true\n  view\n    column\n      {node} -exact-background-extension={value} width=40 height=40 testId=\"it\"\n")
}

#[test]
fn an_image_takes_auto_or_none() {
    for value in ["auto", "none"] {
        let source = app(
            r#"image "https://example.com/a.png""#,
            &format!("\"{value}\""),
        );
        let plan = contract::bake(contract::compile(&source).unwrap(), NoData).unwrap();
        let runner = Runner::boot(
            plan,
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        let kernel = runner.kernel();
        let node = kernel.node_by_key(kernel.find_by_test_id("it")[0]).unwrap();
        assert_eq!(node.props.str(PropId::BackgroundExtension), Some(value));
    }
    // A computed value is the host's to read.
    contract::compile(&app(
        r#"image "https://example.com/a.png""#,
        r#"(wide ? "auto" : "none")"#,
    ))
    .unwrap();
}

#[test]
fn any_other_node_or_word_is_refused() {
    let image = r#"image "https://example.com/a.png""#;
    for (node, value) in [
        ("column", "\"auto\""),
        (image, "\"always\""),
        // Every literal a value can be is checked: a conditional's branches and a template with no `${}`.
        (image, r#"(wide ? "auto" : "always")"#),
        (image, r#"(wide ? "none" : (wide ? "Auto" : "auto"))"#),
        (image, "`all`"),
    ] {
        let error = contract::compile(&app(node, value)).unwrap_err();
        assert_eq!(error.id, "lower-attr-value", "{node} {value}: {error}");
    }
    // The refusal points at the literal that is no word.
    let source = app(image, r#"(wide ? "auto" : "always")"#);
    let error = contract::compile(&source).unwrap_err();
    let line = source.lines().nth(error.span.line as usize - 1).unwrap();
    assert_eq!(
        &line[error.span.col as usize - 1..error.span.end_col as usize - 1],
        "\"always\"",
        "{error}"
    );
}
