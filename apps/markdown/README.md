# Markdown

A reader for Markdown files, on macOS, iOS and the web (LLP 1033).

```sh
exact install markdown     # ~/Applications/Markdown.app, and `mdview` on PATH
mdview README.md
mdview ~/notes             # a folder opens its README.md

exact run markdown README.md   # from this repo, in the foreground, with its log
bun host/web/dev.mjs --app markdown
```

Open a document with `⌘O`, a Finder double-click or Open With, a path typed in
the field at the top, `mdview` from a shell, the Open… and Folder… buttons, or
by following a link inside a document that names a file beside it. All of
those are the same event: a value arriving at the `open-file` node, which is
a `doc:` path the host minted for what you chose (LLP 1069.010 D1). A second
`mdview` hands its file to the copy already running rather than starting
another.

Headings, prose with bold/italic/code/links, ordered and unordered lists,
block quotes, rules, fenced code, images, and pipe tables. Raw HTML is shown
and interpreted by nothing. Reference links, setext headings and footnotes are
not interpreted. Files are read up to 4 MB and must be UTF-8; one that will
not open leaves the document you were reading where it was and says why.

The strip on the left lists the Markdown beside the open file. Text selects
and copies; a link to a page opens in the browser.

## How it is put together

- `parse/` (`markdown-parse`) — the parser: text to ordered blocks and styled
  runs, and their conversion into plan values. No I/O, no host. `apps/llp`
  depends on this crate; it is what the two readers share.
- `data/` (`markdown-data`) — the data source: `open(path)`. Reads through
  storage requests under `fs.read doc:/` (stat, the folder's README, the
  file, the folder beside it; LLP 1027.001) and parses on the host's worker
  through a continuation, so neither is on the thread that lays out (LLP
  1016 D1). It owns the open document, which is why a refusal does not lose
  it.
- `app.contract` — the whole view. A document is a flat list of blocks
  (Contract inlines components syntactically and cannot recurse), and a
  paragraph is a `text` node whose children are its runs.
- `app.json` — `file_handlers` says what this opens; the macOS bake derives
  `CFBundleDocumentTypes` from it. `app.command` is `mdview`.

On the web the reader opens what Open… or Folder… chose, through the File
System Access API's pickers where the browser has them (Chromium); elsewhere
the pickers refuse, and the reader shows its welcome document. A typed path
opens nothing there: a page has no filesystem of its own.

## Smoothness continuation — 2026-09-19

The overnight `lane/markdown-smooth` work is preserved and integrated with
`origin/main` through `4d91f9ee`; the lower-hitch goal remains unproven. LLP 1044 is
the inherited investigation, not a new design authority. Retained work increases
measurement memo capacity, settles measured rows within one native call, defers
retirement-only passes, and permits AppKit responsive scrolling.

The `06149e8d` budgeted-list pilot has four valid matched pairs at 120 input
events/s: Exact mean hitch time is 3.75 ms/s versus Legend 5.00 ms/s,
with three wins and one tie. Baseline trials 1 and 3 were rejected for focus;
candidate trial 1 was rejected for a 31.66 ms input gap. This is promising pilot
evidence, not completion of the goal. Raw trials are retained under
`target/markdown-comparison/smooth/resume-budget-06149e8d-120hz`. That snapshot
passed 1,896 Rust tests (9 ignored), 226 native tests, the app bundle, and the
functional scroll/resize/file/copy checks.

The next integration keeps the same CADisplayLink scheduler, worker IOSurfaces,
list budget fairness and urgent visible fallback. It adopts upstream's reused
line-break tokenizer and disables AppKit automatic application-state saving;
the separate after-commit/synchronous paragraph-paint path is not adopted.
Published-dependency validation uses clean Ibex `9cbf9e62`, origin's `ureq` 3.4.0
lock entry, and explicitly identified external Hermes/compiler artifacts. This
new source and dependency snapshot still needs its own gates and comparison.

Independent review found that a distant pinned row could conceal an unmounted
gap from the coverage fast path. Discontiguous mounted positions now force a
viewport report. Retirement deferral now requires unchanged viewport bounds and
pins, so pin release and a hidden or resized viewport retire the appropriate rows.
The native pinned-gap fixture and all ten web list tests pass. Offscreen text
rasterization admits at most two outstanding jobs without a backlog, retries
deferred paragraphs, and does not retain retired NodeViews. Integration also fixes
completed worker-stage argument retention and a race in the Linux mailbox test.

Further work reuses owned measurement specifications at new widths, resolves
JSON text metrics in the same f32 precision as the measurement ABI, orders raster
admission by distance from the viewport, and ignores a late worker result after
an urgent paint has already supplied matching pixels. A 30 Hz admission-pump
experiment did not improve the comparison and was reverted.

The earlier candidate, `4fce1bd4`, defers speculative text admission after a batch
and uses the scroll document's clipped viewport to decide whether `updateLayer`
must rasterize synchronously. AppKit can include offscreen overdraw in a child
view's `visibleRect`; a native regression reproduced that unnecessary synchronous
paint. Both changes have failing old-code controls and passing corrected runs;
all 221 native tests pass. A separate diagnostic sample contained no main-thread
text-raster rendering samples, but this does not establish a scrolling advantage.

| Later comparison | Exact median hitch ms/s | Legend median hitch ms/s | Outcome |
| --- | ---: | ---: | --- |
| `a666d46f`, fixed 8 pairs, 120 input events/s | 5.83 | 5.42 | Goal not met |
| `a666d46f`, fixed 4 pairs, 60 input events/s | 10.00 | 9.17 | Goal not met |
| `122845a1`, 3-run publication-guard pilot | 5.83 | 2.50 | Reverted; retained-build median 5.00 |
| `4fce1bd4`, 3-run offscreen-overdraw pilot | 3.33 | 1.67 | No win; retained-build median 2.50; only 2 Legend runs valid |

The last pilot retained Legend's third attempt as a failure: its pre-input capture
showed a blank document, so timing never started. All other pilot trials passed
direct-focus and content-change checks. Cadence is now also reported by joining
frame-lifetime display/swap/surface identifiers to this process's updates, then
counting distinct presentation timestamps in the input interval. Display-wide
cadence is reported separately; neither proves changed pixels. Some Exact trials
fell well below 120 presentations/s, including 89.2 and 91.9 in the eight-pair
confirmation and 103.2 in the latest pilot. Lower hitch time at a lower cadence
is not accepted as a 120 fps improvement. Every trial remains in its
`smooth/resume-confirm-a666d46f-*`, `smooth/resume-publication-122845a1-*`, or
`smooth/resume-overdraw-4fce1bd4-*` directory.

A subsequent control (`9df53eae`) disabled responsive scrolling on the contained
panes. It regressed: median hitch time was 7.92 ms/s across four valid trials,
versus 4.17 for the retained build (five) and 3.33 for Legend (four). One candidate
attempt had no trace because Instruments could not attach; one Legend attempt
failed before input. Both are retained in `smooth/resume-mainthread-9df53eae-120hz`.
The control was reverted. Its 221 native tests and unchanged-binary functional
retry passed; the first agent launch again timed out before readiness.

The next isolated candidate defers offscreen worker-result publication to the
existing text pump. Visible completions still publish immediately, and a pending
surface can be published on visible takeover without rerendering. Invalidation
and retirement discard pending publication. All 222 native tests pass; restoring
the old immediate-publication behavior fails three assertions in the new
regression. The frozen `bb134e09` binary (`05516cca…729`) passes forward
7,200-point/reverse scrolling, three widths, file switching and full logical copy
with the unchanged hash. Its pilot rejected eight of nine trials because cmux
PID 8099 took focus; only Legend trial 3 was valid (12.50 ms/s), leaving no candidate
performance evidence. Direct-AX/OCR blank checks reject the owned negative-focus
control; normal/double-speed 120 Hz input produced 116 captures each, maximum
inkless bands 92/92.5 points and none over 250. Endpoint pixels still changed while
settling. Startup retained a 60-second direct timeout (`dyld_start`, 96 KiB), a
20-second agent timeout, and an unchanged successful direct retry in 0.289 seconds
with 80 ms app boot. These are functional observations, not a smoothness win.

The frozen `bb134e09` candidate passed forward/reverse scrolling, three widths,
file switching, and full logical copy with the same 2,153,496-character hash.
Its first agent launch timed out before readiness; the unchanged binary passed
on retry. The preceding publication candidate had the same first-launch failure.
These attempts are retained and their cause is unconfirmed. They preceded the
`06149e8d` validation and pilot above; the published-dependency integration still
needs final workspace validation and a convincing comparison before delivery.

The one-shot experiment (`3127234e`, now reverted) replaced display-link scheduling with
coalesced one-shot callbacks, preserving the existing coverage thresholds and
urgent synchronous fill. A generation invalidates callbacks after settlement;
reentrant scheduling is coalesced until the active slice completes. Later slices
yield for four milliseconds, which does not guarantee separate display frames.
All 152 native tests passed, and an independent source review found no material
issue. The corrected comparison below did not establish a performance advantage,
so the existing display-link scheduler is retained. Final whole-workspace check
results are recorded separately against the retained commit.

