//! Literal refusals and the measured fallback for virtual collection bounds.
use contract::BakeError;
use exact_plan::Value;
use exact_runner::{DataError, DataSource};

struct Rows;
impl DataSource for Rows {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::list(
            (0..64).map(|i| Value::Number(i as f64)).collect(),
        ))
    }
}
fn source(attrs: &str) -> String {
    format!("component App\n  state bound = \"auto\"\n  state grow = 0\n  resource rows = rows() as shape list<number>\n  view\n    row\n      list {attrs} testId=\"collection\"\n        each x in rows key=x\n          text `${{x}}` height=20\n")
}
#[test]
fn literal_auto_and_zero_flex_are_not_vertical_bounds() {
    for attrs in [
        "height=\"auto\"",
        "max-height=\"auto\"",
        "flex=0",
        "height=\"auto\" max-height=\"auto\" flex=0",
    ] {
        let error = contract::compile(&source(&format!("virtualized=true {attrs}"))).unwrap_err();
        assert_eq!(error.id, "lower-collection-unbounded", "{attrs}: {error}");
    }
    for attrs in [
        "height=0",
        "max-height=0",
        "height=200",
        "max-height=200",
        "flex=1",
        "height=\"auto\" max-height=200",
        "height=200 flex=0",
        "height=bound",
        "flex=grow",
    ] {
        contract::compile(&source(&format!("virtualized=true {attrs}"))).unwrap();
    }
}
#[test]
fn expanded_style_bounds_follow_the_same_rule_and_inline_override_wins() {
    let styled = format!(
        "style Unbounded\n  height=\"auto\"\n{}",
        source("virtualized=true class=Unbounded")
    );
    assert_eq!(
        contract::compile(&styled).unwrap_err().id,
        "lower-collection-unbounded"
    );
    contract::compile(&styled.replace("class=Unbounded", "class=Unbounded max-height=200"))
        .unwrap();
}
#[test]
fn bake_checks_computed_auto_height_and_zero_flex_on_virtual_lists() {
    for attrs in ["height=bound", "flex=grow"] {
        let plan = contract::compile(&source(&format!("virtualized=true {attrs}"))).unwrap();
        match contract::bake(plan, Rows).unwrap_err() {
            BakeError::Lint { id, message, .. } => {
                assert_eq!(id, "bake-scroll-unbounded");
                assert!(message.contains("testId=\"collection\""), "{message}");
            }
            other => panic!("{other:?}"),
        }
    }
    let bounded = source("virtualized=true height=bound")
        .replace("state bound = \"auto\"", "state bound = 200");
    contract::bake(contract::compile(&bounded).unwrap(), Rows).unwrap();
}
#[test]
fn eager_lists_and_actual_parent_stretch_remain_valid() {
    contract::bake(
        contract::compile(&source("virtualized=false height=\"auto\"")).unwrap(),
        Rows,
    )
    .unwrap();
    let stretched =
        source("virtualized=true height=bound").replace("    row\n", "    row height=200\n");
    contract::bake(contract::compile(&stretched).unwrap(), Rows).unwrap();
}

fn nested_source(row: &str) -> String {
    source("virtualized=true height=200").replace("          text `${x}` height=20", row)
}
/// One level of nesting compiles wherever the row puts it (LLP 1070 N6);
/// `collection_nest.rs` refuses a second.
fn nests(source: &str) {
    contract::compile(source).unwrap_or_else(|e| panic!("{e}"));
}
#[test]
fn nested_virtual_templates_compile_through_branches_and_eager_rows() {
    let inner =
        "list virtualized=true height=80\n{indent}  each y in rows key=y\n{indent}    text `${y}`";
    for prefix in [
        "",
        "column\n{indent}  when false\n{indent}    ",
        "list virtualized=false\n{indent}  each z in rows key=z\n{indent}    column\n{indent}      ",
        "column\n{indent}  match some(x)\n{indent}    case some(n)\n{indent}      ",
    ] {
        let indent = "          ";
        let start = format!("{indent}{}", prefix.replace("{indent}", indent));
        let depth = start.rsplit('\n').next().unwrap().len();
        let child = inner.replace("{indent}", &" ".repeat(depth));
        let mut row = format!("{start}{child}");
        if prefix.contains("match") {
            row.push_str("\n              case none\n                text \"empty\"");
        }
        nests(&nested_source(&row));
    }
}
#[test]
fn expanded_components_can_hold_a_nested_virtual_collection() {
    let s = format!("{}\ncomponent Inner\n  props\n    items: list<number>\n  view\n    column\n      list virtualized=true height=80\n        each y in items key=y\n          text `${{y}}`\n", nested_source("          Inner(items=rows)"));
    nests(&s);
}
#[test]
fn ordinary_sheet_and_eager_outer_list_can_contain_virtual_collections() {
    let s = nested_source("          column\n            list virtualized=true height=80\n              each y in rows key=y\n                text `${y}`");
    contract::compile(&s.replacen(
        "virtualized=true height=200",
        "virtualized=false height=200",
        1,
    ))
    .unwrap();
    let sheet =
        source("virtualized=true height=200").replace("    row\n", "    column height=400\n");
    contract::compile(&sheet).unwrap();
}

