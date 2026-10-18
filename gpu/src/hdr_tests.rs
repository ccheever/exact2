//! LLP 1100 D12b on a real `CAMetalLayer`: a surface that asks for HDR gets
//! half floats in extended sRGB, EDR and its host's headroom; one that
//! doesn't keeps the 8-bit target and a headroom of 1, through an sRGB view
//! when it asks for one.

use crate::{fixture, wgpu, Frame, Module, Registry, Surface, SurfaceError, Value};
use std::sync::Mutex;

#[link(name = "QuartzCore", kind = "framework")]
extern "C" {}

/// What each surface's last frame was told: (headroom, format).
static SEEN: Mutex<Vec<(&'static str, f32, wgpu::TextureFormat)>> = Mutex::new(Vec::new());

/// Its name, whether it asks for HDR, and whether for an sRGB target.
struct Probe(&'static str, bool, bool, f32, f64);
impl Surface for Probe {
    fn opaque(&self) -> bool {
        self.4 == 1.0
    }
    fn render_scale(&self) -> f32 {
        self.3
    }
    fn high_dynamic_range(&self) -> bool {
        self.1
    }
    fn srgb_target(&self) -> bool {
        self.2
    }
    fn bind(&mut self, values: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        if let Some(Value::Number(scale)) = values.first() {
            self.3 = *scale as f32;
        }
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
        // Twice SDR white: kept by the extended target, clipped by the 8-bit
        // one. Through an sRGB view, linear mid-grey, which the view encodes.
        let white = if self.2 {
            0.5
        } else {
            f64::from(frame.headroom.min(2.0))
        };
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
                        a: self.4,
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
        ("hdr", 0, || Box::new(Probe("hdr", true, true, 1.0, 1.0))),
        ("sdr", 0, || Box::new(Probe("sdr", false, false, 1.0, 0.5))),
        ("srgb", 0, || Box::new(Probe("srgb", false, true, 1.0, 1.0))),
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
    let mut make = |m: &mut Module, name: &str, headless: bool| {
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
        let id = if headless {
            let id = m.create_headless(name).unwrap();
            assert_eq!(
                m.presentation_flags(id),
                if name == "hdr" { 1 } else { 3 },
                "the HDR request is visible before attachment"
            );
            assert!(m.attach(id, target, 8, 8));
            id
        } else {
            m.create(name, target, 8, 8).unwrap()
        };
        assert!(m.bind(id, &[], None));
        layers.push(layer);
        id
    };
    let (hdr, sdr, srgb) = (
        make(&mut m, "hdr", true),
        make(&mut m, "sdr", false),
        make(&mut m, "srgb", true),
    );
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
    // SAFETY: as above.
    let srgb_format: usize = unsafe { objc2::msg_send![&*layers[2], pixelFormat] };
    assert_eq!(
        srgb_format, 80,
        "MTLPixelFormatBGRA8Unorm: the bytes shown are unchanged"
    );

    // The same surface type has different clear alpha on each instance.
    for (i, opaque) in [(0, true), (1, false), (2, true)] {
        let actual: bool = unsafe { objc2::msg_send![&*layers[i], isOpaque] };
        assert_eq!(actual, opaque);
    }
    assert_eq!(
        m.instances[&hdr].config.as_ref().unwrap().alpha_mode,
        wgpu::CompositeAlphaMode::Opaque
    );
    assert_eq!(
        m.instances[&sdr].config.as_ref().unwrap().alpha_mode,
        wgpu::CompositeAlphaMode::PostMultiplied
    );

    m.set_headroom(hdr, 3.5);
    m.set_headroom(sdr, 3.5);
    m.set_headroom(hdr, f32::NAN);
    m.set_headroom(hdr, 3.5);
    assert!(m.dirty(hdr), "a new headroom is a new frame");
    for id in [hdr, sdr, srgb] {
        assert_eq!(m.render(id, &frame()), Some(false));
    }
    assert!(m.flush());
    let (pixels, _) = m.readback(srgb, &frame()).unwrap();
    assert!(
        pixels.at(4, 4)[..3].iter().all(|c| c.abs_diff(188) <= 1),
        "linear 0.5, encoded by the view: {:?}",
        pixels.at(4, 4)
    );
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
    assert!(
        seen.contains(&("srgb", 1.0, wgpu::TextureFormat::Bgra8UnormSrgb)),
        "{seen:?}"
    );
    for (scale, size) in [
        (0.75, 12),
        (0.5, 8),
        (1.0, 16),
        (0.0, 16),
        (f64::NAN, 16),
        (2.0, 16),
    ] {
        assert!(m.bind(srgb, &[Value::Number(scale)], None));
        let f = Frame {
            width: 8.0,
            height: 12.0,
            scale: 2.0,
            ..frame()
        };
        assert_eq!(m.render(srgb, &f), Some(false));
        assert!(m.flush());
        let config = m.instances[&srgb].config.as_ref().unwrap();
        assert_eq!((config.width, config.height), (size, size * 3 / 2));
        let (capture, _) = m.readback(srgb, &f).unwrap();
        assert_eq!((capture.width, capture.height), (16, 24));
        assert!(capture.at(8, 12)[..3].iter().all(|c| c.abs_diff(188) <= 1));
    }
    drop(m);
    drop(layers);
}
