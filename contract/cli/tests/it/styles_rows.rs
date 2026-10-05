//! CSS rows beside the named styles (styles.rs): `order`, the CSS-wide
//! keywords and `currentcolor`, and `position-area`, proven on the kernel's
//! rows after boot. Split from styles.rs.

use exact_kernel::{Color, Kernel};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// CSS `order` is a box's style row, bound or literal: a flex item moves in
/// its row, as on the web (feed F19: it compiled as an SVG filter
/// primitive's attribute and moved nothing).
#[test]
fn order_moves_a_flex_item_and_a_bound_order_moves_it_again() {
    let src = "component A\n  state rail = true\n  action flip\n    rail = not rail\n  view\n    row testId=\"row\" width=300\n      view testId=\"main\" width=200 height=10\n      view testId=\"rail\" width=100 height=10 order=(rail ? -1 : 0)\n      view testId=\"last\" width=0 height=10 order=1\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let x = |r: &mut Runner<NoData>, id: &str| {
        let root = r.roots()[0];
        r.kernel_mut()
            .compute_layout(root, exact_kernel::Offer::definite(400.0, 400.0))
            .unwrap();
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0]).unwrap().frame.x
    };
    assert_eq!((x(&mut r, "rail"), x(&mut r, "main")), (0.0, 100.0));
    r.act("flip", vec![]).unwrap();
    assert_eq!((x(&mut r, "main"), x(&mut r, "rail")), (0.0, 200.0));
    assert_eq!(x(&mut r, "last"), 300.0);
}

/// CSS-wide keywords and `currentcolor` (feed F1): `inherit` on an
/// inherited row and `unset` leave the row unset, so the parent's colour
/// shows, bound or literal, over a class's row; `currentcolor` on a
/// background or a tint is the keyword, which each host paints in `color`.
#[test]
fn inherit_unset_and_currentcolor_are_csss() {
    let src = "style Loud\n  color=\"#ff0000\"\ncomponent A\n  state on = false\n  action flip\n    on = not on\n  view\n    column color=\"#0000ff\"\n      text \"a\" testId=\"inherit\" class=Loud color=\"inherit\"\n      text \"b\" testId=\"bound\" color=(on ? \"unset\" : \"#00ff00\")\n      view testId=\"swatch\" background-color=\"currentcolor\"\n      image \"symbol:add\" testId=\"icon\" tint-color=\"currentColor\" alt=\"Add\"\n      input testId=\"field\" value=\"\" enterkeyhint=\"send\"\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let node = |r: &Runner<NoData>, id: &str| {
        let k = r.kernel();
        let key = k.find_by_test_id(id)[0];
        let n = k.node_by_key(key).unwrap();
        (n.style.clone(), n.text_color())
    };
    let blue: exact_kernel::ColorValue = Color::parse_hex("#0000ff").unwrap().into();
    let (inherit, color) = node(&r, "inherit");
    assert!(!inherit.mask.has(exact_kernel::StyleId::TextColor));
    assert_eq!(color, blue, "the class's red is not the node's");
    assert_eq!(
        node(&r, "bound").1,
        Color::parse_hex("#00ff00").unwrap().into()
    );
    r.act("flip", vec![]).unwrap();
    assert_eq!(node(&r, "bound").1, blue);
    assert!(
        r.journal().all(|l| !l.contains("invalid")),
        "{:?}",
        r.journal().collect::<Vec<_>>()
    );
    let (swatch, _) = node(&r, "swatch");
    assert!(swatch.mask.has(exact_kernel::StyleId::BackgroundColor));
    assert_eq!(swatch.background_color, None);
    let (icon, _) = node(&r, "icon");
    assert_eq!(icon.tint_color, None);
    let k = r.kernel();
    let icon = k.node_by_key(k.find_by_test_id("icon")[0]).unwrap();
    assert_eq!(
        icon.props.str(exact_kernel::PropId::AccessibilityLabel),
        Some("Add")
    );
    let field = k.node_by_key(k.find_by_test_id("field")[0]).unwrap();
    assert_eq!(
        field.props.str(exact_kernel::PropId::EnterKeyHint),
        Some("send")
    );
}

#[test]
fn position_area_places_a_popover_in_the_admitted_subset_only() {
    let popover = |value: &str| {
        format!("component App\n  view\n    column\n      button \"Open\" popovertarget=\"p\"\n      column id=\"p\" popover=\"auto\" position-area=\"{value}\"\n        text \"x\"\n")
    };
    for value in [
        "none",
        "bottom",
        "bottom span-right",
        "bottom span-all",
        "top",
        "top span-right",
        "top span-all",
        "center",
    ] {
        contract::compile(&popover(value)).unwrap_or_else(|e| panic!("{value}: {e}"));
    }
    for value in [
        "left",
        "top left",
        "span-all top",
        "top span-left",
        "block-start",
        "nonsense",
    ] {
        let error = contract::compile(&popover(value)).unwrap_err().to_string();
        assert!(
            error.contains("subset of CSS `position-area`") && error.contains("top span-all"),
            "{value}: {error}"
        );
    }
    let error = contract::compile("component App\n  view\n    column position-area=\"top\"\n")
        .unwrap_err()
        .to_string();
    assert!(error.contains("on a `popover` only"), "{error}");
}
