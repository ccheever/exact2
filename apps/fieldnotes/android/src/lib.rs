//! Fieldnotes on Android's Canvas host (LLP 1076 §3.3): the host's JNI
//! (`exact_linux::canvas_jni!`) over the notes' TypeScript (Hermes bytecode),
//! the Rust backup source and the baked plan, with app storage. The reader
//! (`host/android`) loads this library as `libexactcanvas.so`.

#[cfg(target_os = "android")]
mod android {
    #[allow(dead_code)]
    mod baked {
        include!(concat!(env!("OUT_DIR"), "/module.rs"));
    }
    use baked::*;

    type ExactEmbeddedData =
        exact_data_host::Storage<fieldnotes_data::Data<exact_js::Placed<exact_js::Module>>>;
    fn embedded_data() -> ExactEmbeddedData {
        exact_data_host::Storage::new(fieldnotes_data::mixed(
            exact_js::Module::new(APP_BYTECODE, APP, GRANTS).placed(TYPESCRIPT_PLACEMENT),
            RUST_PLACEMENT,
        ))
    }
    include!(concat!(env!("OUT_DIR"), "/logic.rs"));
    include!(concat!(env!("OUT_DIR"), "/hatches.rs"));

    exact_linux::canvas_jni!(super::AppData, hatches = crate::android::exact_hatches());
}
