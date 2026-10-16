//! The live table on Apple (LLP 1121 D13, stage 4): a batch from the dev
//! stream sets a running build's rows in place, with no restart (its state
//! stays) and no layout for colours; a build past the batch's base answers
//! stale.

use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;
use exact_runner::{design, DataError, DataSource, Event, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

#[test]
fn a_running_build_applies_a_batch_in_place() {
    let dir = std::env::temp_dir().join(format!("exact-apple-live-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let view = "component App\n  state n = 0\n  action bump\n    n = n + 1\n  view\n    column background-color=\"page\"\n      button press=bump testId=\"bump\"\n        text `${n}` color=\"ink\" testId=\"count\"\n";
    let plan = |tamber: &str| {
        std::fs::write(dir.join("app.tamber"), tamber).unwrap();
        std::fs::write(dir.join("app.contract"), view).unwrap();
        contract::compile_path(&dir.join("app.contract")).unwrap()
    };
    let a = plan("tamber t\n\ntheme\n  ink = #111111\n  page = #ffffff\n");
    let b = plan("tamber t\n\ntheme\n  ink = #ff0000\n  page = #ffffff\n");
    let _ = std::fs::remove_dir_all(&dir);
    let batch =
        design::batch(&design::shape(&a), &design::tokens(&a), &design::tokens(&b)).unwrap();
    let (mut host, _) = Host::boot(
        &a.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    let id = |host: &Host<NoData>, t: &str| {
        let k = host.runner().kernel();
        k.node_by_key(k.find_by_test_id(t)[0]).unwrap().id
    };
    let (bump, count) = (id(&host, "bump"), id(&host, "count"));
    host.dispatch(bump, Event::Press);
    let frame = host.runner().kernel().node(count).unwrap().frame;
    let out = host.apply_live(&batch.json());
    assert!(out.starts_with("{\"live\":\"applied\""), "{out}");
    let k = host.runner().kernel();
    let node = k.node(count).unwrap();
    assert_eq!(
        node.style.text_color,
        exact_kernel::ColorValue::Fixed(exact_kernel::Color::parse("#ff0000").unwrap())
    );
    assert_eq!(
        node.props.str(exact_kernel::PropId::Text),
        Some("1"),
        "no restart: the state stays"
    );
    assert_eq!(node.frame, frame, "a colour moves no geometry");
    assert!(host
        .apply_live(&batch.json())
        .starts_with("{\"live\":\"stale\""));
}