Three alternatives were rejected. Shared AppKit backing did not improve its
uncontested comparison. Delaying refills until a quarter of the overscan was used
halved diagnostic list-fill calls (245 to 124 over ten seconds), but raised their
p95 duration from 3.1 to 4.7 ms. Its three completed candidate trials measured
5.00/5.00/14.17 hitch ms/s, versus baseline 4.17/6.67/4.17/11.67 and Legend
3.33/3.33/0.00/1.67; one candidate launch failed before measurement. The delay
was reverted. Separate signpost recordings are diagnostic and their nested
intervals overlap; totals are not additive.

**Observer correction:** endpoint screenshots exposed inactive reader windows in
some runs that the original foreground check had accepted. This invalidates the
apparent one-shot win and the earlier integrated comparison as decisive evidence.
The audit and every raw trial remain in `smooth/resume-*/foreground-audit.json`.
The interrupted confirmation is retained with its owned-process cancellation
record. Its planned 60 Hz series never ran. No result is removed or retroactively
selected into a favorable sample.

The replacement observer directly queries Accessibility for the focused process
once per second and after input, records every check, and rejects missing or
mismatched coverage. A negative control deliberately activated a second owned
app two seconds into scrolling; the observer rejected it on the next check.
The fresh fixed comparison uses the same 2,242,305-byte corpus, 900×700 outer
windows, ten seconds at 3,600 pixels/second, fresh processes in alternating order,
and only the Hitches instrument. Eight pairs at 120 input events/second and four
at 60 completed without rejected trials. These are input rates, not measured display FPS. No owned
builds run during measurement; other activity on the shared machine is uncontrolled.
The apps retain their own reading layouts. The corrected 120 Hz comparison
completed all sixteen trials with valid direct-focus records and active endpoint
windows: Exact median **6.25 hitch ms/s**, Legend **4.58**; longest hitches
16.67 and 8.33 ms respectively. Exact runs were
8.33/15.83/9.17/5.83/6.67/4.17/4.17/4.17, and Legend runs were
5.00/21.67/4.17/3.33/4.17/4.17/7.50/5.00. These results fail the goal.
At 60 Hz, Exact measured 22.50/22.50/20.00/23.33, median **22.50 ms/s**,
versus Legend 44.17/36.67/40.83/13.33, median **38.75**; both longest hitches
were 16.67 ms. The rate-dependent result does not establish the requested
advantage. These measurements describe the reverted one-shot experiment, not
the retained display-link build. Do not pool them with the invalidated series.

WindowServer captures, rather than the agent's alternate paint path, verify
reading at multiple widths and copying the corpus (2,153,496 characters). The
one-shot candidate's separate rapid-scroll captures sampled 116 frames at each
of normal and double speed without a 250-point inkless run. Endpoint pixels still
changed during settling; these samples do not establish every-frame continuity.
All binaries, probe hashes, traces, failures and screenshots remain under
`target/markdown-comparison/smooth/resume-*`. Nothing has been pushed.

## Performance work in progress — 2026-09-18

The 2026-09-18 comparison: **Exact opens this README ahead of Legend, but Legend still
scrolls more smoothly on the full corpus**. Reusing scalar measurements repairs
part of the scrolling regression introduced by integrating `origin/main`. The
speed goal is not achieved.

### Separate upstream experiment — 2026-09-19

The following measurements describe upstream through `4d91f9ee` on its separate machine
and scheduling/rendering path. This integration retains the display-link pump and
worker IOSurfaces, including `bb134e09` publication; it does not adopt upstream's
after-commit observer or synchronous whole-paragraph CGImage painting. These
numbers therefore do not validate the integrated candidate.

**Scrolling now measures ahead of Legend by the one method available to this
pass, at 60 Hz here and at 120 Hz on a Retina panel (below); nothing here is a
real-input result.** An
M4 Pro Mac mini, two 60 Hz displays, an agent session without Accessibility,
event-posting or screen-capture permission. So input is synthesized inside each
process by an injected library (`DYLD_INSERT_LIBRARIES`; both apps are ad-hoc
signed without the hardened runtime): the same 120 wheel events a second of 30
points, the same 2.34 MB document (the 40 largest `llp/*.md`), a 900×700 window,
20 seconds, three interleaved rounds. Legend is `2b7b91d9`, ARM64 Release, with
the one build-only edit recorded above. Medians, with the range of the three:

| Wheel path: input to commit, 2,400 inputs a run | Exact `6214c47` | Exact now | Legend |
| --- | ---: | ---: | ---: |
| p50, ms | 3.69 | 1.97 | 3.43 |
| p95, ms | 13.74 | 2.88 | 7.85 |
| p99, ms | 20.18 | 5.56 | 13.78 |
| max, ms | 30.2 | 32.8 | 27.6 |
| Inputs later than 8.33 ms | 624 (498–689) | 6 (4–7) | 84 (26–94) |
| Inputs later than 16.67 ms | 60 (44–68) | 3 (1–3) | 3 (2–5) |
| Refreshes showing space no row covers | 0 | 0 | 0 (0–1) |

An input is *late* when more than one 120 Hz frame passes between the moment it
was due and the end of the Core Animation commit that follows its delivery: a
refresh-rate-independent stand-in for a hitch, read from a run-loop observer
ordered just after Core Animation's.

| Trackpad path: vsync to commit, 1,200 frames a run | Exact `6214c47` | Exact now |
| --- | ---: | ---: |
| p50, ms | 5.36 | 1.98 |
| p95, ms | 10.37 | 4.29 |
| p99, ms | 11.64 | 5.82 |
| max, ms | 12.8 | 8.6 |
| Frames longer than 8.33 ms | 271 (254–338) | 1 |

A phased gesture is AppKit's to scroll, from its own display link on the main
thread, so this is what a finger gets. Legend has no column: its scroll view
uses AppKit's responsive scrolling, which takes events from the window server
and ignores the `changed` events an in-process driver can deliver.

Under the Animation Hitches instrument, at 60 Hz, three interleaved ten-second
runs each of this build and Legend:

| Animation Hitches, 60 Hz, three runs | Exact now | Legend |
| --- | ---: | ---: |
| App updates in the scroll | 600, 600, 600 | 108, 127, 115 |
| App update p50, ms | 0.87, 0.83, 0.81 | 2.70, 2.79, 2.61 |
| App update p95, ms | 4.03, 4.10, 3.93 | 6.80, 6.41, 7.61 |
| App update p99, ms | 5.55, 5.53, 5.83 | 7.89, 7.42, 8.48 |
| App update max, ms | 8.08, 8.73, 9.14 | 7.92, 9.84, 13.57 |
| Hitches counted in the scroll | 0, 2, 1 | 1, 0, 0 |

Every hitch is one 16.67 ms frame, and each was read for what it is
(`probe/hitchrows.py`). Three of the four are the first frame of the scroll,
within 40 ms of its start, in Exact twice and in Legend once: the instrument's
view of a display pipeline leaving idle. The fourth is Exact's, 2.2 seconds into
a run, marked "potentially expensive app update", in the run whose longest
update was 8.73 ms. So at 60 Hz the instrument does not separate the two: past
the first frame, one hitch in thirty seconds of Exact and none in Legend. Exact
updates on every frame, since it scrolls on the main thread; Legend only when it
mounts rows, since AppKit scrolls it concurrently. That difference is what the
120 Hz machine has to judge. An earlier series, on the build before the
window-restoration change below, counted 1, 1 and 4 for Exact and none for Legend.

**What was wrong**, in the order measurement found it (`llp/1044` F4–F7 asked the
questions; none of this was the parser, JSON or view creation):

1. **Every layout laid the whole mounted list out twice.** Vendored Taffy passed
   its min-content measurement of a flex item to `unwrap_or`, which evaluates
   it whether or not it is used. The reader's `list` is a `flex: 1` scroll
   container, whose automatic minimum is zero: on every pass its content was
   laid out at no width, every paragraph wrapped a word to a line, and that
   evicted each row's cached layout so it was laid out again at its real width.
   Measure-function calls over three seconds of scrolling: 59,682 → 614
   (`vendor/taffy/EXACT-PATCHES.md`, patch 7).
2. **A created row took two batches.** The host now lays out, reads the rows back
   and reports again inside one `exact_list` call; the presenter gets one batch
   with final frames.
3. **`Presenter.apply` read every view, several times, on every batch** — for a
   navigation stack, a popover, a tab list, a toolbar, a shortcut, a context
   panel, the key-view loop — 18% of the main thread's long periods at
   `6214c47`. The passes visit a small index
   of the props they look for, kept by an observer on `NodeView.props`; the
   key-view loop waits for a scroll to pause.
4. **The window was filled inside the frame that scrolled.** A report now
   creates the rows the scrollport shows and one more
   (`Runner::list_viewport_within`); the rest of the window is filled between
   frames, from a run-loop observer ordered after Core Animation's commit, a unit
   at a time against a quarter-frame budget. Rows that left go with a report that
   creates, never in a report of their own (45% of reports before). An agent's
   reports stay whole and synchronous.
5. **Text was painted on every frame.** AppKit paints a view's layer when the
   view can first be seen and will not be asked sooner, so each frame painted
   the strips the scroll exposed and a long paragraph's first paint was a long
   frame. A paragraph in a windowed list now renders `draw` into its own layer
   contents in the turn that mounted it (pixel-identical to `draw` at 1× and 2×,
   and across a switch to dark mid-scroll); the frames that follow only move it.
   Paragraphs taller than 4,096 points, and every paragraph outside a list, keep
   the strips.
