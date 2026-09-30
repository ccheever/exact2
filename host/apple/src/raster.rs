//! Thread-safe raster handles, independent of the replaceable Runtime/Host.
//! No native pointers are dereferenced here. Payload handles are transferred
//! with a worker-safe destructor; charges retain only the shared budget account.

use exact_raster::*;
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// Metadata-only C demand. Variant 1 is EXIF-transformed, sRGB RGBA8, first frame.
#[repr(C)]
#[derive(Clone, Copy, Default)]
#[allow(missing_docs)]
pub struct RasterDemand {
    pub view: u64,
    pub view_generation: u64,
    pub source: u64,
    pub generation: u64,
    pub width: u32,
    pub height: u32,
    pub natural_width: u32,
    pub natural_height: u32,
    pub priority: u32,
    pub encoded_bytes: u64,
    pub header_bytes: u64,
    pub stride: u64,
    pub scratch_bytes: u64,
}

/// A reserved decode and its preallocation charge. Zero permit means no work.
#[repr(C)]
#[derive(Default)]
#[allow(missing_docs)]
pub struct RasterWork {
    pub permit: u64,
    pub session: u64,
    pub source: u64,
    pub generation: u64,
    pub charge: u64,
    pub width: u32,
    pub height: u32,
}

/// A pinned lease and a borrowed native payload. Keep the lease while using it.
#[repr(C)]
#[derive(Default)]
#[allow(missing_docs)]
pub struct RasterReady {
    pub lease: u64,
    pub payload: u64,
}

/// Existing agent-state diagnostics; encoded bytes and platform RSS are separate.
#[repr(C)]
#[derive(Default)]
#[allow(missing_docs)]
pub struct RasterStats {
    pub resident_bytes: u64,
    pub reserved_bytes: u64,
    pub pinned_bytes: u64,
    pub cold_bytes: u64,
    pub retiring_bytes: u64,
    pub peak_bytes: u64,
    pub queued: u64,
    pub running: u64,
    pub ready: u64,
    pub delivery_cells: u64,
    pub pending_jobs: u64,
    pub subscribers: u64,
    pub cold_entries: u64,
    pub dedup_hits: u64,
    pub cancelled: u64,
    pub evicted: u64,
    pub process_running: u64,
    pub last_refusal: u64,
    pub waiting_budget: u64,
}

struct NativePayload {
    handle: u64,
    release: extern "C" fn(u64),
}
impl Drop for NativePayload {
    fn drop(&mut self) {
        (self.release)(self.handle);
    }
}
struct Session {
    core: RasterSession,
    requests: BTreeMap<u64, RequestId>,
    views: BTreeMap<ViewKey, u64>,
    last_refusal: u32,
}
#[derive(Default)]
struct Handles {
    serial: u64,
    sessions: BTreeMap<u64, Session>,
    permits: BTreeMap<u64, DecodePermit>,
    charges: BTreeMap<u64, AllocationCharge>,
    leases: BTreeMap<u64, RasterLease>,
}
impl Handles {
    fn id(&mut self) -> u64 {
        self.serial = self
            .serial
            .checked_add(1)
            .expect("Apple raster handle exhaustion");
        self.serial
    }
}
fn handles() -> &'static Mutex<Handles> {
    static HANDLES: OnceLock<Mutex<Handles>> = OnceLock::new();
    HANDLES.get_or_init(Mutex::default)
}
fn session(id: u64) -> Option<RasterSession> {
    handles()
        .lock()
        .unwrap()
        .sessions
        .get(&id)
        .map(|s| s.core.clone())
}
fn interest(session: u64, id: u64) -> Option<(RasterSession, RequestId)> {
    let h = handles().lock().unwrap();
    let s = h.sessions.get(&session)?;
    Some((s.core.clone(), *s.requests.get(&id)?))
}

/// Creates a session account above active/candidate/retiring Host instances,
/// holding up to `budget` decoded bytes (never less than `SESSION_BYTES`).
pub fn session_create(budget: u64) -> u64 {
    let core = Gate::process().session_with_budget(budget);
    let id = core.id();
    handles().lock().unwrap().sessions.insert(
        id,
        Session {
            core,
            requests: BTreeMap::new(),
            views: BTreeMap::new(),
            last_refusal: 0,
        },
    );
    id
}

/// Update one session's capacity while preserving its live backing owners.
pub fn session_budget(id: u64, budget: u64) {
    if let Some(core) = session(id) {
        core.set_budget(budget);
    }
}

