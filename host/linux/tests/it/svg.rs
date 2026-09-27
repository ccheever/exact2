//! LLP 1055.000 D17: a press inside an `svg` goes to the element under it,
//! by `pointer-events`, and bubbles as a box's press does.
use exact_linux::{presenter::PainterChoice, Presenter};
use exact_runner::{DataError, DataSource, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const APP: &str = "component App\n  state picked = \"none\"\n  action pick(which: string) writes picked\n    picked = which\n  view\n    column\n      text picked testId=\"picked\"\n      svg testId=\"chart\" width=100 height=100 viewBox=\"0 0 100 100\"\n        rect width=100 height=100 fill=\"#eeeeee\" press=pick(\"back\")\n        g press=pick(\"bars\")\n          rect x=10 y=10 width=20 height=80 fill=\"#2563eb\"\n          rect x=40 y=40 width=20 height=50 fill=\"#2563eb\" pointer-events=\"none\"\n        polyline points=\"70,90 90,10\" fill=\"none\" stroke=\"#000000\" stroke-width=6 press=pick(\"line\")\n";

#[test]
fn a_press_in_an_svg_reaches_the_element_under_it() {
    let plan = contract::compile(APP).unwrap_or_else(|e| panic!("{e}"));
    let (mut presenter, error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (400., 400.),
        1.,
        std::env::temp_dir(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let kernel = presenter.host().kernel();
    let chart = kernel
        .node_by_key(kernel.find_by_test_id("chart")[0])
        .unwrap()
        .id;
    let b = *presenter
        .boxes()
        .iter()
        .find(|b| b.id == chart)
        .expect("the svg's box");
    let picked = |p: &mut Presenter<NoData>, x: f32, y: f32| {
        p.press_at(b.rect.0 + x, b.rect.1 + y, 0.0);
        let k = p.host().kernel();
        let text = k.node_by_key(k.find_by_test_id("picked")[0]).unwrap();
        text.props
            .str(exact_kernel::PropId::Text)
            .unwrap_or("")
            .to_string()
    };
    assert_eq!(
        picked(&mut presenter, 20., 50.),
        "bars",
        "a bar bubbles to its g"
    );
    assert_eq!(
        picked(&mut presenter, 50., 60.),
        "back",
        "pointer-events: none passes through to what is below"
    );
    assert_eq!(picked(&mut presenter, 80., 50.), "line", "on the stroke");
    assert_eq!(picked(&mut presenter, 95., 95.), "back");
}
