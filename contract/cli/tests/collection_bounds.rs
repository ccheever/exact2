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
fn refuses_nested(source: &str) {
    let error = contract::compile(source).unwrap_err();
    assert_eq!(error.id, "lower-collection-nested", "{error}");
    assert!(error.message.contains("ancestor"), "{error}");
}
#[test]
fn nested_virtual_templates_are_rejected_through_branches_and_eager_rows() {
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
        refuses_nested(&nested_source(&row));
    }
}
#[test]
fn expanded_components_cannot_hide_nested_virtual_collections() {
    let s = format!("{}\ncomponent Inner\n  props\n    items: list<number>\n  view\n    column\n      list virtualized=true height=80\n        each y in items key=y\n          text `${{y}}`\n", nested_source("          Inner(items=rows)"));
    refuses_nested(&s);
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

#[test]
fn virtual_container_vertical_padding_requires_literal_zero() {
    for name in ["padding", "padding-top", "padding-bottom"] {
        for value in ["16", "64", "grow"] {
            let s = source(&format!("virtualized=true height=200 {name}={value}"));
            let error = contract::compile(&s).unwrap_err();
            assert_eq!(error.id, "lower-collection-flow", "{name}={value}: {error}");
            assert!(error.message.contains("inside measured rows"), "{error}");
            assert!(error.message.contains("inset"), "{error}");
        }
        let styled = format!(
            "style Insets\n  {name}=64\n{}",
            source("virtualized=true height=200 class=Insets")
        );
        assert_eq!(
            contract::compile(&styled).unwrap_err().id,
            "lower-collection-flow"
        );
        contract::compile(&styled.replace("class=Insets", &format!("class=Insets {name}=0")))
            .unwrap();
    }
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
