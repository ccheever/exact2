//! Primary-contact Arrange uses the same Presenter path as evdev and VNC.
use exact_kernel::{motion::motion_node, NodeKey};
use exact_linux::{presenter::PainterChoice, Presenter};
use exact_motion::Property;
use exact_runner::{DataError, DataSource, Items, Value};
use std::path::PathBuf;

struct Rows {
    rows: Items,
    calls: usize,
}
impl Rows {
    fn new(n: usize) -> Self {
        Self {
            rows: Items::from(
                (0..n)
                    .map(|i| {
                        Value::Record(Items::from(vec![
                            Value::str(&i.to_string()),
                            Value::str(if i == 0 { "#ff0000" } else { "#0000ff" }),
                            Value::Number(40.),
                        ]))
                    })
                    .collect::<Vec<_>>(),
            ),
            calls: 0,
        }
    }
}
impl DataSource for Rows {
    fn query(&mut self, name: &str, args: &[Value]) -> Result<Value, DataError> {
        self.calls += 1;
        if name == "removeFirst" {
            self.rows = Items::from(self.rows.iter().skip(1).cloned().collect::<Vec<_>>());
        }
        if name == "move" {
            let key = |v: &Value| {
                let Value::Record(fields) = v else {
                    unreachable!()
                };
                fields[0].as_str().unwrap().to_owned()
            };
            let mut rows = self.rows.to_vec();
            let i = rows
                .iter()
                .position(|v| key(v) == args[0].as_str().unwrap())
                .unwrap();
            let item = rows.remove(i);
            let before = match &args[1] {
                Value::Option(Some(v)) => v.as_str(),
                _ => None,
            };
            let at = before
                .and_then(|b| rows.iter().position(|v| key(v) == b))
                .unwrap_or(rows.len());
            rows.insert(at, item);
            self.rows = Items::from(rows);
        }
        Ok(Value::List(self.rows.clone()))
    }
}
const APP: &str = r#"shape Row
  id: string
  color: string
  height: number
component App
  state count = 0
  state draft = ""
  state disabled = false
  resource initial = rows() as shape list<Row>
  mutation changed as shape list<Row>
  derive rows = match changed { case some(value) => value, case none => initial }
  action drop(item: string, before: option<string>) writes changed, count
    send changed = move(item, before)
    count = count + 1
  action disable writes disabled
    disabled = true
  action edit(v: string) writes draft
    draft = v
  view
    box width="100%" height="100%"
      box position="absolute" left=20 top=40 width=300 height=184
        list id="items" testId="list" width="100%" height="100%" border-width=2 padding=0 virtualized=true reorderdrop=drop
          each x in rows key=x.id
            box testId=`row-${x.id}` height=x.height background-color=x.color
              box testId=`grip-${x.id}` width="100%" height="100%" reorderFor="items" touch-action="none" disabled=disabled
      button position="absolute" top=300 testId="disable" press=disable
        text "disable"
      input position="absolute" top=350 testId="draft" value=draft change=edit
