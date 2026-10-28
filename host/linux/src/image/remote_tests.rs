//! Remote sources against a server on 127.0.0.1: the download's bounds,
//! and a URL through the raster pipeline.
use super::*;
use crate::image::{Assets, Images};
use exact_kernel::{Kernel, NodeType, Op, PropId};
use std::io::{BufRead, BufReader};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;

/// A hang detector, not a speed claim.
const SETTLED: Duration = Duration::from_secs(60);

/// What the server does with a request.
enum Answer {
    /// A 200 with this body and its length.
    Body(Vec<u8>),
    /// A status with no body.
    Status(u16),
    /// A status with no body that says when to ask again (`Retry-After`).
    RetryAfter(u16, String),
    /// A status that declares a body of this length and sends none.
    Declared(u16, u64),
    /// A 302 to this location.
    Redirect(String),
    /// A 200 whose body runs to the connection's close, with no length.
    Unsized(Vec<u8>),
    /// A 200 that declares `len` bytes and sends `sent` of them, says so on
    /// `arrived`, then waits for the client to close, which it reports on
    /// `closed`.
    Stall {
        len: usize,
        sent: usize,
        arrived: mpsc::Sender<()>,
        closed: mpsc::Sender<()>,
    },
    /// Read the request and say nothing.
    Silent,
}

/// Serve each request on its own thread, by `answer(path, headers)` (the
/// header lines, lowercased); the origin.
fn serve(answer: impl Fn(&str, &str) -> Answer + Send + Sync + 'static) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let answer = Arc::new(answer);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let answer = answer.clone();
            std::thread::spawn(move || respond(stream, &*answer));
        }
    });
    origin
}

fn respond(mut stream: TcpStream, answer: &dyn Fn(&str, &str) -> Answer) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
    let mut headers = String::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 || header == "\r\n" {
            break;
        }
        headers.push_str(&header.to_ascii_lowercase());
    }
    let head = |status: &str, extra: &str| {
        format!("HTTP/1.1 {status}\r\nConnection: close\r\n{extra}\r\n").into_bytes()
    };
    let _ = match answer(&path, &headers) {
        Answer::Body(body) => stream
            .write_all(&head(
                "200 OK",
                &format!("Content-Length: {}\r\n", body.len()),
            ))
            .and_then(|()| stream.write_all(&body)),
        Answer::Status(status) => {
            stream.write_all(&head(&format!("{status} Nope"), "Content-Length: 0\r\n"))
        }
        Answer::RetryAfter(status, after) => stream.write_all(&head(
            &format!("{status} Nope"),
            &format!("Retry-After: {after}\r\nContent-Length: 0\r\n"),
        )),
        Answer::Declared(status, len) => stream.write_all(&head(
            &format!("{status} Nope"),
            &format!("Content-Length: {len}\r\n"),
        )),
        Answer::Redirect(to) => stream.write_all(&head(
            "302 Found",
            &format!("Location: {to}\r\nContent-Length: 0\r\n"),
        )),
        Answer::Unsized(body) => stream
            .write_all(&head("200 OK", ""))
            .and_then(|()| stream.write_all(&body)),
        Answer::Stall {
            len,
            sent,
            arrived,
            closed,
        } => {
            let _ = stream
                .write_all(&head("200 OK", &format!("Content-Length: {len}\r\n")))
                .and_then(|()| stream.write_all(&vec![0; sent]));
            let _ = arrived.send(());
            // The client's close (or reset) ends the wait.
            let mut rest = Vec::new();
            let _ = std::io::Read::read_to_end(&mut reader, &mut rest);
            let _ = closed.send(());
            Ok(())
        }
        Answer::Silent => {
            std::thread::sleep(Duration::from_secs(30));
            Ok(())
        }
    };
}

fn png(w: u32, h: u32, color: [u8; 4]) -> Vec<u8> {
    let mut data = Vec::new();
    let mut encoder = png::Encoder::new(&mut data, w, h);
    encoder.set_color(png::ColorType::Rgba);
    let mut writer = encoder.write_header().unwrap();
    let pixels: Vec<u8> = (0..w * h).flat_map(|_| color).collect();
    writer.write_image_data(&pixels).unwrap();
    writer.finish().unwrap();
    data
}

