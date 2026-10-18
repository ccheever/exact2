//! `exact_boot_entry`: a snapshot entry's plan boots by its kind and by
//! nothing else (LLP 1126 D2).

use crate::host::{handles, out, put};

fn entry_plan() -> Vec<u8> {
    let widget = contract::Widget {
        kind: "card".into(),
        title: None,
        description: None,
        families: vec!["systemSmall".into()],
        watch_families: vec![],
        component: "Card".into(),
        props: "Props".into(),
        refresh: "refresh".into(),
        preview: serde_json::json!({"title": "Hello"}),
    };
    let src = "shape Props\n  title: string\ncomponent Card\n  props\n    title: string\n  view\n    text title\n";
    contract::snapshot::compile(src, &widget).unwrap().encode()
}

#[test]
fn an_entry_plan_boots_by_its_kind_and_a_running_boot_refuses_it() {
    let plan = entry_plan();
    let rt = handles::exact_create();
    let len = put(rt, &plan);
    let refused = out(rt, handles::exact_boot_plan(rt, len, 200.0, 100.0));
    assert!(refused.contains("WrongEntry"), "{refused}");

    let mut bytes = plan.clone();
    bytes.extend_from_slice(b"other");
    put(rt, &bytes);
    let refused = out(
        rt,
        handles::exact_boot_entry(rt, plan.len(), 5, 0, 200.0, 100.0),
    );
    assert!(refused.contains("WrongEntry"), "{refused}");

    let mut bytes = plan.clone();
    bytes.extend_from_slice(b"card");
    put(rt, &bytes);
    let batch = out(
        rt,
        handles::exact_boot_entry(rt, plan.len(), 4, 0, 200.0, 100.0),
    );
    assert!(batch.contains("\"text\":\"Hello\""), "{batch}");
    assert!(batch.contains("\"error\":null"), "{batch}");
    handles::exact_destroy(rt);
}
