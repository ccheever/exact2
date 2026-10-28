//! A picture's download retry as the presenter places it: a picture under a
//! `display: none` box (no painted box) is not on screen, so a failed
//! download of it is not asked again until it shows.
use super::*;
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
    let loads = |p: &mut Presenter<()>| {
        let state = crate::agent::handle(p, r#"{"op":"state"}"#);
        let state: serde_json::Value = serde_json::from_str(&state).unwrap();
        state["slots"]["loads"].as_f64().unwrap()
    };
    // Landed for another source (the node's changed since), or another node at this id: not sent.
    p.images
        .load_events
        .push((id, packed, "pictures/old.png".into()));
    p.images
        .load_events
        .push((id, packed + 1, "pictures/a.png".into()));
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
        .push((id, packed, "pictures/a.png".into()));
    p.apply_reports(Vec::new());
    assert_eq!(loads(&mut p), 1.);
}