/// A stall's two signals, shared by every request the server answers.
struct Stalls {
    arrived: Mutex<mpsc::Sender<()>>,
    closed: Mutex<mpsc::Sender<()>>,
}
fn stalls() -> (Arc<Stalls>, mpsc::Receiver<()>, mpsc::Receiver<()>) {
    let (arrived, arrivals) = mpsc::channel();
    let (closed, closes) = mpsc::channel();
    let stalls = Stalls {
        arrived: Mutex::new(arrived),
        closed: Mutex::new(closed),
    };
    (Arc::new(stalls), arrivals, closes)
}
impl Stalls {
    fn answer(&self, len: usize, sent: usize) -> Answer {
        Answer::Stall {
            len,
            sent,
            arrived: self.arrived.lock().unwrap().clone(),
            closed: self.closed.lock().unwrap().clone(),
        }
    }
}

/// Load `url` and wait for its outcome.
fn fetched(downloads: &Arc<Downloads>, url: &str) -> Outcome {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let load = downloads
        .load(url, false, move || {
            let _ = tx.lock().unwrap().send(());
        })
        .unwrap();
    if !load.settled() {
        rx.recv_timeout(SETTLED).expect("the download settles");
    }
    load.outcome().expect("settled")
}

fn bytes(body: &Body) -> Vec<u8> {
    std::fs::read(&body.path).unwrap()
}

/// A small cap and cache, and bounds no loaded machine reaches.
fn bounded() -> Arc<Downloads> {
    Downloads::new(Policy {
        limit: 4096,
        cache: 8192,
        ..Policy::default()
    })
}

#[test]
fn a_body_and_a_redirect_load_and_the_cache_answers_again() {
    let picture = png(4, 3, [10, 20, 30, 255]);
    let body = picture.clone();
    let origin = serve(move |path, _| match path {
        "/a.png" => Answer::Body(body.clone()),
        "/moved" => Answer::Redirect("/a.png".into()),
        _ => Answer::Status(404),
    });
    let downloads = bounded();
    let url = format!("{origin}/a.png");
    let first = fetched(&downloads, &url).unwrap();
    assert_eq!(bytes(&first), picture);
    assert_eq!(downloads.fetches.load(Ordering::Relaxed), 1);
    // Asked again, the body is the cache's: settled at once, not fetched.
    let again = downloads.load(&url, false, || {}).unwrap();
    assert!(again.settled());
    assert_eq!(downloads.fetches.load(Ordering::Relaxed), 1);
    assert_eq!(downloads.cached_bytes(), (picture.len() + url.len()) as u64);
    // A redirect is followed (to the same body, under another URL).
    let moved = fetched(&downloads, &format!("{origin}/moved")).unwrap();
    assert_eq!(bytes(&moved), picture);
    assert_eq!(downloads.fetches.load(Ordering::Relaxed), 3);
}

