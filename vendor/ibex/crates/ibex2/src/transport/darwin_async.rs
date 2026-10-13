//! One `NSURLSession` per lane behind [`AsyncTransport`] (exact2 LLP 1120
//! D2): tasks with their own delegates, callbacks on the session's serial
//! queue, the upload lent without a copy. The Objective-C++ half is
//! `darwin_task.mm`.

use std::ffi::{c_char, c_int, c_uchar, c_void, CStr, CString};
use std::sync::{Arc, OnceLock};

use crate::boundary::HostError;
use crate::stdlib::fetch::Headers;
use crate::stdlib::fetch_async::{
    AsyncTransport, Exchange, Flow, Head, HopMetrics, HopRequest, Sink, Upload,
};

extern "C" {
    fn ibex2_darwin_lane_create(max_per_host: c_int) -> *mut c_void;
    fn ibex2_darwin_lane_destroy(lane: *mut c_void);
    #[cfg(test)]
    fn ibex2_darwin_lane_state(
        lane: *mut c_void,
        out_cookies: *mut c_int,
        out_cache: *mut c_int,
        out_per_host: *mut c_int,
    );
    fn ibex2_darwin_task_start(
        lane: *mut c_void,
        method: *const c_char,
        url: *const c_char,
        header_block: *const c_char,
        body: *const c_uchar,
        body_length: usize,
        body_ctx: *mut c_void,
        timeout_seconds: f64,
        refusal: *const c_char,
        ctx: *mut c_void,
    ) -> *mut c_void;
    fn ibex2_darwin_task_activate(task: *mut c_void);
    fn ibex2_darwin_task_cancel(task: *mut c_void);
    fn ibex2_darwin_task_release(task: *mut c_void);
}

/// A lane's session, created at its first exchange (the first
/// `NSURLSession` costs tens of milliseconds) and invalidated, cancelling
/// every task, when dropped.
pub struct DarwinLane {
    /// The native lane, as a `usize` so the struct stays `Send + Sync`.
    lane: OnceLock<usize>,
    max_per_host: usize,
}

impl DarwinLane {
    /// A lane whose session allows `max_per_host` connections to a host;
    /// zero keeps URLSession's default (6).
    pub fn new(max_per_host: usize) -> Self {
        Self {
            lane: OnceLock::new(),
            max_per_host,
        }
    }

    /// Whether the session exists yet.
    pub fn created(&self) -> bool {
        self.lane.get().is_some()
    }

    fn lane(&self) -> *mut c_void {
        *self.lane.get_or_init(|| {
            let max = c_int::try_from(self.max_per_host).unwrap_or(c_int::MAX);
            // SAFETY: returns a retained lane, released once in `drop`.
            unsafe { ibex2_darwin_lane_create(max) as usize }
        }) as *mut c_void
    }
}

impl Drop for DarwinLane {
    fn drop(&mut self) {
        if let Some(lane) = self.lane.get() {
            // SAFETY: came from `ibex2_darwin_lane_create`, released once.
            unsafe { ibex2_darwin_lane_destroy(*lane as *mut c_void) };
        }
    }
}

/// A prepared task, retained until the exchange's closures go.
struct Task(usize);
impl Drop for Task {
    fn drop(&mut self) {
        // SAFETY: a retained task from `ibex2_darwin_task_start`, once.
        unsafe { ibex2_darwin_task_release(self.0 as *mut c_void) };
    }
}

impl AsyncTransport for DarwinLane {
    fn start(&self, request: HopRequest, sink: Box<dyn Sink>) -> Exchange {
        let ctx = Box::into_raw(Box::new(sink)) as *mut c_void;
        let block = request
            .headers
            .entries()
            .iter()
            .map(|(name, value)| format!("{name}: {value}"))
            .collect::<Vec<_>>()
            .join("\n");
        let inputs = (
            CString::new(request.method.as_str()),
            CString::new(request.url.as_str()),
            CString::new(block),
        );
        // Text the platform can't take is refused on the lane's queue,
        // as any other failure known at once.
        let (method, url, block, refusal) = match inputs {
            (Ok(method), Ok(url), Ok(block)) => (method, url, block, None),
            _ => (
                CString::default(),
                CString::default(),
                CString::default(),
                Some(c"TypeError: invalid method, URL or header value"),
            ),
        };
        let (body, body_length, body_ctx) = match &request.body {
            Some(upload) if !upload.bytes().is_empty() && refusal.is_none() => (
                upload.bytes().as_ptr(),
                upload.bytes().len(),
                // The platform's reference: freed by the NSData deallocator.
                Box::into_raw(Box::new(upload.clone())) as *mut c_void,
            ),
            _ => (std::ptr::null(), 0, std::ptr::null_mut()),
        };
        let timeout = request.timeout.map_or(0.0, |t| t.as_secs_f64());
        // SAFETY: inputs are copied (or lent with their owner, `body_ctx`)
        // before this returns; `ctx` belongs to the native side from here,
        // which frees it once its end (and metrics) have been delivered.
        let start = unsafe {
            ibex2_darwin_task_start(
                self.lane(),
                method.as_ptr(),
                url.as_ptr(),
                block.as_ptr(),
                body,
                body_length,
                body_ctx,
                timeout,
                refusal.map_or(std::ptr::null(), |r| r.as_ptr()),
                ctx,
            )
        };
        let start = Arc::new(Task(start as usize));
        let cancel = start.clone();
        Exchange::new(
            // SAFETY: the start is retained by `Task` for each closure's life.
            move || unsafe { ibex2_darwin_task_activate(start.0 as *mut c_void) },
            move || unsafe { ibex2_darwin_task_cancel(cancel.0 as *mut c_void) },
        )
    }
}

