# iOS scrolling: what Exact runs per frame, and three changes

**Status:** Built on the `ide/exact2` fork's `main` branch, 2026-10-02:
- `4ffb9488`
- `2be59e59`
- `9a3d70cc`

Each can be taken alone. Measured on an iOS 27 simulator (iPhone 17 Pro).

## 1. The question

A pull-down-and-release on a short form screen (a few paragraphs, two
fields, buttons; no lists, canvases or video) felt like it juddered on the
rubber-band return. The goal was to know two things:
- whether Exact caused it
- what Exact runs on the main thread during a scroll and its bounce-back

Ideally that is almost nothing.

## 2. Method

**Recording.** Instruments (`xctrace`, Time Profiler plus `os_signpost`),
attached to the running app. It records UIKit's own scroll signposts
(`Scroll_Dragging`, `Scroll_Deceleration`), Core Animation's `Commit` and
UIKit's `UpdateSequence` per frame, and Exact's `scrolled` / `pump-*`
intervals.

**Breakdown.** Temporary intervals were added around each piece of the
scroll callback to split its cost. They are not in the commits.

**The bounce.** AXe drove real pull-and-release touches; each run had six or
eight.

**Skipped frames.** A frame with an `UpdateSequence` but no offset change
and no commit is one the user sees as a stall then a double step. Screen
recordings confirmed the same frames.

## 3. Finding 1: the skipped frame is UIKit's

About 125 ms after release, in nearly every bounce, one frame passes with no
offset change, and the next catches up with a double step (e.g. −47.3 → −40.0
where the steps around it are ~3.5 pt).

| Setup | Bounces with the skip |
|---|---|
| The app | 7/7 |
| The app, Exact's scroll callback a no-op | 7/7 |
| The app, Exact's scroll display link disabled | 7/7 |
| A minimal native app, plain `UIScrollView`, content shorter than the screen | 7/7 |
| The same, content taller than the screen | 7/7 |

Exact neither causes nor can fix it on the simulator. Settings looked clean
only because its large-title animation commits every frame, which hides the
skip from this measure. Whether a 120 Hz device shows it is not measured
here.

## 4. Finding 2: what Exact ran per scroll frame

Mean main-thread time inside `NodeView.scrollViewDidScroll`, per frame, on
the form screen (about 240 frames per run):

| Part | Before | After all three |
|---|---|---|
| Whole callback | **58 µs** | **15 µs** |
| `scrollPump.scrolled` | 45 µs | 6 µs |
| of which: queueing a pass and making a display link | **36 µs** | 0 |
| `leaves.scrolled` (heavy-leaf distance) | 3.6 µs | 2.4 µs |
| `repaintThrough` (canvas above) | 3.0 µs | 2.1 µs |
| `paintVisibleText` | 2.4 µs | 1.7 µs |
| `collections.changed` | 2.4 µs | 1.5 µs |
| `transformGeometry.changed` | 1.1 µs | 0.8 µs |
| `videoVisibility.changed` | 0.4 µs | 0.4 µs |
| Follow-up pump passes queued | 241 | 0 |
| Display links made | one per frame | 0 |

The "after" column includes the temporary intervals' own overhead. Per
frame, UIKit spent about 0.5 ms in `UpdateSequence` and about 0.2 ms
committing; the main thread was otherwise idle.

So Exact was cheap in absolute terms, but almost two-thirds of it was work
for nothing:
- On *every* scroll frame, `ScrollPump.scrolled` set `textPending = true`.
- It queued a main-queue pass and created a new `CADisplayLink` at the
  panel's maximum rate.
- The pass then found no text and no rows owed, and invalidated the link.

On a 120 Hz device, that also means a frame-rate vote added and withdrawn
every frame.

## 5. The changes

### 5.1 The pump asks for frames only when work is owed (`4ffb9488`)

`paintVisibleText()` already scans the unsettled paragraphs. It now returns
whether any paragraph still lacks pixels. `ScrollPump.scrolled` sets
`textPending` and schedules only then, or when a collection owes rows
(`fillPending`). A screen whose text is painted and whose lists owe nothing
scrolls with no pass and no frame request. A long list, or text still
rastering, works as before.

**Tests:** `ScrollPumpIOSTests`. A scroll over painted text asks for nothing;
over text without its pixels, it asks for a pass.

### 5.2 Press feedback eases on the render server (`2be59e59`)

`press-scale` eased frame by frame from `PressClock`'s display link,
rewriting `UIView.transform` on the main thread for 120 ms per press and
release. On iOS it is now Core Animation's:
- The model transform takes the press's target at once.
- An **additive** `transform` animation eases the difference from the factor
  on screen to nothing, with the same curve (`cubic-bezier(.16, 1, .3, 1)`)
  and duration.
- Additive animations compose under the model, so an engine write mid-ease
  keeps the press.

That composition was LLP 1061's reason to reject a Core Animation version,
which applied to a non-additive one; LLP 1061 is updated to say so. A re-aim
replaces the ease from what shows. The held release (a quick tap in a scroll
view) and the return to idle are each one timed callback. macOS keeps
`PressClock`.

