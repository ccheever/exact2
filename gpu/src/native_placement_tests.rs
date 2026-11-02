use super::{bind, child, create_headless, error, load, recover, unload, with};
use crate::{wgpu, Frame, Placement, Registry, Surface, SurfaceError, Value};
#[derive(Default)]
struct Sign {
    frame: [f32; 4],
    preparations: usize,
    retired: bool,
    formats: Vec<wgpu::TextureFormat>,
    lose_on_prepare: bool,
    child_name: String,
}
impl Surface for Sign {
    fn bind(&mut self, _: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn render(
        &mut self,
        _: &Frame,
        _: &wgpu::Device,
        _: &wgpu::Queue,
        _: &mut wgpu::CommandEncoder,
        _: &wgpu::TextureView,
        _: wgpu::TextureFormat,
    ) -> bool {
        false
    }
    fn children_mode(&self) -> crate::ChildrenMode {
        if self.retired {
            crate::ChildrenMode::Overlay
        } else {
            crate::ChildrenMode::Each
        }
    }
    fn prepare_assets(
        &mut self,
        device: &wgpu::Device,
        _: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) {
        if self.lose_on_prepare {
            self.lose_on_prepare = false;
            device.destroy();
            let _ = device.poll(wgpu::PollType::Poll);
        }
        self.formats.push(format);
        self.preparations += 1;
    }
    fn agent(&mut self, request: &str) -> Option<String> {
        self.retired = request == "retire";
        if request == "lose-on-prepare" {
            self.lose_on_prepare = true;
        }
        Some(format!(
            "{{\"preparations\":{},\"formats\":\"{:?}\",\"child\":{:?}}}",
            self.preparations, self.formats, self.child_name
        ))
    }
    fn child(&mut self, _: usize, name: &str, _: Option<&wgpu::TextureView>, frame: [f32; 4]) {
        self.child_name = name.into();
        self.frame = frame;
    }
    fn placement(&self, index: usize) -> Option<Placement> {
        (index != 0).then_some(Placement {
            clip_depth: [[0., 0., 1.]; 2],
            hidden: index == 2,
            homography: [self.frame[2]; 9],
            depth: -3.,
        })
    }
}
static REGISTRY: Registry = Registry {
    surfaces: &[("sign", 0, || Box::<Sign>::default())],
    shaders: &[],
};
crate::module!(REGISTRY);
#[test]
fn retiring_each_releases_textures_and_zero_frame_releases_a_capture() {
    assert_eq!(load(&REGISTRY, "test-gpu"), 0);
    let id = create_headless("sign");
    assert_eq!(bind(id, "[]"), 0);
    for _ in 0..3 {
        with(|m| m.agent(id, "active"));
        assert_eq!(child(id, 0, "", [0., 0., 20., 20.], [1, 1], &[255; 4]), 0);
        assert_eq!(
            with(|m| m.instances[&id].each.iter().filter(|t| t.is_some()).count()),
            Some(1)
        );
        assert_eq!(child(id, 0, "", [0.; 4], [0, 0], &[]), 0);
        assert_eq!(
            with(|m| m.instances[&id].each.iter().filter(|t| t.is_some()).count()),
            Some(0)
        );
        assert_eq!(child(id, 0, "", [0., 0., 20., 20.], [1, 1], &[255; 4]), 0);
        with(|m| m.agent(id, "retire"));
        assert_eq!(gpu_children_mode(id), 0);
        assert_eq!(with(|m| m.instances[&id].each.len()), Some(0));
        assert_eq!(
            with(|m| m.placement(id, 1).unwrap().homography),
            Some([0.; 9])
        );
    }
    unload();
}

/// A drawable acquired off the main thread and never taken is discarded
/// through its surface, so it must go before the surface does. On the
/// iPad a list retiring a row dropped it after the canvas's handle and
/// the acquiring thread's were gone, and wgpu's discard panicked. Here
/// in that order: the `Acquire` keeps the surface until the channel goes.
#[cfg(target_os = "macos")]
#[test]
fn an_untaken_acquired_texture_is_discarded_while_its_surface_exists() {
    #[link(name = "QuartzCore", kind = "framework")]
    extern "C" {}
    assert_eq!(load(&REGISTRY, "test-gpu"), 0);
    // SAFETY: retained layer outlives the module's presentation target.
    let layer: objc2::rc::Retained<objc2::runtime::AnyObject> =
        unsafe { objc2::msg_send![objc2::class!(CAMetalLayer), new] };
    let ptr = objc2::rc::Retained::as_ptr(&layer) as *mut std::ffi::c_void;
    let id = unsafe { super::create("sign", ptr, 16, 16) };
    assert_ne!(id, 0);
    let target = with(|m| m.instances[&id].presentation.clone().unwrap()).unwrap();
    let mut acquire = crate::acquire::Acquire::default();
    acquire.request(&target);
    std::thread::sleep(std::time::Duration::from_millis(200));
    // The canvas's handle goes, then the thread's; the texture has landed.
    drop(target);
    super::destroy(id);
    let answer = acquire.hang_up().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(200));
    drop(answer);
    drop(acquire);
    unload();
}

