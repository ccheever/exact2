use super::*;
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::Duration;

#[derive(Default)]
pub(super) struct Probe {
    starts: AtomicUsize,
    active: AtomicUsize,
    peak: AtomicUsize,
    drops: Arc<AtomicUsize>,
    pause: Mutex<Option<(SyncSender<()>, Receiver<()>)>>,
}

pub(super) struct Active(Arc<Probe>);

impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Probe {
    pub(super) fn enter(self: &Arc<Self>) -> Active {
        self.starts.fetch_add(1, Ordering::SeqCst);
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(active, Ordering::SeqCst);
        Active(self.clone())
    }

    pub(super) fn built(&self, parsed: &mut Parsed) {
        parsed.drop_witness = Some(crate::DropWitness(self.drops.clone()));
        let pause = self.pause.lock().unwrap().take();
        if let Some((ready, resume)) = pause {
            ready.send(()).unwrap();
            resume.recv_timeout(Duration::from_secs(10)).unwrap();
        }
    }

    fn pause_after_allocation(&self) -> (Receiver<()>, SyncSender<()>) {
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (resume_tx, resume_rx) = mpsc::sync_channel(1);
        *self.pause.lock().unwrap() = Some((ready_tx, resume_rx));
        (ready_rx, resume_tx)
    }

    fn counts(&self) -> (usize, usize, usize) {
        (
            self.starts.load(Ordering::SeqCst),
            self.peak.load(Ordering::SeqCst),
            self.drops.load(Ordering::SeqCst),
        )
    }
}

fn args(revision: usize, page: usize) -> Vec<Value> {
    vec![
        Value::str("blocks"),
        Value::Number(16_384.0),
        Value::Number(revision as f64),
        Value::Number(page as f64),
        Value::Bool(false),
    ]
}

fn issue(source: &mut NativeMarkdownStress, args: &[Value]) -> u64 {
    let Answer::Later(request) = source
        .answer(&mut Store::default(), "document", args)
        .unwrap()
    else {
        panic!("expected native pending")
    };
    assert!(request.is_ordered());
    request.continuation.unwrap()
}

fn finish(source: &mut NativeMarkdownStress, args: &[Value], outcome: Outcome) -> Value {
    let Answer::Now(value) = source
        .parse(&mut Store::default(), "document", args, outcome)
        .unwrap()
    else {
        panic!("expected current result")
    };
    value
}

#[test]
fn hundred_queued_keys_skip_stale_generation_before_any_parse() {
    let mut source = NativeMarkdownStress::default();
    let probe = source.probe.clone();
    let mut jobs = Vec::new();
    for revision in 0..100 {
        let token = issue(&mut source, &args(revision, 0));
        jobs.push(source.continuation(token).unwrap());
    }
    assert_eq!(probe.counts(), (0, 0, 0));
    // The native host has one ordered continuation worker, not 100 workers.
    let outcomes =
        std::thread::spawn(move || jobs.into_iter().map(|job| job()).collect::<Vec<_>>())
            .join()
            .unwrap();
    assert_eq!(probe.counts(), (1, 1, 0));
    let current = outcomes.into_iter().last().unwrap();
    let actual = finish(&mut source, &args(99, 0), current);
    assert_eq!(
        actual,
        MarkdownStress::default()
            .query("document", &args(99, 0))
            .unwrap()
    );
    drop(source);
    assert_eq!(probe.counts(), (1, 1, 1));
}

#[test]
fn same_key_twenty_page_tickets_share_one_producer_and_latest_can_settle() {
    let mut source = NativeMarkdownStress::default();
    let probe = source.probe.clone();
    let mut jobs = Vec::new();
    for page in 0..20 {
        let token = issue(&mut source, &args(4, page));
        jobs.push(source.continuation(token).unwrap());
    }
    let mut outcomes =
        std::thread::spawn(move || jobs.into_iter().map(|job| job()).collect::<Vec<_>>())
            .join()
            .unwrap();
    assert_eq!(probe.counts(), (1, 1, 0));
    let current = outcomes.pop().unwrap();
    drop(outcomes); // No source parse callbacks for the nineteen obsolete tickets.
    let actual = finish(&mut source, &args(4, 19), current);
    assert_eq!(
        actual,
        MarkdownStress::default()
            .query("document", &args(4, 19))
            .unwrap()
    );
    assert_eq!(probe.counts(), (1, 1, 0));
    drop(source);
    assert_eq!(probe.counts(), (1, 1, 1));
}

