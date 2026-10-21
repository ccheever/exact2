# Crypto list benchmarks (iOS)

A list of 5,000 coins with live prices and a small live chart per row: the "standard crypto exchange" case. Five
apps draw the same rows from the same data: exact2 twice (`exact-crypto-svg/`, inline SVG, and `exact-crypto-gpu/`,
a GPU canvas per row), SwiftUI (`swiftui/`, a plain `List`), UIKit (`uikit/`, a hand-tuned `UICollectionView`) and
Expo (`expo/`, React Native + a Skia canvas per row + LegendList). `SPEC.md` is the screen, the row, the live ticks
and the parity rules ("Decisions" resolves every ambiguity); `GAPS.md` is what exact2 lacked when it was built.
Every app is measured by `bench/heavy-list`'s probe, injected (or linked) into it on a real device.

```sh
bench/crypto-list/prepare.sh                 # the data (gen.py, two seconds)
bench/heavy-list/probe/build.sh              # the probe dylibs
bench/crypto-list/build-exact.sh svg         # exact2 for a device, signed with the probe (and `gpu`)
bench/crypto-list/swiftui/build.sh           # and uikit/build.sh, expo/build.sh
bench/crypto-list/series.sh                  # scroll series on BENCH_DEVICE (SERIES=<name>)
bench/crypto-list/cold-series.sh             # cold start series (linked probe)
python3 bench/heavy-list/probe/summarize.py target/bench/crypto-list/results/<series> swiftui,uikit,svgi
```

## What is measured

`series.sh` runs, per round and app: `fling` (constant-speed 2 s segments at ±1k–24k pt/s) for timing, `fling` in
live mode (`BENCH_LIVE=1`: 40 coins change every 100 ms), `rest` (10 s without scrolling, ticks on: the "idle but
everything is animating" cost), `ladder` (±3k–96k pt/s), `jump` (ten absolute jumps), `coldstart` (probe inserted)
and `fling` with blank sampling (`BENCH_RENDER=layer`). Rounds rotate the app order. The probe and every column of
the table are `../heavy-list`'s (its README has the column meanings; `probe/README.md` the scenarios); `rest` is a
speed-0 row in `probe/agg.py`'s per-speed detail.

`cold-series.sh` is `../heavy-list/cold-series.sh` with this benchmark's bundle ids: the probe linked into a copy of
each app by a load command (`../heavy-list/probe/makecold.sh`, bundle ids `…<app>cold`), launched without
insertion, interleaved rounds; `../heavy-list/coldsum.py` prints its table.

## The apps

| App | Series name | Bundle id | Build | Output |
|---|---|---|---|---|
| exact2, inline SVG | `svgi` | `dev.exact.cryptobench.svgi` | `build-exact.sh svg` | `exact-crypto-svg/build/CryptoExact2SVGI.app`, signed, stamped `crypto-svgi@<commit>` |
| exact2, GPU canvas | `gpu` | `dev.exact.cryptobench.gpu` | `build-exact.sh gpu` | `exact-crypto-gpu/build/CryptoExact2GPU.app`, signed, stamped `crypto-gpu@<commit>` |
| SwiftUI | `swiftui` | `dev.exact.cryptobench.swiftui` | `swiftui/build.sh` | `swiftui/build/CryptoBench.app` (device), `build-sim/` |
| UIKit | `uikit` | `dev.exact.cryptobench.uikit` | `uikit/build.sh` | `uikit/build/CryptoUIKit.app`, `build-sim/` |
| Expo | `expo` | `dev.exact.cryptobench.expo` | `expo/build.sh` | `expo/build/CryptoExpo.app`, `build-sim/` |

Each directory's README says how its app expresses the spec. The exact2 apps are their own Cargo workspaces consumed
by path from this checkout, built by `host/apple/build.mjs` with `EXACT_APP_DIR` set (their READMEs have the
simulator commands). The SVG app is the one the series since 2026-09-30 measure; the GPU app was last measured on
2026-09-29 and has since been ported to current main's GPU module API, unmeasured. The SwiftUI, UIKit and Expo
device builds are unsigned; sign each with the probe, then install:

```sh
P=bench/heavy-list/probe C=bench/crypto-list
$P/resign.sh $C/swiftui/build/CryptoBench.app dev.exact.cryptobench.swiftui
$P/resign.sh $C/uikit/build/CryptoUIKit.app dev.exact.cryptobench.uikit
$P/resign.sh $C/expo/build/CryptoExpo.app dev.exact.cryptobench.expo
for a in $C/{exact-crypto-svg/build/CryptoExact2SVGI,swiftui/build/CryptoBench,uikit/build/CryptoUIKit}.app; do
  xcrun devicectl device install app --device $BENCH_DEVICE $a
done
```

`gen.py` writes `data/coins.json` deterministically (seed 49975); `prepare.sh` runs it, checks it against
`data.sha256` and copies it into both exact2 apps and `expo/` (whose builds refuse or cannot reach files outside
their directories). None of the data is committed. `shoot.sh <app> <name> <delay> [ENV=VAL…]` takes a simulator
screenshot of any installed app; with `BENCH_FREEZE=1` (and `BENCH_START_INDEX=500`) it is a parity shot.

## Running

```sh
export BENCH_DEVICE=<udid> BENCH_SIGN_IDENTITY=<sha1> BENCH_PROFILE=<profile.mobileprovision>
SERIES=ipad-$(git rev-parse --short HEAD) bench/crypto-list/series.sh
SERIES=four APPS="swiftui uikit svgi gpu expo" bench/crypto-list/series.sh
# cold start: linked-probe copies of the signed bundles, then the series
C=target/bench/crypto-list/cold
bench/heavy-list/probe/makecold.sh bench/crypto-list/exact-crypto-svg/build/CryptoExact2SVGI.app dev.exact.cryptobench.svgi $C
#   … the same for swiftui, uikit (and gpu, expo)
bench/crypto-list/cold-series.sh
python3 bench/heavy-list/coldsum.py target/bench/crypto-list/results/cold swiftui,uikit,svgi
```

A series takes about 9 minutes per app per round. Results go to `target/bench/crypto-list/results/<series>/`
(`BENCH_RESULTS` moves them) with a `provenance.txt` and any bench crash logs (`crash/`); a run that printed `ok`
may still have crashed, so read them. `BENCH_DELAY` (default 15 s) is the wait from launch to the first measured
segment. `../heavy-list/probe/tptrace.sh dev.exact.cryptobench.<app> rest <outdir>` records Time Profiler around
one run. The environment variables are `../heavy-list`'s (`BENCH_DEVICE`, `BENCH_SIM`, `BENCH_SIGN_IDENTITY`,
`BENCH_PROFILE` for `dev.exact.cryptobench.*`, `BENCH_TEAM`, `BENCH_LOCK`, `SERIES`, `APPS`, `ROUNDS`,
`START_ROUND`, `ROTATE`, `BENCH_RESULTS`, `BENCH_COLD`, `BENCH_ENV`).

