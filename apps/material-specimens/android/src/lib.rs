//! The Material oracle's specimens on Android's Canvas host (LLP 1115.001.001
//! D4): the host's JNI (`exact_linux::canvas_jni!`) over a data source that
//! answers nothing and the baked plan, which a launch's `x_plan` replaces
//! with a specimen's. The reader (`host/android`) loads this library as
//! `libexactcanvas.so`.

#[cfg(target_os = "android")]
include!(concat!(env!("OUT_DIR"), "/hatches.rs"));

#[cfg(target_os = "android")]
exact_linux::canvas_jni!(hello_data::Hello, hatches = crate::exact_hatches());