#[test]
fn running_stale_payload_drops_before_latest_work_without_a_parse_callback() {
    let mut source = NativeMarkdownStress::default();
    let probe = source.probe.clone();
    let (ready, resume) = probe.pause_after_allocation();
    let token = issue(&mut source, &args(0, 0));
    let old_worker = std::thread::spawn(source.continuation(token).unwrap());
    ready.recv_timeout(Duration::from_secs(10)).unwrap();
    let mut jobs = Vec::new();
    for revision in 1..=100 {
        let token = issue(&mut source, &args(revision, 0));
        jobs.push(source.continuation(token).unwrap());
    }
    assert_eq!(
        probe.counts(),
        (1, 1, 0),
        "old allocation still belongs to running worker"
    );
    resume.send(()).unwrap();
    drop(old_worker.join().unwrap());
    assert_eq!(
        probe.counts(),
        (1, 1, 1),
        "stale allocation dropped without UI parse"
    );
    let outcome = std::thread::spawn(move || {
        let mut last = None;
        for job in jobs {
            last = Some(job());
        }
        last.unwrap()
    })
    .join()
    .unwrap();
    assert_eq!(probe.counts(), (2, 1, 1));
    finish(&mut source, &args(100, 0), outcome);
    drop(source);
    assert_eq!(probe.counts(), (2, 1, 2));
}

#[test]
fn ready_replacement_and_source_drop_release_payloads_while_acks_remain_owned() {
    let mut source = NativeMarkdownStress::default();
    let probe = source.probe.clone();
    let a = issue(&mut source, &args(0, 0));
    let old_ack = std::thread::spawn(source.continuation(a).unwrap())
        .join()
        .unwrap();
    assert_eq!(probe.counts(), (1, 1, 0));
    let b = issue(&mut source, &args(1, 0));
    assert_eq!(
        probe.counts(),
        (1, 1, 1),
        "old ready payload is not in an ACK/done map"
    );
    let new_ack = std::thread::spawn(source.continuation(b).unwrap())
        .join()
        .unwrap();
    assert_eq!(probe.counts(), (2, 1, 1));
    drop(source);
    assert_eq!(probe.counts(), (2, 1, 2));
    drop((old_ack, new_ack));
    assert_eq!(
        probe.counts(),
        (2, 1, 2),
        "ACK destruction cannot refund/drop another result"
    );
}

#[test]
fn source_drop_while_allocated_work_runs_does_not_publish_or_reset_ownership() {
    let mut source = NativeMarkdownStress::default();
    let probe = source.probe.clone();
    let (ready, resume) = probe.pause_after_allocation();
    let a = issue(&mut source, &args(0, 0));
    let worker = std::thread::spawn(source.continuation(a).unwrap());
    ready.recv_timeout(Duration::from_secs(10)).unwrap();
    drop(source);
    assert_eq!(probe.counts(), (1, 1, 0));
    resume.send(()).unwrap();
    let ack = worker.join().unwrap();
    assert_eq!(probe.counts(), (1, 1, 1));
    drop(ack);
}

#[test]
fn source_drop_before_queued_work_starts_skips_generation_and_parse() {
    let mut source = NativeMarkdownStress::default();
    let probe = source.probe.clone();
    let token = issue(&mut source, &args(0, 0));
    let work = source.continuation(token).unwrap();
    drop(source);
    drop(std::thread::spawn(work).join().unwrap());
    assert_eq!(probe.counts(), (0, 0, 0));
}