#[test]
fn disabled_lists_and_ordinary_scrolls_remain_valid_inside_virtual_rows() {
    for inner in ["scroll", "list virtualized=false"] {
        let s = nested_source(&format!("          {inner} height=80\n            each y in rows key=y\n              text `${{y}}`"));
        contract::compile(&s).unwrap();
    }
}

/// Main-axis padding is CSS's room before the first row and after the last
/// (LLP 1010 §6.9): a number, an `env()` inset or its `calc()`, computed as
/// a number too; a percentage, or a computed string that could be one, is
/// refused.
#[test]
fn virtual_container_vertical_padding_takes_lengths_not_percentages() {
    for name in ["padding", "padding-top", "padding-bottom"] {
        for value in [
            "16",
            "grow",
            "\"env(safe-area-inset-top)\"",
            "\"calc(env(safe-area-inset-bottom) + 49px)\"",
            "(grow > 0 ? 92 : \"env(safe-area-inset-top)\")",
        ] {
            let s = source(&format!("virtualized=true height=200 {name}={value}"));
            contract::compile(&s).unwrap_or_else(|e| panic!("{name}={value}: {e}"));
        }
        for value in [
            "\"10%\"",
            "\"calc(10% + 8px)\"",
            "bound",
            "(grow > 0 ? 8 : \"5%\")",
        ] {
            let s = source(&format!("virtualized=true height=200 {name}={value}"));
            let error = contract::compile(&s).unwrap_err();
            assert_eq!(error.id, "lower-collection-flow", "{name}={value}: {error}");
            assert!(error.message.contains("percentage"), "{error}");
        }
        let styled = format!(
            "style Insets\n  {name}=64\n{}",
            source("virtualized=true height=200 class=Insets")
        );
        contract::compile(&styled).unwrap();
        let e = contract::compile(&styled.replace("=64", "=\"10%\"")).unwrap_err();
        assert_eq!(e.id, "lower-collection-flow", "{name} in a class: {e}");
    }
    // The shorthand's main-axis sides are what count: `0 0 10%`'s bottom.
    contract::compile(&source(
        "virtualized=true height=200 padding=\"92 4% 49 4%\"",
    ))
    .unwrap();
    let e =
        contract::compile(&source("virtualized=true height=200 padding=\"0 0 10%\"")).unwrap_err();
    assert_eq!(e.id, "lower-collection-flow", "{e}");
}
#[test]
fn horizontal_container_padding_and_measured_row_spacing_still_compile() {
    for attrs in [
        "padding=0 padding-top=0 padding-bottom=0",
        "padding-left=16 padding-right=64",
        "padding-left=grow padding-right=grow",
    ] {
        contract::compile(&source(&format!("virtualized=true height=200 {attrs}"))).unwrap();
    }
    for name in ["padding", "padding-top", "padding-bottom"] {
        let row = format!("          column {name}=grow\n            text `${{x}}`");
        contract::compile(&nested_source(&row)).unwrap();
        contract::compile(&source(&format!("virtualized=false height=200 {name}=64"))).unwrap();
    }
}

/// The `flex` shorthand's text is a bound as its longhands are (Grok's batch
/// 2 reviews): a positive grow, or a definite length basis (zero included);
/// `none`, `initial`, `"0"` and a zero grow over `auto` or `0%` are not, on a virtualized
/// list and on a `scroll` alike. A scroller's shrink comes from the
/// shorthand too, so `flex="none"` does not shrink under a bounded column.
#[test]
fn the_flex_shorthand_text_bounds_as_its_longhands_do() {
    let scroll = |parent: &str, attrs: &str| {
        format!(
            "component App\n  view\n    {parent}\n      scroll {attrs}\n        box height=1000\n"
        )
    };
    for flex in ["none", "0", "initial", "0 1 auto", "0 0 0%", "0 0 content"] {
        let e =
            contract::compile(&source(&format!("virtualized=true flex=\"{flex}\""))).unwrap_err();
        assert_eq!(e.id, "lower-collection-unbounded", "{flex}: {e}");
        let e = contract::compile(&scroll("column", &format!("flex=\"{flex}\""))).unwrap_err();
        assert_eq!(e.id, "lower-scroll-unbounded", "{flex}: {e}");
    }
    // A zero length basis is a definite, empty scrollport, as `height=0` is.
    for flex in [
        "1",
        "auto",
        "2 1 0%",
        "0 0 200px",
        "0 1 50%",
        "0 0 0px",
        "0 0 0",
    ] {
        contract::compile(&source(&format!("virtualized=true flex=\"{flex}\""))).unwrap();
        contract::compile(&scroll("column", &format!("flex=\"{flex}\""))).unwrap();
    }
    // The scroll exception: a shrinking item with a zero minimum under a
    // bounded column, its shrink read from the shorthand.
    contract::compile(&scroll(
        "column height=200",
        "flex=\"0 1 auto\" min-height=0",
    ))
    .unwrap();
    let e =
        contract::compile(&scroll("column height=200", "flex=\"none\" min-height=0")).unwrap_err();
    assert_eq!(e.id, "lower-scroll-unbounded", "{e}");
}