## Prerequisites

As `../heavy-list`: Xcode with an iOS 17+ SDK; a device in Developer Mode, paired, unlocked, Auto-Lock off; the
signing identity and a profile covering `dev.exact.cryptobench.*`; Python 3 (no packages); GNU `timeout`
(`brew install coreutils`); this checkout's toolchains for the exact2 apps (Bun, Rust, `bun install
--frozen-lockfile`). The Expo app also needs Node, npm and CocoaPods.

## Last standings

From `../heavy-list/probe/summarize.py` and `../heavy-list/coldsum.py` over the raw series (not committed).

**iPad Pro M1, 2026-09-30, exact2 origin/main 9fdf7e564** (three rounds; `svgi`; cold ms is the inserted probe's):

| app | fps | live fps | late/s | worst ms | busy/f | cpu ms/s | main ms/s | peak MB | end MB | blanks | ladder <110 | jump p50 | cold ms | rest cpu | rest main |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| swiftui | 90.1 | 63.2 | 8.4 | 89 | 6.98 | 702 | 578 | 33 | 33 | 0 | 12000,12000,12000 | 97 | 208 | 377 | 314 |
| uikit | 120.0 | 120.0 | 0.0 | 8 | 2.48 | 312 | 301 | 29 | 28 | 0 | None,None,None | 15 | 154 | 26 | 25 |
| svgi | 118.1 | 115.3 | 1.1 | 38 | 2.94 | 503 | 289 | 43 | 42 | 0 | -24000,48000,-24000 | 7 | 141 | 69 | 36 |

exact2 led on jump and cold start and was ahead of SwiftUI on everything but memory; it trailed UIKit on late frames, worst
frame, the ladder (UIKit never broke), process CPU (503 vs 312 ms/s), CPU at rest (69 vs 26) and memory (43 vs 29 MB).
Per speed, exact2 held 120 fps to ±12k pt/s and 108–110 at ±24k; SwiftUI fell to 14–15 fps at ±24k.

**iPhone 13 Pro Max, 2026-09-30, exact2 origin/main 9fdf7e564** (three rounds, thermal state nominal throughout):

| app | fps | live fps | late/s | worst ms | busy/f | cpu ms/s | main ms/s | peak MB | end MB | blanks | ladder <110 | jump p50 | cold ms | rest cpu | rest main |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| swiftui | 118.5 | 56.2 | 1.2 | 25 | 4.10 | 501 | 395 | 41 | 41 | 125 | 48000,48000,48000 | 29 | 193 | 510 | 500 |
| uikit | 120.0 | 120.0 | 0.0 | 8 | 1.95 | 247 | 236 | 30 | 23 | 0 | None,None,None | 14 | 157 | 22 | 22 |
| svgi | 118.5 | 114.0 | 1.5 | 25 | 2.81 | 511 | 278 | 35 | 34 | 0 | 48000,48000,48000 | 8 | 118 | 68 | 30 |

**With Expo and the GPU app, 2026-09-29, exact2 origin/main dedbe6e8** (three rounds; `svg` is the first SVG app,
which formatted every changed coin's points string in the data source; the cold ms column is inflated by the probe
of that day and not comparable, see SPEC "Cold start measurement"). iPad Pro M1:

| app | fps | live fps | late/s | worst ms | busy/f | cpu ms/s | main ms/s | peak MB | end MB | blanks | ladder <110 | jump p50 | cold ms | rest cpu | rest main |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| swiftui | 89.0 | 63.5 | 8.5 | 88 | 7.10 | 704 | 580 | 33 | 33 | 3 | 12000,12000,12000 | 98 | 208 | 377 | 315 |
| expo | 105.0 | 101.8 | 14.1 | 27 | 9.43 | 1201 | 433 | 151 | 146 | 107 | 6000,3000,6000 | 68 | 287 | 867 | 388 |
| gpu | 113.8 | 111.4 | 6.2 | 20 | 6.23 | 1069 | 635 | 199 | 175 | 2 | 24000,24000,24000 | 9 | 1572 | 897 | 379 |
| svg | 119.9 | 119.7 | 0.0 | 8 | 3.68 | 480 | 443 | 64 | 63 | 0 | 48000,48000,48000 | 7 | 1541 | 112 | 108 |
| uikit | 120.0 | 120.0 | 0.0 | 8 | 2.45 | 308 | 296 | 41 | 41 | 0 | None,None,None | 15 | 1198 | 24 | 24 |

iPhone 13 Pro Max:

| app | fps | live fps | late/s | worst ms | busy/f | cpu ms/s | main ms/s | peak MB | end MB | blanks | ladder <110 | jump p50 | cold ms | rest cpu | rest main |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| swiftui | 118.6 | 60.0 | 1.2 | 26 | 4.10 | 501 | 397 | 41 | 41 | 105 | 48000,48000,48000 | 29 | 187 | 508 | 500 |
| expo | 118.3 | 117.7 | 1.7 | 25 | 4.33 | 1002 | 354 | 143 | 142 | 47 | 48000,-24000,48000 | 47 | 237 | 701 | 368 |
| gpu | 112.5 | 108.4 | 7.5 | 25 | 4.93 | 870 | 529 | 131 | 113 | 1 | 24000,24000,24000 | 12 | 435 | 753 | 395 |
| svg | 119.2 | 116.4 | 0.8 | 17 | 3.72 | 492 | 444 | 46 | 43 | 0 | -48000,48000,48000 | 12 | 409 | 96 | 92 |
| uikit | 120.0 | 120.0 | 0.0 | 8 | 1.88 | 239 | 228 | 37 | 30 | 0 | None,None,None | 14 | 299 | 22 | 22 |

**Cold start, linked probe, 2026-09-29 evening, exact2 origin/main dedbe6e8** (`gpu` and the first SVG app `svg`;
the `svgi` app has not been measured this way):

| app | iPhone 13 Pro Max ms (3 rounds) | pre-main | busy to ink | iPad Pro M1 ms (2 rounds) | pre-main | busy to ink |
|---|---|---|---|---|---|---|
| swiftui | 194 | 15 | 177 | 219 | 42 | 182 |
| expo | 309 | 49 | 145 | 487 | 182 | 258 |
| gpu | 155 | 24 | 129 | 252 | 55 | 201 |
| svg | 130 | 23 | 105 | 174 | 44 | 133 |
| uikit | 168 | 18 | 143 | 175 | 33 | 153 |
