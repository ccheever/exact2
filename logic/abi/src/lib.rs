//! Versioned, bounded data-only module seam. No Rust allocation or layout crosses it.
//! @ref LLP 1029.000 — Rust logic replacement after first pixel.

use exact_plan::{
    bytes::{Reader, Writer},
    Plan, Value,
};
use exact_runner::{
    Answer, DataError, DataSource, FailureKind, HttpScheduling, Outcome, Request, Response, Store,
    SurfaceOutcome, SurfaceRequest,
};

pub mod draw;

/// Seam version, independent of the plan format version.
pub const ABI: u32 = 3;
/// Maximum request or response bytes, enforced on both sides.
pub const MAX_MESSAGE: usize = exact_runner::MAX_HOST_WORK_BYTES;

fn error(e: impl std::fmt::Debug) -> String {
    format!("logic ABI: {e:?}")
}
fn bytes(w: &mut Writer, data: &[u8]) {
    w.u32(data.len() as u32);
    w.bytes(data);
}
fn read_bytes<'a>(r: &mut Reader<'a>) -> Result<&'a [u8], String> {
    let n = r.count().map_err(error)?;
    r.bytes(n).map_err(error)
}
fn pairs(w: &mut Writer, data: &[(String, String)]) {
    w.u32(data.len() as u32);
    for (a, b) in data {
        w.string(a);
        w.string(b);
    }
}
fn read_pairs(r: &mut Reader<'_>) -> Result<Vec<(String, String)>, String> {
    let n = r.count().map_err(error)?;
    (0..n)
        .map(|_| Ok((r.string().map_err(error)?, r.string().map_err(error)?)))
        .collect()
}
fn finish(w: Writer) -> Result<Vec<u8>, String> {
    if w.len() > MAX_MESSAGE {
        Err("logic ABI message too large".into())
    } else {
        Ok(w.into_vec())
    }
}
fn header(op: u8) -> Writer {
    let mut w = Writer::default();
    w.u32(ABI);
    w.u8(op);
    w
}
fn reader(data: &[u8]) -> Result<Reader<'_>, String> {
    if data.len() > MAX_MESSAGE {
        return Err("logic ABI message too large".into());
    }
    let mut r = Reader::new(data);
    if r.u32().map_err(error)? != ABI {
        return Err("logic ABI version differs".into());
    }
    Ok(r)
}
fn end(r: &Reader<'_>) -> Result<(), String> {
    if r.is_empty() {
        Ok(())
    } else {
        Err("logic ABI trailing bytes".into())
    }
}

/// Read-only metadata request; creating a session and dispatching it is post-pixel.
pub fn metadata_request() -> Vec<u8> {
    header(0).into_vec()
}
/// Bind the plan's declared source shapes before activation.
pub fn bind_request(plan: &[u8]) -> Result<Vec<u8>, String> {
    let mut w = header(1);
    bytes(&mut w, plan);
    finish(w)
}
/// Activate the app's data implementation.
pub fn activate_request() -> Vec<u8> {
    header(2).into_vec()
}
/// Encode an answer or parse operation with a grant-filtered snapshot.
pub fn call_request(
    store: &Store,
    source: &str,
    args: &[Value],
    outcome: Option<&Outcome>,
) -> Result<Vec<u8>, String> {
    let mut w = header(if outcome.is_some() { 4 } else { 3 });
    w.string(source);
    Value::list(args.to_vec()).encode(&mut w);
    let snapshot = store
        .snapshot()
        .into_iter()
        .filter(|(name, _)| !name.starts_with(Store::KEPT) && store.granted().contains(name))
        .collect::<Vec<_>>();
    pairs(&mut w, &snapshot);
    if let Some(outcome) = outcome {
        encode_outcome(&mut w, outcome);
    }
    finish(w)
}

