# Markdown stress

A third LLP 1041 consumer: generated Markdown goes through the existing
`markdown-parse` crate, its shared value conversion and the shipped reader's
Contract `Blocks`/`Runs` components. No browser-only renderer or private layout
engine. The source and results are identical on web, macOS and Linux.

## Workloads

Choose a source budget of 16 KiB, 256 KiB, 1 MiB or 4 MiB, then:

- **Mixed:** long prose with inline styles, fenced code, tables, quotes and lists.
- **One long paragraph:** one heading and one unsplit paragraph, including UTF-8.
- **One code block:** one heading and one unsplit fenced block with long lines.
- **Giant table:** four columns and as many complete rows as fit.
- **Many blocks:** thousands of individually keyed headings and short paragraphs.

Generation is deterministic. Complete syntax is emitted up to the selected byte
budget; the app reports actual source bytes, parsed blocks, supplied blocks, table
rows and the largest block's text bytes. These are not process-memory figures.
The 4 MiB ceiling matches the existing reader's file admission limit.

The default is a **manual page of 40 blocks**. It still generates and parses the
entire document and retains that parsed document; it is not virtualization.
One 4 MiB paragraph remains one 4 MiB paragraph even in page mode. **Render ALL
blocks** explicitly mounts the complete document and can stall the current UI.
**Window full document** supplies the same complete parsed block list to the
shared viewport collection. It mounts nearby blocks while keeping their real
styled text and table cells. All parsed data remains resident; a giant paragraph
or code block stays giant. The mode does not split, truncate or page its text.

The 640/1200 px column controls reflow the same document without reparsing.
Also drag the real window edge, scroll partway through the document and type in
the persistent input. The echo is bound to Contract state and capped at 512
Unicode characters. Neither width changes nor typing depend on the document
resource arguments. Paging reuses the parsed fixture; changing profile, size or
revision reparses it. **Parse again** requests one full reparse; the timed control
requests 20 reparses at 1 Hz, capped at revision 1,000 until Reset. Pause cannot
interrupt work already running.

Manual and eager modes remain stress controls beside the windowed path. This
fixture does not establish 120 FPS. Web and ordinary Linux retain the synchronous
synthetic source. The Apple runtime and explicit Linux trial use the native
continuation; converting returned values and ordinary layout still run on the UI
owner. Bake retains the original synchronous source.
The production Markdown reader already reads and parses native files on its continuation worker. Do not present
its parse latency as a regression in that reader's existing file-open worker.

## Run

From the repository root:

```sh
export EXACT_UPDATE_TRUST=development
EXACT_WEB_DIST=target/markdown-stress-dist bun host/web/build.mjs markdown-stress-web
bun host/web/serve.mjs --origin target/markdown-stress-dist --loopback 4321

bun host/apple/build.mjs markdown-stress-apple --run

cargo build --release -p markdown-stress-linux
EXACT_AGENT=1 EXACT_PAINTER=cpu target/release/markdown-stress-linux
```

The web dev loop is `bun host/web/dev.mjs --app markdown-stress`.
The Linux executable uses the existing headless/DRM host; a headless screenshot
does not exercise a desktop compositor or prove physical-display refresh rate.

### Explicit Linux cold-paragraph trial

The same executable can select a complete cold paragraph before its first layout:

```sh
EXACT_PAINTER=cpu target/release/markdown-stress-linux --content-region=1048576
EXACT_PAINTER=cpu target/release/markdown-stress-linux --content-region=4194304
```

These are nominal source budgets, with the unchanged complete-chunk generator.
Bare, unknown or duplicate region arguments refuse before boot. The trial uses
an explicitly contained 400px content area with controls outside it. Generation
and parsing use the ordered native continuation; a separate bounded text worker
prepares fonts, shape, width layout and the CPU ink index. Contract and layout
publication remain on the UI owner. The initial loading view is real authored
content, and acceptance requires the complete requested paragraph.