"#;
fn boot() -> Presenter<Rows> {
    boot_source(APP)
}
fn boot_source(source: &str) -> Presenter<Rows> {
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        Rows::new(100),
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    for _ in 0..3 {
        p.frame();
    }
    p
}
fn key(p: &Presenter<Rows>, name: &str) -> NodeKey {
    p.host().kernel().find_by_test_id(name)[0]
}
fn id(p: &Presenter<Rows>, name: &str) -> u32 {
    p.host().kernel().node_by_key(key(p, name)).unwrap().id
}
fn recognize(p: &mut Presenter<Rows>) -> (NodeKey, (f32, f32)) {
    let h = key(p, "grip-0");
    let wrapper = p.host().runner().reorder_binding(h).unwrap().wrapper;
    let rect = p.rect_of(id(p, "grip-0")).unwrap();
    let point = (rect.0 + 10., rect.1 + 10.);
    assert!(p.pointer_down(point.0, point.1, 0.).unwrap());
    assert!(p.pointer_move(point.0, point.1 + 8., 10.).unwrap());
    assert!(p
        .host()
        .engine()
        .is_held(motion_node(wrapper), Property::Translate));
    assert_eq!(
        p.host()
            .engine()
            .value(motion_node(wrapper), Property::Translate)
            .unwrap()
            .y,
        0.
    );
    (wrapper, (point.0, point.1 + 8.))
}
#[test]
fn physical_vertical_contact_catches_wrapper_and_keeps_typing_live() {
    let mut p = boot();
    let (wrapper, point) = recognize(&mut p);
    let calls = p.host().runner().data_ref().calls;
    assert!(p.pointer_move(point.0, point.1 + 60., 20.).unwrap());
    assert_eq!(
        p.host()
            .engine()
            .value(motion_node(wrapper), Property::Translate)
            .unwrap()
            .y,
        60.
    );
    p.type_text(id(&p, "draft"), "typing").unwrap();
    assert!(p
        .host()
        .engine()
        .is_held(motion_node(wrapper), Property::Translate));
    assert_eq!(p.host().runner().data_ref().calls, calls);
    assert!(p.pointer_up(point.0, point.1 + 70., 30.).unwrap());
    assert_eq!(p.host().runner().slot("count"), Some(&Value::Number(1.)));
    assert!(!p
        .host()
        .engine()
        .is_held(motion_node(wrapper), Property::Translate));
    assert!(
        p.collection_interaction().is_some(),
        "settling source must stay pinned"
    );
    p.tick(5000.);
    p.frame();
    assert!(p.collection_interaction().is_none());
}
#[test]
fn latest_unproved_release_cannot_dispatch_previous_gap_and_late_up_is_inert() {
    let mut p = boot();
    let (_, point) = recognize(&mut p);
    p.pointer_move(point.0, point.1 + 60., 20.).unwrap();
    assert!(!p.pointer_up(point.0, 1000., 30.).unwrap());
    assert_eq!(p.host().runner().slot("count"), Some(&Value::Number(0.)));
    let now = p.host().now();
    assert!(!p.pointer_up(f32::NAN, 0., f64::NAN).unwrap());
    assert_eq!(p.host().now(), now);
}
#[test]
fn elevated_source_paints_and_hits_above_later_rows_inside_list_clip() {
    let mut p = boot();
    let (_, point) = recognize(&mut p);
    p.pointer_move(point.0, point.1 + 45., 20.).unwrap();
    let pixels = p.frame();
    let row = p.rect_of(id(&p, "row-0")).unwrap();
    let x = (row.0 + 10.) as u32;
    let y = (row.1 + 20.) as u32;
    let pixel = pixels.pixel(x, y).unwrap();
    assert_eq!((pixel.red(), pixel.green(), pixel.blue()), (255, 0, 0));
    assert_eq!(p.hit(x as f32, y as f32), Some(id(&p, "grip-0")));
    assert_ne!(
        p.hit(x as f32, 20.),
        Some(id(&p, "grip-0")),
        "lift must not escape list clip"
    );
}

#[test]
fn terminal_rebases_all_surviving_wrappers_and_uses_source_velocity_only() {
    let mut p = boot();
    let (source, point) = recognize(&mut p);
    p.pointer_move(point.0, point.1 + 50., 20.).unwrap();
    p.pointer_move(point.0, point.1 + 70., 30.).unwrap();
    let rows = p.host().collections()[0].rows.clone();
    let mut before = Vec::new();
    for row in rows {
        let key = p.host().kernel().node(row.view).unwrap().key;
        if let Some(rect) = p.rect_of(row.view) {
            before.push((key, rect));
        }
    }
    assert!(p.pointer_up(point.0, point.1 + 70., 30.).unwrap());
    let mut neighbors = 0;
    for (key, old) in before {
        let Some(id) = p.host().kernel().node_by_key(key).map(|n| n.id) else {
            continue;
        };
        let new = p.rect_of(id).unwrap();
        assert!(
            (new.0 - old.0).abs() < 0.01 && (new.1 - old.1).abs() < 0.01,
            "{key:?}: {old:?} -> {new:?}"
        );
        if let Some(curve) = p
            .host()
            .engine()
            .spring_descriptor(motion_node(key), Property::Translate)
        {
            if key == source {
                assert!(curve.velocity.y > 0.);
            } else {
                assert_eq!(curve.velocity, exact_motion::Value::ZERO);
                neighbors += 1;
            }
        }
    }
    assert!(neighbors > 0, "a no-neighbor oracle is vacuous");
}

#[test]
fn invalid_live_sample_is_atomic_and_disabled_receipt_cancels_at_receipt_time() {
    let mut p = boot();
    let (source, point) = recognize(&mut p);
    p.pointer_move(point.0, point.1 + 60., 20.).unwrap();
    let now = p.host().now();
    let value = p
        .host()
        .engine()
        .value(motion_node(source), Property::Translate);
    assert!(p.pointer_move(f32::NAN, point.1, 500.).is_err());
    assert_eq!(p.host().now(), now);
    assert_eq!(
        p.host()
            .engine()
            .value(motion_node(source), Property::Translate),
        value
    );
    p.clock(200.);
    p.tap(id(&p, "disable")).unwrap();
    assert!(!p
        .host()
        .engine()
        .is_held(motion_node(source), Property::Translate));
    let curve = p
        .host()
        .engine()
        .spring_descriptor(motion_node(source), Property::Translate)
        .unwrap();
    assert_eq!(curve.start, 0.2);
    assert!(!p.pointer_up(f32::NAN, 0., f64::NAN).unwrap());
    assert_eq!(p.host().now(), 200.);
    assert_eq!(p.host().runner().slot("count"), Some(&Value::Number(0.)));
}

