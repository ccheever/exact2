# LLP 1044: Why the Markdown reader scrolls worse than Legend's — measured

**Type:** Research
**Status:** Draft
**Systems:** Scrolling (LLP 1010 §6 — the measured-row `list`, its window, settle loop and retirement), Apple host (`ChainingScrollView`, `Presenter.syncLists`, the batch bridge, `TextEngine.measure`), Kernel (layout from the root; the per-leaf measurement memo), Runner (`ListWindow`), Markdown reader (LLP 1033 — row granularity, `estimated-item-height`)
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-18
**Revised:** 2026-09-21
**Related:** LLP 1033 (the reader; `apps/markdown/README.md` is the lane's running record), LLP 1010 §6 (bounded list memory, measured rows), LLP 1002 D4 ("scroll always wins": the platform recognizes and scrolls, the engine follows), LLP 1008 §3, §5 (CoreText, the AppKit presenter), LLP 1001 §6 (the `TextMeasurer` seam), LLP 1043 (what a CoreText re-break costs)

## Summary

Charlie asked, 2026-09-18: the Markdown lane has made startup as fast as Legend
Markdown's, *"but scroll performance is significantly worse… get to the bottom of
why"* — the list strategy, the app, the parser, or Taffy. This is that
investigation. It decides nothing, changes no product code, and is not linked into
`llp/current/` (a research record; the working set is full).

The lane's latest matched series (2.24 MB corpus, 120 Hz wheel input, 900×700):
**Exact 21.67 hitch ms/s, Legend 3.33** (medians of three).

**Current answer, 2026-09-21:** that comparison and the findings below describe
the pre-fix reader. The principal diagnosis was borne out and the ordinary macOS
scroll path was substantially repaired the next day (§7). The evidence now
supports a narrower concern: rare construction paths still have measured tail
costs, UIKit does not yet have macOS's rationed presenter, and a repeatable 120 Hz
advantage over Legend has not been established. It does **not** support saying
that ordinary current macOS scrolling is generally slow. Section 8 audits the
claim against current `origin/main` at `6e583ae1` and states what must be rerun.

**The answer is placement, then redundancy — not throughput.** In the one series
where both were profiled, Exact used *less* CPU than Legend while scrolling. But
every piece of Exact's list work runs on the
main thread, synchronously, inside the display-link callback that also has to move
the content — while Legend's equivalent work runs on a JavaScript thread and a
background queue, under an AppKit scroll view that keeps scrolling without the main
thread. On top of that, each of our window changes does several times the work it
needs to. The parser is not involved at all, and Taffy is involved only through how
we call it.

| | |
|---|---|
| F1 Where the gap is | In the per-frame **app update**: p50 4.1 ms vs 2.0–2.8, p95 8.0–12.0 vs 3.0–5.3, against an 8.33 ms frame. Render and GPU phases are close |
| F2 What a hitch is | 73 of Exact's 77 hitches follow an app update longer than one frame; 3 of Legend's 10 do |
| F3 Who scrolls | Legend: AppKit's **concurrent** scrolling (a `NSScrollingConcurrentVBLMonitor` thread). Exact: `NSScrollingBehaviorSingleThreadedVBL` on main — because `ChainingScrollView` overrides `scrollWheel(with:)` |
| F4 What rides on that frame | Exact: window → row build → kernel → **layout from the root with CoreText callbacks** → JSON → AppKit views → a second pass of all of it → draw. Legend: mount and draw only |
| F5 One window change costs | 3.6 ms mean, 6.9 ms p95 (quiet-ish machine); 7.8 / 14.9 ms (busy). 36% of 30 px steps are window changes |
| F6 Most of the Rust share is redundant | 94,361 text-measure callbacks for 254 new text leaves. Two 4-entry memos face ~6 offers per leaf. **Raising both to 16: 1,463 callbacks, Rust −61%, p95 6.9 → 5.2 ms** (measured here) |
| F7 Too many changes, each doubled | 45% of changes only *retire* rows and still relayout; creating changes always take a second settle pass. Legend recycles on demand and has exact heights inside the commit |
| F8 Not the cause | Parser (0 samples), JSON (2%), strip text painting (−0.1 ms/step when removed), render server, total CPU, view creation |
| F9 Load | Exact's hitch rate tracks machine load (3.3 → 21.7 → 40.0 ms/s); Legend's barely moves (0.8 → 3.3 → 4.2) |

## 1. Evidence, by kind

Every number below says which of these it is.

- **Lane, recorded** — the Markdown lane's Instruments traces and `sample` files
  under `target/markdown-comparison/` (HID wheel input, verified movement; Exact
  `d222c8b0…` "Scalar candidate", Legend `a16b1a8d…` at upstream `2b7b91d9`).
- **Derived here** — per-frame tables exported from those same traces with
  `xctrace export` and summarized (`research-scroll-gap/frames*`), and call-tree
  attribution of their profiles. No new recording: this session has no
  Accessibility, post-event or screen-capture permission, and did not work around
  that. So **every Legend number is from the lane's recordings.**
- **Measured here, in process** — a probe that compiles the lane's ExactKit sources
  (HEAD `ed541b82`, clean) with a `main.swift`, links the Rust archive, opens the
  corpus, and scrolls the list 1,200 × 30 px with `scroll(to:)`, timing each step's
  synchronous cost by phase. Counts are deterministic; timings are medians of three
  alternating runs and are **CPU update costs, not display frame intervals**. Load
  average was 12–28 throughout (other lanes building).
- **Read** — Legend Markdown, the vendored Legend List, `react-native-enriched-markdown`
  and react-native-macos 0.81.7, from source under `target/markdown-comparison/legend`.
  Nothing there was built or run. Paths in §3 are relative to that directory.

## 2. Findings

### F1. The gap is the app update, per frame

*Derived here* from the lane's `scalar-hitches-120hz` series (1,200 × 30 px wheel
events at 120/s for ten seconds, Hitches instrument only), inside each run's
verified scroll interval:

