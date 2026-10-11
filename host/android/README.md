# The Android reader

The Kotlin side of Android's Canvas host (LLP 1076 §3.3), in the repository by LLP 1115.001 D2 (Charlie, 2026-10-11).

An app on Android is its Rust, built for `aarch64-linux-android` (`host/linux`: the runner, the kernel, layout, text
and the paint walk), and this. The Rust records what it paints as an op stream (`host/linux/src/canvas.rs`). The
reader replays that stream onto Android's own Canvas: a `RenderNode` for each row, scroller and animated layer,
hardware bitmaps for pictures, `drawGlyphs` for text the Rust has shaped. The two meet at `dev.exact.canvas.Native`,
whose methods `exact_linux::canvas_jni!(Data)` defines for an app (`host/linux/src/canvas/jni.rs`). Nothing versions
the stream or those calls, so both sides are built from one commit.

| | |
|---|---|
| `canvas/` | the reader, an Android library, package `dev.exact.canvas`: `ExactActivity`, `ExactView`, `Native`, `Player` (the replay), `Owner` (exact2 on a thread of its own), `Layers`, `Natives` (hosted views), `Keys` |
| `app/` | the shell: one activity over the reader, packaging the app that `-Papp=<name>` names |
| `gradlew`, `gradle/` | the Gradle wrapper (9.8.0), committed. Its jar is Gradle's own (sha256 `238e777f…`, as services.gradle.org publishes it) and the distribution's sha256 is pinned in `gradle-wrapper.properties` |

minSdk 31, targetSdk 36, compileSdk 37.2, AGP 9.4.1, Kotlin 2.4.10, `arm64-v8a` only. Views, with no Compose,
AppCompat or Material library.

## Where it came from

The benchmark lane wrote it outside this repository. These are the files of
`expo-build-1000:~/bench/lists/exact-v11/android/canvas` at commit `c30c683` (2026-10-11). That repository has no
remote, and its history (55 commits) is not imported: for why a line is as it is, `git log -- android/canvas` there.

What changed on the way in, one commit each:

- `ExactCanvas.kt` (2,299 lines) is `Native.kt`, `Player.kt`, `ExactView.kt` and `ExactActivity.kt`: code moved by
  line range, not rewritten. `Player` is `internal` where it was file-private.
- The benchmark's driver is cut out (below). Its module (`:bench`: `Driver.kt`, `Feed.kt`) and its five apps are not here.
- The package is `dev.exact.canvas` (it was `dev.exact.bench.exactcanvas`), in Kotlin and in the JNI's function names.

## Building an app

    bun scripts/agent-android.mjs build caltrain --apk

It needs the Android SDK (`ANDROID_HOME`, else `~/Library/Android/sdk` or `~/Android/Sdk`) with platform 37.2 and an
NDK (`ANDROID_NDK_HOME`, else the newest under the SDK), a JDK (17 or newer) and the Rust target
(`rustup target add aarch64-linux-android`). It:

1. builds the app's `android/` crate for `aarch64-linux-android` at `host-dev`. The crate is a cdylib whose source is
   `exact_linux::canvas_jni!(<the data source>)`, with a build script that bakes the plan; `apps/caltrain/android` is
   the one to copy (three files, and a line in the root `Cargo.toml`'s `members`);
2. copies the library to `app/build/<app>/rust/jniLibs/arm64-v8a/libexactcanvas.so` and the app's `assets/` to
   `app/build/<app>/rust/assets/app/assets/`;
3. runs `./gradlew :app:assembleRelease -Papp=<app> -PappDir=<the app's directory>` here,

and prints `app/build/<app>/outputs/apk/release/app-release.apk`. The package id and the label are the app's
`app.json` (`app.id`, `app.name`). `ORG_GRADLE_PROJECT_idSuffix=.dev` in the environment builds a side-by-side package
(`<app id>.dev`). `./gradlew :canvas:assembleRelease` builds the reader alone.

Then `adb install -r <apk>` and `adb shell am start -n <app id>/dev.exact.app.MainActivity`. Nothing drives the APK
yet: `scripts/agent.mjs android` drives the Linux host headless (LLP 1107).

## Attaching a driver

The reader drives nothing. A benchmark, or a test, that scrolls and taps an app from outside subclasses the activity:

```kotlin
// In the benchmark's own module, which depends on this project's `:canvas`
// (settings.gradle.kts: include(":canvas"); project(":canvas").projectDir = file("<exact2>/host/android/canvas")).
open class BenchActivity(private val key: String) : ExactActivity() {
    override fun drive(scroll: (Float) -> Unit): Drive {
        val driver = Driver(this, key, scroll)
        return Drive { driver.contentReady() }
    }
}
```

`drive` is called once in `onCreate`, where the reader used to construct the benchmark's `Driver` (the host has
begun to boot, the view is not made yet), with the call that scrolls the feed by a distance in pixels. The `Drive` it
returns is told when the first content has been drawn. The default returns null.

