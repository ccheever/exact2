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

/// SVG 2 §13.4.7: a zero-length subpath with round or square caps paints
/// its cap, a dot where Chrome paints one; a butt cap paints nothing.
#[test]
fn a_zero_length_subpath_paints_its_cap() {
    const DOTS: &str = "component App\n  view\n    svg testId=\"dots\" width=100 height=40 viewBox=\"0 0 100 40\"\n      path d=\"M 10 20 Z\" stroke=\"#ff0000\" stroke-width=12 stroke-linecap=\"round\"\n      path d=\"M 40 20 L 40 20\" stroke=\"#ff0000\" stroke-width=12 stroke-linecap=\"square\"\n      path d=\"M 70 20 L 70 20\" stroke=\"#ff0000\" stroke-width=12\n";
    let plan = contract::compile(DOTS).unwrap_or_else(|e| panic!("{e}"));
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (100., 40.),
        1.,
        std::env::temp_dir(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let k = p.host().kernel();
    let dots = k.node_by_key(k.find_by_test_id("dots")[0]).unwrap().id;
    let b = *p.boxes().iter().find(|b| b.id == dots).unwrap();
    let (x0, y0) = (b.rect.0 as u32, b.rect.1 as u32);
    let frame = p.frame();
    let red = |x: u32| {
        let c = frame.pixel(x0 + x, y0 + 20).unwrap().demultiply();
        (c.red(), c.green(), c.alpha())
    };
    assert_eq!(red(10), (255, 0, 255), "M p Z, round: a dot");
    assert_eq!(red(40), (255, 0, 255), "M p L p, square: a square");
    assert_ne!(red(70), (255, 0, 255), "a butt cap paints nothing");
}
