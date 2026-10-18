use super::{bake, Fixture, Tools, CONTRACT, SOURCE};

#[test]
fn widget_refresh_has_a_checked_source_without_changing_the_app_plan() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    let f = Fixture::new();
    f.write("app.contract", &format!("{CONTRACT}\nshape CardProps\n  title: string\ncomponent Card\n  props\n    title: string\n  view\n    text title\n"));
    let before = f.bake().plan;
    f.write("app.json", r#"{"app":{"id":"test.exact.logic","name":"Logic"},"widgets":[{"kind":"card","families":["systemSmall"],"component":"Card","props":"CardProps","refresh":"refreshCard","preview":{"title":"Preview"}}]}"#);
    let source = SOURCE.replace(
        "const sources: Sources = {",
        "const sources: Sources = {\n  refreshCard: ([date]) => [{date, props: {title: 'Next'}}],",
    );
    f.write("app.ts", &source);
    let baked = f.bake();
    assert_eq!(before, baked.plan);
    assert!(baked.declarations.contains("refreshCard"));
    for broken in [
        source.replace("refreshCard:", "renamed:"),
        source.replace("title: 'Next'", "title: 42"),
    ] {
        f.write("app.ts", &broken);
        let error = bake(&f.0, &Tools::default()).err().unwrap();
        assert!(
            error.contains("TS"),
            "the source contract must fail TypeScript: {error}"
        );
    }
}
