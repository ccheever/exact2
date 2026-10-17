//! @ref LLP 1121 D3, D7 (stage 6 D) — shadow tokens: a `box-shadow` or
//! `text-shadow` row names one, the kernel reads the node's slice, a row of
//! the other kind refuses it, and a live edit (its own, or a colour token's
//! inside it) paints and lays nothing out.

use exact_kernel::{Kernel, StyleId};
use exact_plan::{Plan, Value};
use exact_runner::{DataError, DataSource, Runner};
use std::path::PathBuf;

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn app(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-shadows-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir.join("app.contract")
}

fn booted(plan: Plan) -> Runner<NoData> {
    let plan = contract::bake(plan, NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn row(r: &Runner<NoData>, id: &str, style: StyleId) -> String {
    let key = r.kernel().find_by_test_id(id)[0];
    format!("{:?}", r.kernel().node_by_key(key).unwrap().computed(style))
}

const DESIGN: &str = "tamber t\n\naxes\n  density = regular | compact\n\ntheme\n  shade = #00000040\n    when density=compact: #00000080\n  shadow.card: box-shadow = 0 1px 3px shade\n  glow: text-shadow = 0 0 2px #ff0000\n";

#[test]
fn a_shadow_token_takes_the_nodes_slice_and_a_live_edit_paints() {
    let path = app(
        "rows",
        &[
            ("app.tamber", DESIGN),
            ("app.contract", "component App\n  view\n    column testId=\"card\" box-shadow=\"shadow.card\"\n      text \"a\" testId=\"text\" text-shadow=\"glow\"\n      column testId=\"compact\" -exact-mode=\"density=compact\" box-shadow=\"shadow.card\"\n        text \"b\"\n"),
        ],
    );
    let plan = contract::compile_path(&path).unwrap();
    let kinds: Vec<(String, u8)> = plan
        .theme
        .iter()
        .map(|t| (plan.str(t.name).to_string(), t.kind))
        .collect();
    assert!(
        kinds.contains(&("shadow.card".into(), 3)) && kinds.contains(&("glow".into(), 4)),
        "{kinds:?}"
    );
    let mut r = booted(plan);
    let card = row(&r, "card", StyleId::BoxShadow);
    assert!(
        card.contains("offset: Vec2 { x: 0.0, y: 1.0 }") && card.contains("blur: 3.0"),
        "{card}"
    );
    let text = row(&r, "text", StyleId::TextShadow);
    assert!(text.contains("blur: 2.0"), "{text}");
    // The pin's slice: the shade a compact tuple holds.
    assert_ne!(row(&r, "compact", StyleId::BoxShadow), card);
    // A live edit of the shadow paints and lays nothing out.
    let (receipt, set) = r
        .set_token(
            "shadow.card",
            "0px 4px 8px #00000040",
            &["0px 4px 8px #00000040", "0px 4px 8px #00000080"],
        )
        .unwrap();
    assert_eq!(set, 2);
    assert!(!receipt.layout_invalidated, "a shadow paints");
    assert!(row(&r, "card", StyleId::BoxShadow).contains("blur: 8.0"));
}

#[test]
fn a_shadow_token_in_another_row_is_refused() {
    let refused = |name: &str, view: &str| {
        let path = app(name, &[("app.tamber", DESIGN), ("app.contract", view)]);
        contract::compile_path(&path).unwrap_err()
    };
    let e = refused(
        "box-in-text",
        "component App\n  view\n    text \"a\" text-shadow=\"shadow.card\"\n",
    );
    assert_eq!(e.id, "tamber-kind", "{e:?}");
    let e = refused(
        "text-in-box",
        "component App\n  view\n    column box-shadow=\"glow\"\n      text \"a\"\n",
    );
    assert_eq!(e.id, "tamber-kind", "{e:?}");
    let e = refused(
        "shadow-in-colour",
        "component App\n  view\n    column background-color=\"glow\"\n      text \"a\"\n",
    );
    assert_eq!(e.id, "tamber-kind", "{e:?}");
}

/// A colour token's edit reaches a shadow folded over it: the design
/// session moves the shadow's value too, live.
#[test]
fn a_colour_inside_a_shadow_follows_its_own_live_edit() {
    let files = vec![("app.tamber".to_string(), DESIGN.to_string())];
    let none: Vec<(String, bool)> = Vec::new();
    let synthetic = exact_tamber::Synthetic {
        file: "app.tamber",
        exprs: &none,
        retention: None,
    };
    let ops = vec![(
        "app.tamber".to_string(),
        exact_tamber::Op::SetToken {
            name: "shade".into(),
            when: Vec::new(),
            value: "#00000060".into(),
            at: None,
            comment: None,
        },
    )];
    let planned = exact_tamber::plan(
        &files,
        &ops,
        &contract_lower::tamber_vocab(),
        false,
        synthetic,
    )
    .unwrap();
    assert_eq!(planned.class, exact_tamber::Class::Live);
    let changed: Vec<&str> = planned.changed.iter().map(|(_, t)| t.as_str()).collect();
    assert!(
        changed.contains(&"shade") && changed.contains(&"shadow.card"),
        "{changed:?}"
    );
}