#[test]
fn cancel_recatch_preserves_displayed_origin_and_old_settle_cannot_end_successor() {
    let mut p = boot();
    let (source, point) = recognize(&mut p);
    p.pointer_move(point.0, point.1 + 60., 20.).unwrap();
    p.pointer_cancel(20.).unwrap();
    p.tick(40.);
    let caught = p
        .host()
        .engine()
        .value(motion_node(source), Property::Translate)
        .unwrap();
    let rect = p.rect_of(id(&p, "grip-0")).unwrap();
    let point = (rect.0 + 5., rect.1 + 10.);
    p.pointer_down(point.0, point.1, 40.).unwrap();
    assert!(p.pointer_move(point.0, point.1 - 8., 40.).unwrap());
    assert_eq!(
        p.host()
            .engine()
            .value(motion_node(source), Property::Translate),
        Some(caught)
    );
    p.tick(5000.);
    assert!(p
        .host()
        .engine()
        .is_held(motion_node(source), Property::Translate));
    assert!(p.collection_interaction().is_some());
    p.pointer_cancel(5000.).unwrap();
    p.tick(10000.);
    p.frame();
    assert!(p.collection_interaction().is_none());
    assert_eq!(p.host().runner().slot("count"), Some(&Value::Number(0.)));
}

#[test]
fn edge_scroll_reads_actual_offset_and_keeps_stationary_source_in_view() {
    let mut p = boot();
    let (source, point) = recognize(&mut p);
    let port = p.rect_of(id(&p, "list")).unwrap();
    let y = port.1 + port.3 - 8.;
    p.pointer_move(point.0, y, 20.).unwrap();
    let before = p.rect_of(id(&p, "row-0")).unwrap();
    let list = id(&p, "list");
    let old = p.scroll_of(list).1;
    assert!(p.needs_animation_frame());
    p.tick(70.);
    p.frame();
    let after = p.rect_of(id(&p, "row-0")).unwrap();
    assert!(p.scroll_of(list).1 > old);
    assert!((after.1 - before.1).abs() < 0.01, "{before:?} {after:?}");
    assert!(p
        .host()
        .engine()
        .is_held(motion_node(source), Property::Translate));
    assert!(p.host().collections()[0].rows.len() < 32);
    p.pointer_cancel(70.).unwrap();
    p.tick(5000.);
    p.frame();
    assert!(!p.needs_animation_frame());
}

fn scroll_fixture(action: &str, writes: &str) -> String {
    APP.replace(
        "  state count = 0",
        &format!("  state scrolls = 0\n  state portWidth = 300\n  action scrolled(x: number, y: number) writes scrolls{writes}\n    scrolls = scrolls + 1\n{action}  state count = 0"),
    )
    .replace("width=300 height=184", "width=portWidth height=184")
    .replace("virtualized=true reorderdrop=drop", "virtualized=true reorderdrop=drop scroll=scrolled")
}

#[test]
fn edge_scroll_authored_counter_preserves_hold_position_and_one_terminal_drop() {
    let mut p = boot_source(&scroll_fixture("", ""));
    let (source, point) = recognize(&mut p);
    let list = id(&p, "list");
    let port = p.rect_of(list).unwrap();
    let y = port.1 + port.3 - 8.;
    p.pointer_move(point.0, y, 20.).unwrap();
    let before = p.rect_of(id(&p, "row-0")).unwrap();
    let old_scroll = p.scroll_of(list).1;
    p.tick(70.);
    assert_eq!(p.host().runner().slot("scrolls"), Some(&Value::Number(1.)));
    assert!(p.scroll_of(list).1 > old_scroll);
    assert!(p
        .host()
        .engine()
        .is_held(motion_node(source), Property::Translate));
    let after = p.rect_of(id(&p, "row-0")).unwrap();
    assert!((after.1 - before.1).abs() < 0.01, "{before:?} -> {after:?}");
    // The next boundary may be outside the port at its edge. Finish at a
    // measured interior gap; never turn NeedsMeasurement into a prior-gap drop.
    let release_y = y - 32.;
    assert!(p.pointer_up(point.0, release_y, 80.).unwrap());
    assert_eq!(p.host().runner().slot("count"), Some(&Value::Number(1.)));
    assert!(!p.pointer_up(point.0, release_y, 80.).unwrap());
    assert_eq!(p.host().runner().slot("count"), Some(&Value::Number(1.)));
}

