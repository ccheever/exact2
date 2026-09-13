//! The app links its native capability; the core host and JS executor do not.
struct Snapback(exact_snapback4::Module);

impl exact_js::NativeModule for Snapback {
    fn configure_storage(
        &mut self,
        data: std::path::PathBuf,
        cache: std::path::PathBuf,
        temporary: std::path::PathBuf,
    ) -> Result<(), String> {
        self.0.configure_storage(data, cache, temporary)
    }

    fn call(&mut self, request: &serde_json::Value) -> Result<serde_json::Value, String> {
        self.0.call(request)
    }
}

pub fn module(bytecode: &[u8], app: &str, grants: &str) -> exact_js::Module {
    exact_js::Module::new(bytecode.to_vec(), app, grants).with_native(|grants| {
        Box::new(Snapback(
            exact_snapback4::Module::new("com.exact.messages", grants)
                .expect("baked Messages grants"),
        ))
    })
}
