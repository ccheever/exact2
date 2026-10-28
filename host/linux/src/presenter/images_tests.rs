//! A picture's download retry as the presenter places it: a picture under a
//! `display: none` box (no painted box) is not on screen, so a failed
//! download of it is not asked again until it shows.
use super::*;
use crate::image::LoadEvent;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A server answering 503 to every request, counting them; its origin.
fn busy_server(asked: Arc<AtomicUsize>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            asked.fetch_add(1, Ordering::SeqCst);
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                line.clear();
            }
            let _ = stream
                .write_all(b"HTTP/1.1 503 Busy\r\ncontent-length: 0\r\nconnection: close\r\n\r\n");
        }
    });
    origin
}

#[test]
fn a_hidden_pictures_failed_download_is_asked_again_only_once_it_shows() {
    let asked = Arc::new(AtomicUsize::new(0));
    let origin = busy_server(asked.clone());
    let source = format!(
        r#"component App
  state shown = false
  action reveal
    shown = true
  view
    column width=300 height=300
      button "Show" press=reveal testId="show" width=100 height=40
      box display=(shown ? "flex" : "none") width=200 height=200
        image "{origin}/busy.png" width=200 height=200 testId="picture"
"#
    );
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(&source),
        (),
        (300., 300.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let settle = |p: &mut Presenter<()>, ms: u64| {
        let end = std::time::Instant::now() + Duration::from_millis(ms);
        let mut t = 16.;
        while std::time::Instant::now() < end {
            p.poll_images();
            t += 16.;
            p.advance(t);
            p.frame();
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    // Hidden: asked for once (a first load), it fails, and nothing more is asked or due.
    settle(&mut p, 1500);
    let hidden = asked.load(Ordering::SeqCst);
    assert!(hidden <= 1, "{hidden} asks while hidden");
    assert_eq!(
        p.image_turn_due_in_ms(),
        None,
        "no retry is due for a picture with no painted box"
    );
    // Shown: it is on screen, so its failed download is asked again (within the test's backoff of milliseconds).
    let k = p.host().kernel();
    let show = k.node_by_key(k.find_by_test_id("show")[0]).unwrap().id;
    assert!(p
        .host_mut()
        .dispatch_at(show, exact_runner::Event::Press, 0.)
        .is_none());
    settle(&mut p, 1500);
    assert!(
        asked.load(Ordering::SeqCst) > hidden + 1,
        "asked again once shown: {} after {hidden}",
        asked.load(Ordering::SeqCst)
    );
}

/// A server answering every request with a 400 by 400 PNG, counting them; its origin.
fn picture_server(asked: Arc<AtomicUsize>) -> String {
    let mut body = Vec::new();
    let mut encoder = png::Encoder::new(&mut body, 400, 400);
    encoder.set_color(png::ColorType::Rgba);
    let mut writer = encoder.write_header().unwrap();
    writer
        .write_image_data(&[0, 90, 200, 255].repeat(400 * 400))
        .unwrap();
    writer.finish().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            asked.fetch_add(1, Ordering::SeqCst);
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                line.clear();
            }
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    origin
}

#[test]
fn an_image_hears_load_once_a_source_when_its_first_picture_lands() {
    let asked = Arc::new(AtomicUsize::new(0));
    let origin = picture_server(asked.clone());
    let source = format!(
        r#"component App
  state loads = 0
  state big = false
  state name = "a"
  action loaded
    loads = loads + 1
  action grow
    big = true
  action other
    name = "b"
  action first
    name = "a"
  view
    column width=300 height=300
      button "Grow" press=grow testId="grow" width=100 height=30
      button "Other" press=other testId="other" width=100 height=30
      button "First" press=first testId="first" width=100 height=30
      image "{origin}/" + name + ".png" width=(big ? 200 : 20) height=(big ? 200 : 20) load=loaded testId="picture"
"#
    );
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(&source),
        (),
        (300., 300.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let mut t = 16.;
    let mut settle = |p: &mut Presenter<()>, ms: u64| {
        let end = std::time::Instant::now() + Duration::from_millis(ms);
        while std::time::Instant::now() < end {
            p.poll_images();
            t += 16.;
            p.advance(t);
            p.frame();
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    let loads = |p: &mut Presenter<()>| {
        let state = crate::agent::handle(p, r#"{"op":"state"}"#);
        let state: serde_json::Value = serde_json::from_str(&state).unwrap();
        state["slots"]["loads"].as_f64().unwrap()
    };
    let press = |p: &mut Presenter<()>, test_id: &str| {
        let k = p.host().kernel();
        let id = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id;
        assert!(p
            .host_mut()
            .dispatch_at(id, exact_runner::Event::Press, 0.)
            .is_none());
    };
    settle(&mut p, 800);
    assert_eq!(loads(&mut p), 1., "its first picture sends load");
    // Decoded again for a larger box: the same source, no second load.
    let first = p.images.bitmaps.values().next().unwrap().clone();
    press(&mut p, "grow");
    settle(&mut p, 500);
    let again = p.images.bitmaps.values().next().unwrap().clone();
    assert!(!Arc::ptr_eq(&first, &again), "decoded again");
    assert_eq!(loads(&mut p), 1., "a re-decode is not a second load");
    // Another source: its own load.
    press(&mut p, "other");
    settle(&mut p, 800);
    assert_eq!(loads(&mut p), 2., "a new source's picture sends load");
    // The first source again, its body from the cache: assigned again, so its own load.
    let fetched = asked.load(Ordering::SeqCst);
    press(&mut p, "first");
    settle(&mut p, 800);
    assert_eq!(asked.load(Ordering::SeqCst), fetched, "from the cache");
    assert_eq!(
        loads(&mut p),
        3.,
        "a cached source assigned again sends load"
    );
}

#[test]
fn a_load_waiting_is_sent_only_to_the_node_and_source_it_landed_for_and_owes_a_turn() {
    let source = r#"component App
  state loads = 0
  action loaded
    loads = loads + 1
  view
    column width=300 height=300
      image "pictures/a.png" width=20 height=20 load=loaded testId="picture"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (300., 300.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let k = p.host().kernel();
    let node = k.node_by_key(k.find_by_test_id("picture")[0]).unwrap();
    let (id, key) = (node.id, node.key);
    let packed = u64::from(key.index) | (u64::from(key.generation) << 32);
    let assignment = p.images.assignment(id).unwrap();
    let event = |key: u64, source: &str, assignment: u64| LoadEvent {
        id,
        key,
        source: source.into(),
        assignment,
        error: None,
    };
    let loads = |p: &mut Presenter<()>| {
        let state = crate::agent::handle(p, r#"{"op":"state"}"#);
        let state: serde_json::Value = serde_json::from_str(&state).unwrap();
        state["slots"]["loads"].as_f64().unwrap()
    };
    // Landed for another source (the node's changed since), another node at this id, or another
    // assignment of the source: not sent.
    p.images
        .load_events
        .push(event(packed, "pictures/old.png", assignment));
    p.images
        .load_events
        .push(event(packed + 1, "pictures/a.png", assignment));
    p.images
        .load_events
        .push(event(packed, "pictures/a.png", assignment + 1));
    assert_eq!(
        p.image_turn_due_in_ms(),
        Some(0.0),
        "a waiting load owes a turn at once"
    );
    p.apply_reports(Vec::new());
    assert_eq!(loads(&mut p), 0.);
    assert!(p.images.load_events.is_empty());
    // Its own node and source: sent.
    p.images
        .load_events
        .push(event(packed, "pictures/a.png", assignment));
    p.apply_reports(Vec::new());
    assert_eq!(loads(&mut p), 1.);
}

#[test]
fn a_load_is_not_sent_once_an_earlier_loads_action_changed_its_source() {
    // Two pictures land in one pass; the first's load changes the second's source.
    let source = r#"component App
  state second = "pictures/b.png"
  state loads = 0
  action first
    second = "pictures/c.png"
  action loaded
    loads = loads + 1
  view
    column width=300 height=300
      image "pictures/a.png" width=20 height=20 load=first testId="one"
      image second width=20 height=20 load=loaded testId="two"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (300., 300.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let event = |p: &Presenter<()>, test_id: &str, source: &str| {
        let k = p.host().kernel();
        let node = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap();
        LoadEvent {
            id: node.id,
            key: u64::from(node.key.index) | (u64::from(node.key.generation) << 32),
            source: source.to_owned(),
            assignment: p.images.assignment(node.id).unwrap(),
            error: None,
        }
    };
    let one = event(&p, "one", "pictures/a.png");
    let two = event(&p, "two", "pictures/b.png");
    p.images.load_events.extend([one, two]);
    p.apply_reports(Vec::new());
    let state = crate::agent::handle(&mut p, r#"{"op":"state"}"#);
    let state: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert_eq!(
        state["slots"]["second"], "pictures/c.png",
        "the first's load ran"
    );
    assert_eq!(state["slots"]["loads"], 0., "{state}");
}

#[test]
fn a_load_waiting_is_dropped_when_actions_change_its_source_away_and_back() {
    // Three loads in one pass: the first's action moves the third to another
    // source, the second's moves it back. The third's old load is for an
    // assignment that has gone.
    let source = r#"component App
  state third = "pictures/a.png"
  state loads = 0
  action away
    third = "pictures/b.png"
  action back
    third = "pictures/a.png"
  action loaded
    loads = loads + 1
  view
    column width=300 height=300
      image "pictures/one.png" width=20 height=20 load=away testId="one"
      image "pictures/two.png" width=20 height=20 load=back testId="two"
      image third width=20 height=20 load=loaded testId="three"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (300., 300.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let event = |p: &Presenter<()>, test_id: &str, source: &str| {
        let k = p.host().kernel();
        let node = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap();
        LoadEvent {
            id: node.id,
            key: u64::from(node.key.index) | (u64::from(node.key.generation) << 32),
            source: source.to_owned(),
            assignment: p.images.assignment(node.id).unwrap(),
            error: None,
        }
    };
    let events = [
        event(&p, "one", "pictures/one.png"),
        event(&p, "two", "pictures/two.png"),
        event(&p, "three", "pictures/a.png"),
    ];
    p.images.load_events.extend(events);
    p.apply_reports(Vec::new());
    let state = crate::agent::handle(&mut p, r#"{"op":"state"}"#);
    let state: serde_json::Value = serde_json::from_str(&state).unwrap();
    assert_eq!(state["slots"]["third"], "pictures/a.png", "away and back");
    assert_eq!(state["slots"]["loads"], 0., "{state}");
}

/// What a scripted server answers a path's `n`th request with.
enum Reply {
    Status(u16, &'static str),
    Picture,
    Text,
}

/// A server answering each request as `answer(path, n)` says, counting them; its origin.
fn scripted_server(
    asked: Arc<std::sync::Mutex<Vec<String>>>,
    answer: impl Fn(&str, usize) -> Reply + Send + Sync + 'static,
) -> String {
    let mut picture = Vec::new();
    let mut encoder = png::Encoder::new(&mut picture, 8, 8);
    encoder.set_color(png::ColorType::Rgba);
    let mut writer = encoder.write_header().unwrap();
    writer
        .write_image_data(&[200, 40, 40, 255].repeat(64))
        .unwrap();
    writer.finish().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let answer = Arc::new(answer);
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            let _ = reader.read_line(&mut line);
            let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
            let mut header = String::new();
            while reader.read_line(&mut header).is_ok_and(|n| n > 2) {
                header.clear();
            }
            let n = {
                let mut asked = asked.lock().unwrap();
                let n = asked.iter().filter(|p| **p == path).count();
                asked.push(path.clone());
                n
            };
            let (head, body): (String, Vec<u8>) = match answer(&path, n) {
                Reply::Status(code, extra) => (format!("{code} Nope\r\n{extra}"), Vec::new()),
                Reply::Picture => ("200 OK\r\n".into(), picture.clone()),
                Reply::Text => ("200 OK\r\n".into(), b"hello, not a picture".to_vec()),
            };
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 {head}content-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                )
                .as_bytes(),
            );
            let _ = stream.write_all(&body);
        }
    });
    origin
}

/// An app whose picture hears both events, and can be hidden and given another source.
fn error_app(source: &str) -> Presenter<()> {
    let source = format!(
        r#"component App
  state errors = 0
  state loads = 0
  state message = ""
  state shown = true
  state name = "a"
  action failed(m: string)
    errors = errors + 1
    message = m
  action loaded
    loads = loads + 1
  action toggle
    shown = not shown
  action other
    name = "b"
  view
    column width=300 height=300
      button "Toggle" press=toggle testId="toggle" width=100 height=30
      button "Other" press=other testId="other" width=100 height=30
      box display=(shown ? "flex" : "none") width=200 height=200
        image "{source}" + name + ".png" width=200 height=200 load=loaded error=failed testId="picture"
"#
    );
    let (p, error) = Presenter::boot_with(
        &crate::test_plan(&source),
        (),
        (300., 300.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}
fn run(p: &mut Presenter<()>, ms: u64) {
    let end = std::time::Instant::now() + Duration::from_millis(ms);
    while std::time::Instant::now() < end {
        p.poll_images();
        let now = p.host().now() + 16.;
        p.advance(now);
        p.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn slots(p: &mut Presenter<()>) -> serde_json::Value {
    let state = crate::agent::handle(p, r#"{"op":"state"}"#);
    serde_json::from_str::<serde_json::Value>(&state).unwrap()["slots"].clone()
}
fn press(p: &mut Presenter<()>, test_id: &str) {
    let k = p.host().kernel();
    let id = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id;
    let now = p.host().now();
    assert!(p
        .host_mut()
        .dispatch_at(id, exact_runner::Event::Press, now)
        .is_none());
}

#[test]
fn a_picture_that_cannot_load_hears_error_once_with_its_reason() {
    let asked = Arc::new(std::sync::Mutex::new(Vec::new()));
    let origin = scripted_server(asked.clone(), |path, n| match path {
        p if p.starts_with("/flaky/") && n < 2 => Reply::Status(503, ""),
        p if p.starts_with("/flaky/") => Reply::Picture,
        p if p.starts_with("/dead/") => Reply::Status(503, ""),
        p if p.starts_with("/text/") => Reply::Text,
        p if p.starts_with("/moved/") => {
            Reply::Status(302, "location: ftp://cdn.example/x.png?ex=1&hm=secret\r\n")
        }
        _ => Reply::Status(404, ""),
    });
    // Asking again cannot mend a 404: its error at once, with the reason.
    let mut p = error_app(&format!("{origin}/gone/"));
    run(&mut p, 400);
    let s = slots(&mut p);
    assert_eq!(
        (s["errors"].as_f64(), s["loads"].as_f64()),
        (Some(1.), Some(0.)),
        "{s}"
    );
    assert_eq!(s["message"], "HTTP 404");

    // A busy server that answers in the end: its load, and no error.
    let mut p = error_app(&format!("{origin}/flaky/"));
    run(&mut p, 600);
    let s = slots(&mut p);
    assert_eq!(
        (s["errors"].as_f64(), s["loads"].as_f64()),
        (Some(0.), Some(1.)),
        "{s}"
    );

    // A body this host cannot decode, and a redirect to a link it cannot
    // follow, quoted without its query.
    let mut p = error_app(&format!("{origin}/text/"));
    run(&mut p, 400);
    let s = slots(&mut p);
    assert_eq!(s["errors"].as_f64(), Some(1.), "{s}");
    assert!(
        s["message"]
            .as_str()
            .unwrap()
            .starts_with("not an image this host decodes"),
        "{s}"
    );
    let mut p = error_app(&format!("{origin}/moved/"));
    run(&mut p, 400);
    let s = slots(&mut p);
    assert_eq!(s["message"], "a redirect to ftp://cdn.example/x.png", "{s}");

    // A bundled picture that is not there.
    let mut p = error_app("pictures/");
    run(&mut p, 300);
    assert_eq!(slots(&mut p)["errors"].as_f64(), Some(1.));
}

#[test]
fn a_failure_that_asking_again_may_mend_errs_once_its_showings_retries_are_spent() {
    let asked = Arc::new(std::sync::Mutex::new(Vec::new()));
    let origin = scripted_server(asked.clone(), |path, _| match path {
        p if p.ends_with("/b.png") => Reply::Picture,
        _ => Reply::Status(503, ""),
    });
    let mut p = error_app(&format!("{origin}/dead/"));
    // Some retries in, it leaves the screen: no error, and its count starts over.
    let started = std::time::Instant::now();
    while asked.lock().unwrap().len() < 3 {
        assert!(started.elapsed() < Duration::from_secs(10));
        run(&mut p, 10);
    }
    press(&mut p, "toggle");
    run(&mut p, 300);
    let hidden = asked.lock().unwrap().len();
    run(&mut p, 400);
    assert_eq!(
        asked.lock().unwrap().len(),
        hidden,
        "nothing asked off screen"
    );
    assert_eq!(
        slots(&mut p)["errors"].as_f64(),
        Some(0.),
        "no error off screen"
    );
    // Shown again: a whole ladder, then one error.
    press(&mut p, "toggle");
    run(&mut p, 1500);
    assert_eq!(
        asked.lock().unwrap().len(),
        hidden + 6,
        "{:?}",
        asked.lock().unwrap()
    );
    let s = slots(&mut p);
    assert_eq!(
        (s["errors"].as_f64(), s["loads"].as_f64()),
        (Some(1.), Some(0.)),
        "{s}"
    );
    assert_eq!(s["message"], "HTTP 503");
    // After its error, nothing more is asked for it; another source loads.
    run(&mut p, 300);
    assert_eq!(asked.lock().unwrap().len(), hidden + 6);
    press(&mut p, "other");
    run(&mut p, 400);
    let s = slots(&mut p);
    assert_eq!(
        (s["errors"].as_f64(), s["loads"].as_f64()),
        (Some(1.), Some(1.)),
        "{s}"
    );
}
