//! A tab not yet selected, on a platform that builds it when first shown
//! (`unopened.rs`): the runner keeps it, the presenter is given it in the
//! batch that selects it.
use super::*;
use exact_kernel::MonospaceMeasurer;
use exact_plan::Value;
use exact_runner::DataError;
use serde_json::Value as Json;

#[derive(Clone, Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn canvas_surfaces(&self) -> Vec<(String, usize)> {
        vec![("spark".into(), 1)]
    }
    fn draw_2d(
        &mut self,
        _: &str,
        _: &[Value],
        ctx: &exact_runner::exact_canvas::Context2d,
        _: &exact_runner::exact_canvas::Frame,
    ) -> Result<bool, exact_runner::exact_canvas::DrawError> {
        ctx.fill_rect(0.0, 0.0, 4.0, 4.0);
        Ok(false)
    }
}

const APP: &str = r##"routes nav
  tab home "/"
  tab saved "/saved"

keyframes tint
  to color="#0000ff" opacity=0.3

component App
  state label = "one"
  state shown = true
  action pick(name: string)
    nav = select(nav, name)
  action rename
    label = "two"
    shown = false
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" width="100%" height="100%" display="flex" flex-direction="column"
      column flex=1 position="relative"
        for t in nav.tabs key=t.name
          column role="tabpanel" id=`panel-${t.name}` testId=`panel-${t.name}` position="absolute" inset=0
            for e in t.stack key=e.id
              column navigationKey=`${e.id}` testId=`route-${e.name}` position="absolute" inset=0
                color=(shown ? "#ff0000" : "#0000ff") transition="color 1s linear"
                text (e.name == "home" ? "Home" : label) testId=`title-${e.name}`
                if e.name == "saved"
                  box opacity=0.5 width=10 height=10 testId="faded"
                  canvas surface=chart(label) width=10 height=10 testId="chart"
                  column color="#00ff00"
                    text "Tinted" animation="tint 100ms forwards" testId="tinted"
                  if shown
                    box width=10 height=10 testId="gone"
      row role="tablist"
        button role="tab" aria-controls="panel-home" aria-selected=(nav.tab == "home") press=pick("home") testId="tab-home"
          text "Home"
        button role="tab" aria-controls="panel-saved" aria-selected=(nav.tab == "saved") press=pick("saved") testId="tab-saved"
          text "Saved"
      button press=rename testId="rename"
        text "Rename"
"##;

fn boot(wait: bool) -> (Host<NoData>, Vec<Json>) {
    boot_app(APP, wait)
}

fn boot_app(app: &str, wait: bool) -> (Host<NoData>, Vec<Json>) {
    unopened::TABS_WAIT.set(wait);
    let plan = contract::compile(app)
        .unwrap_or_else(|e| panic!("{e}"))
        .encode();
    let (host, batch) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    (host, ops(&batch))
}

fn ops(batch: &str) -> Vec<Json> {
    let v: Json = serde_json::from_str(batch).unwrap();
    v["ops"].as_array().cloned().unwrap_or_default()
}

fn id(host: &Host<NoData>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// The ops of one kind naming `test_id`.
fn named<'a>(host: &Host<NoData>, ops: &'a [Json], op: &str, test_id: &str) -> Vec<&'a Json> {
    let id = id(host, test_id);
    ops.iter()
        .filter(|o| o["op"] == op && o["id"] == id)
        .collect()
}

fn press(host: &mut Host<NoData>, test_id: &str, at: f64) -> Vec<Json> {
    let view = id(host, test_id);
    ops(&host.dispatch_at(view, Event::Press, at))
}

/// At boot the other tab's panel and route are views; what the route
/// holds is not, nor is it laid out, while the runner keeps all of it.
#[test]
fn an_unselected_tab_is_kept_but_not_presented_or_measured() {
    let (host, ops) = boot(true);
    for shown in ["panel-saved", "route-saved", "title-home", "tab-saved"] {
        assert_eq!(
            named(&host, &ops, "create", shown).len(),
            1,
            "{shown} is made"
        );
    }
    for held in ["title-saved", "faded", "gone"] {
        assert!(
            named(&host, &ops, "create", held).is_empty(),
            "{held} waits"
        );
        assert!(host.runner().inactive(id(&host, held)));
        let frame = host.runner().kernel().node(id(&host, held)).unwrap().frame;
        assert_eq!(
            (frame.width, frame.height),
            (0.0, 0.0),
            "{held} is not laid out"
        );
    }
    let route = id(&host, "route-saved");
    let children = ops
        .iter()
        .find(|o| o["op"] == "children" && o["id"] == route);
    assert!(children.is_none_or(|o| o["ids"].as_array().unwrap().is_empty()));
}

