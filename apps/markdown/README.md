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

## Smoothness continuation — 2026-09-19

The overnight `lane/markdown-smooth` work is preserved and integrated with
`origin/main` at `f61ff1c4`; the lower-hitch goal remains unproven. LLP 1044 is
the inherited investigation, not a new design authority. Retained work increases
measurement memo capacity, settles measured rows within one native call, defers
retirement-only passes, permits AppKit responsive scrolling, and schedules list
fill and text preparation in display-link slices.

Independent source review found that a distant pinned row could conceal an
unmounted gap from the new coverage fast path. Discontiguous mounted positions
now force a viewport report; the native regression fixture scrolls into that gap.
Offscreen text rasterization now admits at most two outstanding jobs without a
backlog, retries deferred paragraphs, and does not retain retired NodeViews.

A fresh 120 Hz full-corpus baseline retained Exact's 25.00 and 0.00 hitch ms/s
against Legend's 4.17, 2.50 and 5.83; the third Exact trace ended before input and
was rejected. This is too variable and incomplete to establish superiority.
A shared AppKit backing experiment was removed: its uncontested repeat measured
4.17 in one completed run versus 2.50/1.67 with ordinary backing; another shared
backing run failed the document-movement check. Earlier diagnostic runs overlapped
window inspection and are explicitly unsuitable for performance claims.
All raw runs, failures, binary hashes and observer notes remain under
`target/markdown-comparison/smooth/resume-*`. The observer now rejects a trial
if another application takes the foreground during input. No final comparison
of the integrated, bounded candidate is claimed yet.

## Performance work in progress — 2026-09-18

Latest comparison: **Exact opens this README ahead of Legend, but Legend still
scrolls more smoothly on the full corpus**. Reusing scalar measurements repairs
part of the scrolling regression introduced by integrating `origin/main`. The
speed goal is not achieved.

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
recorded in `QUEUE.md`, not reported as green. The scalar change separately
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
