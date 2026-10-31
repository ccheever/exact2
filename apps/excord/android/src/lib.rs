//! Excord on Android's Canvas host (LLP 1076 §3.3): the host's JNI
//! (`exact_linux::canvas_jni!`) over the app's TypeScript data module
//! (`app.ts` as Hermes bytecode, run by the lean Hermes VM in this library)
//! and its baked plan. The reader (`host/android`) loads this library as
//! `libexactcanvas.so`.

#[cfg(target_os = "android")]
mod android {
    // The bake's identity and placement; the rest of what it records goes unused.
    #[allow(dead_code)]
    mod baked {
        include!(concat!(env!("OUT_DIR"), "/module.rs"));
    }

    type ExactEmbeddedData = exact_js::Placed<exact_js::Module>;
    fn embedded_data() -> ExactEmbeddedData {
        let mut module =
            exact_js::Module::new(baked::APP_BYTECODE, baked::APP, baked::GRANTS).with_native(host);
        // A large account's READY is megabytes of JSON, and parsed it is several times that: a 16 MB READY
        // ran out of the default 64 MiB heap.
        module.set_max_heap(256 << 20);
        module.placed(baked::TYPESCRIPT_PLACEMENT)
    }

    /// The app's native module: which host this is, which the data module cannot tell
    /// (`live.ts` `hostOf`, as `modules/apple/Host.swift` answers on Apple).
    struct Host;
    impl exact_js::NativeModule for Host {
        fn configure_storage(&mut self, _storage: exact_js::NativeStorage) -> Result<(), String> {
            Ok(())
        }
        fn call(&mut self, request: &serde_json::Value) -> Result<serde_json::Value, String> {
            match request["op"].as_str() {
                Some("host") => Ok(serde_json::json!({ "host": "android" })),
                _ => Err("host answers op host".into()),
            }
        }
    }
    fn host(_source: &str) -> Box<dyn exact_js::NativeModule> {
        Box::new(Host)
    }
    include!(concat!(env!("OUT_DIR"), "/logic.rs"));
    include!(concat!(env!("OUT_DIR"), "/hatches.rs"));

    exact_linux::canvas_jni!(super::AppData, hatches = crate::android::exact_hatches());
}
