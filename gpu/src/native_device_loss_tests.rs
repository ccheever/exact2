use crate::*;
struct Probe;
impl Surface for Probe {
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
        panic!("lost device rendered")
    }
    fn agent(&mut self, _: &str) -> Option<String> {
        Some("alive".into())
    }
}
static REGISTRY: Registry = Registry {
    surfaces: &[("probe", 0, || Box::new(Probe))],
    shaders: &[],
};
#[test]
fn device_loss_guards_each_gpu_entry_before_any_other_call() {
    for operation in 0..7 {
        let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
            return;
        };
        let mut m = Module::new(&REGISTRY);
        m.set_gpu(gpu);
        let id = m.create_headless("probe").unwrap();
        m.bind(id, &[], None);
        // Model the asynchronous callback, without calling lose_device first.
        m.device_lost.store(true, Ordering::Release);
        match operation {
            0 => assert!(!m.child(id, 0, "", [0., 0., 1., 1.], [1, 1], &[0; 4])),
            1 => assert!(!m.texture(id, 1, 1, &[0; 4])),
            2 => assert!(m
                .readback(
                    id,
                    &Frame {
                        width: 1.,
                        height: 1.,
                        scale: 1.,
                        now_ms: 0.,
                        seekable: true,
                        period_ms: 0.,
                        children_generation: 0,
                        shader_generation: 0,
                        headroom: 1.0,
                    }
                )
                .is_none()),
            3 => assert!(!m.dirty(id)),
            4 => assert!(!m.sync()),
            5 => assert!(m.gpu().is_none()),
            _ => {
                #[cfg(any(target_os = "macos", target_os = "ios", target_os = "tvos"))]
                // A sentinel must never be retained/imported after loss.
                assert!(!unsafe { m.texture_from_metal(id, 1, 1, std::ptr::dangling_mut()) });
            }
        }
        assert_eq!(m.agent(id, "state").as_deref(), Some("alive"));
        assert!(!m.dirty(id));
    }
}