| Run | hitch ms/s | load | app update p50 / p90 / p95 / p99 (ms) | over 8.33 ms | render p50 |
|---|---:|---:|---|---:|---:|
| Exact 1 | 3.33 | 7 | 4.10 / 6.37 / 8.05 / 9.76 | 51 of 1,195 | 2.40 |
| Exact 2 | 40.00 | 30 | 7.65 / 9.13 / 12.04 / 15.97 | 310 of 950 | 2.50 |
| Exact 3 | 21.67 | 22 | 4.14 / 7.82 / 8.79 / 12.07 | 84 of 1,165 | 2.29 |
| Legend 1 | 0.83 | 9 | 2.05 / 2.76 / 3.04 / 4.04 | 3 of 1,200 | 1.39 |
| Legend 2 | 3.33 | 18 | 0.51 / 2.69 / 5.25 / 16.78 | 50 of 1,181 | 1.87 |
| Legend 3 | 4.17 | 23 | 2.76 / 3.50 / 3.76 / 4.79 | 1 of 1,194 | 1.85 |

Exact's median update is about twice Legend's and its p95 sits on the frame
budget. GPU time is the same (p50 ≈ 0.5 ms). Exact's render phase is ~0.5–1 ms
longer per frame, and no hitch in this series is attributed to rendering. Exact 2
recorded only 950 app updates and 1,000 renders in its ten seconds, against about
1,195 in every other run.

Total CPU points the other way. *Lane, recorded* (the earlier 60 Hz series, 231 KB
fixture), CPU milliseconds over the ten-second scroll:

| | Main | Other threads that matter |
|---|---:|---|
| Legend | 565 | JavaScript 187 · `NSScrollingConcurrentVBLMonitor` 28 |
| Exact | 492 | none (≈ 90 ms of housekeeping) |

Legend spends more and hitches less. And in the lane's 120 Hz `sample` of Exact the
main thread is **idle 76% of the time**. This is a tail-latency problem: short
bursts that land inside a frame, not a shortage of CPU.

### F2. Hitches follow long updates