/// Selecting it builds it in that batch, with what changed while it
/// waited; it stays built when another tab is selected.
#[test]
fn a_tab_is_built_in_the_batch_that_first_selects_it() {
    let (mut host, _) = boot(true);
    // Changed and taken away while it waits: nothing is sent for it.
    let renamed = press(&mut host, "rename", 10.0);
    let title = id(&host, "title-saved");
    assert!(!renamed.iter().any(|o| o["id"] == title), "{renamed:?}");
    let opened = press(&mut host, "tab-saved", 20.0);
    for made in ["title-saved", "faded"] {
        assert_eq!(
            named(&host, &opened, "create", made).len(),
            1,
            "{made}: {opened:?}"
        );
    }
    assert!(
        opened
            .iter()
            .any(|o| o["id"] == title && o.to_string().contains("two")),
        "{opened:?}"
    );
    let route = id(&host, "route-saved");
    let children = opened
        .iter()
        .find(|o| o["op"] == "children" && o["id"] == route)
        .unwrap();
    assert_eq!(children["ids"][0], title);
    let frame = named(&host, &opened, "frame", "title-saved");
    assert!(
        frame.last().unwrap()["w"].as_f64().unwrap() > 0.0,
        "{frame:?}"
    );
    let faded = named(&host, &opened, "present", "faded");
    assert_eq!(faded.len(), 1, "{opened:?}");
    assert!(
        faded[0]["property"] == "opacity" && faded[0]["x"] == 0.5,
        "{opened:?}"
    );
    let back = press(&mut host, "tab-home", 30.0);
    let again = press(&mut host, "tab-saved", 40.0);
    for batch in [&back, &again] {
        assert!(!batch
            .iter()
            .any(|o| o["id"] == title && (o["op"] == "destroy" || o["op"] == "create")));
    }
}

/// Built when first selected, a tab is laid out and presented as it is
/// where every tab is built at boot.
#[test]
fn a_tab_built_late_is_laid_out_as_one_built_at_boot() {
    let frames = |wait: bool| {
        let (mut host, _) = boot(wait);
        let opened = press(&mut host, "tab-saved", 20.0);
        ["title-saved", "faded", "gone"].map(|t| {
            let node = host.runner().kernel().node(id(&host, t)).unwrap();
            let sent = named(&host, &opened, "frame", t)
                .last()
                .map(|o| o["w"].clone());
            (node.frame, wait.then_some(sent))
        })
    };
    let (late, early) = (frames(true), frames(false));
    for ((late, sent), (early, _)) in late.into_iter().zip(early) {
        assert_eq!(late, early);
        assert_eq!(
            sent.flatten().and_then(|w| w.as_f64()),
            Some(f64::from(late.width))
        );
    }
}

/// A GPU canvas's inputs set while its tab waits reach it when the tab is
/// built: the runner does not send them again unchanged.
#[test]
fn a_waiting_canvas_gets_its_surface_inputs_when_its_tab_is_built() {
    let (mut host, boot_ops) = boot(true);
    assert!(named(&host, &boot_ops, "surface", "chart").is_empty());
    let renamed = press(&mut host, "rename", 10.0);
    assert!(named(&host, &renamed, "surface", "chart").is_empty());
    let opened = press(&mut host, "tab-saved", 20.0);
    let surface = named(&host, &opened, "surface", "chart");
    assert_eq!(surface.len(), 1, "{opened:?}");
    assert_eq!(surface[0]["name"], "chart");
    assert_eq!(surface[0]["values"], serde_json::json!(["two"]));
}

