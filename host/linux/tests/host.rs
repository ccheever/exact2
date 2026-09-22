//! The Linux host over the Caltrain app, headless: the tree lays out with
//! the kernel's layout and real text; every node has a painted box; the
//! page is a viewport over a document; presses go through hit-testing;
//! typing is one change; wheels chain; motion arrives as presentation
//! values; an image's size lays out; the agent's operations answer on the
//! wire.

use exact_kernel::{Kernel, NodeType, Op, PropId};
use exact_linux::agent::handle;
use exact_linux::image::Images;
use exact_linux::paint::PaintedBox;
use exact_linux::Presenter;
use exact_runner::{DataError, DataSource, Value};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn assets() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain"))
}

/// The pinned font (LLP 1015 §3): the fixture directory and the family
/// name the driver sets, so every number in these tests is the same on a
/// Mac and on a builder. Set once, before the first engine is made; the
/// environment is process-wide and every test wants the same values.
fn pin_font() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::env::set_var(
            "EXACT_FONTS",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/fixtures/fonts/assets"
            ),
        );
        std::env::set_var("EXACT_FONT", "DejaVu Sans");
    });
}

fn boot() -> Presenter<caltrain_data::Caltrain> {
    pin_font();
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

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

#[test]
fn an_analysis_core_entry_refuses_before_constructing_app_data() {
    struct MustNotBoot;
    impl Default for MustNotBoot {
        fn default() -> Self {
            panic!("analysis must never construct the app")
        }
    }
    impl DataSource for MustNotBoot {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            unreachable!()
        }
    }
    let compat = r#"{"inputs":{"store":{"L":"0"}},"embedded":{"analysis":true,"seq":null}}"#;
    assert_eq!(exact_linux::run::<MustNotBoot>(b"EXPL", compat), 1);
}

