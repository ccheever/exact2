# An app that runs on Android 10 or 11 (build.gradle.kts): the reader's code for those releases, kept as the rest of the
# reader is (canvas/proguard-rules.pro). The Rust calls Legacy.decode by name.
-keep class dev.exact.canvas.Legacy* { *; }