#[test]
fn serial_exhaustion_preserves_ready_cell_accepted_cache_and_current_token() {
    let mut source = NativeMarkdownStress::default();
    let accepted = source.query("document", &args(0, 0)).unwrap();
    source.serial_counter = Some(Arc::new(AtomicU64::new(u64::MAX - 1)));
    let token = issue(&mut source, &args(1, 0));
    assert_eq!(token, u64::MAX);
    let current = std::thread::spawn(source.continuation(token).unwrap())
        .join()
        .unwrap();
    let probe = source.probe.clone();
    assert!(source
        .answer(&mut Store::default(), "document", &args(2, 0))
        .is_err());
    assert!(source
        .answer(&mut Store::default(), "document", &args(1, 1))
        .is_err());
    assert_eq!(probe.counts(), (1, 1, 0));
    assert_eq!(
        source.sync.query("document", &args(0, 0)).unwrap(),
        accepted
    );
    assert_eq!(
        finish(&mut source, &args(1, 0), current),
        MarkdownStress::default()
            .query("document", &args(1, 0))
            .unwrap()
    );
    drop(source);
    assert_eq!(probe.counts(), (1, 1, 1));
}

#[test]
fn replacement_source_serial_does_not_accept_old_same_key_ack() {
    let mut old = NativeMarkdownStress::default();
    let old_token = issue(&mut old, &args(0, 0));
    let old_ack = std::thread::spawn(old.continuation(old_token).unwrap())
        .join()
        .unwrap();
    drop(old);
    let mut current = NativeMarkdownStress::default();
    let token = issue(&mut current, &args(0, 0));
    assert_ne!(old_token, token);
    let ack = std::thread::spawn(current.continuation(token).unwrap())
        .join()
        .unwrap();
    assert!(current
        .parse(&mut Store::default(), "document", &args(0, 0), old_ack)
        .is_err());
    finish(&mut current, &args(0, 0), ack);
}

#[test]
fn returning_to_accepted_key_does_not_allow_query_to_overlap_retiring_parse() {
    let mut source = NativeMarkdownStress::default();
    let accepted = source.query("document", &args(0, 0)).unwrap();
    let probe = source.probe.clone();
    let (ready, resume) = probe.pause_after_allocation();
    let token = issue(&mut source, &args(1, 0));
    let worker = std::thread::spawn(source.continuation(token).unwrap());
    ready.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(
        source
            .answer(&mut Store::default(), "document", &args(0, 0))
            .unwrap(),
        Answer::Now(accepted)
    );
    let refused = source.query("document", &args(2, 0)).is_err();
    // Always release the paused worker before an assertion can unwind.
    resume.send(()).unwrap();
    drop(worker.join().unwrap());
    assert!(
        refused,
        "cancelled-but-running allocation still excludes synchronous query"
    );
    assert_eq!(probe.counts(), (1, 1, 1));
    assert!(source.query("document", &args(2, 0)).is_ok());
}

#[test]
fn executor_refusal_preserves_accepted_document_and_never_parses_on_ui() {
    let mut source = NativeMarkdownStress::default();
    let accepted = source.query("document", &args(0, 0)).unwrap();
    let probe = source.probe.clone();
    let token = issue(&mut source, &args(1, 0));
    drop(source.continuation(token).unwrap()); // Admission refused the owned job.
    assert!(source
        .parse(
            &mut Store::default(),
            "document",
            &args(1, 0),
            Outcome::Failed {
                kind: FailureKind::Refused,
                message: "native executor admission limit reached".into(),
            }
        )
        .is_err());
    assert_eq!(probe.counts(), (0, 0, 0));
    assert_eq!(
        source
            .answer(&mut Store::default(), "document", &args(0, 0))
            .unwrap(),
        Answer::Now(accepted)
    );
    let token = issue(&mut source, &args(1, 0));
    let outcome = std::thread::spawn(source.continuation(token).unwrap())
        .join()
        .unwrap();
    finish(&mut source, &args(1, 0), outcome);
    assert_eq!(probe.counts(), (1, 1, 0));
}
