# List scrolling benchmark (iOS): Expo `List.ForEach` vs exact2 vs SwiftUI

The "easy" list: Expo PR 49975's demo screen (`DataListForEachScreen`), 10,000 messages in
two sections, built three ways and scrolled by the same injected probe on a phone or a
simulator.

| App | Source | Bundle id |
|---|---|---|
| Expo | `expo/App.tsx`, the PR's demo: `@expo/ui` SwiftUI `List` with two `List.ForEach` | `dev.exact.listbench.expo` |
| exact2 | `exact-listbench/`: one `list virtualized=true`; header, section titles and footers ride in the boundary rows | `dev.exact.listbench.exact` |
| SwiftUI | `swiftui/App.swift`: the most ordinary `List` with `Section`s and `ForEach` | `dev.exact.listbench.swiftui` |

The rows match: an SF Symbol label, a 44 pt bookmark button, the body, "Saved for later",
an attachment chip on every fifth message, swipe to delete, and Edit mode with selection and
drag to reorder.

## What is measured

`probe/probe.m` is a dylib injected at launch (`DYLD_INSERT_LIBRARIES`). After
`BENCH_DELAY` seconds (5) it finds the tallest `UIScrollView` with more than 20,000 pt of
content, moves to a third of the way down, and drives `contentOffset` from a
`CADisplayLink` at the screen's maximum rate. It writes one JSON result to `BENCH_OUT`.

- `fling`: a warm-up at 3,000 pt/s down and up, then 2 s segments at ±1,000, 3,000, 6,000,
  12,000 and 24,000 pt/s. Each frame's interval is recorded against the one the display
  link expected.
- `jump`: ten absolute jumps to fixed offsets; each is timed until three frames in a row
  show no blank band (3 s is a timeout).
- `BENCH_SAMPLE=N` snapshots the list every Nth frame and measures blank bands: runs of
  at least 60 pt with no ink across the row's inner width. Sampling costs frames, so
  timing and blank passes are separate runs. `BENCH_RENDER=layer` snapshots with
  `renderInContext` on the presentation layer (cheap); otherwise
  `drawViewHierarchyInRect` (closer to the screen, ~26 ms a sample).
- `BENCH_DUMP=<dir>` saves up to six blank snapshots as PNGs (relative to Documents on a
  phone).

`agg.py` reports per speed: fps, hitch (ms per second by which frames ran late), p95
frame, and blank samples (`>=250` counts bands of 250 pt or more; `maxPt` is the largest).
Jumps report time to a full viewport. `analyze.py` prints one result file in detail.

`probe/diag.m` is a separate diagnostic: it lists the tall layers under the list and times
`renderInContext` for each, on a simulator: `BENCH_DYLIB=diag bench/list/run.sh <bundle-id> - 0 out.json`.

## Prerequisites

Xcode with the iOS SDK and a simulator; for a phone, one paired with Developer Mode on,
a development signing identity and a development provisioning profile that covers the
three bundle ids (a wildcard one does). The exact2 app needs this checkout's own setup
(`bun install --frozen-lockfile`, the pinned Rust from `rust-toolchain.toml` with the iOS
targets). The Expo app needs Node and CocoaPods. Set `DEVELOPER_DIR` if `xcode-select`
points at the Command Line Tools.

Everything built or recorded goes under `target/bench/list/` (`BENCH_WORK` overrides).

| Variable | Used by | Meaning |
|---|---|---|
| `BENCH_SIM` | `run.sh`, `series.sh` | simulator UDID or name (default `booted`) |
| `BENCH_DEVICE` | `devrun.sh`, `devseries.sh` | phone UDID or name (`xcrun devicectl list devices`) |
| `BENCH_IDENTITY`, `BENCH_TEAM`, `BENCH_PROFILE` | `resign.sh` | signing identity SHA-1, its team id, the `.mobileprovision` path |
| `BENCH_LOCK` | both series | optional command, run as `$BENCH_LOCK take listbench` / `give listbench` around each round |
| `SERIES`, `APPS`, `ROUNDS` | both series | results subdirectory, apps (`expo exact swiftui`), rounds (3) |

## Building

From the repo root. First the probe:

```sh
bench/list/probe/build.sh          # target/bench/list/{probe,diag}-{sim,ios}.dylib
```

**exact2.** The app is its own Cargo workspace, consumed by path like an app `exact new`
made. Its first build needs a copy of the root's lock, which the build then resolves:

