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
the field at the top, `mdview` from a shell, or by following a link inside a
document that names a file beside it. All of those are the same event: a value
arriving at the `open-file` node. A second `mdview` hands its file to the copy
already running rather than starting another.

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
- `data/` (`markdown-data`) — the data source: `open(path)`. Reads *and*
  parses on the host's worker through a continuation, so neither is on the
  thread that lays out (LLP 1016 D1). It owns the open document, which is why
  a refusal does not lose it.
- `app.contract` — the whole view. A document is a flat list of blocks
  (Contract inlines components syntactically and cannot recurse), and a
  paragraph is a `text` node whose children are its runs.
- `app.json` — `file_handlers` says what this opens; the macOS bake derives
  `CFBundleDocumentTypes` from it. `app.command` is `mdview`.

A browser has no filesystem to open, so on the web the reader shows its
welcome document and says so for anything else.

## Performance work in progress — 2026-09-18

### Scrolling against Legend — 2026-09-19

**Scrolling now measures ahead of Legend on the one machine and method
available to this pass; nothing here is a 120 Hz or a real-input result.** An
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
   are the one AppKit wait described below. A paint costs 2.1 to 2.6 times as
   much at two pixels a point, which these 1× displays do not show.

Zero refreshes showed uncovered space in any run, including a reversal, a
250,000-point jump and 48,000 points a second. A jump still builds the rows it
lands on synchronously (one stall of 30–55 ms).

**Not established:** anything at 120 Hz; anything with HID input; responsive
scrolling (an opt-in was tried and cannot be driven from inside the process);
iOS, where the same presenter changes were not made. One 20–30 ms main-thread
wait remains about sixteen seconds after launch — AppKit's first persistent-state
flush, asking Launch Services about the app — and four attempts at it changed
nothing; Legend does not show it. Sources, raw runs and the probe are under
`target/markdown-comparison/scroll-smoothness-20260919/`.

### Startup and memory — 2026-09-18

Latest startup comparison: **no established user-visible Exact advantage yet**.
The latest external startup series uses Exact `e98722ab…`, including the
empty-container, flattened-row and parser improvements below. Thirty alternating
fresh processes per app and document, warm filesystem caches, 900×700 windows:

| Launch to readable document | Exact median / p95 | Legend median / p95 |
| --- | ---: | ---: |
| 27 KB README | 224 / 280 ms | 249 / 1,686 ms |
| 263 KB specification | 275 / 3,862 ms | 282 / 319 ms |
| 964 KB distinct-document corpus | 271 / 4,040 ms | 268 / 295 ms |

Typical startup is effectively tied within the observer's timing uncertainty.
The long tails are real observations, but they are capture-completion times,
not precise app rendering durations. A later 40-launch diagnostic found one
Exact window at 170 ms followed by a screenshot call that took 1,931 ms;
the returned image contained the document. That does not locate readable
presentation within that interval or explain every earlier outlier. Capture
calls typically took around 55 ms in that diagnostic, in addition to polling.
Other work was running; these are not isolated-machine or cold-cache trials.
The raw large-document tails are worse for Exact, but neither app's p95 is
currently a reliable measure of its rendering tail.
`parser-startup-{readme,spec,corpus}-summary.json` contains the results;
corresponding `runs.json` files retain every launch. Attribution evidence is
in `startup-phase-diagnostic-{runs,summary,method}.json`.

The latest scrolling series uses retained Exact `6e85cc90…`, three ten-second
runs per app on the same 231 KB repeated document. With the Hitches-only
instrument, median hitch time is 2.50 vs 7.50 ms/s at 60 input events/second,
and 1.67 vs 0.83 ms/s at 120 input events/second (Exact vs Legend). These are
input rates, not measured display FPS. The heavier instrument series favored
Legend. The small, variable trials do not establish a consistent winner.

Last complete process-footprint measurements also use `d26b0c81…`. On the
263 KB specification, Exact/Legend use 63/67 MiB after opening and 299/317 MiB
after scrolling. On the README, post-scroll footprint is worse for Exact:
242/88 MiB. This excludes WindowServer and children; memory is a mixed result.
`corpus-memory-v2-summary.json` contains all three documents.

Use launch-to-readable-content median/p95 and scroll hitch ms/s plus longest
stall as the headline metrics. Track file-switch latency separately from
process startup, and check that fast scrolling never reveals blank or stale
content. Memory after sustained traversal and repeated passes is secondary.
Input-to-visible-motion latency remains unmeasured. Internal parse, layout
and synchronous flush timings diagnose costs; they do not establish visible
frame rate or a competitor win. Future comparison runs should use the same
latest frozen builds across metrics, realistic distinct content, matched
viewports and repeated input, with background build work stopped.

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
existing cramped folder/header layout is recorded in `QUEUE.md`.
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