Without a driver the activity no longer does what `Driver` did: show over the keyguard, turn the screen on, keep the
screen on, ask for a 120 Hz display mode, and call `reportFullyDrawn()`. The shell's manifest asks for none of them
either, so a benchmark that runs on a locked phone declares them in its own.

An app's files (the copied assets, what the app stores, the fonts it writes) are in the app's own files directory,
which Android keeps encrypted until the phone is first unlocked. The benchmark kept them in device-protected storage
so that it could run before that; a benchmark that still must run on a phone nobody unlocked needs a reader that
does so, which this one no longer is.

## Still the benchmark's

The cut was the smallest one. Left in the reader:

- its 57 launch extras, and the `mi_*` and `x_*` families: compare switches (`owner=0`, `async=0`, `retain=0`, …)
  and `scenario`, `freeze`, `start`, `kinds` and `live`, with the `BENCH_*` variables they set (`Native.start` takes
  `live`);
- `startPitchDp`, the tag view (`tag=1`) and the coverage counters (`cover=1`);
- one scroller, "the feed", which a driver's `scroll` moves;
- comments that speak of the benchmark's apps and its measurements.

## Not done

- **Most of LLP 1115.001.** A finger does not scroll, text arrives one key at a time, the Rust calls nothing on the
  platform, and there is no accessibility. What is built of it:
  - *Edge to edge* (D3.3, D3.4, D3.6). The reader tells exact2 the window's size, the display's density and the
    system's raw insets (bars, cutout, keyboard, gestures; `Native.window`) and pads nothing. exact2 says where the
    page is, what colour is behind it and whether its scheme is dark (`Native.chrome`); the reader puts the view
    there, makes that colour the window's background and sets the bars' icons by the scheme. The software keyboard
    ends the page at its top, and the field being edited stays in view. The keyboard's own slide is not followed
    frame by frame (`WindowInsetsAnimation`), and a `status-bar-style` (LLP 1105) is not read.
  - *The device's settings* (D3.1, `Device.kt`, `Native.configure`): night mode, the font scale (the root font size
    is 16 times it; Material's type ramp is a later stage), the locale, "remove animations", high contrast, and that
    the screen is a finger's. The layout direction, the 12 or 24-hour clock and bold text cross too and change
    nothing yet: exact2 has no door for them.
  - *Back, in order* (D6, `Native.back`): from the navigation root's selected route, the Back control the root names,
    else the root's `navigate`, else the runner's own `back`; with nowhere to go back to, the activity finishes. The
    back callback is always enabled, so there is no predictive preview, and no top layer closes first (there is
    none yet).
  - *A finger is a finger* (D7.4, D7.6). Each touch says what touched (`Native.touch`'s tool type), so a finger is a
    `touch` pointer to the app, hovers nothing, and leaves no focus ring on the button it presses (a text field keeps
    its own focus ring). The system's touch slop, told with the settings, is how far a touch moves before it is a
    drag. One pointer still: a second finger cancels the touch.
  - *The lifecycle's first two parts* (D15): the manifest declares the configuration changes the host takes in
    place, an activity made again adopts the host that runs (`Native.start` on a thread that has one), and the host
    does no frame work while the activity is stopped. Android discards a stopped window's display lists, the reader's
    kept rows among them, so a start after a stop makes the reader again (`ExactActivity.read`) and the host paints
    everything for it; hosted views are made again with it. Nothing is saved for a process the system kills (D15.3).
- **No agent carrier** for the APK (`--reader`, LLP 1107 §4.4).
- **No check builds any of it.** Gradle is in none of the five checks or the async lane, and nothing builds
  `exact-linux` for the Android target.
- **The shell is a development build**: signed with the debug key, with no icon. Its one activity turns keys on for
  every app, and gives none a native module (LLP 1024) or a video player, which the benchmark's apps passed from an
  activity of their own.
- **An app's GPU module** (LLP 1009) is not built or packaged, as the benchmark's `build.sh` did for its apps.
  Caltrain's sky was not tried.
- **No update store** and no replaced Rust module: the Canvas host has neither, though an app's compatibility
  record is computed as its Linux crate's is, and Caltrain's names both (`store.L` is `A`, `rustMode` is `native`).
- **Only an app whose data is Rust** has been built. `exact new` writes no `android/` crate.
- **The benchmark still builds its own copy** of the reader, which no longer matches this repository's JNI names.
