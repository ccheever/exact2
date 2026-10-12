-keep class dev.exact.canvas.Native { *; }
# The reader as written: R8 may not inline or merge across its methods. Without ExoPlayer in the program R8 inlined into
# Layers.sample (and the frame callbacks) a shape ART's compiler leaves to the interpreter (main thread +55 ms/s on crypto).
-keep class dev.exact.canvas.** { *; }