#[cfg(target_os = "macos")]
#[test]
fn presented_device_loss_rebinds_live_layer_and_readback_keeps_its_format() {
    #[link(name = "QuartzCore", kind = "framework")]
    extern "C" {}
    assert_eq!(load(&REGISTRY, "test-gpu"), 0);
    // SAFETY: retained layer outlives the module's presentation target.
    let layer: objc2::rc::Retained<objc2::runtime::AnyObject> =
        unsafe { objc2::msg_send![objc2::class!(CAMetalLayer), new] };
    let ptr = objc2::rc::Retained::as_ptr(&layer) as *mut std::ffi::c_void;
    let id = unsafe { super::create("sign", ptr, 16, 16) };
    assert_ne!(id, 0);
    assert_eq!(bind(id, "[]"), 0);
    let frame = Frame {
        width: 16.,
        height: 16.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.0,
    };
    with(|m| {
        assert!(m.render(id, &frame).is_some());
        let format = m.instances[&id].config.as_ref().unwrap().format;
        assert_eq!(format, wgpu::TextureFormat::Bgra8Unorm);
        assert!(m.readback(id, &frame).is_some());
        let state = m.agent(id, "state").unwrap();
        assert!(!state.contains("Rgba8"), "{state}");
        let gpu = m.gpu.as_ref().unwrap();
        gpu.device.destroy();
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        m.check_device();
        assert!(
            m.instances[&id].presentation.is_none(),
            "dead presentation must be dropped"
        );
        assert!(
            m.instances[&id].config.is_some(),
            "retry retains configuration"
        );
    });
    assert!(recover().contains("recovered"));
    with(|m| {
        assert!(m.has_device(id));
        assert!(m.render(id, &frame).is_some());
        assert!(m.readback(id, &frame).is_some());
        let state = m.agent(id, "state").unwrap();
        assert!(!state.contains("Rgba8"), "{state}");
    });
    unload();
}

#[test]
fn replacement_lost_during_preparation_refuses_then_retries() {
    assert_eq!(load(&REGISTRY, "test-gpu"), 0);
    let id = create_headless("sign");
    with(|m| {
        m.agent(id, "lose-on-prepare");
        m.lose_device();
    });
    let failed = recover();
    assert!(
        failed.contains("replacement device was lost during recovery"),
        "{failed}"
    );
    assert!(super::device_is_lost());
    assert!(recover().contains("recovered"));
    unload();
}

#[test]
fn recovery_of_a_healthy_device_does_not_prepare_again() {
    assert_eq!(load(&REGISTRY, "test-gpu"), 0);
    let id = create_headless("sign");
    let before = with(|m| m.agent(id, "state")).unwrap();
    assert!(recover().contains("healthy"));
    assert_eq!(with(|m| m.agent(id, "state")).unwrap(), before);
    unload();
}

#[test]
fn destroyed_device_recovers_without_replacing_the_surface_table() {
    if load(&REGISTRY, "test-gpu") != 0 {
        // Only an absent adapter skips (and fails under EXACT_REQUIRED); a load failure fails.
        crate::fixture::device_or_skip::<()>(Err(error()));
        return;
    }
    let id = create_headless("sign");
    assert_ne!(id, 0);
    assert_eq!(bind(id, "[]"), 0);
    assert_eq!(child(id, 0, "", [10., 20., 100., 50.], [0, 0], &[]), 0);
    with(|m| {
        m.instances
            .get_mut(&id)
            .unwrap()
            .messages
            .push("pending".into())
    });
    with(|m| {
        m.gpu().unwrap().device.destroy();
        let _ = m.gpu.as_ref().unwrap().device.poll(wgpu::PollType::Poll);
    });
    let result = recover();
    assert!(result.contains("recovered"), "{result}");
    assert!(result.contains("\"preparations\":1"), "{result}");
    assert_eq!(
        with(|m| m.instances.get(&id).unwrap().messages.clone()).unwrap(),
        ["pending"]
    );
    assert!(with(|m| m.instances.get(&id).unwrap().bound).unwrap());
    assert_eq!(
        with(|m| m.placement(id, 1).unwrap().homography).unwrap(),
        [100.; 9]
    );
    assert!(with(|m| m.gpu().is_some()).unwrap());
    unload();
}

#[test]
fn frame_only_child_and_explicit_hidden_out_do_not_need_a_device() {
    gpu_load_headless();
    let report = recover();
    assert!(report.contains("no device"), "{report}");
    // SAFETY: name bytes and output arrays live across these synchronous ABI calls.
    unsafe {
        let id = gpu_create_headless(b"sign".as_ptr(), 4);
        assert_ne!(id, 0);
        assert_eq!(gpu_children_mode(id), 3);
        assert_eq!(
            gpu_child_view(
                id,
                0,
                b"sign".as_ptr(),
                4,
                10.,
                20.,
                100.,
                50.,
                0,
                0,
                std::ptr::null(),
                0
            ),
            0
        );
        let name = "標識 🏮";
        assert_eq!(
            gpu_child_view(
                id,
                0,
                name.as_ptr(),
                name.len(),
                10.,
                20.,
                100.,
                50.,
                0,
                0,
                std::ptr::null(),
                0
            ),
            0
        );
        assert!(with(|m| m.agent(id, "").unwrap()).unwrap().contains(name));
        assert_eq!(
            gpu_child_view(
                id,
                0,
                [0xff].as_ptr(),
                1,
                10.,
                20.,
                100.,
                50.,
                0,
                0,
                std::ptr::null(),
                0
            ),
            1
        );
        assert!(
            with(|m| m.agent(id, "").unwrap()).unwrap().contains(name),
            "refusal leaves previous metadata intact"
        );
        let mut out = [77.; 10];
        assert_eq!(gpu_placement(id, 0, out.as_mut_ptr(), out.len()), 0);
        assert_eq!(out, [77.; 10]);
        assert_eq!(gpu_placement(id, 2, out.as_mut_ptr(), out.len()), 2);
        assert_eq!(out, [77.; 10]);
        assert_eq!(gpu_placement(id, 1, out.as_mut_ptr(), out.len()), 1);
        assert_eq!(out[..9], [100.; 9]);
        assert_eq!(out[9], -3.);
    }
    gpu_unload();
}
