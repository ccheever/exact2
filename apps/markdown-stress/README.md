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
budget; the app reports actual source bytes, parsed blocks, mounted blocks, table
rows and the largest block's text bytes. These are not process-memory figures.
The 4 MiB ceiling matches the existing reader's file admission limit.

The default is a **manual page of 40 blocks**. It still generates and parses the
entire document and retains that parsed document; it is not virtualization.
One 4 MiB paragraph remains one 4 MiB paragraph even in page mode. **Render ALL
blocks** explicitly mounts the complete document and can stall the current UI.

The 640/1200 px column controls reflow the same document without reparsing.
Also drag the real window edge, scroll partway through the document and type in
the persistent input. The echo is bound to Contract state and capped at 512
Unicode characters. Neither width changes nor typing depend on the document
resource arguments. Paging reuses the parsed fixture; changing profile, size or
revision reparses it. **Parse again** performs one full synchronous reparse;
the timed control requests 20 reparses at 1 Hz, capped at revision 1,000 until
Reset. Pause cannot interrupt a synchronous operation already in flight.

This is a stress baseline, not a worker, cancellation, virtualization, scroll
anchoring or 120 FPS implementation. In particular the production Markdown
reader reads and parses native files on its continuation worker, whereas this
cross-platform synthetic source currently parses synchronously. Do not present
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

# Input behind synchronous reparsing, and real native resize followed by input.
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
Large variable-height block virtualization, giant text-block subdivision with
selection continuity, native resize coalescing, stable scroll anchors and worker
placement are framework follow-ups, to be measured separately from this fixture.

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
Those are functional assertions, not visual parity: the Linux mixed-document
screenshot exposes incorrectly positioned nested inline text, overlapping later
blocks. The AppKit screenshot renders those paragraphs and tables correctly.
Linux paragraph painting is a separate framework fix; the fixture retains it
as a reproduction.

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