/// A colour moving while the tab waits shows where it is when the tab is
/// built, on the views that inherit it, as a boot would show it.
#[test]
fn a_waiting_tab_is_built_with_its_presented_colour() {
    // Each view's last text colour and lowered animations, after the
    // colour inherited from the route has moved halfway and an animation
    // of its own has ended filling forwards, both while the tab waited.
    let shown = |wait: bool| {
        let (mut host, mut all) = boot(wait);
        all.extend(press(&mut host, "rename", 10.0));
        all.extend(ops(&host.advance(510.0)));
        all.extend(press(&mut host, "tab-saved", 510.0));
        ["title-saved", "tinted"].map(|t| {
            let view = id(&host, t);
            let colour = all
                .iter()
                .rev()
                .filter(|o| o["id"] == view && (o["op"] == "create" || o["op"] == "style"))
                .find_map(|o| o["style"].get("text_color").cloned());
            let lowered = all
                .iter()
                .rev()
                .find(|o| o["id"] == view && o["op"] == "animations")
                .map(|o| o["specs"].clone());
            (colour, lowered)
        })
    };
    let (late, early) = (shown(true), shown(false));
    assert_eq!(late, early);
    assert_eq!(late[1].0, Some(serde_json::json!([0, 0, 255, 255])));
    assert!(late[1].1.is_some(), "the opacity track is the compositor's");
}

/// A tabpanel in flow is laid out while its tab waits: its size moves its
/// siblings, so the layout never depends on which tabs were visited.
#[test]
fn a_waiting_panel_in_flow_keeps_its_place_in_the_layout() {
    let app = in_flow(APP);
    let frames = |wait: bool| {
        let (host, ops) = boot_app(&app, wait);
        let held = named(&host, &ops, "create", "title-saved").is_empty();
        let k = host.runner().kernel();
        let boxes = ["panel-home", "panel-saved", "title-saved", "tab-home"]
            .map(|t| k.node(id(&host, t)).unwrap().frame);
        (boxes, held)
    };
    let ((late, held), (early, _)) = (frames(true), frames(false));
    assert!(held, "its contents still wait for the presenter");
    assert_eq!(late, early);
}

/// Where tabs are not built lazily (the Mac), every tab is presented at boot.
#[test]
fn elsewhere_every_tab_is_presented_at_boot() {
    let (host, ops) = boot(false);
    for shown in ["title-saved", "faded", "gone"] {
        assert_eq!(named(&host, &ops, "create", shown).len(), 1, "{shown}");
    }
}

/// A tab inside a waiting tab waits on its own: building the outer one
/// leaves what the inner one holds held, its canvas's inputs kept until
/// the inner tab is selected.
#[test]
fn a_tab_inside_a_waiting_tab_waits_for_its_own_selection() {
    let app = APP
        .replace(
            "  action rename\n",
            "  state inner = \"a\"\n  action pickInner(name: string)\n    inner = name\n  action rename\n",
        )
        .replace(
            "                  canvas surface=chart(label) width=10 height=10 testId=\"chart\"\n",
            r#"                  column navigationKey=inner navigationBack="back" position="absolute" inset=0
                    column role="tabpanel" id="panel-a" position="absolute" inset=0
                      column navigationKey="a" position="absolute" inset=0
                        text "A" testId="inner-a"
                    column role="tabpanel" id="panel-b" position="absolute" inset=0
                      column navigationKey="b" position="absolute" inset=0
                        canvas surface=chart(label) width=10 height=10 testId="chart"
                    row role="tablist"
                      button role="tab" aria-controls="panel-a" aria-selected=(inner == "a") press=pickInner("a") testId="inner-tab-a"
                        text "A"
                      button role="tab" aria-controls="panel-b" aria-selected=(inner == "b") press=pickInner("b") testId="inner-tab-b"
                        text "B"
"#,
        );
    let (mut host, _) = boot_app(&app, true);
    press(&mut host, "rename", 10.0);
    let outer = press(&mut host, "tab-saved", 20.0);
    assert_eq!(
        named(&host, &outer, "create", "inner-a").len(),
        1,
        "{outer:?}"
    );
    let chart = id(&host, "chart");
    assert!(!outer.iter().any(|o| o["id"] == chart), "{outer:?}");
    let inner = press(&mut host, "inner-tab-b", 30.0);
    assert_eq!(
        named(&host, &inner, "create", "chart").len(),
        1,
        "{inner:?}"
    );
    let surface = named(&host, &inner, "surface", "chart");
    assert_eq!(surface.len(), 1, "{inner:?}");
    assert_eq!(surface[0]["values"], serde_json::json!(["two"]));
}

/// A route of a panel whose layout was skipped took its `scrollTop` with
/// nothing to scroll; it takes it again in the batch that builds the tab.
#[test]
fn a_skipped_tabs_route_takes_its_scroll_position_when_built() {
    let app = APP.replace(
        "testId=`route-${e.name}` position=\"absolute\" inset=0",
        "testId=`route-${e.name}` position=\"absolute\" inset=0 overflow=\"auto\" scrollTop=40",
    );
    let (mut host, _) = boot_app(&app, true);
    let opened = press(&mut host, "tab-saved", 20.0);
    let props = named(&host, &opened, "props", "route-saved");
    assert!(
        props.iter().any(|o| o["set"]["scrollTop"] == "40"),
        "{opened:?}"
    );
}

