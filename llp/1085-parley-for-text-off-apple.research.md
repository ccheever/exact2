# LLP 1085: Parley for text off Apple — measured against cosmic-text on exact2's inputs

**Type:** Research
**Status:** Draft
**Systems:** Linux host text (`host/linux/src/text*`: cosmic-text shaping, the width-specific `Paragraph`, the font catalog, swash ink), the `TextMeasurer` seam (LLP 1001 §6), `exact-textflow` (LLP 1043.000), `vendor/cosmic-text` (three local patches)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Revised:** 2026-10-05 (F8: cosmic-text's share of a scroll's main-thread CPU on the Linux host, measured on Linux with both painters; the Summary, Recommendation and Confidence updated for it)
**Related:** LLP 1015 §3 (the Linux host's one text engine), LLP 1001 §1, §6 (declared deviations; the injected measurer), LLP 1043 and 1043.000 (Pretext; the text-flow walker and its Chrome corpus), LLP 1053 (CSS `direction`; cosmic patch 3), LLP 1033 (the Markdown reader), `vendor/cosmic-text/EXACT-PATCHES.md`, `rules/DEFERRED.md` (Linux skips Thai word breaking, 2026-09-28; Linux editing is v2), `QUEUE.md` ("Parley for text off Apple"). External, each read 2026-10-04: crates.io `parley` 0.11.1 and `fontique` 0.11.1 sources (Apache-2.0/MIT); `~/bench/dioxus/blitz-bugs-for-nico.md`.

## Summary

Blitz lays out text with Parley (Linebender). exact2's Linux host uses
cosmic-text 0.19 (`vendor/cosmic-text`, three local patches). This compares
the two on exact2's own inputs: the heavy list's 17,523 message paragraphs,
1,415 paragraphs from six Markdown documents the reader opens, Chrome as the
oracle with the same font files, and the text-flow walker's Chrome
break corpus. The comparison changes no code. This LLP decides nothing and is not linked
into `llp/current/`.

The two engines share a shaper (harfrust, the Rust HarfBuzz port) and break
lines by the same rules, so on ordinary text they agree with each other
on 98.6–99.3% of paragraphs. Parley is faster, holds much less memory and,
with the Blink break table it ships, matches Chrome more often. It also lacks
three things exact2 already has in cosmic-text: CSS base direction, ellipsis/
`line-clamp`, and refusing a damaged font. Two of the three cosmic patches
would have to be paid again.

| | cosmic-text 0.19 (vendored) | Parley 0.11.1 |
|---|---|---|
| Shape, µs per message paragraph (best of 7) | 47.0–51.0 | 27.2–30.3 |
| Break at one width, µs per paragraph (200–800 px) | 2.0–3.5 | 0.9–1.8 |
| Heap held per byte of text, 1 MiB paragraph | ~255 B (260 MiB) | ~62 B (64 MiB) |
| Line starts equal to Chrome's (2,150 paragraphs × 4 widths) | 86.4–98.9% | 87.5–98.8% as is; **98.3–99.8%** with `CHROMIUM_LINE_BREAK_OVERRIDE` |
| CSS `direction: rtl` base level | yes (patch 3) | no API: first strong character always |
| `line-clamp`, `text-overflow: ellipsis` | yes (`Ellipsize`) | none |
| Damaged face (`unitsPerEm` 0 or no `head`) | refused (patch 1) | admitted: infinite advances |
| Linux aarch64 ELF bytes added over a base with swash, fontdb, skrifa 0.44 | +1.07 MB | +0.94 MB; +4.73 MB with `complex-scripts` |
| System fonts on Linux without a C library | fontdb scans directories | fontique's `system` links fontconfig; without it the host states every fallback itself |

**Recommendation.** Don't switch now. Parley is the better engine to
switch to when the Linux host's text is next rebuilt, and the switch fits
behind the measurer seam without changing it. Three things should come
first, and Nico Burns can answer most of them (§Questions): base direction,
font admission, and the Devanagari line-advance defect. Ellipsis and clamp
belong in `exact-textflow`, which already clamps flowed text. Two findings
stand without a switch: cosmic-text's ordinary layout breaks URLs after `/`
where Chrome doesn't (3–5% of real paragraphs), and its shaped paragraphs
are about four times the size of Parley's. §Recommendation gives the order.

**Where it matters (F8).** cosmic-text is the text engine of every non-Apple
native surface: `exact-linux` also builds for Android (its `cfg(target_os =
"android")` dependencies present into the window's swapchain), and the game
engine uses it directly. Apple uses CoreText and the web the browser. On
the GPU painter, the path Android uses, shaping and line breaking are
**5.2–5.5%** of the main thread's CPU while scrolling the heavy list, so
Parley's speed would buy about 2.5–3% there. The case for a switch is
Chrome's breaks, memory and re-layout, not scroll speed.

## Method

Everything ran on the Mac Studio (M5 Ultra) on 2026-10-04, outside the
repo, in `~/bench/dioxus/q5/`. Other sessions shared the machine (load
average 28–31 on 36 cores). Timings are single-threaded, best of 7 passes,
and two separate runs agreed within 10%.

- **Fonts.** Both engines and Chrome get the same Noto files, as a Debian
  box ships them: Noto Sans (Regular, Bold, Italic), Noto Sans Mono,
  Devanagari, Arabic, Hebrew, Thai, Khmer, Myanmar, Lao, Noto Sans CJK JP
  (OTF) and Noto Color Emoji (CBDT, v2.047). No system font takes part.
- **cosmic-text** is the vendored crate (all three patches), called as
  `ShapedSource` calls it: `Buffer::set_rich_text`, then
  `ShapeLine::new_with_base` per hard line, then `layout_to_buffer` per
  width with `Wrap::Word`. Its fallback is a copy of its own `unix.rs` list,
  because on macOS it would use `macos.rs`. The only change in that copy
  adds "Noto Sans CJK JP" after "Noto Sans CJK SC" for Han, since only the
  JP OTF was downloaded.
- **Parley** is 0.11.1 with `system_fonts: false`. The bench registers the
  same files, maps the generic families (sans-serif, monospace, emoji) and
  states each script's fallback, as cosmic's `unix.rs` does. It calls
  `ranged_builder` → `build` → `break_all_lines` → `align(Start)`.
- **Styled runs**: message runs are 16 px; bold and mention are 700, italic
  is italic, code is Noto Sans Mono at 15 px. Markdown headings are 28, 22 or
  18 px bold, prose 16 px, code spans 15 px mono.
- **Chrome 154** (`chrome.mjs`, playwright-core) lays out each paragraph in
  a `div` of the same width under `white-space: pre-wrap`. The fonts load
  through `@font-face` from the same files. It records line starts as
  UTF-8 offsets, height, widest line, each character's x, and the fonts
  used (`CSS.getPlatformFontsForNode`).
- **Inputs**: every 20th message paragraph (835) and every Markdown prose
  paragraph (1,315) from `apps/markdown/data/src/welcome.rs`, `README.md`, and LLPs 1001,
  1043, 1033 and 1071, parsed by `markdown-parse`. The widths are 200, 302 (the heavy list's
  phone column), 500 and 800 px. Timing uses all 17,523 + 1,415 paragraphs.
- **Size** (`sizebench/`): four small binaries load fonts, shape and break
  argv text, and rasterize a glyph with swash. A skrifa 0.44 outline stands
  in for vello. They use the repo's release profile (thin LTO, one codegen unit,
  `panic = "abort"`, strip) and target `aarch64-unknown-linux-gnu`. They are
  linked with the toolchain's `rust-lld` against empty libc stubs and
  `--unresolved-symbols=ignore-all`, with `main` as the entry. The result
  is not runnable, but `--gc-sections` leaves the real code size. The whole
  host could not be cross-linked here (`ring` wants a C cross-compiler).
  Its macOS build, `caltrain-linux` release, is 11.70 MB (9.46 MB `__text`).

## Findings

### F1. Speed

| input | engine | shape µs/para | break @200 | @302 | @500 | @800 | shape p50/p99 | first pass (cold fonts) |
|---|---|---|---|---|---|---|---|---|
| messages (17,523; 3.46 MB) | cosmic | 47.0 | 2.9 | 2.5 | 2.1 | 2.0 | 43.5 / 131 | 894 ms |
| | Parley | 27.2 | 1.6 | 1.1 | 1.0 | 0.9 | 25.0 / 83 | 540 ms |
| Markdown (1,415; 273 KB) | cosmic | 39.5 | 3.5 | 3.1 | 2.7 | 2.7 | 15.9 / 411 | 62 ms |
| | Parley | 23.7 | 1.5 | 1.1 | 1.0 | 0.9 | 10.5 / 256 | 40 ms |

The second run is shown. The first was within 10% and in the same direction.
Parley shapes about 1.7× faster. It re-breaks a shaped paragraph at a new width 2.5–3× faster, and
that re-break is what Taffy's repeated offers cost (LLP 1015 §3 counts 878 requests for 128
nodes at boot). The shaper is the same family in both, so the difference is the
work around it. cosmic-text allocates a word/glyph structure per call, while
Parley reuses its `LayoutContext` scratch. These numbers leave out the host's
own per-line line-box pass and its caches, which a Parley host would also need.