*Derived here.* For each hitch, the longest app update in the preceding 30 ms:
Exact 4 of 4, 43 of 47, 26 of 26 hitches follow an update over 8.33 ms (median
11–13 ms). Legend: 1 of 1, 2 of 4, 0 of 5. Exact's hitches are its own main-thread
bursts. Nearly all are single dropped frames (76 of 77 are 8.33 ms).

### F3. Legend's scroll view scrolls without the main thread; ours cannot

*Lane, recorded; attributed here.* Legend's process has a
`com.apple.NSScrollingConcurrentVBLMonitor` thread, and 352 of its 565 main-thread
milliseconds sit under `_NSScrollingConcurrentMainThreadSynchronizer` — AppKit's
responsive scrolling: a separate thread moves already-rendered layers every
refresh and the main thread *catches up* when it can. *Read:* Fabric's
`RCTEnhancedScrollView` is a stock `NSScrollView` and nothing on the Fabric path
overrides `scrollWheel:` (only the unused Paper class does,
`React/Views/ScrollView/RCTScrollView.m:109-126`), so it stays eligible.

Exact's trace shows `-[NSScrollingBehaviorSingleThreadedVBL _advanceTimeWithDisplayLink:]`
on the main thread driving every frame of the scroll. `ChainingScrollView` overrides
`scrollWheel(with:)` (`NodeViewMac.swift:146`) to implement CSS scroll chaining, and
AppKit withdraws responsive scrolling from a subclass that does. The reader's list
is `overscroll-behavior: contain`, so a trackpad gesture is routed to
`super.scrollWheel` — AppKit animates it, single-threaded.

This is the web's own design, and we are on the wrong side of it: a browser scrolls
on the compositor and lets layout arrive late. LLP 1002 D4 says the platform scrolls
and the engine follows. On macOS today the engine *leads*: the platform's frame
waits for it.

### F4. What sits on that frame

*Lane, recorded; attributed here* — 7 s of `sample` at 1 ms during the 120 Hz scroll,
Exact's main thread: 5,876 samples, 1,416 of them active.

| Inside the display-link callback | samples | |
|---|---:|---|
| `NodeView.clipScrolled` → `Presenter.syncLists` | 701 | the list, synchronously, in the bounds-change notification |
| · `exact_list` (Rust) | 470 | of which **layout 396** |
| · · `TextEngine.measure` callbacks | 353 | real paragraph building 104, CoreText typesetting 63 — **the rest is cache hits** |
| · `ExactSession.apply` → `Presenter.apply` | 332 | views, frames, and whole-tree finalization passes |
| · JSON batch: decode 29, encode ≈ 12 | ≈ 40 | |
| `CA::Transaction::commit` | 594 | text rasterization 356; `CABackingStoreGetFrontTexture` wait 111 |

`syncLists` re-enters itself from `Presenter.apply`'s `defer` (`PresenterMac.swift:365`)
until row heights stop changing, so one scroll tick can run Rust → JSON → AppKit →
Rust → JSON → AppKit before the frame's commit.

Legend, *read*: the scroll event is posted to JavaScript asynchronously and
coalesced (`ReactCommon/react/renderer/core/EventQueue.cpp:33-54`); Legend List,
React, Yoga and text measurement all run on the JS thread inside the Fabric commit
(`.../mounting/ShadowTree.cpp:327`); inline Markdown parsing and attributed-string
building run on a per-view background queue (`ios/EnrichedMarkdown.mm:589-646`);
the main thread applies mount mutations and draws. *Lane, recorded:* TextKit layout
shows up on Legend's **JS thread** (16 ms), and its main thread spends 1 ms in ten
seconds on `addSubview`/`removeFromSuperview`. When Legend's JS falls behind, the
user would see blank rows past a 500 px buffer, not a stalled scroll.

### F5. One window change, by phase

*Measured here, in process* (30 px steps; 900×672 content; first 36,000 px of the
corpus). Deterministic: **436 of 1,200 steps change the window**; views mounted
p50 212, peak 334; a creating change emits ~91 ops / 8.3 KB of JSON and creates
~14 views.