/// The recipe's panels moved into flow: `IN_FLOW`.
fn in_flow(app: &str) -> String {
    app.replace(
        r#"testId=`panel-${t.name}` position="absolute" inset=0"#,
        r#"testId=`panel-${t.name}` height=200"#,
    )
    .replace(r#"column flex=1 position="relative""#, "column flex=1")
}

/// What the saved route holds, with `extra` after its faded box.
fn with(app: &str, extra: &str) -> String {
    let faded = "                  box opacity=0.5 width=10 height=10 testId=\"faded\"\n";
    app.replace(faded, &format!("{faded}{extra}"))
}

/// A tab holding a 2D canvas is built at once, at boot or in the commit
/// that makes the canvas, before it draws: a canvas draws onto its bitmap,
/// and what it drew while it waited could not be drawn again.
#[test]
fn a_tab_holding_a_2d_canvas_is_built_at_once() {
    let spark = "                  canvas surface=spark(1) width=10 height=10 testId=\"spark\"\n";
    let (host, boot) = boot_app(&with(APP, spark), true);
    for made in ["spark", "title-saved"] {
        assert_eq!(named(&host, &boot, "create", made).len(), 1, "{made}");
    }
    assert!(!named(&host, &boot, "canvas2d", "spark").is_empty());
    let later = "                  if not shown\n    canvas surface=spark(1) width=10 height=10 testId=\"spark\"\n";
    let later = later.replace("\n    canvas", "\n                    canvas");
    let (mut host, boot) = boot_app(&with(APP, &later), true);
    assert!(named(&host, &boot, "create", "title-saved").is_empty());
    let renamed = press(&mut host, "rename", 10.0);
    let view = id(&host, "spark");
    let made = renamed
        .iter()
        .position(|o| o["op"] == "create" && o["id"] == view);
    let drawn = renamed
        .iter()
        .position(|o| o["op"] == "canvas2d" && o["id"] == view);
    assert!(
        made.is_some_and(|m| drawn.is_some_and(|d| d > m)),
        "{renamed:?}"
    );
    assert_eq!(named(&host, &renamed, "create", "title-saved").len(), 1);
}

/// An `svg` whose scene was published while it waited (its colour moving
/// with a route's in a panel laid out in flow) is sent its scene when its
/// view is made, though the scene did not change.
#[test]
fn a_waiting_svg_is_sent_its_scene_when_built() {
    let svg = "                  svg width=10 height=10 viewBox=\"0 0 10 10\" testId=\"mark\"\n                    polyline points=\"0,0 10,10\" fill=\"none\" stroke=\"currentcolor\"\n";
    let (mut host, _) = boot_app(&with(&in_flow(APP), svg), true);
    press(&mut host, "rename", 10.0);
    host.advance(1100.0);
    let opened = press(&mut host, "tab-saved", 1200.0);
    assert_eq!(
        named(&host, &opened, "create", "mark").len(),
        1,
        "{opened:?}"
    );
    assert_eq!(named(&host, &opened, "svg", "mark").len(), 1, "{opened:?}");
}

/// A panel moved from out of flow into flow while it waits keeps the
/// scroll positions it owes its routes: its layout was skipped when they
/// were first given.
#[test]
fn a_waiting_panel_moved_into_flow_still_owes_its_scroll_positions() {
    let app = APP
        .replace(
            "testId=`route-${e.name}` position=\"absolute\" inset=0",
            "testId=`route-${e.name}` position=\"absolute\" inset=0 overflow=\"auto\" scrollTop=40",
        )
        .replace(
            r#"testId=`panel-${t.name}` position="absolute" inset=0"#,
            r#"testId=`panel-${t.name}` position=(shown ? "absolute" : "relative") inset=0"#,
        );
    let (mut host, _) = boot_app(&app, true);
    press(&mut host, "rename", 10.0);
    let opened = press(&mut host, "tab-saved", 20.0);
    let props = named(&host, &opened, "props", "route-saved");
    assert!(
        props.iter().any(|o| o["set"]["scrollTop"] == "40"),
        "{opened:?}"
    );
}
