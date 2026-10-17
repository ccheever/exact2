//! The binding core's other kinds (LLP 1121 D12, stage 6 G): a length, a
//! number and a type token by name, each resolved for a place under the
//! window's mode and a pin, a type token as the font a host draws (its text
//! style's weight where it writes none, Bold Text's step on a system
//! family).

use exact_kernel::Kernel;
use exact_plan::{Plan, Value};
use exact_runner::tamber::{Length, Number, Token, Type};
use exact_runner::{DataError, DataSource, Place, Runner};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const DESIGN: &str = "tamber t\n\naxes\n  density = regular | compact\n\ntheme\n  space.row = 12px\n    when density=compact: 8px\n  space.rem = 1.5rem\n  opacity.faded: number = 0.5\n    when density=compact: 0.25\n  type.body = -exact-body/1.35 \"Inter\"\n    when density=compact: -exact-footnote/1.2 \"Inter\"\n  type.sys = 600 -exact-body system-ui\n";
const VIEW: &str = "component App\n  view\n    column padding=\"space.row\" opacity=\"opacity.faded\" margin=\"space.rem\"\n      text \"a\" testId=\"outer\" font=\"type.body\"\n      text \"s\" font=\"type.sys\"\n      column testId=\"card\" -exact-mode=\"density=compact\" padding=\"space.row\"\n        text \"b\" testId=\"inner\" font=\"type.body\"\n";

fn plan() -> Plan {
    let dir = std::env::temp_dir().join(format!("exact2-binding-kinds-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.tamber"), DESIGN).unwrap();
    std::fs::write(dir.join("app.contract"), VIEW).unwrap();
    let plan =
        contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e:?}"));
    let _ = std::fs::remove_dir_all(&dir);
    plan
}

fn node(r: &Runner<NoData>, id: &str) -> Place {
    let k = r.kernel();
    Place::Node(k.node_by_key(k.find_by_test_id(id)[0]).unwrap().id)
}

const ROW: Token<Length> = Token::new("space.row");
const REM: Token<Length> = Token::new("space.rem");
const FADED: Token<Number> = Token::new("opacity.faded");
const BODY: Token<Type> = Token::new("type.body");
const SYS: Token<Type> = Token::new("type.sys");

#[test]
fn each_kind_resolves_for_its_place() {
    let mut r = Runner::boot(
        contract::bake(plan(), NoData).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let (outer, inner) = (node(&r, "outer"), node(&r, "inner"));
    let mut t = r.tamber();
    assert_eq!(t.get(ROW, outer), Some(12.0));
    assert_eq!(t.get(ROW, inner), Some(8.0), "the pin's tuple");
    assert_eq!(
        t.get(REM, Place::Root),
        Some(24.0),
        "1.5rem at the root's 16"
    );
    assert_eq!(t.get(FADED, outer), Some(0.5));
    assert_eq!(t.get(FADED, inner), Some(0.25));
    let body = t.get(BODY, outer).unwrap();
    assert_eq!(body.weight, 400, "the body style's own weight, unsaid");
    assert!(
        (body.line_height.unwrap() - body.size * 1.35).abs() < 1e-6,
        "{body:?}"
    );
    assert!(body.family.is_some());
    let dense = t.get(BODY, inner).unwrap();
    assert!(dense.size < body.size, "footnote under the pin: {dense:?}");
    assert!((dense.line_height.unwrap() - dense.size * 1.2).abs() < 1e-6);
    assert_eq!(t.get(Token::<Length>::new("no.such"), Place::Root), None);
    // The window's mode moves the root's tuple.
    r.set_mode("density", "compact").unwrap();
    assert_eq!(r.tamber().get(ROW, Place::Root), Some(8.0));
    // Bold Text steps a system family's weight, not a named one's.
    r.set_bold_text(true).unwrap();
    let mut t = r.tamber();
    assert_eq!(t.get(SYS, Place::Root).unwrap().weight, 800);
    assert_eq!(
        t.get(BODY, outer).unwrap().weight,
        400,
        "Inter is the app's"
    );
}