Width reflow retains the last successfully painted document. Later source
reparses can display “Generating…” instead; keeping that document visible through
source parsing remains unfinished. Mixed/code remains outside this paragraph
trial. The ordinary synchronous control remains available; a pinned font-admission
repair now rejects zero-unit fonts that previously produced nonfinite widths and
a paint-time overflow. CPU painting and one fixed DPR are required; changing DPR or
unsupported transforms refuses the trial. Read-only source/link metadata is
retained, but general selection and link activation are not implemented here.

Mac-hosted Rust tests exercise full 1MiB/4MiB cold publication with zero giant UI
measurement calls. The fixed actual Linux 1MiB trial on captured d6b3431 sources
passes: autonomous publication without another input, exact independent-reference
content pixels, continued typing and wheel scrolling 0→40px. An overlapping
472,320-byte RGB strip moves by exactly 40px; newly exposed content is outside
that comparison. UI giant shape/index counters remain zero. This instrumented
CPU/VKMS run is functional evidence, not physical-display or latency evidence.

The earlier d6 4MiB candidate aborts before publication at the 2.5GiB address-space
cap; that failure remains recorded in LLP1041 §8.27. With shared width layouts
on captured ef12f06 sources, both 1MiB and 4MiB trials now complete under the same
cap. Two distinct measurement requests reuse one worker layout and ink index.
The complete 4,194,149-byte paragraph publishes without another input, accepts
further typing and scrolls 40px. Accepted content pixels match the fresh
independent reference; the overlapping scroll strip matches exactly too.

The 4MiB run samples VmPeak at 2,303,612KiB and RSS/HWM at 1,384,404KiB.
These are observations from this fixture, not a general memory bound or an
isolated allocation-site measurement. The source, fonts and cap were not reduced.
Continuous resize, repeated latency measurements and physical120Hz remain
outstanding. LLP1041 §8.29 records the native sharing replay and its limits.

Earlier failed scroll-metadata and prelaunch port cells remain unchanged in §8.22.
The scroll repair keeps limits tied to the successfully painted document; the old
zero metadata did not prove immobile pixels. The display loop watches completion
readiness; the stdio agent only pumps on commands. Neither worker queue bounds nor
source/index limits imply a total-memory or 120Hz claim.

### Explicit AppKit cold-paragraph trial

```sh
EXACT_CONTENT_REGION=1048576 bun host/apple/build.mjs markdown-stress-apple --run
EXACT_CONTENT_REGION=4194304 bun host/apple/build.mjs markdown-stress-apple --run
```

Only those two values are accepted; the region trial refuses iOS. Apple runtime
generation/parsing already uses the native continuation without this flag. The
flag additionally selects the contained paragraph before first layout and sends
CoreText preparation and viewport raster work to one bounded serial worker.
Contract, shell layout, source capture and publication remain on the UI owner.

Pixels and hit metadata publish together. A changed viewport phase can hide the
old image until replacement pixels arrive; this is not a promise to retain the
old picture through every resize. Effective appearance is fixed for a region
registration: changing it refuses the trial until restart. Source/font capture,
value copies, metadata and opaque CoreText allocations remain outside any total
memory or latency bound. Selection and copy are limited to the accepted paragraph.

Earlier 1MiB/4MiB AppKit binaries passed functional input, resize and plain-wheel
checks. They predate the current publication, appearance and selection fixes;
focused native fixtures cover those repairs. A fresh optimized build at 6663356
also passes a full-host 1MiB smoke: exact complete source, typing while pending
and accepted, width changes, and plain-wheel offset matching the published raster.
State polls can pump the agent boundary, and screenshots use the native view
cache. WindowServer capture was refused; synthetic phased-wheel/real-trackpad,
autonomous progress, continuous-resize latency and physical 120Hz remain unproved.
Exact revision-qualified evidence is recorded in LLP1041 §8.23.

## Reproduce

