//! The Tamber binding host on Android's Canvas host (LLP 1121 D12): the host's JNI
//! (`exact_linux::canvas_jni!`) over the app's data source, its baked plan and
//! its hatches. The reader (`host/android`) loads this library as
//! `libexactcanvas.so`.

#[cfg(target_os = "android")]
include!(concat!(env!("OUT_DIR"), "/hatches.rs"));

#[cfg(target_os = "android")]
exact_linux::canvas_jni!(
    tamber_host_data::TamberHost,
    hatches = crate::exact_hatches()
);
