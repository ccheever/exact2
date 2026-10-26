//! @ref LLP 1121 stage 14 C2 — `exact_kernel::Icon::parse` against the
//! shared cases (kernel/tests/fixtures/icon-forms.json), which the JS target's
//! mirror (host/web-js/symbols.js `iconForm`) is held to as well.

#[test]
fn icon_parse_reads_each_shared_case() {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../kernel/tests/fixtures/icon-forms.json"
    ))
    .unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let source = case[0].as_str().unwrap();
        let got = exact_kernel::Icon::parse(source).map(|i| {
            serde_json::json!({"sf": i.sf, "view_box": i.view_box, "path": i.path, "svg": i.svg, "even_odd": i.even_odd})
        });
        assert_eq!(got.unwrap_or(serde_json::Value::Null), case[1], "{source}");
    }
}