```sh
cargo test -p markdown-stress-data
cargo test -p markdown-stress-data --test fixtures -- --ignored
cargo clippy -p markdown-stress-data -p markdown-stress-web -p markdown-stress-apple -p markdown-stress-linux --all-targets -- -D warnings

# Start the local web server first; set CHROME to an installed Chrome binary
# if it is not at the shared driver's default /Applications/Google Chrome.app.
bun apps/markdown-stress/smoke.mjs web
# Build each native host first, and run Linux on a Linux machine for Linux evidence.
bun apps/markdown-stress/smoke.mjs macos
bun apps/markdown-stress/smoke.mjs linux
# Explicit heavier run; per-command 30-second diagnostic timeout.
bun apps/markdown-stress/smoke.mjs linux --profile blocks --bytes 1048576 --out /tmp/markdown-stress
# Same complete parsed document with viewport-sized rich-text realization.
bun apps/markdown-stress/smoke.mjs web --mode windowed --profile blocks --bytes 1048576
bun apps/markdown-stress/smoke.mjs macos --mode windowed --profile blocks --bytes 1048576
bun apps/markdown-stress/smoke.mjs linux --mode windowed --profile blocks --bytes 1048576

# Native reparse/input and real resize probes; record the selected runtime source.
bun apps/markdown-stress/native-sample.mjs macos --samples 12
bun apps/markdown-stress/native-sample.mjs linux --samples 12 --out /tmp/markdown-native-samples
```

The drive checks real input echo, profile/size changes, reparse, scrolling,
column-width changes, paging, eager rendering, finite producer controls and
reset. Reports and screenshots default to `apps/markdown-stress/target/`.
`--size 640x820` requests a different initial window size; it does not simulate
continuous OS resizing. Native command acknowledgements and browser state/tree
inspection are not physical input-to-presentation latency or frame timing.

`native-sample.mjs` uses a separate fresh app for each of three profiles: idle
40-block page, a 1 MiB document reparsed before every input with 40 blocks mounted,
and a 419-block eager document while alternating real window sizes between
640×820 and 980×820 before each input. It sends the work and type commands without
waiting between them, then checks the exact echo and the final revision. Native
resize uses the host's `tap {resize:[w,h]}` operation; that is a programmatic
AppKit window resize or Linux presenter resize, not a pointer-driven edge drag.
Reports include raw samples, the actual OS, source state and executable SHA-256.

Export the exact same fixture as a real file for the existing Markdown app:

```sh
cargo run -q -p markdown-stress-data --bin fixture -- paragraph 4194304 > /tmp/giant-paragraph.md
exact run markdown /tmp/giant-paragraph.md

# Separate generation, the shared parser and conversion of ALL blocks to values.
# This does not run Contract, layout, painting or presentation.
cargo run --release -p markdown-stress-data --bin parse-cost -- paragraph 4194304 3
```

## Integration and remaining framework work

The four workspace crates depend on the existing `apps/markdown/parse` crate;
no framework edit is required to run.

The view components are copied from `apps/markdown/app.contract` because Contract
does not currently import view components across app files. Parsing, block shapes,
value conversion and theme tokens are shared; there is no second Markdown parser.
The opt-in variable-height block window uses the shared runner collection and
host measurement/anchor path. `data/tests/windowed.rs` exercises the actual
1 MiB / 26,885-block document through scrolling, width changes, typing, reparse
and reset, and checks that a 256 KiB paragraph remains complete. Scoped tests
and Clippy pass. Physical presentation and selection across unmounted blocks remain
unverified; giant text-block subdivision with selection continuity, native
resize coalescing and worker placement remain follow-ups.

Final windowed browser and AppKit drives pass the 1 MiB / 26,885-block case.
The smoke requires the actual first/last logical block to intersect the nested
scrollport before reflow, then preserves the reading key across column changes.
Web mounts 20–21 blocks / 209–215 nodes; AppKit mounts 21 / 215–219. All five
16 KiB browser profiles also pass, including unsplit paragraph/code blocks.
These are endpoint jumps and functional layout checks, not full-document
traversals or frame-rate measurements. Artifacts are at the repository root in
`target/markdown-windowed-web-final/`, `target/markdown-windowed-web-profiles/`
and `target/markdown-stress-native/windowed-blocks-1m/`. Top/end spacing lives
inside measured first/last row wrappers so the shared logical extent includes it.

