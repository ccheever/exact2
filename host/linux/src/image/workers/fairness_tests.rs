//! Deterministic turns of the real worker loop, without process-pool competitors.
use super::*;
use exact_raster::{Demand, Priority, RasterKey, ViewKey};
use std::sync::atomic::{AtomicUsize, Ordering};

type Events = Arc<Mutex<Vec<&'static str>>>;

fn workers() -> Workers {
    Workers {
        gate: Gate::new(),
        backends: Mutex::new(Vec::new()),
        cursor: Mutex::new(0),
    }
}
fn pixels() -> Arc<[u8]> {
    let mut pixmap = tiny_skia::Pixmap::new(3, 3).unwrap();
    pixmap.fill(tiny_skia::Color::WHITE);
    pixmap.encode_png().unwrap().into()
}
struct Known {
    backend: Arc<Backend>,
    _assets: Arc<Assets>,
    _sources: Vec<Arc<SourceOwner>>,
    demands: Vec<Demand>,
}
impl Known {
    fn new(workers: &Workers, events: &Events, decoded: &Arc<AtomicUsize>) -> Self {
        let backend = Backend::for_workers(workers);
        let generation = backend.generation();
        let bytes = pixels();
        let assets = Arc::new(Assets::selected(
            Default::default(),
            Arc::new(move |_| Ok(Some(bytes.clone()))),
        ));
        let mut sources = Vec::new();
        let mut demands = Vec::new();
        for id in 1..=2 {
            let source = backend
                .source(generation, &id.to_string(), &assets)
                .unwrap();
            assert!(backend.prepare());
            let Prepared::Ready(header) = backend.prepared(source.id).unwrap() else {
                panic!("fixture header")
            };
            let plan = png_decode::DecodePlan::new(header, (3, 3)).unwrap();
            demands.push(Demand {
                key: RasterKey {
                    source: source.id,
                    generation,
                    pixels: plan.pixels,
                    variant: 1,
                    crop: exact_raster::Crop::default(),
                },
                view: ViewKey {
                    view: id,
                    generation,
                },
                metadata: header.metadata,
                cost: plan.cost,
                priority: Priority::Visible,
            });
            sources.push(source);
        }
        let events = events.clone();
        let decoded = decoded.clone();
        *backend.hook.lock().unwrap() = Some(Arc::new(move |_, _| {
            decoded.fetch_add(1, Ordering::SeqCst);
            events.lock().unwrap().push("decode");
        }));
        Self {
            backend,
            _assets: assets,
            _sources: sources,
            demands,
        }
    }
    fn enqueue(&self) {
        for demand in &self.demands {
            self.backend.request(*demand).unwrap();
        }
    }
}
fn metadata(workers: &Workers, events: &Events, name: &'static str) -> (Arc<Backend>, Arc<Assets>) {
    let backend = Backend::for_workers(workers);
    let events = events.clone();
    let bytes = pixels();
    let assets = Arc::new(Assets::selected(
        Default::default(),
        Arc::new(move |_| {
            events.lock().unwrap().push(name);
            Ok(Some(bytes.clone()))
        }),
    ));
    (backend, assets)
}

#[test]
fn waited_decode_must_give_the_next_turn_to_metadata() {
    let workers = workers();
    let events = Events::default();
    let known = Known::new(&workers, &events, &Arc::new(AtomicUsize::new(0)));
    let (b, assets) = metadata(&workers, &events, "metadata");
    let mut queued = None;
    let mut turn = true;
    workers.turn(&mut turn, || {
        // Work arrives after this worker's empty metadata scan, before wait_decode.
        queued = Some(b.source(b.generation(), "new", &assets).unwrap());
        known.enqueue();
    });
    assert_eq!(*events.lock().unwrap(), ["decode"]);
    workers.turn(&mut turn, || panic!("ready work must not wait"));
    assert_eq!(*events.lock().unwrap(), ["decode", "metadata"]);
    drop(queued);
}

#[test]
fn competing_metadata_turns_still_leave_known_decode_progress() {
    let workers = workers();
    let events = Events::default();
    let known = Known::new(&workers, &events, &Arc::new(AtomicUsize::new(0)));
    let (b, ba) = metadata(&workers, &events, "metadata-b");
    let (c, ca) = metadata(&workers, &events, "metadata-c");
    let _b = b.source(b.generation(), "new", &ba).unwrap();
    let _c = c.source(c.generation(), "new", &ca).unwrap();
    known.enqueue();
    let mut turn = false;
    for _ in 0..4 {
        workers.turn(&mut turn, || panic!("ready work must not wait"));
    }
    let events = events.lock().unwrap();
    assert_eq!(events[0], "decode");
    assert_eq!(events[2], "decode");
    assert!(events[1].starts_with("metadata-") && events[3].starts_with("metadata-"));
    assert_ne!(events[1], events[3]);
}

#[test]
fn metadata_already_claimed_does_not_block_other_workers_decode_progress() {
    let workers = workers();
    let events = Events::default();
    let decoded = Arc::new(AtomicUsize::new(0));
    let known: Vec<_> = (0..4)
        .map(|_| Known::new(&workers, &events, &decoded))
        .collect();
    let b = Backend::for_workers(&workers);
    let (claimed_tx, claimed_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let release_rx = Mutex::new(release_rx);
    let observed = Arc::new(AtomicUsize::new(usize::MAX));
    let late = observed.clone();
    let count = decoded.clone();
    let bytes = pixels();
    let assets = Arc::new(Assets::selected(
        Default::default(),
        Arc::new(move |_| {
            claimed_tx.send(()).unwrap();
            release_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .map_err(|e| e.to_string())?;
            late.store(count.load(Ordering::SeqCst), Ordering::SeqCst);
            Ok(Some(bytes.clone()))
        }),
    ));
    let source = b.source(b.generation(), "new", &assets).unwrap();
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| workers.turn(&mut true, || panic!("metadata available")));
        claimed_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(b.prepared(source.id), Some(Prepared::Reading)));
        for a in &known {
            a.enqueue();
        }
        let mut turn = false;
        for _ in 0..8 {
            workers.turn(&mut turn, || {});
        }
        let count = decoded.load(Ordering::SeqCst);
        // Release before assertions so the scoped worker cannot be stranded.
        release_tx.send(()).unwrap();
        worker.join().unwrap();
        assert_eq!(count, 8);
        assert_eq!(
            observed.load(Ordering::SeqCst),
            8,
            "a late callback sample can exceed four after prompt metadata admission"
        );
    });
}