fn encode_outcome(w: &mut Writer, outcome: &Outcome) {
    match outcome {
        Outcome::Storage(payload) => {
            w.u8(5);
            bytes(w, payload);
        }
        Outcome::Surface(SurfaceOutcome::Captured(payload)) => {
            w.u8(6);
            bytes(w, payload);
        }
        Outcome::Surface(SurfaceOutcome::Restored) => w.u8(7),
        Outcome::Response(r) => {
            w.u8(0);
            w.u16(r.status);
            pairs(w, &r.headers);
            bytes(w, &r.body);
        }
        // One message of a stream (LLP 1016.000); additive, as tag 6 was.
        Outcome::Message(m) => {
            w.u8(8);
            w.string(&m.event);
            w.string(&m.id);
            w.string(&m.data);
            w.u32(m.coalesced);
        }
        Outcome::Failed { kind, message } => {
            w.u8(match kind {
                FailureKind::Network => 1,
                FailureKind::Refused => 2,
                FailureKind::Unsupported => 3,
                FailureKind::Aborted => 4,
            });
            w.string(message);
        }
    }
}
fn read_outcome(r: &mut Reader<'_>) -> Result<Outcome, String> {
    let tag = r.u8().map_err(error)?;
    if tag == 0 {
        return Ok(Outcome::Response(Response {
            status: r.u16().map_err(error)?,
            headers: read_pairs(r)?,
            body: read_bytes(r)?.to_vec(),
        }));
    }
    if tag == 5 {
        return Ok(Outcome::Storage(read_bytes(r)?.to_vec()));
    }
    if tag == 6 {
        return Ok(Outcome::Surface(SurfaceOutcome::Captured(
            read_bytes(r)?.to_vec(),
        )));
    }
    if tag == 7 {
        return Ok(Outcome::Surface(SurfaceOutcome::Restored));
    }
    if tag == 8 {
        return Ok(Outcome::Message(exact_runner::Message {
            event: r.string().map_err(error)?,
            id: r.string().map_err(error)?,
            data: r.string().map_err(error)?,
            coalesced: r.u32().map_err(error)?,
        }));
    }
    let kind = match tag {
        1 => FailureKind::Network,
        2 => FailureKind::Refused,
        3 => FailureKind::Unsupported,
        4 => FailureKind::Aborted,
        _ => return Err("invalid outcome tag".into()),
    };
    Ok(Outcome::Failed {
        kind,
        message: r.string().map_err(error)?,
    })
}
fn encode_result(w: &mut Writer, result: Result<Answer, DataError>) {
    match result {
        Ok(Answer::Later(r))
            if (r.storage.is_some() || r.continuation.is_some() || r.surface.is_some())
                && r.http != HttpScheduling::Ordered =>
        {
            encode_result(
                w,
                Err(DataError::Unavailable(
                    "independent scheduling is HTTP-only".into(),
                )),
            );
        }
        Ok(Answer::Now(v)) => {
            w.u8(0);
            v.encode(w);
        }
        Ok(Answer::Later(r))
            if r.continuation.is_none()
                && r.storage.is_none()
                && r.surface.is_some()
                && r.method == "GET"
                && r.url.is_empty()
                && r.headers.is_empty()
                && r.body.is_empty() =>
        {
            match r.surface.as_deref().unwrap() {
                SurfaceRequest::Capture { name } => {
                    w.u8(7);
                    w.string(name);
                }
                SurfaceRequest::Restore {
                    name,
                    bytes: payload,
                } => {
                    w.u8(8);
                    w.string(name);
                    bytes(w, payload);
                }
            }
            w.u8(u8::from(r.grants.is_some()));
            w.string(r.grants.as_deref().unwrap_or(""));
        }
        Ok(Answer::Later(r))
            if r.continuation.is_none() && r.surface.is_none() && r.storage.is_some() =>
        {
            w.u8(5);
            bytes(w, r.storage.as_ref().unwrap());
            w.u8(u8::from(r.grants.is_some()));
            w.string(r.grants.as_deref().unwrap_or(""));
        }
        Ok(Answer::Later(r))
            if r.continuation.is_none() && r.surface.is_none() && r.storage.is_none() =>
        {
            // Additive HTTP result kind. Older hosts reject tag 6 before
            // executing effects; it must never silently decode as ordered.
            // A stream (tag 9) is always independent (LLP 1016.000).
            match r.http {
                HttpScheduling::Ordered => w.u8(1),
                HttpScheduling::Independent { max_response_bytes } => {
                    w.u8(if r.stream { 9 } else { 6 });
                    w.u32(max_response_bytes);
                }
            }
            w.u8(u8::from(r.grants.is_some()));
            w.string(r.grants.as_deref().unwrap_or(""));
            w.string(&r.method);
            w.string(&r.url);
            pairs(w, &r.headers);
            bytes(w, &r.body);
        }
        Ok(Answer::Later(r)) if r.continuation.is_some() => encode_result(
            w,
            Err(DataError::Unavailable(
                "executor-local continuations cannot cross the Rust module seam".into(),
            )),
        ),
        Ok(Answer::Later(_)) => encode_result(
            w,
            Err(DataError::Unavailable(
                "request combines multiple host-work kinds".into(),
            )),
        ),
        Err(e) => {
            let (tag, message) = match e {
                DataError::UnknownSource(s) => (2, s),
                DataError::BadArguments(s) => (3, s),
                DataError::Unavailable(s)
                | DataError::Interface(s)
                | DataError::DeferredAtBake(s) => (4, s),
            };
            w.u8(tag);
            w.string(&message);
        }
    }
}
fn read_result(r: &mut Reader<'_>) -> Result<Result<Answer, DataError>, String> {
    let tag = r.u8().map_err(error)?;
    Ok(match tag {
        0 => Ok(Answer::Now(Value::decode(r).map_err(error)?)),
        1 | 6 | 9 => Ok(Answer::Later(Request {
            stream: tag == 9,
            http: if tag != 1 {
                let limit = r.u32().map_err(error)?;
                if limit == 0 || limit > 64 << 20 {
                    return Err("invalid independent HTTP response limit".into());
                }
                HttpScheduling::Independent {
                    max_response_bytes: limit,
                }
            } else {
                HttpScheduling::Ordered
            },
            continuation: None,
            storage: None,
            surface: None,
            grants: {
                let present = r.u8().map_err(error)?;
                let s = r.string().map_err(error)?;
                match present {
                    0 => None,
                    1 => Some(s),
                    _ => return Err("invalid grant scope tag".into()),
                }
            },
            method: r.string().map_err(error)?,
            url: r.string().map_err(error)?,
            headers: read_pairs(r)?,
            body: read_bytes(r)?.to_vec(),
        })),
        5 => {
            let payload = read_bytes(r)?.to_vec();
            let present = r.u8().map_err(error)?;
            let scope = r.string().map_err(error)?;
            let mut request = Request::storage(payload);
            request.grants = match present {
                0 => None,
                1 => Some(scope),
                _ => return Err("invalid grant scope tag".into()),
            };
            Ok(Answer::Later(request))
        }
        tag @ (7 | 8) => {
            let name = r.string().map_err(error)?;
            let payload = if tag == 8 {
                Some(read_bytes(r)?.to_vec())
            } else {
                None
            };
            let present = r.u8().map_err(error)?;
            let scope = r.string().map_err(error)?;
            let mut request = match payload {
                Some(bytes) => Request::restore_surface(name, bytes),
                None => Request::capture_surface(name),
            };
            request.grants = match present {
                0 => None,
                1 => Some(scope),
                _ => return Err("invalid grant scope tag".into()),
            };
            Ok(Answer::Later(request))
        }
        2 => Err(DataError::UnknownSource(r.string().map_err(error)?)),
        3 => Err(DataError::BadArguments(r.string().map_err(error)?)),
        4 => Err(DataError::Unavailable(r.string().map_err(error)?)),
        _ => return Err("invalid result tag".into()),
    })
}