/// 0 reset, 1 pause, 2 resume, 3 shutdown, 4 trim. Charge handles survive all.
pub fn session_control(id: u64, op: u32) {
    let Some(core) = session(id) else { return };
    match op {
        0 => core.reset(),
        1 => core.pause(),
        2 => core.resume(),
        3 => core.shutdown(),
        4 => core.trim(),
        _ => return,
    }
    let removed = {
        let mut h = handles().lock().unwrap();
        if op == 3 {
            h.sessions.remove(&id)
        } else {
            if op < 2 {
                if let Some(s) = h.sessions.get_mut(&id) {
                    s.requests.clear();
                    s.views.clear();
                }
            }
            None
        }
    };
    drop(removed);
}

/// Register one independently cancellable interest; zero is observable refusal.
pub fn request(id: u64, d: RasterDemand) -> u64 {
    let Some(core) = session(id) else { return 0 };
    let cost = match DecodeCost::checked(d.stride, d.height, d.scratch_bytes, 0) {
        Ok(cost) => cost,
        Err(reason) => {
            refused(id, reason);
            return 0;
        }
    };
    let demand = Demand {
        view: ViewKey {
            view: d.view,
            generation: d.view_generation,
        },
        key: RasterKey {
            source: d.source,
            generation: d.generation,
            pixels: PixelSize {
                width: d.width,
                height: d.height,
            },
            variant: 1,
        },
        metadata: Metadata {
            natural: PixelSize {
                width: d.natural_width,
                height: d.natural_height,
            },
            encoded_bytes: d.encoded_bytes,
            header_bytes: d.header_bytes,
        },
        cost,
        priority: if d.priority == 0 {
            Priority::Visible
        } else {
            Priority::Overscan
        },
    };
    let request = match core.request(demand) {
        Ok(request) => request,
        Err(reason) => {
            refused(id, reason);
            return 0;
        }
    };
    let mut h = handles().lock().unwrap();
    if let Some(s) = h.sessions.get_mut(&id) {
        if let Some(old) = s.views.insert(demand.view, request.get()) {
            s.requests.remove(&old);
        }
        s.last_refusal = 0;
        s.requests.insert(request.get(), request);
        request.get()
    } else {
        drop(h);
        core.cancel(request);
        0
    }
}

/// Retire one observer without affecting another observer of the same raster.
pub fn cancel(id: u64, request: u64) {
    let item = {
        let mut h = handles().lock().unwrap();
        h.sessions.get_mut(&id).and_then(|s| {
            s.requests.remove(&request).map(|r| {
                s.views.retain(|_, id| *id != request);
                (s.core.clone(), r)
            })
        })
    };
    if let Some((core, request)) = item {
        core.cancel(request);
    }
}

/// 0 absent, 1 queued, 2 budget wait, 3 decoding, 4 ready; 100+refusal on failure.
pub fn status(id: u64, request: u64) -> u32 {
    let Some((core, request)) = interest(id, request) else {
        return 0;
    };
    match core.status(request) {
        None => 0,
        Some(RequestStatus::Queued) => 1,
        Some(RequestStatus::WaitingBudget) => 2,
        Some(RequestStatus::Decoding) => 3,
        Some(RequestStatus::Ready) => 4,
        Some(RequestStatus::Failed(reason)) => 100 + refusal_code(reason),
    }
}

/// Worker-only bounded wait. The returned charge must enter the pixel provider
/// BEFORE allocation; dropping the Runtime never invalidates this handle.
pub fn next_decode(timeout_ms: u32) -> RasterWork {
    let gate = Gate::process();
    let permit = if timeout_ms == 0 {
        gate.next_decode()
    } else {
        gate.wait_decode(Duration::from_millis(u64::from(timeout_ms.min(1000))))
    };
    let Some(permit) = permit else {
        return RasterWork::default();
    };
    let key = permit.key();
    let charge = permit.allocation_charge();
    let mut h = handles().lock().unwrap();
    let charge_id = h.id();
    h.charges.insert(charge_id, charge);
    let permit_id = h.id();
    let work = RasterWork {
        permit: permit_id,
        session: permit.session_id(),
        source: key.source,
        generation: key.generation,
        charge: charge_id,
        width: key.pixels.width,
        height: key.pixels.height,
    };
    h.permits.insert(permit_id, permit);
    work
}

/// Query cancellation before allocating/decoding; cancellation does not refund
/// a running worker's buffers or permit before that worker unwinds.
pub fn is_cancelled(id: u64) -> u32 {
    u32::from(
        handles()
            .lock()
            .unwrap()
            .permits
            .get(&id)
            .is_none_or(DecodePermit::is_cancelled),
    )
}

