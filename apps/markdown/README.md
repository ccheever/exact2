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

Latest comparison: **typical startup remains tied; the larger-document scroll result favors Exact but needs replication**.
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

The latest Hitches-only scrolling series uses retained Exact `e98722ab…`
and a 2,242,305-byte corpus of 75 distinct repository documents. Three fresh
processes per app, 900×700 outer windows, 600 HID wheel events at 60 events/s
for ten seconds; all input intervals are covered by their trace. Median of
the per-run hitch totals, and longest stall observed across the three runs:

| Sustained scroll, 2.24 MB corpus | Exact | Legend |
| --- | ---: | ---: |
| Hitch milliseconds / second | 3.33 | 18.75 |
| Longest individual stall | 8.33 ms | 16.67 ms |

All outliers are retained: Exact's three hitch totals are 17.50, 3.33 and
2.50 ms/s; Legend's are 9.17, 18.75 and 19.17 ms/s. Before/after captures
confirm substantial body movement. The apps have different reading layouts
inside their matched outer windows, and background builds were running.
These small trials suggest a lead on this workload, not an overall win.
`async-text-full-hitches-{identity,runs,summary}.json` preserves the evidence;
raw labels `flat` mean retained Exact and `exact` mean the rejected async-text
experiment. The summary uses unambiguous `retained`, `async` and `legend` keys.
Asynchronous paragraph drawing had a worse median (4.17 ms/s) and is not kept.
The higher-input-rate comparison is deferred until integration of the newly
fetched text changes from `origin/main` is validated.

The earlier 231 KB repeated-document series used Exact `6e85cc90…`.
Its Hitches-only medians were 2.50 vs 7.50 ms/s at 60 input events/second,
and 1.67 vs 0.83 ms/s at 120 input events/second (Exact vs Legend). Longest
stalls were respectively 16.67 vs 8.33 ms, and 8.33 ms for both apps. These
are input rates, not measured display FPS. The heavier instrument series
favored Legend; do not pool these different fixtures and instruments.

Last complete process-footprint measurements also use `d26b0c81…`. On the
263 KB specification, Exact/Legend use 63/67 MiB after opening and 299/317 MiB
after scrolling. On the README, post-scroll footprint is worse for Exact:
242/88 MiB. This excludes WindowServer and children; memory is a mixed result.
`corpus-memory-v2-summary.json` contains all three documents.

Charlie confirmed this metric priority on 2026-09-18:

1. Launch to a readable document: median and p95. Measure opening another
   file in an already-running process separately.
2. Scroll smoothness: total hitch milliseconds per second and longest stall.
3. Input to visible response: scroll response plus blank or stale content
   during fast movement. This still lacks a reliable measurement.
4. Memory after sustained scrolling: settling footprint and growth over
   repeated passes, not just one peak or a single post-scroll sample.

Internal parse, layout and synchronous flush timings diagnose costs; they do
not establish visible frame rate or a competitor win. Future comparison runs
should use the same latest frozen builds across metrics, realistic distinct
content, matched viewports and repeated input, with background build work
stopped. Screenshot completion currently obscures startup p95, so it remains
an observation with timing uncertainty rather than a precise rendering tail.

`origin/main` at `7e77aaf1` adds warm paragraph ink indexing, bounded cache
maintenance, paragraph identities and Apple scalar-measurement reuse. Its
cold AppKit background paragraph path is opt-in for Markdown Stress and is
not automatically enabled in this reader. The integration is committed as `0d32840c` in
`exact2-wt-markdown-origin`; the measurements above still identify the previous
frozen binaries. The combined Mac Release build, 227 core
unit tests, targeted parser/kernel/collection/selection/media/refusal/compiler
tests, strict host/compiler Clippy, formatting, caps and boot pass. Physical
launch validation is still pending: the driver was sampled blocked inside
macOS `posix_spawn`, before an app PID or ready reply. The original shared checkout remains
recoverable as snapshot `2c765e80`. The whole-workspace build subsequently
found a browser worker dispatch call still expecting decoded JSON after the
upstream byte-transport change. Commit `8137210c` decodes that response; the
three browser-module tests, targeted strict Clippy, formatting, caps and boot
pass. Whole-workspace build/test/lint completion remains pending.

A minimal optimized Swift/AppKit Hello World app was also measured after
Charlie asked for the native baseline: one `NSTextField`, no document loading
or Exact code, and a 900×700 outer window. Thirty fresh processes with warm
filesystem caches produced 233 ms median / 259 ms p95 until the window was
observed, and 305 ms median / 330 ms p95 until a screenshot matched the label.
The capture call itself took a median 68 ms. The reference was visually
checked, and the observer requires dark text pixels as well as the body match
so a blank window cannot pass. These are capture observations, not exact
presentation timestamps. Both Exact and Legend stalled in fresh pilot launch
calls during this run, so the Hello World numbers are **not a paired overhead
comparison** with the earlier 224–275 / 249–282 ms reader medians. Machine
activity and the different run time prevent subtracting them or claiming
Exact is faster than Hello World. `probe/hello-world/{summary,runs}.json`,
its Swift sources, build commands and verified reference retain the evidence.

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
