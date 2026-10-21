# exact2 vs Dioxus — the heavy list on Chrome, macOS and iPhone

The heavy list (`../heavy-list/SPEC.md`: 10,000 rich messages, the same `messages.json`, the same 104 JPEGs) on
Dioxus 0.8.0-alpha.1 — Dioxus Web in Chrome and Dioxus Native (Blitz 0.3.0-beta.1) on macOS — against exact2's
JS target and its macOS host. It measures scroll (frames, late frames, blank samples, CPU), memory, page weight,
tap latency and cold start. A second exact2 app with a bounded answer (LLP 1027.004) is measured beside the
whole-feed one on all three, and on an iPhone with `../heavy-list`'s probe.

## The apps

| App | Directory | Built by |
|---|---|---|
| exact2, whole feed | `../heavy-list/exact-heavylist` (that bench's app, with its `web/` crate) | `web/build.sh`, `mac/build.sh`, `device/build.sh` |
| exact2, bounded window | `exact-bounded/` (bundle id `dev.exact.heavybench.bounded`) | the same |
| Dioxus (Web and Native) | `heavy-dx/`: one codebase, features `web`, `native`, `vello` | `web/build.sh`, `mac/build.sh` |

`heavy-dx/src/style.css` mirrors the exact2 app's `app.contract` rule for rule. Dioxus has no virtualized list, so
`Feed` windows rows itself (one viewport of overscan each side, a 160 px estimate, heights measured at scroll events
from kept `onmounted` handles: Blitz fires `onmounted` before layout, and `onresize` is `unimplemented!()`).
`heavy-dx/src/bin/repro.rs` is the minimal Blitz repro of two fidelity bugs below.

**Pinned Dioxus.** `heavy-dx/Cargo.toml` pins `dioxus` and `dioxus-native` at `=0.8.0-alpha.1` and the committed
`heavy-dx/Cargo.lock` pins the rest (Blitz 0.3.0-beta.1, vello 0.0.9's hybrid renderer, …); Cargo fetches them from
crates.io. Nothing of Dioxus is vendored. The CLI is dioxus-cli 0.8.0-alpha.1 (`dx --version`: `0.8.0-alpha.1
(5fb7e6e)`), installed outside the source tree:

```sh
cargo install dioxus-cli --version 0.8.0-alpha.1 --locked --root target/bench/dioxus/tools   # DX defaults to its bin/dx
```

Why 0.8.0-alpha.1: stable 0.7.10 resolves blitz-shell 0.2.3, whose `WindowEvent::Touch` is `// Todo implement
touch scrolling` (no finger scrolling on iOS); 0.8.0-alpha.1 (Blitz 0.3 beta) has touch panning and fling and was
the newest release on 2026-10-03.

## Building

```sh
bench/dioxus/prepare.sh            # bench/heavy-list's data (gen.py), copied into heavy-dx/ and exact-bounded/
bench/dioxus/web/build.sh          # web/dist/{exact,bounded,dioxus}: exact2's production JS target ×2, Dioxus Web
bench/dioxus/mac/build.sh          # the measuring tools, then the three macOS apps (PART= one of them)
bench/dioxus/device/build.sh       # the iPhone builds of both exact2 apps, signed with the probe
```

exact2's production web build is delivery's two steps: the web crate's bake without its wasm (`host/web/build.mjs
<crate> --bake` with `EXACT_UPDATE_TRUST=production`), then the JS target over the bake's plan
(`host/web-js/build.mjs <app> --plan <bake>/app.plan --production`). The macOS exact2 apps are ad-hoc-signed
development builds (`EXACT_IDENTITY=-`); Dioxus Native builds with the full Vello renderer (`--features vello`)
because the default one panics (Findings).

## Running

- **Web** (`web/`): `web/series.sh` runs `bench.mjs` — headed Chrome through playwright-core (this checkout's
  `node_modules`), a fresh profile per run, a 420 × 900 CSS px window at DPR 2 — over the `dist/` builds in
  interleaved rounds: load (first rAF with message 0's avatar laid out, LCP, bytes, JS heap, DOM nodes, renderer
  RSS), scroll (`Input.synthesizeScrollGesture` at 3k/6k/12k/24k px/s for 3 s: rAF intervals, main-thread task /
  script / layout / style ms, all Chrome processes' CPU, blank samples on five points of the list's centre line) and
  five taps on a reaction chip (Event Timing duration, count check). Then `progress.mjs` at 12k and 24k px/s: rows
  passed per second by the message at the list's top each frame, and backward steps (a bounded window moves
  `scrollTop` back at a shift, so `px/s` understates it). `loadtime.mjs`, `loadprof.mjs`, `tapprof.mjs` are
  single-page diagnostics (first row and heap; a CPU profile of the load; of the taps). `CHROME` names the browser.
- **macOS** (`mac/`): `mac/series.sh [outdir]` launches each app, places its window at 420 × 900 pt, and
  `macbench` measures from outside the app: wheel events at 120 Hz through a HID tap (the cursor is warped onto the
  window; AppKit ignores wheel events posted to a pid), the window's frames from a ScreenCaptureKit stream at 120 fps,
  white bands ≥ 150 pt, task-info CPU and `phys_footprint`, and cold start (spawn to the first frame with text ink
  below the window's top 140 pt). `ROUNDS=3`, `SPEEDS`, `APPS="exact dioxus"` (also `bounded`); it kills only
  the PIDs it started. The terminal needs Accessibility, post-event and Screen Recording permission (`mac/tcc`
  prints all three); `mac/wins <pid>` lists a process's windows.
- **iPhone** (`device/`): `device/series.sh` (fling-t, fling-b, coldstart; `LADDER=1` for ladder-t/b) with
  `../heavy-list/probe`, `BENCH_DEVICE` and its signing variables.
- **Window shifts** (`native/shiftcheck.mjs`): scrolls the bounded app through its window shifts with the agent's
  wheel on macOS, iOS or the web, and checks the row at the top only moves forward.

Results go to `target/bench/dioxus/results/`. Aggregate:

```sh
python3 bench/dioxus/summarize.py web target/bench/dioxus/results/web-<date>/web.json
python3 bench/dioxus/summarize.py mac target/bench/dioxus/results/mac-r1          # or label=app@dir,dir …
python3 bench/heavy-list/probe/summarize.py target/bench/dioxus/results/device-r1 whole,bounded
python3 bench/heavy-list/probe/agg.py target/bench/dioxus/results/<ladder series>
```

## Prerequisites

macOS with Xcode; Google Chrome; this checkout's toolchains (Bun, Rust with the web nightly that `host/web/build.mjs`
names, `bun install --frozen-lockfile`, which brings playwright-core); the dx CLI above; Python 3 with Pillow
11.3.0 for the data. The macOS runs need the TCC permissions above; the iPhone runs need what `../heavy-list` lists.
Run nothing else heavy on the machine: the published runs had a load average of 37–54 from other sessions, with
rounds interleaved so it landed on every app.

## Findings (2026-10-03/04)

- Dioxus Native's default renderer (vello-hybrid) panics after ~9,000 pt of scrolling:
  `vello_hybrid-0.0.9/src/render/wgpu.rs:596: AtlasLimitReached` (its image atlas never evicts). `vello` works;
  it aborted once more in a later round (a Rust panic in a stripped binary).
- Blitz fidelity: an inline `<span>`'s trailing space is dropped ("expo.dev/blogRender"); no colour-emoji fallback.
- Blitz keeps every decoded image at full size: 1.1–1.5 GB footprint against exact2's 140–180 MB.
- exact2's whole-feed web build ships the feed three times (app.js 6.5 MB + app.plan 7.85 MB + data wasm 7.3 MB =
  21.7 MB raw, 3.4 MB brotli) against Dioxus's 7.85 MB raw / 1.3 MB; a reaction tap cost 80–96 ms to the next paint
  against 16–24 ms (the whole 10,000-row feed answered across the wasm → JS seam per tap). The bounded answer fixes
  both. Landed on exact2 main because of this bench: cf12cb9ae (`app.bind.plan`), ca83e4eaa (a collection's anchor
  correction after its extent shrank; it jumped ~58 rows back at every window shift) and 04c1822ec (the same on the
  Apple host: macOS and the iOS simulator jumped back 55/59 rows at every forward shift).
- JSON.parse for baked values was tried and reverted: first row 143.5 vs 148 ms, +12 MB heap.

## Last standings

From `summarize.py` (web, macOS) and `../heavy-list/probe/summarize.py` (iPhone) over the raw results (not
committed), on a Mac17,6 and an iPhone 13 Pro Max.

**Chrome, whole feed, 2026-10-03, exact2 origin/main 31ba85771** (three rounds, medians):

| app | first row ms | LCP ms | bytes (non-image) | JS heap | renderer RSS | DOM nodes | tap ms (median) |
|---|---|---|---|---|---|---|---|
| exact2 | 147 | 188 | 21.68 MB | 18.6 MB | 480 MB | 390 | 88 |
| Dioxus | 114 | 156 | 7.85 MB | 1.8 MB | 283 MB | 815 | 16 |

| px/s | exact2 fps / late / worst ms / main ms/s / Chrome CPU ms/s | Dioxus fps / late / worst ms / main ms/s / Chrome CPU ms/s |
|---|---|---|
| ~2,840 | 123.1 / 0 / 10.5 / 74 / 327 | 123.1 / 1 / 16.9 / 121 / 331 |
| ~5,690 | 123.4 / 0 / 9.4 / 89 / 397 | 123.2 / 1 / 16.5 / 127 / 355 |
| ~11,360 | 123.4 / 0 / 11.2 / 115 / 426 | 123.1 / 1 / 16.7 / 144 / 405 |
| ~22,730 | 123.1 / 0 / 9.4 / 132 / 493 | 123.5 / 0 / 9.4 / 148 / 569 |

Neither showed a blank sample.

**Chrome, bounded, 2026-10-03, exact2 with cf12cb9ae and ca83e4eaa** (three rounds): first row 45 vs 129 ms, LCP
112 vs 172, 7.54 vs 7.85 MB, JS heap 4.1 vs 1.8 MB, renderer RSS 285 vs 279 MB, taps 24 vs 16 ms (Event Timing
reports only ≥ 16 ms), main thread 54/70/102/138 vs 84/96/132/216 ms/s at the four speeds, no blanks, no late
frames above one per run. (The bounded app's px/s at 12k and 24k reads 1,362 and −2,379 because its `scrollTop`
steps back at each shift; `progress.mjs` measures it by rows.)

**macOS, 2026-10-03, exact2 origin/main 31ba85771** (exact2 standalone vs Dioxus Native; cold n=5 for exact2 and
vello, 3 for hybrid; scroll n=5):

| app | cold first ink ms | footprint at 1.5 s | 3k pt/s fps / late / worst / cpu / MB | 6k | 12k | 24k |
|---|---|---|---|---|---|---|
| exact2 | 311 | 86 MB | 109.3 / 5 / 18.2 / 264 / 143 | 109.0 / 3 / 13.2 / 385 / 175 | 110.0 / 6 / 13.5 / 517 / 182 | 108.3 / 10 / 13.9 / 758 / 160 |
| Dioxus (vello) | 443 | 440 MB | 107.3 / 8 / 34.5 / 383 / 1121 | 109.3 / 6 / 17.1 / 413 / 1299 | 109.3 / 8 / 18.6 / 439 / 1392 | 108.3 / 5 / 16.1 / 499 / 1533 |
| Dioxus (vello-hybrid) | 305 | 295 MB | panics after ~9,000 pt | | | |

A later macOS series with the bounded app (`mac-bounded2`) ran with the display capped at 60 Hz midway (cause
unknown), so only its memory compares: bounded 180–239 MB, whole 311–351 MB, Dioxus 1,123–1,535 MB while scrolling.

**iPhone 13 Pro Max, 2026-10-04, exact2 04c1822ec** (probe; three rounds of fling, two of the ladder; thermal
nominal):

| app | fps | late/s | worst ms | busy/f | cpu ms/s | main ms/s | peak MB | end MB | blanks | cold ms (inserted probe) |
|---|---|---|---|---|---|---|---|---|---|---|
| whole | 117.6 | 2.4 | 25 | 1.85 | 437 | 191 | 102 | 84 | 0 | 151 |
| bounded | 117.4 | 2.3 | 34 | 1.95 | 445 | 199 | 83 | 65 | 0 | 112 |

The fling never leaves the bounded app's first window; the ladder does: through 48k pt/s both held 116–120 fps with
no blanks and full travel; at 96k pt/s both broke (bounded 79/84 fps with blanks), and bounded lost 15% of its
upward travel, clamping at the window's top before `reachstart`'s window arrived.
