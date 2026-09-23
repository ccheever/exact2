//! Real Contract resource, input, paging and reparse behavior.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{DataSource, Event, Runner};
use markdown_stress_data::MarkdownStress;

fn boot() -> Runner<MarkdownStress> {
    let plan = contract::compile(include_str!("../../../app.contract")).unwrap();
    let baked = contract::bake(plan, MarkdownStress::default()).unwrap();
    Runner::boot(
        baked,
        MarkdownStress::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn press(r: &mut Runner<MarkdownStress>, name: &str) {
    let key = r.kernel().find_by_test_id(name)[0];
    r.dispatch(r.kernel().node_by_key(key).unwrap().id, Event::Press)
        .unwrap();
}

#[test]
fn paging_and_eager_mode_use_the_same_parsed_blocks() {
    let mut r = boot();
    press(&mut r, "profile-blocks");
    assert_eq!(r.kernel().find_by_test_id("block-0").len(), 1);
    assert_eq!(r.kernel().find_by_test_id("block-39").len(), 1);
    assert!(r.kernel().find_by_test_id("block-40").is_empty());
    press(&mut r, "next-page");
    assert!(r.kernel().find_by_test_id("block-0").is_empty());
    assert_eq!(r.kernel().find_by_test_id("block-40").len(), 1);
    press(&mut r, "toggle-eager");
    assert_eq!(r.kernel().find_by_test_id("block-0").len(), 1);
    assert_eq!(r.kernel().find_by_test_id("block-80").len(), 1);
    press(&mut r, "reset");
    assert_eq!(r.slot("eager"), Some(&Value::Bool(false)));
    assert_eq!(r.slot("size"), Some(&Value::Number(16_384.0)));
}

#[test]
fn typing_width_and_reparse_preserve_the_draft() {
    let mut r = boot();
    r.act("editDraft", vec![Value::str("Reading while reflowing 🦀")])
        .unwrap();
    press(&mut r, "width-wide");
    press(&mut r, "reparse");
    assert_eq!(
        r.slot("draft"),
        Some(&Value::str("Reading while reflowing 🦀"))
    );
    assert_eq!(r.slot("revision"), Some(&Value::Number(1.0)));
    press(&mut r, "start");
    for tick in 1..=22 {
        r.advance(tick as f64 * 1000.0).unwrap();
    }
    assert_eq!(r.slot("revision"), Some(&Value::Number(21.0)));
    assert_eq!(r.slot("running"), Some(&Value::Bool(false)));
    r.act("editDraft", vec![Value::str(&"x".repeat(513))])
        .unwrap();
    assert_eq!(r.slot("draftTooLong"), Some(&Value::Bool(true)));
    assert_eq!(
        r.slot("draft"),
        Some(&Value::str("Reading while reflowing 🦀"))
    );
}

#[test]
fn malformed_controls_are_refused_before_generation() {
    let mut source = MarkdownStress::default();
    for n in [f64::NAN, f64::INFINITY, -1.0, 16_384.5, 4_194_305.0] {
        assert!(source
            .query(
                "document",
                &[
                    Value::str("mixed"),
                    Value::Number(n),
                    Value::Number(0.0),
                    Value::Number(0.0),
                    Value::Bool(false),
                    Value::Bool(false)
                ]
            )
            .is_err());
    }
    assert!(source.query("document", &[]).is_err());
    assert!(source.query("unknown", &[]).is_err());
}
