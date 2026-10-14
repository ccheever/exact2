//! @ref LLP 1069.000 D3 — `rem` and `em` from a Contract source to the
//! kernel, and the root font size a host sets through the runner.
use exact_kernel::{Dimension, Kernel, RowValue, StyleId};
use exact_runner::{DataError, DataSource, Runner, RunnerError, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("no data: {source}")
    }
}

const SOURCE: &str = "component App\n  view\n    column testId=\"box\" padding-top=\"1rem\" font-size=\"0.875rem\"\n      text \"Offline\" testId=\"label\" letter-spacing=\"0.1em\" margin-top=\"8px\"\n";

fn node<'a>(r: &'a Runner<NoData>, id: &str) -> exact_kernel::NodeRef<'a> {
    let key = r.kernel().find_by_test_id(id)[0];
    r.kernel().node_by_key(key).unwrap()
}

fn font(r: &Runner<NoData>, id: &str) -> f64 {
    match node(r, id).computed(StyleId::FontSize) {
        RowValue::Number(n) => n,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_root_font_size_rescales_rem_and_em_in_one_commit() {
    let plan = contract::bake(contract::compile(SOURCE).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(r.root_font_size(), 16.0);
    assert_eq!(font(&r, "label"), 14.0);
    assert_eq!(node(&r, "box").style.padding_top, Dimension::Points(16.0));
    assert!(r.set_root_font_size(24.0).unwrap().is_some());
    assert_eq!(font(&r, "label"), 21.0);
    assert_eq!(node(&r, "box").style.padding_top, Dimension::Points(24.0));
    assert!((node(&r, "label").style.letter_spacing - 2.1).abs() < 1e-5);
    assert_eq!(node(&r, "label").style.margin_top, Dimension::Points(8.0));
    assert!(
        r.set_root_font_size(24.0).unwrap().is_none(),
        "the same size"
    );
    assert!(matches!(
        r.set_root_font_size(f64::NAN),
        Err(RunnerError::Kernel(_))
    ));
    assert_eq!(r.root_font_size(), 24.0);
}

const APP: &str = "component App\n  action pick(v: number)\n    setRootFontSize(v)\n  action reset\n    setRootFontSize(\"medium\")\n  view\n    column testId=\"box\" padding-top=\"1rem\" font-size=\"1.5rem\"\n      text \"rem\" testId=\"label\"\n      text \"px\" testId=\"px\" font-size=\"24px\"\n";

#[test]
fn the_apps_root_font_size_lands_in_its_commit_over_the_hosts() {
    let plan = contract::bake(contract::compile(APP).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.act("pick", vec![Value::Number(20.0)]).unwrap();
    assert_eq!(r.root_font_size(), 20.0, "in the action's own commit");
    assert_eq!(font(&r, "label"), 30.0);
    assert_eq!(node(&r, "box").style.padding_top, Dimension::Points(20.0));
    assert_eq!(font(&r, "px"), 24.0, "px never scales");
    let commands = r.take_commands();
    assert_eq!(
        commands[0].name, "setRootFontSize",
        "kept for the web's document"
    );
    // The host's size changes beneath the app's, which stands (Dynamic Type
    // under `html { font-size: 20px }`), and comes back with "medium".
    assert!(r.set_root_font_size(24.0).unwrap().is_none());
    assert_eq!(font(&r, "label"), 30.0);
    assert_eq!(r.host_root_font_size(), 24.0, "what a new runner is told");
    assert!(r.set_root_font_size(f64::NAN).is_err());
    r.act("reset", vec![]).unwrap();
    assert_eq!(r.root_font_size(), 24.0, "the host's again");
    assert_eq!(font(&r, "label"), 36.0);
    // A computed size of 0 or less is refused, journaled, and never reaches a host.
    r.take_commands();
    let before = r.journal().count();
    r.act("pick", vec![Value::Number(0.0)]).unwrap();
    assert_eq!(r.root_font_size(), 24.0);
    assert!(r.take_commands().is_empty());
    let said: Vec<&str> = r.journal().skip(before).collect();
    assert!(
        said.iter()
            .any(|l| l.contains("setRootFontSize(0) refused")),
        "{said:?}"
    );
    assert!(
        !said.iter().any(|l| l.contains("command setRootFontSize")),
        "{said:?}"
    );
    // With the app's size gone, the host's moves the root again.
    assert!(r.set_root_font_size(16.0).unwrap().is_some());
    assert_eq!(font(&r, "label"), 24.0);
}

const SHARE: &str = "component App\n  action larger\n    setRootFontSize(\"125%\")\n  action pick(v: string)\n    setRootFontSize(v)\n  action fixed\n    setRootFontSize(20)\n  action reset\n    setRootFontSize(\"medium\")\n  view\n    column testId=\"box\" padding-top=\"1rem\" font-size=\"1.5rem\"\n      text \"rem\" testId=\"label\"\n";

#[test]
fn the_apps_percentage_follows_the_hosts_size() {
    let plan = contract::bake(contract::compile(SHARE).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    // Two Dynamic Type sizes under one stored percentage.
    assert!(r.set_root_font_size(17.0).unwrap().is_some());
    r.act("larger", vec![]).unwrap();
    assert_eq!(r.root_font_size(), 21.25, "in the action's own commit");
    assert_eq!(font(&r, "label"), 31.875);
    assert_eq!(
        r.take_commands()[0].args,
        vec![Value::str("125%")],
        "kept for the web's document"
    );
    assert!(r.set_root_font_size(20.0).unwrap().is_some(), "re-resolves");
    assert_eq!(r.root_font_size(), 25.0);
    assert_eq!(node(&r, "box").style.padding_top, Dimension::Points(25.0));
    assert_eq!(r.host_root_font_size(), 20.0, "what a new runner is told");
    assert!(
        r.set_root_font_size(20.0).unwrap().is_none(),
        "the same size"
    );
    // A host size whose share does not fit is refused; the share stands.
    assert!(r.set_root_font_size(3e38).is_err());
    assert_eq!(r.root_font_size(), 25.0);
    assert_eq!(r.host_root_font_size(), 20.0);
    // So does an app string that is no percentage.
    r.act("pick", vec![Value::str("1e-100%")]).unwrap();
    r.act("pick", vec![Value::str("125")]).unwrap();
    assert_eq!(r.root_font_size(), 25.0);
    assert!(r.set_root_font_size(16.0).unwrap().is_some());
    assert_eq!(r.root_font_size(), 20.0, "still a share of the host's");
    assert!(r.set_root_font_size(20.0).unwrap().is_some());
    // A computed percentage, then px, which stands, then the host's again.
    r.act("pick", vec![Value::str("80%")]).unwrap();
    assert_eq!(r.root_font_size(), 16.0);
    r.act("fixed", vec![]).unwrap();
    assert!(r.set_root_font_size(24.0).unwrap().is_none());
    assert_eq!(r.root_font_size(), 20.0);
    r.act("pick", vec![Value::str("50%")]).unwrap();
    assert_eq!(
        r.root_font_size(),
        12.0,
        "a share of the host's, not the app's px"
    );
    r.act("reset", vec![]).unwrap();
    assert_eq!(r.root_font_size(), 24.0);
    // A computed string that is no percentage is refused and journaled.
    r.take_commands();
    let before = r.journal().count();
    for bad in ["0%", "125", "1e400%", "125 %"] {
        r.act("pick", vec![Value::str(bad)]).unwrap();
    }
    assert_eq!(r.root_font_size(), 24.0);
    assert!(r.take_commands().is_empty());
    let said: Vec<&str> = r.journal().skip(before).collect();
    assert!(
        said.iter()
            .any(|l| l.contains("setRootFontSize(\"125\") refused")),
        "{said:?}"
    );
}

#[test]
fn a_literal_size_of_zero_and_a_string_are_refused_by_the_compiler() {
    for good in ["setRootFontSize(\"125%\")", "setRootFontSize(\".5e2%\")"] {
        let source = format!("component App\n  action pick\n    {good}\n  view\n    text \"x\"\n");
        assert!(contract::compile(&source).is_ok(), "{good}");
    }
    for bad in [
        "setRootFontSize(0)",
        "setRootFontSize(\"20px\")",
        "setRootFontSize(\"0%\")",
        "setRootFontSize(\"125\")",
        "setRootFontSize()",
    ] {
        let source = format!("component App\n  action pick\n    {bad}\n  view\n    text \"x\"\n");
        let errors = contract::compile(&source).unwrap_err();
        assert!(
            format!("{errors:?}").contains("type-root-font-size"),
            "{bad}: {errors:?}"
        );
    }
}
