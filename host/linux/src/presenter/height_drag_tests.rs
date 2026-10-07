use super::*;
use exact_kernel::Dimension;
use exact_runner::{DataError, Value};
#[derive(Default)]
struct Rows;
impl DataSource for Rows {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::list(
            (0..25_000).map(|n| Value::Number(n as f64)).collect(),
        ))
    }
}
const APP: &str = r#"component App
  resource rows = rows() as shape list<number>
  state target = 180
  state count = 0
  state seen = 0
  state velocity = 0
  state disabled = false
  state showing = true
  state draft = ""
  action edit(value)
    draft = value
  action release(h: number, v: number)
    count = count + 1
    seen = h
    velocity = v
    target = h < 270 ? 180 : h < 500 ? 360 : 640
  action disable
    disabled = true
  action hide
    showing = false
  view
    box width=400 height=500
      button press=disable testId="disable"
        text "disable"
      button press=hide testId="hide"
        text "hide"
      input value=draft input=edit testId="input"
      when showing
        column id="panel" testId="panel" position="absolute" bottom=0 width=400 height=target max-height="100%" box-sizing="border-box" transition="height -exact-spring(300,30,1)"
          box testId="handle" heightDragFor="panel" heightrelease=release height=40 touch-action="none" disabled=disabled
            text "drag header"
          list virtualized=true scrollFollowEnd=true flex=1 min-height=0 width="100%" testId="port"
            each x in rows key=x
              text `row ${x}` height=24
      text `${count}` testId="count"
      text `${seen}` testId="seen"
      text `${velocity}` testId="velocity"
"#;
fn boot(source: &str) -> Presenter<Rows> {
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        Rows,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    for _ in 0..10 {
        assert!(p.pump(0.).is_none());
        let _ = p.frame();
    }
    p
}
fn id(p: &Presenter<Rows>, name: &str) -> ViewId {
    p.host
        .kernel()
        .node_by_key(p.host.kernel().find_by_test_id(name)[0])
        .unwrap()
        .id
}
fn text<'a>(p: &'a Presenter<Rows>, name: &str) -> &'a str {
    p.host
        .kernel()
        .node(id(p, name))
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
}
fn panel_height(p: &Presenter<Rows>) -> f32 {
    p.host.kernel().node(id(p, "panel")).unwrap().frame.height
}
fn recognize(p: &mut Presenter<Rows>) {
    assert!(p.pointer_down(200., 340., 0.).unwrap());
    assert!(p.pointer_move(200., 330., 10.).unwrap());
    assert_eq!(
        panel_height(p),
        180.,
        "recognition catches with zero displacement"
    );
}
#[test]
fn header_drag_final_sample_typed_action_release_and_pin_order() {
    let mut p = boot(APP);
    recognize(&mut p);
    let token = p
        .contact
        .as_ref()
        .unwrap()
        .hold
        .as_ref()
        .unwrap()
        .primary
        .token;
    assert_eq!(p.collection_interaction(), Some(id(&p, "handle")));
    // Up itself is the final 120px rise; no earlier move can supply it.
    assert!(p.pointer_up(200., 210., 40.).unwrap());
    assert_eq!(text(&p, "count"), "1");
    assert_eq!(text(&p, "seen"), "300");
    assert_eq!(panel_height(&p), 300.);
    assert_eq!(
        p.host.kernel().node(id(&p, "panel")).unwrap().style.height,
        Dimension::Points(360.)
    );
    assert!(!p.host.has_hold(token));
    assert!(p.collection_interaction().is_none());
    assert!(!p.pointer_up(200., 100., 50.).unwrap());
    assert_eq!(text(&p, "count"), "1");
    p.tick(10_000.);
    let _ = p.frame();
    assert_eq!(panel_height(&p), 360.);
}
#[test]
fn header_is_exclusive_inner_list_wheel_and_typing_keep_working() {
    let mut p = boot(APP);
    assert!(p.pointer_down(200., 420., 0.).unwrap());
    assert!(!p.pointer_move(200., 390., 10.).unwrap());
    assert!(!p.pointer_up(200., 390., 20.).unwrap());
    assert_eq!(panel_height(&p), 180.);
    assert_eq!(text(&p, "count"), "0");
    p.wheel(id(&p, "port"), 0., 10_000_000.).unwrap();
    for _ in 0..10 {
        p.pump(p.host.now());
        let _ = p.frame();
    }
    let snapshot = p.host.collections().pop().unwrap();
    assert_eq!(snapshot.rows.last().unwrap().index, 24_999);
    assert!(snapshot.rows.len() < 64);
    p.pointer_down(200., 340., 30.).unwrap();
    p.pointer_move(200., 330., 40.).unwrap();
    p.pointer_move(200., 210., 50.).unwrap();
    p.type_text(id(&p, "input"), "still typing").unwrap();
    assert_eq!(panel_height(&p), 300.);
    p.wheel(id(&p, "port"), 0., -100.).unwrap();
    assert!(p.contact.is_none());
    assert!(p.collection_interaction().is_none());
    assert!(!p.pointer_up(200., 100., 60.).unwrap());
    assert_eq!(text(&p, "count"), "0");
}
#[test]
fn constrained_height_plateau_has_zero_displayed_release_velocity() {
    let mut p = boot(APP);
    recognize(&mut p);
    p.pointer_move(200., -300., 20.).unwrap();
    assert_eq!(panel_height(&p), 500.);
    p.pointer_move(200., -500., 130.).unwrap();
    p.pointer_move(200., -700., 160.).unwrap();
    p.pointer_up(200., -900., 190.).unwrap();
    assert_eq!(text(&p, "seen"), "500");
    assert_eq!(text(&p, "velocity"), "0");
    assert_eq!(
        p.host.kernel().node(id(&p, "panel")).unwrap().style.height,
        Dimension::Points(640.)
    );
    assert_eq!(panel_height(&p), 500.);
}
#[test]
fn cancellation_disable_deletion_and_reload_never_dispatch_late_release() {
    for action in ["cancel", "disable", "hide", "reload"] {
        let mut p = boot(APP);
        recognize(&mut p);
        p.pointer_move(200., 210., 20.).unwrap();
        let token = p
            .contact
            .as_ref()
            .unwrap()
            .hold
            .as_ref()
            .unwrap()
            .primary
            .token;
        match action {
            "cancel" => p.pointer_cancel(30.).unwrap(),
            "reload" => {
                p.reload(&contract::compile(APP).unwrap().encode(), Rows)
                    .unwrap();
            }
            name => {
                p.tap(id(&p, name)).unwrap();
            }
        }
        assert!(!p.host.has_hold(token), "{action}");
        assert!(p.collection_interaction().is_none(), "{action}");
        assert!(!p.pointer_up(200., 100., 40.).unwrap());
        assert_eq!(text(&p, "count"), "0");
    }
}