| Medians of three runs, load ≈ 16 | ms |
|---|---:|
| Step with no window change, p50 | 0.74 |
| Window change: mean / p50 / p95 / max | 3.64 / 3.38 / 6.87 / 11.1 |
| · Rust (runner, kernel, layout, batch) | 1.74 |
| · Swift apply, exclusive of nested syncs | 0.78 |
| · AppKit display + CA flush | ≈ 1.0 |
| · JSON decode | 0.10 |
| Steps over 8.33 ms / over 4.17 ms | 15 / 173 |

The first run, at load 21–28: 7.81 ms mean, 14.9 ms p95, 164 steps over 8.33 ms.
That is the mechanism behind F1's p95 and F9.

### F6. Most of the Rust share is text measurement that was already known

*Measured here.* Over the scroll, the kernel called the host's text measurer
**94,361 times; 616 were misses**. Only 254 visits were to text leaves new that
step; 3,485 were to leaves laid out in an earlier step (93%). Each visited leaf
receives about six distinct `(width, height)` offers in one pass — the lane's own
offer diagnostic shows the same shape, including degenerate ones: width 0 and
−64, and 165 of 945 real CoreText wraps at width 0.

Two memos sit in front of the host and both hold four offers: the kernel's per-leaf
`measurements` (`kernel/src/layout.rs:342`, FIFO) and the host's identified memo
(`host/apple/src/measure/identified.rs:8`, `OFFERS = 4`). Six offers through four
slots evict in rotation, so every pass asks again. A "hit" on the Swift side is not
cheap either: `TextEngine.measure` decodes every run to a `String`, builds a `Spec`
and hashes it before it can look anything up (`Text.swift:667-680`) — 5–12 µs per
call depending on load.

**Variant V1** (`v1-memo16.patch`: both constants 4 → 16), three alternating pairs:

| | Baseline | V1 |
|---|---:|---:|
| Measure callbacks | 94,361 | **1,463** |
| Rust per window change, mean | 1.74 ms | 0.68 ms |
| Window change, mean / p95 | 3.64 / 6.87 ms | 2.70 / 5.19 ms |
| Steps over 4.17 ms / 8.33 ms | 173 / 15 | 63 / 6 |

Layout was 28% of Exact's active main-thread time in the lane's profile (F4), and
nine-tenths of that was these callbacks.

Why already-laid-out leaves are visited at all: every window change dirties the
path from the list's content to the root, and the kernel always computes from the
root (`kernel/src/layout.rs:414`). The re-visited leaves are document rows, not
chrome (bullets, headings, list items — 117 distinct texts). Which Taffy cache
entries miss, and why, was not run down; with V1 it stops mattering to cost.

### F7. Too many window changes, and each creating one runs twice

*Measured here.*

- **Retirement is its own pass.** 197 of the 436 changes (45%) create nothing: a
  row left the trailing edge, so `render` emits `SetChildren` + `DestroyView`
  (`runner/src/instance/window.rs:443-451`), the content is dirty, and the whole
  relayout runs — 2.26 ms mean against 4.77 for a creating change, with 84
  measure callbacks for zero new text. Leading and trailing edges are different
  row boundaries, so they rarely coincide.
- **Creating changes always settle.** 238 of 239 take a second non-empty pass: the
  row's height was the estimate (96), the presenter reads the laid-out height back
  from AppKit frames, calls `exact_list` again, and a three-op batch (the content's
  new height) pays for a second layout from the root, a second JSON batch and a
  second `Presenter.apply` with its finalization passes (`captureScrollPosition`
  and `restoreScrollPosition` over every view, `syncKeyViewLoop`, navigation sync).
  The host already reads row heights from the kernel (`host/apple/src/host.rs:647-659`),
  so that Swift round trip carries no information Rust lacked.
- **Rows are fine.** Every list item and table row is its own list row. A creating
  change arrives every ~150 px of travel and a change of some kind every ~83 px —
  at 3,600 px/s, about 43 per second.

