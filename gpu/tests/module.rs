//! The module refuses what a host gets wrong before it touches a device
//! (LLP 1009 D2: the ABI is the one `unsafe` boundary, and it is sound
//! against a confused host — a null pointer, a byte count that overflows, a
//! child out of order — each refused by name).

#![cfg(not(target_arch = "wasm32"))]

use exact_gpu::{Module, Registry};

static EMPTY: Registry = Registry {
    surfaces: &[],
    shaders: &[],
};

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

#[test]
fn device_event_json_preserves_every_variant_and_refuses_fields_by_name() {
    use exact_gpu::{json::parse_input, InputEvent, PointerKind, PointerPhase};
    assert_eq!(
        parse_input(
            r#"{"t":"key","code":"KeyW","key":"w","down":true,"repeat":false,"at":1234.5}"#
        )
        .unwrap(),
        InputEvent::Key {
            code: "KeyW".into(),
            key: "w".into(),
            down: true,
            repeat: false,
            at_ms: 1234.5
        }
    );
    for (phase, expected) in [
        ("down", PointerPhase::Down),
        ("move", PointerPhase::Move),
        ("up", PointerPhase::Up),
        ("cancel", PointerPhase::Cancel),
    ] {
        for (kind, device) in [
            ("mouse", PointerKind::Mouse),
            ("touch", PointerKind::Touch),
            ("pen", PointerKind::Pen),
        ] {
            let text = format!(
                r#"{{"t":"pointer","phase":"{phase}","id":1,"x":10.5,"y":20,"kind":"{kind}","buttons":1,"at":2}}"#
            );
            assert_eq!(
                parse_input(&text).unwrap(),
                InputEvent::Pointer {
                    id: 1,
                    phase: expected,
                    x: 10.5,
                    y: 20.0,
                    kind: device,
                    buttons: 1,
                    at_ms: 2.0
                }
            );
        }
    }
    assert_eq!(
        parse_input(r#"{"t":"wheel","dx":0,"dy":-120,"x":2,"y":3,"at":4}"#).unwrap(),
        InputEvent::Wheel {
            dx: 0.0,
            dy: -120.0,
            x: 2.0,
            y: 3.0,
            at_ms: 4.0
        }
    );
    assert_eq!(
        parse_input(r#"{"t":"blur","at":5}"#).unwrap(),
        InputEvent::Blur { at_ms: 5.0 }
    );
    let pointer =
        r#"{"t":"pointer","phase":"down","id":1,"x":10,"y":20,"kind":"mouse","buttons":1,"at":2}"#;
    let mut module = Module::new(&EMPTY);
    for (text, field) in [
        (r#"{"t":"blur","at":1e999}"#.into(), "at"),
        (r#"{"t":"blur","at":01}"#.into(), "at"),
        (r#"{"t":"blur","at":1,}"#.into(), "field"),
        (r#"{"t":"wat","at":0}"#.into(), "t:"),
        (r#"{"t":"key","at":0,"code":3}"#.into(), "code"),
        (pointer.replace("\"down\"", "\"wat\""), "phase"),
        (pointer.replace("\"mouse\"", "\"wat\""), "kind"),
        (pointer.replace("\"id\":1", "\"id\":-1"), "id"),
        (
            pointer.replace("\"buttons\":1", "\"buttons\":1.5"),
            "buttons",
        ),
        (pointer.replace("\"x\":10", "\"x\":1e100"), "x"),
        (pointer.replace("\"y\":20", "\"y\":[]"), "y"),
    ] {
        assert!(!module.input_json(1, &text));
        let error = module.take_error();
        assert!(
            error.starts_with("input:") && error.contains(field),
            "{text}: {error}"
        );
    }
    let strings = vec!["quote\" slash\\ newline\n tab\t nul\0 雪".to_string()];
    assert_eq!(
        exact_gpu::json::parse_values(&exact_gpu::json::strings(&strings)).unwrap(),
        vec![exact_gpu::Value::str(&strings[0])]
    );
}

#[cfg(target_os = "macos")]
mod seams {
    use exact_gpu::{wgpu, Frame, InputEvent, Registry, Surface, SurfaceError, Value};
    use std::cell::RefCell;
    thread_local! { static FRAMES: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) }; }
    struct Probe(Vec<String>);
    impl Surface for Probe {
        fn bind(&mut self, _: &[Value]) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn wants_input(&self) -> bool {
            true
        }
        fn input(&mut self, event: &InputEvent) {
            self.0.push(format!("{event:?}"));
        }
        fn messages(&mut self) -> Vec<String> {
            std::mem::take(&mut self.0)
        }
        fn agent(&mut self, request: &str) -> Option<String> {
            self.0.push("agent".into());
            (request != "{}").then(|| request.to_string())
        }
        fn render(
            &mut self,
            frame: &Frame,
            _: &wgpu::Device,
            _: &wgpu::Queue,
            _: &wgpu::TextureView,
            _: wgpu::TextureFormat,
        ) -> bool {
            FRAMES.with(|f| f.borrow_mut().push(*frame));
            self.0.push("render".into());
            false
        }
    }
    static REGISTRY: Registry = Registry {
        surfaces: &[("probe", 0, || Box::new(Probe(Vec::new())))],
        shaders: &[],
    };
    exact_gpu::module!(REGISTRY);
    #[link(name = "QuartzCore", kind = "framework")]
    extern "C" {}

    #[test]
    fn module_seams_drain_once_and_stamp_frames() {
        if gpu_load() != 0 {
            eprintln!("{}; seam device test skipped", exact_gpu::native::error());
            return;
        }
        // SAFETY: a retained Metal layer stays alive until after gpu_destroy.
        let layer: objc2::rc::Retained<objc2::runtime::AnyObject> = unsafe {
            objc2::msg_send![objc2::runtime::AnyClass::get(c"CAMetalLayer").unwrap(), new]
        };
        let name = b"probe";
        let id = unsafe {
            gpu_create(
                name.as_ptr(),
                name.len(),
                (&*layer as *const objc2::runtime::AnyObject)
                    .cast_mut()
                    .cast(),
                4,
                4,
            )
        };
        assert_ne!(id, 0, "{}", exact_gpu::native::error());
        assert!(gpu_wants_input(id));
        assert_eq!(exact_gpu::native::bind(id, "[]"), 0);
        gpu_seekable(true);
        let event = br#"{"t":"blur","at":12.5}"#;
        assert!(unsafe { gpu_input(id, event.as_ptr(), event.len()) });
        assert_eq!(
            exact_gpu::native::messages(id),
            r#"["Blur { at_ms: 12.5 }"]"#
        );
        assert_eq!(exact_gpu::native::messages(id), "[]");
        let request = br#"{"op":"state","now":12.5}"#;
        let len = unsafe { gpu_agent(id, request.as_ptr(), request.len()) };
        assert_eq!(
            unsafe { std::slice::from_raw_parts(gpu_out_ptr(), len as usize) },
            request
        );
        assert_eq!(exact_gpu::native::messages(id), r#"["agent"]"#);
        assert_eq!(unsafe { gpu_agent(id, b"{}".as_ptr(), 2) }, 0);
        assert_eq!(gpu_messages(id), 9);
        assert_eq!(gpu_messages(id), 2);
        let mut pixels = [0; 64];
        assert_eq!(
            exact_gpu::native::readback(id, 4.0, 4.0, 1.0, 12.5, &mut pixels),
            0
        );
        assert_eq!(exact_gpu::native::messages(id), r#"["render"]"#);
        assert_eq!(exact_gpu::native::messages(id), "[]");
        FRAMES.with(|f| assert!(f.borrow().last().unwrap().seekable));
        gpu_seekable(false);
        let result = gpu_render(id, 4.0, 4.0, 1.0, 13.0);
        assert_eq!(result, 0, "{}", exact_gpu::native::error());
        assert_eq!(exact_gpu::native::messages(id), r#"["render"]"#);
        FRAMES.with(|f| assert!(!f.borrow().last().unwrap().seekable));
        assert!(!unsafe { gpu_input(id, std::ptr::null(), 1) });
        assert!(exact_gpu::native::error().contains("gpu_input"));
        assert!(!unsafe { gpu_input(id, [255].as_ptr(), 1) });
        assert!(exact_gpu::native::error().contains("UTF-8"));
        gpu_destroy(id);
    }
}
