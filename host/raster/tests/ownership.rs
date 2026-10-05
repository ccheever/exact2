use exact_raster::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

const MIB: u64 = 1024 * 1024;

fn demand(view: u64, source: u64, output_mib: u64, scratch_mib: u64) -> Demand {
    let pixels = PixelSize {
        width: 256,
        height: (output_mib * 1024) as u32,
    };
    Demand {
        view: ViewKey {
            view,
            generation: 1,
        },
        key: RasterKey {
            source,
            generation: 1,
            pixels,
            variant: 0,
        },
        metadata: Metadata {
            natural: PixelSize {
                width: 4000,
                height: 2000,
            },
            encoded_bytes: 100,
            header_bytes: 33,
        },
        cost: DecodeCost::checked(1024, pixels.height, scratch_mib * MIB, 0).unwrap(),
        priority: Priority::Visible,
    }
}

struct Backing {
    pixels: Vec<u8>,
    _charge: AllocationCharge,
    drops: Arc<AtomicUsize>,
}
impl Drop for Backing {
    fn drop(&mut self) {
        drop(std::mem::take(&mut self.pixels));
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
fn backing(permit: &DecodePermit, drops: &Arc<AtomicUsize>) -> Arc<Backing> {
    Arc::new(Backing {
        pixels: vec![17; permit.cost().output_bytes as usize],
        _charge: permit.allocation_charge(),
        drops: drops.clone(),
    })
}
fn complete(permit: DecodePermit, drops: &Arc<AtomicUsize>) {
    let pixels = backing(&permit, drops);
    let bytes = ResidentBytes {
        output: permit.cost().output_bytes,
        copy: 0,
    };
    assert!(permit.complete(pixels, bytes).unwrap());
}
fn total(session: &RasterSession) -> u64 {
    let s = session.stats();
    s.resident_bytes + s.reserved_bytes
}

#[test]
fn cancelled_allocated_decoders_keep_budget_and_slots_until_real_owner_drop() {
    let gate = Gate::new();
    let session = gate.session();
    let drops = Arc::new(AtomicUsize::new(0));
    let a = session.request(demand(1, 1, 20, 0)).unwrap();
    complete(gate.next_decode().unwrap(), &drops);
    let displayed = session.take_ready(a).unwrap();
    let painter = displayed.payload::<Arc<Backing>>().unwrap().clone();
    let b1 = session.request(demand(2, 2, 4, 2)).unwrap();
    let b2 = session.request(demand(3, 3, 4, 2)).unwrap();
    let worker1 = gate.next_decode().unwrap();
    let worker2 = gate.next_decode().unwrap();
    let pixels1 = backing(&worker1, &drops);
    let pixels2 = backing(&worker2, &drops);
    let scratch1 = vec![1u8; 2 * MIB as usize];
    let scratch2 = vec![1u8; 2 * MIB as usize];
    assert_eq!(total(&session), 32 * MIB);
    assert!(session.cancel(b1));
    assert!(session.cancel(b2));
    let mut replacement = demand(2, 2, 4, 2);
    replacement.view.generation = 2;
    replacement.key.generation = 2;
    let c = session.request(replacement).unwrap();
    assert!(worker1.is_cancelled() && worker2.is_cancelled());
    assert_eq!(total(&session), 32 * MIB);
    assert_eq!(gate.stats().running, 2);
    assert!(gate.next_decode().is_none());
    assert!(!session.cancel(b1));
    assert!(session.take_ready(b1).is_none());
    drop(scratch1);
    assert!(!worker1
        .complete(
            pixels1,
            ResidentBytes {
                output: 4 * MIB,
                copy: 0
            }
        )
        .unwrap());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(total(&session), 26 * MIB);
    let current = gate.next_decode().unwrap();
    assert_eq!(current.key().generation, 2);
    assert_eq!(total(&session), 32 * MIB);
    assert!(session.take_ready(b1).is_none());
    assert_eq!(session.status(c), Some(RequestStatus::Decoding));
    session.reset();
    assert_eq!(
        total(&session),
        32 * MIB,
        "reset cannot refund workers or a displayed backing"
    );
    drop(scratch2);
    drop(pixels2);
    drop(worker2);
    drop(current);
    assert_eq!(total(&session), 20 * MIB);
    drop(displayed);
    assert_eq!(
        total(&session),
        20 * MIB,
        "native painter outlives RasterLease"
    );
    drop(painter);
    assert_eq!(total(&session), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 3);
    assert_eq!(gate.stats().running, 0);
}

#[test]
fn two_unconsumed_results_do_not_starve_other_sessions() {
    let gate = Gate::new();
    let a = gate.session();
    let b = gate.session();
    let c = gate.session();
    let drops = Arc::new(AtomicUsize::new(0));
    for i in 1..=3 {
        a.request(demand(i, i, 1, 0)).unwrap();
    }
    complete(gate.next_decode().unwrap(), &drops);
    complete(gate.next_decode().unwrap(), &drops);
    assert_eq!(a.stats().ready, 2);
    assert_eq!(a.stats().delivery_cells, 2);
    assert_eq!(gate.stats().running, 0);
    b.request(demand(1, 10, 1, 0)).unwrap();
    c.request(demand(1, 11, 1, 0)).unwrap();
    let first = gate.next_decode().unwrap();
    let second = gate.next_decode().unwrap();
    assert_eq!([first.session_id(), second.session_id()], [b.id(), c.id()]);
    assert!(gate.next_decode().is_none());
    a.pause();
    assert_eq!(
        drops.load(Ordering::SeqCst),
        2,
        "no UI consumer needed to destroy results"
    );
    assert_eq!(a.stats().delivery_cells, 0);
    assert_eq!(total(&a), 0);
    drop(first);
    drop(second);
}

#[test]
fn cache_two_views_and_native_painter_share_one_allocation_until_last_drop() {
    let gate = Gate::new();
    let session = gate.session();
    let drops = Arc::new(AtomicUsize::new(0));
    let a = session.request(demand(1, 1, 1, 0)).unwrap();
    let b = session.request(demand(2, 1, 1, 0)).unwrap();
    assert_eq!(session.stats().pending_jobs, 1);
    complete(gate.next_decode().unwrap(), &drops);
    let one = session.take_ready(a).unwrap();
    let two = session.take_ready(b).unwrap();
    assert_eq!(
        one.metadata().natural,
        PixelSize {
            width: 4000,
            height: 2000
        }
    );
    let native = one.payload::<Arc<Backing>>().unwrap().clone();
    session.cancel(a);
    session.cancel(b);
    session.trim();
    drop(one);
    assert_eq!(total(&session), MIB);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(two);
    session.trim();
    assert_eq!(session.stats().retiring_bytes, MIB);
    session.shutdown();
    assert_eq!(total(&session), MIB);
    drop(native);
    assert_eq!(total(&session), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn stale_completion_and_duplicate_consumption_never_retire_replacement() {
    let gate = Gate::new();
    let session = gate.session();
    let drops = Arc::new(AtomicUsize::new(0));
    let old = session.request(demand(1, 1, 1, 0)).unwrap();
    complete(gate.next_decode().unwrap(), &drops);
    let mut next = demand(1, 1, 2, 0);
    next.key.generation = 2;
    let current = session.request(next).unwrap();
    assert!(session.take_ready(old).is_none());
    assert!(!session.cancel(old));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    let permit = gate.next_decode().unwrap();
    assert_eq!(total(&session), 2 * MIB);
    complete(permit, &drops);
    let a = session.take_ready(current).unwrap();
    let b = session.take_ready(current).unwrap();
    assert_eq!(session.stats().delivery_cells, 0);
    assert_eq!(total(&session), 2 * MIB);
    drop(a);
    drop(b);
    session.reset();
    assert_eq!(total(&session), 0);
}

#[test]
fn scratch_and_independent_copy_are_reserved_before_allocation() {
    let gate = Gate::new();
    let session = gate.session();
    let mut d = demand(1, 1, 4, 2);
    d.cost.copy_bytes = 4 * MIB;
    let id = session.request(d).unwrap();
    let permit = gate.next_decode().unwrap();
    let output = permit.allocation_charge();
    let copy = permit.copy_charge().unwrap();
    assert_eq!(total(&session), 10 * MIB);
    assert!(permit
        .complete(
            (),
            ResidentBytes {
                output: 4 * MIB,
                copy: 4 * MIB
            }
        )
        .unwrap());
    assert_eq!(session.stats().resident_bytes, 8 * MIB);
    assert_eq!(session.stats().reserved_bytes, 0);
    let lease = session.take_ready(id).unwrap();
    session.reset();
    drop(lease);
    assert_eq!(total(&session), 8 * MIB);
    drop(output);
    assert_eq!(total(&session), 4 * MIB);
    drop(copy);
    assert_eq!(total(&session), 0);
    let reservation = session.reserve_allocation(3 * MIB).unwrap();
    let provider = reservation.charge();
    let committed = reservation.commit(2 * MIB).unwrap();
    assert_eq!(total(&session), 2 * MIB);
    drop(committed);
    assert_eq!(total(&session), 2 * MIB);
    drop(provider);
    assert_eq!(total(&session), 0);
}

#[test]
fn budget_waiter_retries_when_last_native_owner_releases_bytes() {
    let gate = Gate::new();
    let session = gate.session();
    let held = session
        .reserve_allocation(30 * MIB)
        .unwrap()
        .commit(30 * MIB)
        .unwrap();
    let id = session.request(demand(1, 1, 4, 0)).unwrap();
    assert!(gate.next_decode().is_none());
    assert_eq!(session.status(id), Some(RequestStatus::WaitingBudget));
    let (tx, rx) = std::sync::mpsc::channel();
    let worker_gate = gate.clone();
    let worker = std::thread::spawn(move || {
        tx.send(worker_gate.wait_decode(std::time::Duration::from_secs(2)))
            .unwrap();
    });
    assert!(matches!(
        rx.recv_timeout(std::time::Duration::from_millis(30)),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
    ));
    drop(held);
    let permit = rx
        .recv_timeout(std::time::Duration::from_secs(3))
        .unwrap()
        .unwrap();
    assert_eq!(permit.key().source, 1);
    drop(permit);
    worker.join().unwrap();
}

#[test]
fn metadata_queue_subscribers_and_retired_source_history_are_bounded() {
    let gate = Gate::new();
    let session = gate.session();
    let first = session.request(demand(1, 1, 1, 0)).unwrap();
    for view in 2..=SUBSCRIPTIONS as u64 {
        session.request(demand(view, 1, 1, 0)).unwrap();
    }
    assert_eq!(session.stats().pending_jobs, 1);
    assert_eq!(
        session.request(demand(1025, 1, 1, 0)),
        Err(Refusal::SubscriberLimit)
    );
    session.reset();
    assert!(session.status(first).is_none());
    for view in 1..=PENDING_JOBS as u64 {
        session.request(demand(view, view, 1, 0)).unwrap();
    }
    assert_eq!(
        session.request(demand(100, 100, 1, 0)),
        Err(Refusal::QueueFull)
    );
    session.reset();
    for view in 1..=100 {
        let id = session.request(demand(view, view, 1, 0)).unwrap();
        let p = gate.next_decode().unwrap();
        p.complete((), ResidentBytes { output: 1, copy: 0 })
            .unwrap();
        drop(session.take_ready(id).unwrap());
        session.cancel(id);
        assert!(session.stats().cold_entries <= COLD_ENTRIES);
    }
    session.trim();
    assert_eq!(total(&session), 0);
}

#[test]
fn invalid_metadata_and_peak_overflow_leave_old_demand_untouched() {
    let gate = Gate::new();
    let session = gate.session();
    let d = demand(1, 1, 1, 0);
    let id = session.request(d).unwrap();
    let mut bad = d;
    bad.metadata.encoded_bytes = MAX_ENCODED_BYTES + 1;
    assert_eq!(session.request(bad), Err(Refusal::EncodedLimit));
    bad = d;
    bad.metadata.header_bytes = MAX_HEADER_BYTES + 1;
    assert_eq!(session.request(bad), Err(Refusal::HeaderLimit));
    bad = d;
    bad.metadata.natural.width = 0;
    assert_eq!(session.request(bad), Err(Refusal::InvalidDimensions));
    bad = d;
    bad.cost.scratch_bytes = SESSION_BYTES;
    assert_eq!(session.request(bad), Err(Refusal::TooLarge));
    assert_eq!(
        DecodeCost::checked(u64::MAX, 2, 0, 0),
        Err(Refusal::Overflow)
    );
    assert_eq!(
        DecodeCost::checked(4, 1, u64::MAX, 0),
        Err(Refusal::Overflow)
    );
    assert_eq!(session.status(id), Some(RequestStatus::Queued));
    assert_eq!(session.stats().subscribers, 1);
}

#[test]
fn completion_racing_shutdown_drops_payload_and_can_reenter_stats_outside_locks() {
    struct Probe {
        session: RasterSession,
        sent: std::sync::mpsc::Sender<()>,
    }
    impl Drop for Probe {
        fn drop(&mut self) {
            let _ = self.session.stats();
            self.sent.send(()).unwrap();
        }
    }
    let gate = Gate::new();
    let session = gate.session();
    session.request(demand(1, 1, 1, 0)).unwrap();
    let p = gate.next_decode().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let probe = Probe {
        session: session.clone(),
        sent: tx,
    };
    session.shutdown();
    let worker = std::thread::spawn(move || {
        assert!(!p
            .complete(
                probe,
                ResidentBytes {
                    output: MIB,
                    copy: 0
                }
            )
            .unwrap());
    });
    rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
    worker.join().unwrap();
    assert_eq!(gate.stats().running, 0);
    assert_eq!(total(&session), 0);
}

#[test]
fn round_robin_and_visible_priority_do_not_depend_on_polling_one_session() {
    let gate = Gate::new();
    let a = gate.session();
    let b = gate.session();
    let mut overscan = demand(1, 1, 1, 0);
    overscan.priority = Priority::Overscan;
    a.request(overscan).unwrap();
    a.request(demand(2, 2, 1, 0)).unwrap();
    b.request(demand(1, 3, 1, 0)).unwrap();
    let first = gate.next_decode().unwrap();
    let second = gate.next_decode().unwrap();
    assert_eq!(first.key().source, 2);
    assert_eq!(second.session_id(), b.id());
    drop(first);
    drop(second);
}

#[test]
fn reclaimed_cold_bytes_admit_visible_before_smaller_overscan() {
    let gate = Gate::new();
    let session = gate.session();
    let drops = Arc::new(AtomicUsize::new(0));
    let old = session.request(demand(1, 1, 28, 0)).unwrap();
    complete(gate.next_decode().unwrap(), &drops);
    drop(session.take_ready(old).unwrap());
    assert!(session.cancel(old));
    assert_eq!(session.stats().cold_bytes, 28 * MIB);

    let visible = session.request(demand(2, 2, 8, 0)).unwrap();
    let mut overscan = demand(3, 3, 4, 0);
    overscan.priority = Priority::Overscan;
    let overscan = session.request(overscan).unwrap();
    let first = gate.next_decode().unwrap();
    assert_eq!(
        first.key().source,
        2,
        "reclaim cold bytes before admitting a lower-priority fitting request"
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(session.stats().evicted, 1);
    assert_eq!(session.status(visible), Some(RequestStatus::Decoding));
    assert_eq!(session.status(overscan), Some(RequestStatus::Queued));
    assert_eq!(total(&session), 8 * MIB);
    let second = gate.next_decode().unwrap();
    assert_eq!(second.key().source, 3);
    assert_eq!(gate.stats().running, RUNNING_DECODES);
    assert_eq!(session.stats().delivery_cells, DELIVERY_CELLS);
    drop(first);
    drop(second);
}

#[test]
fn a_decode_short_of_budget_evicts_only_the_oldest_cold_bytes_it_needs() {
    let gate = Gate::new();
    let session = gate.session_with_budget(64 * MIB);
    let drops = Arc::new(AtomicUsize::new(0));
    // Three cold images, oldest first: 20 + 20 + 20 of 64 MiB.
    for source in 1..=3 {
        let request = session.request(demand(source, source, 20, 0)).unwrap();
        complete(gate.next_decode().unwrap(), &drops);
        drop(session.take_ready(request).unwrap());
        assert!(session.cancel(request));
    }
    assert_eq!(session.stats().cold_bytes, 60 * MIB);
    // 10 MiB more fits after evicting the oldest alone.
    let next = session.request(demand(4, 4, 10, 0)).unwrap();
    let permit = gate.next_decode().unwrap();
    assert_eq!(permit.key().source, 4);
    assert_eq!(session.stats().evicted, 1);
    assert_eq!(session.stats().cold_bytes, 40 * MIB);
    complete(permit, &drops);
    drop(session.take_ready(next).unwrap());
    // The two newer images are still cached: asking again decodes nothing.
    let again = session.request(demand(5, 3, 20, 0)).unwrap();
    assert_eq!(session.status(again), Some(RequestStatus::Ready));
    assert_eq!(session.stats().dedup_hits, 1);
}

#[test]
fn retained_cold_backing_allows_fitting_overscan_without_spinning() {
    let gate = Gate::new();
    let session = gate.session();
    let drops = Arc::new(AtomicUsize::new(0));
    let old = session.request(demand(1, 1, 28, 0)).unwrap();
    complete(gate.next_decode().unwrap(), &drops);
    let lease = session.take_ready(old).unwrap();
    let native = lease.payload::<Arc<Backing>>().unwrap().clone();
    drop(lease);
    assert!(session.cancel(old));
    let visible = session.request(demand(2, 2, 8, 0)).unwrap();
    let mut overscan = demand(3, 3, 4, 0);
    overscan.priority = Priority::Overscan;
    session.request(overscan).unwrap();

    // Evicting metadata cannot free a backing retained by the native painter.
    // A bounded wait makes a retry loop that expects those bytes to disappear
    // fail deterministically instead of hanging the ownership suite.
    let (tx, rx) = std::sync::mpsc::channel();
    let worker_gate = gate.clone();
    let worker = std::thread::spawn(move || tx.send(worker_gate.next_decode()).unwrap());
    let first = rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("admission must skip bytes still retained by an external owner")
        .unwrap();
    worker.join().unwrap();
    assert_eq!(first.key().source, 3);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(session.stats().retiring_bytes, 28 * MIB);
    assert_eq!(session.stats().evicted, 1);
    assert_eq!(total(&session), SESSION_BYTES);
    assert_eq!(session.status(visible), Some(RequestStatus::WaitingBudget));

    drop(native);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    let second = gate.next_decode().unwrap();
    assert_eq!(second.key().source, 2);
    assert_eq!(total(&session), 12 * MIB);
    assert!(session.stats().peak_bytes <= SESSION_BYTES);
    drop(first);
    drop(second);
}

#[test]
fn displayed_lease_preserves_cached_dedup_while_replacement_waits_for_budget() {
    let gate = Gate::new();
    let session = gate.session();
    let drops = Arc::new(AtomicUsize::new(0));
    let old = session.request(demand(1, 1, 20, 0)).unwrap();
    complete(gate.next_decode().unwrap(), &drops);
    let displayed = session.take_ready(old).unwrap();
    let replacement = session.request(demand(1, 2, 16, 0)).unwrap();
    assert!(gate.next_decode().is_none());
    assert_eq!(
        session.status(replacement),
        Some(RequestStatus::WaitingBudget)
    );
    session.trim();
    assert_eq!(
        session.stats().evicted,
        0,
        "displayed backing remains indexed"
    );
    let again = session.request(demand(2, 1, 20, 0)).unwrap();
    assert_eq!(session.status(again), Some(RequestStatus::Ready));
    let shared = session.take_ready(again).unwrap();
    assert!(Arc::ptr_eq(
        displayed.payload::<Arc<Backing>>().unwrap(),
        shared.payload::<Arc<Backing>>().unwrap()
    ));
    session.cancel(again);
    drop(shared);
    assert_eq!(total(&session), 20 * MIB);
    drop(displayed);
    let next = gate.next_decode().unwrap();
    assert_eq!(next.key().source, 2);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(total(&session), 16 * MIB);
}

#[test]
fn detached_pinned_metadata_is_bounded_and_final_unpin_bounds_cold_entries() {
    let gate = Gate::new();
    let session = gate.session();
    let mut leases = Vec::new();
    let mut d = demand(1, 1, 1, 0);
    d.key.pixels = PixelSize {
        width: 1,
        height: 1,
    };
    d.cost = DecodeCost::checked(4, 1, 0, 0).unwrap();
    let mut last = None;
    for source in 1..=SUBSCRIPTIONS as u64 {
        d.key.source = source;
        let id = session.request(d).unwrap();
        gate.next_decode()
            .unwrap()
            .complete((), ResidentBytes { output: 4, copy: 0 })
            .unwrap();
        leases.push(session.take_ready(id).unwrap());
        last = Some(id);
    }
    d.key.source += 1;
    assert_eq!(session.request(d), Err(Refusal::SubscriberLimit));
    assert_eq!(
        session.status(last.unwrap()),
        Some(RequestStatus::Ready),
        "failed replacement preserves current interest"
    );
    session.cancel(last.unwrap());
    assert_eq!(session.stats().cold_entries, 0);
    assert_eq!(total(&session), SUBSCRIPTIONS as u64 * 4);
    // Reattaching one pinned entry does not consume another metadata slot.
    d.key.source = 1;
    let id = session.request(d).unwrap();
    assert_eq!(session.status(id), Some(RequestStatus::Ready));
    session.cancel(id);
    drop(leases);
    assert_eq!(session.stats().cold_entries, COLD_ENTRIES);
    assert_eq!(total(&session), COLD_ENTRIES as u64 * 4);
    session.trim();
    assert_eq!(total(&session), 0);
}

#[test]
fn forged_costs_and_conflicts_cannot_replace_a_ready_subscription() {
    let gate = Gate::new();
    let session = gate.session();
    let d = demand(1, 1, 1, 0);
    let id = session.request(d).unwrap();
    gate.next_decode()
        .unwrap()
        .complete(
            (),
            ResidentBytes {
                output: MIB,
                copy: 0,
            },
        )
        .unwrap();
    let mut cases = Vec::new();
    let mut bad = d;
    bad.cost.output_bytes -= 1;
    cases.push((bad, Refusal::InvalidDimensions));
    bad = d;
    bad.cost.height /= 2;
    bad.cost.output_bytes /= 2;
    cases.push((bad, Refusal::InvalidDimensions));
    bad = d;
    bad.cost.stride /= 2;
    bad.cost.output_bytes /= 2;
    cases.push((bad, Refusal::InvalidDimensions));
    bad = d;
    bad.key.pixels.width = 0;
    cases.push((bad, Refusal::InvalidDimensions));
    bad = d;
    bad.cost.stride = u64::MAX;
    cases.push((bad, Refusal::Overflow));
    bad = d;
    bad.metadata.natural.width += 1;
    cases.push((bad, Refusal::ConflictingMetadata));
    bad = d;
    bad.cost.scratch_bytes += 1;
    cases.push((bad, Refusal::ConflictingMetadata));
    for (bad, expected) in cases {
        assert_eq!(session.request(bad), Err(expected));
        assert_eq!(session.status(id), Some(RequestStatus::Ready));
        assert_eq!(session.stats().subscribers, 1);
        assert_eq!(total(&session), MIB);
    }
    assert!(session.take_ready(id).is_some());
}

#[test]
fn final_session_drop_drains_completed_mailbox_without_ui_or_account_cycle() {
    let gate = Gate::new();
    let session = gate.session();
    let drops = Arc::new(AtomicUsize::new(0));
    session.request(demand(1, 1, 1, 0)).unwrap();
    let permit = gate.next_decode().unwrap();
    let native = backing(&permit, &drops);
    let native_weak = Arc::downgrade(&native);
    permit
        .complete(
            native,
            ResidentBytes {
                output: MIB,
                copy: 0,
            },
        )
        .unwrap();
    assert_eq!(gate.stats().ready, 1);
    drop(session);
    assert_eq!(gate.stats().sessions, 0);
    assert_eq!(gate.stats().ready, 0);
    assert!(native_weak.upgrade().is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn pause_destroys_completed_payload_outside_gate_lock() {
    struct Probe {
        gate: Gate,
        sent: std::sync::mpsc::Sender<()>,
    }
    impl Drop for Probe {
        fn drop(&mut self) {
            assert_eq!(self.gate.stats().ready, 0);
            self.sent.send(()).unwrap();
        }
    }
    let gate = Gate::new();
    let session = gate.session();
    session.request(demand(1, 1, 1, 0)).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    gate.next_decode()
        .unwrap()
        .complete(
            Probe {
                gate: gate.clone(),
                sent: tx,
            },
            ResidentBytes {
                output: MIB,
                copy: 0,
            },
        )
        .unwrap();
    let owner = session.clone();
    let worker = std::thread::spawn(move || owner.pause());
    rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
    worker.join().unwrap();
    assert_eq!(total(&session), 0);
    assert_eq!(session.request(demand(2, 2, 1, 0)), Err(Refusal::Paused));
    session.resume();
    session.request(demand(2, 2, 1, 0)).unwrap();
    assert!(gate.next_decode().is_some());
}

#[test]
fn blocked_waiter_skips_full_mailbox_when_another_session_becomes_runnable() {
    let gate = Gate::new();
    let paused_ui = gate.session();
    let next_session = gate.session();
    for source in 1..=3 {
        paused_ui.request(demand(source, source, 1, 0)).unwrap();
    }
    for _ in 0..DELIVERY_CELLS {
        gate.next_decode()
            .unwrap()
            .complete(
                (),
                ResidentBytes {
                    output: MIB,
                    copy: 0,
                },
            )
            .unwrap();
    }
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (result_tx, result_rx) = std::sync::mpsc::channel();
    let worker_gate = gate.clone();
    let worker = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        result_tx
            .send(worker_gate.wait_decode(std::time::Duration::from_secs(2)))
            .unwrap();
    });
    started_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();
    assert!(matches!(
        result_rx.recv_timeout(std::time::Duration::from_millis(30)),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
    ));
    next_session.request(demand(1, 10, 1, 0)).unwrap();
    let permit = result_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap()
        .unwrap();
    assert_eq!(permit.session_id(), next_session.id());
    assert_eq!(paused_ui.stats().ready, DELIVERY_CELLS);
    assert_eq!(paused_ui.stats().queued, 1);
    assert_eq!(gate.stats().running, 1);
    drop(permit);
    worker.join().unwrap();
}

#[test]
fn viewport_budget_shrink_preserves_backings_and_growth_wakes_queued_decodes() {
    let gate = Gate::new();
    let session = gate.session_with_budget(64 * MIB);
    let drops = Arc::new(AtomicUsize::new(0));
    let a = session.request(demand(1, 1, 24, 0)).unwrap();
    complete(gate.next_decode().unwrap(), &drops);
    let displayed_a = session.take_ready(a).unwrap();
    let b = session.request(demand(2, 2, 24, 0)).unwrap();
    complete(gate.next_decode().unwrap(), &drops);
    let displayed_b = session.take_ready(b).unwrap();
    session.set_budget(32 * MIB);
    assert_eq!(total(&session), 48 * MIB);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(matches!(
        session.reserve_allocation(1),
        Err(Refusal::Budget)
    ));
    let pending = session.request(demand(3, 3, 8, 0)).unwrap();
    assert!(gate.next_decode().is_none());
    session.set_budget(64 * MIB);
    complete(gate.next_decode().unwrap(), &drops);
    assert!(session.take_ready(pending).is_some());
    assert!(session.cancel(a));
    drop(displayed_a);
    assert_eq!(session.stats().cold_bytes, 24 * MIB);
    session.set_budget(32 * MIB);
    assert_eq!(session.stats().cold_bytes, 0);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(displayed_b.payload::<Arc<Backing>>().is_some());
    assert!(session.cancel(b));
    drop(displayed_b);
    session.trim();
    assert!(session.reserve_allocation(1).is_ok());
}

#[test]
fn concurrent_budget_changes_keep_decode_admission_atomic() {
    let gate = Gate::new();
    let session = gate.session_with_budget(64 * MIB);
    // This retained allocation leaves 40 MiB at the larger capacity, but
    // only 8 MiB after shrinking: a 24 MiB decode fits in just one state.
    let held = session.reserve_allocation(24 * MIB).unwrap();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            for _ in 0..5000 {
                session.set_budget(32 * MIB);
                session.set_budget(64 * MIB);
            }
        });
        for source in 1..=5000 {
            let request = session.request(demand(1, source, 16, 8)).unwrap();
            let permit = gate.next_decode();
            session.cancel(request);
            drop(permit);
        }
    });
    drop(held);
    session.set_budget(32 * MIB);
    let request = session.request(demand(1, 5001, 16, 8)).unwrap();
    let permit = gate.next_decode().expect("the gate remains usable");
    session.cancel(request);
    drop(permit);
    assert_eq!(total(&session), 0);
}

#[test]
fn a_decode_its_view_let_go_of_before_taking_it_stays_cold_for_the_next_view() {
    let gate = Gate::new();
    let session = gate.session();
    let drops = Arc::new(AtomicUsize::new(0));
    let first = session.request(demand(1, 1, 4, 0)).unwrap();
    complete(gate.next_decode().unwrap(), &drops);
    assert_eq!(session.stats().delivery_cells, 1);
    // Its row left the screen before the picture was taken.
    assert!(session.cancel(first));
    assert_eq!(session.stats().delivery_cells, 0);
    assert_eq!(session.stats().cold_bytes, 4 * MIB);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    // Another view showing the same picture decodes nothing.
    let again = session.request(demand(2, 1, 4, 0)).unwrap();
    assert_eq!(session.status(again), Some(RequestStatus::Ready));
    assert!(gate.next_decode().is_none());
    assert_eq!(session.stats().dedup_hits, 1);
    drop(session.take_ready(again).unwrap());
}
