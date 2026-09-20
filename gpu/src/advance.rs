use crate::{Module, SurfaceError};
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
    struct Probe;
    impl Surface for Probe {
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
    static REGISTRY: Registry = Registry {
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
