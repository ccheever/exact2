//! The one door from the module into Rust (`__exact_host`), and the
//! malloc'd strings it hands back.
use super::{crypto, pure, wire, HostState};
use std::ffi::{c_char, c_void, CStr};

/// The one door from the module into Rust (`__exact_host` in the prelude).
///
/// # Safety
/// Called by the shim on the engine's thread with `ctx` the `HostState` the
/// engine was created with, and `a`/`b` NUL-terminated for the call.
pub(crate) unsafe extern "C" fn host_door(
    ctx: *mut c_void,
    op: u32,
    a: *const c_char,
    b: *const c_char,
    out: *mut *mut c_char,
) -> i32 {
    let state = &mut *(ctx as *mut HostState);
    let a = CStr::from_ptr(a).to_string_lossy();
    let b = CStr::from_ptr(b).to_string_lossy();
    let reply: Result<Option<String>, String> = match op {
        1 => match a.parse::<u64>() {
            Ok(ticket) => match wire::request_from_json(&b) {
                Ok(request) => {
                    state.requests.push((ticket, request));
                    Ok(None)
                }
                Err(e) => Err(format!("fetch: {e}")),
            },
            Err(_) => Err("fetch: a ticket that is not a number".into()),
        },
        2 => Ok(state.store.and_then(|s| (*s).get(&a).map(str::to_string))),
        3 => match state.store {
            Some(s) => (*s)
                .set(&a, &b)
                .map(|_| None)
                .map_err(|e| format!("store.set: {e:?}")),
            None => Err(
                "store.set: no store at bake or in background work, which no answer waits for"
                    .into(),
            ),
        },
        4 => match state.store {
            Some(s) => (*s)
                .forget(&a)
                .map(|_| None)
                .map_err(|e| format!("store.forget: {e:?}")),
            None => Err(
                "store.forget: no store at bake or in background work, which no answer waits for"
                    .into(),
            ),
        },
        // Storage's availability, as the prelude's refusal code (kanban
        // F28): none at bake; none for a drive that names no scratch store;
        // none where the host configured no directories.
        5 => match state.store {
            Some(store) => {
                // A read even at bake: the build compiles no answer that tried.
                (*store).observe_external_read();
                if state.baking {
                    Err("bake".into())
                } else {
                    // `a` is the path a file operation names: a document
                    // needs no app storage, as for a Rust source.
                    let document = state.documents && a.starts_with("doc:/");
                    super::storage::refusal(true, state.storage || document, state.agent.is_some())
                }
            }
            // No answer's store: a background round (LLP 1097 D5) or a
            // let-go chain (splitter rough 7). Storage is live, with the
            // same refusal codes as an answer's call, not the bake's.
            None if state.between_answers => {
                let document = state.documents && a.starts_with("doc:/");
                super::storage::refusal(
                    !state.baking,
                    state.storage || document,
                    state.agent.is_some(),
                )
            }
            None => Err("bake".into()),
        },
        6 => {
            if a == "kind" {
                // A native executor can always link a module; no read.
                Ok(Some("native".into()))
            } else if a == "available" {
                // Only a linked, configured module: `native.available` is false
                // at bake, in agent mode, and when the app links none. Whether
                // there is one is the device's fact, not the build's: an answer
                // that asks is not compiled, and the host asks it again.
                if let Some(store) = state.store {
                    (*store).observe_external_read();
                }
                Ok((state.native.is_some() || state.hosted).then(|| "native".into()))
            } else if a == "watch" {
                // The answer watches a device topic; its announcement asks
                // the answer again (LLP 1016.002).
                if let Some(store) = state.store {
                    (*store).observe_topic(&b);
                }
                Ok(None)
            } else if a == "later" {
                Ok(state.later.then(|| "later".into()))
            } else {
                if let Some(store) = state.store {
                    (*store).observe_external_read();
                }
                match (&mut state.native, state.store) {
                    (Some(module), Some(_)) => serde_json::from_str(&b)
                        .map_err(|error| error.to_string())
                        .and_then(|request| module.call(&request))
                        .map(|reply| Some(reply.to_string())),
                    // The host's app module, on this thread (LLP 1067.000 D9).
                    (None, Some(_)) if state.hosted => match &state.hosted_call {
                        Some(call) => call(b.as_bytes())
                            .map(|reply| Some(String::from_utf8_lossy(&reply).into_owned())),
                        None => Err("the app's module answers no native.call".into()),
                    },
                    _ => Err(
                        "native storage is unavailable during bake or in an unconfigured host"
                            .into(),
                    ),
                }
            }
        }
        7 => pure::call(&a, &b).map(Some),
        // The answer drew secure randomness (LLP 1069.005 D2): the device's,
        // so bake compiles none of it. No store (an in-process query): no mark.
        8 => {
            if let Some(store) = state.store {
                (*store).observe_entropy();
            }
            Ok(None)
        }
        9 => wire::canvas_measure(state.canvas.as_ref(), &a).map(Some),
        // Under the agent, `b` bytes of its repeatable stream as hex; else
        // nothing, and the draw is the OS's (LLP 1069.005 D2b).
        11 => Ok(state.agent.as_mut().map(|stream| {
            let mut bytes = vec![0; b.parse::<usize>().unwrap_or(0).min(65_536)];
            stream.fill(&mut bytes);
            crypto::hex(&bytes)
        })),
        10 => Ok(wire::canvas_image(state.canvas.as_ref(), &a)),
        12 => Ok(crypto::auth_callback(state, &a)),
        // A journal line the runtime writes (LLP 1097 D8): a failed or
        // refused storage operation, an unhandled rejection. `logs` reads
        // it as it is, where `console` lines are marked as the app's.
        13 => {
            state.journal.push(a.into_owned());
            Ok(Some("1".into()))
        }
        other => Err(format!("__exact_host: no op {other}")),
    };
    *out = std::ptr::null_mut();
    match reply {
        Ok(None) => 0,
        Ok(Some(text)) => {
            *out = c_string(&text);
            0
        }
        Err(text) => {
            *out = c_string(&text);
            1
        }
    }
}

/// A malloc'd copy the shim frees.
pub(crate) fn c_string(text: &str) -> *mut c_char {
    let bytes = text.as_bytes();
    // SAFETY: malloc'd with room for the NUL; the shim `free`s it.
    unsafe {
        let p = libc_malloc(bytes.len() + 1) as *mut u8;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        *p.add(bytes.len()) = 0;
        p as *mut c_char
    }
}

extern "C" {
    #[link_name = "malloc"]
    fn libc_malloc(size: usize) -> *mut c_void;
}
