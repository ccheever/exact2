//! `motion.wasm`'s exports: numbers in, numbers out (text only in)
//! (`host/web-js/motion.js` is the one caller). Serials cross as f64,
//! exact below 2^53. One thread, one engine.
#![allow(static_mut_refs, clippy::missing_safety_doc)]

use super::{Motion, TARGETS};
use exact_motion::{Property, Value};

static mut STATE: Option<Motion> = None;
static mut IO: Vec<u8> = Vec::new();
static mut OPS: Vec<f64> = Vec::new();
static mut SCRATCH: [f64; 4] = [0.0; 4];

fn state() -> &'static mut Motion {
    unsafe { STATE.get_or_insert_with(Motion::new) }
}

fn property(p: u32) -> Option<Property> {
    match p {
        4 => Some(Property::Height),
        p => TARGETS.get(p as usize).copied(),
    }
}

/// Room for `len` bytes of input text; its address.
#[no_mangle]
pub extern "C" fn m_in(len: u32) -> *mut u8 {
    unsafe {
        IO.clear();
        IO.resize(len as usize, 0);
        IO.as_mut_ptr()
    }
}

/// The last lowering's numbers (`m_lower`).
#[no_mangle]
pub extern "C" fn m_out() -> *const f64 {
    unsafe { OPS.as_ptr() }
}

/// The numbers the last call left (a hold's value, a velocity, a pair).
#[no_mangle]
pub extern "C" fn m_scratch(i: u32) -> f64 {
    unsafe { SCRATCH[(i & 3) as usize] }
}

/// A node's `transition` row, the input's `len` bytes of CSS text.
#[no_mangle]
pub extern "C" fn m_transitions(node: f64, len: u32) -> u32 {
    let text = unsafe { String::from_utf8_lossy(&IO[..len as usize]).into_owned() };
    state().transitions(node as u64, &text) as u32
}

/// A node's four targets after a commit at `now` seconds: translate as its
/// lengths and its percentages of the box.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn m_observe(
    node: f64,
    x: f64,
    y: f64,
    px: f64,
    py: f64,
    scale: f64,
    rotate: f64,
    opacity: f64,
    now: f64,
) -> u32 {
    state()
        .observe(node as u64, [x, y, px, py, scale, rotate, opacity], now)
        .is_ok() as u32
}

/// The height owner's numeric height at `now`.
#[no_mangle]
pub extern "C" fn m_height(node: f64, height: f64, now: f64) -> u32 {
    state().height(node as u64, height, now).is_ok() as u32
}

/// The node owns height no more: 1 when it had height motion.
#[no_mangle]
pub extern "C" fn m_unheight(node: f64) -> u32 {
    state().retire_height(node as u64) as u32
}

/// A node left the tree.
#[no_mangle]
pub extern "C" fn m_remove(node: f64) {
    state().remove(node as u64);
}

/// Begin a hold: its serial (0 for none), its value in the scratch.
#[no_mangle]
pub extern "C" fn m_begin(node: f64, p: u32, x: f64, y: f64, now: f64) -> f64 {
    let Some(p) = property(p) else { return 0.0 };
    match state().begin(node as u64, p, Value::new(x, y), now) {
        Ok(Some((serial, value))) => {
            unsafe { SCRATCH[..2].copy_from_slice(&[value.x, value.y]) };
            serial as f64
        }
        _ => 0.0,
    }
}

/// Begin a transform pair on `node`: its translate serial (0 for none);
/// the scale serial and the values taken (x, y, scale) in the scratch.
#[no_mangle]
pub extern "C" fn m_pair_begin(node: f64, x: f64, y: f64, scale: f64, now: f64) -> f64 {
    match state().begin_pair(node as u64, [x, y, scale], now) {
        Ok(Some((t, s, v))) => {
            unsafe { SCRATCH = [v[0], v[1], v[2], s as f64] };
            t as f64
        }
        _ => 0.0,
    }
}

/// Move a pair by its translate serial: 1 when accepted.
#[no_mangle]
pub extern "C" fn m_pair_update(serial: f64, x: f64, y: f64, scale: f64, now: f64) -> u32 {
    matches!(
        state().update_pair(serial as u64, [x, y, scale], now),
        Ok(true)
    ) as u32
}

/// Move a hold: 1 when accepted.
#[no_mangle]
pub extern "C" fn m_update(serial: f64, x: f64, y: f64, now: f64) -> u32 {
    matches!(
        state().update(serial as u64, Value::new(x, y), now),
        Ok(true)
    ) as u32
}

/// End a hold: `mode` 0 releases at (vx, vy), 1 cancels, 2 releases at the
/// engine's measured velocity. 1 when accepted, 0 stale, 2 refused input.
#[no_mangle]
pub extern "C" fn m_end(serial: f64, vx: f64, vy: f64, mode: u32, now: f64) -> u32 {
    let s = state();
    let velocity = match mode {
        0 => Some(Value::new(vx, vy)),
        2 => Some(s.measured(serial as u64, now)),
        _ => None,
    };
    match s.end(serial as u64, velocity, now) {
        Ok(true) => 1,
        Ok(false) => 0,
        Err(_) => 2,
    }
}

/// The engine's velocity over a live hold's values at `now` (x in the
/// scratch, then y), where the platform measures none.
#[no_mangle]
pub extern "C" fn m_measured(serial: f64, now: f64) {
    let v = state().measured(serial as u64, now);
    unsafe { SCRATCH[..2].copy_from_slice(&[v.x, v.y]) };
}

/// Record a constrained display's value for a hold.
#[no_mangle]
pub extern "C" fn m_track(serial: f64, x: f64, y: f64, now: f64) -> u32 {
    state().track(serial as u64, now, Value::new(x, y)) as u32
}

/// A live hold's node (0 for none); its property index in the scratch.
#[no_mangle]
pub extern "C" fn m_held(serial: f64) -> f64 {
    match state().held(serial as u64) {
        Some((node, p)) => {
            let i = TARGETS.iter().position(|t| *t == p).unwrap_or(4);
            unsafe { SCRATCH[0] = i as f64 };
            node as f64
        }
        None => 0.0,
    }
}

/// Seek to `now` and lower: how many numbers `m_out` holds.
#[no_mangle]
pub extern "C" fn m_lower(now: f64) -> u32 {
    unsafe {
        OPS.clear();
        if state().lower(now, &mut OPS).is_err() {
            OPS.clear();
        }
        OPS.len() as u32
    }
}

/// When the last spring ends, seconds; -1 when none runs.
#[no_mangle]
pub extern "C" fn m_settle() -> f64 {
    state().settle_time().unwrap_or(-1.0)
}

/// exact2's gesture thresholds (LLP 1057.001 §3): knee, resistance, edge, slop.
#[no_mangle]
pub extern "C" fn m_gesture(i: u32) -> f64 {
    exact_motion::gesture::CONSTANTS
        .get(i as usize)
        .copied()
        .unwrap_or(0.0)
}

/// A pan's pointer sample (LLP 1057 §10.6).
#[no_mangle]
pub extern "C" fn m_pan_sample(view: u32, first: u32, x: f64, y: f64, ms: f64) {
    state().pan_sample(view, first != 0, x, y, ms);
}

/// A pan's release velocity, in the scratch.
#[no_mangle]
pub extern "C" fn m_pan_release(view: u32, ms: f64) {
    let v = state().pan_release(view, ms);
    unsafe { SCRATCH[..2].copy_from_slice(&[v.x, v.y]) };
}