```sh
cp Cargo.lock bench/list/exact-listbench/
APP=$PWD/bench/list/exact-listbench
EXACT_APP_DIR=$APP bun host/apple/build.mjs --ios exact-listbench-apple   # installs on a simulator (--sim <udid|name>)
EXACT_APP_DIR=$APP EXACT_IDENTITY=$BENCH_IDENTITY EXACT_PROFILE=$BENCH_PROFILE \
  bun host/apple/build.mjs --device exact-listbench-apple --archive target/bench/list/exact.ipa
(cd target/bench/list && rm -rf exact && mkdir exact && ditto -x -k exact.ipa exact)
bench/list/resign.sh target/bench/list/exact/Payload/List\ Bench.app dev.exact.listbench.exact
xcrun devicectl device install app --device "$BENCH_DEVICE" target/bench/list/exact/Payload/List\ Bench.app
```

**SwiftUI.** `swiftc` with a written Info.plist, no Xcode project:

```sh
bench/list/swiftui/build.sh
xcrun simctl install "$BENCH_SIM" target/bench/list/swiftui/build-sim/ListBench.app
bench/list/resign.sh target/bench/list/swiftui/build/ListBench.app dev.exact.listbench.swiftui
xcrun devicectl device install app --device "$BENCH_DEVICE" target/bench/list/swiftui/build/ListBench.app
```

**Expo.** Expo SDK 58 preview.6, React Native 0.88 rc1, `@expo/ui` 58.0.6 (its
`DataListForEachView.swift` byte-identical to the merged PR). Make the project outside
this repo:

```sh
npx create-expo-app expo-listbench --template blank-typescript
cd expo-listbench
npm install expo@58.0.0-preview.6 react-native@0.88.0-rc.1 @expo/ui@58.0.6   # expo/package.json's pins
npx expo install --fix                     # React and the rest, to the SDK's versions
cp <exact2>/bench/list/expo/App.tsx <exact2>/bench/list/expo/app.json .
npx expo prebuild -p ios --clean
npx expo run:ios --configuration Release   # the simulator
xcodebuild -workspace ios/ExpoListBench.xcworkspace -scheme ExpoListBench -configuration Release \
  -sdk iphoneos -derivedDataPath build CODE_SIGNING_ALLOWED=NO
```

Then, from the exact2 checkout, re-sign and install
`build/Build/Products/Release-iphoneos/ExpoListBench.app` as the others, with the bundle id
`dev.exact.listbench.expo`. Release builds only: a debug build runs JavaScript from Metro.

`resign.sh` must run again whenever the probe changes.

## Running

```sh
bench/list/devseries.sh                    # phone: 3 rounds x 3 apps, apps alternating
SERIES=sim bench/list/series.sh            # simulator, the same passes
python3 bench/list/agg.py target/bench/list/results/dev
python3 bench/list/analyze.py target/bench/list/results/dev/exact-fling-t-1.json
```

One run is `devrun.sh <bundle-id> <fling|jump> <sample> <out.json>` (`run.sh` on a
simulator). A phone run takes about 30 s; keep the phone unlocked and awake (the probe
disables the idle timer, but a sleeping phone drops the devicectl tunnel). The simulator
runs ran at 60 Hz; only a phone measures 120 Hz.

## Last standings

2026-09-25, iPhone 17 Pro Max (120 Hz), exact2 9e06c4ea8, 3 rounds, medians.

| pt/s | Expo fps / hitch / p95 | exact2 | SwiftUI |
|---|---|---|---|
| ±1,000 | 117.5–119.0 / 8–21 / 8.3 | 120.0 / 0 / 8.3 | 120.0 / 0 / 8.3 |
| ±3,000 | 118.0–119.5 / 4–17 / 8.3 | 120.0 / 0 / 8.3 | 119.5–120.0 / 0–4 / 8.3 |
| ±6,000 | 119.0–119.5 / 5–8 / 8.3 | 120.0 / 0 / 8.3 | 120.0 / 0 / 8.3 |
| ±12,000 | 120.0 / 0 / 8.3 | 120.0 / 0 / 8.3 | 120.0 / 0 / 8.3 |
| ±24,000 | 90.2–92.8 / 227–248 / 17.2–17.6 | 120.0 / 0 / 8.3 | 120.0 / 0 / 8.3 |

Blank samples (layer, every 4th frame, summed over runs): none for any app up to
12,000 pt/s; at +24,000 Expo 13 of 135 (12 at 250 pt or more, up to 780 pt) and at
−24,000 45 of 118 (37 severe); exact2 and SwiftUI none.

Time to a full viewport after a jump, p50 / p90 ms (30 jumps): sampled by layer, exact2
6 / 20, SwiftUI 13 / 48, Expo 39 / 67; by view hierarchy, exact2 31 / 64, SwiftUI 34 / 86,
Expo 110 / 144.
