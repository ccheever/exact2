//! @ref LLP 1121 D3, D7 (stage 6 B) — length and number tokens: a row names
//! one as it names a colour, the kernel reads the node's slice as the row's
//! value (a length through the relative side table), a row of another kind
//! is refused, and an edit sets only the rows that read it, laying out for
//! a length and not for a number in a paint row.

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
    let dir = std::env::temp_dir().join(format!("exact2-measures-{name}-{}", std::process::id()));
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

/// A row's computed value, as the kernel prints it.
fn row(r: &Runner<NoData>, id: &str, style: StyleId) -> String {
    let key = r.kernel().find_by_test_id(id)[0];
    format!("{:?}", r.kernel().node_by_key(key).unwrap().computed(style))
}

const DESIGN: &str = "tamber t\n\naxes\n  density = regular | compact\n\ntheme\n  space.row = 12px\n    when density=compact: 8px\n  radius.card = 6px\n  size.title = 1.25rem\n  opacity.faded: number = 0.5\n    when density=compact: 0.25\n  weight.strong = 600\n  line.tight: number = 1.2\n    when density=compact: 1.1\n  size.style = -exact-title1\n  ink = #111111\n";

#[test]
fn a_length_and_a_number_token_take_the_nodes_slice() {
    let path = app(
        "rows",
        &[
            ("app.tamber", DESIGN),
            ("app.contract", "component App\n  view\n    column testId=\"card\" padding=\"space.row\" border-radius=\"radius.card\" opacity=\"opacity.faded\"\n      text \"a\" testId=\"title\" font-size=\"size.title\" font-weight=\"weight.strong\"\n      column testId=\"compact\" -exact-mode=\"density=compact\" padding=\"space.row\" opacity=\"opacity.faded\"\n        text \"b\"\n"),
        ],
    );
    let plan = contract::compile_path(&path).unwrap();
    // The table keeps each kind (LLP 1121 D7).
    let kinds: Vec<(String, u8)> = plan
        .theme
        .iter()
        .map(|t| (plan.str(t.name).to_string(), t.kind))
        .collect();
    assert!(kinds.contains(&("space.row".into(), 1)), "{kinds:?}");
    assert!(kinds.contains(&("opacity.faded".into(), 2)), "{kinds:?}");
    let mut r = booted(plan);
    assert!(
        row(&r, "card", StyleId::PaddingTop).contains("12.0"),
        "{}",
        row(&r, "card", StyleId::PaddingTop)
    );
    assert!(row(&r, "card", StyleId::BorderRadiusTopLeft).contains("6.0"));
    assert!(
        row(&r, "card", StyleId::Opacity).contains("0.5"),
        "{}",
        row(&r, "card", StyleId::Opacity)
    );
    // A `rem` token is the relative side table's entry: 1.25 × 16.
    assert!(
        row(&r, "title", StyleId::FontSize).contains("20.0"),
        "{}",
        row(&r, "title", StyleId::FontSize)
    );
    assert!(
        row(&r, "title", StyleId::FontWeight).contains("600"),
        "{}",
        row(&r, "title", StyleId::FontWeight)
    );
    // A pin's slice.
    assert!(
        row(&r, "compact", StyleId::PaddingTop).contains("8.0"),
        "{}",
        row(&r, "compact", StyleId::PaddingTop)
    );
    assert!(row(&r, "compact", StyleId::Opacity).contains("0.25"));
    // The window's mode: every row that read a token takes its slice.
    let receipt = r.set_mode("density", "compact").unwrap().unwrap();
    assert!(receipt.layout_invalidated, "a length moves geometry");
    assert!(row(&r, "card", StyleId::PaddingTop).contains("8.0"));
    assert!(row(&r, "card", StyleId::Opacity).contains("0.25"));
}

