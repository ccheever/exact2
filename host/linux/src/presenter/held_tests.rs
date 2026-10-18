//! Held states on the Linux host (LLP 1122 D8, LLP 1121 stage 8 E3; and
//! so Android's, which paints through this presenter): a hold frame or the
//! agent's `holdState` shows the state as the real one (the press paint for
//! `active`), real input on the node lets it go, a disabled node refuses
//! one and disabling lets it go, so does going, and a new generation lets
//! it go unsaid and takes it again under the same plan.
use super::*;

const APP: &str = r##"component App
  state presses = 0
  state off = false
  state shown = true
  action go
    presses = presses + 1
  action disable
    off = true
  action hide
    shown = false
  view
    column width=400 height=400 background-color="#ffffff"
      if shown
        box press=go disabled=off background-color="#00ff00" width=300 height=100 testId="card"
          :active
            background-color = "#0000ff"
          :hover
            background-color = "#ffff00"
          :focus-visible
            background-color = "#ff00ff"
          text "c" testId="child"
      box background-color="#00ff00" width=300 height=40 testId="plain"
      box press=go disabled=true background-color="#00ff00" width=300 height=40 testId="off"
        :hover
          background-color = "#ffff00"
      box press=disable width=100 height=30 testId="disable"
      box press=hide width=100 height=30 testId="hide"
      box press=go width=100 height=30 testId="outer"
        box disabled=true width=20 height=20 testId="inner-off"
      button press=go testId="native"
        text "n"
"##;

fn plan(app: &str) -> Vec<u8> {
    contract::compile(app)
        .unwrap_or_else(|e| panic!("{e}"))
        .encode()
}

fn boot() -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &plan(APP),
        (),
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.set_development(Some("http://127.0.0.1:9/".into()), Some("plan-a".into()));
    // A stream, as a registration opens one: hold reports are its.
    p.dev
        .as_mut()
        .unwrap()
        .link
        .as_mut()
        .unwrap()
        .settle_nonce_for_test("n1");
    p
}

