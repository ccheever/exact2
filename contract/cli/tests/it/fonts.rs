//! LLP 1019 D1–D3/§5: declared fonts lower to plan identity, and every
//! portable refusal happens before a host can substitute a font.

use exact_kernel::{FontStyle, Kernel};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

struct SelectedFont;
impl DataSource for SelectedFont {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "selectedFont" => Ok(Value::str("Menlo")),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

struct AppDir(PathBuf);

impl AppDir {
    fn new(source: &str) -> AppDir {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "exact-contract-fonts-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/fixtures/fonts");
        for name in ["DejaVuSans.ttf", "DejaVuSans-Bold.ttf"] {
            std::fs::copy(
                fixtures.join("assets").join(name),
                dir.join("assets").join(name),
            )
            .unwrap();
        }
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../host/apple/tests/fonts/InterVariable.ttf"),
            dir.join("assets/InterVariable.ttf"),
        )
        .unwrap();
        std::fs::write(dir.join("app.contract"), source).unwrap();
        AppDir(dir)
    }

    fn compile(&self) -> Result<exact_plan::Plan, contract::CompileError> {
        contract::compile_path(&self.0.join("app.contract"))
    }
}

impl Drop for AppDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const DECLARATION: &str = "font \"Fixture Sans\"\n  400 = \"assets/DejaVuSans.ttf\"\n  700 = \"assets/DejaVuSans-Bold.ttf\"\n";

#[test]
fn declared_faces_and_stack_identity_reach_the_kernel_row() {
    let app = AppDir::new(&format!(
        "{DECLARATION}component App\n  view\n    text \"Change station\" font-family=\"Fixture Sans\" font-weight=600 font-style=\"normal\" testId=\"label\"\n"
    ));
    let plan = app.compile().unwrap();
    assert_eq!(plan.faces.len(), 2);
    assert_eq!(plan.str(plan.faces[0].source), "assets/DejaVuSans.ttf");
    assert_eq!(plan.str(plan.families[0].name), "Fixture Sans");
    assert_eq!(plan.stacks.len(), 9);

    let runner = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let kernel = runner.kernel();
    let node = kernel
        .node_by_key(kernel.find_by_test_id("label")[0])
        .unwrap();
    assert_eq!(node.style.font_family, 8);
    assert_eq!(node.style.font_weight, 600);
    assert_eq!(node.style.font_style, FontStyle::Normal);
}

#[test]
fn shorthand_declares_one_400_normal_face() {
    let app = AppDir::new(
        "font \"Fixture Sans\" = \"assets/DejaVuSans.ttf\"\ncomponent App\n  view\n    text \"hello\" font-family=\"Fixture Sans\"\n",
    );
    let plan = app.compile().unwrap();
    assert_eq!(plan.faces.len(), 1);
    assert_eq!((plan.faces[0].weight, plan.faces[0].weight_max), (400, 400));
    assert_eq!(
        (plan.faces[0].stretch_min, plan.faces[0].stretch_max),
        (1000, 1000)
    );
    assert!(!plan.faces[0].italic);
}

/// LLP 1019 D2: a variable face's range is its `@font-face` descriptor,
/// read from the file's `wght` axis when the declaration leaves it unsaid.
#[test]
fn a_variable_face_declares_its_file_range_or_the_written_one() {
    let app = AppDir::new(
        "font \"Inter\" = \"assets/InterVariable.ttf\"\nfont \"Pinned\"\n  600 = \"assets/InterVariable.ttf\"\nfont \"Ranged\"\n  300 700 = \"assets/InterVariable.ttf\"\ncomponent App\n  view\n    text \"hello\" font-family=\"Inter\" font-weight=650 font-stretch=\"condensed\" testId=\"label\"\n",
    );
    let plan = app.compile().unwrap();
    let weights: Vec<_> = plan
        .faces
        .iter()
        .map(|f| (f.weight, f.weight_max))
        .collect();
    assert_eq!(weights, [(100, 900), (600, 600), (300, 700)]);
    // Inter has no `wdth` axis: each face is normal width.
    assert!(plan
        .faces
        .iter()
        .all(|f| (f.stretch_min, f.stretch_max) == (1000, 1000)));

    let runner = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let kernel = runner.kernel();
    let node = kernel
        .node_by_key(kernel.find_by_test_id("label")[0])
        .unwrap();
    assert_eq!(node.style.font_weight, 650);
    assert_eq!(node.style.font_stretch, 75.0);
}

