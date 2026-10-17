//! @ref LLP 1121 D3, D13 (stage 6 E) — duration and easing tokens: written
//! into a `transition` or `animation` value at build, so the plan holds the
//! values, a `when` on one is refused, and an edit to one rebuilds.

use std::path::PathBuf;

fn app(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-motion-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir.join("app.contract")
}

const DESIGN: &str =
    "tamber t\n\ntheme\n  duration.fast = 150ms\n  easing.standard = cubic-bezier(0.2, 0, 0, 1)\n";
const VIEW: &str = "keyframes appear\n  from opacity=0\n  to opacity=1\n\ncomponent App\n  view\n    column transition=\"background-color duration.fast easing.standard\"\n      text \"a\" animation=\"appear duration.fast easing.standard\"\n";

#[test]
fn a_motion_token_is_written_into_its_transition_and_animation() {
    let path = app("rows", &[("app.tamber", DESIGN), ("app.contract", VIEW)]);
    let plan = contract::compile_path(&path).unwrap();
    let strings: Vec<&str> = plan.strings.iter().map(String::as_str).collect();
    assert!(
        strings.contains(&"background-color 150ms cubic-bezier(0.2, 0, 0, 1)"),
        "{strings:?}"
    );
    assert!(
        strings.contains(&"appear 150ms cubic-bezier(0.2, 0, 0, 1)"),
        "{strings:?}"
    );
    // The plan keeps no motion token: the rows hold the values.
    assert!(plan.theme.is_empty(), "{:?}", plan.theme.len());
    // An edit is structure: the plan the dev loop compares differs.
    std::fs::write(
        path.with_file_name("app.tamber"),
        DESIGN.replace("150ms", "300ms"),
    )
    .unwrap();
    let edited = contract::compile_path(&path).unwrap();
    assert_ne!(
        exact_runner::design::structure(&plan),
        exact_runner::design::structure(&edited)
    );
}

#[test]
fn a_motion_token_varies_by_no_axis_and_is_no_other_rows() {
    let varying = "tamber t\n\naxes\n  density = regular | compact\n\ntheme\n  duration.fast = 150ms\n    when density=compact: 100ms\n";
    let path = app(
        "axis",
        &[
            ("app.tamber", varying),
            ("app.contract", "component App\n  view\n    text \"a\"\n"),
        ],
    );
    let e = contract::compile_path(&path).unwrap_err();
    assert_eq!(e.id, "tamber-motion-axis", "{e:?}");
    let path = app(
        "other-row",
        &[
            ("app.tamber", DESIGN),
            (
                "app.contract",
                "component App\n  view\n    column padding=\"duration.fast\"\n      text \"a\"\n",
            ),
        ],
    );
    let e = contract::compile_path(&path).unwrap_err();
    assert_eq!(e.id, "tamber-kind", "{e:?}");
}

/// An edit to a motion token, or to a dial it folds from, rebuilds: the
/// session's class, which scrubs and the live table read.
#[test]
fn a_motion_edit_proposes_a_rebuild() {
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
            name: "duration.fast".into(),
            when: Vec::new(),
            value: "200ms".into(),
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
    assert_eq!(planned.class, exact_tamber::Class::Rebuild);
}
