//! @ref LLP 1121 D3, D6 (stage 6 C) — type and family tokens: `font` names
//! a type token and writes only the longhands it names, so a text style's
//! own weight applies where it writes none; `font-family` names a family
//! token; a token that varies by an app axis is the text's own, refused on
//! a node whose children would inherit it; Bold Text weights a system
//! family under a type token and leaves a named one alone.

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

fn app(name: &str, view: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-type-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.tamber"), DESIGN).unwrap();
    std::fs::write(dir.join("app.contract"), view).unwrap();
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

fn node<'a>(r: &'a Runner<NoData>, id: &str) -> exact_kernel::NodeRef<'a> {
    let key = r.kernel().find_by_test_id(id)[0];
    r.kernel().node_by_key(key).unwrap()
}

const DESIGN: &str = "tamber t\n\naxes\n  density = regular | roomy\n\ntheme\n  family.mono = ui-monospace, monospace\n  type.body = -exact-body/1.35 \"InterVariable\"\n  type.title = 700 -exact-title1 Georgia, serif\n  type.system = 1.25rem/1.4 system-ui, sans-serif\n  type.dense = -exact-footnote/1.2\n    when density=roomy: -exact-body/1.5\n  type.mixed: type = -exact-footnote\n    when density=roomy: -exact-body/1.5\n  face = \"InterVariable\", system-ui\n  family.quoted: family = \"monospace\"\n  family.apple: family = -apple-system\n  family.vary: family = Georgia\n    when density=roomy: serif\n  type.upper = -EXACT-BODY/1.4\n  ink2 = #111111\n  type.emphasis = italic 700 1rem\n";

const VIEW: &str = "component App\n  view\n    column\n      text \"Body\" testId=\"body\" font=\"type.body\"\n      text \"Title\" testId=\"title\" font=\"type.title\"\n      text \"System\" testId=\"system\" font=\"type.system\"\n      text \"Mono\" testId=\"mono\" font-family=\"family.mono\"\n      text \"Dense\" testId=\"dense\" font=\"type.dense\"\n      column -exact-mode=\"density=roomy\"\n        text \"Roomy\" testId=\"roomy\" font=\"type.dense\"\n";

#[test]
fn a_type_token_writes_only_its_longhands() {
    let plan = contract::compile_path(&app("rows", VIEW)).unwrap_or_else(|e| panic!("{e:?}"));
    let kinds: Vec<(String, u8)> = plan
        .theme
        .iter()
        .map(|t| (plan.str(t.name).to_string(), t.kind))
        .collect();
    assert!(kinds.contains(&("type.body".into(), 5)), "{kinds:?}");
    assert!(kinds.contains(&("family.mono".into(), 6)), "{kinds:?}");
    let mut r = booted(plan);
    let body = node(&r, "body");
    // The size, the leading and the family, written; the weight and the
    // style unsaid: the text style's weight applies (D12.2).
    assert!(body.style.mask.has(StyleId::FontSize));
    assert!(body.style.mask.has(StyleId::LineHeight));
    assert!(body.style.mask.has(StyleId::FontFamily));
    assert!(!body.style.mask.has(StyleId::FontWeight));
    assert!(!body.style.mask.has(StyleId::FontStyle));
    assert!(body.style.written.has(StyleId::LineHeight), "written rows");
    assert_eq!(
        format!("{:?}", body.computed(StyleId::LineHeight)),
        "LineHeight(Number(1.35))"
    );
    assert_ne!(
        body.style.font_family, 0,
        "the plan's stack for InterVariable"
    );
    let source = exact_runner::agent::node(&r, body.id);
    assert!(
        source.contains(&format!(
            "\"font_weight\":{{\"value\":400,\"source\":\"style\",\"from\":{}}}",
            body.id
        )),
        "{source}"
    );
    let title = node(&r, "title");
    assert_eq!(title.style.font_weight, 700);
    assert!(title.style.mask.has(StyleId::FontWeight));
    let mono = node(&r, "mono").style.font_family;
    assert!(
        mono != 0 && mono != body.style.font_family,
        "a family token's own stack"
    );
    // A token that varies: each text reads its own tuple's.
    assert_eq!(
        format!("{:?}", node(&r, "dense").computed(StyleId::LineHeight)),
        "LineHeight(Number(1.2))"
    );
    assert_eq!(
        format!("{:?}", node(&r, "roomy").computed(StyleId::LineHeight)),
        "LineHeight(Number(1.5))"
    );
    // Bold Text: a system family under a type token steps, an authored
    // weight included; a named family is left alone (D12.3).
    let weight = |r: &Runner<NoData>, id: &str| node(r, id).text_style().font_weight;
    let (system, title) = (weight(&r, "system"), weight(&r, "title"));
    r.set_bold_text(true).unwrap();
    assert_eq!(weight(&r, "system"), exact_kernel::bolder(system));
    assert_eq!(weight(&r, "title"), title, "Georgia is the app's");
    // A value-only edit (the same longhands, the same family) is live: the
    // rows that name the token read it again.
    let (receipt, set) = r
        .set_token("type.body", "-exact-headline/1.5 \"InterVariable\"", &[])
        .unwrap();
    assert_eq!(set, 3, "size, leading and family of the one text");
    assert!(receipt.layout_invalidated);
    let body = node(&r, "body");
    assert_eq!(
        format!("{:?}", body.computed(StyleId::LineHeight)),
        "LineHeight(Number(1.5))"
    );
}