#[test]
fn a_spool_file_goes_with_its_last_holder() {
    let origin = serve(|_, _| Answer::Body(png(2, 2, [1, 2, 3, 255])));
    let downloads = Downloads::new(Policy {
        cache: 0,
        ..Policy::default()
    });
    let body = fetched(&downloads, &format!("{origin}/a.png")).unwrap();
    let path = body.path.clone();
    assert!(path.exists());
    drop(body);
    // The download thread may hold the load a moment longer.
    let started = Instant::now();
    while path.exists() {
        assert!(started.elapsed() < SETTLED, "the spool file is removed");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_sweep_takes_the_directories_no_process_holds() {
    let root = std::env::temp_dir().join(format!("exact-sweep-test-{}", spool_name().unwrap()));
    let (gone, live) = (root.join("gone"), root.join("live"));
    for dir in [&gone, &live] {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("body"), b"x").unwrap();
    }
    lock_file(&gone.join(".lock")).unwrap();
    let held = lock_file(&live.join(".lock")).unwrap();
    assert!(lock(&held));
    sweep(&root);
    assert!(!gone.exists());
    assert!(live.join("body").exists());
    drop(held);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn credentials_in_a_url_or_a_redirect_are_not_sent() {
    let picture = png(2, 2, [9, 9, 9, 255]);
    let (seen, heard) = mpsc::channel();
    let seen = Mutex::new(seen);
    let origin = serve(move |path, headers| {
        let _ = seen.lock().unwrap().send(headers.contains("authorization"));
        match path {
            "/hop" => Answer::Redirect("/a.png".into()),
            _ => Answer::Body(picture.clone()),
        }
    });
    let downloads = bounded();
    let with_user = origin.replacen("http://", "http://user:secret@", 1);
    fetched(&downloads, &format!("{with_user}/a.png")).unwrap();
    assert!(!heard.recv().unwrap());
    // A redirect's Location may name a user too.
    let origin = serve(move |path, headers| {
        assert!(!headers.contains("authorization"), "{path}");
        match path {
            "/hop" => Answer::Redirect(format!("{with_user}/b.png")),
            _ => Answer::Body(png(2, 2, [9, 9, 9, 255])),
        }
    });
    fetched(&downloads, &format!("{origin}/hop")).unwrap();
    assert!(!heard.recv().unwrap());
}

#[test]
fn a_status_other_than_2xx_is_its_error_whatever_its_length() {
    let origin = serve(|path, _| match path {
        "/huge" => Answer::Declared(404, 1 << 40),
        _ => Answer::Status(404),
    });
    let downloads = bounded();
    for path in ["/gone.png", "/huge"] {
        let failure = fetched(&downloads, &format!("{origin}{path}")).unwrap_err();
        assert_eq!(failure.message, "HTTP 404", "{path}");
        assert_eq!(failure.refusal, Refusal::DecodeFailed, "{path}");
    }
}

#[test]
fn a_body_over_the_cap_is_refused_by_its_length_and_as_it_streams() {
    let origin = serve(|path, _| match path {
        "/declared" => Answer::Body(vec![7; 5000]),
        _ => Answer::Unsized(vec![7; 5000]),
    });
    let downloads = bounded();
    for path in ["/declared", "/streamed"] {
        let failure = fetched(&downloads, &format!("{origin}{path}")).unwrap_err();
        assert_eq!(failure.refusal, Refusal::EncodedLimit, "{path}");
        assert_eq!(failure.message, "over 4096 encoded bytes", "{path}");
    }
    assert_eq!(downloads.cached_bytes(), 0);
}

#[test]
fn a_silent_server_and_a_stalled_body_time_out() {
    let (stalls, _arrivals, _closes) = stalls();
    let origin = serve(move |path, _| match path {
        "/silent" => Answer::Silent,
        _ => stalls.answer(1000, 10),
    });
    let downloads = Downloads::new(Policy {
        request: Duration::from_millis(300),
        whole: Duration::from_secs(2),
        ..Policy::default()
    });
    for path in ["/silent", "/stalled"] {
        let started = Instant::now();
        let failure = fetched(&downloads, &format!("{origin}{path}")).unwrap_err();
        assert_eq!(failure.message, "The request timed out.", "{path}");
        // The idle bound or the whole one, not the server's 30 s.
        assert!(started.elapsed() < Duration::from_secs(10), "{path}");
    }
}

#[test]
fn a_dropped_load_closes_its_connection() {
    let (stalls, arrivals, closes) = stalls();
    let origin = serve(move |_, _| stalls.answer(100_000, 100));
    let downloads = Downloads::new(Policy::default());
    let load = downloads
        .load(&format!("{origin}/slow"), false, || {})
        .unwrap();
    arrivals
        .recv_timeout(SETTLED)
        .expect("the request reaches the server");
    assert!(!load.settled());
    drop(load);
    closes
        .recv_timeout(SETTLED)
        .expect("the abort shuts the socket");
}

fn kernel(source: &str) -> Kernel {
    let mut k = Kernel::with_monospace();
    k.apply(
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
                value: source.into(),
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k
}

fn set(k: &mut Kernel, source: &str) {
    let epoch = k.epoch();
    k.apply(
        0,
        epoch + 1,
        &[Op::SetProp {
            id: 1,
            prop: PropId::ImageSource,
            value: source.into(),
        }],
    )
    .unwrap();
}

fn until(images: &mut Images, mut test: impl FnMut(&Images) -> bool) -> Vec<crate::image::Report> {
    let started = Instant::now();
    let mut reports = Vec::new();
    while !test(images) {
        assert!(started.elapsed() < SETTLED, "bounded download progress");
        reports.extend(images.poll());
        std::thread::sleep(Duration::from_millis(5));
    }
    reports
}

fn shown(images: &Images) -> Option<(String, (u32, u32))> {
    images.loaded.first().cloned()
}

fn images() -> Images {
    Images::with_assets(Assets::embedded(std::path::PathBuf::new()))
}

#[test]
fn a_url_is_decoded_and_a_failed_one_keeps_what_was_shown() {
    let origin = serve(|path, _| match path {
        "/a.png" => Answer::Body(png(6, 4, [200, 0, 0, 255])),
        "/text" => Answer::Body(b"not a picture".to_vec()),
        _ => Answer::Status(404),
    });
    let url = format!("{origin}/a.png");
    let mut k = kernel(&url);
    let mut images = images();
    let mut reports = images.sync(&k, &k.roots());
    reports.extend(images.wait(SETTLED));
    assert_eq!(reports, vec![(1, Some((6., 4.)))]);
    assert_eq!(shown(&images), Some((url, (6, 4))));

    // A source that fails keeps the picture before it, as Apple's loader
    // does; a first source that fails shows nothing.
    for path in ["/missing.png", "/text"] {
        set(&mut k, &format!("{origin}{path}"));
        let mut reports = images.sync(&k, &k.roots());
        reports.extend(until(&mut images, |images| {
            images.views[&1].refusal.is_some()
        }));
        assert!(reports.is_empty(), "{path}: {reports:?}");
        assert_eq!(images.views[&1].refusal, Some(Refusal::DecodeFailed));
        assert!(images.bitmaps.contains_key(&1), "{path}");
        assert!(!images.pending(), "{path}");
        let mut first = Images::with_assets(Assets::embedded(std::path::PathBuf::new()));
        let k = kernel(&format!("{origin}{path}"));
        first.sync(&k, &k.roots());
        first.wait(SETTLED);
        assert!(first.bitmaps.is_empty(), "{path}");
        assert_eq!(first.views[&1].refusal, Some(Refusal::DecodeFailed));
    }
    // A body this host cannot decode is not kept, so it is fetched again
    // when next asked for; a picture is.
    let downloads = &images.backend.downloads;
    assert!(!downloads.holds(&format!("{origin}/text")));
    assert!(downloads.holds(&format!("{origin}/a.png")));
}

#[test]
fn a_source_changed_mid_load_keeps_the_old_picture_until_the_new_one_lands() {
    let (stalls, arrivals, closes) = stalls();
    let origin = serve(move |path, _| match path {
        "/first.png" => Answer::Body(png(3, 3, [0, 0, 200, 255])),
        "/slow.png" => stalls.answer(100_000, 100),
        _ => Answer::Body(png(5, 2, [0, 200, 0, 255])),
    });
    let first = format!("{origin}/first.png");
    let mut k = kernel(&first);
    let mut images = images();
    images.sync(&k, &k.roots());
    images.wait(SETTLED);
    assert_eq!(shown(&images), Some((first.clone(), (3, 3))));

    // While the next source downloads, the old picture stays.
    set(&mut k, &format!("{origin}/slow.png"));
    images.sync(&k, &k.roots());
    arrivals
        .recv_timeout(SETTLED)
        .expect("the request reaches the server");
    images.poll();
    assert_eq!(shown(&images), Some((first, (3, 3))));
    assert!(images.pending());

    // Changed again mid-download: that download is abandoned (its socket
    // shut) and the latest source is the one that lands.
    let last = format!("{origin}/last.png");
    set(&mut k, &last);
    images.sync(&k, &k.roots());
    closes
        .recv_timeout(SETTLED)
        .expect("the abandoned download's connection closes");
    until(&mut images, |images| {
        shown(images).is_some_and(|(source, _)| source == last)
    });
    assert_eq!(shown(&images), Some((last, (5, 2))));
}

#[test]
fn a_failure_says_whether_asking_again_may_mend_it() {
    let origin = serve(|path, _| match path {
        "/busy" => Answer::Status(503),
        "/slow-down" => Answer::Status(429),
        "/big" => Answer::Body(vec![7; 5000]),
        "/silent" => Answer::Silent,
        _ => Answer::Status(404),
    });
    let downloads = Downloads::new(Policy {
        request: Duration::from_millis(300),
        whole: Duration::from_secs(2),
        limit: 4096,
        ..Policy::default()
    });
    for (path, transient) in [
        ("/busy", true),
        ("/slow-down", true),
        ("/silent", true),
        ("/gone", false),
        ("/big", false),
    ] {
        let failure = fetched(&downloads, &format!("{origin}{path}")).unwrap_err();
        assert_eq!(failure.transient, transient, "{path}: {}", failure.message);
    }
    let failure = fetched(&downloads, "ftp://example.invalid/a.png").unwrap_err();
    assert!(!failure.transient, "{}", failure.message);
}

#[test]
fn a_picture_that_fills_the_screen_is_downloaded_ahead_of_those_waiting() {
    let (stalls, arrivals, _closes) = stalls();
    let origin = serve(move |_, _| stalls.answer(100_000, 100));
    let downloads = Downloads::new(Policy::default());
    // Every download thread busy.
    let busy: Vec<_> = (0..THREADS)
        .map(|n| {
            downloads
                .load(&format!("{origin}/busy{n}"), false, || {})
                .unwrap()
        })
        .collect();
    for _ in 0..THREADS {
        arrivals
            .recv_timeout(SETTLED)
            .expect("each thread takes one");
    }
    let a = downloads
        .load(&format!("{origin}/a"), false, || {})
        .unwrap();
    let b = downloads
        .load(&format!("{origin}/b"), false, || {})
        .unwrap();
    let viewer = downloads
        .load(&format!("{origin}/viewer"), true, || {})
        .unwrap();
    let c = downloads
        .load(&format!("{origin}/c"), false, || {})
        .unwrap();
    let order: Vec<String> = downloads
        .queue
        .lock()
        .unwrap()
        .iter()
        .map(|j| j.url.rsplit('/').next().unwrap().to_owned())
        .collect();
    assert_eq!(
        order,
        ["viewer", "a", "b", "c"],
        "the viewer's first, the others in their order"
    );
    drop((busy, a, b, viewer, c));
}

#[test]
fn a_picture_on_screen_asks_again_for_a_failed_download_and_shows_it() {
    let asked = Arc::new(AtomicUsize::new(0));
    let count = asked.clone();
    let origin = serve(move |path, _| match path {
        // Busy for the first two asks, then the picture.
        "/flaky.png" if count.fetch_add(1, Ordering::SeqCst) < 2 => Answer::Status(503),
        "/flaky.png" => Answer::Body(png(7, 5, [0, 120, 0, 255])),
        "/dead.png" => Answer::Status(503),
        _ => Answer::Status(404),
    });
    let url = format!("{origin}/flaky.png");
    let k = kernel(&url);
    let mut images = images();
    images.sync(&k, &k.roots());
    let started = Instant::now();
    while shown(&images).is_none() {
        assert!(
            started.elapsed() < SETTLED,
            "the picture is asked again until it shows"
        );
        images.poll();
        images.sync(&k, &k.roots());
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(shown(&images), Some((url, (7, 5))));
    assert_eq!(asked.load(Ordering::SeqCst), 3);

    // A URL that never mends is asked once and then six times more a showing, and not while it is off screen.
    let dead = format!("{origin}/dead.png");
    let k = kernel(&dead);
    let mut images = self::images();
    images.sync(&k, &k.roots());
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(3) {
        images.poll();
        images.sync(&k, &k.roots());
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        images.views[&1].retries,
        crate::image::RETRY_AFTER.len(),
        "then it waits for the next showing"
    );
    assert_eq!(images.retry_due_in(), None);
    // Off screen: nothing more is asked; shown again, it starts over.
    images.sync_visible(&k, &k.roots(), 1., |_| false);
    assert_eq!(images.views[&1].retries, 0);
    assert_eq!(images.retry_due_in(), None);
}

/// Two image nodes of one source.
fn kernel2(source: &str) -> Kernel {
    let mut k = Kernel::with_monospace();
    let mut ops = Vec::new();
    for id in [1, 2] {
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::Image,
        });
        ops.push(Op::SetProp {
            id,
            prop: PropId::ImageSource,
            value: source.into(),
        });
        ops.push(Op::AttachRoot { id });
    }
    k.apply(0, 1, &ops).unwrap();
    k
}

#[test]
fn a_view_of_the_same_url_joins_another_views_retry_even_when_it_was_off_screen() {
    let asked = Arc::new(AtomicUsize::new(0));
    let count = asked.clone();
    let origin = serve(move |_, _| {
        if count.fetch_add(1, Ordering::SeqCst) == 0 {
            Answer::Status(503)
        } else {
            Answer::Body(png(4, 4, [9, 9, 200, 255]))
        }
    });
    let url = format!("{origin}/shared.png");
    let k = kernel2(&url);
    let mut images = images();
    images.sync(&k, &k.roots());
    until(&mut images, |images| images.views[&1].refusal.is_some());
    // Only the first is on screen while it asks again and lands.
    let started = Instant::now();
    while !images.bitmaps.contains_key(&1) {
        assert!(started.elapsed() < SETTLED, "the first shows");
        images.poll();
        images.sync_visible(&k, &k.roots(), 1., |id| id == 1);
        std::thread::sleep(Duration::from_millis(5));
    }
    // The second, shown again, joins that download rather than keeping the failed one.
    let started = Instant::now();
    while !images.bitmaps.contains_key(&2) {
        assert!(started.elapsed() < SETTLED, "the second shows too");
        images.poll();
        images.sync(&k, &k.roots());
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(asked.load(Ordering::SeqCst), 2, "one retry served both");
}

#[test]
fn a_retry_waits_only_for_a_remote_picture_on_screen() {
    let origin = serve(|_, _| Answer::Status(503));
    let url = format!("{origin}/busy.png");
    let mut k = kernel(&url);
    let mut images = images();
    images.sync(&k, &k.roots());
    // On screen: a retry comes due (polled, as a host's turns poll).
    until(&mut images, |images| images.retry_due_in().is_some());
    // Its source becomes a symbol before the retry: nothing is due any more (no turn owed forever).
    set(&mut k, "symbol:close");
    images.sync(&k, &k.roots());
    assert_eq!(images.retry_due_in(), None);
    // Asked for at mount but not in the viewport: no retry.
    set(&mut k, &format!("{origin}/other.png"));
    images.sync_shown(&k, &k.roots(), 1., &|_| true, &|_| false);
    until(&mut images, |images| images.views[&1].refusal.is_some());
    images.sync_shown(&k, &k.roots(), 1., &|_| true, &|_| false);
    assert_eq!(images.retry_due_in(), None);
    assert_eq!(images.views[&1].retries, 0);
}

#[test]
fn a_waiting_download_moves_up_when_its_picture_comes_to_fill_the_screen() {
    let (stalls, arrivals, _closes) = stalls();
    let origin = serve(move |_, _| stalls.answer(100_000, 100));
    let downloads = Downloads::new(Policy::default());
    let busy: Vec<_> = (0..THREADS)
        .map(|n| {
            downloads
                .load(&format!("{origin}/busy{n}"), false, || {})
                .unwrap()
        })
        .collect();
    for _ in 0..THREADS {
        arrivals
            .recv_timeout(SETTLED)
            .expect("each thread takes one");
    }
    let u = downloads.load(&format!("{origin}/u"), true, || {}).unwrap();
    let a = downloads
        .load(&format!("{origin}/a"), false, || {})
        .unwrap();
    let b = downloads
        .load(&format!("{origin}/b"), false, || {})
        .unwrap();
    let ordinary = b.whole();
    assert!(downloads.promote(&b));
    assert!(!downloads.promote(&b), "once");
    let v = downloads.load(&format!("{origin}/v"), true, || {}).unwrap();
    let order: Vec<String> = downloads
        .queue
        .lock()
        .unwrap()
        .iter()
        .map(|j| j.url.rsplit('/').next().unwrap().to_owned())
        .collect();
    // Behind the urgent one already waiting: urgent ones keep the order they became so.
    assert_eq!(order, ["u", "b", "v", "a"]);
    // Its bound is now the urgent one, counted from its ask.
    assert_eq!(b.whole(), b.asked + Policy::default().urgent);
    assert!(b.whole() > ordinary);
    drop((busy, u, a, b, v));
}

#[test]
fn urgent_downloads_never_starve_the_others() {
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let log = seen.clone();
    let origin = serve(move |path, _| {
        log.lock()
            .unwrap()
            .push(path.trim_start_matches('/').to_owned());
        Answer::Silent
    });
    let downloads = Downloads::new(Policy::default());
    let mut busy: VecDeque<_> = (0..THREADS)
        .map(|n| {
            downloads
                .load(&format!("{origin}/busy{n}"), false, || {})
                .unwrap()
        })
        .collect();
    let wait_for = |n: usize| {
        let started = Instant::now();
        while seen.lock().unwrap().len() < n {
            assert!(started.elapsed() < SETTLED, "{n} requests");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    wait_for(THREADS);
    let held: Vec<_> = [("o1", false), ("u1", true), ("u2", true), ("u3", true)]
        .into_iter()
        .map(|(name, urgent)| {
            downloads
                .load(&format!("{origin}/{name}"), urgent, || {})
                .unwrap()
        })
        .collect();
    // One thread freed at a time: two urgent ones in the order asked, then
    // the ordinary one waiting, then the last urgent one.
    for n in 1..=4 {
        drop(busy.pop_front());
        wait_for(THREADS + n);
    }
    assert_eq!(seen.lock().unwrap()[THREADS..], ["u1", "u2", "o1", "u3"]);
    drop((busy, held));
}

#[test]
fn a_view_that_joined_a_retry_never_cancels_it_with_its_own_older_deadline() {
    let asked = Arc::new(AtomicUsize::new(0));
    let count = asked.clone();
    let origin = serve(move |_, _| match count.fetch_add(1, Ordering::SeqCst) {
        0 => Answer::Status(503),
        // The retry lands slowly, past the second view's own deadline.
        _ => {
            std::thread::sleep(Duration::from_millis(400));
            Answer::Body(png(3, 3, [1, 2, 3, 255]))
        }
    });
    let url = format!("{origin}/shared.png");
    let k = kernel2(&url);
    let mut images = images();
    images.sync(&k, &k.roots());
    // The second view's backoff is further along (it showed earlier): its deadline comes later than the first's.
    images.views.get_mut(&2).unwrap().retries = 3;
    let started = Instant::now();
    while !(images.bitmaps.contains_key(&1) && images.bitmaps.contains_key(&2)) {
        assert!(started.elapsed() < SETTLED, "both show");
        images.poll();
        images.sync(&k, &k.roots());
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        asked.load(Ordering::SeqCst),
        2,
        "the first view's retry served both; the second's deadline cancelled nothing"
    );
}

#[test]
fn a_retry_after_is_read_as_seconds_or_a_date() {
    use super::retry_after;
    use std::time::SystemTime;
    // 1994-11-06 08:49:37 UTC.
    let at = SystemTime::UNIX_EPOCH + Duration::from_secs(784_111_777);
    assert_eq!(retry_after(" 7 ", at), Some(Duration::from_secs(7)));
    assert_eq!(retry_after("300", at), Some(Duration::from_secs(300)));
    assert_eq!(retry_after("600", at), Some(Duration::from_secs(600)));
    let date = "Sun, 06 Nov 1994 08:50:07 GMT";
    assert_eq!(retry_after(date, at), Some(Duration::from_secs(30)));
    let past = "Sun, 06 Nov 1994 08:49:00 GMT";
    assert_eq!(retry_after(past, at), Some(Duration::ZERO));
    let later = "Sun, 06 Nov 1994 09:49:37 GMT";
    assert_eq!(retry_after(later, at), Some(Duration::from_secs(3600)));
    for unread in [
        "",
        "-1",
        "1.5",
        "soon",
        "Sun, 06 Nov 1994 08:49:37 PST",
        "Sun, 06 Foo 1994 08:49:37 GMT",
        "Sun, 06 Nov 99999999999999 08:49:37 GMT",
        "Sun, 06 Nov 1994 -1:49:37 GMT",
        "99999999999999999999999",
    ] {
        assert_eq!(retry_after(unread, at), None, "{unread:?}");
    }
}

#[test]
fn a_picture_on_screen_is_never_asked_again_sooner_than_its_server_says() {
    let asks = Arc::new(Mutex::new(Vec::<(String, Instant)>::new()));
    let log = asks.clone();
    let origin = serve(move |path, _| {
        let mut asks = log.lock().unwrap();
        let first = !asks.iter().any(|(p, _)| p == path);
        asks.push((path.to_owned(), Instant::now()));
        match path {
            "/later.png" if first => Answer::RetryAfter(429, "1".into()),
            "/later.png" => Answer::Body(png(4, 4, [9, 9, 9, 255])),
            // Longer than a showing waits: not asked again in this one.
            _ => Answer::RetryAfter(503, "600".into()),
        }
    });
    let url = format!("{origin}/later.png");
    let k = kernel(&url);
    let mut images = images();
    images.sync(&k, &k.roots());
    let started = Instant::now();
    while shown(&images).is_none() {
        assert!(started.elapsed() < SETTLED, "it shows after its wait");
        images.poll();
        images.sync(&k, &k.roots());
        std::thread::sleep(Duration::from_millis(5));
    }
    let times: Vec<Instant> = asks.lock().unwrap().iter().map(|(_, at)| *at).collect();
    assert_eq!(times.len(), 2);
    assert!(
        times[1] - times[0] >= Duration::from_secs(1),
        "asked again after {:?}",
        times[1] - times[0]
    );

    let long = format!("{origin}/long.png");
    let k = kernel(&long);
    let mut images = self::images();
    images.sync(&k, &k.roots());
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(800) {
        images.poll();
        images.sync(&k, &k.roots());
        std::thread::sleep(Duration::from_millis(5));
    }
    let long_asks = asks
        .lock()
        .unwrap()
        .iter()
        .filter(|(p, _)| p == "/long.png")
        .count();
    assert_eq!(long_asks, 1);
    assert_eq!(images.retry_due_in(), None);
    assert_eq!(
        images.views[&1].retries,
        crate::image::RETRY_AFTER.len(),
        "no more in this showing, however long it lasts"
    );
}

#[test]
fn a_picture_shown_again_with_its_source_unchanged_sends_no_second_load() {
    let origin = serve(|_, _| Answer::Body(png(6, 6, [5, 5, 5, 255])));
    let url = format!("{origin}/again.png");
    let k = kernel(&url);
    let mut images = images();
    let show = |images: &mut Images| {
        let started = Instant::now();
        while shown(images).is_none() {
            assert!(started.elapsed() < SETTLED, "it shows");
            images.poll();
            images.sync(&k, &k.roots());
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    images.sync(&k, &k.roots());
    show(&mut images);
    let events = std::mem::take(&mut images.load_events);
    assert_eq!(
        events
            .iter()
            .map(|e| (e.id, e.source.as_str()))
            .collect::<Vec<_>>(),
        [(1, url.as_str())]
    );
    // Out of view, its picture let go of; back in view, drawn again from the same source.
    images.sync_visible(&k, &k.roots(), 1., |_| false);
    images.poll();
    assert!(images.bitmaps.is_empty());
    images.sync(&k, &k.roots());
    show(&mut images);
    assert!(images.load_events.is_empty(), "{:?}", images.load_events);
}

#[test]
fn a_url_whose_server_said_when_to_ask_again_is_not_fetched_sooner_by_a_new_source() {
    let asked = Arc::new(AtomicUsize::new(0));
    let count = asked.clone();
    let origin = serve(move |path, _| {
        count.fetch_add(1, Ordering::SeqCst);
        match path {
            "/huge.png" => Answer::RetryAfter(503, "18446744073709551615".into()),
            _ => Answer::RetryAfter(429, "600".into()),
        }
    });
    let downloads = Downloads::new(Policy::default());
    let url = format!("{origin}/wait.png");
    let failure = fetched(&downloads, &url).unwrap_err();
    assert!(failure.transient && failure.after.is_some(), "{failure:?}");
    // A new source of the same URL (its picture mounted again): the same answer, nothing asked.
    let again = fetched(&downloads, &url).unwrap_err();
    assert_eq!(again.after, failure.after);
    assert_eq!(asked.load(Ordering::SeqCst), 1);
    // A wait too long for the clock is no panic: it is never, in effect.
    let huge = fetched(&downloads, &format!("{origin}/huge.png")).unwrap_err();
    assert!(huge.after.unwrap() > Instant::now() + Duration::from_secs(86_400));
}

#[test]
fn an_errors_links_lose_their_queries() {
    use super::without_queries;
    assert_eq!(
        without_queries("a redirect to https://cdn.example/a.png?ex=1&is=2&hm=3 now"),
        "a redirect to https://cdn.example/a.png now"
    );
    assert_eq!(without_queries("ftp://h/p#frag"), "ftp://h/p");
    assert_eq!(without_queries("HTTP 404"), "HTTP 404");
    assert_eq!(without_queries("why? because"), "why? because");
    assert_eq!(
        without_queries("`https://h/a.svg?x=)&hm=secret`: x"),
        "`https://h/a.svg`: x"
    );
    let long = format!("<{}?{}>", "a".repeat(200_000), "b".repeat(200_000));
    assert_eq!(
        without_queries(&long).len(),
        200_002,
        "linear, and the query gone"
    );
    assert_eq!(
        without_queries("@import url( a.css?hm=secret);"),
        "@import url( a.css);"
    );
    assert_eq!(
        super::redacted("url(https://h/a.svg?x=1&hm=secret)", "https://h/a.svg?x=1"),
        "url(https://h/a.svg)"
    );
    assert_eq!(
        without_queries("fill=#fff stroke=#000"),
        "fill=#fff stroke=#000"
    );
    assert_eq!(
        without_queries("@import url(a.css?hm=secret);"),
        "@import url(a.css);"
    );
    let spaced = "https://h/a b.png?hm=secret";
    assert_eq!(
        super::redacted(&format!("image {spaced}: gone"), spaced),
        "image https://h/a b.png gone"
    );
    assert_eq!(
        without_queries("image `/a.svg?hm=1`: x"),
        "image `/a.svg`: x"
    );
    assert_eq!(
        without_queries("image `//cdn.example/a.svg?hm=1`: x"),
        "image `//cdn.example/a.svg`: x"
    );
    assert_eq!(
        without_queries("fill=\"url(https:h/x?hm=s)\""),
        "fill=\"url(https:h/x)\""
    );
    assert_eq!(
        without_queries("fill=\"url(//h/x?hm=s)\""),
        "fill=\"url(//h/x)\""
    );
    assert_eq!(without_queries("HTTP 503: why?"), "HTTP 503: why?");
    assert_eq!(
        without_queries("image `https:cdn.example/a.png?hm=1`: x"),
        "image `https:cdn.example/a.png`: x"
    );
    assert_eq!(
        without_queries("fill=\"url(https://h/x?hm=s)\""),
        "fill=\"url(https://h/x)\""
    );
    assert_eq!(
        without_queries("a redirect to //bad:port/a.png?hm=1#f"),
        "a redirect to //bad:port/a.png"
    );
}
