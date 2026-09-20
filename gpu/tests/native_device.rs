use exact_gpu::{wgpu, Frame, Registry, Surface, SurfaceError, Value};
struct Probe;
impl Surface for Probe {
    fn bind(&mut self, values: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        assert_eq!(values[0].as_str().unwrap().len(), 20 * 1024);
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
}
static REGISTRY: Registry = Registry {
    surfaces: &[("probe", 1, || Box::new(Probe))],
    shaders: &[],
};
exact_gpu::module!(REGISTRY);
#[test]
fn device_exports_keep_twenty_kib_bindings() {
    if std::env::var_os("EXACT_DEVICE_CHILD").is_none() {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "device_exports_keep_twenty_kib_bindings"])
            .env("EXACT_DEVICE_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        return;
    }
    gpu_load_headless();
    let text = format!("[\"{}\"]", "x".repeat(20 * 1024));
    // SAFETY: both byte ranges remain live during the actual device ABI calls.
    unsafe {
        let id = gpu_create_headless(b"probe".as_ptr(), 5);
        assert_ne!(id, 0);
        assert_eq!(gpu_bind(id, text.as_ptr(), text.len()), 0);
        assert_eq!(gpu_error(), 0);
    }
    gpu_unload();
}
