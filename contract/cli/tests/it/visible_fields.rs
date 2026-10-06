//! LLP 1104: a bare text field is a visible one — a border, padding and a
//! fill written as a sheet under the author's rows — and a literal
//! `appearance="none"` is the bare box.

use exact_kernel::{Kernel, Offer};
use exact_runner::{DataError, DataSource, Event, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// A component whose view is `body` under a 402-wide column, with `styles`
/// declared ahead of it.
fn app(styles: &str, body: &str) -> String {
    let body = body
        .lines()
        .map(|l| format!("      {l}\n"))
        .collect::<String>();
    format!(
        "{styles}component App\n  state on = true\n  action flip\n    on = not on\n  view\n    column testId=\"root\" width=402\n{body}"
    )
}

fn boot(styles: &str, body: &str) -> Runner<NoData> {
    let plan = contract::bake(
        contract::compile(&app(styles, body)).unwrap_or_else(|e| panic!("{e}")),
        NoData,
    )
    .unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    layout(&mut r);
    r
}

fn layout(r: &mut Runner<NoData>) {
    let k = r.kernel_mut();
    let root = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
    k.compute_layout(root, Offer::definite(402.0, 874.0))
        .unwrap();
}

fn id(r: &Runner<NoData>, test_id: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// A node's border widths, frame height and whether it paints a fill.
fn look(r: &Runner<NoData>, test_id: &str) -> ([f32; 4], f32, bool) {
    let n = r.kernel().node(id(r, test_id)).unwrap();
    let filled = n
        .style
        .background_color
        .is_some_and(|c| c.resolve(false).a() > 0);
    (n.style.border_widths(), n.frame.height, filled)
}

#[test]
fn a_bare_field_is_visible_and_appearance_none_is_the_bare_box() {
    let r = boot(
        "",
        "input testId=\"field\"\ninput appearance=\"none\" testId=\"bare\"\ntextarea testId=\"area\"\ninput type=\"email\" testId=\"email\"\ninput type=(on ? \"password\" : \"text\") testId=\"secret\"",
    );
    let (border, height, filled) = look(&r, "field");
    let (bare_border, bare_height, bare_filled) = look(&r, "bare");
    assert_eq!(border, [1.0; 4], "a one-pixel border on every side");
    assert!(filled, "and a fill");
    assert_eq!(bare_border, [0.0; 4]);
    assert!(!bare_filled, "`appearance=\"none\"` paints nothing");
    assert_eq!(
        height - bare_height,
        14.0,
        "6 + 6 of padding and 1 + 1 of border, outside the line (content-box)"
    );
    for dressed in ["area", "email", "secret"] {
        assert_eq!(look(&r, dressed).0, [1.0; 4], "{dressed} is a text field");
    }
    let k = r.kernel();
    let ink = k.node(id(&r, "field")).unwrap().style.text_color;
    assert!(
        ink.resolve(false) != ink.resolve(true),
        "the ink follows the fill's scheme"
    );
}

#[test]
fn only_fields_one_types_into_are_dressed() {
    let r = boot(
        "",
        "input type=\"hidden\" testId=\"hidden\"\ninput type=\"color\" testId=\"color\"\ninput type=\"checkbox\" testId=\"check\"\ntextarea markup=\"markdown\" testId=\"editor\"\nbutton testId=\"button\"\n  text \"Go\"",
    );
    for bare in ["hidden", "color", "check", "editor", "button"] {
        let (border, _, filled) = look(&r, bare);
        assert_eq!(border, [0.0; 4], "{bare} takes no field border");
        assert!(!filled, "{bare} takes no field fill");
    }
}

#[test]
fn an_authored_row_or_class_replaces_one_row_and_keeps_the_rest() {
    let r = boot(
        "style Filled\n  background-color=\"#eeeeee\"\n",
        "input border=\"3px solid #ff0000\" testId=\"thick\"\ninput padding=0 testId=\"tight\"\ninput appearance=\"none\" testId=\"bare\"\ninput class=Filled testId=\"filled\"\ninput border-width=0 width=100 testId=\"wide\"",
    );
    assert_eq!(
        look(&r, "thick").0,
        [3.0; 4],
        "the shorthand replaces the sheet's border"
    );
    let (_, tight, _) = look(&r, "tight");
    let (_, bare, _) = look(&r, "bare");
    assert_eq!(tight - bare, 2.0, "`padding=0` leaves only the border");
    let (border, _, filled) = look(&r, "filled");
    assert_eq!(border, [1.0; 4], "a class's fill keeps the sheet's border");
    assert!(filled);
    let k = r.kernel();
    assert_eq!(
        k.node(id(&r, "wide")).unwrap().frame.width,
        116.0,
        "an authored width is the content's: 8 + 8 of padding outside it"
    );
}

#[test]
fn a_conditional_class_falls_back_to_the_sheet_not_the_kernel() {
    let mut r = boot(
        "style Roomy\n  padding=12\n  border=\"2px solid #ff0000\"\nstyle Plain\n  font-weight=600\n",
        "input class=(on ? Roomy : Plain) testId=\"field\"\ninput appearance=\"none\" testId=\"bare\"\nbutton press=flip testId=\"flip\"\n  text \"Flip\"",
    );
    let (_, bare, _) = look(&r, "bare");
    assert_eq!(look(&r, "field").1 - bare, 28.0, "`Roomy`: 12 + 12 + 2 + 2");
    let flip = id(&r, "flip");
    r.dispatch(flip, Event::Press).unwrap();
    layout(&mut r);
    assert_eq!(
        look(&r, "field").1 - bare,
        14.0,
        "`Plain` sets no padding: the sheet's 6 + 6, not the kernel's 0"
    );
    assert_eq!(look(&r, "field").0, [1.0; 4], "nor a border: the sheet's");
}

#[test]
fn a_fields_appearance_is_none_or_absent() {
    for body in [
        "input appearance=\"auto\"",
        "input appearance=(on ? \"none\" : \"auto\")",
    ] {
        let e = contract::compile(&app("", body)).unwrap_err();
        assert_eq!(e.id, "lower-field-appearance", "{}", e.message);
    }
}