**Tests:** `PressFeedbackIOSTests`. A new test shows Core Animation composes
the ease before the model, under a rotation, a translation and a non-centre
`transform-origin`, at a mid-ease instant; the existing held-release and
engine-write tests pass unchanged.

### 5.3 One display link for the app's life (`9a3d70cc`)

Four iOS users made their own `CADisplayLink` on demand and invalidated it
when done:
- the session's motion, timers and canvases (`Frames`)
- the scroll pump
- heavy leaves
- live SVG filters

`FrameClock` is now the one link:
- It is made at first use and paused while no one wants frames, so an idle
  app still gets no callbacks (LLP 1009 D4).
- A user asks with the frame-rate range it needs; the clock asks for the
  highest range among current users.
- Users tick in a fixed order (session, a list's smooth correction, a
  navigation transition's reveal, scroll, heavy leaves, SVG filters) instead
  of the order their links happened to fire. The correction and the reveal
  also moved onto the clock (`9810e695`), so it has six users.
- A user that moves something on screen asks for the display's full rate
  (`FrameClock.full(on:)`). Their own links had asked for nothing, which a
  ProMotion panel ran at its full rate; on one shared link, `.default` is a
  timer's and never raises the rate, so a paint-only cap of 60 would
  otherwise have slowed a correction or a reveal under a breathing fade
  (review, 2026-10-05).
- The pump keeps its request until the scroll has been still for two frames,
  so a long scroll is one request, not one per frame.
- macOS keeps per-view links, since a link there belongs to a screen.

**Tests:** `FrameClockIOSTests`:
- one link made, then paused and not torn down
- the rate is the highest asked
- the order holds, and a user dropped mid-tick does not tick
- a gone owner stops
- owed scroll work takes the app's link without making another

## 6. Verification

- **UIKit XCTests:** 132 before the merge with main; 153 after (one
  skipped), all passing.
- **macOS XCTests:** 520 (one skipped), all passing.
- **Rust:** build, clippy, fmt and caps clean.
- **On the simulator:**
  - the form screen's traces above
  - a press filmed on a `press-scale: 0.9` button (eases in, holds, eases
    out)
  - a full-screen overlay animation still drawing every frame through the
    shared link

## 7. What remains, and suggestions

- **The remaining per-frame checks** (heavy leaves, canvas repaint, visible
  text, collections, transform geometry, video) are each a few
  microseconds of "is there anything to do?". They could become
  coverage-driven, as `UITableView` is: compare the visible rect with what
  is already built and painted, and do nothing until it crosses that
  margin. Not needed for the numbers above, but it would make "nothing
  runs" literal.
- **A 120 Hz device.** The display-link churn mattered most there and could
  not be measured here; a device trace of the same bounce would confirm it.
- **macOS** has the same per-scroll pattern in `PresenterMac`'s pump link
  and `PressClock`; the same two fixes apply.
- **Measuring.** The `scrolled` signpost interval already exists; per-part
  intervals behind a flag (or in a profiling build) would make this
  breakdown a standing metric in `metrics.mjs`.

## 8. A fast fling's first pass (2026-10-06)

Charlie's report: a fast fling through rows not yet built steps. Measured
on the iPhone 17 Pro simulator, Signal Clone, plain launch, with temporary
in-app hooks: the chat opens at 4 s and 6000 pt/s flings set the offset each
frame (no touches, no accessibility client). The sampler and per-batch
timing were logged per frame. The simulator ran at 60 Hz.

- **Where the time went.** Pass 1 (rows never built) applied 518 batches in
  4 s, up to 10 in one frame, about 0.16 ms of main thread per node created
  (a message row is about 9 nodes). A Time Profiler run of the scroll path
  put `NavigationHost.sync` at 27% of `Presenter.apply`, `ControlHost.sync`
  11%, `MenuHost.sync` 6%: per-batch passes that re-project the routes and
  bars, run for every fill batch whatever it holds.
- **What changed.**
  - A batch that only builds, moves or drops a list's rows
    (`Presenter.onlyListRows`, `ListRowsIOS.swift`) skips the route
    projection. One that touches anything outside a list, or arrives while
    a sync is owed (a transition, no window yet, a stack UIKit moved itself
    since the last sync: `NavigationHost.syncOwed`), still syncs.
  - `MenuHost.sync` and `ControlHost.sync` no longer set `isHidden` to the
    value it already has: UIKit's setter walks the view's subtree even then.
- **Before and after**, three runs each:

| | pass-1 apply, sum | apply, worst frame | busy p95 | busy, worst frame | pass-2 apply, sum |
|---|---|---|---|---|---|
| before | 355–404 ms | 11.2–13.4 ms | 10.8–12.7 ms | 16.8–27.3 ms | 384–416 ms |
| after | 280–287 ms | 7.5–8.3 ms | 8.9–9.5 ms | 13.2–14.4 ms | 306–311 ms |

  The simulator missed no frames either way; a phone two or three times
  slower misses them in the worst frames before. Tests:
  `ListRowsIOSTests`; the navigation suites, which caught a pop UIKit
  finishes on its own (`nativeMoved`).
- **Not done.** A per-frame budget for the rows built ahead of a fast fling
  (they land in bursts of up to 26 nodes in a frame), and a cheaper row
  build. A device trace (Signal Clone build 34 carries the sampler's
  overruns) should come first.