Legend, *read*, differs on each point. Containers that leave the window are **not
released**; the farthest one is taken only when a new row needs it, and with
`recycleItems` the same `NSTextView` gets a new attributed string
(`packages/legend-list-sparse-layout/react-native.mjs:5535-5542, 7589`). Heights are
exact **inside the commit**: the row's shadow node measures in Yoga's measure
function with a 512-entry cache (`ios/internals/ShadowMeasurementUtils.h:89-149`),
so only unmounted rows use the 120 estimate. A block is a run of non-blank lines —
a tight list is one row, one `NSTextView`
(`packages/markdown-parser/cpp/MarkdownBlockParser.cpp:364-396`). The buffer is 500
px split 125 behind / 375 ahead, against our full viewport on each side. Between
row boundaries its JavaScript does O(1) work and sends nothing to native.

### F8. Ruled out

- **The parser.** It runs once, at open, on the worker. No parser frame appears in
  any scroll profile.
- **The JSON batch bridge.** 2% of synchronous time here; about 40 of 1,416 active
  samples in the lane's profile. Ugly, not the problem.
- **Painting only the visible strip of each paragraph.** *Variant V4* (paint a
  mounted paragraph whole, no per-tick invalidation), on top of V1: display + flush
  p50 on unchanged steps 0.87 → 0.75 ms; total −8%. Minor. Whether AppKit really
  paints offscreen rows under V4 was not verified.
- **The render server and GPU.** Close in F1; no render-attributed hitch in the
  120 Hz series.
- **Total CPU, view creation.** F1. Swift-side view work is 0.8 ms per change.
- **Taffy's speed.** What costs is how often the leaves are asked, how much an
  answer costs, and that we start from the root — F6. Yoga asks a text node once
  or twice per layout and Legend pays even that off the main thread.

### F9. Load

*Lane, recorded.* Across the series Exact went 3.33 → 21.67 → 40.00 hitch ms/s as
the one-minute load went 7 → 22 → 30; Legend went 0.83 → 3.33 → 4.17 over 9 → 18
→ 23. A design whose every millisecond is on the frame's critical path degrades
with contention; one whose critical path is nearly empty does not. On a quiet
machine the gap is small (3.33 vs 0.83) — the lane's earlier near-ties were real.
The reader has to be good on a busy machine too.

## 3. The two designs, side by side

| | Exact Markdown (HEAD `ed541b82`) | Legend Markdown (*read*) |
|---|---|---|
| Scroll owner | AppKit, single-threaded, on main | AppKit, concurrent; main synchronizes |
| Scroll → list | `boundsDidChange` → `syncLists`, synchronous | async coalesced `onScroll` to the JS thread; 2 px gate; band early-out |
| Window | one viewport behind, one ahead (`window.rs:337-341`) | 125 px behind, 375 ahead, velocity-projected |
| Leaving rows | destroyed at once, with a relayout | kept until a new row needs the container |
| Entering rows | kernel nodes, Taffy nodes and `NSView`s created | an existing row's props change (`recycleItems`, by item type) |
| Row | wrapper → column → box → `text` → runs; one layer per paragraph | ~9 views, one `NSTextView` (TextKit 1) per block |
| Row unit | every block, list item and table row | a run of non-blank lines |
| Height | estimate 96; measured after layout, corrected in a second pass on main | exact in the commit, on the JS thread; 512-entry cache; estimate 120 for unmounted rows |
| Layout | Taffy from the root, on main; CoreText through a callback | Yoga in the commit, on the JS thread |
| Text source | run values through the plan into kernel nodes | native view pulls the block's text by id from a C++ document; md4c on a background queue |
| Offsets | prefix sums (`heights.rs`), O(log n) | sparse treap, O(log n) |
| Anchor | runner sets `scrollTop` | native `maintainVisibleContentPosition` in the mount transaction |

React Native's thread model gives Legend the right placement without anyone having
designed it. Ours has one thread, by design (LLP 1016 D1 keeps I/O and parsing off
it; nothing keeps layout off it), so placement has to come from *when* on that
thread the work runs, and from the platform owning the scroll.

## 4. What the findings suggest

Decides nothing; the Markdown lane owns the code. In order of evidence.