/// Decode identity exported by the actual instantiated module.
pub fn metadata_reply(data: &[u8]) -> Result<(String, String), String> {
    let mut r = reader(data)?;
    if r.u8().map_err(error)? != 0 {
        return Err("expected metadata reply".into());
    }
    let result = (r.string().map_err(error)?, r.string().map_err(error)?);
    end(&r)?;
    Ok(result)
}
/// Decode a bind or activate acknowledgement.
pub fn unit_reply(data: &[u8]) -> Result<(), String> {
    let mut r = reader(data)?;
    if r.u8().map_err(error)? != 1 {
        return Err("expected unit reply".into());
    }
    match read_result(&mut r)? {
        Ok(Answer::Now(Value::Unit)) => {}
        other => return Err(error(other)),
    };
    end(&r)
}
/// Validate the complete reply before applying its ordered, grant-checked writes.
/// An observed read remains observed even when the module returns a data error.
pub fn call_reply(data: &[u8], store: &mut Store) -> Result<Answer, DataError> {
    let decode = || -> Result<_, String> {
        let mut r = reader(data)?;
        if r.u8().map_err(error)? != 2 {
            return Err("expected call reply".into());
        }
        let observed = match r.u8().map_err(error)? {
            0 => false,
            1 => true,
            _ => return Err("invalid read flag".into()),
        };
        let count = r.count().map_err(error)?;
        let mut writes = Vec::new();
        for _ in 0..count {
            let name = r.string().map_err(error)?;
            if name.starts_with(Store::KEPT) || !store.granted().contains(&name) {
                return Err(format!("module wrote ungranted secret {name}"));
            }
            let value = match r.u8().map_err(error)? {
                0 => None,
                1 => Some(r.string().map_err(error)?),
                _ => return Err("invalid write tag".into()),
            };
            writes.push((name, value));
        }
        let result = read_result(&mut r)?;
        end(&r)?;
        Ok((observed, writes, result))
    };
    let (observed, writes, result) = decode().map_err(DataError::Unavailable)?;
    if observed {
        store.observe_external_read();
    }
    for (name, value) in writes {
        match value {
            Some(value) => store.set(&name, &value)?,
            None => store.forget(&name)?,
        };
    }
    result
}