6. **A row was four views deep.** A paragraph is now one `text` with its own
   padding and the reading column's measure; flexing text says `min-width=0`.
   153 nodes at boot → 118. That exposed a kernel bug: a padded `text` that
   flexes was wrapped at its border box and painted in its content box, a line
   short (`kernel/src/layout.rs`; `a_padded_text_that_flexes_wraps_in_its_content_box`).
7. **The window was restorable.** Scrolling invalidates AppKit's restorable
   state, and its flush waits on the window server on the main thread — 19 and
   25 ms at the same second of two runs. Nothing restores this window.

8. **A paragraph was typeset and painted in one turn, and each made its own
   line-break tokenizer.** Making a `CFStringTokenizer` opens an ICU break
   iterator, a tenth of what measuring a paragraph cost; one is now shared. And
   between frames, typesetting a mounted paragraph in its colours and painting it
   are a unit each, where together they were the longest thing a turn did. Pixels
   are identical, in light and across a switch to dark. Four interleaved rounds in
   a later sitting, the build of the tables above first:

   | Trackpad path, 1,200 frames a run | Tables above | Shared tokenizer | And typeset apart from paint |
   | --- | ---: | ---: | ---: |
   | Frames longer than 8.33 ms | 4, 5, 1, 2 | 2, 2, 0, 1 | 0, 0, 1, 0 |
   | Longest frame, ms | 13.8, 16.5, 9.5, 11.4 | 11.4, 9.1, 8.1, 9.0 | 7.7, 8.0, 8.9, 7.5 |
   | p99, ms | 6.07 | 6.21 | 5.82 |
   | Wheel path p99, ms | 5.20 | 5.58 | 4.54 |
   | Wheel path, inputs later than 8.33 ms | 5, 6, 5, 3 | 3, 7, 5, 4 | 4, 6, 7, 7 |

   The wheel path's late inputs did not move: about half of them in every run
   were the one AppKit wait of cause 9. A paint costs 2.1 to 2.6 times as
   much at two pixels a point, which these 1× displays do not show.
9. **AppKit saved the application's state mid-scroll.** Some fifteen seconds
   after launch it encodes the application's restorable state, and to do that it
   asks the window server for the order of the app's windows and waits for the
   reply on the main thread: 20 to 34 ms while a scroll keeps the server busy,
   in every run, the longest wait left. A `sample` shows the blocked stack, which
   a time profile does not, and an earlier reading of this as Launch Services
   was wrong. Encoding nothing in an `NSApplication` subclass moves the wait to
   the next private caller of the same question. `ApplePersistence`, AppKit's own
   switch, registered as a default before the application is made, removes all
   of it; the window's frame is saved as before. Three interleaved rounds:

   | 2,400 inputs or 1,200 frames a run | Before | Persistence off |
   | --- | ---: | ---: |
   | Wheel path, inputs later than 8.33 ms | 5, 5, 4 | 2, 2, 1 |
   | Wheel path, inputs later than 16.67 ms | 3, 2, 1 | 0, 0, 0 |
   | Wheel path, longest wait, ms | 34.5, 24.0, 19.5 | 11.4, 10.8, 10.9 |
   | Trackpad path, longest busy period, ms | 30.8, 78.4, 12.4 | 13.9, 11.6, 11.1 |
   | Main-thread periods over 16.67 ms, both paths | 1, 1, 1, 1, 3, 0 | none |

Zero refreshes showed uncovered space in any run, including a reversal, a
250,000-point jump and 48,000 points a second. A jump still builds the rows it
lands on synchronously (one stall of 30–55 ms).

