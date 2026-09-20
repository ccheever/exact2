use crate::{Module, SurfaceError};
// An agent may already have explained a failure in its bounded JSON reply.
// Preserve that explanation; ordinary surface errors still refuse the call.
pub(crate) fn error_reply(reply: Option<&str>) -> bool {
    reply
        .and_then(|text| crate::json::parse_fields(text).ok())
        .is_some_and(|fields| {
            fields
                .iter()
                .any(|(name, value)| name == "error" && value.as_str().is_some())
        })
}
impl Module {
    /// Drive an owned surface without rendering, JSON requests or observation.
    pub fn advance(&mut self, id: u32, now_ms: f64) -> bool {
        let Some(inst) = self.instances.get_mut(&id) else {
            return self.fail::<()>("no such canvas").is_some();
        };
        let changed = inst.surface.advance(now_ms);
        if let Some(SurfaceError(error)) = inst.surface.take_error() {
            self.error = error;
            return false;
        }
        if changed {
            inst.drain();
            inst.dirty = true;
        }
        changed
    }
}
#[cfg(test)]
mod tests {
    use crate::*;
    pub(super) struct Probe;
    impl Surface for Probe {
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
        fn bind(&mut self, _: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
            Ok(())
        }
        fn agent(&mut self, _: &str) -> Option<String> {
            Some("{\"error\":\"original failure\"}".into())
        }
        fn take_error(&mut self) -> Option<SurfaceError> {
            Some(SurfaceError("original failure".into()))
        }
    }
    pub(super) static REGISTRY: Registry = Registry {
        surfaces: &[("probe", 0, || Box::new(Probe))],
        shaders: &[],
    };
    #[test]
    fn agent_failure_keeps_structured_reply() {
        let mut m = Module::new(&REGISTRY);
        let id = m.create_headless("probe").unwrap();
        assert_eq!(
            m.agent(id, "{}"),
            Some("{\"error\":\"original failure\"}".into())
        );
        assert_eq!(m.take_error(), "original failure");
    }
    #[test]
    fn regular_owner_refuses_exhaustion_without_wrapping_or_running_factory() {
        let mut m = Module::new(&REGISTRY);
        m.next = u32::MAX;
        assert!(m.create_headless("probe").is_none());
        assert!(m.take_error().contains("limit"));
        assert!(m.instances.is_empty());
        m.next = 0;
        for _ in 0..256 {
            assert!(m.create_headless("probe").is_some());
        }
        assert!(m.create_headless("probe").is_none());
        assert_eq!(m.instances.len(), 256);
    }
}

#[cfg(test)]
mod device_identity {
    use super::tests::{Probe, REGISTRY};
    use crate::*;
    #[test]
    fn device_insertion_retains_the_f418821_range() {
        let mut m = Module::new(&REGISTRY);
        for _ in 0..257 {
            assert!(m.insert(|| Box::new(Probe), None).is_some());
        }
        assert!(
            m.create_headless("probe").is_none(),
            "owned capacity remains bounded"
        );
        m.next = u32::MAX - 1;
        assert_eq!(m.insert(|| Box::new(Probe), None), Some(u32::MAX));
    }
    #[test]
    fn presentable_device_agent_error_still_returns_no_reply() {
        let mut m = Module::new(&REGISTRY);
        let id = m.insert(|| Box::new(Probe), None).unwrap();
        // Config survives device loss: classification must not depend on a live target.
        m.instances.get_mut(&id).unwrap().config = Some(wgpu::SurfaceConfiguration {
            color_space: Default::default(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Rgba8Unorm,
            width: 1,
            height: 1,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        });
        assert_eq!(m.agent(id, "{}"), None);
        assert_eq!(m.take_error(), "original failure");
    }
}
