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
        op if state.refused_ops.contains(&op) => Err(format!("__exact_host: no op {op}")),
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
        // A locked presence name throws `InvalidAccessError` (LLP 1133.010 D5).
        2 => match state.store {
            Some(s) => (*s)
                .read(&a)
                .map(|v| v.map(str::to_string))
                .map_err(store_error),
            // An absence it cannot know would read as "nothing kept" (a
            // signed-out session): refused as a write is (LLP 1146 D3).
            None if state.between_answers => Err(no_store("store.get")),
            None => Ok(None),
        },
        3 => match state.store {
            Some(s) => (*s).set(&a, &b).map(|_| None).map_err(|e| match e {
                exact_runner::StoreError::NotSupported(_) => store_error(e),
                e => format!("store.set: {e:?}"),
            }),
            None if state.between_answers => Err(no_store("store.set")),
            None => Err("store.set: no store at bake".into()),
        },
        4 => match state.store {
            Some(s) => (*s)
                .forget(&a)
                .map(|_| None)
                .map_err(|e| format!("store.forget: {e:?}")),
            None if state.between_answers => Err(no_store("store.forget")),
            None => Err("store.forget: no store at bake".into()),
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
                    // needs no app storage, as for a Rust source; nor does a
                    // `home:` or `bundle:` path (LLP 1133.007 D6).
                    let document = (state.documents && a.starts_with("doc:/"))
                        || a.starts_with("home:/")
                        || a.starts_with("bundle:/");
                    super::storage::refusal(true, state.storage || document, state.agent.is_some())
                }
            }
            // No answer's store: a background round (LLP 1097 D5) or a
            // let-go chain (splitter rough 7). Storage is live, with the
            // same refusal codes as an answer's call, not the bake's.
            None if state.between_answers => {
                let document = (state.documents && a.starts_with("doc:/"))
                    || a.starts_with("home:/")
                    || a.starts_with("bundle:/");
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
            } else if a == "kept" {
                // What the app keeps across launches (LLP 1069.010.000 D5):
                // the device's, and a candidate on trial changes none of it.
                if let Some(store) = state.store {
                    (*store).observe_external_read();
                }
                if state.baking || (state.validating && !b.is_empty()) {
                    Err("bake".into())
                } else {
                    super::storage::kept(state.kept, &b)
                }
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
                    // The bake links no module. The prelude throws storage's
                    // `code: 'bake'` refusal for it (LLP 1067 D4).
                    _ if state.baking => Err("bake".into()),
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
        // Whether this host listens (LLP 1133.005 §8): the desktop's, where
        // the prelude installs `exactListen`.
        12 if a == "listen" => {
            Ok(cfg!(any(target_os = "macos", target_os = "linux")).then(|| "1".into()))
        }
        12 => Ok(crypto::auth_callback(state, &a)),
        // A journal line the runtime writes (LLP 1097 D8): a failed or
        // refused storage operation, an unhandled rejection. `logs` reads
        // it as it is, where `console` lines are marked as the app's.
        13 => {
            state.journal.push(a.into_owned());
            Ok(Some("1".into()))
        }
        // A socket operation a mutation's source or a mapper call staged
        // (14), or a turn of an answer sent (17), on the stream of fetch
        // ticket `a` (LLP 1016.000 D7 and its amendment of 2026-10-18;
        // `socket.rs`).
        14 | 17 => match (a.parse::<u64>(), super::socket::op_from_json(&b)) {
            (Ok(ticket), Ok(sent)) if op == 17 => {
                state.answer_ops.push((ticket, sent));
                Ok(None)
            }
            (Ok(ticket), Ok(staged)) => {
                state.socket_ops.push((ticket, staged));
                Ok(None)
            }
            (Err(_), _) => Err("socket: a ticket that is not a number".into()),
            (_, Err(e)) => Err(e),
        },
        // The bytes the socket of stream ticket `a` buffered as this turn
        // began (D7), with what an answer has sent on it in this turn: the
        // start of its handle's `bufferedAmount`.
        15 => Ok(Some({
            let ticket = a.parse::<u64>().ok();
            let start = (state.socket_buffered.iter())
                .find(|(s, _)| Some(*s) == ticket)
                .map_or(0, |(_, buffered)| *buffered);
            let handed = (state.answer_ops.iter())
                .filter(|(t, _)| Some(*t) == ticket)
                .map(|(_, op)| op.size() as u64);
            (start + handed.sum::<u64>()).to_string()
        })),
        // A fetch the module withdrew before it was handed out (D7: a
        // mutation that also sent on a socket; or one the app aborted).
        16 => {
            let ticket = a.parse::<u64>().ok();
            state.requests.retain(|(t, _)| Some(*t) != ticket);
            Ok(None)
        }
        // A presence name's occupancy, never its value (LLP 1133.010 D5): a
        // value is kept, locked or not (18); kept locked or awaiting the move
        // (19); this device keeps locked secrets now (20). Each a device read.
        18..=20 if state.store.is_none() && state.between_answers => Err(no_store(match op {
            18 => "store.has",
            19 => "store.locked",
            _ => "store.protect",
        })),
        18..=20 => Ok(state.store.map(|s| {
            let yes = match op {
                18 => (*s).has(&a),
                19 => (*s).locked(&a),
                _ => {
                    (*s).observe_external_read();
                    (*s).presence_capable()
                }
            };
            if yes { "1" } else { "0" }.to_string()
        })),
        // The runner's clock as this call began, ms: when a timer the answer
        // arms is due (binary.js `timers`). None at bake.
        21 => Ok(state.store.map(|s| (*s).clock_ms().to_string())),
        // Module work's state (LLP 1146 D2, binary.js `__exact_module_work`):
        // what may still settle a waiting answer.
        22 => match serde_json::from_str::<serde_json::Value>(&b) {
            Ok(v) => {
                state.module_outstanding = v["outstanding"].as_u64().unwrap_or(0) as usize;
                Ok(None)
            }
            Err(e) => Err(format!("module work state: {e}")),
        },
        // A handed-out fetch the app aborted: the runner cancels its request
        // and resumes the answer that awaits it (`Module::take_aborted`).
        23 => match a.parse::<u64>() {
            Ok(ticket) => {
                state.aborted.push(ticket);
                Ok(None)
            }
            Err(_) => Err("abort: a ticket that is not a number".into()),
        },
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

/// Why a store operation is refused where the executor offers no store:
/// the continuation of an answer that was let go, or background work
/// (LLP 1146 D3).
pub(crate) fn no_store(operation: &str) -> String {
    format!("{operation}: no store here, in background work or the continuation of an answer that was let go. Read the store before the answer's first await. A write that must stand goes after a request with effects (a POST), whose continuation keeps the store.")
}

/// A store refusal as the prelude names it: its message starts with the
/// error's name (`InvalidAccessError: …`, `NotSupportedError: …`).
pub(crate) fn store_error(e: exact_runner::StoreError) -> String {
    match exact_runner::DataError::from(e) {
        exact_runner::DataError::Unavailable(message) => message,
        other => other.to_string(),
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