### F2. Memory

A counting allocator, with both engines warm on every script first, measured:

| | cosmic-text | Parley |
|---|---|---|
| one 64 KiB paragraph, shaped and laid out at 800 | 16.3 MiB | 4.0 MiB held (peak 4.4) |
| one 1 MiB paragraph | 259.8 MiB | 63.5 MiB held (peak 70.8); 33.1 MiB of it is the context's scratch, kept after the `Layout` drops |
| all 17,523 message paragraphs shaped and kept | 501.9 MiB (shapes only) | 168.2 MiB (shaped and broken at 302) |

That is about 255 bytes of heap per byte of text for a `ShapeLine` and about 60 for a Parley
`Layout`. Cosmic patch 2 (EXACT-PATCHES.md) removed one per-byte reservation,
and this is what remains. Parley has its own high-water: per-character
analysis kept in the `LayoutContext`, which the host would own and could
drop after giant documents.

### F3. Line breaking against Chrome

Paragraphs whose line starts equal Chrome's at the same width and fonts:

| input | width | cosmic | Parley | Parley + Chrome override |
|---|---|---|---|---|
| Markdown | 200 | 86.4% | 87.5% | 99.5% |
| | 302 | 89.9% | 90.6% | 99.4% |
| | 500 | 93.2% | 94.5% | 99.5% |
| | 800 | 95.2% | 96.2% | 99.5% |
| messages | 200 | 93.2% | 91.9% | 98.3% |
| | 302 | 96.0% | 95.9% | 99.5% |
| | 500 | 98.4% | 98.2% | 99.8% |
| | 800 | 98.9% | 98.8% | 99.6% |

