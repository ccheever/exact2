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
    assert!(!m.bind(10, &[], None));
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
    thread_local! { static BINDS: RefCell<Vec<Option<f64>>> = const { RefCell::new(Vec::new()) }; }
    struct Unload;
    impl Drop for Unload {
        fn drop(&mut self) {
            exact_gpu::native::unload();
        }
    }
    struct Probe(Vec<String>, Option<SurfaceError>, Option<String>, Vec<u8>);
    impl Surface for Probe {
        fn bind(&mut self, _: &[Value]) -> Result<(), SurfaceError> {
            self.2 = Some("{\"phase\":\"bind\"}".into());
            Ok(())
        }
        fn bind_at(&mut self, inputs: &[Value], at_ms: Option<f64>) -> Result<(), SurfaceError> {
            BINDS.with(|b| b.borrow_mut().push(at_ms));
            if at_ms == Some(999.) {
                self.1 = Some(SurfaceError("advance capacity".into()));
            }
            self.bind(inputs)
        }
        fn carry(&mut self) -> Option<Vec<u8>> {
            Some(self.3.clone())
        }
        fn restore(&mut self, bytes: &[u8]) -> Result<(), String> {
            if bytes.first() == Some(&255) {
                return Err("probe byte refused".into());
            }
            self.3 = bytes.to_vec();
            self.2 = Some("{\"phase\":\"restore\"}".into());
            Ok(())
        }
        fn take_error(&mut self) -> Option<SurfaceError> {
            self.1.take()
        }
        fn wants_input(&self) -> bool {
            true
        }
        fn input(&mut self, event: &InputEvent) {
            self.2 = Some("{\"phase\":\"input\"}".into());
            self.0.push(format!("{event:?}"));
            if matches!(event, InputEvent::Blur { at_ms: -13. }) {
                self.1 = Some(SurfaceError("input capacity".into()));
            }
        }
        fn published(&mut self) -> Option<String> {
            self.2.take()
        }
        fn messages(&mut self) -> Vec<String> {
            std::mem::take(&mut self.0)
        }
        fn agent(&mut self, request: &str) -> Option<String> {
            if request == "fail" {
                self.1 = Some(SurfaceError("agent capacity".into()));
            }
            if request == "null" {
                return None;
            }
            self.2 = Some("{\"phase\":\"agent\"}".into());
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
            if frame.now_ms == -13. {
                self.1 = Some(SurfaceError("render capacity".into()));
            }
            self.2 = Some("{\"phase\":\"render\"}".into());
            self.0.push("render".into());
            false
        }
    }
    static REGISTRY: Registry = Registry {
        surfaces: &[("probe", 0, || {
            Box::new(Probe(Vec::new(), None, None, vec![1, 2]))
        })],
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
        let _cleanup = Unload;
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
        assert_eq!(gpu_wants_input(id), 1);
        assert_eq!(exact_gpu::native::bind(id, "[]"), 0);
        assert_eq!(unsafe { gpu_bind_at(id, b"[]".as_ptr(), 2, 500.0) }, 0);
        BINDS.with(|b| assert_eq!(*b.borrow(), [None, Some(500.0)]));
        assert_eq!(
            exact_gpu::native::published(id).as_deref(),
            Some(r#"{"phase":"bind"}"#)
        );
        assert_eq!(gpu_published(id), u32::MAX);
        gpu_seekable(true);
        let event = br#"{"t":"blur","at":12.5}"#;
        assert_eq!(unsafe { gpu_input(id, event.as_ptr(), event.len()) }, 0);
        assert_eq!(
            exact_gpu::native::messages(id).as_deref(),
            Some(r#"["Blur { at_ms: 12.5 }"]"#)
        );
        assert_eq!(exact_gpu::native::messages(id).as_deref(), None);
        assert!(exact_gpu::native::published(id).is_some());
        assert_eq!(gpu_published(id), u32::MAX);
        let request = br#"{"op":"state","now":12.5}"#;
        let len = unsafe { gpu_agent(id, request.as_ptr(), request.len()) };
        assert_eq!(
            unsafe { std::slice::from_raw_parts(gpu_out_ptr(), len as usize) },
            request
        );
        assert_eq!(
            exact_gpu::native::messages(id).as_deref(),
            Some(r#"["agent"]"#)
        );
        let n = gpu_published(id);
        assert_ne!(n, u32::MAX);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(gpu_out_ptr(), n as usize) },
            br#"{"phase":"agent"}"#
        );
        assert_eq!(gpu_published(id), u32::MAX);
        assert_eq!(unsafe { gpu_agent(id, b"{}".as_ptr(), 2) }, 0);
        assert_eq!(gpu_messages(id), 9);
        assert_eq!(gpu_messages(id), u32::MAX);
        let mut pixels = [0; 64];
        assert_eq!(
            exact_gpu::native::readback(id, 4.0, 4.0, 1.0, 12.5, &mut pixels),
            0
        );
        assert_eq!(
            exact_gpu::native::messages(id).as_deref(),
            Some(r#"["render"]"#)
        );
        assert_eq!(exact_gpu::native::messages(id).as_deref(), None);
        assert!(exact_gpu::native::published(id).is_some());
        assert_eq!(gpu_published(id), u32::MAX);
        FRAMES.with(|f| assert!(f.borrow().last().unwrap().seekable));
        gpu_seekable(false);
        let result = gpu_render(id, 4.0, 4.0, 1.0, 13.0);
        assert_eq!(result, 0, "{}", exact_gpu::native::error());
        assert_eq!(
            exact_gpu::native::messages(id).as_deref(),
            Some(r#"["render"]"#)
        );
        FRAMES.with(|f| assert!(!f.borrow().last().unwrap().seekable));
        assert_eq!(gpu_dirty(id), 0);
        assert_eq!(gpu_carry(id), 2);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(gpu_out_ptr(), 2) },
            &[1, 2]
        );
        assert!(!unsafe { gpu_restore(id, [255].as_ptr(), 1) });
        assert_eq!(exact_gpu::native::error(), "probe byte refused");
        assert_eq!(gpu_dirty(id), 0, "failed restore leaves dirty unchanged");
        assert_eq!(exact_gpu::native::carry(id), Some(vec![1, 2]));
        assert!(unsafe { gpu_restore(id, [3, 4, 5].as_ptr(), 3) });
        assert_eq!(gpu_dirty(id), 1);
        assert_eq!(
            exact_gpu::native::published(id).as_deref(),
            Some(r#"{"phase":"restore"}"#)
        );
        assert_eq!(exact_gpu::native::carry(id), Some(vec![3, 4, 5]));
        assert_eq!(
            exact_gpu::native::messages(id),
            None,
            "old outputs are not restored messages"
        );
        assert!(unsafe { gpu_restore(id, [].as_ptr(), 0) });
        assert_eq!(gpu_carry(id), 0, "empty carry is not nothing");
        assert_eq!(gpu_carry(u32::MAX), u32::MAX);
        assert_eq!(exact_gpu::native::error(), "no such canvas");
        assert!(!unsafe { gpu_restore(id, std::ptr::null(), 1) });
        assert!(exact_gpu::native::error().contains("gpu_restore"));
        assert_eq!(gpu_render(id, 4., 4., 1., 13.), 0);
        assert_eq!(unsafe { gpu_agent(id, b"null".as_ptr(), 4) }, 0);
        assert_eq!(gpu_dirty(id), 0, "an unanswered read costs no frame");
        assert_eq!(unsafe { gpu_agent(id, b"{}".as_ptr(), 2) }, 0);
        assert_eq!(gpu_dirty(id), 1, "a posted message dirties the surface");
        assert_eq!(gpu_render(id, 4.0, 4.0, 1.0, 13.0), 0);
        assert_ne!(unsafe { gpu_agent(id, request.as_ptr(), request.len()) }, 0);
        assert_eq!(gpu_dirty(id), 1, "an answer dirties the surface");
        assert_eq!(unsafe { gpu_input(id, std::ptr::null(), 1) }, 1);
        assert!(exact_gpu::native::error().contains("gpu_input"));
        assert_eq!(unsafe { gpu_input(id, [255].as_ptr(), 1) }, 1);
        assert!(exact_gpu::native::error().contains("UTF-8"));
        gpu_destroy(id);
        exact_gpu::native::unload();
    }
    #[test]
    fn surface_failures_reach_the_real_abi_and_committed_binds_stay_accepted() {
        if gpu_load() != 0 {
            eprintln!("SKIP error ABI fixture: {}", exact_gpu::native::error());
            return;
        }
        // SAFETY: retained layer outlives the canvas.
        let layer: objc2::rc::Retained<objc2::runtime::AnyObject> = unsafe {
            objc2::msg_send![objc2::runtime::AnyClass::get(c"CAMetalLayer").unwrap(), new]
        };
        let _cleanup = Unload;
        let id = unsafe {
            gpu_create(
                b"probe".as_ptr(),
                5,
                (&*layer as *const objc2::runtime::AnyObject)
                    .cast_mut()
                    .cast(),
                4,
                4,
            )
        };
        assert_ne!(id, 0);
        assert_eq!(unsafe { gpu_bind_at(id, b"[]".as_ptr(), 2, 999.) }, 0);
        assert_eq!(gpu_render(id, 4., 4., 1., 0.), 2);
        assert_eq!(exact_gpu::native::error(), "advance capacity");
        assert_eq!(exact_gpu::native::error(), "");
        assert_eq!(gpu_dirty(id), 1);
        assert_eq!(gpu_render(id, 4., 4., 1., -13.), 2);
        assert_eq!(exact_gpu::native::error(), "render capacity");
        let mut pixels = [0; 64];
        assert_eq!(
            exact_gpu::native::readback(id, 4., 4., 1., -13., &mut pixels),
            1
        );
        assert_eq!(exact_gpu::native::error(), "render capacity");
        assert!(!exact_gpu::native::input(id, r#"{"t":"blur","at":-13}"#));
        assert_eq!(exact_gpu::native::error(), "input capacity");
        assert_eq!(exact_gpu::native::agent(id, "fail"), "");
        assert_eq!(exact_gpu::native::error(), "agent capacity");
        assert_eq!(gpu_render(id, 4., 4., 1., 0.), 0);
        gpu_destroy(id);
        exact_gpu::native::unload();
    }
}

#[test]
fn empty_null_carry_and_oversized_length_are_distinct() {
    // SAFETY: zero-length ranges require no backing allocation.
    unsafe {
        assert_eq!(
            exact_gpu::native::bytes("restore", std::ptr::null(), 0),
            Some(&[][..])
        );
        assert_eq!(
            exact_gpu::native::bytes_mut("restore", std::ptr::null_mut(), 0),
            Some(&mut [][..])
        );
    }
    assert_eq!(exact_gpu::native::carry_length(0), Some(0));
    assert_eq!(exact_gpu::native::carry_length(u32::MAX as usize), None);
    assert_eq!(exact_gpu::native::carry_length(u32::MAX as usize + 1), None);
    assert!(exact_gpu::native::error().contains("carry exceeds ABI byte limit"));
}

#[test]
fn headless_ownership_keeps_every_non_drawing_seam() {
    use exact_gpu::{wgpu, Frame, InputEvent, Surface, SurfaceError, Value};
    #[derive(Default)]
    struct Probe {
        at: f64,
        bytes: Vec<u8>,
        output: bool,
        error: bool,
    }
    impl Surface for Probe {
        fn bind(&mut self, _: &[Value]) -> Result<(), SurfaceError> {
            self.output = true;
            Ok(())
        }
        fn bind_at(&mut self, v: &[Value], at: Option<f64>) -> Result<(), SurfaceError> {
            self.at = at.unwrap_or_default();
            self.bind(v)
        }
        fn render(
            &mut self,
            _: &Frame,
            _: &wgpu::Device,
            _: &wgpu::Queue,
            _: &wgpu::TextureView,
            _: wgpu::TextureFormat,
        ) -> bool {
            panic!("headless never draws")
        }
        fn wants_input(&self) -> bool {
            true
        }
        fn input(&mut self, _: &InputEvent) {
            self.output = true;
        }
        fn messages(&mut self) -> Vec<String> {
            if self.output {
                vec!["message".into()]
            } else {
                vec![]
            }
        }
        fn published(&mut self) -> Option<String> {
            std::mem::take(&mut self.output).then(|| "{}".into())
        }
        fn agent(&mut self, q: &str) -> Option<String> {
            self.error = q == "fail";
            self.output = true;
            Some(self.at.to_string())
        }
        fn take_error(&mut self) -> Option<SurfaceError> {
            std::mem::take(&mut self.error).then(|| SurfaceError("probe refused".into()))
        }
        fn carry(&mut self) -> Option<Vec<u8>> {
            Some(self.bytes.clone())
        }
        fn restore(&mut self, b: &[u8]) -> Result<(), String> {
            self.bytes = b.to_vec();
            self.output = true;
            Ok(())
        }
    }
    static REGISTRY: Registry = Registry {
        surfaces: &[("probe", 0, || Box::<Probe>::default())],
        shaders: &[("unregistered", 123)],
    };
    let mut m = Module::new(&REGISTRY);
    assert!(m.create_headless("missing").is_none());
    assert!(m.take_error().contains("missing"));
    let id = m.create_headless("probe").unwrap();
    assert!(!m.has_device(id));
    assert!(m.bind(id, &[], None));
    assert!(m.bind(id, &[], Some(23.)));
    assert_eq!(m.agent(id, "state").as_deref(), Some("23"));
    assert!(m.wants_input(id));
    assert!(m.input(id, &InputEvent::Blur { at_ms: 23. }));
    assert!(!m.take_messages(id).is_empty());
    assert!(m.take_messages(id).is_empty());
    assert_eq!(m.take_published(id).as_deref(), Some("{}"));
    assert!(m.take_published(id).is_none());
    assert!(m.restore(id, &[1, 2, 3]));
    assert_eq!(m.carry(id), Some(vec![1, 2, 3]));
    assert_eq!(m.take_published(id).as_deref(), Some("{}"));
    let f = Frame {
        width: 10.,
        height: 10.,
        scale: 1.,
        now_ms: 23.,
        seekable: true,
        period_ms: 0.0,
        children_generation: 0,
        shader_generation: 0,
    };
    for _ in 0..2 {
        assert_eq!(m.render(id, &f), None);
        assert_eq!(m.take_error(), "");
    }
    m.lose_device();
    assert_eq!(m.agent(id, "state").as_deref(), Some("23"));
    assert!(m.agent(id, "fail").is_none());
    assert_eq!(m.take_error(), "probe refused");
    assert_eq!(m.take_error(), "");
    assert!(m.restore(id, &[]));
    assert_eq!(m.carry(id), Some(vec![]));
    m.destroy(id);
    assert!(m.agent(id, "state").is_none());
    assert_eq!(m.take_error(), "no such canvas");
}

#[test]
fn assets_are_validated_drained_and_delivered_without_a_device() {
    use exact_gpu::{wgpu, Frame, Surface, SurfaceError, Value};
    #[derive(Default)]
    struct AssetProbe {
        wanted: Vec<String>,
        delivered: Vec<String>,
    }
    impl Surface for AssetProbe {
        fn bind(&mut self, _: &[Value]) -> Result<(), SurfaceError> {
            self.wanted = vec![
                "tables/lookup.bin".into(),
                "missing.bin".into(),
                "tables/lookup.bin".into(),
                "../escape".into(),
                "/absolute".into(),
                "a//b".into(),
                "x/../y".into(),
                "雪".into(),
            ];
            Ok(())
        }
        fn assets(&mut self) -> Vec<String> {
            std::mem::take(&mut self.wanted)
        }
        fn asset(&mut self, name: &str, bytes: Option<&[u8]>) {
            self.delivered.push(format!("{name}:{bytes:?}"));
            self.wanted.push(name.into());
            if name == "tables/lookup.bin" {
                self.wanted.push("next.bin".into());
            }
        }
        fn messages(&mut self) -> Vec<String> {
            std::mem::take(&mut self.delivered)
        }
        fn render(
            &mut self,
            _: &Frame,
            _: &wgpu::Device,
            _: &wgpu::Queue,
            _: &wgpu::TextureView,
            _: wgpu::TextureFormat,
        ) -> bool {
            false
        }
    }
    static ASSETS: Registry = Registry {
        surfaces: &[("lookup", 0, || Box::<AssetProbe>::default())],
        shaders: &[],
    };
    let mut m = Module::new(&ASSETS);
    let id = m.create_headless("lookup").unwrap();
    assert!(m.bind(id, &[], None));
    assert_eq!(m.take_assets(id), ["tables/lookup.bin", "missing.bin"]);
    assert!(m.take_error().contains("asset `雪`"));
    assert!(m.take_assets(id).is_empty());
    assert!(!m.asset(id, "../escape", Some(&[1])));
    assert!(!m.asset(id, "unrequested", Some(&[1])));
    assert!(m.asset(id, "tables/lookup.bin", Some(&[1, 2, 3])));
    assert_eq!(m.take_assets(id), ["next.bin"]);
    assert!(m.take_assets(id).is_empty());
    assert!(m.asset(id, "missing.bin", None));
    assert!(m.asset(id, "next.bin", Some(&[])));
    assert_eq!(
        m.take_messages(id),
        [
            "tables/lookup.bin:Some([1, 2, 3])",
            "missing.bin:None",
            "next.bin:Some([])"
        ]
    );
    assert!(m.take_messages(id).is_empty());
    assert!(!m.asset(id, "next.bin", None));
}

#[test]
fn lifecycle_and_clock_reach_surfaces_without_a_device() {
    use exact_gpu::{wgpu, Frame, Lifecycle, Surface, SurfaceError, Value};
    #[derive(Default)]
    struct Probe {
        events: Vec<String>,
    }
    impl Surface for Probe {
        fn bind(&mut self, _: &[Value]) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn render(
            &mut self,
            _: &Frame,
            _: &wgpu::Device,
            _: &wgpu::Queue,
            _: &wgpu::TextureView,
            _: wgpu::TextureFormat,
        ) -> bool {
            false
        }
        fn clock(&mut self, seekable: bool) {
            self.events.push(format!("clock:{seekable}"));
        }
        fn lifecycle(&mut self, event: Lifecycle) {
            self.events.push(format!("{event:?}"));
        }
        fn agent(&mut self, _: &str) -> Option<String> {
            Some(self.events.join(","))
        }
    }
    static REGISTRY: Registry = Registry {
        surfaces: &[("probe", 0, || Box::new(Probe::default()))],
        shaders: &[],
    };
    let mut m = Module::new(&REGISTRY);
    let live = m.create_headless("probe").unwrap();
    assert_eq!(m.agent(live, "").unwrap(), "clock:false");
    m.set_seekable(true);
    m.set_seekable(true); // only changes are delivered
    let agent = m.create_headless("probe").unwrap();
    for id in [live, agent] {
        for code in [0, 1, 2, 3, 999] {
            m.lifecycle(id, code);
        }
    }
    assert_eq!(
        m.agent(live, "").unwrap(),
        "clock:false,clock:true,Hidden,Visible,AudioInterrupted,AudioResumed"
    );
    assert_eq!(
        m.agent(agent, "").unwrap(),
        "clock:true,Hidden,Visible,AudioInterrupted,AudioResumed"
    );
    m.set_seekable(false);
    assert!(m.agent(agent, "").unwrap().ends_with("clock:false"));
    exact_gpu::native::load_headless(&REGISTRY);
    let id = exact_gpu::native::create_headless("probe");
    assert_eq!(exact_gpu::native::agent(id, ""), "clock:true");
    exact_gpu::native::unload();
}

#[test]
fn asset_names_use_the_portable_bounded_ascii_path_grammar() {
    for name in [
        "a",
        "a/b.tex",
        "space name",
        "x:y",
        "x%20y",
        "x?y#z",
        "..foo",
        "a_-.tex",
    ] {
        assert!(exact_gpu::asset_name(name), "rejected {name:?}");
    }
    for name in [
        "", "/a", "a/", "a//b", ".", "..", "a/./b", "a/../b", "a\\b", "雪", "a\n", "a\u{7f}",
    ] {
        assert!(!exact_gpu::asset_name(name), "accepted {name:?}");
    }
    assert!(exact_gpu::asset_name(&"a".repeat(128)));
    assert!(!exact_gpu::asset_name(&"a".repeat(129)));
}
