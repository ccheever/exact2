//! The auth fixture on Android's Canvas host (LLP 1076 §3.3): the host's JNI
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
        exact_js::Module::new(baked::APP_BYTECODE, baked::APP, baked::GRANTS)
            .placed(baked::TYPESCRIPT_PLACEMENT)
    }
    include!(concat!(env!("OUT_DIR"), "/logic.rs"));
    include!(concat!(env!("OUT_DIR"), "/hatches.rs"));

    exact_linux::canvas_jni!(super::AppData, hatches = crate::android::exact_hatches());
}