Both engines break the way UAX #14 does, and so they disagree with Chrome
in the same places. After `/` in a URL or path (`apple.com/|swiftui`,
`rules/|DEFERRED.md`) they break where Blink doesn't: that is 3.1% of
message rows and 5.3% of Markdown rows. Around hyphens before digits
(`8:30-|4:30`, `bun-|1.4.2`) Chrome breaks and they don't: 1.4% of
Markdown rows. Parley ships `CHROMIUM_LINE_BREAK_OVERRIDE`, Blink's
`kFastLineBreakTable`, which is the table `textflow/src/chrome.rs` carries
for the flow walker. Applied through `set_line_break_override`, it removes
almost all of the difference. What is left is about 50 of 8,600 rows. Most
are lines filled to within 0.02 px of the width, where Chrome's LayoutUnit
rounding fits one more word. The rest are a few Markdown headings with an
em dash, where the two engines agree with each other and not with Chrome.
cosmic-text has no such hook. Today the Linux host applies Blink's table
only on the flow path (LLP 1043.000), not in ordinary paragraphs.

The text-flow corpus (`textflow/tests/corpus/breaks.txt`) holds Chrome's
line starts at width 0, which is its break-opportunity set. Against it, out of
17,220 Chrome starts:

| | cosmic | Parley | Parley + `complex-scripts` |
|---|---|---|---|
| missing / extra, all corpora | 1,758 / 10 | 2,183 / 544 | 565 / 571 |
| Thai (2 corpora) | 1,130 / 0 | 1,130 / 7 | 36 / 31 |
| Myanmar (2) | 552 / 0 | 559 / 97 | 43 / 100 |
| Khmer | 13 / 0 | 427 / 425 | 427 / 425 |
| Japanese *kumo-no-ito* | 53 / 0 | 53 / 0 | 53 / 0 |
| Hindi, Urdu, Hebrew | 0 / 2 | 4 / 8 | 4 / 8 |
| CJK, Korean, Arabic, English | 0 / 1 | 0 / 1 | 0 / 1 |