fn fixture(name: &str) -> Presenter<NoData> {
    pin_font();
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
    // Two lines: DejaVu Bold sets "Mountain View" 201 wide and the button
    // beside it 128, more than the 322 a 390-wide header leaves — on every
    // machine, now that the font is pinned (a Mac's Helvetica fit it on one).
    assert!(name.rect.3 < 70.0, "at most two lines: {name:?}");
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
    let l = p.layout_json(None, false);
    assert!(
        l.starts_with("{\"clock\":0,\"viewport\":{\"w\":390,\"h\":844},\"env\":{\"safe-area-inset-top\":0,\"safe-area-inset-right\":0,\"safe-area-inset-bottom\":0,\"safe-area-inset-left\":0,\"keyboard-inset-height\":0},\"nodes\":["),
        "{}",
        &l[..200]
    );
    assert_eq!(l.matches("\"sx\":").count(), 1, "one scroll container");
    assert!(!l.contains("planDigest"));
    let id = p.host().runner().roots()[0];
    let plain: serde_json::Value = serde_json::from_str(&p.layout_json(Some(id), false)).unwrap();
    let mapped: serde_json::Value = serde_json::from_str(&p.layout_json(Some(id), true)).unwrap();
    assert!(plain["node"].get("planDigest").is_none());
    assert_eq!(
        mapped["node"]["planDigest"],
        contract::plan_digest(&p.host().runner().plan().encode())
    );
    assert_eq!(plain["node"]["site"], mapped["node"]["site"]);
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
        reply.starts_with(&format!("{{\"tapped\":{button},\"at\":[")),
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
fn disabled_controls_refuse_pointer_and_text_input() {
    let source = r#"component Disabled
  state count = 0
  state value = "kept"
  action press writes count
    count = count + 1
  action change(next: string) writes value
    value = next
  view
    column
      button press=press disabled=true testId="disabled-button"
        text "Disabled"
      input value=value change=change disabled=true testId="disabled-input"
"#;
    let plan = contract::compile(source).unwrap();
    let (mut presenter, error) =
        Presenter::boot(&plan.encode(), NoData, (390.0, 844.0), 1.0, assets()).unwrap();
    assert!(error.is_none());
    presenter.tap(view(&presenter, "disabled-button")).unwrap();
    assert_eq!(
        presenter.host().runner().slot("count"),
        Some(&Value::Number(0.0))
    );
    let input = view(&presenter, "disabled-input");
    assert!(presenter.type_text(input, "changed").is_err());
    presenter.key(Some('x'), false, presenter.host().now());
    assert_eq!(presenter.focus(), None);
    assert_eq!(
        presenter.host().runner().slot("value"),
        Some(&Value::str("kept"))
    );
}

#[test]
fn unsupported_emoji_picker_does_not_dispatch_a_fake_selection() {
    let plan = contract::compile(
        r#"component Picker
  state value = "kept"
  action change(next: string) writes value
    value = next
  view
    column
      input emojiPicker=true change=change testId="picker"
      input change=change testId="text"
"#,
    )
    .unwrap();
    let (mut p, error) =
        Presenter::boot(&plan.encode(), NoData, (390.0, 844.0), 1.0, assets()).unwrap();
    assert!(error.is_none());
    let picker = view(&p, "picker");
    assert!(p
        .type_text(picker, "☕️")
        .unwrap_err()
        .contains("not supported"));
    assert_eq!(p.focus(), None);
    assert_eq!(p.host().runner().slot("value"), Some(&Value::str("kept")));
    let input = view(&p, "text");
    p.type_text(input, "☕️").unwrap();
    assert_eq!(p.host().runner().slot("value"), Some(&Value::str("☕️")));
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
fn a_nonregular_image_is_refused_off_the_boot_thread_without_blocking_a_worker() {
    let dir = std::env::temp_dir().join(format!("exact-linux-image-worker-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let fifo = dir.join("blocking.png");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(made.success(), "the blocking image fixture is a FIFO");
    let mut kernel = Kernel::with_monospace();
    kernel
        .apply(
            0,
            1,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::Image,
                },
                Op::SetProp {
                    id: 1,
                    prop: PropId::ImageSource,
                    value: "blocking.png".into(),
                },
                Op::AttachRoot { id: 1 },
            ],
        )
        .unwrap();
    let mut images = Images::new(dir.clone());
    let started = Instant::now();
    let reports = images.sync(&kernel, &kernel.roots());
    let elapsed = started.elapsed();
    assert!(reports.is_empty(), "the worker owns the first image report");
    assert!(
        elapsed < Duration::from_millis(100),
        "image scheduling waited {elapsed:?} for the FIFO reader"
    );
    images.wait(Duration::from_secs(2));
    assert!(!images.pending());
    assert!(images.bitmaps.is_empty());
    assert_eq!(images.diagnostics()["refused"], 1);
    let _ = std::fs::remove_dir_all(fifo.parent().unwrap());
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
    // Every host reply is tagged with the runner's epoch and incarnation
    // (LLP 1035.002 D3); a `clock` reply keeps its own `clock`.
    assert!(
        ty.starts_with(&format!(
            "{{\"typed\":{field},\"value\":\"Sunny\",\"epoch\":"
        )),
        "{ty}"
    );
    assert!(ty.contains(",\"incarnation\":"), "{ty}");
    assert!(ty.contains(",\"clock\":"), "{ty}");
    let c = handle(&mut p, "{\"op\":\"clock\",\"to\":1000}");
    assert!(c.starts_with("{\"clock\":1000,\"epoch\":"), "{c}");
    assert_eq!(c.matches("\"clock\":").count(), 1, "{c}");
    assert!(handle(&mut p, "{\"op\":\"clock\",\"to\":500}").contains("backwards"));
    assert!(handle(&mut p, "{\"op\":\"clock\",\"settle\":true}")
        .starts_with("{\"clock\":1000,\"settled\":true,\"epoch\":"));
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
fn the_agent_tree_declares_iframes_unavailable() {
    let mut p = fixture("iframe");
    let tree = handle(&mut p, "{\"op\":\"tree\"}");
    assert!(
        tree.contains("\"type\":\"WebView\",\"unavailable\":true"),
        "{tree}"
    );
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

/// LLP 1016 D2 on this host: a `send` whose source answers later goes to
/// the executor thread, the reply comes back through `pump`, and the tree
/// shows it — against a loopback server, the real transport underneath.
#[test]
fn a_request_runs_on_the_executor_and_its_reply_commits() {
    use exact_runner::{Answer, Outcome, Request};
    use std::io::{Read as _, Write as _};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().take(2) {
            let Ok(mut s) = stream else { break };
            let mut buf = [0u8; 8192];
            let _ = s.read(&mut buf);
            let body = b"{\"data\":{\"login\":{\"username\":\"ada\"}}}";
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = s.write_all(body);
        }
    });

    struct Later {
        port: u16,
        grants: &'static str,
    }
    impl DataSource for Later {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::Unavailable(source.into()))
        }
        fn answer(
            &mut self,
            _: &mut exact_runner::Store,
            _: &str,
            _: &[Value],
        ) -> Result<Answer, DataError> {
            Ok(Answer::Later(Request::post_json(
                &format!("http://127.0.0.1:{}/graphql", self.port),
                "{\"q\":1}",
            )))
        }
        fn parse(
            &mut self,
            _: &mut exact_runner::Store,
            _: &str,
            _: &[Value],
            outcome: Outcome,
        ) -> Result<exact_runner::Answer, DataError> {
            let ok = matches!(outcome, Outcome::Response(ref r) if r.status == 200);
            let text = match &outcome {
                Outcome::Response(r) => String::from_utf8_lossy(&r.body).into_owned(),
                Outcome::Failed { message, .. } => message.clone(),
                Outcome::Storage(_) => panic!("HTTP request received a storage result"),
                Outcome::Surface(_) => panic!("HTTP request received a surface result"),
            };
            let name = if text.contains("ada") {
                "ada".to_string()
            } else {
                format!("?{text}")
            };
            Ok(exact_runner::Answer::Now(Value::record(vec![
                Value::Bool(ok),
                Value::str(&name),
            ])))
        }
        fn grants(&self) -> &'static str {
            self.grants
        }
    }

    const SRC: &str = r#"
