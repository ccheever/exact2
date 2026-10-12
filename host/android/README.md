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
| `canvas/` | the reader, an Android library, package `dev.exact.canvas`: `ExactActivity`, `ExactView`, `Native`, `Player` (the replay), `Owner` (exact2 on a thread of its own), `Layers`, `Natives` (hosted views), `Fields` (the app's text fields, each an `EditText`), `Keys`, `Scrolling`, `Device` and `Palette` (what the device says), `Requests` (what exact2 asks of it) |
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
| `Native.under(x, y)` (`CanvasHost::under`, `Presenter::reader_under`) | reader asks, at a touch's down | Which scrollers are under the point, innermost first, then the page itself where the document is larger than the viewport (id 0). For each: its id, the axes it takes a drag from here on, its revision, its offset, its farthest offset and the box it shows in. Also the axes exact2's own gestures claim there: a swipe row sideways, a reorder grip or a height handle up and down, a transform drag both, a `pan` whatever its `touch-action` does not give to scrolling. And, from D8 on, the hosted text field the touch is for (below) |
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
  stages. A hosted `EditText` takes part as a child of a ScrollView does (below).
- A mouse's wheel and a trackpad do not scroll (`onGenericMotionEvent`), and nothing scrolls for TalkBack.
- A pressable under the finger shows its press at the down, not after the platform's tap delay, so a row may flash
  as a scroll starts.

## The device's colours (LLP 1115.001 D4)

An app's colour roles (`Canvas`, `-exact-secondary-label`, `AccentColor` and the rest, LLP 1095) are the device's
Material 3 colours, light and dark, from its dynamic palette. Nothing is tabled: the reader resolves them on the
device and tells exact2, which reports them through the door the Apple host reports its own through
(`roles::set_reported`).

**What the reader resolves** (`Palette.kt`): twelve Material roles under each scheme, with the platform's own
resources and no library. From Android 14 the platform has each as a colour of its own
(`android.R.color.system_primary_light`, `system_surface_container_high_dark`, …) and nothing is derived. Android 12
and 13 have only the tonal palettes, so each role is the tone Material's scheme gives it:

| Role | Palette | Tone, light | Tone, dark |
|---|---|---|---|
| `primary` | `system_accent1` | 40 | 80 |
| `onPrimary` | `system_accent1` | 100 | 20 |
| `secondaryContainer` | `system_accent2` | 90 | 30 |
| `surface` | `system_neutral1` | 98 | 6 |
| `onSurface` | `system_neutral1` | 10 | 90 |
| `onSurfaceVariant` | `system_neutral2` | 30 | 80 |
| `surfaceContainerLow` | `system_neutral1` | 96 | 10 |
| `surfaceContainer` | `system_neutral1` | 94 | 12 |
| `surfaceContainerHigh` | `system_neutral1` | 92 | 17 |
| `surfaceContainerHighest` | `system_neutral1` | 90 | 22 |
| `outlineVariant` | `system_neutral2` | 80 | 30 |
| `error` | none: Material's baseline error | `#B3261E` | `#F2B8B5` |

A palette has a swatch for tones 100, 99, 95, 90, 80 … 10 and 0 (`system_neutral1_100` is tone 90). A tone between
them (98, 96, 94, 92, 6, 12, 17, 22) is the palette's colour at that lightness, which the platform computes
(`ColorStateList.withLStar`).

**The call.** `Native.colors(IntArray)`: the twelve roles light and then the same twelve dark, ARGB, in the order of
the table (`presenter::Material`). Told before the host boots, so its first frame is in them, and again whenever what
the reader reads is not what it last told: each time a reader is made (the activity made, or started after a stop) and
on every configuration change. A new wallpaper changes the palette; the activity does not declare that change
(`assetsPaths`), so Android makes it again and its reader, adopting the host that runs, tells the new palette. Both
schemes are always told: night mode chooses between them in exact2.

**Which of Exact's roles is which** is exact2's to say, in `host/linux/src/presenter/palette.rs`, which has D4's
table and the four rows it adds. A report paints the page again; a role D4 leaves out (the system hues but red and
blue) keeps its table pair.

**One accent.** A control, a focus ring and a filled button paint in the node's `accent-color` where it sets one, else
in `AccentColor` as reported, with `AccentColorText` drawn on it (a check, a switch's thumb, a filled button's title):
the device's `primary` and `onPrimary`. The painter's own blue is desktop Linux's alone, where nothing is reported.
The controls' shapes are still the desktop painter's (D9).

**The launch window.** From Android 14 the launch theme names the platform's `system_surface_light` and
`system_surface_dark` (`res/values-v34`, `values-night-v34`), the colour the first frame's unsaid background has, with
the bars' icons set for it. On Android 12 and 13 the platform has no `surface` to name in a theme, and the launch
window is the device theme's own day or night background (`?android:attr/colorBackground`).

## The channel out (LLP 1115.001 D5)

Every other call here goes from the reader into exact2. What an app asks of the platform goes the other way, and all
of it takes one channel: exact2's presenter **queues a request and wakes the reader**, the reader **takes the queue
and acts on the main thread**, and a request that has an answer is **answered or cancelled later by its id**. No
capability has a call of its own.

- **The wake** is the one the reader already waits on: the first descriptor of `Native.fds()` (the executor's, which
  a data source's replies also wake). A request does not wait for a paint: a haptic changes no pixel, and a frame may
  carry no stream. Whoever holds the host (`Owner` on exact2's thread, else `ExactView` on the main thread) calls
  `Native.requests()` after every turn and whenever that descriptor wakes it, and hands the words to the main thread
  (`Owner.asked`, `ExactView.asked`), where `Requests.perform` acts on them.
- **The words** (`Native.requests(): IntArray?`, null when nothing is asked). For each request: its kind, its id, the
  count of words that follow, then those words. A string is its count of bytes and then its UTF-8 padded to whole
  words, low byte first, as the op stream writes one. A reader skips a kind it does not know by its count. Ids start
  at 1 and are never reused.
- **An answer** is `Native.answer(id, String?)` on exact2's thread (`Exact.answer`): the answer's text, or null when
  the request was cancelled. An id that waits for no answer changes nothing. When a reader is made again, exact2
  cancels what the last one was asked and never answered, and tells the new one the state again (kinds 5 and 6).

| Kind | Request | Words after the count | What the reader does | Answer |
|---|---|---|---|---|
| 1 | open a link that leaves the app | a string: an absolute `http`, `https`, `mailto` or `tel` URL (exact2 lets no other scheme through) | `ACTION_VIEW` for the web, `ACTION_SENDTO` for `mailto:`, `ACTION_DIAL` for `tel:`. No Custom Tab, which is a library's | none |
| 2 | a haptic (`haptic()`, a pressable's `-exact-press-haptic`) | one word: 1 `selection`, 2 `impact-light`, 3 `impact-medium` (and `impact`), 4 `impact-heavy`, 5 `impact-soft`, 6 `impact-rigid`, 7 `success`, 8 `warning`, 9 `error` | `View.performHapticFeedback` | none |
| 3 | `copyText` | a string | the clipboard (`ClipboardManager`) | none |
| 4 | `share` | one word saying which of the three the app gave (bit 0 the title, 1 the text, 2 the URL), then three strings: title, text, URL | the Sharesheet (`ACTION_SEND` in a chooser) | `shared`, `dismissed`, or why it could not be shown; exact2 journals `share: shared`, `share: dismissed` or `share: refused: …` |
| 5 | whether Back has somewhere to go in the app (D6) | one word: 1 or 0 | registers its `OnBackInvokedCallback` while it has, and unregisters it while it has not, so Back at the first screen is the system's own and leaves with its animation | none |
| 6 | the software keyboard | two words: 0 hidden, 1 shown for an `input`, 3 for a `textarea`; then the hosted field it is for (D8), or 0 for a field exact2 paints | for a hosted field, that `EditText` takes the focus and shows its own keyboard (`Fields.focus`); else `Keys.show` | none |

Kinds 5 and 6 are statements of the app's state, told when it changes and anew to a reader made again; exact2 reads
them as the queue is taken (`Presenter::tell_shell`), Back only after a commit, and neither during a scroll's own
frames.

**Haptics.** Android names occasions, not strengths. Under its constants are a texture tick, a tick, a click, a
heavy click and a double click, and Exact's names choose by those: `selection` is `SEGMENT_TICK` (Android 14;
`CLOCK_TICK` below), `impact-light` `CONTEXT_CLICK`, `impact-medium` `VIRTUAL_KEY`, `impact-heavy` `LONG_PRESS`,
`impact-soft` `TEXT_HANDLE_MOVE`, `success` `CONFIRM`, `error` `REJECT`. Android has nothing for two of them:
`impact-rigid` is the click `impact-medium` is, and `warning` is `REJECT`, as `error` is. The system plays a haptic or
not by the person's touch feedback setting.

**The Sharesheet's answer** is as much as Android tells. The sheet says what was chosen to a sender it is handed
(a target; from Android 15 also its own actions, such as Copy): that is `shared`, though nothing says the share was
finished in the app chosen. A dismissal is not told: when the activity is in front again and the sheet has said
nothing for half a second, the answer is `dismissed`.

**Adding a kind.** In exact2: a variant of `presenter::Request` with the next number, its words in `encode`, and
where it is asked (`Presenter::request`, which wakes the reader); with an answer, a `Waiting` variant and its arm in
`answer_request`. Here: its number and its arm in `Requests.perform`, and `Exact.answer` when it has one. Write the
row above. What layout needs while it runs (a field's metrics, D8) is not a request: a queue taken after the frame is
too late for it.

**Not on it yet:** notifications and the pickers (`showNotification`, `showPicker`, `saveFile`, the document
pickers) are still refused, and the journal then says the channel does not carry them yet. Accessibility events and
the top layer's windows (D10, D13) will take it too.

`x_log_requests=1` at launch (exact2's `EXACT_LOG_REQUESTS`) logs each request as exact2 queues it and as the reader
acts on it, with what `performHapticFeedback` returned: a haptic and a clipboard write show nowhere else.

## Text fields (LLP 1115.001 D8)

Each `input` of a text type and each `textarea` is the framework's own `android.widget.EditText` (`Fields.kt`), with
no background of its own, over the container exact2 paints for it. No library: the one subclass (`FieldText`) only
says when its selection moves, which no listener of the class tells. The IME and its composition, autofill's hints,
the selection handles, the magnifier, the floating toolbar and the spell checker come with the class. A field is an
`EditText` whether or not it is focused, so nothing shifts when it takes the focus. exact2's side is
`host/linux/src/fields.rs`, `presenter/hosted.rs` and `canvas/field.rs`.

**Desktop Linux hosts nothing.** A shell that hosts fields attaches when it boots the host: Android's reader does at
`Native.start`, where the JNI finds `Fields.measure` (`canvas::field::attach`). No other host attaches, and with no
shell a field is measured, painted and typed into as it always was. A Markdown editor (`markup="markdown"`) is not
hosted on Android either: it keeps the field exact2 paints, typed into one key at a time through this view's own
input connection (`Keys`, `Native.key`), because an `EditText` shows plain text.

**Measured during layout.** The kernel lays the field out. The text in it is the `EditText`'s to set, so its size
is the `EditText`'s to say: when the kernel measures a text control's text it asks its measurer
(`TextMeasurer::field_text`, the control-measurement seam's new hook; a host that paints its fields answers
nothing), and on Android that calls up into the reader, from the thread layout runs on, and has the answer before
it returns:

    Fields.measure(size px, weight, flags, tracking em, line px, font file, text, width px): Long

An `EditText` of that thread's own, never attached, is set as the field is (no background, no padding, no font
padding, the field's size, weight, slant, tracking, line height and face) and measured; the answer is its width,
height and first baseline in pixels. A single line is as long as its text; a `textarea` wraps at the width
offered. Each distinct question is asked once (a form of twenty fields in one font asks two). It is no request (D5):
that queue is taken after the frame. The container's own insets are still the painter's (its border and padding,
`field_chrome`); the `EditText` has none.

**What exact2 tells** (`Native.fields(): IntArray?`, null while nothing changed; taken with each stream, before the
stream is drawn). A count, then for each live field, in tree order:

- its id; its **generation**, moved on when its node is renewed for another row's item (LLP 1078): the view then
  starts over, with no text, selection or undo of the last item's; flags (bit 0 it was painted and the box that
  follows is where it goes, bit 1 it takes no touch and no focus: disabled, or under a route that covers it); and
  the scroller whose rows it was painted among (0: none);
- its content box and the clip it shows through (x, y, width, height each, as float bits, in the view's pixels; a
  negative clip width: none), where the last paint put them;
- the count of the reader's edits that reached exact2, and the value the app shows as of it (a string as the op
  stream writes one);
- its look, eighteen words: what it is (bit 0 a `textarea`, 1 a password, 2 disabled, 3 read-only, 4 no
  autocorrect, 5 no suggestions, 6 italic, 7 Enter submits, 8 another field follows it); its keyboard (0 text, 1
  email, 2 URL, 3 telephone, 4 digits, 5 a decimal number, 6 search, 7 none), from `inputmode`, else `type`; its
  capitalization (0 none, 1 sentences, 2 words, 3 characters); its enter key (0 unsaid, 1 enter, 2 done, 3 go, 4
  next, 5 previous, 6 search, 7 send); its `maxlength` (-1 none); its alignment; the colours of its text, its
  placeholder, its caret, its handles and its selection (ARGB, as the roles resolve in its scheme: `FieldText`,
  `-exact-placeholder`, the accent for the caret, the handles and, thinned, the selection, with an authored `color`,
  `caret-color` or `accent-color` winning); its font's size in pixels, weight, tracking and line height (the
  computed font, whose own starting size is 16 by the device's font scale); and the revision, start and end of
  the selection a command last set (`setSelectionRange`, `select()`);
- three strings: its placeholder, its `autocomplete` and its font's file.

A field a statement leaves out is gone, and its view with it. A field that was not painted (its row is out of the
page's view) keeps its view, its focus and its keyboard, out of sight.

**The keyboard's shape.** `type`, `inputmode`, `autocapitalize`, `autocorrect` and `spellcheck` are the `EditText`'s
input type: an email, a URL, a telephone, a number pad with or without a decimal point, a password (masked; of
digits with `inputmode="numeric"`), capitalization as said and none unsaid (the platform's), autocorrect on for
prose unless `autocorrect="off"`, and no suggestions with `spellcheck="false"` (Android has one switch for the spell
checker and the suggestions). `enterkeyhint` is the IME action. Unsaid, a search field's is Search, a field with a
`submit` handler's Go, and any other's the platform's own default for a field: Next where another field follows it,
Done where none does. `autocomplete`'s last token is autofill's hint (`email`, `username`, `current-password`,
`new-password`, `one-time-code`, `name`, `tel`, `postal-code` and the address and card tokens); `off` takes the
field out of autofill. `maxlength` is an `InputFilter.LengthFilter`.

**Where a field is, and how it is drawn.** The view is put where its box was painted, for real: its handles, its
magnifier and the keyboard's cursor anchor are placed by the view's own position. It sits in a `Port` the size of
its clip (the platform hides a handle outside the field's ancestors, so it hides one the clip cuts), and it moves
with its scroller's rows as the reader moves them between exact2's paints (`Fields.place`). The view system draws
none of it: as for `Natives`, the field's own drawing is recorded into a `RenderNode` that the stream draws in the
field's place (op 28, kind 4: `NATIVE view 4 rect radii ""`, in the content box, after the container's fill and
border and under the clips around it). So a field stacks in tree order with what exact2 paints. A field that
invalidates (its caret blinks, a character is typed) has only its node recorded again; Android frees a node's
drawing when a paint leaves it out of the tree, so a field a stream draws again is recorded again
(`Fields.drawn`). A transform over a field moves its box's corner and does not scale or turn the view (as for
every hosted view).

**Whose a touch is.** The reader's view still takes every touch. A down inside a field's box is held, with what
follows it, until exact2 says what is under the point (`Native.under`'s third word: the hosted field the node hit
is, by exact2's own hit test). If it is the field, the touch is the field's from its down on, in the field's own
coordinates, and exact2 hears none of it: the caret lands under the finger, and a long press, the handles, the
magnifier and the toolbar are the platform's. Otherwise (the field is under a bar or a sheet) it goes to exact2 as
any touch does. A drag that begins on a field still scrolls the list around it: past the slop along an axis a
scroller takes, the reader cancels the touch in the field and scrolls (`Scrolling`), unless the field asked its
parents to leave it its touch (`requestDisallowInterceptTouchEvent`: a selection or a caret being dragged), as a
ScrollView would.

**What the reader tells back**, on exact2's thread and in order:

| Call | When | What exact2 does |
|---|---|---|
| `Native.fieldFocused(id, generation, focused)` | the field took the focus (a finger tapped it), or lost it with no other field taking it | its focus follows, with the `blur`, `change` and `focus` that makes |
| `Native.fieldEdited(id, generation, seq, value, start, end)` | the person's edit number `seq` left the field holding `value`, the selection at `start` to `end` (UTF-16 units) | the field's text is that; `input` fires, with the selection |
| `Native.fieldSelected(id, generation, start, end)` | the selection moved with no edit | kept for the next `input`; `select` fires for a range |
| `Native.fieldAction(id, generation, action)` | the action key, or Enter or Tab on a hardware keyboard: 0 Enter with no action, 2 done, 3 go, 4 next, 5 previous, 6 search, 7 send, 16 Tab, 17 Shift+Tab | Enter's `key` handlers, then `change` and `submit` for a single line; then `next` and `previous` move to the field after or before (the focus leaves where there is none), and `done`, `go`, `search` and `send` leave the field, which puts the keyboard away. Tab is the page's own Tab |

What a call says of a generation exact2 has moved on from is dropped.

**Who owns the text.** The field does, while the person edits it. Each edit is counted; a statement says the count
exact2 has reached beside the value the app then shows. The reader takes a told value only when it answers the
field's newest edit and differs from what the field holds. So a commit that sets the value the field already has
changes nothing in it (its composition and its selection stand), and one that sets another replaces the text where
it stands (the field keeps its undo), puts the caret where it was among the text after it, and restarts the
keyboard, which ends a composition. **Composing text is reported as it composes**: `input` fires for every change
of the field's text, the composing region included, as the web reports it and as an Android `TextWatcher` hears it.
Android's keyboards compose ordinary words, so reporting on commit (AppKit's way) would hold each word back until
its space.

**The focus.** exact2 owns it. A focus the app moves (`autofocus`, `focus()`, the action key's Next) reaches the
field through the keyboard request (kind 6, with the field's id): the `EditText` takes the focus and the keyboard
shows. A focus that leaves the fields (a press on a button, `blur()`, Done) hides the keyboard and gives the focus
back to the reader's view, so keys go on to exact2 (`Native.key`). Back closes the keyboard first, which the system
does itself for an `EditText`: the IME takes Back before the reader's callback. A hardware keyboard types into the
`EditText`; its Enter is the action key and its Tab the page's.

`x_log_fields=1` at launch (exact2's `EXACT_LOG_FIELDS`) logs each edit told to exact2 with its composing region,
and what the keyboard asks of the field's input connection (`setComposingText`, `commitText`, …).

Not built of D8:

- **A field's views are all made as its page appears**, on the main thread: about 0.7 to 1 ms a field on a Pixel
  10 Pro XL (the `EditText`'s constructor is a third of it). A form of a hundred fields would stall its first
  frame; making a view only once its field is near its port would bound it.
- **A swipe row, a reorder grip or a `pan` under a field's touch does not get that touch**: exact2 never heard its
  down. A drag there scrolls or stays the field's.
- **A field inside a layer the reader animates** (a transition) is drawn moving, and its view is where the last
  paint put it until the next.
- **A font the app declares by its bytes** has no file to name: the field is set in the device's default family.
- **A password's own width** is measured unmasked, and a `textarea`'s min-content width is its max-content.
- **The Material container** (D9), the floating label, a field's place in TalkBack's order (D13: the `EditText`
  reports itself, with its text, its hint and its bounds, and nothing orders it among the painted content yet), a
  mouse's I-beam, and autofill's save UI.
- **`inputmode="none"`** keeps the keyboard down on focus; nothing else of an app's own keys is built.

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

- **Most of LLP 1115.001.** The controls have the desktop painter's shapes, a dialog or a menu is refused, and
  there is no accessibility tree. What is built of it:
  - *A text field is a real `EditText`* (D8): the section above, with what is not built of it.
  - *The device's colours* (D4, colours only) and *the channel out* (D5): the two sections above. The type ramp
    is not reported yet.
  - *A finger scrolls* (D7.1 to D7.3): the section above, with what is not built of it.
  - *An authored swipe yields the system's edges* (D7.5): a swipe row's own gesture does not start from the sides
    the system's Back gesture starts from, by the system-gesture insets `Native.window` tells, in place of the fixed
    20 points the other hosts yield. With a button for Back there is no such edge, and nothing is yielded.
  - *Edge to edge* (D3.3, D3.4, D3.6). The reader tells exact2 the window's size, the display's density and the
    system's raw insets (bars, cutout, keyboard, gestures; `Native.window`) and pads nothing. exact2 says where the
    page is, what colour is behind it and whether its scheme is dark (`Native.chrome`); the reader puts the view
    there, makes that colour the window's background and sets the bars' icons by the scheme. The software keyboard
    ends the page at its top, and the field being edited stays in view (once the page has been painted in the
    viewport the keyboard left: the boxes a reveal goes by are the last paint's). The keyboard's own slide is not followed
    frame by frame (`WindowInsetsAnimation`), and a `status-bar-style` (LLP 1105) is not read.
  - *The device's settings* (D3.1, `Device.kt`, `Native.configure`): night mode, the font scale (the root font size
    is 16 times it; Material's type ramp is a later stage), the locale, "remove animations", high contrast, and that
    the screen is a finger's. The layout direction, the 12 or 24-hour clock and bold text cross too and change
    nothing yet: exact2 has no door for them.
  - *Back, in order* (D6, `Native.back`): from the navigation root's selected route, the Back control the root names,
    else the root's `navigate`, else the runner's own `back`. The reader takes Back only while exact2 says it has
    somewhere to go (a request, above), so at the first screen Back is the system's own and leaves the app with
    its animation. A pop has no predictive preview (routes have no transition yet), and no top layer closes first
    (there is none yet).
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