/// Transfer a sole retained immutable native owner, with a worker-safe release
/// callback. Consumes payload even when stale. All decoder scratch is gone.
pub fn complete(id: u64, payload: u64, release: Option<extern "C" fn(u64)>, bytes: u64) -> u32 {
    let permit = handles().lock().unwrap().permits.remove(&id);
    let Some(release) = release else {
        drop(permit);
        return 0;
    };
    let payload = NativePayload {
        handle: payload,
        release,
    };
    let Some(permit) = permit else {
        drop(payload);
        return 0;
    };
    u32::from(
        permit
            .complete(
                payload,
                ResidentBytes {
                    output: bytes,
                    copy: 0,
                },
            )
            .unwrap_or(false),
    )
}

/// Worker failure, only after dropping scratch/pixels; charge release is separate.
pub fn fail(id: u64) {
    let permit = handles().lock().unwrap().permits.remove(&id);
    drop(permit);
}

/// Last provider alias calls this on any thread, after freeing its pixel buffer.
pub fn charge_release(id: u64) {
    let charge = handles().lock().unwrap().charges.remove(&id);
    drop(charge);
}

/// Take one ready interest, releasing its session delivery cell. The pinned
/// lease remains until the native view relinquishes it.
pub fn take_ready(id: u64, request: u64) -> RasterReady {
    let Some((core, request)) = interest(id, request) else {
        return RasterReady::default();
    };
    let Some(lease) = core.take_ready(request) else {
        return RasterReady::default();
    };
    let Some(payload) = lease.payload::<NativePayload>() else {
        return RasterReady::default();
    };
    let payload = payload.handle;
    let mut h = handles().lock().unwrap();
    let id = h.id();
    h.leases.insert(id, lease);
    RasterReady { lease: id, payload }
}

/// Release a native view lease. Payload and charge drops happen outside locks.
pub fn lease_release(id: u64) {
    let lease = handles().lock().unwrap().leases.remove(&id);
    drop(lease);
}

fn refused(id: u64, reason: Refusal) {
    if let Some(s) = handles().lock().unwrap().sessions.get_mut(&id) {
        s.last_refusal = refusal_code(reason);
    }
}
fn refusal_code(reason: Refusal) -> u32 {
    match reason {
        Refusal::Overflow => 1,
        Refusal::InvalidDimensions => 2,
        Refusal::EncodedLimit => 3,
        Refusal::HeaderLimit => 4,
        Refusal::SourcePixels => 5,
        Refusal::TooLarge => 6,
        Refusal::Budget => 7,
        Refusal::QueueFull => 8,
        Refusal::SubscriberLimit => 9,
        Refusal::ConflictingMetadata => 10,
        Refusal::Paused => 11,
        Refusal::Shutdown => 12,
        Refusal::Stale => 13,
        Refusal::ActualExceedsReservation => 14,
        Refusal::DecodeFailed => 15,
    }
}

/// Session accounting including old providers and running cancelled workers.
pub fn stats(id: u64) -> RasterStats {
    let Some(core) = session(id) else {
        return RasterStats::default();
    };
    let s = core.stats();
    let last_refusal = handles()
        .lock()
        .unwrap()
        .sessions
        .get(&id)
        .map_or(0, |s| s.last_refusal);
    let requests: Vec<_> = handles()
        .lock()
        .unwrap()
        .sessions
        .get(&id)
        .map(|s| s.requests.values().copied().collect())
        .unwrap_or_default();
    let waiting_budget = requests
        .into_iter()
        .filter(|request| matches!(core.status(*request), Some(RequestStatus::WaitingBudget)))
        .count() as u64;
    RasterStats {
        resident_bytes: s.resident_bytes,
        reserved_bytes: s.reserved_bytes,
        pinned_bytes: s.pinned_bytes,
        cold_bytes: s.cold_bytes,
        retiring_bytes: s.retiring_bytes,
        peak_bytes: s.peak_bytes,
        queued: s.queued as u64,
        running: s.running as u64,
        ready: s.ready as u64,
        delivery_cells: s.delivery_cells as u64,
        pending_jobs: s.pending_jobs as u64,
        subscribers: s.subscribers as u64,
        cold_entries: s.cold_entries as u64,
        dedup_hits: s.dedup_hits,
        cancelled: s.cancelled,
        evicted: s.evicted,
        process_running: Gate::process().stats().running as u64,
        last_refusal: u64::from(last_refusal),
        waiting_budget,
    }
}

#[cfg(test)]
#[path = "raster_tests.rs"]
mod tests;
