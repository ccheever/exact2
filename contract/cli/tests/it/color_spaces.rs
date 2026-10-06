//! LLP 1100: `dynamic-range-limit` and `color-profile`.

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

/// LLP 1100 D8.
#[test]
fn dynamic_range_limit_is_inherited_and_initially_no_limit() {
    let src = "component App\n  view\n    column\n      column testId=\"outer\" dynamic-range-limit=\"constrained\"\n        image \"a.png\" testId=\"inner\"\n      image \"b.png\" testId=\"plain\"\n";
    let r = boot(src);
    let k = r.kernel();
    let limit = exact_kernel::StyleId::DynamicRangeLimit;
    for (id, want) in [
        ("outer", "constrained"),
        ("inner", "constrained"),
        ("plain", "no-limit"),
    ] {
        let got = format!(
            "{:?}",
            k.node_by_key(k.find_by_test_id(id)[0])
                .unwrap()
                .computed(limit)
        );
        assert!(got.contains(want), "{id}: {got}");
    }
}

/// LLP 1100 D3.
#[test]
fn a_color_profile_is_declared_and_its_colours_used() {
    let ok = "color-profile --press src=\"assets/press.icc\" rendering-intent=\"perceptual\"\ncomponent App\n  view\n    column\n      box width=4 height=4 background-color=\"color(--press 0.1 0.8 0.2 0.05)\"\n";
    let plan = contract::compile(ok).unwrap();
    assert_eq!(plan.profiles.len(), 1);
    assert_eq!(plan.str(plan.profiles[0].src), "assets/press.icc");
    assert_eq!(plan.str(plan.profiles[0].intent), "perceptual");
    for (src, says) in [
        ("color-profile --p rendering-intent=\"perceptual\"\ncomponent App\n  view\n    column\n", "needs `src`"),
        ("color-profile --p src=\"a.icc\" gamut=\"wide\"\ncomponent App\n  view\n    column\n", "`gamut` is not a descriptor"),
        ("color-profile --p src=\"a.icc\" rendering-intent=\"loud\"\ncomponent App\n  view\n    column\n", "rendering-intent"),
        ("color-profile brand src=\"a.icc\"\ncomponent App\n  view\n    column\n", "dashed ident"),
    ] {
        let e = contract::compile(src).unwrap_err();
        assert!(e.message.contains(says), "{src}: {e}");
    }
}
