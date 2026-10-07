# Extra Heavy feed benchmark

A 3,000-row social feed of 19 row kinds, each full of expensive content: a GPU shader over a photo, Canvas 2D,
inline SVG, muted autoplay video, maps, Markdown, ten fonts, nested carousels, GIF/WebP/Lottie, glass over images,
complex scripts, live counters and rings, rows that change height, skeleton shimmer, web views, text inputs, and
two nested virtualized lists (a 2,000-thumbnail horizontal strip every 6th row, a 400 pt inbox of 1,000 messages
every 12th). The same feed is built four ways and driven by the same probe: exact2, SwiftUI, UIKit and Expo
(React Native) on iOS; exact2 and SwiftUI on macOS; exact2 and Expo on the web. `SPEC.md` is the benchmark
(rows, data, clocks, freeze, scenarios, and per-stack Decisions); `mac/SPEC.md` is its macOS and web part.

The probe drives each list from a display link and records frame pacing, run-loop busy time, process and
main-thread CPU, memory, blank bands while scrolling, jumps, cold start and the cost at rest. Scenarios: `fling`
(±1k–24k pt/s), `ladder` (±3k–96k, the first speed under 110 fps), `jump`, `coldstart`, `rest` (10 s at the top,
live ticks on), `live` (`fling` with the 1 Hz clocks running), and for the nested lists `innerfling` and
`innerkeep` (an inner offset kept across the row's retirement).

## Layout

| path | what |
|---|---|
| `SPEC.md`, `mac/SPEC.md` | the benchmark; `EXACT2-GAPS.md` is the record of what exact2 lacked for it (2026-09-27) |
| `gen.py`, `gen_video.swift`, `fetch-fonts.sh`, `fetch-lottie.sh`, `prepare.sh` | the data and every generated input (below) |
| `exact-xheavy/` | the exact2 app, one app for iOS, macOS and the web (its README says how each kind is written) |
| `expo/` | the Expo app, iOS and react-native-web |
| `ios/` | `swiftui/`, `uikit/`, the series, the builder for the exact2 archive, per-kind runs (`kinds/`), shots, trace tools |
| `mac/` | the SwiftUI port, the AppKit probe, the runner and series, the stage script, the checks |
| `web/` | the web pair: builds, the page probe, the server, the Chrome runner |
| `summarize.py` | the per-app table for any series (iOS, macOS or web); `tpsum.py` reads a Time Profiler export |

The iOS probe is shared with the heavy list: `../heavy-list/probe/` (`probe.m`, `build.sh`, `devrun.sh`,
`resign.sh`, `devclean.sh`, `crashcheck.sh`, `analyze.py`). Build it once with `../heavy-list/probe/build.sh`.

## Prerequisites

- Xcode 27 with the Metal Toolchain (`xcodebuild -downloadComponent MetalToolchain`; `ios/*/metal.sh` can use
  another Mac's over ssh instead: `BENCH_METAL_HOST`), Python 3 with Pillow 11.3.0 (the checksums assume it),
  `timeout` (Homebrew coreutils), Bun and this checkout's `bun install --frozen-lockfile`, Node/npm for Expo
  (Expo SDK 58 preview, CocoaPods), Google Chrome for the web pair.
- A device: an iPad Pro M1 12.9" (landscape, 120 Hz) and an iPhone 13 Pro Max were the published ones. It must
  be unlocked, paired and in Developer Mode; `xcrun devicectl list devices` gives its UDID.
- Signing for device runs: a development identity and a provisioning profile that covers `dev.exact.xheavy.*`.

| variable | used by | meaning |
|---|---|---|
| `BENCH_DEVICE` | iOS scripts, the probe | the device UDID (required) |
| `BENCH_SIGN_IDENTITY`, `BENCH_PROFILE`, `BENCH_TEAM` | `resign.sh`, `ios/build-exact.sh` | signing identity SHA-1, `.mobileprovision`, team id (read from the profile when unset) |
| `BENCH_LOCK` | iOS series | optional command run as `$BENCH_LOCK take <name>` / `give <name>` around each hold of a shared device |
| `BENCH_SIM` | `ios/shoot-sim.sh` | a simulator UDID |
| `BENCH_RESULTS` | series | results root (default `<checkout>/target/bench/extra-heavy/results`; the web pair's default is `web/results`) |
| `BENCH_METAL_HOST` | `ios/*/metal.sh` | an ssh host with the Metal Toolchain, when this Mac has none |
| `BENCH_STAGE`, `BENCH_HOST` | `mac/push.sh`, `mac/run/*` | the bench Mac's stage directory (default `~/xhm`) and its ssh host |
| `BENCH_CHROME` | `web/` | Chrome's executable, when not in /Applications |

## Prepare

```sh
bench/extra-heavy/prepare.sh
```

It fetches the fonts (open licences, `fonts.sha256`), runs `gen.py` (seed 49975: `data/feed.json`, photos,
avatars, GIF/WebP/Lottie, SVGs, the web-view template; the four videos by `gen_video.swift`, about a minute),
checks `data/` against `data.sha256` (Pillow 11.3.0 reproduces every byte; video bytes depend on the encoder, so
they are not listed), writes the exact2 app's assets, `svgs.contract`, `rings.contract`, icon and `Cargo.lock`
(the checkout's, copied every time), the Expo app's copy of the data, and fetches lottie-ios at the measured
commit. Nothing it writes is committed.

## iOS

Build and sign each app (from the checkout's root; `P=bench/heavy-list/probe`, `X=bench/extra-heavy`):

```sh
$X/ios/build-exact.sh main                                   # → $X/device/exact2-main/app/Payload/<App>.app, signed, stamped
$X/ios/swiftui/build.sh && cp -R $X/ios/swiftui/build/XHeavy.app $X/device/ && $P/resign.sh $X/device/XHeavy.app dev.exact.xheavy.swiftui
$X/ios/uikit/build.sh && cp -R $X/ios/uikit/build/XHeavyUIKit.app $X/device/ && $P/resign.sh $X/device/XHeavyUIKit.app dev.exact.xheavy.uikit
# Expo: expo/README.md "Build", then copy the .app to $X/device/XHeavyExpo.app and resign it as dev.exact.xheavy.expo
```

`SIM=1` on the SwiftUI and UIKit builds adds a simulator build; `EXACT_APP_DIR=$PWD/$X/exact-xheavy bun
host/apple/build.mjs --ios exact-xheavy-apple --run` runs the exact2 app on a simulator. Then the series:

```sh
export BENCH_DEVICE=<udid>
INSTALL="$(ls -d $X/device/exact2-main/app/Payload/*.app) $X/device/XHeavy.app $X/device/XHeavyExpo.app $X/device/XHeavyUIKit.app" \
  SERIES=main-19-ipad $X/ios/series.sh                      # 3 rounds × 4 apps × 10 scenarios, about 3 hours
python3 $X/summarize.py target/bench/extra-heavy/results/main-19-ipad
```

`APPS` picks apps, `BENCH_KINDS=17` leaves the nested lists out, `LOCK_PER_APP=1` holds the device per app,
`FROM`/`SKIP_DONE` resume. Each result carries the build's stamp, the probe's thermal state and Low Power Mode;
read the crash logs the series copies into `crash/` before trusting it. Other scripts: `ios/full.sh` (the
series, then the per-kind breakdown), `ios/kinds/run.sh <round> <kind>…` with `agg.py` and `table.py` (one kind
per feed), `ios/kinds/traces.sh <kind>…` (Time Profiler per kind; `tpk.py`, `tpbin.py`, `tpup.py`,
`ios/tools/tptree.py`, `ios/tools/systrace.py` read the exports), `ios/shoot.sh <bundle-id> <name> 3 7 500`
(frozen screenshots for parity), `ios/dbgrun.sh` (one run with the app's console), `ios/keepdump.sh` (innerkeep
with the anchor images). `ios/build-exact.sh <label> <suffix>` builds another checkout (`EXACT2=<path>`) under
another bundle id for an A/B.

## macOS

Build here, run on a bench Mac whose display is awake (`mac/SPEC.md`, "Machines and their limits"):

```sh
$X/mac/probe/build.sh; $X/mac/swiftui/build.sh; $X/mac/build-exact.sh ""   # dist/main.app
$X/mac/push.sh swiftui <host>; $X/mac/push.sh main <host>                  # → <host>:~/xhm/apps/{XHeavy,exact2}.app
ssh <host> 'cd ~/xhm && SERIES=main-r1 APPS="swiftui exact2" ./series-m2.sh'
rsync -a <host>:xhm/results/main-r1 target/bench/extra-heavy/results/
python3 $X/mac/check-m2.py target/bench/extra-heavy/results/main-r1 swiftui,exact2 && python3 $X/summarize.py target/bench/extra-heavy/results/main-r1 swiftui,exact2
```

Without a host, `push.sh` stages on this Mac. `series-m2.sh` alternates the order each round and writes
provenance; `machine.state` in the stage (example in `mac/run/mstate/`) holds each run to a CPU-speed floor.
`mac/abtable.py` reads an A/B, `mac/byspeed.py` the fling by speed, `mac/kinds.py` a `PART=kinds` series of
`run/series.sh`; `run/trace2.sh` records every thread and `run/tp/tpx.py` reads it.

## Web

```sh
WEB_FLAGS=--wasm $X/web/build.sh                     # dist/exact (exact2's web build), dist/expo (Expo's web export)
SERIES=web-r1 $X/web/series.sh                       # in Chrome, 1366 × 940; or rsync web/ to a bench Mac and run it there
python3 $X/summarize.py $X/web/results/web-r1 exact2,expo
```

The page probe (`web/probe.js`, injected by `server.py`) drives the feed's `scrollTop` from requestAnimationFrame;
CPU and RSS are the Chrome process tree's, summed with `ps`. No live tick and no per-kind runs on the web (the
exact2 web build reads no `BENCH_*` environment).

## Last standings

Medians of 3 interleaved rounds, 19 kinds. fps and late frames are whole-run; cpu, main and rest are ms/s.

iPad Pro M1, iOS 27.2, 120 Hz, 2026-09-30, exact2 origin/main `7647f6d78` (SwiftUI and UIKit from the same day's
board):

| app | fps | late/s | worst ms | busy ms/f | cpu | main | peak MB | blanks | jump p50 ms | cold ms | rest cpu / main |
|---|---|---|---|---|---|---|---|---|---|---|---|
| exact2 | 117.6 | 1.1 | 95 | 2.76 | 616 | 267 | 313 | 54 | 11 | 188 | 239 / 98 |
| UIKit | 113.0 | 4.1 | 112 | 3.07 | 400 | 244 | 479 | 126 | 27 | 180 | 164 / 57 |
| SwiftUI | 106.5 | 8.4 | 119 | 4.41 | 621 | 423 | 314 | 89 | 51 | 222 | 405 / 380 |

iPhone 13 Pro Max, 120 Hz, 2026-09-30, same builds (thermal state 0–1, recorded per result; SwiftUI and UIKit
have 2 of 3 rounds):

| app | fps | late/s | worst ms | busy ms/f | cpu | main | peak MB | blanks | jump p50 ms | cold ms | rest cpu / main |
|---|---|---|---|---|---|---|---|---|---|---|---|
| exact2 | 115.1 | 3.0 | 50 | 2.38 | 587 | 219 | 244 | 489 | 37 | 187 | 83 / 46 |
| UIKit | 112.4 | 5.2 | 69 | 2.39 | 409 | 205 | 533 | 321 | 31 | 191 | 46 / 18 |
| SwiftUI | 103.1 | 10.2 | 100 | 3.72 | 543 | 352 | 275 | 466 | 46 | 210 | 364 / 353 |

exact2 led on frame rate, late and worst frames, the ladder (24k–48k pt/s against 3k–12k), blanks on the iPad and
jumps there; it lost process CPU and the cost at rest to UIKit. Expo, on the iPad in the four-way series of the
same night (exact2 `796b201e9`): 86.1 fps, 13.2 late/s, cpu 1044, 860 blanks, cold 295 ms.

macOS, Mac mini M4, 60 Hz 1x display, 2026-09-30, fling only, exact2 at the mac lane's tip that landed as
`706cc0022`: exact2 59.5 fps, 6.94 busy ms/frame, cpu 585, main 349, peak 803 MB; SwiftUI 56.0, 6.79, 549, 345,
1040 MB; rest 147 / 63 against 400 / 326. 120 Hz on macOS is unmeasured (`mac/SPEC.md`).

Web, the same Mac mini, Chrome 154, 60 Hz, 2026-09-28, exact2 `558a5bd8` (the wasm target): exact2 57.8 fps,
0.1 late/s, 35 ms worst, 0 blanks, cold 187 ms, rest 364 / 158; Expo 57.0, 1.0, 50 ms, 9 blanks, cold 212 ms,
rest 546 / 214. Chrome's per-page memory: exact2 82 MB, Expo 153 MB.

## Not here

The original harness's results, screenshots, traces and built bundles; the lane experiments (per-commit copies of
the exact2 app used for A/Bs, the rest-lane and hook measurements, the GPU and map-reuse probes); the old device
locks and board scripts; the predecessor web copy of the exact2 app and the Expo app (both folded into the one
app each here). The apps' sources are as measured, with these changes: paths into this checkout, the exact2 app
renamed (`exact-xheavy`, crates `exact-xheavy-*`) with its Apple entry in today's link-by-use shape and the
Contract `writes` clauses current main refuses removed, the web build's no-thread fallbacks folded in, and the
Expo web shims folded into the iOS source (`expo/README.md`).