Without dictionaries, neither engine has Thai or Myanmar words. That is
DEFERRED's ruling for Linux (2026-09-28). Parley's `complex-scripts`
feature brings ICU4X's dictionaries and models, which add 4.7 MB (F6). In
Khmer, which is written with U+200B between words, Parley breaks *before* each
U+200B where Chrome and UAX #14 (LB7/LB8) break after it. Firefox does the
same (`record.mjs`). The bench's Latin `aaaa​bbbb` broke after the
U+200B correctly, so this is specific to the SA class.

### F4. Bidi, Devanagari, Cyrillic, emoji

Nineteen cases (`cases`), at 120 and 2,000 px, against Chrome:

- **Cyrillic, Hebrew/Arabic inside Latin, digits in Arabic, brackets:**
  both engines give Chrome's line starts and widths (within 0.2 px),
  and Chrome's visual order on single lines. On the two-line Arabic cases at 120
  px Parley places one cluster out of Chrome's visual order on a line.
  cosmic-text places none.
- **CSS `direction: rtl` with a Latin first word** (`abc אבג` and
  `Exact הוא מנוע` in an `rtl` paragraph): cosmic-text with patch 3 orders
  them as Chrome does. Parley can't express it.
  `analysis::analyze_text` always calls `bidi.resolve(…, None)`, so the base level is the
  first strong character's, and two clusters land on the wrong side. This is
  the LLP 1053 regression patch 3 fixed.
- **Devanagari:** single words shape identically in both, and both match
  Chrome's width. In some multi-word lines, though, Parley's *line* advance is larger
  than the sum of its runs. `क्षत्रिय श्रृंखला द्विज` gives runs summing to
  106.68 px and a line advance of 109.88 px. Chrome and cosmic-text say
  106.7. At 120 px, `Release नमस्ते 2026 क्षत्रिय ok` therefore breaks one
  character early (start 26 against Chrome's 27). Any two of the three words
  measure correctly. The cause wasn't traced: it's a question for Nico.
  Parley's natural line height for Devanagari is 22 px, against Chrome's
  24. Chrome grows the line box for the fallback face's metrics, and so does
  the host's line-box pass (LLP 1015 §3). For CJK Parley gives Chrome's 24.
- **Emoji fallback:** basic, ZWJ sequences (👩‍💻, 👨‍👩‍👧‍👦), flags,
  keycaps, skin tones, and VS16 against text-presentation ❤. Both engines
  pick Noto Color Emoji wherever Chrome does, and match Chrome's widths
  within 0.2 px. Parley needs the host to map `GenericFamily::Emoji`. With
  `system_fonts: false` nothing else names it. Blitz's blank emoji (bug 3 in
  `blitz-bugs-for-nico.md`) were vello_hybrid's painting, not Parley's
  fallback.
- **`line-clamp: 2` and `text-overflow: ellipsis`:** cosmic-text gives
  Chrome's two lines and 44 px, ending in "…" (its second line starts one
  byte early, at the space). For a one-line ellipsis it gives Chrome's single line.
  Parley 0.11.1 has neither: no ellipsis, clamp or max-lines anywhere in
  the crate. A Parley host would truncate itself. `exact-textflow` already
  clamps and adds ellipses on the flow path (`flow.rs` builds the "…" and
  hyphen glyphs), so that walker is where clamping would belong.
- **Soft hyphen, NBSP, URL at a narrow width:** the two engines agree with each other.
  Neither draws the hyphen Chrome draws at a soft-hyphen break. Both
  break a long URL after `/` where Chrome overflows (F3).

### F5. Cosmic's three patches, as a switch would treat them