**What the work cost elsewhere: nothing found.** Measured afterwards on the same
M4 Pro with three in-process probes (`probe/resizeprobe.m`, `startprobe.m`, and
the scroll probe's footprint), `6214c47` against main at `abe5145`, which also
carries the move to Taffy 0.14:

| Same 2.34 MB document | Exact `6214c47` | Exact `abe5145` | Legend |
| --- | ---: | ---: | ---: |
| Live resize, one 6-point step to its commit, p50 / p95 ms | 7.22 / 11.33 | 7.58 / 9.49 | 6.51 / 9.47 |
| Resize steps over 16.67 ms, of about 890 | 4 | 4 | 12 |
| Footprint after scrolling 72,000 points, MB | 172–174 | 62–65 | 157–158 |
| Launch to first committed content, median / p90 / max ms | 228 / 244 / 535 | 202 / 209 / 221 | 200 / 209 / 212 |

The footprint fell because the zero-width layout pass of cause 1 had been
wrapping every paragraph a word to a line and leaving those lines in the text
cache. A resize step is 7.25 ms of main-thread time, about two fifths layout and
apply and three fifths repaint; keeping the measuring typesetter across widths
and skipping the ink index on whole paints were each tried and saved 1.5% and 1%
of it, nothing measurable in a scroll, and were not kept. A sampler had made them
look like 30% and 8%: it is not a ruler.

**At 120 Hz, on a Retina panel.** The same probe over SSH on an M5 Max, 3456×2234
at 2×, three interleaved rounds, every window placed visible and unactivated, the
machine under other agents' load throughout (load average 30 to 67). Exact before
is `1bc430f`, now is `4d91f9e`:

| Wheel path, 2,400 inputs | Exact before | Exact now | Legend |
| --- | ---: | ---: | ---: |
| Input to commit p50 / p99, ms | 3.43 / 7.90 | 1.84 / 3.19 | 3.06 / 4.55 |
| Inputs later than 8.33 ms | 3, 12, 58 | 1, 1, 1 | 1, 6, 4 |
| Longest main-thread period, ms | 7.9, 35.0, 15.8 | 3.4, 4.2, 4.3 | 9.2, 13.1, 9.7 |
| Main thread busy, ms per second | about 300 | about 154 | about 410 |
| Refreshes showing uncovered space | 0 | 0 | 1, 1, 1 |

| Trackpad path, about 2,400 frames | Exact before | Exact now |
| --- | ---: | ---: |
| Vsync to commit p50 / p99, ms | 2.15 / 6.44 | 0.50 / 0.87 |
| Frames longer than 8.33 ms | 3, 2, 1 | 1, 0, 0 |

A window another window covers is not rendered and may be throttled, and a first
series there measured Legend covered; the probe now frames the scroll view's
window and orders it front without activating it (`SCROLLPROBE_PLACE`), and
records whether it was visible. One traced run under that load had app updates
at four times their untraced length, so the Hitches instrument was not used there.

**Not established:** anything with HID input; responsive
scrolling (an opt-in was tried and cannot be driven from inside the process);
iOS, where the same presenter changes were not made. Sources, raw runs and the
probe are under `target/markdown-comparison/scroll-smoothness-20260919/`.

### Startup and memory — 2026-09-18

Latest startup comparison: **no established user-visible Exact advantage yet**.
The latest external startup series uses Exact `e98722ab…`, including the
empty-container, flattened-row and parser improvements below. Thirty alternating
fresh processes per app and document, warm filesystem caches, 900×700 windows:

After macOS screen-capture approval, launch timing uses a ScreenCaptureKit
stream started before process launch. The observer matches the document body
against a visually verified reference, including dark text pixels, and uses
[WindowServer's frame display timestamp](https://developer.apple.com/documentation/screencapturekit/scstreamframeinfo/displaytime).
A blank window cannot satisfy the match. Thirty fresh processes per app, six
rotating orders, warm filesystem caches, 900×700 outer windows, 27,405-byte
README; no workspace builds owned by this comparison ran during measurement.
Other shared-machine activity is uncontrolled. The display stream requests
120 Hz; this is a WindowServer timestamp, not a photon measurement.

| Launch until reference content is displayed | Median | p95 |
| --- | ---: | ---: |
| Minimal Swift/AppKit Hello World | 158 ms | 193 ms |
| Exact, integrated `origin/main` plus scalar reuse | 240 ms | 286 ms |
| Legend Markdown | 277 ms | 318 ms |

The native baseline is an optimized Swift AppKit executable with one
`NSTextField`, no document loading and no Exact code. Exact's README appears
about 82 ms after that baseline and 37 ms ahead of Legend at the median in
this test. This does not establish a meaningful general startup lead or
already-running file-switch latency. All 90 launches matched; none were
discarded. An earlier attempt stopped at its first launch because an unrelated
Chrome crash dialog visibly occluded the document. That failure is retained
in `origin-integration/scalar-display-startup/`; the dialog was dismissed and
the complete series restarted with unchanged references and thresholds. Raw
runs and binary/reference identity are in
`target/markdown-comparison/origin-integration/scalar-display-startup-v2/`;
the observer is in `probe/display-startup/`. The preceding origin-only binary
measured 150/240/264 ms median for Hello/Exact/Legend in its separate series
(`origin-integration/display-startup/`); do not pool the two series.

A separate paired series opens the 2,242,305-byte corpus of 75 distinct repository
documents, using the same observer and window size: 30 fresh processes per app,
alternating order, all matched. Exact is 255 ms median / 286 ms p95; Legend is
292 / 329 ms. This observes the opening body content, not full-document layout
completion. `origin-integration/scalar-full-display-startup/` retains every run
and the reference/binary hashes. Hello World was measured with the README series,
so do not subtract its timing from this later series.

The preceding paired CGWindow screenshot series recorded first windows at
167/193/195 ms median for Hello World/Exact/Legend, and capture completion at
224/249/293 ms. Its multi-second tails occurred inside capture calls. Those
numbers answer different questions and are superseded for visible startup by
the display-stream series above. The earlier standalone Hello World result
(305 ms median capture completion) was not paired and must not be subtracted
from reader runs made at other times. Its sources and evidence remain in
`probe/hello-world/`; the paired screenshot series is in
`origin-integration/paired-readme-startup/`.

The current scrolling comparison uses a 2,242,305-byte corpus of 75 distinct
repository documents. Three fresh processes per app/build, rotating order,
900×700 outer windows, 1,200 HID wheel events of 30 pixels at 120 events/s for
ten seconds, Hitches instrument only. All nine input intervals are covered
by their target PID's trace. Endpoint OCR confirms substantial nonblank text
movement. These are input rates, not measured display FPS.

| Sustained scroll, 2.24 MB corpus | Median hitch ms/s | Longest stall across runs |
| --- | ---: | ---: |
| Exact, integrated `origin/main` plus scalar reuse | 21.67 | 16.67 ms |
| Exact, integrated `origin/main` before scalar reuse | 59.16 | 33.33 ms |
| Legend Markdown | 3.33 | 8.33 ms |

All runs are retained: scalar Exact 3.33/40.00/21.67 ms/s;
origin-only Exact 55.00/59.16/148.33; Legend 0.83/3.33/4.17. Shared-machine
load rose during the series; no comparison-owned builds were active. The apps
have different reading layouts inside matched outer windows. Three runs do
not establish a general ranking, but this series favors the scalar fix over
the merged baseline and still favors Legend overall.
`origin-integration/scalar-hitches-120hz/{identity,runs,summary}.json` retains
all nine target-PID traces and verified input coverage.

The earlier, separate series established the integration regression:
origin-only Exact 155.00 median hitch ms/s, pre-merge Exact 7.50, Legend 5.83.
Its raw runs remain in `origin-integration/hitches-120hz/`. Active-scroll
profiles put the added work in list settlement and text measurement. Repeated
width offers discarded complete paragraphs and then wrapped the same text
again. The fix retains only their scalar size/baseline under the existing
cache bounds when obsolete paragraphs retire. A regression test verifies
revisited widths hit the scalar cache without retaining obsolete CTLine arrays,
and that font-catalog replacement invalidates those scalars. The fresh scalar
profile shows fewer text-layout samples, but repeated native measurement and
list settlement remain. Profiles are diagnostic samples, not elapsed costs
or display frame rates (`origin-integration/{scroll-profile,scalar-scroll-profile}/`).

Earlier 60-event/s full-corpus runs favored the retained pre-merge Exact:
3.33 vs 18.75 median hitch ms/s, longest stalls 8.33 vs 16.67 ms.
Exact's three runs were 17.50/3.33/2.50; Legend's 9.17/18.75/19.17.
Background builds were running. These results are historical and do not
apply to the merged build. `async-text-full-hitches-{identity,runs,summary}.json`
retains them; raw labels `flat` mean retained Exact and `exact` mean the
rejected asynchronous text experiment. The async experiment's median was
worse (4.17 ms/s), so it was not kept. The earlier 231 KB repeated-document
series also varies by input rate and instrument; do not pool those results.

Current-build memory uses the same 2.24 MB corpus and 900×700 windows, with
three alternating fresh processes per app. Each process makes four forward
and reverse passes over approximately the first 36,000 points of the document,
not its entire length. Each direction lasts five seconds at 120 HID events/s;
reverse travel is slightly longer to reach the beginning. Endpoint OCR verifies
changed body content and return to the first heading. `vmmap` measures process
physical footprint, excluding WindowServer and children. No Instruments or
screen stream runs during this test. Median of three processes:

| Physical footprint | Exact | Legend |
| --- | ---: | ---: |
| After opening | 80.2 MiB | 63.7 MiB |
| After pass 1, two seconds idle | 98.3 MiB | 90.6 MiB |
| After pass 2, two seconds idle | 99.1 MiB | 98.1 MiB |
| After pass 3, two seconds idle | 99.3 MiB | 103.1 MiB |
| After pass 4, two seconds idle | 99.3 MiB | 109.5 MiB |
| After ten more seconds idle | 91.7 MiB | 100.7 MiB |
| Peak during the run | 359.0 MiB | 381.8 MiB |

Exact settles consistently across these four passes. Legend's settled footprint
rises in this short series; that does not establish indefinite growth or a leak.
Both have much larger transient footprints than their idle values. Evidence is
in `origin-integration/scalar-memory-repeated/{identity,runs,summary,endpoint-validation}.json`.
The older `d26b0c81…` results in `corpus-memory-v2-summary.json` measured only
opening and immediate post-scroll memory; they do not describe the current build
or settling behavior.

Wheel input to first visibly changed body frame now uses ScreenCaptureKit's
WindowServer timestamp, from immediately before posting one 120-pixel ordinary
wheel event (no gesture phase). Three alternating fresh processes per app,
30 inputs per process, with the body stable for at least 520 ms before each
input. All 90 inputs per app produced a changed frame; none were discarded.
The first changed crops were visually checked for actual text movement.

| Wheel input to visible response | Median | p95 |
| --- | ---: | ---: |
| Exact | 21.5 ms | 28.5 ms |
| Legend | 21.5 ms | 25.2 ms |

This is synthetic wheel-post to WindowServer presentation, not physical
trackpad-to-photon latency, and does not establish a meaningful response lead.
Each process then receives a ten-second fast-scroll burst at 120 events/s.
No entirely blank body crops were detected among 3,067 observed Exact frames
and 3,320 Legend frames. These are changed/complete captured frames, not display
FPS counts; the ink-presence check cannot rule out partial blanks or stale text.
`origin-integration/scalar-input-display/{identity,runs,summary}.json` retains
frame timestamps, stability checks, misses (none), and images. An earlier
isolated gesture-start pilot produced no response to alternating Legend events;
it was not accepted as a latency comparison, and is retained with the reason
in `origin-integration/input-display-pilot/`. Both apps used the same corrected
ordinary-wheel protocol for the reported series.

Charlie confirmed this metric priority on 2026-09-18:

1. Launch to readable document: median and p95; already-running file changes
   measured separately.
2. Scroll smoothness: hitch milliseconds per second and longest stall.
3. Input to visible response, including blank/stale content during fast motion.
4. Settling memory and growth across repeated scrolling passes.

Internal parse/layout/flush timings diagnose costs; they do not establish
visible frame rate. Partial blank/stale-content detection and already-running
file-switch latency still need reliable comparative measurements.

`origin/main` at `7e77aaf1` adds warm paragraph ink indexing, bounded cache
maintenance, paragraph identities and Apple scalar-measurement reuse. Its
cold AppKit background paragraph path is opt-in for Markdown Stress, not
this reader. The integration is committed as `0d32840c` in
`exact2-wt-markdown-origin`; the current measured binary is `d222c8b0…` from
`8b69fa64`, the origin-only baseline `245d3466…`, and the retained pre-merge
binary `e98722ab…`. During measurement, the shared `origin/main` ref advanced
to `f61ff1c4` with later collection/Messages changes; those are not part of the
frozen build reported here. The combined Mac Release build, 227 core unit tests, targeted parser/kernel/collection/selection/media/refusal/compiler
tests, strict host/compiler Clippy, formatting, caps and boot passed.
Native launch and the agent's full-corpus open now run successfully; an
intermittent launch stall was observed before Swift main, separately from
rendering. The original shared checkout remains recoverable as snapshot
`2c765e80`.

The whole-workspace build found a browser worker dispatch call still expecting
decoded JSON after the upstream byte-transport change. Commit `8137210c`
decodes that response; its three module tests and targeted strict Clippy pass.
Superseded full-workspace checks were stopped at recorded process IDs after
profiling found TypeScript scanning a heavily populated shared temporary
ancestor. With a private temporary directory, the final workspace test run
reports 1,555 passed, four failed and eight ignored: one fixture inherited the
worktree's Cargo workspace because its temporary directory was inside it;
three assertions still treat the newly assigned event tag 18 as unknown. Strict
workspace Clippy, formatting, caps and boot pass. The final workspace build
failed when a Messages bake helper exited without replying; an earlier build
passed. After three validation rounds, these broader repairs are stopped and
recorded in `queue/`, not reported as green. The scalar change separately
passes all 149 Swift host tests and the standard Mac Release bundle build.
`origin-integration/workspace-validation-round{2,3}.json`,
`workspace-round3-test-corrected.json` and `cache-diagnostic/scalar-tests-round1.json`
retain the results. Future private temporary directories must be outside any
Cargo workspace.

The comparison target is [Legend Markdown](https://github.com/LegendApp/legend-apps/tree/2b7b91d949cf873ddef7ea0dde892ddd51944501/apps/markdown),
pinned to `2b7b91d949cf873ddef7ea0dde892ddd51944501`. Its ARM64 Release
build runs locally. The source needed one build-only adjustment: import
`Commands` under its existing public alias, then export that alias, because
the React Native Babel plugin rejected the direct aliased re-export.
The app renders the same local document. It also provides editing; these
experiments concern reading.

Exact's macOS presenter now indexes paragraph rectangles within each native
scroll document. Scrolling queries that index instead of converting every
paragraph's bounds. Layout/structure updates invalidate it; tall overlapping
paragraphs and nested scrollers remain visible. Selection and full-document
copy retain the complete paragraph order. Missing/fixed colors no longer
ask AppKit to resolve an appearance.

A 230,933-byte fixture (six copies of LLP 1033), 1,000×760-point content
viewport, 873 paragraphs and 6,440 logical views was measured with an
optimized Swift probe linked to the same Markdown Rust archive. Three fresh
processes per version, in alternating order, each performed 180 synchronous
scroll/display/Core Animation flush updates, then repeated with all text
selected. These are CPU update costs, **not display-link frame intervals**.
Median of the three per-process statistics, milliseconds:

| Measurement | Before | After |
| --- | ---: | ---: |
| Visible-text lookup, p50 | 0.221 | 0.011 |
| Scroll/display/flush, p50 | 6.65 | 6.79 |
| Scroll/display/flush, p95 | 17.91 | 16.12 |
| Open through content display/flush | 1,047 | 1,012 |

This establishes a faster lookup, not a meaningful overall scrolling or
startup win. The machine had other work running. Both versions copied
223,625 characters. All 46 Apple host tests pass, including the new long-list,
reverse-scroll, overlapping-paragraph and nested-scroll fixtures. Caps and
boot pass. Full workspace verification is separate and remains in progress;
concurrent media changes initially broke the Rust build.

Local evidence and reproducible probe sources are under
`target/markdown-comparison/`: `probe/main.swift`, the before/after binaries
and raw sample JSON lines, `SHA256SUMS`, `identity.json`, the pinned Legend
source/build and its small build patch. `probe/visible.swift` independently
captures each launched app's window and uses OCR to confirm actual document
content, rather than treating the welcome window as startup completion.
Those preliminary screen observations are not yet a controlled comparison.
The updated diagnostic app is `target/markdown-comparison/Exact Markdown.app`;
it uses the last successfully built Rust archive, identified in `SHA256SUMS`.

**Still required:** bound variable-height block construction/layout and native
view lifetime, preserve reading position through measurement and resizing,
then measure both Release apps on the same small/large documents and viewport
with repeated first-content and sustained-scroll traces. No claim that Exact
is faster than Legend follows from this slice.

### Measured row windows — prototype, 2026-09-18

**Superseded 2026-09-27 (LLP 1070 stage 1):** the windowed list this section
describes is deleted; the reader's document is a `virtualized=true` list (the
collection), with the page's top and end space on its first and last rows.

The shared runner now accepts `estimated-item-height` on a list. A compact
prefix-sum index tracks measured rows; offset lookup and individual height
updates are logarithmic. Width changes invalidate measurements. Height changes
and record reorders preserve the reading key and its offset. Row bodies and
native view objects remain limited to the viewport and overscan. The browser
reports DOM row sizes; the Mac presenter reports its actual scrollport and
the Apple host takes row sizes from kernel layout. Ordinary `scroll` stays eager.

An experimental reader plan uses this path. The regular app remains eager
until selection and full-document copy work across unmounted blocks. The
prototype and evidence are in `target/markdown-comparison/`, including
`plans/windowed.plan`, `windowed.contract`, `probe/windowed/main.swift`,
`windowed-probe-summary.json`, raw samples, and `windowed-identity.json`.

The same optimized probe binary ran the eager and windowed plans in three
alternating fresh processes each, with the same 230,933-byte file and
1,000×760 viewport. Each run performed 180 scroll/display/flush updates.
Median of the per-process statistics:

| Measurement | Eager | Windowed prototype |
| --- | ---: | ---: |
| Open through content display/flush | 774 ms | 49 ms |
| Scroll/display/flush, p50 | 7.28 ms | 4.72 ms |
| Scroll/display/flush, p95 | 17.85 ms | 8.58 ms |
| View objects after opening | 6,440 | 147 |
| Peak view objects during traversal | 6,440 | 283 |

These are CPU update costs, not displayed frame intervals; other workspace
work was running. Selection was excluded because it is not yet correct for
unmounted rows. The Mac agent drove the prototype through deep scrolling
and back, with screenshots visually checked. Chrome rendered and scrolled
the prototype's welcome document. The custom browser stress fixture was
stopped after three setup fixes; it does not supply acceptance evidence.

Three alternating screen-observed launches of the prototype and Legend gave
medians of 418 ms and 497 ms respectively. They use the same file and outer
window size, but different reading layouts, warmed filesystem caches, and
only three samples. The prototype uses a development plan. This is promising
evidence, not a verified claim of a noticeably faster finished application.

Eight runner/web list tests and the Apple kernel-measurement test pass;
46 Swift host tests pass. Targeted clippy passes, and macOS Release and web
builds succeed. The full workspace gates are running separately. Remaining:
logical selection/copy, complete native interaction pins, iOS/Linux geometry,
reader adoption, and matched repeated Legend scrolling/first-content trials.

### Default reader adoption and selection — 2026-09-18

The regular reader now uses measured row windows. Keys include the document
path, so opening another file resets its reading position and selection.
AppKit selection retains row keys and UTF-16 endpoints across view retirement.
Copy evaluates the existing row template one unloaded row at a time, without
committing views or changing the live window. The browser uses the same text
projection and paints selection on the mounted paragraphs; its selection
module loads after first paint. UIKit now supplies list geometry too.

The corrected Mac probe explicitly rejects a failed custom-plan boot and
asserts the renderer kind. An earlier selection probe had silently fallen
back to the eager plan after a schema change; its apparent comparison was
invalid. The corrected run opens with 147 views and copies all 223,594
document characters, byte-for-byte equal to the eager reader's document
text. Full copy takes about 9 ms in that diagnostic run and leaves the
window bounded. These are correctness checks, not a fresh Legend comparison.

Chrome's actual reader was resized from all 21 welcome rows to nine mounted
rows, then scrolled until eight remained. Full copy still returns all 871
characters; partial selection also survives retirement of both original
endpoints. Mac deep scrolling and file switching, and simulator scrolling,
were driven through the existing agent and screenshots inspected. The ten
runner/web list tests, 47 Swift host tests, targeted clippy, caps, boot, and
formatting pass. Mac Release, iOS simulator and web builds succeed. The
broader affected-crate run passes 235 Rust tests. Full workspace gates remain
running through slow child-process startup.

Evidence is under `target/markdown-comparison/`: the
`windowed-selection-probe-verified.*` and `adopted-selection-probe.*` results,
`browser-selection.json`, and the `adopted-*` screenshots. The older startup
comparison above remains preliminary. Matched repeated startup, actual
scrolling hitch traces against Legend, and process memory are still required
before claiming the goal is achieved.

### Thirty-launch comparison — 2026-09-18

A faster external screen observer compares the document body against each
app's previously verified screenshot. OCR no longer delays the sampling loop.
Thirty alternating fresh-process launches per app, the same 230,933-byte
file, and verified 900×700 outer windows give median first-content times of
**304 ms for Exact and 301 ms for Legend**. Typical startup is effectively
tied. This supersedes the three-run suggestion of a startup lead above.
Filesystem caches were warm, reading layouts differ, and other workspace and
Instruments analysis work was running. Every trial is retained, including
slow process starts; these are not clean-machine cold-start numbers.

Raw screen samples, matched images, launch order, system load and binary
hashes are in `target/markdown-comparison/startup-reference-runs.json` and
`startup-reference-summary.json`; the observer and orchestration sources are
`probe/visible-reference.swift` and `probe/startup-reference.py`. The first
external scrolling trace was an invalid idle run: process-targeted wheel
events had not moved the page. HID-delivered input was then verified by
changed document content before collecting matched scrolling traces. Their
first 10-second scrolling intervals show 20.4 hitch ms/s for Exact and 18.7
for Legend (19 and 17 hitches, respectively). Both windows were 900×700;
input was 60 pixel-wheel events per second, 60 pixels each. One trace per
app is diagnostic only, and establishes no Exact scrolling advantage.
`scroll-matched-pilot-summary.json` records the interval alignment and raw
trace/export filenames follow `*-scroll-matched-1`.

### Scroll hot path and single boot — 2026-09-18

The Mac adapter no longer boots again after attaching `ExactView` has already
booted the session. Document loading starts before window activation, and the
window appears without an opening animation. Text-cache hits update recency
without hashing the paragraph a second time. Empty list updates skip native
layout/presenter work, and scrolling invalidates newly exposed text strips.

A row-window batching experiment was removed: it improved median synchronous
CPU update cost but worsened p95 and its scrolling-hitch pilot. The retained
paint changes preserve selection/copy and pass 18 screenshot comparisons
against complete redraws, with fewer than 0.002% of color channels differing
by more than two levels. A first capture during the window-opening animation
was invalid because its image dimensions differed; the probe now settles the
window before testing scrolling. The synchronous CPU comparison does not show
an overall gain (5.20 → 5.17 ms median, 8.71 → 9.27 ms p95).

**Benchmark correction:** the earlier external probes filtered out `HOME`.
A diagnostic launch subsequently reported unavailable app-storage activation.
Those runs still showed the document, but are preliminary. A later startup
run also failed its dark reference after the system switched to light mode;
that run was stopped, not counted as performance evidence. Older evidence is
retained alongside `benchmark-environment-correction.json`.

With `HOME` inherited and newly verified reference images, 30 alternating
fresh launches give **237 ms Exact / 259 ms Legend** median, and **348 / 383 ms**
p95, for the same 230,933-byte document and 900×700 windows. All trials are
retained, including Exact's 7.94-second maximum (Legend's is 0.92 seconds).
Caches are warm, reading layouts differ, and other workspace work was running.
This is a modest typical-startup lead, not completion of the performance goal.
See `home-startup-{summary,runs}.json` and the per-launch screenshots.

The latest old-environment scrolling pilots measured 11.2 hitch ms/s for Exact
and 2.5 for Legend; they establish no Exact advantage. Three paired traces per
app with the corrected environment are running from a frozen Exact bundle;
`home-scroll-identity.json` and `home-scroll-runs.json` identify that work.

Validation: 129 runner/web/Apple Rust tests, 49 Swift tests, targeted clippy,
caps, boot and formatting pass. Mac and iOS simulator builds succeed. The Mac
agent drove deep/reverse scrolling and file switching; screenshots were checked.
The existing browser selection drive passes. Full workspace gates remain live
behind the previously identified executable-startup stall. Current local
evidence is under `target/markdown-comparison/`, including `strip-updates-*`,
`strip-paint-pixel-check.json`, `strip-mac-*`, and `home-startup-*`.

### Reusing identical text offers — 2026-09-18

**The `home-scroll-*` comparison is invalid.** Its three Exact intervals record
44.2, 5.8 and 8.3 hitch ms/s, but Legend's zero-hitch intervals were idle. The
movement check compared OCR strings for inequality; it accepted a one-character
recognition difference while Legend still displayed the document's beginning.
The same flaw affects all three Legend `cache-scroll-*` runs. Their zero values
cannot establish a scrolling advantage. `benchmark-scroll-verification-correction.json`
audits all twelve runs; raw evidence is retained. New probes explicitly activate
the target app, require substantial normalized word changes, and inspect body
content. The older missing-HOME pilots did move the document but remain single
diagnostic pairs with the environment limitation described above.

The kernel now keeps at most four exact `(width, height)` text measurements per
live leaf. Taffy's intrinsic layout passes were asking the same paragraph under
identical offers repeatedly. Reusing metrics avoids flattening its runs and
crossing into CoreText again. Text/style/child invalidation clears the cache;
retiring a node releases it. Height remains part of the key for host measurers
that use it. A reader regression fails without this change on duplicated
min-content and intrinsic-width measurements. Mutation/rehydration tests cover
wrapping, inherited fonts, inline edits, deletion/reuse, reparenting and a
height-sensitive measurer.

Three alternating native CPU-probe runs per version reduce callbacks over the
same 180 scroll updates from **60,036 to 30,348**. Opening callbacks fall from
1,762 to 1,184. Median update cost is nearly unchanged (**4.18 → 4.15 ms**),
while p95 improves **7.64 → 7.21 ms**; selected-scroll p95 is **5.72 → 5.31 ms**.
Copies are byte-identical and selection clears when the file changes. These
are synchronous display/flush costs on the same repeated-text fixture, with
concurrent machine work, not physical frame times.
`measurement-cache-{runs,summary}.json` records the CPU comparison.

**Bundle attribution correction:** the Mac rebuilds above omitted `--bundle`.
They updated the standalone executable, but the `.app` copied into the external
probes still contained the earlier binary. Hash inspection found that
`Home-scroll-baseline.app`, `Measurement-cache.app` and
`Deferred-inline-layers.app` all contain SHA-256 `792db0ff…`. The `cache-scroll-*`
trials therefore ran **the earlier baseline**, not the new cache. Their Exact
intervals record 256.7, 16.7 and 16.7 hitch ms/s; Legend was idle as noted above.
The first includes an unexplained 2.50-second stall flagged as potentially
expensive GPU work, with no running-thread samples during that interval.
These results are retained, but cannot support a before/after claim about the
cache. Internal CPU probes linked the rebuilt archive directly and remain valid.
`benchmark-bundle-correction.json` records the hashes and affected evidence.

The combined affected Rust tests pass, including the kernel's incremental/
rehydrated layout equality suite. The first run exceeded the web dev test's
existing 100 ms compile/bake assertion; both the separate web suite and the
complete four-package rerun passed. Targeted
clippy, caps, boot and formatting pass; Mac Release, iOS simulator and web
builds succeed. `measurement-cache-identity.json` identifies sources, binaries
and validation. The older full-workspace job stalled at native helper startup
and was subsequently cancelled to reclaim its build cache after disk exhaustion;
these focused checks do not establish a complete workspace gate.

### Deferring inline layers and broadening the corpus — 2026-09-18

Unattached inline text now retains its presenter identity without a Core
Animation layer. A paragraph gets its layer, mask, z position and transform when
mounted; becoming an inline run releases that layer again. All 50 Swift tests
pass, including restyling and promoting/demoting a run while preserving its
identity and content.

Three alternating native CPU-probe runs per version cover the original repeated
fixture plus fixed copies of a 27 KB README, a 263 KB specification and 964 KB of
five distinct documents. Layers after scrolling drop 252 → 121 on the repeated
fixture and 281 → 171 on the specification/corpus. CPU improvements are small
and mixed: p95 is 7.02 → 6.81 ms (repeated), 5.55 → 5.15 ms (README),
9.75 → 9.86 ms (specification), and 9.19 → 9.39 ms (corpus). Copies match in
every comparison. The small README reaches its end during the fixed scroll
sequence, so its near-zero median is not a scrolling-frame measurement.
`inline-layers-{runs,summary}.json` and `fixtures/corpus/manifest.json` retain
the results and document provenance. These probes link an immutable archive
and the selected Swift sources, independently of the stale `.app` issue above.

The first corpus memory attempt was stopped: it used that stale Exact bundle,
and Legend's README body did not move under the injected wheel input. No paired
memory claim follows. The next run assembles with `--bundle`, rejects the old
binary hash, and explicitly activates the target app before scrolling. The
frozen `Deferred-inline-layers-v2.app` has SHA-256 `d26b0c81…`, matching the
newly assembled app; `inline-layers-identity.json` records sources and build.
Both apps now demonstrably move through the documents, with before/after images
retained and `corpus-memory-v2-movement-audit.json` checking substantial changes.

The corrected memory run completed all 18 trials. Each number below is the
median of three fresh processes, in MiB from `vmmap` physical footprint.
Peak is the process lifetime peak observed after the scroll sequence.

| Document | Opened Exact / Legend | After scroll Exact / Legend | Peak Exact / Legend |
| --- | ---: | ---: | ---: |
| README, 27 KB | 49.4 / 57.9 | 242.4 / 88.3 | 270.3 / 383.5 |
| Specification, 263 KB | 62.8 / 67.1 | 298.9 / 317.3 | 302.8 / 362.6 |
| Distinct-document corpus, 964 KB | 69.8 / 62.3 | 284.4 / 324.5 | 309.6 / 365.1 |

These are process samples, excluding the system compositor and any child
processes. The post-scroll sample includes transient backing allocations and
varies considerably (Legend's README trials range 84.8–255.9 MiB), so it is
not a steady-idle memory claim. Both apps reach the README's end; the long
documents use the same ten-second wheel sequence without a full-document
traversal claim. All 18 trials pass the stronger movement audit, with at least
87.7% normalized word novelty, and representative before/after images were
inspected. Exact has lower measured peaks, but no uniform memory advantage.
`corpus-memory-v2-{runs,summary}.json` contains the per-run values and method.

The same verified bundles completed 30 alternating fresh launches per app and
document (180 launches total), measured from process launch until the body
matches its verified screen reference. Times below are milliseconds.

| Document | Median Exact / Legend | p95 Exact / Legend |
| --- | ---: | ---: |
| README, 27 KB | 237 / 238 | 266 / 271 |
| Specification, 263 KB | 282 / 265 | 392 / 2,057 |
| Distinct-document corpus, 964 KB | 273 / 265 | 2,915 / 3,151 |

Typical startup is effectively tied: median differences are smaller than the
observer's approximately 33 ms sampling interval. Filesystem caches are warm,
layouts differ, and concurrent workspace work prevents treating the long tails
as isolated app costs. All outliers are retained. These results supersede the
older startup comparisons for the current candidate; they establish no
noticeable startup advantage. `verified-corpus-startup-{readme,spec,corpus}-`
`{runs,summary}.json` records the samples, references, fixture and binary hashes.

### Empty container backing stores — 2026-09-18

AppKit containers without background or border paint now use `updateLayer` to
keep their contents empty, avoiding a bitmap drawing pass. Text, images,
canvases, WebKit captures and decorated boxes retain the existing draw path.
Removing decoration also clears the old bitmap; adding it resumes drawing.
All 51 Swift tests pass. Eighteen on-screen document comparisons, including
selection and reverse scrolling, have no color-channel differences above two
levels. All eighteen CPU-probe copies agree with their corresponding baseline.

Three alternating pairs per fixture show median synchronous scroll costs of
3.72 → 3.50 ms (repeated document), 4.37 → 3.95 ms (specification), and
3.99 → 3.90 ms (distinct-document corpus). The corresponding p95 values are
6.58 → 6.10, 10.12 → 9.26, and 9.12 → 9.17 ms. The largest fixture's selected
p95 worsens 6.64 → 7.02 ms, so this is a modest, mixed CPU result, not evidence
of a user-visible scrolling win. `empty-layers-{repeated,spec,corpus}-`
`{runs,summary}.json` and `empty-layers-pixel-check.json` hold the evidence.

The first corrected active-scrolling trace on the preceding `d26b0c81…`
bundle records 10 hitch ms/s and a 25 ms longest hitch. Its export succeeded
after freeing disposable build caches. Legend's next recording failed with
Instruments data-provider errors when the disk filled again; that trace is
invalid. The visible body did move in both apps, unlike the earlier idle trials.
`verified-active-scroll-*` retains these diagnostic attempts. A complete paired
scrolling result remains outstanding.

The reader also drops one redundant centering container from every mounted
block: the existing maximum-width column centers itself with CSS auto margins.
Measured after the empty-container change, three alternating pairs per fixture
reduce median synchronous scroll cost 3.88 → 3.55, 4.10 → 3.73, and
4.13 → 3.86 ms on the repeated/specification/corpus documents respectively.
p95 becomes 6.82 → 6.76, 9.59 → 9.12, and 9.37 → 8.98 ms. Both versions use
the same native probe binary and independently baked plans. Copies agree and
eighteen selected/unselected screen states have no channel differences above
two levels. This is additional internal CPU evidence; the external startup,
memory and hitch results above predate this change. `flat-rows-*` holds raw
results, plan hashes and pixel comparisons.

Mac, iOS simulator and web builds pass, as do caps, boot and formatting. The
browser drive preserves full and partial selections across row retirement and
checks the actual reading column at 420, 1,000 and 1,400 pixels (bounded by
740 pixels and centered). Its first geometry assertion mistakenly measured
the framework's full-width row wrapper; inspecting the DOM identified the
authored column inside it, and the corrected drive passes. Native screenshots
at 420 and 1,400 points confirm wrapping and centering; the narrow window's
existing cramped folder/header layout is recorded in `queue/`.
`flat-rows-browser-selection.json` and `flat-rows-mac-{narrow,wide}.png` retain
the drive. The assembled `Flat-reader-layers.app` has SHA-256 `6e85cc90…`;
`flat-reader-identity.json` records sources and build. The `flat-active-scroll-*`
series uses this frozen bundle and refuses to start Instruments below 12 GiB
free disk space. Five of six recordings completed with verified document
movement: Exact's three ten-second runs measure 11.67, 6.67 and 6.67 hitch
ms/s, with a longest hitch of 25 ms in each; Legend's two runs measure 5.00
and 3.33 ms/s, with a longest hitch of 16.67 ms in each. The disk-space guard
stopped the final Legend run before recording. These are provisional results
on the repeated 231 KB fixture, with the same 900×700 window and injected
wheel sequence; they favor Legend, not Exact. The startup and memory results
above predate the empty-container and flattened-row changes.

An AppKit responsive-scrolling opt-in was tested and removed. Overriding
`scrollWheel` makes the subclass report incompatibility; opting in enabled
the concurrent scrolling thread, confirmed by a live process sample. The
frozen experiment is `Responsive-scroll.app` (`a1584ca4…`), with 51 Swift
tests passing. Fifty rapid forward/reverse screenshots per build return to
the same document beginning; 34 paired samples are identical. Six real HID
nested-pane scenarios per build preserve manual movement, edge chaining and
containment. These samples do not prove every display frame.

Two further series use only Instruments' Hitches instrument, avoiding the
heavy template's CPU profiling and multi-gigabyte temporary kernel recording.
Each has three verified ten-second runs per build, with all results retained:

| Input | Retained Exact median | Opt-in median | Legend median |
| --- | ---: | ---: | ---: |
| 60 Hz, 60 px/event | 2.50 ms/s | 1.67 ms/s | 7.50 ms/s |
| 120 Hz, 30 px/event | 1.67 ms/s | 1.67 ms/s | 0.83 ms/s |

The opt-in's first 60 Hz run has a 75 ms hitch (36.25 ms/s overall), while
Legend's last has none. At 120 Hz every build's worst hitch is 8.33 ms. This
small, variable sample establishes neither an opt-in benefit nor a noticeable
Exact advantage; do not combine these results with the heavier template's.
The third 60 Hz traces have 675/662/939 app update submissions for retained
Exact/opt-in/Legend, respectively; these are not direct display FPS counts.
`responsive-hitches-{only,120hz}-{runs,summary}.json`,
`responsive-frame-update-audit.json`, `responsive-nested-*-validation.json`
and `responsive-quality-comparison.json` retain the evidence. A 16.4 GB
temporary kernel recording belonging to the completed earlier Exact trace
was removed after checking its process had exited; the saved trace and exports
remain (`completed-trace-cache-cleanup.json`).

A temporary `NSWindow(defer: true)` startup experiment also showed no benefit:
six alternating launches per variant measured 287 ms median before and 292 ms
after, below the approximately 33 ms observer resolution. Both variants use
the same linked objects and SDK metadata; their SDK 27 native chrome differs
from the frozen app's SDK 14 chrome, so this is an internal comparison only.
An initial mismatched-reference trial was rejected by the pixel check and
retained as invalid. No window-creation change was adopted.
`matched-window-startup-{runs,summary}.json` records the comparison.

A further temporary plan merged heading, paragraph, code and raw-HTML
wrappers into their text nodes. On the repeated document, median synchronous
scroll CPU cost improved 3.29 → 2.91 ms; on the specification and distinct
corpus it changed only 3.52 → 3.49 and 3.51 → 3.48 ms. The corpus's selected
p95 worsened 6.44 → 6.62 ms, and neither real-content fixture opened faster.
Copies agree; twelve of eighteen screen states are identical and six have
small raster differences (at most 0.0026% of channels differ by more than two
levels). No production change was adopted from this experiment. The results
are `compact-blocks-{repeated,spec,corpus}-{runs,summary}.json` and
`compact-blocks-pixel-check.json` under the same local evidence directory.

### Parser work and document switching — 2026-09-18

A sample profile of repeated native document switches found redundant UTF-8
validation and horizontal-rule detection among the parser's largest costs.
The inline parser now borrows its already-valid string at character boundaries
instead of validating the remaining suffix at each possible bare URL. Rule
detection scans marker bytes once, stops on the first mismatch, and allocates
no stripped copies. Markdown behavior is unchanged: complete parsed-output
digests agree for 75 repository documents and 17,307 generated Unicode,
delimiter and rule cases. All 14 parser tests and targeted parser/data clippy pass.

Three alternating process pairs, 31 parses per document per process, reduce
median parser time 0.346 → 0.161 ms (README), 3.76 → 1.83 ms (specification),
and 12.43 → 6.10 ms (distinct-document corpus). These timings exclude I/O
and UI. A matched native probe linked the same Swift object against old/new
production Rust archives and repeatedly switched the two larger documents:
median update-through-display/flush time improves 26.47 → 23.63 ms and
36.48 → 28.36 ms respectively, over three fresh process pairs. This is a
native update-cost improvement, not an external presentation-latency or
Legend comparison. `probe/parser/{bench-summary,corpus-equality}.json` and
`probe/file-switch/matched-{runs,summary}.json` retain the evidence.

Mac, iOS simulator and web builds pass. Chrome preserves full and partial
selection across row retirement, centers the column at 420/1,000/1,400 pixels,
and reports no exceptions (`parser-browser-selection.json`). Caps, boot and
formatting pass. The assembled Mac app is frozen as
`Parser-candidate.app` (`e98722ab…`); `probe/parser/app-identities.json`
records both builds and archives. The 180 new screen-observed launches are
summarized at the top of this section. They still establish no noticeable
startup lead and retain every slow trial. Whole-workspace validation is
being rerun separately; the earlier stalled run did not pass.

### Rejected follow-up experiments — 2026-09-18

Explicitly finishing AppKit launch before ordering the window improved one
2.24 MB trial series, but a second 20-pair README series was effectively tied:
225.74 vs 223.90 ms median and 245.63 vs 245.81 ms p95 readable captures.
It delayed the first window, so that experimental call is not retained.
Twelve lifecycle smoke runs passed; the failed performance replication is
in `probe/finish-launch/{readme-summary,decision}.json`.

Sharing Markdown runtime-value constants saved only about 0.53 ms of conversion
on the full corpus and was not adopted. Explicit 8-bit text backing had no
benefit because both modes already used RGBA8. Neither experiment changes
production. A seven-second active-scroll profile on the full corpus verifies
coverage within the ten-second input interval and still shows repeated native
list settlement and Core Animation drawing work; inclusive profile counts
are not independent elapsed costs.

The retained source passed `cargo build --workspace` in 42m10s. The subsequent
workspace tests/Clippy were still running when upstream integration began;
this is not a completed whole-workspace validation of the merged source.

### Textflow and Taffy integration — 2026-09-19

The next validation baseline integrates upstream `b3d12c3e`: text exclusions,
Taffy 0.14, and deadline-based app timers. Ordinary Markdown retains its bounded
CADisplayLink list pump, two raster workers, and sixteen leaf-measure offers.
Flowed paragraphs use fragment-aware native painting; changing or clearing
exclusions retires ordinary raster results. The shared tokenizer keeps its
bounded single-space reset, and the clean published Ibex dependency keeps
`ureq` 3.4.0. Transform replies preserve timer deadlines.

The `06149e8d` pilot above remains historical evidence (four valid matched
pairs, 3.75 versus 5.00 ms/s). This layout-engine integration needs fresh
validation and displayed-performance measurements before it supports a claim
about the new candidate.

The fixed ten-pair 120 Hz confirmation of `0a1a6011` did not establish a lead:
all twenty trials passed the input/focus/content screen, but Exact's median
hitch time was 7.50 ms/s versus Legend's 5.83, and means were 10.08 versus 6.50.
The paired mean difference was +3.58 ms/s (bootstrap 95% interval −0.50 to
+8.50); two Exact trials also had reduced presentation cadence. Every trial
is retained in `smooth/published-0a1a6011-confirm-120hz`. Native 254 tests,
three Rust textflow tests, scroll/resize/file/copy controls and sampled rapid
scroll captures passed; this is not a completed workspace validation.

A six-pair frozen `06149e8d`/`0a1a6011` diagnostic did not isolate a clear
integration regression. Active-scroll profiles show native layer commit work
while publishing prepared text rasters. The next experiment groups publications
within one refresh-local transaction, opened only before a potentially
publishing ensure. It preserves each accepted surface and urgent readiness;
its performance is unproven until separately measured.

The refresh-local transaction experiment (`7ad989a9`) is not retained. Its
six-round, three-app pilot had all eighteen trials valid. Candidate mean/median
hitch time was 5.83/5.83 ms/s, base 8.12/4.58, and Legend 5.56/3.75; candidate
won two of six pairs against each. Its final trial also fell to 86.8 target
presentations/s. The lower mean against base depended heavily on one bad base
trial; it did not establish an improvement. Native 254 tests, reader controls,
light/dark/system appearance and sampled rapid-scroll captures passed.
`smooth/publication-7ad989a9-pilot-120hz` retains all results.

The next isolated experiment removes explicit text-publication commits and
lets the surface writes join AppKit's current transaction. It saves and restores
the caller's disabled-action setting around the synchronous assignments. No
worker, admission, selection or raster-identity behavior changes; same-frame
urgency and displayed hitches still require measurement, not an inference from
moving commit work out of the publication stack.

The implicit-transaction variant (`f8817516`) is also not retained. Five valid
matched pairs gave candidate/base/Legend means of 4.00/3.67/3.17 ms/s and
medians of 4.17/3.33/2.50. The first base and candidate trials failed the preset
input-timing screen; all six observed pairs instead gave means of
5.00/3.89/3.06. All raw trials remain in `smooth/implicit-f8817516-pilot-120hz`.
Native 254 tests, reader/appearance controls and sampled rapid scroll passed,
but moving publication into the implicit transaction did not earn its place.

The next bounded experiment raises only the native pump's offscreen admission
allowance from two rows to three. Visible and pinned rows remain exempt; the
window, urgency, list/text alternation and two-worker cap are unchanged. The
allowance also affects bounded retirement, so fewer reports alone would not
establish a win. Diagnostic traces must first show reduced list/apply frequency
without worse tails before another displayed-performance comparison is useful.

The three-row allowance (`7832d926`) is not retained. Four alternating diagnostic
traces recorded 247/245 baseline list calls and 242/241 candidate calls—about a
2% reduction, without lower total list-pump cost. Candidate 1 and baseline 2
failed the input screen; all four passed focus/content checks and remain in
`smooth/tails/budget3-*`. This is a rejected mechanism test, not a physical
smoothness comparison. Native 254 tests passed.

A narrower refill-threshold combination is worth measuring: keep the two-row
budget and list/text fairness, but schedule background refill after one quarter
of overscan is used (`.soon` at .75 viewport). The previous `.75` rejection
predates bounded admission: `2d8ba879` called unbudgeted `syncLists()` and could
starve text work. The current continuation can spread the accumulated missing
rows over bounded slices. The `.35` urgent threshold remains unchanged; the
shorter runway still risks urgent bursts, especially on short rows and fast
reversals. Normal/double-speed and dense-row controls plus diagnostic tails and
physical timing must decide whether this combination earns its place.

The delayed-refill variant (`ac5385d0`) is not retained. All four alternating
diagnostic traces passed the input/focus/content screens, but candidate reports
were 239/239 versus 247/247 for baseline, only 3.2% fewer. Total list-pump time
was 343/369 ms versus 354/354 ms, with no consistent tail reduction. Decoded
apply operation totals were almost unchanged (21,446 versus 21,452): delaying
the trigger made more consecutive bounded continuations, not substantially less
work. Native 254 tests, seven Rust list tests, reader and appearance controls,
and eight sampled baseline/candidate forward/reversal captures passed, including
32-point rows. No physical-hitch pilot was run for this rejected mechanism test.
Evidence remains in `smooth/tails/bounded-refill-*` and
`smooth/bounded-refill-reversal-ac5385d0`; the one-viewport refill threshold returns.

The next isolated candidate restricts navigation's disabled/editable property
lookups to nodes with native controls. Scroll profiles attribute part of every
batch's navigation work to these lookups on ordinary text and wrapper nodes.
Ancestor inert/hidden checks, unconditional AX restoration, focus clearing and
actual native editor-state comparisons remain. Existing availability tests cover
external native-state changes; a displayed-performance benefit is unproven.

The guarded-control candidate (`c7f8ee22`) is not retained. Its four alternating
diagnostic traces all passed input/focus/content checks and carried the same
21,452 apply operations. Baseline apply totals were 157/166 ms and candidate
173/159 ms; list-pump tails also showed no consistent improvement. Assembly
confirmed that the guards skipped dictionary lookups on ordinary nodes, but
that did not establish lower measured cost. Native 254 tests, reader/appearance
controls and normal/double-speed sampled captures passed. No physical-hitch
pilot was run; evidence remains in `smooth/tails/control-lookup-*`.

Older completed `diag1`, `diag2`, `diag4`–`diag6`, `flat`, `lean`, `single` and
`extent` runs are now compressed under `smooth/archives/`. Each archive's files
were SHA-256 verified before deleting the uncompressed copy;
`verified-archives-20260919.json` records the original paths and hashes. A full
workspace validation at `799bf1ba` passed build and the separate 14 web tests,
but test compilation exhausted usable disk headroom and was interrupted. It is not a
verified gate. The task-owned debug cache was removed; subsequent workspace
checks disable debug symbols and incremental caching to bound disk consumption.

The subsequent `08941a89` gate passed lint, caps and boot but failed build/test
while launching the mutable filesystem-helper output (ENOENT and an exit before
reply). The tooling bridge now serializes compilation plus executable capture,
launches immutable digest-named copies, removes inherited Clippy settings and
reports helper exit status/signal without retrying operations. The existing
tooling suite's nine tests pass; restoring the mutable launch path fails the new
rebuild control. Full workspace verification must run again on the fixed commit.

At `ef56fda7`, build, lint, caps and boot passed. The workspace test run reached
completion with one failure in the newly added pan ABI fixture: it expected a
layout root's `left` inset to move the root, which Taffy places at the origin.
The fixture now puts the relatively positioned pan target inside a container
and checks the carried state as well as its frame; the focused test passes.
Upstream `3ddd059e` is integrated, including owned mouse-release delivery across
AppKit event wrappers. Its queue tests and the full integrated gate remain to run.

All five workspace gates pass at `0ca9fdfb` with clean published dependencies.
The subsequent upstream collection-ownership fix (`f8bb0cc2`) is adapted to the
retained bounded scheduler: common-owned lists cannot report through the legacy
path, retain its pending work, or schedule its coverage checks. Ownership is
rechecked after a synchronous report callback. Existing legacy scheduling tests
now use noncollection fixtures; a takeover regression covers both callback and
between-report transitions. This integration still requires native and workspace
validation and does not establish a displayed-performance improvement.

The prepend change at `7442359c` is retained. A new leading child mounts directly
before retained siblings in an Exact-only container: the native regression falls
from four add callbacks and three removals to one add and no removals. Mixed
native decorations retain their ordering. All 263 native tests and the reader's
forward/reverse, resize, selection/copy and file-switch controls pass.

Four physical reversal diagnostics (`smooth/tails/prepend-reversal-*`, baseline
`56add02e`, candidate `7442359c`, ABBA order) all pass input/focus/content checks:
six seconds forward, four reverse, 1,200 events at 120 Hz. In the reverse phase,
each run applies 7,087 operations in 74 batches. Presenter time is 169.3/166.0 ms
for baseline and 42.5/49.8 ms for candidate; enclosing list-pump time is
206.7/202.5 and 79.2/91.5 ms respectively (nested times are not additive).
Eight sampled reversal controls on the corpus and dense rows, at normal and
double speed, all pass input/focus/content checks; none shows an inkless band
over 250 pt (`smooth/prepend-blank-7442359c/`). The captures sample about 20 fps,
not every display frame. These establish reduced reverse-scroll presenter work,
not a fixed-protocol displayed-hitch win over Legend. The previous corrected
120 Hz confirmation still fails that comparison.

At integrated `e84ad146`, all five workspace gates pass with clean published
Ibex inputs, alongside 263 native tests and the reader/appearance controls.
The fixed six-round forward pilot (`smooth/integrated-e84ad146-pilot-120hz/`)
retains all 18 trials: all pass input/focus/content checks. Mean hitch ms/s is
4.861 for both prior `56add02e` and integrated `e84ad146`, versus 2.361 for Legend;
medians are 5.416, 4.583 and 1.667 respectively. The integrated build beats Legend
in one of six rounds. The 114.0 FPS baseline and 105.9 FPS Legend observations
remain included. No confirmation is triggered: the retained reverse-scroll
presenter improvement does not establish lower forward displayed hitches.