/// A module-owned session; used by the export macro, useful for codec fixtures.
pub struct Session<D> {
    data: D,
    output: Vec<u8>,
}
impl<D: DataSource> Session<D> {
    /// Construct one data source for one host session.
    pub fn new(data: D) -> Self {
        Self {
            data,
            output: Vec::new(),
        }
    }
    /// Borrow the last output until the next dispatch.
    pub fn output(&self) -> &[u8] {
        &self.output
    }
    /// Dispatch one bounded byte request; no filesystem host capabilities exist.
    pub fn dispatch(&mut self, input: &[u8]) -> Result<(), String> {
        let mut r = reader(input)?;
        let op = r.u8().map_err(error)?;
        let mut w = header(match op {
            0 => 0,
            1 | 2 => 1,
            3 | 4 => 2,
            _ => return Err("unknown logic operation".into()),
        });
        match op {
            0 => {
                end(&r)?;
                w.string(self.data.app_id());
                w.string(self.data.grants());
            }
            1 => {
                let plan = Plan::decode(read_bytes(&mut r)?).map_err(error)?;
                end(&r)?;
                self.data.bind(&plan);
                encode_result(&mut w, Ok(Answer::Now(Value::Unit)));
            }
            2 => {
                end(&r)?;
                encode_result(
                    &mut w,
                    self.data.activate().map(|_| Answer::Now(Value::Unit)),
                );
            }
            3 | 4 => {
                let source = r.string().map_err(error)?;
                let Value::List(args) = Value::decode(&mut r).map_err(error)? else {
                    return Err("arguments must be a list".into());
                };
                let mut store = Store::module_mirror(self.data.grants(), read_pairs(&mut r)?);
                let outcome = if op == 4 {
                    Some(read_outcome(&mut r)?)
                } else {
                    None
                };
                end(&r)?;
                let result = if let Some(outcome) = outcome {
                    self.data.parse(&mut store, &source, &args, outcome)
                } else {
                    self.data.answer(&mut store, &source, &args)
                };
                w.u8(u8::from(store.reads() > 0));
                let writes = store.take_writes();
                w.u32(writes.len() as u32);
                for write in writes {
                    w.string(&write.name);
                    w.u8(u8::from(write.value.is_some()));
                    if let Some(value) = write.value {
                        w.string(&value);
                    }
                }
                encode_result(&mut w, result);
            }
            _ => unreachable!(),
        }
        self.output = finish(w)?;
        Ok(())
    }
}