1. **Font admission** (`unitsPerEm` 0, or `bhed` without `head`): this
   cost would be paid again. With the host test's own damaged DejaVu fixtures
   (`transfer_tests/font_admission.rs`), fontique registers the
   face and Parley lays out "office café" in it with infinite advances (`width
   inf`, height 4,768). A Parley host would refuse such faces when
   registering them, or fontique would need the same three-line check.
2. **Span storage** (no per-byte `ShapeSpan` reservation): this patch would
   be dropped. Parley's storage is different, and smaller (F2).
3. **Base direction** (`ShapeLine::new_with_base`): this cost would be paid
   again, as a patch threading a base level through the builder to
   `BidiResolver::resolve`, which already takes one. It is small, and it
   could go upstream.

The host also compensates for cosmic-text in ways that are not patches.
`snap_weight` exists because cosmic-text's fallback ranks a variable face
covering the requested weight above the family's nearest static face
(LLP 1015 §3). The serif and sans resolution in `catalog.rs` exists because cosmic defaults
to families the machine may not have. fontique's matching is CSS font
matching, which should retire the first of these. The bench did not test that.

### F6. Binary size

Linux aarch64 ELF, stripped, repo release profile:

| variant | bytes | over base |
|---|---|---|
| base: fontdb + swash raster + skrifa 0.44 outline | 1,237,896 | — |
| cosmic-text (shape, break, ellipsize, SwashCache raster) | 2,303,320 | +1,065,424 |
| Parley (shape, break, align) + swash raster | 2,172,440 | +934,544 |
| Parley with `complex-scripts` | 5,964,848 | +4,726,952 |
| both linked | 3,262,464 | +2,024,568 |

The Mach-O builds give the same ordering (cosmic +1.02 MB, Parley +0.93 MB,
`complex-scripts` +4.74 MB). The host binary is 11.7 MB on macOS, so the
text engine is about 9% of it either way. Replacing cosmic-text with Parley
saves about 0.13 MB, if swash stays for raster. Linking both during a
transition costs about 1 MB. `complex-scripts` costs about 3.8 MB more, and
by exact2's own rule (optional capability is a separate artifact, never a
cargo feature on a core crate) it would have to load as data on demand.

### F7. Fit with the measurer seam and `exact-textflow`

- **The seam (LLP 1001 §6)** doesn't change. `TextMeasurer::measure`
  receives ordered runs, a style per run, an offer and the paragraph's direction. That maps
  onto a ranged builder, one `push` per run. The host's shape-once,
  break-per-width split (`ShapedSource` / `Paragraph`) is Parley's own
  split (`build` once, `break_all_lines` per width). Min- and max-content
  are `Layout::calculate_content_widths`, so the host would no longer have to
  wrap at width zero to find them (LLP 1015 §3). Paragraph stamps, the
  width cache, `transfer.rs`'s worker handoff and residency accounting would
  all keep their shape. What changes is the shaped data they hold: an
  immutable `Layout` plus its styles, not `ShapeLine`s.
- **`exact-textflow`** wants per-cluster widths over byte ranges in logical
  order, from one unwrapped line. `flow.rs` builds them today by sorting
  cosmic-text's visual glyphs back into logical clusters. Parley's
  `Run::clusters()` yields logical clusters with `text_range()` and
  `advance()` directly. Parley's `BreakLines` can also change each line's
  maximum advance and its x/y, which is the shape of a line walker for text
  around shapes. exact2's walker carries Blink's table, the soft-hyphen and
  ellipsis glyphs, and the segmenter words the web host supplies, so it
  should keep breaking lines and use Parley only for cluster widths.
- **Fonts on Linux.** fontique's `system` feature links libfontconfig
  through `yeslogic-fontconfig-sys` (or dlopens it, with
  `fontconfig-dlopen`). That conflicts with LLP 1015's "no system library
  is linked". Without it, fontique scans no directories and has no fallback
  table. The host would keep its own discovery (fontdb's directory scan
  could feed `register_fonts`, or fontique's `load_fonts_from_paths`) and
  state per-script fallbacks itself, much as cosmic's `unix.rs` does today.
- **The size of the change:** about 5,100 non-test lines in `host/linux/src`
  touch cosmic-text types (`text.rs`, `text/{shaping,cache,ink,flow,
  catalog,catalog_recipe,transfer}.rs`, `canvas2d/text.rs`), and about
  6,300 test lines. All 174 of the host's text tests pass today
  (`cargo test -p exact-linux --lib text`, 2026-10-04). Most of them pin
  cosmic-text's structures (span capacity, shaped-line sharing, glyph
  batches), not CSS behavior, so they would be rewritten, not ported.
