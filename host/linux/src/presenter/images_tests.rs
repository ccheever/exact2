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
        p.image_retry_due_in_ms(),
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
