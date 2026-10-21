-keep class dev.exact.canvas.Native { *; }
# The reader as written: R8 may not inline or merge across its methods. Without ExoPlayer in the program R8 inlined into
# Layers.sample (and the frame callbacks) a shape ART's compiler leaves to the interpreter (main thread +55 ms/s on crypto).
# Android 10 and 11's code (Legacy.kt, LegacyGlyphs.kt) is the app's to keep, below Android 12 only
# (app/proguard-legacy.pro); for any other app R8 folds each SDK_INT check that leads there and drops it.
-keep class !dev.exact.canvas.Legacy*,dev.exact.canvas.** { *; }