The same final 1 MiB windowed smoke passes on actual Ubuntu 24.04 ARM64 in Lima,
using CPU raster and the newly provisioned Noto CJK/emoji fonts: 21 mounted blocks
and 215–219 nodes, with endpoint/reflow/input assertions. Evidence is in
`/tmp/exact2-linux-endfollow-final-6840b5e9/artifacts/markdown-runner-reset/`;
executable SHA-256 is
`c0e9c6a142a0366cf6dd6d767f7e640d6fb15d9c96a51ed2bfb108b01cb9ffa5`.
This is a functional headless Linux result, not a compositor measurement or a
controlled timing comparison against the older DejaVu-only environment below.

## Initial parse evidence, 2026-09-16

Apple M4, 16 GiB, macOS 26.2 (25C56), Rust 1.97.0. Three repetitions per case,
native optimized diagnostic (`opt-level=3`, no LTO), source `024ace7` plus this
working branch. Other goal work was active on the machine: these are exploratory
wall-clock samples, not an isolated benchmark or physical frame measurements.
Raw CSVs are in the ignored `target/parse-*.csv` when run locally.

| Fixture | Source budget | Blocks | Parse median | All-block values median |
| --- | --- | ---: | ---: | ---: |
| One paragraph | 16 KiB | 2 | 0.47 ms | 0.001 ms |
| One paragraph | 256 KiB | 2 | 85.76 ms | 0.008 ms |
| One paragraph | 1 MiB | 2 | 1,467.38 ms | 0.025 ms |
| One paragraph | 4 MiB | 2 | 22,114.30 ms | 0.117 ms |
| One code block | 4 MiB | 2 | 1.15 ms | 0.087 ms |
| Many blocks | 1 MiB | 26,885 | 17.80 ms | 9.898 ms |
| Mixed | 1 MiB | 6,310 | 9.31 ms | 2.066 ms |
| Giant table | 1 MiB | 11,523 | 8.60 ms | 7.984 ms |

A 120 Hz interval is 8.33 ms for **everything**, not a private parser budget.
The paragraph's 256 KiB / 1 MiB / 4 MiB parse medians alone span roughly
10 / 176 / 2,654 such intervals. The 4 MiB paragraph's three samples ranged
21.27–24.23 seconds. Generation was under 3 ms in these cases and is reported
separately. The values column converts every block, as eager mode does; a manual
page converts only its selected blocks. None of these numbers includes rendering.

The near-quadratic paragraph growth is consistent with a source-level candidate:
`apps/markdown/parse/src/inline.rs` checks bare URLs at `h` and `rest()` validates
the entire remaining byte suffix as UTF-8 on each attempt. This observation is
not an attribution profile or a parser fix. Worker placement would protect UI
execution from synchronous parsing, but throughput, cancellation/coalescing and
the eventual giant text layout still need attention. The production reader's
native file parsing already uses its continuation worker, so its opening delay
and this app's synchronous input stall must be measured as different behaviors.

## Initial host evidence, 2026-09-16

Optimized web and AppKit builds passed. Both hosts passed all five 16 KiB
profiles and the 1 MiB many-block case (1,048,504 source bytes, 26,885 parsed
blocks, 40 mounted), including exact input echo, scroll, reparse, width controls,
paging, eager mode, finite producer checks and reset. Screenshots were inspected.
All four app crates passed Clippy and formatting after fixing the Linux bake's
borrowed-grants lifetime. The same smoke cases also passed on actual Ubuntu
24.04 ARM64 in Lima/VZ (Linux 6.8.0-134, 4 vCPUs, 4 GiB RAM, CPU raster).

The initial Linux capture exposed incorrectly positioned nested inline text,
overlapping later blocks. That framework defect is now fixed: the painter draws
the kernel's canonical runs once at the measured paragraph width, preserving
inherited light/dark colors and font metrics across CPU/GPU painting. GPU batches
retain synthesized italics and run/source identity; normal code now resolves to
an installed monospace family instead of accidentally selecting an italic face.
The app fixture was unchanged. Inspected follow-up screenshots show paragraphs
wrapping across the reading column, separate code/table blocks and visible text
after scrolling.