1. **Give the memos a pass's worth of offers.** Measured here (V1): two constants,
   −98% callbacks, p95 of a window change 6.9 → 5.2 ms. The deeper version: make a
   host hit cheap (key on the kernel's paragraph stamp, so no `Spec` is built), and
   find where the width-0 / −64 offers come from — those are real CoreText wraps.
2. **Let the platform scroll, and fill behind it.** Two halves. (a) Responsive
   scrolling for a contained list: the lane already built it
   (`Responsive-scroll.app`, `isCompatibleWithResponsiveScrolling`) and measured it
   only where there was no gap to close — the 231 KB fixture on the pre-merge
   build, 1.67 vs 1.67 ms/s. It is untested where the gap is 21.67 vs 3.33.
   (b) Move `syncLists` out of the bounds-change notification to a coalesced turn
   after the commit, with a time budget. One viewport of overscan is ~170 ms of
   slack at 3,600 px/s. The reader's existing check — fast scrolling never shows
   blank or stale content — is the guard. Needs the lane's HID and Instruments
   permissions to measure.
3. **Stop paying for retirement.** Retire rows when a creating pass runs anyway, or
   when idle, with hysteresis. 45% of changes; 10–13% of all synchronous time.
4. **Settle in one call.** Loop measure → re-render → layout inside `list_viewport`
   and return one batch. Removes a JSON batch, a `Presenter.apply` and its
   whole-tree passes from 238 of 436 changes.
5. **A relayout boundary at a definite-size scroll container.** A window change
   cannot affect anything outside the list's content box; computing from the root
   is what re-visits old leaves and re-runs the ancestors' flex passes. A kernel
   change, so LLP 1001's to weigh.
6. **Coarser rows and a truer estimate, in the app.** A tight list as one row is
   Legend's unit and cuts changes per second. An estimate nearer this corpus's rows
   cuts extent corrections (not the settle pass — 4 does that).

1, 3 and 4 shrink the burst; 2 takes the burst off the frame. F9 says 2 is the one
that makes the reader robust, and F6 says 1 is nearly free.

## 5. Confidence and limits

- **High:** F1, F2, F3 (thread names and symbols in the lane's traces), F4's
  attribution, F5–F7's counts (deterministic), V1's callback count.
- **Medium:** in-process timings — a synthetic `scroll(to:)` + forced `CATransaction.flush`
  per step under load 12–28, three runs. They rank phases and size V1; they are not
  frame times and not a Legend comparison. The 60 Hz CPU table and the 120 Hz
  per-frame table come from different series, fixtures and Exact builds.
- **Read, not run:** everything about Legend's internals in F4, F7 and §3. Which
  thread runs `measureContent`, and whether `view.measure()` is synchronous in
  `useLayoutEffect` on react-native-macos 0.81.7, are inferences from RN's commit
  model, consistent with TextKit appearing on the JS thread in the lane's trace.
- **Not established:** that suggestion 2 closes the gap — it is an argument from
  Legend's architecture plus F2/F9, not a measurement. Why specific Taffy cache
  entries miss (F6). Anything about iOS, where UIKit scrolls on main and the same
  synchronous fill runs. Anything about the web host, where the browser scrolls on
  its compositor. Nothing here was measured on a quiet machine.
- Legend's app binary is stripped, so its own frames are addresses; only system
  frames are attributed.

## 6. Reproducing

Local and disposable, under `target/markdown-comparison/research-scroll-gap/`
(its `README.md` lists every file): the exported per-frame tables and their
summaries; `phase-probe/` with the probe sources, the three patches
(`instrumented-swift`, `v1-memo16`, `v4-paint-once`), raw per-step JSON and
`SHA256SUMS`. The Rust archive is `cargo build --release -p markdown-apple` with
`EXACT_UPDATE_TRUST=development` at `ed541b82` (29 s cold here); the probe is one
`swiftc` line (`build-probe.sh`, ~70 s). The experiment worktree and binaries were
removed.

## 7. Afterwards — 2026-09-19

The suggestions in §4 were carried out the next day, on a different machine (an
M4 Pro, 60 Hz), by measurement; `apps/markdown/README.md` has the results and
`llp/1010` §6 the list behaviour as built. What this document got right, and what
it left open:

- **F6's open question was the largest cause.** "Which Taffy cache entries miss,
  and why, was not run down." They missed because vendored Taffy measured a flex
  item's min-content size whether or not it used it (`unwrap_or` evaluates its
  argument). The reader's `list` is a `flex: 1` scroll container, so every
  layout laid its whole content out at width zero — the width-0 and −64 offers
  of F6 — and the zero-width and real-width requests share one cache slot per
  node, so every row was laid out again at its real width. Larger memos (§4.1)
  hid the cost; making the measurement lazy removed it: 59,682 measure calls
  over three seconds of scrolling became 614. `vendor/taffy/EXACT-PATCHES.md`,
  patch 7.
- **§4.2(b), §4.3 and §4.4 were done as written**: the fill moved out of the
  bounds-change notification to a budgeted turn after the commit; retirement
  rides with creation; the host settles in one call.
- **§4.2(a), responsive scrolling, is still untested.** It cannot be driven from
  inside the process: AppKit's concurrent tracking takes its events from the
  window server. It needs this lane's HID permission.
- **F8 ruled out strip painting too early.** Removing the per-tick invalidation
  saved little (V4) because AppKit still painted each paragraph when it was
  first seen, inside that frame's commit. Painting a paragraph into its own layer
  contents in the turn that mounts it took text out of the frame: the median
  trackpad frame went from 4.1 ms to 2.0.
- **§4.5, a relayout boundary, was not needed** once the zero-width pass was gone:
  an unchanged row is a cache hit at its wrapper and its leaves are not visited.
- **§4.6**: rows were flattened (a paragraph is one `text`), not coarsened.

Against Legend on that machine, with the same synthesized input in both
processes: inputs committed later than one 120 Hz frame, of 2,400, were 624
before, 6 after, and 84 for Legend. The Hitches instrument at 60 Hz no longer
separates the two (three ten-second runs each: 0, 2, 1 hitches against 1, 0, 0,
three of those four being the first frame of the scroll in either app), and F2's
pattern is gone: one Exact hitch in thirty seconds follows an app update, of
8.73 ms.

The longest wait left in a scroll was not the app's. Some fifteen seconds after
launch AppKit encodes the application's restorable state, asks the window server
for the order of its windows and waits on the main thread, 20 to 34 ms while a
scroll keeps the server busy. A `sample` shows that blocked stack where a time
profile shows nothing, which is how it was first misread as Launch Services.
`ApplePersistence`, registered as a default before the application is made,
removes it: no input later than 16.67 ms in any run, and late inputs at 2 of
2,400.

What a row still costs, from a profile of that build, is where a later pass
would look. A fill unit that creates a row is 2.6 ms at the median and 7 ms at
the 99th percentile, about 0.5 ms in the runner and kernel and 0.35 ms in the
presenter for each view it creates; the long ones are paragraphs of many inline
runs, each of which is a `NodeView`. And a paragraph is typeset twice: once in
black to be measured, once in its colours to be painted (`TextShapeKey` carries
the paint), 0.7 ms at the median in the unit that paints it.

## 8. Current evidence boundary — 2026-09-21

The question this revision answers is whether Exact still has a general
"native text and virtualized lists have frame-time cliffs" problem. That wording
is too broad. The evidence supports three narrower propositions, at different
strengths:

| Proposition | Evidence and snapshot | Strength on current main |
|---|---|---|
| Ordinary macOS scrolling is no longer explained by the original synchronous-fill cliff | Late synthetic inputs fell from 624 of 2,400 to 6 after the list, Taffy and paint changes, and to 2 after disabling AppKit state saving. The 60 Hz Hitches series no longer separated Exact from Legend. The F2 correlation between Exact hitches and app updates disappeared (§7). | **Strong historical evidence of repair.** Later changes did not restore the old mechanism, but this is not a fresh measurement at `6e583ae1`. |
| Exceptional row construction and large jumps had real tails | On the repaired 2026-09-19 build, a creating fill unit was 2.6 ms median and 7 ms p99. Long rows were paragraphs with many inline runs, each represented by a `NodeView`; coloured paint caused a second typeset, 0.7 ms median and 3 ms worst. A 250,000-point jump synchronously built its landing scrollport in one report and stalled for 30–55 ms. `QUEUE.md` retains all three as open work. | **Measured, unresolved at that snapshot; stale as a current-main number.** Commits after the measurements changed text reuse, child lookup and native batch costs, so the exact tails must be remeasured before attributing them to today's code. |
| iOS lacks the macOS scheduling protection | LLP 1010 §6 says the Mac presenter fills between frames with a per-report budget and explicitly says, "UIKit does not do this yet." The shared runner can ration creation, but `PresenterIOS` still reports the whole window in the scroll callback and paints text when first seen (`QUEUE.md`). | **Strong structural evidence; no physical-phone performance measurement.** This establishes a missing mechanism, not an observed current iOS hitch rate. |

### 8.1 What landed after the tail measurements

The 7 ms row-fill and 30–55 ms jump measurements must not be quoted as though
they were taken at current main. Between that profile and `6e583ae1`, relevant
mainline work included:

- `7442359c`, which stopped remounting retained siblings when prepending a native
  child and reduced reverse-scroll presenter work in its diagnostics;
- `cdd4e493`, `5dba3fcd`, `b2fc632d`, `90fe55ea`, `f3f419fb`, and `c95bbbd4`,
  which reduced text-cache cleanup, line-breaking, scalar retirement and flow
  lookup work;
- `135b7b89` and `1413a403`, which reduced repeated native child and listener
  searches during bulk construction;
- `7bb344b2`, `d4fc0b51`, and `c7df13c7`, which removed native batch-string,
  path-construction, and no-op chrome-set work.

Those changes make the old measurements a valid bug-finding lead, not a current
benchmark result. None is evidence that the large-jump path was eliminated, and
the maintained queue still records it as synchronous.

### 8.2 The Legend comparison is still open

The evidence after the repair is mixed rather than convergent. One four-pair
pilot at `06149e8d` favored Exact (mean hitch time 3.75 vs 5.00 ms/s), but the
fixed eight-pair `a666d46f` comparison did not (median 5.83 vs 5.42), and the
six-round integrated `e84ad146` pilot also favored Legend (mean 4.861 vs
2.361; Exact won one round). Some trials had reduced presentation cadence, and
the available 60 Hz synthesized-input result is not a physical 120 Hz result.
The reader README therefore correctly says the lower-hitch goal remains
unproven. No one of these small, noisy series establishes that current Exact is
generally worse, either.

### 8.3 Experiment that would close the question

Freeze one current-main Exact binary and the pinned Legend control, then run a
predeclared alternating comparison on a physical 120 Hz Mac with real HID wheel
and trackpad input. Keep corpus, window, thermal state and background workload
fixed; reject trials only by the recorded focus, input and content-validity
rules already used by the lane. Report presentation cadence alongside Hitches
so a low hitch count cannot be purchased by presenting fewer frames.

Instrument the same Exact binary separately, without a Legend comparison, for:

1. forward and reverse steady scrolling, including dense inline-run paragraphs;
2. 250,000-point jumps and scroller drags, reporting p50, p95 and maximum
   synchronous landing cost rather than only the worst observed stall;
3. creating fill-unit cost grouped by inline-run count, with runner/kernel,
   presenter, shaping and paint phases separated;
4. CoreText shaping count per paragraph, to determine whether measure and paint
   still duplicate work after the later reuse commits;
5. uncovered-band and anchoring correctness at the existing 48,000-point/s
   stress rate.

Run the corresponding scroll, reversal and jump suite on a physical ProMotion
iPhone. That is the evidence needed to turn the structural UIKit gap into a
performance claim or to clear it.

Until those runs exist, the defensible conclusion is: **ordinary macOS scrolling
was repaired; construction-tail and jump work remain open leads; iOS lacks the
Mac's scheduling mechanism; superiority to Legend at 120 Hz is unproven.**