- **Beyond v1:** Parley includes a `PlainEditor` (selection, IME
  preedit, cursor geometry) and AccessKit nodes, behind the `accesskit` feature. Linux editing is
  v2 (DEFERRED, 2026-09-21), and those two pieces are what it would need.

### F8. cosmic-text's share of a scroll (2026-10-05)

F1 times the engines alone. This measures what share of the Linux host's
work they are while a reader scrolls, which bounds what a faster engine
can buy.

**Method.**
- **Machine:** Ryzen AI 9 HX 370 (24 threads, Linux, idle: load average under 1), Radeon 890M over Vulkan (RADV).
- **Build:** the heavy list (`~/bench/dioxus/textprof-app`: the 10,000-message whole feed with a `linux/` crate) on exact2 at `6e649fb51`, profile `host-dev`, 2× scale (840 × 1720 px), assets loaded.
- **Drive:** `scripts/agent.mjs linux`, 200 steps of `tap messages wheel 0 600` then `clock settle`, about 120,000 pt of feed. Three runs per painter.
- **Timers:** `perf` is unavailable on every Linux machine here (`perf_event_paranoid` 4, no root). So a temporary patch, reverted and never committed, timed four `TextEngine` functions with cumulative `Instant` timers against `getrusage` of the thread that runs them:
  - `build_source`: cosmic-text shaping;
  - `layout_source`: breaking at a width;
  - `glyph_runs`: glyph positions for the GPU painter's stream;
  - `paint_clipped`: the CPU painter's glyph raster and blending.

  `TextEngine` is single-threaded (`Rc`/`RefCell`), so its time is a share of that one thread. Boot is excluded (the first sample's totals are subtracted).

| Painter | Main-thread CPU | Shaping + breaking | Glyph runs (GPU stream) | Text raster (CPU painter) |
|---|---:|---:|---:|---:|
| GPU (the path Android uses) | 1,442–1,623 ms | 76–90 ms, **5.2–5.5%** | 26–32 ms, 1.8–2.0% | — |
| CPU (tiny-skia) | 6,231–6,749 ms | 92–93 ms, 1.4–1.5% | — | 1,074–1,083 ms, **16.0–17.3%** |

Shaping was 55–64 ms of the 76–93 ms in every run, breaking 22–29 ms.
Process CPU was 6–8% above main-thread CPU (image decoding and the painter's
threads).

**What it bounds.**
- **Parley's gain while scrolling.** At F1's ratios (shaping ~1.7×, breaking 2.5–3×) it would save about 45% of 5.2–5.5% plus part of the glyph runs: about **2.5–3%** of main-thread CPU on the GPU painter, and under 1% on the CPU painter.
- **The CPU painter's text cost is raster.** Its 16–17% is glyph rasterizing and blending through swash and tiny-skia, which a switch to Parley keeps.
- **This is close to an upper bound for scrolling.** The heavy list is mostly rich paragraphs, and each is shaped once and cached; most screens hold less text.
- **Re-layout is not measured here.** A width change (rotation, a window resize, a foldable's posture) re-breaks every visible paragraph, where Parley's 2.5–3× applies to all the work.

**Caveats.** This is a desktop x86 core, not a phone. The shares should
carry better than the times, but no Android device was measured. The
agent's settled steps are a sequence of scrolls, not a fling at a display's
cadence.

Data: `~/bench/dioxus/textprof/tp-{gpu,cpu}-2-{1,2,3}.jsonl` (cumulative
nanoseconds per 250 ms).

## Confidence

- **High:** F3's agreement rates and their causes, F5.1 and F5.3, F4's
  RTL and clamp/ellipsis findings, and the F6 deltas. All of them are
  reproducible from `~/bench/dioxus/q5` (`textbench`, `chrome.mjs`,
  `compare_*.py`, `sizebench`).
- **Medium:** F1's ratios, measured on a shared machine but stable across
  two runs. The cosmic side calls exactly what the host calls; the Parley
  side is the obvious use, not a tuned one. F2's absolute numbers are a
  counting allocator's, not RSS.
- **Low:** heights. The bench's cosmic line box approximates the host's
  `line_box` (strut plus each glyph font's ascent, descent and gap). Parley's is
  `LineHeight::MetricsRelative(1.0)`. Both are within ±1 px of Chrome on
  52–99% of rows, depending on width, and both fall short where fallback
  faces are taller. The host's real heights are LLP 1035.000.000's and were
  not measured here.
- **Medium:** F8's shares: three runs per painter agreed within 0.3
  points, on an idle Linux machine. They are one app's scroll on a
  desktop core, not a phone's, and not a re-layout.
- Not measured: raster through Parley's types; an Android device; a
  re-layout's share (F8); CJK beyond the JP face standing in for SC/TC/KR.
  F1–F7 ran with pinned fonts on a Mac; F8 ran on Linux.

## Recommendation

1. **Now: nothing in the Linux host changes.** Its 174 text tests pass, and
   its known gaps are F3's (URL `/` and hyphen breaks in ordinary paragraphs,
   about 3–5% of real rows) and Thai/Khmer/Myanmar words, which are ruled out.
2. **Ask Nico** (the questions below). Base direction and font admission
   decide whether Parley needs a vendored patch, as cosmic-text does, or
   none.
3. **When someone is assigned to non-Apple text** (Linux, and Android
   through the same host), with an implementer and a date (`rules/RULES.md`
   §Scope), switch behind the seam. Android or Exposé work is the likely
   occasion. In this order:
   - Parley shapes, with `CHROMIUM_LINE_BREAK_OVERRIDE` on and
     `complex-scripts` off.
   - Clusters feed `exact-textflow` for flow, clamp and ellipsis.
   - The host keeps fontdb discovery, its per-script fallback list and
     swash raster.
   - The host refuses damaged faces at registration.
   - The rewritten text tests are the gate, with the Chrome comparison
     here as a new parity check.

   Gains: about 1.7× faster shaping, 2.5–3× faster re-breaking, about a
   quarter of the memory per shaped paragraph, Chrome's breaks in ordinary
   text, and CSS font matching. While scrolling, the speed is worth about
   2.5–3% of the main thread on the GPU painter (F8); the larger gains are
   re-layout, memory and Chrome parity. Costs: rewriting the host's text module
   and its tests, and two patches paid again.
4. **If Linux text is not rebuilt**, two smaller changes take part of the
   gain without Parley:
   - Break ordinary paragraphs with the textflow walker, as the flow path
     does, to get Blink's table.
   - Look at why a cosmic `ShapeLine` costs about 255 B per byte of text,
     for the giant-Markdown work (LLP 1044).

## Questions for Nico Burns

1. Is CSS base direction planned for the builder? The resolver already
   takes a level, but `analyze_text` passes `None`. Would you take a patch?
   How does Blitz handle `direction: rtl` today?
2. Ellipsis, `-webkit-line-clamp` and `text-overflow`: does Blitz do these
   above Parley, and is there a plan to have Parley do them?
3. fontique admits a face with `unitsPerEm` 0, or with `bhed` but no `head`, and
   Parley then lays out with infinite advances. Should fontique refuse
   such faces at registration?
4. Devanagari: `क्षत्रिय श्रृंखला द्विज` at 16 px (Noto Sans Devanagari
   through fallback) gives a line advance 3.2 px larger than the sum of its
   runs. Any two of its words are fine. Is this a known issue?
5. Khmer: with U+200B between words, Parley's opportunities come before
   each U+200B, not after (UAX #14 LB7/LB8; Chrome breaks after). Is this from
   ICU4X's SA handling, or from Parley?
6. Linux without linking fontconfig: is a pure-Rust discovery and fallback
   path planned for fontique? What does Blitz ship on Linux?
7. `complex-scripts` adds about 4.7 MB. Can ICU4X's segmenter data be
   loaded at runtime, as a separate artifact, rather than compiled in?
8. Line height `normal`: should a fallback face's taller metrics grow the
   line, as Chrome does? We saw 22 px against Chrome's 24 for Devanagari.
