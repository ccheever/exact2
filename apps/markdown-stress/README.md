# Markdown stress

A stress fixture for the Markdown readers: generated Markdown goes through the
existing `markdown-parse` crate, its shared value conversion and the shipped
reader's Contract `Blocks`/`Runs` components. No browser-only renderer or private layout
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
bun host/web/serve.mjs target/markdown-stress-dist --loopback --port 4321

bun host/apple/build.mjs markdown-stress-apple --run

cargo build --profile host-dev -p markdown-stress-linux
EXACT_AGENT=1 EXACT_PAINTER=cpu target/host-dev/markdown-stress-linux
```

The web dev loop is `bun host/web/dev.mjs --app markdown-stress`.
The Linux executable uses the existing headless/DRM host; a headless screenshot
does not exercise a desktop compositor or prove physical-display refresh rate.

### Markdown editing

**One markup node** includes a source-backed editor and a reader of that same
source. Bold, Italic, Code, List, Quote and H2 operate on the editor's current
selection. Link opens a URL field while preserving that selection; Apply Link
returns focus to the editor. The line beneath the toolbar reports the shared
selection state. Return continues a list and an empty item exits it.

On macOS and the web, use the usual bold/italic and undo/redo shortcuts.
Normal copy preserves Markdown; ⌘⇧C on macOS or Mod-Shift-C on the web copies
plain text. iOS exposes Copy Plain Text in its edit menu. Native editors keep
all syntax visible at normal width, dimming inactive markers. The web editor
hides syntax (WYSIWYG), with its rules in `markdown/editor/`. These are fixed-height
editor demonstrations; phone input ergonomics and intrinsic sizing still
need separate proof.

### Explicit Linux cold-paragraph trial

The same executable can select a complete cold paragraph before its first layout:

```sh
EXACT_PAINTER=cpu target/host-dev/markdown-stress-linux --content-region=1048576
EXACT_PAINTER=cpu target/host-dev/markdown-stress-linux --content-region=4194304
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
cap. With shared width layouts
on captured ef12f06 sources, both 1MiB and 4MiB trials now complete under the same
cap. Two distinct measurement requests reuse one worker layout and ink index.
The complete 4,194,149-byte paragraph publishes without another input, accepts
further typing and scrolls 40px. Accepted content pixels match the fresh
independent reference; the overlapping scroll strip matches exactly too.

The 4MiB run samples VmPeak at 2,303,612KiB and RSS/HWM at 1,384,404KiB.
These are observations from this fixture, not a general memory bound or an
isolated allocation-site measurement. The source, fonts and cap were not reduced.
Continuous resize, repeated latency measurements and physical120Hz remain
outstanding.

A later retained-document reflow trial also passes at both 1MiB and 4MiB:
the authored content column changes from 600 to 984px inside a fixed 1024×768
display. It combines the frozen ef12f06 sharing path with glyph compaction and
the span-reservation removal. The old layout remains interactive while the new
width computes; publication needs no additional input. Both accepted viewport
crops and both 40px scroll-overlap strips match fresh independent references.
At 4MiB, sampled VmPeak is 2,411,644KiB and RSS/HWM is 1,774,156KiB, leaving
209,796KiB below the unchanged address-space cap in this one fixture. Earlier
novel-width crashes remain recorded. This proves the bounded content-reflow
case, not a general memory bound, continuous window resize or 120Hz.

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

The worker now reuses a live CoreText layout when another request has the same
captured source object, source ID, generation and definite width. Each answer
still has a fresh request identity. Lookup uses the existing 64-entry live table;
retired widths are not kept as a history. Scoped production-worker tests reduce
three constructions to one and retain pixel, hit, selection and retirement
checks: 16 methods/20,885 assertions pass under strict Swift 6. This repair has
not yet been measured in the full-host giant-paragraph workload.

## Reproduce

```sh
cargo test -p markdown-stress-data
cargo test -p markdown-stress-data --test it fixtures:: -- --ignored
cargo clippy -p markdown-stress-data -p markdown-stress-web -p markdown-stress-apple -p markdown-stress-linux --all-targets -- -D warnings

# Start the local web server first. The driver uses Playwright's Chromium;
# set CHROME to use another browser.
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
does not currently import view components across app files. Parsing, block shapes
and value conversion are shared, and the root `theme` block says the readers'
palette; there is no second Markdown parser.
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
and 215–219 nodes, with endpoint/reflow/input assertions. This is a functional
headless Linux result, not a compositor measurement.

## Known limits

The paragraph parser no longer revalidates the remaining source as UTF-8 at
each bare-URL check, so one 4 MiB paragraph parses in tens of milliseconds
rather than tens of seconds. That is still several 120 Hz frames: the synthetic
synchronous source and giant text layout need their own scheduling.

**One markup node** lays the whole document out as one `text markup="markdown"`
node, styled by `exact-markdown::pieces` for both measure and paint. It is not
windowed.

Native command history is incomplete: a source replacement and a format inside
the same automatic event group can undo together. For example, replacing the
buffer with `- item`, then applying list Return, can undo past `- item` to the
earlier buffer. One-command-one-undo and intrinsic editor-height parity are not
complete.