/// A family token resolves to the stack the compiler declared for it, by
/// its members as the compiler reads them: an undotted name too (never an
/// installed family named for the token), a quoted generic as a family,
/// `-apple-system` as `system-ui`. A name computed at run time that varies
/// is refused on a node whose children would inherit it.
#[test]
fn a_family_token_is_the_stack_the_compiler_declared() {
    let view = "component App\n  state chosen = \"family.vary\"\n  state choice = \"face\"\n  action flip\n    choice = \"Georgia\"\n  view\n    column\n      button testId=\"flip\" press=flip\n        text \"Flip\"\n      text \"Switch\" testId=\"switch\" color=\"ink2\" font-family=choice\n      text \"Face\" testId=\"face\" font-family=\"face\"\n      text \"Quoted\" testId=\"quoted\" font-family=\"family.quoted\"\n      text \"Apple\" testId=\"apple\" font-family=\"family.apple\"\n      text \"Upper\" testId=\"upper\" font=\"type.upper\"\n      column testId=\"container\" font-family=chosen\n        text \"Inner\"\n      column testId=\"choice\" font-family=chosen\n        if true\n          text \"In a choice\"\n";
    let plan = contract::compile_path(&app("stacks", view)).unwrap_or_else(|e| panic!("{e:?}"));
    let stacks = plan.stacks.len() as u16;
    let mut r = booted(plan);
    let face = node(&r, "face").style.font_family;
    assert!(
        face >= 8 && face < stacks,
        "the plan's stack, not a runtime one: {face} of {stacks}"
    );
    let quoted = node(&r, "quoted").style.font_family;
    assert!(
        quoted >= 8 && quoted < stacks,
        "a quoted generic is a family: {quoted}"
    );
    assert_eq!(node(&r, "apple").style.font_family, 0, "system-ui");
    assert!(
        node(&r, "upper").style.mask.has(StyleId::FontSize),
        "a text style in capitals"
    );
    assert!(
        !node(&r, "container").style.mask.has(StyleId::FontFamily),
        "a varying token computed onto a container is refused"
    );
    assert!(
        !node(&r, "choice").style.mask.has(StyleId::FontFamily),
        "and onto one whose child is a choice's"
    );
    // A row that named a token and now names a runtime family forgets the
    // token: an edit to it (or to a token whose refresh bit it shares) sets
    // nothing back.
    assert_eq!(node(&r, "switch").style.font_family, face);
    let flip = node(&r, "flip").id;
    r.dispatch(flip, exact_runner::Event::Press).unwrap();
    let georgia = node(&r, "switch").style.font_family;
    assert!(georgia >= stacks, "a runtime family: {georgia}");
    r.set_token("face", "\"InterVariable\", system-ui", &[])
        .unwrap();
    assert_eq!(node(&r, "switch").style.font_family, georgia);
}

#[test]
fn a_type_token_is_refused_where_its_rows_would_not_hold() {
    let refused = |name: &str, view: &str| contract::compile_path(&app(name, view)).unwrap_err();
    let e = refused(
        "inherited",
        "component App\n  view\n    column font=\"type.dense\"\n      text \"a\"\n",
    );
    assert_eq!(e.id, "tamber-inherited-axis", "{e:?}");
    assert!(e.message.contains("write it on the text"), "{}", e.message);
    let e = refused(
        "font-size",
        "component App\n  view\n    text \"a\" font-size=\"type.body\"\n",
    );
    assert_eq!(e.id, "tamber-kind", "{e:?}");
    assert!(e.message.contains("`font`"), "{}", e.message);
    let e = refused(
        "longhands",
        "component App\n  view\n    text \"a\" font=\"type.mixed\"\n",
    );
    assert_eq!(e.id, "tamber-type-longhands", "{e:?}");
    let e = refused(
        "family-as-type",
        "component App\n  view\n    text \"a\" font=\"family.mono\"\n",
    );
    assert!(
        e.id == "tamber-kind" || e.id == "lower-css-shorthand",
        "{e:?}"
    );
}
