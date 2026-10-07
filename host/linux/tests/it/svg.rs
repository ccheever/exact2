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

const APP: &str = "component App\n  state picked = \"none\"\n  action pick(which: string)\n    picked = which\n  view\n    column\n      text picked testId=\"picked\"\n      svg testId=\"chart\" width=100 height=100 viewBox=\"0 0 100 100\"\n        rect width=100 height=100 fill=\"#eeeeee\" press=pick(\"back\")\n        g press=pick(\"bars\")\n          rect x=10 y=10 width=20 height=80 fill=\"#2563eb\"\n          rect x=40 y=40 width=20 height=50 fill=\"#2563eb\" pointer-events=\"none\"\n        polyline points=\"70,90 90,10\" fill=\"none\" stroke=\"#000000\" stroke-width=6 press=pick(\"line\")\n";

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

/// LLP 1084 §4: a portable symbol role is its path, stroked as the web
/// strokes it, in its tint; an `sf/` name stays an empty box.
#[test]
fn a_symbol_role_is_its_path_and_an_sf_name_is_empty() {
    const SYMBOLS: &str = "component App\n  view\n    row\n      image \"symbol:checkmark\" testId=\"role\" width=48 height=48 object-fit=\"contain\" -exact-tint-color=\"#ff0000\"\n      image \"symbol:sf/checkmark\" testId=\"sf\" width=48 height=48 object-fit=\"contain\" -exact-tint-color=\"#ff0000\"\n";
    let plan = contract::compile(SYMBOLS).unwrap_or_else(|e| panic!("{e}"));
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (100., 48.),
        1.,
        std::env::temp_dir(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let reds = |p: &mut Presenter<NoData>, id: &str| {
        let k = p.host().kernel();
        let node = k.node_by_key(k.find_by_test_id(id)[0]).unwrap().id;
        let b = *p.boxes().iter().find(|b| b.id == node).unwrap();
        let frame = p.frame();
        let mut n = 0;
        for y in 0..48 {
            for x in 0..48 {
                let c = frame
                    .pixel(b.rect.0 as u32 + x, b.rect.1 as u32 + y)
                    .unwrap()
                    .demultiply();
                n += usize::from(c.red() > 200 && c.green() < 80 && c.alpha() > 200);
            }
        }
        n
    };
    assert!(
        reds(&mut p, "role") > 40,
        "the checkmark's stroke, in its tint"
    );
    assert_eq!(
        reds(&mut p, "sf"),
        0,
        "an SF Symbol name draws nothing on Linux"
    );
}

/// Every portable role draws on Linux from its path alone: a role added to
/// the schema with a path this parser cannot read would be an empty box here
/// while Apple shows its SF Symbol (podcast F3's media roles among them).
#[test]
fn every_symbol_role_draws_its_path() {
    let roles = exact_kernel::generated::SYMBOL_ROLES;
    let mut source = String::from("component App\n  view\n    column\n");
    for role in roles {
        source.push_str(&format!(
            "      image \"symbol:{role}\" testId=\"{role}\" width=24 height=24 font-size=24 -exact-tint-color=\"#ff0000\"\n"
        ));
    }
    let plan = contract::compile(&source).unwrap_or_else(|e| panic!("{e}"));
    let height = 24. * roles.len() as f32;
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (24., height),
        1.,
        std::env::temp_dir(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let frame = p.frame();
    let empty: Vec<_> = roles
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            let ink = (0..24u32)
                .flat_map(|y| (0..24u32).map(move |x| (x, y)))
                .filter_map(|(x, y)| frame.pixel(x, *i as u32 * 24 + y))
                .filter(|c| c.alpha() > 40 && c.red() > c.green())
                .count();
            // `ellipsis`, three round dots, is the least ink of any role.
            ink < 3
        })
        .map(|(_, role)| *role)
        .collect();
    assert!(empty.is_empty(), "roles that draw nothing: {empty:?}");
}

/// LLP 1055.000 D15 (issue #123): a path's `d` under `transition` morphs
/// on the agent's clock, as Chrome does: the chevron is flat half way, and
/// stands the other way when it settles.
#[test]
fn a_transitioning_d_morphs_the_path() {
    const MORPH: &str = "component App\n  state open = false\n  action flip\n    open = not open\n  view\n    column\n      button press=flip testId=\"flip\"\n        text \"Flip\"\n      svg testId=\"icon\" width=96 height=96 viewBox=\"0 0 24 24\"\n        path fill=\"none\" stroke=\"#000000\" stroke-width=2 d=(open ? \"M6 15 L12 9 L18 15\" : \"M6 9 L12 15 L18 9\") transition=\"d 400ms linear\"\n";
    let plan = contract::compile(MORPH).unwrap_or_else(|e| panic!("{e}"));
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (200., 200.),
        1.,
        std::env::temp_dir(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let id = |p: &Presenter<NoData>, name: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
    };
    let icon = id(&p, "icon");
    // The rows of the icon with ink in them: its height in points.
    let ink = |p: &mut Presenter<NoData>| {
        let b = *p.boxes().iter().find(|b| b.id == icon).unwrap();
        let frame = p.frame();
        (0..96u32)
            .filter(|y| {
                (0..96u32).any(|x| {
                    let c = frame
                        .pixel(b.rect.0 as u32 + x, b.rect.1 as u32 + y)
                        .unwrap()
                        .demultiply();
                    c.alpha() > 200 && c.red() < 100
                })
            })
            .count()
    };
    let rest = ink(&mut p);
    assert!((28..=34).contains(&rest), "a chevron: {rest}");
    p.tap(id(&p, "flip")).unwrap();
    p.tick(200.);
    let flat = ink(&mut p);
    assert!(flat <= 10, "flat half way: {flat}");
    p.tick(500.);
    assert_eq!(ink(&mut p), rest, "settled the other way");
}
