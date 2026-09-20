//! Exercise the real C exports in subprocesses: a parser/borrow abort must fail the test.
use exact_gpu::{wgpu, Frame, Registry, Restore, Surface, SurfaceError, Value};

#[derive(Default)]
struct Probe {
    mode: String,
}
impl Surface for Probe {
    fn bind(&mut self, _: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
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
    fn agent(&mut self, request: &str) -> Option<String> {
        self.mode = request.into();
        Some(match request {
            "\"large\"" => "x".repeat(65_537),
            "\"maximum\"" => "x".repeat(65_536),
            _ => "ok".into(),
        })
    }
    fn published(&mut self) -> Option<String> {
        (self.mode == "\"published\"").then(|| "x".repeat(65_537))
    }
    fn messages(&mut self) -> Vec<String> {
        match self.mode.as_str() {
            "\"messages\"" => vec![String::new(); 1025],
            "\"escaped\"" => vec!["\n".repeat(40_000)],
            _ => vec![],
        }
    }
    fn carry(&mut self) -> Result<Option<Vec<u8>>, SurfaceError> {
        Ok((self.mode == "\"carry\"").then(|| vec![0; 256 * 1024 * 1024 + 1]))
    }
    fn restore(&mut self, _: &[u8], _: Restore) -> Result<(), String> {
        Ok(())
    }
    fn take_error(&mut self) -> Option<SurfaceError> {
        (self.mode == "\"error\"").then(|| SurfaceError("é".repeat(40_000)))
    }
}
static REGISTRY: Registry = Registry {
    surfaces: &[("probe", 0, || Box::<Probe>::default())],
    shaders: &[],
};
exact_gpu::module!(headless REGISTRY);

fn output(len: u32) -> String {
    // SAFETY: the module owns this buffer until the next synchronous export call.
    unsafe {
        String::from_utf8(std::slice::from_raw_parts(gpu_out_ptr(), len as usize).to_vec()).unwrap()
    }
}
fn run(case: &str) {
    gpu_load_headless();
    // SAFETY: every pointer below refers to a live byte range for the call.
    unsafe {
        let id = gpu_create_headless(b"probe".as_ptr(), 5);
        assert_ne!(id, 0);
        assert_eq!(gpu_bind(id, b"[]".as_ptr(), 2), 0);
        if case == "deep-bind" {
            let text = format!("{}0{}", "[".repeat(100_000), "]".repeat(100_000));
            assert_eq!(gpu_bind(id, text.as_ptr(), text.len()), 1);
            assert!(output(gpu_error()).contains("16384"));
        } else if let Some(op) = case.strip_prefix("request-") {
            for text in [
                format!("[\"{}\"]", "x".repeat(20_000)),
                format!("{}0{}", "[".repeat(100_000), "]".repeat(100_000)),
                format!("{}0{}", "[".repeat(65), "]".repeat(65)),
            ] {
                match op {
                    "bind" => assert_eq!(gpu_bind(id, text.as_ptr(), text.len()), 1),
                    "bind_at" => assert_eq!(gpu_bind_at(id, text.as_ptr(), text.len(), 0.), 1),
                    "agent" => assert_eq!(gpu_agent(id, text.as_ptr(), text.len()), 0),
                    "input" => assert_eq!(gpu_input(id, text.as_ptr(), text.len()), 1),
                    _ => unreachable!(),
                }
                let error = output(gpu_error());
                assert!(
                    error.contains("16384") || error.contains("depth 64"),
                    "{error}"
                );
            }
        } else if case == "restore" {
            assert!(!gpu_restore(id, b"x".as_ptr(), 1, 2));
            assert!(output(gpu_error()).contains("mode"));
            let oversized = vec![0; 256 * 1024 * 1024 + 1];
            assert!(!gpu_restore(id, oversized.as_ptr(), oversized.len(), 0));
            assert!(output(gpu_error()).contains("256 MiB"));
        } else {
            let text = format!("\"{case}\"");
            let len = gpu_agent(id, text.as_ptr(), text.len());
            match case {
                "large" => assert_eq!(len, 0),
                "carry" => assert_eq!(gpu_carry(id), u32::MAX - 1),
                "published" => assert_eq!(gpu_published(id), u32::MAX),
                "messages" | "escaped" => assert_eq!(gpu_messages(id), u32::MAX),
                "error" => {}
                _ => unreachable!(),
            }
            let error = output(gpu_error());
            assert!(!error.is_empty());
            assert!(error.len() <= 4096, "error bytes: {}", error.len());
        }
        assert_eq!(gpu_bind(id, b"[]".as_ptr(), 2), 0);
        let maximum = format!("[\"{}\"]", "x".repeat(16_380));
        assert_eq!(gpu_bind(id, maximum.as_ptr(), maximum.len()), 0);
        let depth = format!("{}0{}", "[".repeat(64), "]".repeat(64));
        assert_eq!(gpu_bind(id, depth.as_ptr(), depth.len()), 0);
        let reply = b"\"maximum\"";
        assert_eq!(gpu_agent(id, reply.as_ptr(), reply.len()), 65_536);
    }
    gpu_unload();
}
#[test]
fn native_owned_exports_are_bounded_in_child_processes() {
    if let Ok(case) = std::env::var("EXACT_NATIVE_CASE") {
        run(&case);
        return;
    }
    let mut failures = vec![];
    for case in [
        "deep-bind",
        "request-bind",
        "request-bind_at",
        "request-agent",
        "request-input",
        "restore",
        "carry",
        "large",
        "published",
        "messages",
        "escaped",
        "error",
    ] {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "native_owned_exports_are_bounded_in_child_processes",
                "--nocapture",
            ])
            .env("EXACT_NATIVE_CASE", case)
            .output()
            .unwrap();
        if !out.status.success() {
            failures.push(format!(
                "{case}: {}\n{}",
                out.status,
                String::from_utf8_lossy(&out.stderr)
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