unsafe fn text<'a>(raw: *const c_char) -> Option<&'a str> {
    (!raw.is_null()).then(|| CStr::from_ptr(raw).to_str().unwrap_or_default())
}

unsafe fn sink<'a>(ctx: *mut c_void) -> &'a mut Box<dyn Sink> {
    &mut *(ctx as *mut Box<dyn Sink>)
}

fn flow(flow: Flow) -> c_int {
    match flow {
        Flow::Continue => 0,
        Flow::Stop => 1,
        Flow::Skip => 2,
    }
}

#[no_mangle]
extern "C" fn ibex2_async_head(
    ctx: *mut c_void,
    status: c_int,
    headers: *const c_char,
    declared: i64,
) -> c_int {
    let mut parsed = Headers::new();
    // SAFETY: a NUL-terminated block the native side owns for this call.
    for line in unsafe { text(headers) }.unwrap_or_default().lines() {
        if let Some((name, value)) = line.split_once(": ") {
            parsed.set_response(name, value);
        }
    }
    let head = Head {
        status: u16::try_from(status).unwrap_or(0),
        headers: parsed,
        declared_length: u64::try_from(declared).ok(),
    };
    // SAFETY: `ctx` is a live sink, called only from its lane's queue.
    flow(unsafe { sink(ctx) }.head(head))
}

#[no_mangle]
extern "C" fn ibex2_async_data(ctx: *mut c_void, bytes: *const c_uchar, length: usize) -> c_int {
    if length == 0 {
        return 0;
    }
    // SAFETY: `bytes` spans `length` bytes for this call.
    let bytes = unsafe { std::slice::from_raw_parts(bytes, length) };
    // SAFETY: as `ibex2_async_head`.
    flow(unsafe { sink(ctx) }.data(bytes))
}

#[no_mangle]
extern "C" fn ibex2_async_end(ctx: *mut c_void, error: *const c_char) {
    // SAFETY: as `ibex2_async_head`; `error` is NUL-terminated or null.
    let result = match unsafe { text(error) } {
        None => Ok(()),
        Some(why) => Err(HostError::Failed(why.to_string())),
    };
    unsafe { sink(ctx) }.end(result);
}

#[no_mangle]
extern "C" fn ibex2_async_metrics(
    ctx: *mut c_void,
    protocol: *const c_char,
    reused: c_int,
    local: *const c_char,
    remote: *const c_char,
    queued_ms: f64,
) {
    // SAFETY: as `ibex2_async_head`.
    let owned = |raw| unsafe { text(raw) }.map(str::to_string);
    let metrics = HopMetrics {
        protocol: owned(protocol),
        reused: (reused >= 0).then_some(reused == 1),
        local: owned(local),
        remote: owned(remote),
        queued_ms: (queued_ms >= 0.0).then_some(queued_ms),
    };
    unsafe { sink(ctx) }.metrics(metrics);
}

#[no_mangle]
extern "C" fn ibex2_async_sink_free(ctx: *mut c_void) {
    // SAFETY: the box `start` made, freed once by its delegate.
    drop(unsafe { Box::from_raw(ctx as *mut Box<dyn Sink>) });
}

#[no_mangle]
extern "C" fn ibex2_async_body_free(body: *mut c_void) {
    // SAFETY: the platform's `Arc` reference `start` made, freed once by
    // the NSData deallocator.
    drop(unsafe { Box::from_raw(body as *mut Arc<Upload>) });
}

#[cfg(test)]
#[path = "darwin_async_tests.rs"]
mod tests;