shape Session
  ok: bool
  username: string

component App
  mutation session as shape Session
  derive busy = pending(session)
  action submit writes session
    send session = login()
  view
    column testId="app"
      button press=submit aria-label="Log in" testId="login"
        text "Log in"
      when busy
        text "Logging in…" testId="busy"
      match session
        case some(s)
          text `Signed in as ${s.username}` testId="signed-in"
        case none
          text "Signed out" testId="signed-out"
"#;
    // The grant is an origin, port included (ibex LLP 0067): the loopback
    // server's, for this run only.
    let grants: &'static str =
        Box::leak(format!("net.fetch http://127.0.0.1:{port}\n").into_boxed_str());
    let plan = contract::compile(SRC).unwrap();
    let baked = contract::bake(plan, Later { port, grants }).unwrap();
    let (mut p, error) = Presenter::boot(
        &baked.encode(),
        Later { port, grants },
        (390.0, 844.0),
        1.0,
        assets(),
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let login = view(&p, "login");
    p.tap(login).unwrap();
    assert!(p.pending(), "the request is in flight");
    assert!(has(&p, "busy"));
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while p.pending() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
        if let Some(e) = p.pump(p.host().now()) {
            panic!("pump: {e}");
        }
    }
    assert!(!p.pending(), "the reply came back within ten seconds");
    assert!(!has(&p, "busy"));
    assert_eq!(text(&p, "signed-in"), "Signed in as ada");
}

#[test]
fn a_launch_location_precedes_initializers_and_root_type_navigates_once() {
    // @ref LLP 1038 D5/D8/D11 — first pixel and the Linux agent input path.
    let source = "routes nav\n  home \"/\"\n    post \"/post/:post\"\ncomponent App\n  state first = top(nav).url\n  action follow(location: string) writes nav\n    nav = open(nav, location)\n  view\n    main navigate=follow navigationKey=`${top(nav).id}` navigationBack=\"back\" width=390 height=844\n      each e in stack(nav) key=e.id\n        column navigationKey=`${e.id}`\n          text e.url\n";
    let plan = contract::compile(source).unwrap().encode();
    let (host, error) = exact_linux::Host::boot_at(
        &plan,
        NoData,
        Box::new(exact_kernel::MonospaceMeasurer::default()),
        390.0,
        844.0,
        None,
        None,
        &exact_linux::app::launch_location(["--agent".into(), "post/42".into()]),
    )
    .unwrap();
    assert!(error.is_none());
    assert!(exact_runner::agent::state(host.runner()).contains("\"first\":\"/post/42\""));
    let (mut presenter, error) =
        Presenter::boot(&plan, NoData, (390.0, 844.0), 1.0, assets()).unwrap();
    assert!(error.is_none());
    let root = presenter.host().kernel().roots()[0];
    presenter.type_text(root, "/post/42").unwrap();
    assert!(exact_runner::agent::state(presenter.host().runner()).contains("/post/42"));
    assert_eq!(
        exact_runner::agent::logs(presenter.host().runner(), 0)
            .matches("navigate view")
            .count(),
        1
    );
}

