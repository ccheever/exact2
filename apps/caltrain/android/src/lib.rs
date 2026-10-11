//! Caltrain on Android's Canvas host (LLP 1076 §3.3): the host's JNI
//! (`exact_linux::canvas_jni!`) over the app's data source and its baked plan.
//! The reader (`host/android`) loads this library as `libexactcanvas.so`.

#[cfg(target_os = "android")]
exact_linux::canvas_jni!(caltrain_data::Caltrain);
