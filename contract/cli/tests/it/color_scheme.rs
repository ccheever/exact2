//! LLP 1034 §8: `color-scheme` on a subtree.

use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn boot(src: &str) -> Runner<NoData> {
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

/// Inherited as CSS's is, a nested `light` winning below it; initially
/// `normal`, the surrounding scheme.
#[test]
fn color_scheme_is_inherited_and_initially_normal() {
    let src = "component App\n  view\n    column\n      column testId=\"sheet\" color-scheme=\"dark\"\n        text \"a\" testId=\"inner\"\n        column color-scheme=\"light\"\n          text \"b\" testId=\"card\"\n      text \"c\" testId=\"plain\"\n";
    let r = boot(src);
    let k = r.kernel();
    let row = exact_kernel::StyleId::ColorScheme;
    for (id, want) in [
        ("sheet", "dark"),
        ("inner", "dark"),
        ("card", "light"),
        ("plain", "normal"),
    ] {
        let got = format!(
            "{:?}",
            k.node_by_key(k.find_by_test_id(id)[0])
                .unwrap()
                .computed(row)
        );
        assert!(got.to_lowercase().contains(want), "{id}: {got}");
    }
}

/// Only `light` and `dark` are written: CSS's `normal` means the page's
/// schemes, not the parent's, and `light dark` and `only` ask a browser to
/// choose.
#[test]
fn only_light_and_dark_are_written() {
    for value in ["normal", "light dark", "only dark", "dim"] {
        let src = format!("component App\n  view\n    column color-scheme=\"{value}\"\n");
        let error = contract::compile(&src).unwrap_err().to_string();
        assert!(error.contains("LLP 1034 §8"), "{value}: {error}");
    }
    for value in ["light", "dark"] {
        let src = format!("component App\n  view\n    column color-scheme=\"{value}\"\n");
        contract::compile(&src).unwrap();
    }
}

/// A bound `normal` unsets the row, so the subtree follows the surrounding
/// scheme (CSS's `normal` would mean the page's).
#[test]
fn a_bound_normal_follows_the_surrounding_scheme() {
    let src = "component App\n  state s = \"normal\"\n  view\n    column color-scheme=\"dark\"\n      column testId=\"bound\" color-scheme=s\n        text \"a\" testId=\"inner\"\n";
    let r = boot(src);
    let k = r.kernel();
    for id in ["bound", "inner"] {
        let node = k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
        assert_eq!(node.color_scheme_dark(), Some(true), "{id}");
    }
}

/// An inline run and an SVG element take their scheme from above.
#[test]
fn a_run_and_an_svg_element_refuse_it() {
    let run = "component App\n  view\n    text \"a \"\n      text \"b\" color-scheme=\"dark\"\n";
    assert!(contract::compile(run)
        .unwrap_err()
        .to_string()
        .contains("inline `text` run"));
    let shape = "component App\n  view\n    svg width=10 height=10 viewBox=\"0 0 10 10\"\n      g color-scheme=\"dark\"\n        rect width=5 height=5\n";
    assert!(contract::compile(shape).is_err());
    let root = "component App\n  view\n    svg color-scheme=\"dark\" width=10 height=10 viewBox=\"0 0 10 10\"\n      rect width=5 height=5 fill=\"light-dark(#fff, #000)\"\n";
    contract::compile(root).unwrap();
}