#[test]
fn edge_scroll_authored_deletion_and_width_reflow_still_cancel_before_drop() {
    for (action, writes, deleted) in [
        ("    send changed = removeFirst()\n", ", changed", true),
        ("    portWidth = 200\n", ", portWidth", false),
    ] {
        let mut p = boot_source(&scroll_fixture(action, writes));
        let (source, point) = recognize(&mut p);
        let list = id(&p, "list");
        let port = p.rect_of(list).unwrap();
        let y = port.1 + port.3 - 8.;
        let width_before = p.host().kernel().node(list).unwrap().frame.width;
        p.pointer_move(point.0, y, 20.).unwrap();
        p.tick(70.);
        assert_eq!(p.host().runner().slot("scrolls"), Some(&Value::Number(1.)));
        assert!(!p
            .host()
            .engine()
            .is_held(motion_node(source), Property::Translate));
        assert_eq!(p.host().kernel().node_by_key(source).is_none(), deleted);
        if !deleted {
            assert_eq!(
                p.host().kernel().node(list).unwrap().frame.width,
                width_before - 100.
            );
        }
        let now = p.host().now();
        assert!(!p.pointer_up(f32::NAN, 0., f64::NAN).unwrap());
        assert_eq!(p.host().now(), now);
        assert_eq!(p.host().runner().slot("count"), Some(&Value::Number(0.)));
    }
}

#[test]
fn eager_and_disabled_grips_do_not_admit_physical_reorder() {
    for source in [
        APP.replace("virtualized=true", "virtualized=false"),
        APP.replace("state disabled = false", "state disabled = true"),
    ] {
        let mut p = boot_source(&source);
        let rect = p.rect_of(id(&p, "grip-0")).unwrap();
        p.pointer_down(rect.0 + 10., rect.1 + 10., 0.).unwrap();
        assert!(!p.pointer_move(rect.0 + 10., rect.1 + 40., 10.).unwrap());
        assert!(!p.pointer_up(rect.0 + 10., rect.1 + 40., 20.).unwrap());
        assert_eq!(p.host().runner().slot("count"), Some(&Value::Number(0.)));
    }
}

#[test]
fn typing_under_empty_when_arms_preserves_mapping_hold_and_exactly_one_drop() {
    let source = APP.replace(
        "state disabled = false",
        "state disabled = false\n  state shown = true\n  state windowed = true",
    );
    let (head, rest) = source.split_once("        list id=\"items\"").unwrap();
    let (list, tail) = rest
        .split_once("      button position=\"absolute\"")
        .unwrap();
    let list = format!("        list id=\"items\"{list}")
        .lines()
        .map(|line| format!("    {line}\n"))
        .collect::<String>();
    let source = format!("{head}        when shown\n          when windowed\n{list}      button position=\"absolute\"{tail}");
    let mut p = boot_source(&source);
    let (wrapper, point) = recognize(&mut p);
    p.pointer_move(point.0, point.1 + 60., 20.).unwrap();
    let handle = key(&p, "grip-0");
    let binding = p.host().runner().reorder_binding(handle).unwrap();
    let list = id(&p, "list");
    let port = p.rect_of(list).unwrap();
    let scroll = p.scroll_of(list);
    let geometry = p.host().runner().reorder_geometry(binding.list).unwrap();
    let pin = p.collection_interaction();
    let calls = p.host().runner().data_ref().calls;
    p.type_text(id(&p, "draft"), "typing without changing the List")
        .unwrap();
    // A mapping assertion first distinguishes valid reflow cancellation from token loss.
    assert_eq!(p.rect_of(list), Some(port));
    assert_eq!(p.scroll_of(list), scroll);
    assert!(
        p.host()
            .engine()
            .is_held(motion_node(wrapper), Property::Translate),
        "same port and scroll must retain the original hold"
    );
    assert_eq!(p.host().runner().reorder_binding(handle), Some(binding));
    assert_eq!(
        p.host().runner().reorder_geometry(binding.list),
        Some(geometry)
    );
    assert_eq!(p.collection_interaction(), pin);
    assert_eq!(p.host().runner().data_ref().calls, calls);
    assert!(p.pointer_up(point.0, point.1 + 70., 30.).unwrap());
    assert_eq!(p.host().runner().slot("count"), Some(&Value::Number(1.)));
    assert!(!p.pointer_up(point.0, point.1 + 70., 30.).unwrap());
    p.tick(5000.);
    p.frame();
    assert!(p.collection_interaction().is_none());
}