Painter validation: 59 Mac host tests, strict Clippy,
formatting and diff checks passed, including a real Apple M4/Metal pixel test.
Six applicable CPU/batch/font regressions passed on Ubuntu; its GPU pixel test
explicitly skipped because the VM has no adapter. A broader Linux library run
had one unrelated HTTP test fail with a connection reset, then pass in isolation;
both logs are retained, so this is not a clean full-Linux-gate claim. Final-binary
app validation passed all five 16 KiB profiles, the 1 MiB Blocks smoke, 36 native
input samples and 72 interleaved resize/type/scroll cohorts, with invalid-size,
viewport, echo, scroll and recovery assertions.

Final frozen Linux executable SHA-256:
`a2f9a246e6ebff0ce8971a47428665e2a0b1e4a43540da1443656afc11372fc1`.
Evidence is at the repository root in
`target/native-resize/linux-richtext-final/NOTES.md`, with raw samples in
`markdown-native-sample/report.json` and `markdown-resize/report.json` under that
directory. Screenshots are in `markdown-smoke/`, `markdown-smoke-1m/`,
`markdown-geometry/` and `markdown-resize/`; test logs are in `checks/`, alongside
source observations and executable/source hashes at the evidence root. The guest
copy is `/tmp/exact2-linux-richtext-final/bin/markdown-stress-linux`.

The final eager 256 KiB profile still mounts 1,576 blocks / 8,801 kernel nodes:
input-ACK p95 spans **165.25–178.49 ms** across three twelve-sample repetitions
(resize ACK p95 **109.54–119.41 ms**). It remains well above the 8.33 ms target.
The minimal DejaVu-only guest still lacks emoji/CJK coverage and shows missing
glyphs; full font parity is not claimed. These timings include IPC and preceding
queued work, not display presentation. Linux used headless CPU raster; the Mac's
JetKVM display is 60 Hz. Parser, scheduler and collection work changed alongside
this fix, so these captures are not a controlled performance A/B comparison.

AppKit samples on the M4 above, optimized binary SHA-256
`3abfe2fa3932c4a1222c795bfd8433089f75b43756aaaf38dfa01e14679f7e96`:

| Native workload | Samples | Input ack median / p95 / max |
| --- | ---: | ---: |
| Idle, 40 mounted blocks | 12 | 1.63 / 21.98 / 21.98 ms |
| 1 MiB reparse queued before input, 40 mounted | 12 | 25.54 / 58.35 / 58.35 ms |
| Real window resize queued before input, 419 mounted | 12 | 93.26 / 209.06 / 209.06 ms |

All 36 echoes matched. The resize receipts reported `NSWindow.setContentSize`
and the requested 640/980×820 viewport; presentation remained unobserved.
These include IPC and preceding synchronous work. At 12 samples, nearest-rank
p95 is the maximum; startup/warmup outliers are retained. Raw observations are
in `target/native-samples.json` from the initial exploratory drive; the committed
`native-sample.mjs` reproduces the same operations with configurable sample count.

On that Linux VM, the corresponding twelve-input runs measured input-ack
median / p95 of **1.26 / 1.44 ms** idle, **19.02 / 20.26 ms** behind the 1 MiB
reparse, and **15.50 / 17.73 ms** behind resizing 419 mounted blocks. All echoes
matched. Raw reports are under `target/native-resize/linux-stage/` at the
repository root; the frozen executable SHA-256 is
`90caa0268c511f40f450a46a6a216e60663ebf4a10ce62543c7d6dfc43a3851b`.
It was built from this branch with the in-progress scheduler change present;
these are exploratory workload measurements, not an isolated before/after test
or evidence that scheduler review is complete. No Linux compositor or physical
display participated.

HeadlessChrome 153.0.8010.12, 1200×850, 1× CPU, ten seconds each, using the shared
`scripts/metrics.mjs --stress-url http://127.0.0.1:4321` sampler:

