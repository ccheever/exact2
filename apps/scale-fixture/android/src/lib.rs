//! The base-scale fixture (LLP 1121 stage 11 C: the Material column, which
//! scripts/scale-check.mjs holds the reader to) on Android's Canvas host (LLP
//! 1076 §3.3): the host's JNI (`exact_linux::canvas_jni!`) over a data source
//! that answers nothing and the baked plan. The reader (`host/android`) loads this library as
//! `libexactcanvas.so`.

#[cfg(target_os = "android")]
include!(concat!(env!("OUT_DIR"), "/hatches.rs"));

#[cfg(target_os = "android")]
exact_linux::canvas_jni!(hello_data::Hello, hatches = crate::exact_hatches());
