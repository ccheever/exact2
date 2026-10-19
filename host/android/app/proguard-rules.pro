# The app's Kotlin hatches are made by name (dev.exact.canvas.HatchReader).
-keep class exact.app.Hatches { <init>(); }

# The optional app module roster is resolved at its first native view.
-keep class exact.app.Modules { <init>(); }

# The roster of the modules' implementations the APK carries (dev.exact.canvas.Extensions).
-keep class exact.app.ExactExtensions { <init>(); }
