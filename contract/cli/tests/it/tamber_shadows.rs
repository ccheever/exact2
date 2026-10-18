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
        params: &exact_tamber::params::NONE,
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

/// Review round 1: `uses` finds a shadow token's rows (its own row kind).
#[test]
fn uses_finds_a_shadow_tokens_rows() {
    let path = app(
        "uses",
        &[
            ("app.tamber", DESIGN),
            ("app.contract", "component App\n  view\n    column box-shadow=\"shadow.card\"\n      text \"a\" text-shadow=\"glow\"\n"),
        ],
    );
    let (plan, map) = contract::compile_path_mapped(&path).unwrap();
    let map: serde_json::Value = serde_json::from_str(&map.json(&plan.encode())).unwrap();
    for (token, row) in [("shadow.card", "box_shadow"), ("glow", "text_shadow")] {
        let sites = contract::tamber_uses::sites(&plan, &map, token, &Default::default(), &[]);
        assert_eq!(sites.len(), 1, "{token}: {sites:?}");
        assert_eq!(sites[0]["row"], row, "{sites:?}");
    }
}

/// Review round 2: a length token inside a shadow is read where the
/// shadow is (`uses` of the length lists the shadow's row too).
#[test]
fn uses_of_a_length_lists_a_shadow_folded_over_it() {
    let design = "tamber t\n\ntheme\n  space.card = 4px\n  shadow.card: box-shadow = 0 space.card 12px #000\n";
    let path = app(
        "uses-length",
        &[
            ("app.tamber", design),
            ("app.contract", "component App\n  view\n    column padding-top=\"space.card\" box-shadow=\"shadow.card\"\n      text \"a\"\n"),
        ],
    );
    let (plan, map) = contract::compile_path_mapped(&path).unwrap();
    let map: serde_json::Value = serde_json::from_str(&map.json(&plan.encode())).unwrap();
    let aliases: std::collections::BTreeSet<String> = ["shadow.card".to_string()].into();
    let sites = contract::tamber_uses::sites(&plan, &map, "space.card", &aliases, &[]);
    let rows: Vec<&str> = sites.iter().filter_map(|s| s["row"].as_str()).collect();
    assert!(
        rows.contains(&"padding_top") && rows.contains(&"box_shadow"),
        "{sites:?}"
    );
}

const MIXES: &str = "tamber t\n\ntheme\n  c.ink = light-dark(#000000, #ffffff)\n";

/// A view with `color-mix()` over a token in a choice's arm and in a
/// shadow, and an arm that is no colour.
fn mixes(view_extra: &str) -> String {
    format!("component App\n  state on = false\n  view\n    column\n      box background-color=(on ? \"color-mix(in srgb, c.ink 35%, transparent)\" : \"transparent\")\n      box box-shadow=(on ? \"0 0 8px color-mix(in srgb, c.ink 60%, transparent)\" : \"none\")\n      box border-color=(on ? \"color-mix(in srgb, c.ink 70%, transparent)\" : \"notacolour\")\n{view_extra}")
}

/// The euxy port: a mix over a token in a choice's arm or a shadow was
/// refused as no colour whenever the type checker refused another line,
/// while the same mix written whole was not. The build folds it in every
/// arm; the arm that is no colour is still named.
#[test]
fn a_mix_over_a_token_in_an_arm_or_a_shadow_is_folded_beside_another_refusal() {
    let lines = |errors: &[contract::CompileError]| -> Vec<(String, u32)> {
        errors.iter().map(|e| (e.id.clone(), e.span.line)).collect()
    };
    let refused = |name: &str, extra: &str| {
        let path = app(
            name,
            &[("app.tamber", MIXES), ("app.contract", &mixes(extra))],
        );
        contract::compile_path_all(&path, false).err().unwrap()
    };
    let errors = refused("mix-arms", "      box class=\"card\"\n");
    assert_eq!(
        lines(&errors),
        [
            ("type-class-name".into(), 8),
            ("tamber-unknown-token".into(), 7)
        ],
        "{errors:#?}"
    );
    assert!(
        errors[1].message.contains("notacolour"),
        "{}",
        errors[1].message
    );
    let errors = refused("mix-arms-clean", "");
    assert_eq!(
        lines(&errors),
        [("tamber-unknown-token".into(), 7)],
        "{errors:#?}"
    );
}

/// A mix of colours alone is no colour a host reads: the refusal names the
/// token to declare, with the value written over it, and that edit builds.
#[test]
fn a_mix_of_colours_alone_names_the_token_to_declare() {
    let view = |bg: &str, shadow: &str| {
        format!("component App\n  state on = false\n  view\n    column\n      box background-color=(on ? \"{bg}\" : \"transparent\")\n      box box-shadow=\"0 0 8px {shadow}\"\n")
    };
    let mix = "color-mix(in srgb, red 35%, transparent)";
    let path = app("mix-literal", &[("app.contract", &view(mix, mix))]);
    let errors = contract::compile_path_all(&path, false).err().unwrap();
    let messages: Vec<&str> = errors.iter().map(|e| e.message.as_str()).collect();
    let lead = "declare the mix as a token in app.tamber's `theme` (a new app.tamber's first line is `tamber <name>`), `mixed = color-mix(in srgb, red 35%, transparent)`, and write";
    assert_eq!(messages.len(), 2, "{messages:#?}");
    assert!(
        messages[0].starts_with(&format!("{lead} `background-color=\"mixed\"`")),
        "{}",
        messages[0]
    );
    assert!(
        messages[1].starts_with(&format!("{lead} `box-shadow=\"0 0 8px mixed\"`")),
        "{}",
        messages[1]
    );
    let design = format!("tamber t\n\ntheme\n  mixed = {mix}\n");
    let path = app(
        "mix-literal-edit",
        &[
            ("app.tamber", &design),
            ("app.contract", &view("mixed", "mixed")),
        ],
    );
    contract::compile_path(&path).unwrap_or_else(|e| panic!("{e}"));
}
