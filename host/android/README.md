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
begun to boot, the view is not made yet), with the call that scrolls the feed by a distance in pixels
(`ExactView.scrollBy`, `Native.scroll`: the scroller under the viewport's centre, or the one a finger last moved).
The `Drive` it returns is told when the first content has been drawn. The default returns null.

Without a driver the activity no longer does what `Driver` did: show over the keyguard, turn the screen on, keep the
screen on, ask for a 120 Hz display mode, and call `reportFullyDrawn()`. The shell's manifest asks for none of them
either, so a benchmark that runs on a locked phone declares them in its own.

An app's files (the copied assets, what the app stores, the fonts it writes) are in the app's own files directory,
which Android keeps encrypted until the phone is first unlocked. The benchmark kept them in device-protected storage
so that it could run before that; a benchmark that still must run on a phone nobody unlocked needs a reader that
does so, which this one no longer is.

## Scrolling: who owns what (LLP 1115.001 D7.1 to D7.3)

A finger scrolls. The gesture and its physics are the reader's (`Scrolling.kt`), made of the platform's own classes
and nothing imitated: the system's touch slop and fling velocities (`ViewConfiguration`), `VelocityTracker`,
`OverScroller` for the fling, the stretch `EdgeEffect` at the ends, and for the indicator the scrollbar a `View` draws
and fades by itself. The reader has no dependency for it. exact2 recognizes no scroll: its side is
`host/linux/src/presenter/scrollers.rs` and `canvas/scroll.rs`.

**The reader moves the scroller, at once, and exact2 catches up.** A scroller whose rows the stream keeps in a node of
their own (a group: ops 20 and 21) is translated on the UI thread in the frame the finger moved. exact2 is told how
far, mounts and retires rows behind it, and its next stream draws the rows where it then has them; the reader places
the node by the difference. This is what a driver's `scroll` always did, now for the scroller under the finger.

Three calls cross, all in the view's pixels:

| Call | Direction | What it says |
|---|---|---|
| `Native.under(x, y)` (`CanvasHost::under`, `Presenter::reader_under`) | reader asks, at a touch's down | Which scrollers are under the point, innermost first, then the page itself where the document is larger than the viewport (id 0). For each: its id, the axes it takes a drag from here on, its revision, its offset, its farthest offset and the box it shows in. Also the axes exact2's own gestures claim there: a swipe row sideways, a reorder grip or a height handle up and down, a transform drag both, a `pan` whatever its `touch-action` does not give to scrolling |
| `Native.scrolled(id, dx, dy, rev)` (`CanvasHost::scrolled`, `Presenter::reader_scrolled`) | reader tells | It moved scroller `id` by that much, knowing revision `rev` of it. exact2 clamps the move to the scroller's travel |
| `Native.scrollers()` (`CanvasHost::scrollers`, `Presenter::reader_scrollers`) | exact2 tells, with each stream | For every scroller it has told the reader of: its id, its axes, its revision, where it is, the offset its rows were last painted at, and its farthest offset. Null when nothing of it changed; a scroller it leaves out is gone, and the reader forgets it |

**Who is the authority.** While a drag or a fling runs, the reader: exact2 takes each move as told. A scroll the app
makes goes the other way (a `scrollTop` or `scrollLeft` write, `scrollIntoView` of an element, `scrollBy`, a field kept
above the keyboard, a list a reorder drags along): exact2 sees the offset is not where the reader's moves left it and
moves the scroller's **revision** on. The reader then takes the offset told and ends the fling that was moving that
scroller; a move it had sent under the old revision is dropped in exact2. A virtualized list's own correction of its
offset (rows that turned out taller than their estimate) is not such a scroll: what shows stays where it is, the
revision stands and a fling goes on.

**Whose a touch is.** A touch's down, and its moves until it is known to be a scroll, go to exact2 as they always did:
it may be a tap, a press or a gesture of exact2's own. Once the finger has moved past the system's slop:

- along an axis exact2 claims there, the touch is exact2's until it lifts, by the rules it has (swipe rows, reorder,
  `pan`, a transform drag);
- else, along an axis some scroller under it scrolls on, the scroll wins. The reader cancels the touch in exact2
  (`Native.touch` with a cancel): its press, its press feedback and whatever drag it might have begun end there, and no
  press fires. No more of that touch reaches exact2;
- else it stays exact2's.

A finger that lands on a list while it flings stops it and has it, as on the platform's lists, and exact2 hears
nothing of that touch.

**Nested hand-off.** The drag belongs to every scroller under the finger that scrolls on its axis, innermost first.
Each move goes to the innermost; what it cannot take, because it is at its end, goes to the next one out, and so on
to the page. A fling hands off the same way, frame by frame. An `overscroll-behavior` other than `auto` ends the
chain at its scroller. What no scroller can take stretches the edge of the innermost one that has a node.

**The ends and the indicator.** The stretch is `EdgeEffect`'s own: it stretches the `RenderNode` it is drawn into, so
while an edge is stretched the scroller's rows are drawn through a node the size of its box (`Scrolling.record`).
The indicator is a `View` over the scroller's box that draws nothing but the scrollbar `View` gives a view that
scrolls, with the theme's thumb, shown as the scroller moves and faded by `View`.

**The feed** is no longer one scroller chosen once at the viewport's centre. It is the scroller a finger last moved
up or down, or the one a driver's `scroll` found under the centre (found again when its node is gone); it is what
exact2 times its collection passes for and what the reader's row heuristics follow (`Native.feed`).

Not built of D7:

- **A scroller with no node of its own does not move at once** (D7.2). The recorder gives a node only to a scroller
  that scrolls up and down, has rows, and is not itself inside another scroller's row. A list that scrolls sideways,
  a scroller nested in a list's row, and the page itself are drawn inside their surroundings, so they move when
  exact2's paint arrives; the frame waits up to 2 ms for it, as it does for a drag's moves. They have no stretch at
  their ends either. Giving each a node needs the recorder to open a group inside a row's recording.
- `scroll-snap-type` and `scrollbar-width` are not read by this host (a swipe row written as a snapping scroller
  rests wherever it is let go). `keyboardDismissMode`, pull-to-refresh and a sheet's part in the hand-off are later
  stages, as is a hosted `EditText`'s.
- A mouse's wheel and a trackpad do not scroll (`onGenericMotionEvent`), and nothing scrolls for TalkBack.
- A pressable under the finger shows its press at the down, not after the platform's tap delay, so a row may flash
  as a scroll starts.

## Still the benchmark's

The cut was the smallest one. Left in the reader:

- its 57 launch extras, and the `mi_*` and `x_*` families: compare switches (`owner=0`, `async=0`, `retain=0`, …)
  and `scenario`, `freeze`, `start`, `kinds` and `live`, with the `BENCH_*` variables they set (`Native.start` takes
  `live`);
- `startPitchDp`, the tag view (`tag=1`) and the coverage counters (`cover=1`);
- the heuristics that follow one scroller, the feed (which rows are recorded on the main thread, which are made
  first, where a row's platform view exists): a second scroller that scrolls has none of them;
- comments that speak of the benchmark's apps and its measurements.

## Not done

- **Most of LLP 1115.001.** Text arrives one key at a time, the Rust calls nothing on the platform, and there is no
  accessibility. What is built of it:
  - *A finger scrolls* (D7.1 to D7.3): the section above, with what is not built of it.
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
    drag. One pointer still: a second finger cancels the touch, unless the touch is a scroll, which the second
    finger then takes.
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
