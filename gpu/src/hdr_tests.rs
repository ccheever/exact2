//! LLP 1100 D12b on a real `CAMetalLayer`: a surface that asks for HDR gets
//! half floats in extended sRGB, EDR and its host's headroom; one that
//! doesn't keeps the 8-bit target and a headroom of 1.

use crate::{fixture, wgpu, Frame, Module, Registry, Surface, SurfaceError, Value};
use std::sync::Mutex;

#[link(name = "QuartzCore", kind = "framework")]
extern "C" {}

/// What each surface's last frame was told: (headroom, format).
static SEEN: Mutex<Vec<(&'static str, f32, wgpu::TextureFormat)>> = Mutex::new(Vec::new());

struct Probe(&'static str, bool);
impl Surface for Probe {
    fn high_dynamic_range(&self) -> bool {
        self.1
    }
    fn bind(&mut self, _: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn render(
        &mut self,
        frame: &Frame,
        _: &wgpu::Device,
        _: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> bool {
        SEEN.lock().unwrap().push((self.0, frame.headroom, format));
        // Twice SDR white: kept by the extended target, clipped by the 8-bit one.
        let white = f64::from(frame.headroom.min(2.0));
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: white,
                        g: white,
                        b: white,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        false
    }
}

static REGISTRY: Registry = Registry {
    surfaces: &[
        ("hdr", 0, || Box::new(Probe("hdr", true))),
        ("sdr", 0, || Box::new(Probe("sdr", false))),
    ],
    shaders: &[],
};

fn frame() -> Frame {
    Frame {
        width: 8.,
        height: 8.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.0,
    }
}

#[test]
fn an_hdr_surface_gets_an_extended_target_and_its_hosts_headroom() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut m = Module::new(&REGISTRY);
    m.set_gpu(gpu);
    m.set_seekable(true);
    let mut layers = Vec::new();
    let mut make = |m: &mut Module, name: &str| {
        // SAFETY: a new CAMetalLayer, kept past the module.
        let layer: objc2::rc::Retained<objc2::runtime::AnyObject> =
            unsafe { objc2::msg_send![objc2::class!(CAMetalLayer), new] };
        let ptr = objc2::rc::Retained::as_ptr(&layer) as *mut std::ffi::c_void;
        // SAFETY: the layer outlives the module.
        let target = unsafe {
            m.gpu()
                .unwrap()
                .instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(ptr))
        }
        .unwrap();
        let id = m.create(name, target, 8, 8).unwrap();
        assert!(m.bind(id, &[], None));
        layers.push(layer);
        id
    };
    let (hdr, sdr) = (make(&mut m, "hdr"), make(&mut m, "sdr"));
    assert!(m.high_dynamic_range(hdr));
    assert!(!m.high_dynamic_range(sdr));
    // SAFETY: CAMetalLayer's properties, on layers alive above.
    let (edr, format): (bool, usize) = unsafe {
        (
            objc2::msg_send![&*layers[0], wantsExtendedDynamicRangeContent],
            objc2::msg_send![&*layers[0], pixelFormat],
        )
    };
    assert!(edr, "Metal EDR on");
    assert_eq!(format, 115, "MTLPixelFormatRGBA16Float");
    // SAFETY: as above.
    let sdr_edr: bool = unsafe { objc2::msg_send![&*layers[1], wantsExtendedDynamicRangeContent] };
    assert!(!sdr_edr);

    m.set_headroom(hdr, 3.5);
    m.set_headroom(sdr, 3.5);
    m.set_headroom(hdr, f32::NAN);
    m.set_headroom(hdr, 3.5);
    assert!(m.dirty(hdr), "a new headroom is a new frame");
    for id in [hdr, sdr] {
        assert_eq!(m.render(id, &frame()), Some(false));
    }
    assert!(m.flush());
    let seen = SEEN.lock().unwrap().clone();
    assert!(
        seen.contains(&("hdr", 3.5, wgpu::TextureFormat::Rgba16Float)),
        "{seen:?}"
    );
    assert!(
        seen.iter()
            .any(|s| s.0 == "sdr" && s.1 == 1.0 && s.2 != wgpu::TextureFormat::Rgba16Float),
        "{seen:?}"
    );
    drop(m);
    drop(layers);
}
