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

/// Review round 1: a token in a timing's place only (never a transition's
/// property or an animation's keyframes name), written in before the
/// keyframes' scoping and the longhands' composing read the value, and
/// through a component's prop.
#[test]
fn a_motion_token_is_written_in_a_timings_place_before_anything_reads_it() {
    let design = "tamber t\n\ntheme\n  opacity = 150ms\n  fast = 200ms\n  duration.fast = 150ms\n";
    let view = "keyframes fast\n  from opacity=0\n  to opacity=1\n\ncomponent App\n  view\n    column transition=\"opacity 1s\"\n      text \"a\" animation=\"fast 1s\"\n      text \"b\" animation=\"fast duration.fast\" animation-fill-mode=\"forwards\"\n      Card(t=\"opacity duration.fast\")\n\ncomponent Card\n  props\n    t: string\n  view\n    column transition=t\n      text \"c\"\n";
    let path = app("places", &[("app.tamber", design), ("app.contract", view)]);
    let plan = contract::compile_path(&path).unwrap();
    let strings: Vec<&str> = plan.strings.iter().map(String::as_str).collect();
    assert!(strings.contains(&"opacity 1s"), "{strings:?}");
    assert!(strings.contains(&"fast 1s"), "{strings:?}");
    // Composed with its longhand: the token's time is the shorthand's.
    assert!(
        strings.contains(&"0.15s ease 0s 1 normal forwards running fast"),
        "{strings:?}"
    );
    assert!(strings.contains(&"opacity 150ms"), "{strings:?}");
}

#[test]
fn uses_finds_a_motion_tokens_words_in_the_sources() {
    let path = app("uses", &[("app.tamber", DESIGN), ("app.contract", VIEW)]);
    let sites = contract::tamber_uses::motion_sites(path.parent().unwrap(), "duration.fast");
    assert_eq!(sites.len(), 2, "{sites:?}");
    assert_eq!(sites[0]["row"], "transition", "{sites:?}");
    assert_eq!(sites[1]["row"], "animation", "{sites:?}");
}

/// Review round 1: a used file's animation naming its own keyframes and a
/// motion token keeps its keyframes (scoping reads the value after the
/// token is written in).
#[test]
fn a_used_files_animation_keeps_its_own_keyframes() {
    let path = app(
        "scoped",
        &[
            ("app.tamber", DESIGN),
            ("app.contract", "use Spinner from \"./lib/ui.contract\"\nkeyframes spin\n  to opacity=0\ncomponent App\n  view\n    column\n      text \"a\" animation=\"spin 1s\"\n      Spinner()\n"),
        ],
    );
    std::fs::create_dir_all(path.with_file_name("lib")).unwrap();
    std::fs::write(
        path.with_file_name("lib").join("ui.contract"),
        "keyframes spin\n  to opacity=1\ncomponent Spinner\n  view\n    text \"b\" animation=\"spin duration.fast easing.standard\"\n",
    )
    .unwrap();
    let plan = contract::compile_path(&path).unwrap();
    let strings: Vec<&str> = plan.strings.iter().map(String::as_str).collect();
    let used = strings
        .iter()
        .find(|s| s.contains("150ms"))
        .unwrap_or_else(|| panic!("{strings:?}"));
    assert!(
        used.starts_with("spin") && !used.starts_with("spin "),
        "renamed keyframes: {used}"
    );
}

/// Review round 1 (Grok): the other copies of the grammars take tokens
/// too: an exit animation, a layout transition, a keyframe's timing.
#[test]
fn an_exit_animation_a_layout_transition_and_a_keyframes_timing_take_tokens() {
    let view = "keyframes appear\n  from opacity=0 animation-timing-function=\"easing.standard\"\n  to opacity=1\n\ncomponent App\n  state on = true\n  view\n    column -exact-layout-transition=\"duration.fast easing.standard\"\n      if on\n        text \"a\" -exact-exit-animation=\"appear duration.fast\"\n";
    let path = app("copies", &[("app.tamber", DESIGN), ("app.contract", view)]);
    let plan = contract::compile_path(&path).unwrap_or_else(|e| panic!("{e:?}"));
    let strings: Vec<&str> = plan.strings.iter().map(String::as_str).collect();
    assert!(
        !strings
            .iter()
            .any(|s| s.contains("duration.fast") || s.contains("easing.standard")),
        "{strings:?}"
    );
}
