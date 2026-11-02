//! @exact/photos's fixture on Android's Canvas host: the host's JNI over
//! the app's data source, with app storage for its image calls, and its
//! baked plan, which the reader loads as `libexactcanvas.so`.

#[cfg(target_os = "android")]
include!(concat!(env!("OUT_DIR"), "/hatches.rs"));

#[cfg(target_os = "android")]
exact_linux::canvas_jni!(
    exact_data_host::Storage<photos_fixture_data::Fixture>,
    hatches = crate::exact_hatches()
);