#[test]
fn agent_contact_phases_drive_presenter_and_refuse_malformed_or_second_down() {
    let mut p = boot(APP);
    let handle = id(&p, "handle");
    let down = format!(r#"{{"op":"tap","id":{handle},"phase":"down","x":200,"y":340}}"#);
    let reply = crate::agent::handle(&mut p, &down);
    assert!(reply.contains(r#""delivery":"presenter""#), "{reply}");
    assert!(!reply.contains(r#""delivery":"platform""#));
    let decoded: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(decoded["at"], serde_json::json!([200., 340.]));
    assert_eq!(decoded["clock"], 0.);

    let before = p.host.now();
    for invalid in [
        down.as_str(),
        r#"{"op":"tap","phase":"move","x":200,"y":"bad","ms":100}"#,
        r#"{"op":"tap","phase":"move","dx":1,"dy":2,"x":3,"y":4}"#,
        r#"{"op":"tap","phase":"hold","ms":-1}"#,
        r#"{"op":"tap","phase":"cancel","ms":1}"#,
        r#"{"op":"tap","phase":"hold","ms":100000000}"#,
    ] {
        assert!(
            crate::agent::handle(&mut p, invalid).contains("error"),
            "{invalid}"
        );
        assert_eq!(p.host.now(), before);
        assert_eq!(p.contact_position(), Some((200., 340.)));
    }
    for (command, point, clock) in [
        (
            r#"{"op":"tap","phase":"move","x":200,"y":330,"ms":10}"#,
            [200., 330.],
            10.,
        ),
        (
            r#"{"op":"tap","phase":"move","dx":0,"dy":-120,"ms":30}"#,
            [200., 210.],
            40.,
        ),
        (
            r#"{"op":"tap","phase":"hold","ms":100}"#,
            [200., 210.],
            140.,
        ),
    ] {
        let reply = crate::agent::handle(&mut p, command);
        assert!(!reply.contains("error"), "{reply}");
        let decoded: serde_json::Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(decoded["at"], serde_json::json!(point));
        assert_eq!(decoded["clock"], clock);
    }
    assert_eq!(panel_height(&p), 300.);
    assert_eq!(text(&p, "count"), "0");
    let reply = crate::agent::handle(&mut p, r#"{"op":"tap","phase":"up"}"#);
    assert!(!reply.contains("error"), "{reply}");
    let decoded: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(decoded["at"], serde_json::json!([200., 210.]));
    assert_eq!(decoded["clock"], 140.);
    assert_eq!(decoded["contact"], false);
    assert_eq!(text(&p, "count"), "1");
    assert_eq!(text(&p, "velocity"), "0");
    assert_eq!(p.host.now(), 140.);
    assert!(p.contact.is_none());
    assert!(p.collection_interaction().is_none());
}

#[test]
fn agent_cancel_retires_contact_without_release_action() {
    let mut p = boot(APP);
    recognize(&mut p);
    p.pointer_move(200., 210., 20.).unwrap();
    // Agent phases seek the Runner clock as well as presentation.
    assert!(p.clock(20.).1.is_none());
    let reply = crate::agent::handle(&mut p, r#"{"op":"tap","phase":"cancel"}"#);
    assert!(!reply.contains("error"), "{reply}");
    let decoded: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(decoded["at"], serde_json::json!([200., 210.]));
    assert_eq!(decoded["clock"], 20.);
    assert_eq!(decoded["contact"], false);
    assert!(p.contact.is_none());
    assert!(p.collection_interaction().is_none());
    assert_eq!(text(&p, "count"), "0");
}

#[test]
fn agent_aborted_move_reports_last_delivered_step_not_requested_endpoint() {
    let mut p = boot(APP);
    p.pointer_down(200., 340., 0.).unwrap();
    // Ten planned samples; the first horizontal sample declines this vertical handle.
    let reply = crate::agent::handle(
        &mut p,
        r#"{"op":"tap","phase":"move","x":300,"y":340,"ms":160}"#,
    );
    let decoded: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(decoded["at"], serde_json::json!([210., 340.]));
    assert_eq!(decoded["clock"], 16.);
    assert_eq!(decoded["contact"], false);
    assert_eq!(p.host.now(), 16.);
    assert_eq!(text(&p, "count"), "0");
}

#[test]
fn agent_late_up_and_cancel_after_accepted_action_retirement_allow_new_down() {
    for phase in ["up", "cancel"] {
        let source = APP.replace("disabled = true", "disabled = not disabled");
        let mut p = boot(&source);
        let down = format!(r#"{{"op":"tap","phase":"down","id":{}}}"#, id(&p, "handle"));
        let reply = crate::agent::handle(&mut p, &down);
        assert!(!reply.contains("error"), "{reply}");
        let reply = crate::agent::handle(
            &mut p,
            r#"{"op":"tap","phase":"move","dx":0,"dy":-10,"ms":10}"#,
        );
        assert!(!reply.contains("error"), "{reply}");
        let disable = format!(r#"{{"op":"tap","id":{}}}"#, id(&p, "disable"));
        let reply = crate::agent::handle(&mut p, &disable);
        assert!(!reply.contains("error"), "{reply}");
        assert!(p.contact.is_none());
        let before = p.host.now();
        let late = format!(r#"{{"op":"tap","phase":"{phase}"}}"#);
        for _ in 0..2 {
            let reply: serde_json::Value =
                serde_json::from_str(&crate::agent::handle(&mut p, &late)).unwrap();
            assert_eq!(reply["contact"], false, "late {phase}: {reply}");
            assert!(reply["at"].is_null());
            assert!(reply.get("error").is_none(), "{reply}");
            assert_eq!(p.host.now(), before);
            assert_eq!(text(&p, "count"), "0");
        }
        // The other input re-enables the handle. A new contact must be usable.
        let reply = crate::agent::handle(&mut p, &disable);
        assert!(!reply.contains("error"), "{reply}");
        let reply: serde_json::Value =
            serde_json::from_str(&crate::agent::handle(&mut p, &down)).unwrap();
        assert_eq!(reply["contact"], true, "{reply}");
        assert!(p.contact.is_some());
        p.pointer_cancel(p.host.now()).unwrap();
    }
}
