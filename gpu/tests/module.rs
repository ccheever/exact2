//! The module refuses what a host gets wrong before it touches a device
//! (LLP 1009 D2: the ABI is the one `unsafe` boundary, and it is sound
//! against a confused host — a null pointer, a byte count that overflows, a
//! child out of order — each refused by name).

#![cfg(not(target_arch = "wasm32"))]

use exact_gpu::{Module, Registry};

static EMPTY: Registry = Registry(&[]);

#[test]
fn a_byte_count_that_overflows_is_refused_by_name() {
    let mut m = Module::new(&EMPTY);
    // `u32::MAX × u32::MAX × 4` does not fit a usize: refused as a count,
    // never computed wrapped.
    assert!(!m.child(1, 0, [0.0; 4], u32::MAX, u32::MAX, &[]));
    assert!(m.take_error().starts_with("child 0:"));
    assert!(!m.texture(1, u32::MAX, u32::MAX, &[]));
    assert!(m.take_error().starts_with("children:"));
    // A zero size is refused before the device is asked for.
    assert!(!m.texture(1, 0, 4, &[]));
    assert!(m.take_error().starts_with("children:"));
}

#[test]
fn module_errors_are_consumed_in_sequence() {
    let mut m = Module::new(&EMPTY);
    assert!(!m.bind(10, &[]));
    assert_eq!(m.take_error(), "no such canvas");
    assert_eq!(m.take_error(), "", "error A was consumed");
    m.destroy(10); // a successful no-op does not revive the consumed error
    assert!(!m.texture(10, 0, 1, &[]));
    assert!(
        m.take_error().starts_with("children:"),
        "error B is current"
    );
    assert_eq!(m.take_error(), "", "error B was consumed");
}

#[test]
fn a_null_pointer_is_refused_by_the_abi() {
    // SAFETY: a null pointer is exactly the case the check is for.
    let none = unsafe { exact_gpu::native::bytes("test", std::ptr::null(), 4) };
    assert!(none.is_none());
    assert!(
        exact_gpu::native::error().contains("null pointer"),
        "the refusal is reported"
    );
    assert_eq!(exact_gpu::native::error(), "", "and reported once");
    // SAFETY: as above, for the writable variant.
    let none = unsafe { exact_gpu::native::bytes_mut("test", std::ptr::null_mut(), 4) };
    assert!(none.is_none());
    // A non-null pointer with its length is the slice.
    let data = [1u8, 2, 3];
    // SAFETY: `data` is three readable bytes that outlive the call.
    let some = unsafe { exact_gpu::native::bytes("test", data.as_ptr(), 3) };
    assert_eq!(some, Some(&data[..]));
}