#[test]
fn a_token_in_a_row_of_another_kind_is_refused_naming_the_row() {
    let refused = |name: &str, view: &str| {
        let path = app(name, &[("app.tamber", DESIGN), ("app.contract", view)]);
        contract::compile_path(&path).unwrap_err()
    };
    let e = refused(
        "number-in-length",
        "component App\n  view\n    column padding=\"opacity.faded\"\n      text \"a\"\n",
    );
    assert_eq!(e.id, "tamber-kind", "{e:?}");
    assert!(e.message.contains("`padding`"), "{}", e.message);
    let e = refused(
        "length-in-number",
        "component App\n  view\n    column opacity=\"space.row\"\n      text \"a\"\n",
    );
    assert_eq!(e.id, "tamber-kind", "{e:?}");
    let e = refused(
        "length-in-colour",
        "component App\n  view\n    column background-color=\"space.row\"\n      text \"a\"\n",
    );
    assert_eq!(e.id, "tamber-kind", "{e:?}");
    assert!(e.message.contains("takes a colour"), "{}", e.message);
    let e = refused(
        "colour-in-length",
        "component App\n  view\n    column padding=\"ink\"\n      text \"a\"\n",
    );
    assert_eq!(e.id, "tamber-kind", "{e:?}");
    // A text style is a font size only (the kernel's relative side table).
    let e = refused(
        "style-in-padding",
        "component App\n  view\n    column padding=\"size.style\"\n      text \"a\"\n",
    );
    assert_eq!(e.id, "tamber-kind", "{e:?}");
    assert!(e.message.contains("-exact-title1"), "{}", e.message);
    // Only a colour is chosen again at a mode boundary (D7).
    let e = refused(
        "inherited-axis",
        "component App\n  view\n    column line-height=\"line.tight\"\n      text \"a\"\n",
    );
    assert_eq!(e.id, "tamber-inherited-axis", "{e:?}");
}

/// LLP 1121 stage 6 B: a live length edit lays out and keeps state; a live
/// number edit in a paint row does not lay out. On a 1,000-row list.
#[test]
fn a_live_length_edit_lays_out_and_a_number_edit_in_a_paint_row_does_not() {
    let rows: Vec<String> = (0..1000).map(|i| i.to_string()).collect();
    let view = format!("component App\n  state count = 0\n  state rows: number[] = [{}]\n  action bump\n    count = count + 1\n  view\n    column\n      button press=bump testId=\"bump\"\n        text `${{count}}` testId=\"count\"\n      for i in rows key=i\n        row testId=`row-${{i}}` padding=\"space.row\" opacity=\"opacity.faded\"\n          text `${{i}}`\n", rows.join(", "));
    let path = app("live", &[("app.tamber", DESIGN), ("app.contract", &view)]);
    let mut r = booted(contract::compile_path(&path).unwrap());
    let bump = {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id("bump")[0]).unwrap().id
    };
    r.dispatch(bump, exact_runner::Event::Press).unwrap();
    let started = std::time::Instant::now();
    let (receipt, set) = r.set_token("space.row", "20px", &["20px", "16px"]).unwrap();
    let length = started.elapsed();
    // `padding` writes four rows a node.
    assert_eq!(set, 4000, "only the rows that read `space.row`");
    assert!(receipt.layout_invalidated, "a length lays out");
    assert!(row(&r, "row-999", StyleId::PaddingLeft).contains("20.0"));
    let started = std::time::Instant::now();
    let (receipt, set) = r
        .set_token("opacity.faded", "0.75", &["0.75", "0.5"])
        .unwrap();
    let number = started.elapsed();
    assert_eq!(set, 1000);
    assert!(!receipt.layout_invalidated, "an opacity paints");
    assert!(row(&r, "row-7", StyleId::Opacity).contains("0.75"));
    // State stands: the edit rebuilt nothing.
    let count = r.kernel().find_by_test_id("count")[0];
    let text = format!("{:?}", r.kernel().node_by_key(count).unwrap());
    assert!(text.contains('1'), "{text}");
    eprintln!("length edit {length:?}, number edit {number:?} (1000 rows)");
}