fn view(p: &Presenter<()>, test_id: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// A point on `test_id`'s ground, clear of its text.
fn ground(p: &mut Presenter<()>, test_id: &str) -> (f32, f32) {
    let id = view(p, test_id);
    let r = p.rect_of(id).unwrap();
    (r.0 + r.2 - 20., r.1 + r.3 / 2.)
}

fn pixel(p: &mut Presenter<()>, at: (f32, f32)) -> [u8; 3] {
    let c = p
        .frame()
        .pixel(at.0 as u32, at.1 as u32)
        .unwrap()
        .demultiply();
    [c.red(), c.green(), c.blue()]
}

/// `test_id`'s `{site, instance, state}`, as a server's hold frame names it.
fn target(p: &Presenter<()>, test_id: &str, state: &str) -> Value {
    let id = view(p, test_id);
    let node: Value = serde_json::from_str(
        &p.host()
            .agent(&format!(r#"{{"op":"node","id":{id},"plan":true}}"#)),
    )
    .unwrap();
    json!({"site": node["site"], "instance": node["instance"], "state": state})
}

/// Apply `frames`, then the reports queued for the next tick.
fn serve(p: &mut Presenter<()>, frames: Vec<Value>) -> Vec<Value> {
    let link = p.dev.as_mut().unwrap().link.as_mut().unwrap();
    link.frames = frames;
    p.serve_build_channel();
    p.frame();
    reports(p)
}

fn reports(p: &mut Presenter<()>) -> Vec<Value> {
    let link = p.dev.as_mut().unwrap().link.as_mut().unwrap();
    let sent = link.exchange().unwrap().reports_for_test();
    sent.into_iter().filter(|r| r["kind"] != "status").collect()
}

fn hold(revision: u64, hold: Value) -> Value {
    json!({"kind": "hold", "revision": revision, "hold": hold})
}

const GREEN: [u8; 3] = [0, 255, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const YELLOW: [u8; 3] = [255, 255, 0];
const MAGENTA: [u8; 3] = [255, 0, 255];

#[test]
fn a_held_active_is_the_press_paint_until_real_input_lets_it_go() {
    let mut p = boot();
    // A finger's press: no focus ring (and its look) after it.
    p.set_pointer_kind(PointerKind::Touch);
    let at = ground(&mut p, "card");
    assert_eq!(pixel(&mut p, at), GREEN);
    // Held on the child: the press the child's press resolves to, the card's.
    let child = target(&p, "child", "active");
    assert_eq!(
        serve(&mut p, vec![hold(1, child)]),
        [json!({"kind": "held", "state": "active", "revision": 1})]
    );
    let now = p.host().now();
    p.clock(now + 30.);
    assert_ne!(pixel(&mut p, at), BLUE, "eased in as a real press");
    p.clock(now + 200.);
    assert_eq!(pixel(&mut p, at), BLUE);
    // `layout` says what is held and the rows it shows.
    let card = view(&p, "card");
    let layout: Value = serde_json::from_str(&p.layout_held(Some(card), false)).unwrap();
    assert_eq!(
        layout["node"]["held"],
        json!({"state": "active", "rows": [{"name": "press_background_color", "value": "#0000ff"}]})
    );
    let layout: Value =
        serde_json::from_str(&p.layout_held(Some(view(&p, "child")), false)).unwrap();
    assert_eq!(
        layout["node"]["held"],
        json!({"state": "active", "rows": [{"name": "press_background_color", "value": "#0000ff"}]}),
        "named on the child, it answers for the child too, with the card's rows"
    );
    // A real press on it lets it go; the press goes on, and its release lets it up.
    p.pointer_down(at.0, at.1, now + 210.).unwrap();
    assert_eq!(
        reports(&mut p),
        [json!({"kind": "held", "state": null, "reason": "input", "revision": 1})]
    );
    p.clock(now + 260.);
    assert_eq!(pixel(&mut p, at), BLUE, "the real press's");
    p.pointer_up(at.0, at.1, now + 260.).unwrap();
    p.clock(now + 500.);
    assert_eq!(pixel(&mut p, at), GREEN);
    assert!(p.held().is_none());
}

/// A real press already down when a hold comes is the hold's: its release
/// leaves the press paint, which goes when the hold is let go.
#[test]
fn a_held_active_takes_a_real_press_over() {
    let mut p = boot();
    p.set_pointer_kind(PointerKind::Touch);
    let at = ground(&mut p, "card");
    p.pointer_down(at.0, at.1, 0.).unwrap();
    let card = target(&p, "card", "active");
    assert_eq!(serve(&mut p, vec![hold(2, card)])[0]["state"], "active");
    p.pointer_up(at.0, at.1, 10.).unwrap();
    p.clock(300.);
    assert_eq!(pixel(&mut p, at), BLUE, "the release leaves it");
    assert!(reports(&mut p).is_empty(), "the release is no input");
    assert_eq!(
        serve(&mut p, vec![hold(3, Value::Null)]),
        [
            json!({"kind": "held", "state": null, "reason": "replaced", "revision": 2}),
            json!({"kind": "held", "state": null, "revision": 3})
        ]
    );
    p.clock(600.);
    assert_eq!(pixel(&mut p, at), GREEN);
}

#[test]
fn a_held_hover_or_focus_is_the_look_until_the_pointer_or_focus_crosses() {
    let mut p = boot();
    let at = ground(&mut p, "card");
    let hover = target(&p, "card", "hover");
    assert_eq!(
        serve(&mut p, vec![hold(1, hover.clone())])[0]["state"],
        "hover"
    );
    assert_eq!(pixel(&mut p, at), YELLOW, "at once, no handler told");
    let card = view(&p, "card");
    let layout: Value = serde_json::from_str(&p.layout_held(Some(card), false)).unwrap();
    assert_eq!(
        layout["node"]["held"],
        json!({"state": "hover", "rows": [{"name": "background_color", "value": "#ffff00"}]})
    );
    // Moving elsewhere is nothing to it; coming over it lets it go, the real
    // hover in its place.
    p.pointer_move(390., 390., 0.).unwrap();
    assert!(reports(&mut p).is_empty());
    p.pointer_move(at.0, at.1, 1.).unwrap();
    assert_eq!(reports(&mut p)[0]["reason"], "input");
    assert_eq!(pixel(&mut p, at), YELLOW, "the real hover's");
    // Held with the pointer over it: moving within it is nothing, leaving it lets go.
    assert_eq!(serve(&mut p, vec![hold(2, hover)])[0]["state"], "hover");
    let child = p.rect_of(view(&p, "child")).unwrap();
    p.pointer_move(child.0 + 1., child.1 + 1., 2.).unwrap();
    assert!(reports(&mut p).is_empty());
    p.pointer_move(390., 390., 3.).unwrap();
    assert_eq!(
        reports(&mut p),
        [json!({"kind": "held", "state": null, "reason": "input", "revision": 2})]
    );
    assert_eq!(pixel(&mut p, at), GREEN);
    // A focus: its look, until the focus comes.
    let focus = target(&p, "card", "focus-visible");
    assert_eq!(
        serve(&mut p, vec![hold(3, focus)])[0]["state"],
        "focus-visible"
    );
    assert_eq!(pixel(&mut p, at), MAGENTA);
    p.focus = Some(card);
    p.frame();
    assert_eq!(reports(&mut p)[0]["reason"], "input");
}

#[test]
fn a_disabled_node_refuses_a_hold_and_disabling_lets_one_go() {
    let mut p = boot();
    let off = target(&p, "off", "hover");
    assert_eq!(
        serve(&mut p, vec![hold(1, off)]),
        [json!({"kind": "held", "state": null, "reason": "disabled", "revision": 1})]
    );
    let at = ground(&mut p, "off");
    assert_eq!(pixel(&mut p, at), GREEN);
    let card = target(&p, "card", "active");
    assert_eq!(serve(&mut p, vec![hold(2, card)])[0]["state"], "active");
    p.tap(view(&p, "disable")).unwrap();
    p.frame();
    assert_eq!(
        reports(&mut p),
        [json!({"kind": "held", "state": null, "reason": "disabled", "revision": 2})]
    );
    assert!(p.held().is_none());
    // A disabled node inside an enabled press stops `active` there, as a real press does.
    let inner = target(&p, "inner-off", "active");
    assert_eq!(
        serve(&mut p, vec![hold(3, inner)]),
        [json!({"kind": "held", "state": null, "reason": "disabled", "revision": 3})]
    );
}

#[test]
fn going_lets_a_hold_go_and_no_single_view_holds_none() {
    let mut p = boot();
    // No press there, nor around it: gone.
    let plain = target(&p, "plain", "active");
    assert_eq!(serve(&mut p, vec![hold(1, plain)])[0]["reason"], "gone");
    let card = target(&p, "card", "hover");
    assert_eq!(
        serve(&mut p, vec![hold(2, card.clone())])[0]["state"],
        "hover"
    );
    // No view at that site and instance path: the one held before goes too.
    let mut nowhere = card.clone();
    nowhere["instance"] = json!([{"region": 999, "arm": 0}]);
    assert_eq!(
        serve(&mut p, vec![hold(3, nowhere)]),
        [
            json!({"kind": "held", "state": null, "reason": "replaced", "revision": 2}),
            json!({"kind": "held", "state": null, "reason": "gone", "revision": 3})
        ]
    );
    // A state this host does not know is said so.
    let mut strange = card.clone();
    strange["state"] = json!("pressed");
    let r = serve(&mut p, vec![hold(4, strange)]);
    assert_eq!(
        (&r[0]["kind"], &r[0]["state"]),
        (&json!("unsupported"), &json!("pressed"))
    );
    // The instance compares as a value, its keys in any order.
    let mut reordered = card.clone();
    let steps: Vec<Value> = (card["instance"].as_array().unwrap().iter())
        .map(|s| {
            let mut keys: Vec<(&String, &Value)> = s.as_object().unwrap().iter().collect();
            keys.reverse();
            Value::Object(
                keys.into_iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            )
        })
        .collect();
    reordered["instance"] = Value::Array(steps);
    assert_eq!(serve(&mut p, vec![hold(5, reordered)])[0]["state"], "hover");
    p.tap(view(&p, "hide")).unwrap();
    p.frame();
    assert_eq!(
        reports(&mut p),
        [json!({"kind": "held", "state": null, "reason": "gone", "revision": 5})]
    );
}

/// A dev loop's new generation lets the hold go unsaid, and takes it again
/// itself under the same plan; under another plan the server replays it.
#[test]
fn a_new_generation_lets_a_hold_go_unsaid_and_takes_it_again() {
    let mut p = boot();
    let at = ground(&mut p, "card");
    let hover = target(&p, "card", "hover");
    assert_eq!(serve(&mut p, vec![hold(7, hover)])[0]["state"], "hover");
    let install = |p: &mut Presenter<()>, identity: &str, app: &str| {
        p.painted = true;
        p.pending_dev = Some(crate::fetch::Generation {
            identity: identity.into(),
            plan: plan(app),
            module: None,
        });
        p.poll_development(|| ());
        assert!(p.pending_dev.is_none(), "installed");
        p.frame();
    };
    install(&mut p, "gen-b", APP);
    assert_eq!(
        reports(&mut p),
        [json!({"kind": "held", "state": "hover", "revision": 7})],
        "no gone; held again"
    );
    assert_eq!(pixel(&mut p, at), YELLOW);
    install(&mut p, "gen-c", &APP.replace("#ffff00", "#ffff01"));
    assert!(reports(&mut p).is_empty(), "unsaid: the server replays");
    assert!(p.held().is_none());
    assert_eq!(pixel(&mut p, at), GREEN);
}

#[test]
fn the_agents_hold_state_holds_and_lets_go() {
    let mut p = boot();
    let at = ground(&mut p, "card");
    let card = view(&p, "card");
    let ask = |p: &mut Presenter<()>, line: String| -> Value {
        serde_json::from_str(&crate::agent::handle(p, &line)).unwrap()
    };
    let r = ask(
        &mut p,
        format!(r#"{{"op":"tap","id":{card},"holdState":"active"}}"#),
    );
    assert_eq!(
        (&r["tapped"], &r["holdState"], &r["delivery"]),
        (&json!(card), &json!("active"), &json!("recognized"))
    );
    assert_eq!(pixel(&mut p, at), BLUE, "at once on the agent's clock");
    let layout = ask(&mut p, format!(r#"{{"op":"layout","id":{card}}}"#));
    assert_eq!(layout["node"]["held"]["state"], "active");
    assert_eq!(
        layout["node"]["held"]["rows"][0]["name"],
        "press_background_color"
    );
    // One hold, shared with the build channel's: a frame's is replaced, said so.
    let hover = target(&p, "card", "hover");
    assert_eq!(serve(&mut p, vec![hold(1, hover)])[0]["state"], "hover");
    let r = ask(
        &mut p,
        format!(r#"{{"op":"tap","id":{card},"holdState":"focus-visible"}}"#),
    );
    assert_eq!(r["holdState"], "focus-visible");
    assert_eq!(
        reports(&mut p),
        [json!({"kind": "held", "state": null, "reason": "replaced", "revision": 1})]
    );
    assert_eq!(pixel(&mut p, at), MAGENTA);
    let r = ask(
        &mut p,
        format!(r#"{{"op":"tap","id":{card},"holdState":null}}"#),
    );
    assert_eq!(
        (&r["holdState"], &r["delivery"]),
        (&Value::Null, &json!("recognized"))
    );
    assert_eq!(pixel(&mut p, at), GREEN);
    let r = ask(
        &mut p,
        format!(r#"{{"op":"tap","id":{card},"holdState":"pressed"}}"#),
    );
    assert_eq!(r["malformed"], true);
    let off = view(&p, "off");
    let r = ask(
        &mut p,
        format!(r#"{{"op":"tap","id":{off},"holdState":"hover"}}"#),
    );
    assert!(r["error"].as_str().unwrap().contains("disabled"), "{r}");
    let plain = view(&p, "plain");
    let r = ask(
        &mut p,
        format!(r#"{{"op":"tap","id":{plain},"holdState":"active"}}"#),
    );
    assert!(
        r["error"].as_str().unwrap().contains("no press there"),
        "{r}"
    );
}

#[test]
fn a_hold_of_another_plan_or_of_a_revision_ended_here_is_nothing() {
    let mut p = boot();
    let card = target(&p, "card", "hover");
    // Addressed to another plan (one installed since): nothing, unsaid.
    let mut other = hold(1, card.clone());
    other["plan"] = json!("another");
    assert!(serve(&mut p, vec![other]).is_empty());
    assert!(p.held().is_none());
    // Addressed to this plan: held.
    let mut this = hold(2, card.clone());
    this["plan"] = json!(p.host().runner().inspection_digest());
    assert_eq!(
        serve(&mut p, vec![this.clone()]),
        [json!({"kind": "held", "state": "hover", "revision": 2})]
    );
    // Real input ends it; its frame again (a resent answer) is nothing.
    let at = ground(&mut p, "card");
    let now = p.host().now();
    p.pointer_down(at.0, at.1, now).unwrap();
    p.pointer_up(at.0, at.1, now + 10.).unwrap();
    assert_eq!(reports(&mut p)[0]["reason"], "input");
    assert!(serve(&mut p, vec![this]).is_empty());
    assert!(p.held().is_none());
    // A lower revision is a new server's: held.
    assert_eq!(
        serve(&mut p, vec![hold(1, card)]),
        [json!({"kind": "held", "state": "hover", "revision": 1})]
    );
}

/// A held press let go while the contact that was down on the node has
/// moved off it: no real press holds it, so the paint goes.
#[test]
fn a_held_press_let_go_with_its_contact_off_the_node_unpaints() {
    let mut p = boot();
    p.set_pointer_kind(PointerKind::Touch);
    let at = ground(&mut p, "card");
    p.pointer_down(at.0, at.1, 0.).unwrap();
    let card = target(&p, "card", "active");
    assert_eq!(serve(&mut p, vec![hold(2, card)])[0]["state"], "active");
    p.pointer_move(at.0, at.1 + 300., 10.).unwrap();
    serve(&mut p, vec![hold(3, Value::Null)]);
    p.clock(600.);
    assert_eq!(pixel(&mut p, at), GREEN);
}

/// A held press on a control the painter draws shows its pressed look, as
/// a contact's press does, and letting it go takes it off.
#[test]
fn a_held_press_on_a_painted_control_shows_its_pressed_look() {
    let mut p = boot();
    let native = view(&p, "native");
    let held = target(&p, "native", "active");
    assert_eq!(serve(&mut p, vec![hold(1, held)])[0]["state"], "active");
    assert!(p.brush.presses(native));
    // A contact's press on another control leaves it pressed too.
    let at = ground(&mut p, "card");
    p.pointer_down(at.0, at.1, 0.).unwrap();
    assert!(p.brush.presses(native));
    p.pointer_up(at.0, at.1, 10.).unwrap();
    serve(&mut p, vec![hold(2, Value::Null)]);
    assert!(!p.brush.presses(native));
}

/// A same-plan generation on a display that acknowledges pictures: a held
/// `active` named on a node inside the press is taken again (the kernel's
/// parents, not the last picture's), with no `gone`.
#[test]
fn a_held_press_named_inside_survives_a_displayed_generation() {
    let mut p = boot();
    let first = p.display_frame().unwrap();
    assert!(p.display_complete(&first));
    let child = target(&p, "child", "active");
    assert_eq!(serve(&mut p, vec![hold(4, child)])[0]["state"], "active");
    p.painted = true;
    p.pending_dev = Some(crate::fetch::Generation {
        identity: "gen-b".into(),
        plan: plan(APP),
        module: None,
    });
    p.poll_development(|| ());
    assert!(p.pending_dev.is_none(), "installed");
    assert_eq!(
        reports(&mut p),
        [json!({"kind": "held", "state": "active", "revision": 4})]
    );
    assert!(p.held().is_some());
}

/// A frame for another plan leaves the frame kept and what is held alone;
/// a hold's let-go on an earlier stream says nothing on the new one.
#[test]
fn a_frame_of_another_plan_and_an_earlier_streams_let_go_say_nothing() {
    let mut p = boot();
    let card = target(&p, "card", "hover");
    assert_eq!(
        serve(&mut p, vec![hold(2, card.clone())])[0]["state"],
        "hover"
    );
    let mut other = hold(3, card);
    other["plan"] = json!("another");
    assert!(serve(&mut p, vec![other]).is_empty());
    assert!(p.held().is_some(), "still held");
    let kept = p
        .dev
        .as_ref()
        .unwrap()
        .link
        .as_ref()
        .unwrap()
        .hold
        .as_ref()
        .map(|h| h.0);
    assert_eq!(kept, Some(2), "the frame kept is the one held");
    // A new stream (a reconnect): the old hold's let-go is no news to it.
    p.dev
        .as_mut()
        .unwrap()
        .link
        .as_mut()
        .unwrap()
        .settle_nonce_for_test("n2");
    p.tap(view(&p, "disable")).unwrap();
    p.frame();
    assert!(reports(&mut p).is_empty(), "{:?}", p.held());
    // Nor is a frame kept on the old stream taken again on the new one.
    p.rehold();
    assert!(p.held().is_none() && reports(&mut p).is_empty());
}