/// LLP 1019 D2: one variable file declared at several weights is one face
/// over their span, so the web fetches it once; a span another face of the
/// same style overlaps, a static file, and the italics stay apart.
#[test]
fn pinned_weights_of_one_variable_file_are_one_face() {
    let app = AppDir::new(
        "font \"Inter\"\n  400 = \"assets/InterVariable.ttf\"\n  500 = \"assets/InterVariable.ttf\"\n  700 = \"assets/InterVariable.ttf\"\n  400 italic = \"assets/InterVariable.ttf\"\nfont \"Split\"\n  400 = \"assets/InterVariable.ttf\"\n  500 = \"assets/DejaVuSans.ttf\"\n  700 = \"assets/InterVariable.ttf\"\nfont \"Static\"\n  400 = \"assets/DejaVuSans.ttf\"\n  600 = \"assets/DejaVuSans.ttf\"\ncomponent App\n  view\n    text \"hello\" font-family=\"Inter\"\n",
    );
    let plan = app.compile().unwrap();
    let faces: Vec<_> = plan
        .families
        .iter()
        .map(|family| {
            family
                .faces
                .iter()
                .map(|id| {
                    let f = plan.face(id);
                    (f.weight, f.weight_max, f.italic)
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(
        faces,
        [
            vec![(400, 700, false), (400, 400, true)],
            vec![(400, 400, false), (500, 500, false), (700, 700, false)],
            vec![(400, 400, false), (600, 600, false)],
        ]
    );
}

/// LLP 1019 §13: `font-optical-sizing` is CSS's, inherited, and reaches
/// the run style a host draws with.
#[test]
fn font_optical_sizing_reaches_the_run_style() {
    let app = AppDir::new(
        "component App\n  view\n    column\n      column font-optical-sizing=\"none\"\n        text \"hello\" testId=\"label\"\n      text \"auto\" testId=\"auto\"\n",
    );
    let runner = Runner::boot(
        app.compile().unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let kernel = runner.kernel();
    let sizing = |id: &str| {
        let key = kernel.find_by_test_id(id)[0];
        kernel
            .node_by_key(key)
            .unwrap()
            .text_style()
            .font_optical_sizing
    };
    assert_eq!(sizing("label"), exact_kernel::FontOpticalSizing::None);
    assert_eq!(sizing("auto"), exact_kernel::FontOpticalSizing::Auto);
    refusal(
        "component App\n  view\n    text \"hello\" font-optical-sizing=\"off\"\n",
        "lower-attr-value",
    );
}

/// LLP 1019 D9: `font-variation-settings` reaches the kernel's row as CSS
/// writes it, and a value CSS refuses is refused at compile.
#[test]
fn font_variation_settings_reach_the_row_and_bad_ones_are_refused() {
    let app = AppDir::new(
        "font \"Inter\" = \"assets/InterVariable.ttf\"\ncomponent App\n  view\n    text \"hello\" font-family=\"Inter\" font-variation-settings=`\"wght\" 650, \"opsz\" 32` testId=\"label\"\n",
    );
    let plan = app.compile().unwrap();
    let runner = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let kernel = runner.kernel();
    let node = kernel
        .node_by_key(kernel.find_by_test_id("label")[0])
        .unwrap();
    let v = node.style.font_variation_settings;
    assert_eq!((v.get(b"wght"), v.get(b"opsz")), (Some(650.0), Some(32.0)));
    refusal(
        "component App\n  view\n    text \"hello\" font-variation-settings=\"wght 650\"\n",
        "lower-attr-value",
    );
}

fn refusal(source: &str, id: &str) {
    let app = AppDir::new(source);
    let error = app.compile().unwrap_err();
    assert_eq!(error.id, id, "{error}");
}

#[test]
fn family_identity_and_sources_fail_closed() {
    // An empty family declares nothing (borders.rs); an empty member is refused.
    refusal(
        "component App\n  view\n    text \"x\" font-family=\"serif,, sans-serif\"\n",
        "lower-font-family-list",
    );
    // An unquoted member is a sequence of CSS identifiers, as the run-time
    // check (exact_plan::is_css_family_name) and the browser read it.
    for value in [
        "Arial;",
        "1Arial",
        "Arial 2",
        "Arial!",
        "default",
        "serif Foo",
    ] {
        refusal(
            &format!("component App\n  view\n    text \"x\" font-family=\"{value}\"\n"),
            "lower-font-family-list",
        );
    }
    for value in ["Helvetica Neue", "'Arial 2'", "Foo default", "emoji Foo"] {
        contract::compile(&format!(
            "component App\n  view\n    text \"x\" font-family=\"{value}\"\n"
        ))
        .unwrap();
    }
    refusal(
        "font \"Missing\" = \"assets/nope.ttf\"\ncomponent App\n  view\n    text \"x\"\n",
        "lower-font-unreadable",
    );
    refusal(
        "font \"Web Only\" = \"assets/font.woff2\"\ncomponent App\n  view\n    text \"x\"\n",
        "lower-font-format",
    );
    refusal(
        "font \"Escape\" = \"../DejaVuSans.ttf\"\ncomponent App\n  view\n    text \"x\"\n",
        "lower-font-path",
    );
    refusal(
        "font \"Root\" = \"DejaVuSans.ttf\"\ncomponent App\n  view\n    text \"x\"\n",
        "lower-font-path",
    );
    refusal(
        "font \"Fixture Sans\" = \"assets/DejaVuSans.ttf\"\nfont \"fixture sans\" = \"assets/DejaVuSans.ttf\"\ncomponent App\n  view\n    text \"x\"\n",
        "lower-font-duplicate",
    );
}

#[test]
fn a_literal_request_that_would_need_synthesis_is_refused() {
    refusal(
        "font \"Regular Only\" = \"assets/DejaVuSans.ttf\"\ncomponent App\n  view\n    text \"x\" font-family=\"Regular Only\" font-weight=700\n",
        "lower-font-face",
    );
    refusal(
        &format!(
            "{DECLARATION}component App\n  view\n    text \"x\" font-family=\"Fixture Sans\" font-style=\"italic\"\n"
        ),
        "lower-font-face",
    );
    refusal(
        "font \"Italic Only\"\n  400 italic = \"assets/DejaVuSans.ttf\"\ncomponent App\n  view\n    text \"x\" font-family=\"Italic Only\" font-style=\"normal\"\n",
        "lower-font-face",
    );
}

#[test]
fn conditional_literal_faces_require_real_declared_faces() {
    for style in [
        r#"font-style=(on ? "italic" : "normal")"#,
        "font-weight=(on ? 700 : 400)",
    ] {
        refusal(&format!(
            "font \"Regular Only\" = \"assets/DejaVuSans.ttf\"\ncomponent App\n  state on = false\n  view\n    text \"x\" font-family=\"Regular Only\" {style}\n"
        ), "lower-font-face");
    }
    AppDir::new(&format!(
        "{DECLARATION}component App\n  state on = false\n  view\n    text \"x\" font-family=\"Fixture Sans\" font-weight=(on ? 700 : 400)\n"
    )).compile().unwrap();
}

#[test]
fn duplicate_direct_attributes_cannot_bypass_the_face_diagnostic() {
    refusal(
        "font \"HasBold\"\n  400 = \"assets/DejaVuSans.ttf\"\n  700 = \"assets/DejaVuSans-Bold.ttf\"\nfont \"OnlyRegular\" = \"assets/DejaVuSans.ttf\"\ncomponent App\n  view\n    text \"hi\" font-family=\"HasBold\" font-family=\"OnlyRegular\" font-weight=700\n",
        "syntax-duplicate-attr",
    );
}

#[test]
fn duplicate_style_attributes_cannot_bypass_the_face_diagnostic() {
    refusal(
        "font \"HasBold\"\n  400 = \"assets/DejaVuSans.ttf\"\n  700 = \"assets/DejaVuSans-Bold.ttf\"\nfont \"OnlyRegular\" = \"assets/DejaVuSans.ttf\"\nstyle Dup\n  font-family=\"HasBold\"\n  font-family=\"OnlyRegular\"\n  font-weight=700\ncomponent A\n  view\n    text \"hi\" class=Dup font-size=13\n",
        "syntax-duplicate-attr",
    );
}

/// LLP 1053 G7: a choice between literal families compiles each arm to its
/// stack id; the face checks cover every declared arm.
#[test]
fn a_choice_of_literal_families_resolves_each_arm_to_its_stack() {
    let app = AppDir::new(&format!(
        "{DECLARATION}component App\n  state mono = true\n  state picked = some(\"x\")\n  view\n    column\n      text \"a\" font-family=(mono ? \"ui-monospace\" : \"Fixture Sans\") testId=\"ternary\"\n      text \"b\" font-family=(mono ? \"serif\" : \"system-ui\") font-weight=700 testId=\"generic\"\n      text \"c\" font-family=match picked {{ case some(p) => \"Fixture Sans\", case none => \"serif\" }} testId=\"match\"\n"
    ));
    let plan = app.compile().unwrap();
    let runner = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let kernel = runner.kernel();
    let family = |id: &str| {
        kernel
            .node_by_key(kernel.find_by_test_id(id)[0])
            .unwrap()
            .style
            .font_family
    };
    // Generic stacks are 0–7 (`ui-monospace` is 5, `serif` 4); the declared
    // family is 8.
    assert_eq!(family("ternary"), 5);
    assert_eq!(family("generic"), 4);
    assert_eq!(family("match"), 8);
}

#[test]
fn a_state_string_can_name_an_installed_family_at_runtime() {
    for source in [
        "state family = \"Arial\"",
        "resource family = selectedFont() as shape string",
    ] {
        let plan = contract::compile(&format!(
            "component App\n  {source}\n  view\n    text \"The quick brown fox\" font-family=family testId=\"sample\"\n"
        ))
        .unwrap();
        assert!(plan.nodes.iter().any(|node| node.bindings.iter().any(|id| {
            let binding = plan.binding(id);
            binding.kind == exact_plan::BindingKind::Style
                && binding.id == exact_kernel::StyleId::FontFamily as u16
        })));
    }
}

#[test]
fn runner_interns_a_runtime_family_above_plan_stacks() {
    let plan = contract::compile(
        "component App\n  state family = \"Arial\"\n  action pick(name: string)\n    family = name\n  view\n    text \"Mmmm\" font-family=family testId=\"sample\"\n",
    )
    .unwrap();
    let count = plan.stacks.len();
    let mut runner = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let family = |runner: &Runner<NoData>| {
        let kernel = runner.kernel();
        kernel
            .node_by_key(kernel.find_by_test_id("sample")[0])
            .unwrap()
            .style
            .font_family
    };
    assert_eq!(family(&runner) as usize, count);
    assert_eq!(runner.runtime_font_stacks(), vec!["Arial".to_string()]);
    let changed = runner
        .act("pick", vec![Value::str("Menlo, ui-monospace")])
        .unwrap();
    assert!(changed.layout_invalidated);
    assert_eq!(family(&runner) as usize, count + 1);
    assert_eq!(
        runner.runtime_font_stacks(),
        ["Arial", "Menlo, ui-monospace"]
    );
    runner.act("pick", vec![Value::str("Arial")]).unwrap();
    assert_eq!(family(&runner) as usize, count);
}

#[test]
fn invalid_runtime_family_values_leave_the_inherited_family() {
    let plan = contract::compile(
        "component App\n  state family = \"Menlo\"\n  action pick(name: string)\n    family = name\n  view\n    column font-family=\"serif\"\n      text \"child\" font-family=family testId=\"sample\"\n",
    ).unwrap();
    let mut runner = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let family = |runner: &Runner<NoData>| {
        let kernel = runner.kernel();
        let node = kernel
            .node_by_key(kernel.find_by_test_id("sample")[0])
            .unwrap();
        node.text_style().font_family
    };
    // Each is refused by the browser, or is a CSS-wide keyword that
    // inherits: the parent's `serif` (stack 4).
    for value in [
        "inherit",
        "unset",
        "revert",
        "revert-layer",
        "serif,,monospace",
        "Arial;",
        "Arial, 1x",
        "default",
        "serif Foo",
    ] {
        runner.act("pick", vec![Value::str("Menlo")]).unwrap();
        runner.act("pick", vec![Value::str(value)]).unwrap();
        assert_eq!(family(&runner), 4, "{value}");
    }
    assert_eq!(runner.runtime_font_stacks(), ["Menlo"]);
    // `initial` is the row's initial value, the kernel's family 0
    // (`system-ui`), not the parent's.
    runner.act("pick", vec![Value::str("initial")]).unwrap();
    assert_eq!(family(&runner), 0);
    assert_eq!(runner.runtime_font_stacks(), ["Menlo"]);
}

#[test]
fn an_optional_runtime_family_uses_its_inner_string() {
    let plan = contract::compile(
        "component App\n  state family = some(\"Menlo\")\n  view\n    text \"Mmmm\" font-family=family testId=\"sample\"\n",
    )
    .unwrap();
    let count = plan.stacks.len();
    let runner = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let kernel = runner.kernel();
    let sample = kernel
        .node_by_key(kernel.find_by_test_id("sample")[0])
        .unwrap();
    assert_eq!(sample.style.font_family as usize, count);
}

#[test]
fn a_data_source_family_is_interned_even_when_its_name_is_absent_from_source() {
    let plan = contract::compile(
        "component App\n  resource family = selectedFont() as shape string\n  view\n    text \"Mmmm\" font-family=family testId=\"sample\"\n",
    )
    .unwrap();
    let count = plan.stacks.len();
    let runner = Runner::boot(
        plan,
        SelectedFont,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let kernel = runner.kernel();
    let sample = kernel
        .node_by_key(kernel.find_by_test_id("sample")[0])
        .unwrap();
    assert_eq!(sample.style.font_family as usize, count);
    assert_eq!(runner.runtime_font_stacks(), ["Menlo"]);
}

#[test]
fn a_family_choice_validates_its_literal_arms() {
    refusal(
        "component App\n  state on = false\n  state family = \"serif\"\n  view\n    text \"x\" font-family=(on ? \"serif,, monospace\" : \"serif\")\n",
        "lower-font-family-list",
    );
    // An empty arm declares nothing, as an empty colour does.
    for value in [
        "family",
        "(on ? family : \"serif\")",
        "`${family}`",
        "(on ? \"\" : \"serif\")",
    ] {
        contract::compile(&format!(
            "component App\n  state on = false\n  state family = \"serif\"\n  view\n    text \"x\" font-family={value}\n"
        ))
        .unwrap();
    }
}

#[test]
fn a_constant_family_template_uses_literal_stack_validation() {
    let plan = contract::compile(
        "component App\n  view\n    text \"x\" font-family=`Arial, ui-monospace`\n",
    )
    .unwrap();
    assert!(plan.stacks.len() > 8);
    refusal(
        "component App\n  view\n    text \"x\" font-family=`serif,, monospace`\n",
        "lower-font-family-list",
    );
    refusal(
        "component App\n  view\n    text \"x\" font-family=`cursive`\n",
        "lower-font-family-generic",
    );
    refusal(
        &format!("{DECLARATION}component App\n  view\n    text \"x\" font-family=`Fixture Sans` font-style=\"italic\"\n"),
        "lower-font-face",
    );
    refusal(
        &format!("{DECLARATION}component App\n  state family = \"Arial\"\n  state on = false\n  view\n    text \"x\" font-family=(on ? `fixture sans` : family)\n"),
        "lower-font-runtime-bundled",
    );
}

#[test]
fn a_runtime_number_is_not_a_family_list() {
    refusal(
        "component App\n  state family = 42\n  view\n    text \"x\" font-family=family\n",
        "lower-attr-type",
    );
}

#[test]
fn a_runtime_string_does_not_substitute_an_installed_face_for_a_bundled_literal() {
    refusal(
        &format!("{DECLARATION}component App\n  state family = \"Arial\"\n  state on = false\n  view\n    text \"x\" font-family=(on ? \"Fixture Sans\" : family)\n"),
        "lower-font-runtime-bundled",
    );
    refusal(
        &format!("{DECLARATION}component App\n  state family = \"Arial\"\n  state on = false\n  view\n    text \"x\" font-family=(on ? \"fixture sans\" : family)\n"),
        "lower-font-runtime-bundled",
    );
}

#[test]
fn a_family_choice_keeps_the_declared_face_checks() {
    // Either arm may be used, so each declared arm must have the face.
    refusal(
        &format!("{DECLARATION}font \"Regular Only\" = \"assets/DejaVuSans.ttf\"\ncomponent App\n  state on = false\n  view\n    text \"x\" font-family=(on ? \"Fixture Sans\" : \"Regular Only\") font-weight=700\n"),
        "lower-font-face",
    );
    refusal(
        &format!("{DECLARATION}component App\n  state on = false\n  view\n    text \"x\" font-family=(on ? \"Fixture Sans\" : \"serif\") font-style=\"italic\"\n"),
        "lower-font-face",
    );
    refusal(
        &format!("{DECLARATION}component App\n  view\n    text \"x\" font-family=\"fixture sans\" font-style=\"italic\"\n"),
        "lower-font-face",
    );
    AppDir::new(&format!(
        "{DECLARATION}component App\n  state on = false\n  view\n    text \"x\" font-family=(on ? \"Fixture Sans\" : \"serif\") font-weight=700\n"
    ))
    .compile()
    .unwrap();
}

#[test]
fn unsupported_css_generics_are_diagnosed_and_quoted_names_stay_local() {
    for name in ["cursive", "fantasy", "math", "emoji", "fangsong"] {
        let error = contract::compile(&format!(
            "component App\n  view\n    text \"x\" font-family=\"{name}, sans-serif\"\n"
        ))
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("CSS generic family") && error.contains("no native mapping"),
            "{error}"
        );
        contract::compile(&format!(
            "component App\n  view\n    text \"x\" font-family=\"'{name}', sans-serif\"\n"
        ))
        .unwrap();
    }
}

#[test]
fn css_quoted_commas_do_not_alias_lists_and_family_matching_ignores_case() {
    let plan = contract::compile("component App\n  view\n    column\n      text \"a\" font-family=\"'Comma, Family'\"\n      text \"b\" font-family=\"Comma, Family\"\n      text \"c\" font-family=\"Inter\"\n      text \"d\" font-family=\"inter\"\n").unwrap();
    assert_eq!(
        plan.families
            .iter()
            .filter(|f| plan.str(f.name) == "Inter")
            .count(),
        1
    );
    assert!(plan.stacks.iter().any(|s| s.members.len == 2
        && s.members
            .iter()
            .all(|id| plan.stack_member(id).kind == exact_plan::StackMemberKind::Family)));
    assert!(plan
        .families
        .iter()
        .any(|f| plan.str(f.name) == "Comma, Family"));
}

#[test]
fn font_variant_ligatures_reaches_text_and_fields_and_refuses_a_pair() {
    let source = "component App\n  state off = true\n  view\n    column\n      text \"a://b\" testId=\"text\" font-variant-ligatures=(off ? \"none\" : \"normal\")\n      input testId=\"field\" value=\"a://b\" font-variant-ligatures=\"no-contextual\"\n";
    let plan = contract::compile(source).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let style = |id: &str| {
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .style
            .font_variant_ligatures
    };
    assert_eq!(
        style("text"),
        exact_kernel::FontVariantLigatures::bits("none").unwrap()
    );
    assert_eq!(
        style("field"),
        exact_kernel::FontVariantLigatures::NoContextual.keyword_bits()
    );
    let error = contract::compile(
        "component App\n  view\n    text \"1\" font-variant-ligatures=\"contextual no-contextual\"\n",
    )
    .unwrap_err();
    assert_eq!(error.id, "lower-attr-value");
    assert!(
        error.message.contains("at most one of each pair"),
        "{error}"
    );
}
