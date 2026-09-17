use crate::raster::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static DROPS: AtomicUsize = AtomicUsize::new(0);
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
extern "C" fn release(_: u64) {
    DROPS.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn provider_charge_outlives_session_and_duplicate_stale_delivery() {
    let _serial = SERIAL.lock().unwrap();
    let session = session_create();
    let demand = RasterDemand {
        view: 7,
        view_generation: 1,
        source: 9,
        generation: 1,
        width: 100,
        height: 50,
        natural_width: 4000,
        natural_height: 2000,
        encoded_bytes: 1000,
        header_bytes: 1000,
        stride: 448,
        scratch_bytes: 65536,
        priority: 0,
    };
    let first = request(session, demand);
    let second = request(session, RasterDemand { view: 8, ..demand });
    assert_ne!(first, 0);
    assert_ne!(first, second);
    let work = next_decode(0);
    assert_eq!(work.source, 9);
    assert_ne!(work.charge, 0);
    assert_eq!(complete(work.permit, 77, Some(release), 22400), 1);
    let taken = take_ready(session, first);
    let shared = take_ready(session, second);
    assert_eq!(taken.payload, 77);
    assert_eq!(shared.payload, 77);
    assert_eq!(stats(session).resident_bytes, 22400);
    session_control(session, 0); // reset must not zero provider accounting
    lease_release(taken.lease);
    assert_eq!(DROPS.load(Ordering::SeqCst), 0);
    lease_release(shared.lease);
    assert_eq!(DROPS.load(Ordering::SeqCst), 1);
    assert_eq!(stats(session).resident_bytes, 22400);
    charge_release(work.charge);
    charge_release(work.charge); // opaque handles are never recycled
    assert_eq!(stats(session).resident_bytes, 0);
    session_control(session, 3);
    assert_eq!(request(session, demand), 0);
    assert_eq!(take_ready(session, first).lease, 0);
    // A stale completion still transfers/destroys its own payload exactly once.
    assert_eq!(complete(work.permit, 88, Some(release), 22400), 0);
    assert_eq!(DROPS.load(Ordering::SeqCst), 2);
}

#[test]
fn accepted_same_view_replacement_bounds_ffi_ids_and_refused_keeps_old() {
    let _serial = SERIAL.lock().unwrap();
    let session = session_create();
    let mut d = RasterDemand {
        view: 1234,
        view_generation: 5,
        source: 1234,
        generation: 1,
        width: 1,
        height: 1,
        natural_width: 1,
        natural_height: 1,
        encoded_bytes: 20,
        header_bytes: 20,
        stride: 4,
        scratch_bytes: 4,
        priority: 1,
    };
    let mut current = 0;
    for _ in 0..4000 {
        d.generation += 1;
        let previous = current;
        current = request(session, d);
        assert_ne!(current, 0);
        assert_ne!(current, previous);
        assert_eq!(status(session, previous), 0);
        assert_eq!(
            super::handles().lock().unwrap().sessions[&session]
                .requests
                .len(),
            1
        );
    }
    d.width = 0;
    assert_eq!(request(session, d), 0);
    assert_ne!(status(session, current), 0);
    assert_eq!(stats(session).last_refusal, 2);
    session_control(session, 3);
}

#[test]
fn native_destructor_reenters_charge_release_after_session_shutdown() {
    let _serial = SERIAL.lock().unwrap();
    extern "C" fn release_charge(charge: u64) {
        charge_release(charge);
    }
    let session = session_create();
    let d = RasterDemand {
        view: 9000,
        view_generation: 1,
        source: 9000,
        generation: 1,
        width: 1,
        height: 1,
        natural_width: 1,
        natural_height: 1,
        encoded_bytes: 20,
        header_bytes: 20,
        stride: 4,
        scratch_bytes: 4,
        priority: 0,
    };
    let request = request(session, d);
    let work = next_decode(0);
    assert_eq!(
        complete(work.permit, work.charge, Some(release_charge), 4),
        1
    );
    let lease = take_ready(session, request);
    session_control(session, 3);
    lease_release(lease.lease);
    assert!(!super::handles()
        .lock()
        .unwrap()
        .charges
        .contains_key(&work.charge));
}
