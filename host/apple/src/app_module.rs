//! The session's app module on Apple (LLP 1067.000): the Swift half is the
//! app's module artifact, which `ExactKit` loads and owns per session; this
//! half turns a source's long native call into a call to it and takes its
//! answer. It is installed in the source's native slot as the host's
//! handler, so it outlives activations (Q6) and reaches a worker-built
//! instance through the shared slot.
#![allow(unsafe_code)] // One boxed reply crosses the C seam and comes back once.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::ffi::c_void;

/// `later(ctx, body, len, reply)`: a long native call's JSON body, on the
/// executor's thread. The host copies the body, dispatches the call to the
/// main thread (Q5) and returns; `reply` is answered exactly once, from
/// any thread, with `exact_app_reply` ([`reply`]).
pub type LaterFn = extern "C" fn(*mut c_void, *const u8, usize, *mut c_void);

/// The handler for a session's app module behind `later` and its `ctx`.
pub fn handler(later: LaterFn, ctx: *mut c_void) -> exact_runner::NativeHandler {
    let ctx = ctx as usize;
    std::sync::Arc::new(move |body: Vec<u8>, reply: exact_runner::Reply| {
        let reply = Box::into_raw(Box::new(reply)) as *mut c_void;
        later(ctx as *mut c_void, body.as_ptr(), body.len(), reply);
    })
}

/// Answer one long native call: `status` 200 carries the JSON reply, any
/// other status a refusal message that rejects the TypeScript promise.
/// Consumes `reply`, which came from a [`LaterFn`] call and has not been
/// answered; `bytes` is null or valid for `len` bytes. The Swift side calls
/// it once per `later` (`exact_app_reply`, [`app_module_exports!`]).
pub fn reply(reply: *mut c_void, status: u32, bytes: *const u8, len: usize) {
    if reply.is_null() {
        return;
    }
    // SAFETY: the caller hands back the box `handler` made, once.
    let reply = unsafe { Box::from_raw(reply as *mut exact_runner::Reply) };
    let body = if bytes.is_null() || len == 0 {
        Vec::new()
    } else {
        // SAFETY: the caller passes `len` valid bytes for this call.
        unsafe { std::slice::from_raw_parts(bytes, len) }.to_vec()
    };
    reply.send(exact_runner::Outcome::Response(exact_runner::Response {
        status: u16::try_from(status).unwrap_or(500),
        headers: Vec::new(),
        body,
    }));
}

/// A topic's UTF-8 from the C seam: `bytes` is null or valid for `len` bytes.
pub fn text(bytes: *const u8, len: usize) -> String {
    if bytes.is_null() {
        return String::new();
    }
    // SAFETY: the caller passes `len` valid bytes for this call.
    String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(bytes, len) }).into_owned()
}

/// Export the app module's reply from the application's static archive.
#[macro_export]
macro_rules! app_module_exports {
    () => {
        /// Answer one long native call, once (`include/exact.h`).
        #[no_mangle]
        pub extern "C" fn exact_app_reply(
            reply: *mut ::std::ffi::c_void,
            status: u32,
            bytes: *const u8,
            len: usize,
        ) {
            $crate::app_module::reply(reply, status, bytes, len)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    extern "C" fn answer(ctx: *mut c_void, body: *const u8, len: usize, reply: *mut c_void) {
        assert_eq!(ctx as usize, 7);
        let body = unsafe { std::slice::from_raw_parts(body, len) }.to_vec();
        let reply = reply as usize;
        std::thread::spawn(move || {
            let text = format!("{{\"echo\":{}}}", String::from_utf8(body).unwrap());
            super::reply(reply as *mut c_void, 200, text.as_ptr(), text.len());
        });
    }

    #[test]
    fn a_long_call_reaches_the_app_module_and_its_answer_comes_back_once() {
        let handler = handler(answer, 7 as *mut c_void);
        let (tx, rx) = mpsc::channel();
        handler(
            br#"{"op":"status"}"#.to_vec(),
            exact_runner::Reply::new(move |o| tx.send(o).unwrap()),
        );
        match rx.recv().unwrap() {
            exact_runner::Outcome::Response(r) => {
                assert_eq!(r.status, 200);
                assert_eq!(r.body, br#"{"echo":{"op":"status"}}"#);
            }
            other => panic!("{other:?}"),
        }
    }
}