| Web workload | Inputs | Input-event to echo DOM median / p95 / max |
| --- | ---: | ---: |
| Many blocks, 16 KiB, manual page, paused | 40 | 1.2 / 1.7 / 2.0 ms |
| Many blocks, 1 MiB, manual page, requested 1 Hz reparse | 39 | 1.1 / 1.6 / 1.8 ms |

The loaded run reached revision 10. Both had zero unmatched inputs and page
errors. Headless rAF gaps were 33–133 ms in ordinary samples, with a loaded
maximum of 150.1 ms: this environment does not establish 120 Hz presentation.
The DOM timing begins when the input event arrives in the page and excludes time
waiting for the event to be dispatched, so the small DOM numbers do not prove
that synchronous parsing left input responsive. Native queued-command samples
above expose a different part of that delay. Reproduce the web cases with
`--tap profile-blocks`, adding `--tap size-1048576 --tap start` for the loaded run.

## Parser suffix fix: paired evidence, 2026-09-16

The suspected UTF-8 cost was confirmed by changing only the inline parser's
suffix access. It now retains the original `&str` alongside its byte view and
uses a checked `&text[at..]` slice. The cursor advances by whole scalars or past
ASCII delimiters, so slicing checks a boundary without validating the entire
remaining paragraph. No unsafe conversion, input truncation, chunking, fixture
change, worker or rendering change was introduced. Other parser algorithms are
unchanged; this is not a claim that every Markdown input now parses linearly.

Same M4/macOS/Rust environment as above. Two frozen executables of the existing
`parse-cost` diagnostic, built with the same isolated manifest/lock and native
release settings (`opt-level=3`, no LTO), ran three before/after pairs per case.
Each invocation measured one parse of the unchanged generated input; neither
binary was rebuilt during the measurements. No cargo/rustc/Swift compiler
processes were observed in the 96 process snapshots immediately before/after
the 48 samples. This is not proof that the VM or other background work was idle.

| Fixture | Source budget | Before median | After median |
| --- | --- | ---: | ---: |
| One paragraph | 16 KiB | 0.503 ms | 0.129 ms |
| One paragraph | 256 KiB | 82.019 ms | 1.790 ms |
| One paragraph | 1 MiB | 1,339.868 ms | 7.351 ms |
| One paragraph | 4 MiB | 20,991.428 ms | 30.458 ms |
| One code block | 4 MiB | 1.257 ms | 1.248 ms |
| Many blocks | 1 MiB | 17.367 ms | 17.190 ms |
| Mixed | 1 MiB | 9.041 ms | 7.844 ms |
| Giant table | 1 MiB | 9.168 ms | 9.188 ms |

The 4 MiB paragraph contains exactly 4,194,181 source bytes and still parses to
two blocks. Before samples were 20,969.215 / 20,991.428 / 21,646.716 ms; after
samples were 30.165 / 33.057 / 30.458 ms. Median speedup is approximately 689×.
Near-equal control results should be treated as noise, not wins or regressions
established by three samples. Raw observations, executable/source SHA-256s and
the local comparison driver are in
`apps/markdown/parse/target/utf8-baseline/comparison.json` and `compare.py`.

Output preservation was checked separately by compiling the pre-change parser
from its Git source snapshot beside the modified parser. All five profiles at
all four sizes (20 documents) produced exactly equal titles and shared block
values, including every text/run/cell and link/style field. The largest
many-block document still has 107,545 blocks. This equivalence drive is recorded
in `apps/markdown/parse/target/utf8-baseline/equivalence.log`; it ran after timing
finished. Sixteen parser tests pass, including four new exact Unicode/URL-output
regressions. The opt-in 4 MiB paragraph/code integrity test, parser Clippy and
formatting also pass.

The remaining budget matters: 30.458 ms is about 3.7 entire 120 Hz intervals,
and the 1 MiB paragraph's 7.351 ms leaves little time for anything else if run on
the UI thread. These are parse-only numbers, not a new host responsiveness or
120 FPS result. Existing web/native baseline artifacts were not rebuilt for this
parser comparison. The production reader's native continuation already moves
parsing off the UI thread; the synthetic synchronous source and eventual text
layout still need their own scheduling and frame-budget work.
