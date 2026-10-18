use super::{bake, Fixture, Tools, CONTRACT, SOURCE};

#[test]
fn widget_refresh_has_a_checked_source_without_changing_the_app_plan() {
    if !exact_js::ENGINE_LINKED {
        return;
    }
    let f = Fixture::new();
    f.write("app.contract", &format!("{CONTRACT}\nshape CardProps\n  title: string\ncomponent Card\n  props\n    title: string\n  view\n    text title\n"));
    let stateful = SOURCE
        .replace(
            "const sources: Sources = {",
            "let refreshed = 0;\nconst sources: Sources = {",
        )
        .replace(
            "return prefix + count;",
            "return prefix + (count + refreshed);",
        );
    f.write("app.ts", &stateful);
    let before = f.bake().plan;
    f.write("app.json", r#"{"app":{"id":"test.exact.logic","name":"Logic"},"widgets":[{"kind":"card","families":["systemSmall"],"component":"Card","props":"CardProps","refresh":"refreshCard","preview":{"title":"Preview"}}]}"#);
    let source = stateful.replace(
        "const sources: Sources = {",
        "const sources: Sources = {\n  refreshCard: ([date]) => { refreshed++; return [{date, props: {title: 'Next'}}]; },",
    );
    f.write("app.ts", &source);
    let baked = f.bake();
    assert_eq!(before, baked.plan);
    assert!(baked.declarations.contains("refreshCard"));
    let async_source = source.replace("refreshCard: ([date]) => { refreshed++;", "refreshCard: async ([date]) => { refreshed++; await fetch('https://example.test/refresh');")
        .replace("export const grants = '';", "export const grants = 'net.fetch https://example.test/';");
    f.write("app.ts", &async_source);
    let mut producer = exact_js_bake::Producer::new(Tools::default()).unwrap();
    let pending = producer.bake(&f.0, None).unwrap();
    assert_eq!(
        before, pending.plan,
        "the refresh must have a fresh module instance"
    );
    assert_eq!(pending.entries.len(), 1);
    // The producer receipt admits the exact pruned entry used by a widget.
    use exact_runner::DataSource;
    let mut module = exact_js::Module::new(
        Vec::new(),
        "test.exact.logic",
        "net.fetch https://example.test/",
    );
    module
        .prepare_snapshot(
            Some(exact_runner::SnapshotModule {
                receipt: &pending.receipt,
                bytecode: &pending.bytecode,
                plan: &pending.entries["card"].plan.encode(),
                kind: "card",
            }),
            std::time::Instant::now() + std::time::Duration::from_secs(30),
        )
        .unwrap();
    let output = f.0.join("output");
    pending.write_new(&output).unwrap();
    assert_eq!(
        std::fs::read(output.join("plans/card.plan")).unwrap(),
        pending.entries["card"].plan.encode()
    );
    std::fs::remove_dir_all(&output).unwrap();
    let seed = exact_js_bake::Seed::new(&before, &["refreshCard".into()]).unwrap();
    // This producer cannot run Rust's refresh. Its declared signature remains
    // in Sources, even though the source has no app resource to seed from.
    assert_eq!(producer.bake(&f.0, Some(&seed)).unwrap().plan, before);
    for broken in [
        async_source.replace("refreshCard:", "renamed:"),
        async_source.replace("title: 'Next'", "title: 42"),
    ] {
        f.write("app.ts", &broken);
        let error = bake(&f.0, &Tools::default()).err().unwrap();
        assert!(
            error.contains("TS"),
            "the source contract must fail TypeScript: {error}"
        );
    }
}