/// Export this app's data-only module as a native cdylib or standalone wasm.
/// `$new` constructs fresh state per session. Call only on the session's thread.
/// Input allocations must be freed by this module's deallocator, output is borrowed.
#[macro_export]
macro_rules! export {
    ($data:ty, $new:expr) => {
        $crate::export!(@exports $data, $new, false);
    };
    // Declares that all observable state lives in host inputs/Store/requests.
    // Construction, binding, activation and destruction must be effect-free;
    // parse must depend only on its explicit arguments/outcome. Derived caches
    // may be rebuilt. This is an author contract, not a proof by the runtime.
    ($data:ty, $new:expr, stateless) => {
        $crate::export!(@exports $data, $new, true);
    };
    (@exports $data:ty, $new:expr, $stateless:expr) => {
        #[no_mangle]
        pub extern "C" fn exact_logic_stateless() -> u32 {
            u32::from($stateless)
        }
        #[no_mangle]
        pub extern "C" fn exact_logic_abi() -> u32 {
            $crate::ABI
        }
        #[no_mangle]
        pub extern "C" fn exact_logic_create() -> usize {
            Box::into_raw(Box::new($crate::Session::<$data>::new($new))) as usize
        }
        #[no_mangle]
        #[allow(clippy::missing_safety_doc)]
        pub unsafe extern "C" fn exact_logic_destroy(session: usize) {
            if session != 0 {
                drop(Box::from_raw(session as *mut $crate::Session<$data>));
            }
        }
        #[no_mangle]
        pub extern "C" fn exact_logic_alloc(len: u32) -> usize {
            if len as usize > $crate::MAX_MESSAGE {
                return 0;
            }
            Box::into_raw(vec![0u8; len as usize].into_boxed_slice()) as *mut u8 as usize
        }
        #[no_mangle]
        #[allow(clippy::missing_safety_doc)]
        pub unsafe extern "C" fn exact_logic_dealloc(ptr: usize, len: u32) {
            if ptr != 0 {
                drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                    ptr as *mut u8,
                    len as usize,
                )));
            }
        }
        #[no_mangle]
        #[allow(clippy::missing_safety_doc)]
        pub unsafe extern "C" fn exact_logic_call(session: usize, ptr: usize, len: u32) -> u32 {
            if session == 0 || ptr == 0 || len as usize > $crate::MAX_MESSAGE {
                return 1;
            }
            let input = std::slice::from_raw_parts(ptr as *const u8, len as usize);
            u32::from(
                (&mut *(session as *mut $crate::Session<$data>))
                    .dispatch(input)
                    .is_err(),
            )
        }
        #[no_mangle]
        #[allow(clippy::missing_safety_doc)]
        pub unsafe extern "C" fn exact_logic_output(session: usize) -> usize {
            (&*(session as *const $crate::Session<$data>))
                .output()
                .as_ptr() as usize
        }
        #[no_mangle]
        #[allow(clippy::missing_safety_doc)]
        pub unsafe extern "C" fn exact_logic_output_len(session: usize) -> u32 {
            (&*(session as *const $crate::Session<$data>))
                .output()
                .len() as u32
        }
    };
}

#[cfg(test)]
mod tests;
