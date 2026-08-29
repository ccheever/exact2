//! The Linux host over the Caltrain app, headless: the tree lays out with
//! the kernel's layout and real text; every node has a painted box; the
//! page is a viewport over a document; presses go through hit-testing;
//! typing is one change; wheels chain; motion arrives as presentation
//! values; an image's size lays out; the agent's operations answer on the
//! wire.

use exact_linux::agent::handle;
use exact_linux::paint::PaintedBox;
use exact_linux::Presenter;
use exact_runner::{DataError, DataSource, Value};
use std::path::PathBuf;
use std::time::Duration;

fn assets() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain"))
}

fn boot() -> Presenter<caltrain_data::Caltrain> {
    let plan = caltrain::build().unwrap();
    let (mut p, error) = Presenter::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        (390.0, 844.0),
        1.0,
        assets(),
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.wait_images(Duration::from_secs(2));
    p
}

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn fixture(name: &str) -> Presenter<NoData> {
    let src = std::fs::read_to_string(format!(
        "{}/../../contract/corpus/{name}.contract",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap();
    let (p, error) =
        Presenter::boot(&plan.encode(), NoData, (390.0, 844.0), 1.0, assets()).unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

fn view<D: DataSource>(p: &Presenter<D>, test_id: &str) -> u32 {
    let k = p.host().kernel();
    let key = k.find_by_test_id(test_id)[0];
    k.node_by_key(key).unwrap().id
}

fn has<D: DataSource>(p: &Presenter<D>, test_id: &str) -> bool {
    !p.host().kernel().find_by_test_id(test_id).is_empty()
}

fn text<D: DataSource>(p: &Presenter<D>, test_id: &str) -> String {
    let id = view(p, test_id);
    p.host()
        .kernel()
        .node(id)
        .and_then(|n| n.props.str(exact_kernel::PropId::Text).map(str::to_string))
        .unwrap_or_default()
}

fn boxed<D: DataSource>(p: &mut Presenter<D>, test_id: &str) -> PaintedBox {
    let id = view(p, test_id);
    p.boxes().iter().find(|b| b.id == id).copied().unwrap()
}

#[test]
fn the_tree_lays_out_with_real_text_and_every_node_has_a_box() {
    let mut p = boot();
    let live = p.host().kernel().live_count();
    assert_eq!(p.boxes().len(), live, "every live node is painted");
    // The root is a block as wide as the viewport and, since the app lives
    // inside a sky canvas that fills the window (LLP 1014 §1a), as tall as
    // it: its `scroll` child holds the page.
    let root = boxed(&mut p, "caltrain-main");
    assert_eq!(
        (root.rect.0, root.rect.1, root.rect.2, root.rect.3),
        (0.0, 0.0, 390.0, 844.0),
        "{root:?}"
    );
    let name = boxed(&mut p, "station-name");
    assert!(
        name.rect.2 > 0.0 && name.rect.3 > 20.0,
        "24 pt text has a line box: {name:?}"
    );
    assert!(name.rect.3 < 40.0, "one line: {name:?}");
    let scroll = p.boxes().iter().filter(|b| b.scroll.is_some()).count();
    assert_eq!(scroll, 1, "one scroll container reports an offset");
    let faces = p.text().borrow().face_count();
    assert!(faces > 0, "the system has fonts");
    let measures = p.text().borrow().measures;
    assert!(measures > 100, "text went through the engine: {measures}");
}

#[test]
fn layout_json_is_the_agent_api_shape() {
    let mut p = boot();
    let l = p.layout_json();
    assert!(
        l.starts_with("{\"clock\":0,\"viewport\":{\"w\":390,\"h\":844},\"nodes\":["),
        "{}",
        &l[..80]
    );
    assert_eq!(l.matches("\"sx\":").count(), 1, "one scroll container");
    assert!(
        l.contains("\"id\":1,\"x\":0,\"y\":0,\"w\":390,"),
        "{}",
        &l[..120]
    );
}

#[test]
fn a_press_goes_through_hit_testing_and_bubbles_to_the_handler() {
    let mut p = boot();
    assert!(!has(&p, "stations-screen"));
    // The button's text child has no handler; the press reaches the button.
    let button = view(&p, "change-station");
    let child = p.host().kernel().node(button).unwrap().children()[0];
    let reply = p.tap(child).unwrap();
    assert!(
        reply.starts_with(&format!("{{\"tapped\":{child},\"at\":[")),
        "{reply}"
    );
    assert!(has(&p, "stations-screen"), "the stations screen opened");
    // A tap on a node nothing handles changes nothing.
    let title = view(&p, "station-search");
    let _ = p.tap(title);
    assert!(has(&p, "stations-screen"));
}

#[test]
fn typing_replaces_the_value_and_the_runner_hears_one_change() {
    let mut p = boot();
    let _ = p.tap(view(&p, "change-station"));
    let field = view(&p, "station-search");
    let reply = p.type_text(field, "Palo").unwrap();
    assert_eq!(reply, format!("{{\"typed\":{field},\"value\":\"Palo\"}}"));
    assert_eq!(p.focus(), Some(field), "the input has focus");
    let state = p.host().agent("{\"op\":\"state\"}");
    assert!(state.contains("\"query\":\"Palo\""), "{state}");
    assert!(
        has(&p, "station-paloalto") && !has(&p, "station-mv"),
        "the search narrowed the list"
    );
    let name = view(&p, "station-name");
    assert!(p.type_text(name, "x").is_err(), "not an input");
}

#[test]
fn a_wheel_scrolls_the_apps_scroll_node_and_the_page_stays() {
    // The app's page is the `scroll` inside the sky canvas (LLP 1014 §1a):
    // the wheel goes there, and the page — the viewport over a document
    // exactly its size — has nothing to take.
    let mut p = boot();
    let before = boxed(&mut p, "station-name").rect.1;
    let reply = p.wheel(view(&p, "station-name"), 0.0, 300.0).unwrap();
    assert!(reply.contains("\"wheel\":[0,300]"), "{reply}");
    assert_eq!(p.page(), (0.0, 0.0), "the page has nothing to scroll");
    assert_eq!(boxed(&mut p, "station-name").rect.1, before - 300.0);
    let inner = |p: &mut Presenter<caltrain_data::Caltrain>| {
        p.boxes()
            .iter()
            .find(|b| b.scroll.is_some())
            .unwrap()
            .scroll
    };
    assert_eq!(inner(&mut p), Some((0.0, 300.0)), "the scroll node took it");
    // Up past the top stops at the top (over the root: the station's name
    // has scrolled off the viewport by now).
    let _ = p.wheel(view(&p, "caltrain-main"), 0.0, -1000.0);
    assert_eq!(inner(&mut p), Some((0.0, 0.0)));
    assert_eq!(p.page(), (0.0, 0.0));
}

#[test]
fn a_nested_scroll_container_takes_the_wheel_then_chains_to_the_page() {
    let mut p = fixture("scroll");
    let rows = view(&p, "rows");
    let _ = p.wheel(view(&p, "row-1"), 0.0, 100.0).unwrap();
    assert_eq!(
        p.scroll_of(rows),
        (0.0, 100.0),
        "the scroll node took a wheel of 100"
    );
    assert_eq!(p.page(), (0.0, 0.0), "the page did not move");
    for _ in 0..12 {
        let _ = p.wheel(rows, 0.0, 400.0);
    }
    let limit = p.scroll_of(rows).1;
    assert!(limit > 100.0, "kept scrolling");
    assert!(
        p.page().1 > 0.0,
        "at its edge the wheel chained to the page (page at {:?})",
        p.page()
    );
    assert_eq!(p.scroll_of(rows).1, limit, "never past its edge");
    let row0 = boxed(&mut p, "row-0");
    let rows_box = boxed(&mut p, "rows");
    assert_eq!(
        row0.clip,
        Some(rows_box.rect),
        "a child is clipped by its scroll container"
    );
    assert!(
        row0.rect.1 < rows_box.rect.1,
        "row 0 scrolled out of the top"
    );
}

#[test]
fn a_spring_arrives_as_presentation_values_frame_by_frame() {
    let mut p = fixture("spring");
    let hello = view(&p, "hello");
    let before = boxed(&mut p, "hello").rect;
    assert!(!p.host().motion());
    let _ = p.tap(view(&p, "toggle")).unwrap();
    assert!(p.host().motion(), "a spring and an easing started");
    let mut last = 1.0f32;
    let mut rising = 0;
    for i in 1..=30 {
        p.tick(i as f64 * 16.0);
        let s = p.host().presented(hello).scale;
        if s > last {
            rising += 1;
        }
        last = s;
    }
    assert!(
        rising > 5 && last > 1.0,
        "the spring moved toward 1.5: {last}"
    );
    let (landed, error) = p.clock(20_000.0);
    assert_eq!((landed, error), (20_000.0, None));
    assert!(!p.host().motion(), "settled");
    let after = p.host().presented(hello);
    assert_eq!((after.scale, after.opacity), (1.5, 0.5));
    let now = boxed(&mut p, "hello").rect;
    assert!(
        (now.2 - before.2 * 1.5).abs() < 0.5,
        "the box grew by the scale: {before:?} → {now:?}"
    );
    assert!(now.0 < before.0, "about its center");
}

#[test]
fn the_clock_fires_timers_at_their_due_times() {
    let mut p = boot();
    let tree = p.host().agent("{\"op\":\"tree\"}");
    let at = tree.find("\"testId\":\"countdown-").unwrap();
    let id: String = tree[at + 10..].chars().take_while(|c| *c != '"').collect();
    let first: i64 = text(&p, &id).parse().unwrap();
    let (landed, error) = p.clock(60_000.0);
    assert_eq!((landed, error), (60_000.0, None));
    let later: i64 = text(&p, &id).parse().unwrap();
    assert_eq!(later, first - 1, "a minute later the countdown is one less");
    let state = p.host().agent("{\"op\":\"state\"}");
    assert!(state.contains("\"clock\":60000"), "{}", &state[..60]);
}

#[test]
fn an_image_lays_out_from_its_decoded_size() {
    let mut p = boot();
    let logo = boxed(&mut p, "logo");
    assert_eq!(
        (logo.rect.2.round(), logo.rect.3.round()),
        (96.0, 36.0),
        "320×120 at width 96 is 96×36: {logo:?}"
    );
    assert_eq!(
        p.images().loaded,
        vec![("assets/caltrain.png".to_string(), (320, 120))]
    );
    assert!(p.images().bitmaps.contains_key(&logo.id));
    assert!(
        p.images().resolve("../secret.png").is_none(),
        "never outside the asset root"
    );
    assert!(
        p.images().resolve("https://example.com/a.png").is_none(),
        "no URLs yet"
    );
}

#[test]
fn agent_requests_answer_on_the_wire() {
    let mut p = boot();
    let l = handle(&mut p, "{\"op\":\"layout\"}");
    assert!(
        l.contains("\"viewport\":{\"w\":390,\"h\":844}"),
        "{}",
        &l[..80]
    );
    let id = view(&p, "change-station");
    let t = handle(&mut p, &format!("{{\"op\":\"tap\",\"id\":{id}}}"));
    assert!(t.starts_with(&format!("{{\"tapped\":{id},\"at\":[")), "{t}");
    assert!(has(&p, "stations-screen"));
    let w = handle(
        &mut p,
        &format!("{{\"op\":\"tap\",\"id\":{id},\"wheel\":[0,50]}}"),
    );
    assert!(w.contains("\"wheel\":[0,50]"), "{w}");
    let field = view(&p, "station-search");
    let ty = handle(
        &mut p,
        &format!("{{\"op\":\"type\",\"id\":{field},\"text\":\"Sunny\"}}"),
    );
    assert_eq!(ty, format!("{{\"typed\":{field},\"value\":\"Sunny\"}}"));
    assert_eq!(
        handle(&mut p, "{\"op\":\"clock\",\"to\":1000}"),
        "{\"clock\":1000}"
    );
    assert!(handle(&mut p, "{\"op\":\"clock\",\"to\":500}").contains("backwards"));
    assert_eq!(
        handle(&mut p, "{\"op\":\"clock\",\"settle\":true}"),
        "{\"clock\":1000,\"settled\":true}"
    );
    let path = std::env::temp_dir().join(format!("exact-linux-{}.png", std::process::id()));
    let shot = handle(
        &mut p,
        &format!("{{\"op\":\"screenshot\",\"path\":{:?}}}", path.display()),
    );
    assert!(shot.contains("\"w\":390,\"h\":844"), "{shot}");
    let png = tiny_skia::Pixmap::load_png(&path).unwrap();
    assert_eq!((png.width(), png.height()), (390, 844));
    let _ = std::fs::remove_file(path);
    let tree = handle(&mut p, "{\"op\":\"tree\"}");
    assert!(tree.contains("\"incarnation\":"));
    assert!(handle(&mut p, "{\"op\":\"nope\"}").contains("unknown op"));
}

#[test]
fn a_reload_carries_state_and_starts_the_pictures_over() {
    let mut p = boot();
    let _ = p.tap(view(&p, "change-station"));
    let _ = p.wheel(view(&p, "station-search"), 0.0, 50.0);
    let plan = caltrain::build().unwrap().encode();
    let error = p.reload(&plan, caltrain_data::Caltrain).unwrap();
    assert!(error.is_none(), "{error:?}");
    assert!(has(&p, "stations-screen"), "the screen slot carried");
    assert_eq!(p.page(), (0.0, 0.0), "scroll does not survive a restart");
    p.wait_images(Duration::from_secs(2));
    assert_eq!(
        boxed(&mut p, "logo").rect.3.round(),
        36.0,
        "the picture loaded again"
    );
}