// @ref LLP 1042 §2 — Linux keeps a video box even without a playback executor.
#[test]
fn new_trunk_apps_compile_and_present_on_the_linux_cpu_host() {
    pin_font();
    for app in ["video-player", "typetour"] {
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps")
            .join(app);
        let plan = contract::compile_path(&assets.join("app.contract")).unwrap();
        let (mut presenter, error) = Presenter::boot_with(
            &plan.encode(),
            NoData,
            (390., 844.),
            1.,
            assets,
            exact_linux::presenter::PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{app}: {error:?}");
        assert!(presenter.node_count() > 10, "{app}");
        if app == "video-player" {
            let id = view(&presenter, "video");
            let node = presenter.host().kernel().node(id).unwrap();
            assert_eq!(node.node_type, NodeType::Video);
            assert!(node.frame.width > 0. && node.frame.height > 0.);
        }
        assert!(!presenter.boxes().is_empty(), "{app}");
        assert!(presenter.resize(390., 580.).is_none(), "{app}");
        assert!(!presenter.boxes().is_empty(), "{app}");
    }
}

#[test]
fn closed_popovers_keep_their_tree_but_never_paint_or_intercept_input() {
    pin_font();
    let plan = contract::compile(
        r#"component App
  state ordinary = 0
  state destructive = 0
  action normal writes ordinary
    ordinary = ordinary + 1
  action danger writes destructive
    destructive = destructive + 1
  view
    column
      button "Ordinary" press=normal testId="ordinary" width=200 height=100
      column id="confirmation" popover="auto" position="absolute" top=0 left=0 width=200 height=100
        button "Delete" press=danger testId="danger" width=200 height=50
        input value="hidden" testId="hidden-input"
"#,
    )
    .unwrap();
    let (mut p, error) =
        Presenter::boot(&plan.encode(), NoData, (390., 844.), 1., assets()).unwrap();
    assert!(error.is_none());
    let ordinary = view(&p, "ordinary");
    let danger = view(&p, "danger");
    let input = view(&p, "hidden-input");
    assert!(!p.boxes().iter().any(|b| b.id == danger || b.id == input));
    let detail: serde_json::Value =
        serde_json::from_str(&p.layout_json(Some(danger), false)).unwrap();
    assert_eq!(detail["node"]["visible"]["hidden"], true);
    assert_eq!(detail["node"]["visible"]["inert"], true);
    assert!(p.tap(danger).unwrap_err().contains("hidden or inert"));
    assert!(p
        .type_text(input, "must not arrive")
        .unwrap_err()
        .contains("hidden or inert"));
    assert!(p
        .wheel(danger, 0., 100.)
        .unwrap_err()
        .contains("hidden or inert"));
    p.tap(ordinary).unwrap();
    assert_eq!(p.host().runner().slot("ordinary"), Some(&Value::Number(1.)));
    assert_eq!(
        p.host().runner().slot("destructive"),
        Some(&Value::Number(0.))
    );
    assert!(p.pointer_down(100., 25., 0.).unwrap());
    p.pointer_up(100., 25., 10.).unwrap();
    assert_eq!(p.host().runner().slot("ordinary"), Some(&Value::Number(2.)));
    assert_eq!(
        p.host().runner().slot("destructive"),
        Some(&Value::Number(0.))
    );
}

#[test]
fn popover_invocation_reports_unsupported_without_dispatching_a_partial_action() {
    pin_font();
    let plan = contract::compile(r#"component App
  state count = 0
  action recount writes count
    count = count + 1
  view
    column
      button press=recount popovertarget="menu" testId="invoke" width=200 height=50
        text "Open" testId="label"
      button "No handler" popovertarget="menu" testId="no-handler" width=200 height=50
      button "Disabled" disabled=true press=recount popovertarget="menu" testId="disabled" width=200 height=50
      column id="menu" popover="auto" position="absolute"
        button "Item" press=recount
"#).unwrap();
    let (mut p, error) =
        Presenter::boot(&plan.encode(), NoData, (390., 844.), 1., assets()).unwrap();
    assert!(error.is_none());
    for name in ["invoke", "label", "no-handler"] {
        let id = view(&p, name);
        let answer = handle(&mut p, &format!(r#"{{"op":"tap","id":{id}}}"#));
        assert!(
            answer.contains("Linux does not support popover presentation"),
            "{answer}"
        );
    }
    let (mut host, error) = exact_linux::host::Host::boot(
        &plan.encode(),
        NoData,
        Box::new(exact_kernel::MonospaceMeasurer::default()),
        390.,
        844.,
    )
    .unwrap();
    assert!(error.is_none());
    let key = host.kernel().find_by_test_id("invoke")[0];
    let invoke = host.kernel().node_by_key(key).unwrap().id;
    assert_eq!(
        host.dispatch_at(invoke, exact_runner::Event::Press, 50.)
            .as_deref(),
        Some("Linux does not support popover presentation")
    );
    assert_eq!(host.runner().slot("count"), Some(&Value::Number(0.)));
    assert_eq!(host.now(), 0.);
    p.tap(view(&p, "disabled")).unwrap();
    assert!(p.pointer_down(100., 25., 0.).unwrap());
    p.pointer_up(100., 25., 10.).unwrap();
    assert_eq!(p.host().runner().slot("count"), Some(&Value::Number(0.)));
    assert!(p
        .host()
        .agent(r#"{"op":"logs"}"#)
        .contains("Linux does not support popover presentation"));
}

#[test]
fn ordinary_node_state_is_not_routed_to_a_surface() {
    let mut p = fixture("scroll");
    let id = 1;
    let plain = handle(&mut p, r#"{"op":"state"}"#);
    let targeted = handle(&mut p, &format!(r#"{{"op":"state","id":{id}}}"#));
    assert_eq!(targeted, plain);
}

#[test]
fn autofocus_starts_again_in_a_reloaded_host_generation() {
    pin_font();
    let plan = contract::compile(
        "component Test\n  view\n    button autofocus testId=\"play\"\n      text \"Play\"\n",
    )
    .unwrap()
    .encode();
    let (mut p, error) = Presenter::boot(&plan, NoData, (390., 844.), 1., assets()).unwrap();
    assert!(error.is_none());
    assert_eq!(p.focus(), Some(view(&p, "play")));
    p.reload(&plan, NoData).unwrap();
    assert_eq!(p.focus(), Some(view(&p, "play")));
}

#[test]
fn covered_id_tap_refuses_without_dispatching_the_cover() {
    let source = r#"component Cover
  state count = 0
  action press writes count
    count = count + 1
  view
    box width=100 height=100
      button testId="under" position="absolute" left=0 top=0 width=100 height=100 press=press
        text "Under"
      button testId="cover" position="absolute" left=0 top=0 width=100 height=100 press=press
        text "Cover"
"#;
    let plan = contract::compile(source).unwrap();
    let (mut p, error) =
        Presenter::boot(&plan.encode(), NoData, (100., 100.), 1., assets()).unwrap();
    assert!(error.is_none());
    let id = view(&p, "under");
    assert!(p.tap(id).unwrap_err().contains("covered or not hit"));
    assert_eq!(p.host().runner().slot("count"), Some(&Value::Number(0.)));
    p.tap(view(&p, "cover")).unwrap();
    assert_eq!(p.host().runner().slot("count"), Some(&Value::Number(1.)));
}

#[test]
fn tap_passive_descendant_activates_parent_but_actionable_descendant_refuses() {
    for actionable in [false, true] {
        let handler = if actionable { " press=child" } else { "" };
        let source = format!(
            r#"component Nested
  state count = 0
  action parent writes count
    count = count + 1
  action child writes count
    count = count + 10
  view
    button testId="parent" width=100 height=100 press=parent
      box testId="child" width=100 height=100{handler}
"#
        );
        let plan = contract::compile(&source).unwrap();
        let (mut p, error) =
            Presenter::boot(&plan.encode(), NoData, (100., 100.), 1., assets()).unwrap();
        assert!(error.is_none());
        let result = p.tap(view(&p, "parent"));
        if actionable {
            assert!(result.unwrap_err().contains("activates"));
            assert_eq!(p.host().runner().slot("count"), Some(&Value::Number(0.)));
        } else {
            result.unwrap();
            assert_eq!(p.host().runner().slot("count"), Some(&Value::Number(1.)));
        }
    }
}
